use std::fs;
use std::io;
use std::path::PathBuf;
use std::process::{Command, Stdio};
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
    use super::{Connection, applescript_escape, is_confirmation_prompt};

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
}
