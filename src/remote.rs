use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

const KEYCHAIN_SERVICE: &str = "dev.mdiew.ssh";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct Connection {
    pub id: String,
    pub nickname: String,
    pub host: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub username: Option<String>,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub identity_file: Option<String>,
    #[serde(default = "default_directory")]
    pub last_directory: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryEntry {
    pub name: String,
    pub path: String,
    pub is_directory: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DirectoryListing {
    pub path: String,
    pub entries: Vec<DirectoryEntry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteDocument {
    pub connection_id: String,
    pub remote_path: String,
    pub cached_path: PathBuf,
}

#[derive(Default, Deserialize, Serialize)]
struct ConnectionStore {
    #[serde(default)]
    connections: Vec<Connection>,
}

impl Connection {
    pub fn new(
        nickname: String,
        host: String,
        username: Option<String>,
        port: u16,
        identity_file: Option<String>,
    ) -> Result<Self, String> {
        let host = host.trim().to_string();
        if host.is_empty() {
            return Err("Host is required".to_string());
        }
        if port == 0 {
            return Err("Port must be between 1 and 65535".to_string());
        }

        let username = cleaned_optional(username);
        let identity_file = cleaned_optional(identity_file);
        let nickname = if nickname.trim().is_empty() {
            host.clone()
        } else {
            nickname.trim().to_string()
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        Ok(Self {
            id: format!("connection-{timestamp:x}"),
            nickname,
            host,
            username,
            port,
            identity_file,
            last_directory: default_directory(),
        })
    }

    pub fn destination(&self) -> String {
        match &self.username {
            Some(username) => format!("{username}@{}", self.host),
            None => self.host.clone(),
        }
    }
}

pub fn load_connections() -> Vec<Connection> {
    let Ok(contents) = fs::read_to_string(settings_path()) else {
        return Vec::new();
    };
    serde_json::from_str::<ConnectionStore>(&contents)
        .map(|store| store.connections)
        .unwrap_or_default()
}

pub fn save_connection(connection: Connection) -> io::Result<()> {
    let mut connections = load_connections();
    if let Some(existing) = connections
        .iter_mut()
        .find(|existing| existing.id == connection.id)
    {
        *existing = connection;
    } else {
        connections.push(connection);
    }
    write_store(connections)
}

pub fn remember_directory(connection_id: &str, directory: &str) -> io::Result<()> {
    let mut connections = load_connections();
    if let Some(connection) = connections
        .iter_mut()
        .find(|connection| connection.id == connection_id)
    {
        connection.last_directory = directory.to_string();
    }
    write_store(connections)
}

pub fn remove_connection(connection_id: &str) -> io::Result<()> {
    let mut connections = load_connections();
    connections.retain(|connection| connection.id != connection_id);
    delete_keychain_secret(connection_id, "password");
    delete_keychain_secret(connection_id, "passphrase");
    write_store(connections)
}

pub fn find_connection(connection_id: &str) -> Option<Connection> {
    load_connections()
        .into_iter()
        .find(|connection| connection.id == connection_id)
}

pub fn run_askpass() -> i32 {
    let prompt = std::env::args().nth(2).unwrap_or_default();
    let connection_id = std::env::var("MDIEW_SSH_CONNECTION_ID").unwrap_or_default();
    if connection_id.is_empty() {
        return 1;
    }

    if is_confirmation_prompt(&prompt) {
        return match ask_confirmation(&prompt) {
            Some(answer) => {
                println!("{answer}");
                0
            }
            None => 1,
        };
    }

    let kind = if prompt.to_ascii_lowercase().contains("passphrase") {
        "passphrase"
    } else {
        "password"
    };
    if let Some(secret) = read_keychain_secret(&connection_id, kind) {
        println!("{secret}");
        return 0;
    }

    match ask_secret(&prompt) {
        Some(secret) => {
            store_keychain_secret(&connection_id, kind, &secret);
            println!("{secret}");
            0
        }
        None => 1,
    }
}

pub fn configure_ssh_command(command: &mut Command, connection: &Connection) {
    command
        .arg("-o")
        .arg("ConnectTimeout=12")
        .arg("-o")
        .arg("ServerAliveInterval=5")
        .arg("-o")
        .arg("ServerAliveCountMax=1")
        .arg("-o")
        .arg("ControlMaster=no")
        .arg("-o")
        .arg("ControlPath=none")
        .arg("-p")
        .arg(connection.port.to_string());

    if let Some(identity_file) = &connection.identity_file {
        command.arg("-i").arg(expand_home(identity_file));
    }

    if let Ok(executable) = std::env::current_exe() {
        command
            .env("SSH_ASKPASS", executable)
            .env("SSH_ASKPASS_REQUIRE", "force")
            .env("DISPLAY", "mdiew")
            .env("MDIEW_SSH_CONNECTION_ID", &connection.id)
            .stdin(Stdio::null());
    }
}

pub fn clear_rejected_credentials(connection_id: &str) {
    delete_keychain_secret(connection_id, "password");
    delete_keychain_secret(connection_id, "passphrase");
}

pub fn list_directory(
    connection: &Connection,
    requested_path: &str,
) -> Result<DirectoryListing, String> {
    let requested_path = if requested_path.trim().is_empty() {
        "~"
    } else {
        requested_path.trim()
    };
    let quoted_path = shell_quote(requested_path);
    let remote_command = format!(
        "requested={quoted_path}; case \"$requested\" in '~') requested=\"$HOME\" ;; '~/'*) requested=\"$HOME/${{requested#~/}}\" ;; esac; cd -- \"$requested\" 2>/dev/null || exit 44; printf '%s\\0' \"$PWD\"; find . -mindepth 1 -maxdepth 1 -type d -print0 2>/dev/null; printf '\\0'; find . -mindepth 1 -maxdepth 1 ! -type d -print0 2>/dev/null"
    );
    let output = run_ssh(connection, &remote_command)?;
    parse_directory_listing(&output.stdout)
}

pub fn download_file(connection: &Connection, remote_path: &str) -> Result<RemoteDocument, String> {
    let remote_command = format!("cat -- {}", shell_quote(remote_path));
    let output = run_ssh(connection, &remote_command)?;
    let cached_path = cache_path(connection, remote_path);
    if let Some(parent) = cached_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let temporary = cached_path.with_extension("tmp");
    fs::write(&temporary, output.stdout).map_err(|error| error.to_string())?;
    fs::rename(temporary, &cached_path).map_err(|error| error.to_string())?;

    Ok(RemoteDocument {
        connection_id: connection.id.clone(),
        remote_path: remote_path.to_string(),
        cached_path,
    })
}

pub fn refresh_document(document: &RemoteDocument) -> Result<RemoteDocument, String> {
    let connection = find_connection(&document.connection_id)
        .ok_or_else(|| "The saved remote connection no longer exists".to_string())?;
    download_file(&connection, &document.remote_path)
}

pub fn is_markdown_file(path: &str) -> bool {
    Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "md" | "markdown" | "mdown" | "mkd" | "mkdn"
            )
        })
}

pub fn parent_directory(path: &str) -> String {
    if path == "/" {
        return "/".to_string();
    }
    Path::new(path)
        .parent()
        .and_then(Path::to_str)
        .filter(|parent| !parent.is_empty())
        .unwrap_or("/")
        .to_string()
}

fn run_ssh(connection: &Connection, remote_command: &str) -> Result<Output, String> {
    let mut command = Command::new("/usr/bin/ssh");
    configure_ssh_command(&mut command, connection);
    let output = command
        .arg(connection.destination())
        .arg(remote_command)
        .output()
        .map_err(|error| format!("Could not start SSH: {error}"))?;

    if output.status.success() {
        return Ok(output);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if stderr.to_ascii_lowercase().contains("permission denied") {
        clear_rejected_credentials(&connection.id);
        return Err(
            "Authentication failed. The rejected saved credential was removed; try again."
                .to_string(),
        );
    }
    if output.status.code() == Some(44) {
        return Err("That remote directory does not exist or cannot be opened".to_string());
    }
    if stderr.is_empty() {
        Err("The remote SSH command failed".to_string())
    } else {
        Err(stderr)
    }
}

fn parse_directory_listing(bytes: &[u8]) -> Result<DirectoryListing, String> {
    let mut parts = bytes.split(|byte| *byte == 0);
    let path = parts
        .next()
        .filter(|path| !path.is_empty())
        .map(|path| String::from_utf8_lossy(path).to_string())
        .ok_or_else(|| "The remote directory returned no path".to_string())?;
    let mut entries = Vec::new();
    let mut is_directory = true;

    for part in parts {
        if part.is_empty() {
            if is_directory {
                is_directory = false;
            }
            continue;
        }
        let relative_path = String::from_utf8_lossy(part);
        let name = relative_path.strip_prefix("./").unwrap_or(&relative_path);
        let entry_path = if path == "/" {
            format!("/{name}")
        } else {
            format!("{path}/{name}")
        };
        entries.push(DirectoryEntry {
            name: name.to_string(),
            path: entry_path,
            is_directory,
        });
    }

    entries.sort_by(|left, right| {
        right
            .is_directory
            .cmp(&left.is_directory)
            .then_with(|| left.name.to_lowercase().cmp(&right.name.to_lowercase()))
    });
    Ok(DirectoryListing { path, entries })
}

fn cache_path(connection: &Connection, remote_path: &str) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    connection.id.hash(&mut hasher);
    remote_path.hash(&mut hasher);
    let hash = hasher.finish();
    let file_name = Path::new(remote_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("remote.md");
    home_directory()
        .join("Library")
        .join("Caches")
        .join("mdiew")
        .join("remote-files")
        .join(&connection.id)
        .join(format!("{hash:016x}-{file_name}"))
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn write_store(connections: Vec<Connection>) -> io::Result<()> {
    let path = settings_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let temporary = path.with_extension("json.tmp");
    let contents =
        serde_json::to_vec_pretty(&ConnectionStore { connections }).map_err(io::Error::other)?;
    fs::write(&temporary, contents)?;
    fs::rename(temporary, path)
}

fn settings_path() -> PathBuf {
    home_directory()
        .join("Library")
        .join("Application Support")
        .join("mdiew")
        .join("remote-connections.json")
}

fn home_directory() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return home_directory();
    }
    if let Some(remainder) = path.strip_prefix("~/") {
        return home_directory().join(remainder);
    }
    PathBuf::from(path)
}

fn keychain_account(connection_id: &str, kind: &str) -> String {
    format!("{connection_id}:{kind}")
}

fn read_keychain_secret(connection_id: &str, kind: &str) -> Option<String> {
    let output = Command::new("/usr/bin/security")
        .args([
            "find-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            &keychain_account(connection_id, kind),
            "-w",
        ])
        .output()
        .ok()?;
    output.status.success().then(|| {
        String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string()
    })
}

fn store_keychain_secret(connection_id: &str, kind: &str, secret: &str) {
    let _ = Command::new("/usr/bin/security")
        .args([
            "add-generic-password",
            "-U",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            &keychain_account(connection_id, kind),
            "-w",
            secret,
        ])
        .status();
}

fn delete_keychain_secret(connection_id: &str, kind: &str) {
    let _ = Command::new("/usr/bin/security")
        .args([
            "delete-generic-password",
            "-s",
            KEYCHAIN_SERVICE,
            "-a",
            &keychain_account(connection_id, kind),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn ask_secret(prompt: &str) -> Option<String> {
    let script = format!(
        "text returned of (display dialog \"{}\" default answer \"\" with hidden answer buttons {{\"Cancel\", \"Continue\"}} default button \"Continue\" with title \"mdiew Remote Connection\")",
        applescript_escape(prompt)
    );
    run_applescript(&script)
}

fn ask_confirmation(prompt: &str) -> Option<String> {
    let script = format!(
        "button returned of (display dialog \"{}\" buttons {{\"No\", \"Yes\"}} default button \"Yes\" with title \"mdiew Remote Connection\")",
        applescript_escape(prompt)
    );
    run_applescript(&script).map(|answer| answer.to_ascii_lowercase())
}

fn run_applescript(script: &str) -> Option<String> {
    let output = Command::new("/usr/bin/osascript")
        .args(["-e", script])
        .output()
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn applescript_escape(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}

fn is_confirmation_prompt(prompt: &str) -> bool {
    let prompt = prompt.to_ascii_lowercase();
    prompt.contains("yes/no") || prompt.contains("are you sure")
}

fn cleaned_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let value = value.trim().to_string();
        (!value.is_empty()).then_some(value)
    })
}

fn default_port() -> u16 {
    22
}

fn default_directory() -> String {
    "~".to_string()
}

#[cfg(test)]
mod tests {
    use super::{
        Connection, applescript_escape, is_confirmation_prompt, is_markdown_file, parent_directory,
        parse_directory_listing, shell_quote,
    };

    #[test]
    fn connection_uses_host_as_default_name() {
        let connection = Connection::new(
            "".to_string(),
            "example.com".to_string(),
            Some("me".to_string()),
            22,
            None,
        )
        .unwrap();

        assert_eq!(connection.nickname, "example.com");
        assert_eq!(connection.destination(), "me@example.com");
        assert_eq!(connection.last_directory, "~");
    }

    #[test]
    fn rejects_empty_hosts() {
        assert!(Connection::new("Name".into(), "  ".into(), None, 22, None).is_err());
    }

    #[test]
    fn escapes_askpass_dialog_text() {
        assert_eq!(applescript_escape("a \\\" b"), "a \\\\\\\" b");
        assert!(is_confirmation_prompt(
            "Continue connecting (yes/no/[fingerprint])?"
        ));
    }

    #[test]
    fn quotes_remote_shell_paths() {
        assert_eq!(shell_quote("/tmp/it's here"), "'/tmp/it'\\''s here'");
    }

    #[test]
    fn parses_and_sorts_remote_entries() {
        let listing =
            parse_directory_listing(b"/home/me\0./zeta\0./Alpha\0\0./notes.md\0./draft.txt\0")
                .unwrap();

        assert_eq!(listing.path, "/home/me");
        assert_eq!(listing.entries[0].name, "Alpha");
        assert!(listing.entries[0].is_directory);
        assert_eq!(listing.entries[2].name, "draft.txt");
    }

    #[test]
    fn recognizes_markdown_and_parent_paths() {
        assert!(is_markdown_file("README.MD"));
        assert!(!is_markdown_file("notes.txt"));
        assert_eq!(parent_directory("/home/me/docs"), "/home/me");
        assert_eq!(parent_directory("/"), "/");
    }
}
