use url::form_urlencoded;

use crate::remote::{self, Connection, DirectoryListing};

pub fn connections_page(error: Option<&str>) -> String {
    let connections = remote::load_connections();
    let connection_cards = if connections.is_empty() {
        "<div class=\"empty\">No remote servers yet. Add an SSH server to get started.</div>"
            .to_string()
    } else {
        connections
            .iter()
            .map(connection_card)
            .collect::<Vec<_>>()
            .join("")
    };
    let error = error.map(error_banner).unwrap_or_default();

    shell(
        "Open Remote File",
        &format!(
            r#"
<main class="picker wide">
  <header class="page-header">
    <div>
      <div class="eyebrow">REMOTE FILES</div>
      <h1>Open from SSH</h1>
      <p>Connections are saved locally. SSH only reconnects while browsing or opening a file.</p>
    </div>
    <a class="button secondary" href="mdiew://cancel">Cancel</a>
  </header>
  {error}
  <section>
    <h2>Saved servers</h2>
    <div class="connections">{connection_cards}</div>
  </section>
  <section class="new-connection">
    <h2>Connect new server</h2>
    <form action="mdiew://add" method="get">
      <div class="field-grid">
        <label><span>Display name</span><input name="nickname" placeholder="Build server"></label>
        <label><span>Host or SSH alias</span><input name="host" placeholder="server.example.com" required></label>
        <label><span>Username</span><input name="username" placeholder="Uses SSH config if empty"></label>
        <label><span>Port</span><input name="port" type="number" min="1" max="65535" value="22" required></label>
        <label class="full"><span>Identity file</span><input name="identity_file" placeholder="Optional, e.g. ~/.ssh/id_ed25519"></label>
      </div>
      <div class="form-footer">
        <span>Passwords and key passphrases are stored in macOS Keychain.</span>
        <button class="button primary" type="submit">Connect</button>
      </div>
    </form>
  </section>
</main>"#
        ),
    )
}

pub fn directory_page(connection: &Connection, listing: &DirectoryListing) -> String {
    let entries = if listing.entries.is_empty() {
        "<div class=\"empty\">This directory is empty.</div>".to_string()
    } else {
        listing
            .entries
            .iter()
            .map(|entry| {
                if entry.is_directory {
                    format!(
                        "<a class=\"entry enabled\" href=\"{}\"><span class=\"icon folder\">◆</span><span>{}</span><span class=\"chevron\">›</span></a>",
                        action_url(
                            "browse",
                            &[("id", &connection.id), ("path", &entry.path)]
                        ),
                        html_escape(&entry.name)
                    )
                } else if remote::is_markdown_file(&entry.path) {
                    format!(
                        "<a class=\"entry enabled\" href=\"{}\"><span class=\"icon file\">M↓</span><span>{}</span><span class=\"file-action\">Open</span></a>",
                        action_url(
                            "open",
                            &[("id", &connection.id), ("path", &entry.path)]
                        ),
                        html_escape(&entry.name)
                    )
                } else {
                    format!(
                        "<div class=\"entry disabled\"><span class=\"icon file\">·</span><span>{}</span><span class=\"file-action\">Not Markdown</span></div>",
                        html_escape(&entry.name)
                    )
                }
            })
            .collect::<Vec<_>>()
            .join("")
    };
    let parent = remote::parent_directory(&listing.path);

    shell(
        "Choose Remote Markdown File",
        &format!(
            r#"
<main class="picker">
  <header class="browser-header">
    <a class="back" href="mdiew://connections">‹ Servers</a>
    <div class="server-title"><strong>{nickname}</strong><span>{destination}</span></div>
    <a class="button secondary" href="mdiew://cancel">Cancel</a>
  </header>
  <form class="path-bar" action="mdiew://browse" method="get">
    <input type="hidden" name="id" value="{id}">
    <a class="up" title="Parent directory" href="{parent_url}">↑</a>
    <input name="path" value="{path}" aria-label="Remote directory path">
    <button type="submit">Go</button>
  </form>
  <div class="entry-list">{entries}</div>
  <footer class="browser-footer">Showing folders and files. Markdown files can be opened.</footer>
</main>"#,
            nickname = html_escape(&connection.nickname),
            destination = html_escape(&connection.destination()),
            id = html_escape(&connection.id),
            parent_url = action_url("browse", &[("id", &connection.id), ("path", &parent)]),
            path = html_escape(&listing.path),
        ),
    )
}

pub fn error_page(message: &str, retry_url: &str) -> String {
    shell(
        "Remote Connection Error",
        &format!(
            r#"
<main class="picker error-page">
  <div class="error-symbol">!</div>
  <h1>Couldn’t open the remote location</h1>
  <p>{}</p>
  <div class="error-actions">
    <a class="button secondary" href="mdiew://connections">Saved servers</a>
    <a class="button primary" href="{}">Try again</a>
  </div>
</main>"#,
            html_escape(message),
            html_escape(retry_url)
        ),
    )
}

pub fn action_url(action: &str, parameters: &[(&str, &str)]) -> String {
    let query = form_urlencoded::Serializer::new(String::new())
        .extend_pairs(parameters.iter().copied())
        .finish();
    if query.is_empty() {
        format!("mdiew://{action}")
    } else {
        format!("mdiew://{action}?{query}")
    }
}

fn connection_card(connection: &Connection) -> String {
    let connect_url = action_url("connect", &[("id", &connection.id)]);
    let remove_url = action_url("remove", &[("id", &connection.id)]);
    let identity = connection
        .identity_file
        .as_deref()
        .map(|path| format!("<span>Key: {}</span>", html_escape(path)))
        .unwrap_or_default();
    format!(
        r#"<div class="connection-card">
  <a class="connection-main" href="{connect_url}">
    <span class="server-icon">⌁</span>
    <span class="connection-copy">
      <strong>{nickname}</strong>
      <span>{destination} · port {port}</span>
      <span>Last folder: {directory}</span>
      {identity}
    </span>
    <span class="chevron">›</span>
  </a>
  <a class="remove" href="{remove_url}" onclick="return confirm('Remove this saved server and its Keychain credentials?')">Remove</a>
</div>"#,
        nickname = html_escape(&connection.nickname),
        destination = html_escape(&connection.destination()),
        port = connection.port,
        directory = html_escape(&connection.last_directory),
    )
}

fn error_banner(message: &str) -> String {
    format!(
        "<div class=\"error-banner\"><strong>Connection not saved.</strong><span>{}</span></div>",
        html_escape(message)
    )
}

fn shell(title: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>{}</title><style>{}</style></head><body>{}</body></html>"#,
        html_escape(title),
        PICKER_CSS,
        body
    )
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const PICKER_CSS: &str = r#"
:root{color-scheme:light dark;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text",sans-serif;background:#f5f5f7;color:#1d1d1f}*{box-sizing:border-box}body{margin:0;min-height:100vh;background:radial-gradient(circle at top,#fff 0,#f5f5f7 55%)}a{color:inherit;text-decoration:none}.picker{width:min(820px,calc(100% - 40px));margin:34px auto;background:rgba(255,255,255,.86);border:1px solid rgba(0,0,0,.1);border-radius:16px;box-shadow:0 18px 60px rgba(0,0,0,.12);overflow:hidden}.picker.wide{width:min(940px,calc(100% - 40px));padding:28px}.page-header,.browser-header{display:flex;align-items:center;justify-content:space-between;gap:20px}.page-header{margin-bottom:28px}.page-header h1{font-size:30px;margin:3px 0 7px}.page-header p{margin:0;color:#68686d}.eyebrow{font-size:11px;letter-spacing:.12em;color:#6c5ce7;font-weight:700}.button{display:inline-flex;align-items:center;justify-content:center;border-radius:8px;padding:9px 15px;border:1px solid rgba(0,0,0,.13);font-size:13px;font-weight:600;white-space:nowrap}.button.primary{background:#6c5ce7;color:#fff;border-color:#6c5ce7}.button.secondary{background:rgba(255,255,255,.72)}h2{font-size:14px;margin:0 0 11px}.connections{display:grid;grid-template-columns:repeat(auto-fit,minmax(330px,1fr));gap:10px}.connection-card{position:relative;border:1px solid rgba(0,0,0,.1);background:rgba(255,255,255,.72);border-radius:11px;overflow:hidden}.connection-main{display:flex;align-items:center;gap:12px;padding:14px 58px 14px 14px;min-height:92px}.connection-main:hover{background:rgba(108,92,231,.08)}.server-icon{display:grid;place-items:center;width:38px;height:38px;border-radius:9px;background:rgba(108,92,231,.13);color:#6c5ce7;font-size:24px}.connection-copy{display:flex;flex-direction:column;gap:3px;min-width:0}.connection-copy strong{font-size:14px}.connection-copy span{font-size:11px;color:#707077;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.chevron{margin-left:auto;color:#999;font-size:25px}.remove{position:absolute;right:10px;bottom:8px;color:#9a3940;font-size:11px;padding:5px}.new-connection{margin-top:26px;padding-top:24px;border-top:1px solid rgba(0,0,0,.09)}.new-connection form{border:1px solid rgba(0,0,0,.1);background:rgba(255,255,255,.66);border-radius:11px;padding:16px}.field-grid{display:grid;grid-template-columns:2fr 2fr 1.4fr .7fr;gap:12px}.field-grid label{display:flex;flex-direction:column;gap:6px}.field-grid label.full{grid-column:1/-1}.field-grid span{font-size:11px;color:#696970;font-weight:600}input{width:100%;height:36px;border:1px solid rgba(0,0,0,.17);border-radius:7px;padding:0 10px;background:rgba(255,255,255,.85);color:inherit;font:inherit;outline:none}input:focus{border-color:#6c5ce7;box-shadow:0 0 0 3px rgba(108,92,231,.13)}.form-footer{display:flex;align-items:center;justify-content:space-between;margin-top:14px}.form-footer span{font-size:11px;color:#76767d}.form-footer button{cursor:pointer}.empty{padding:24px;text-align:center;color:#777;font-size:13px;border:1px dashed rgba(0,0,0,.17);border-radius:10px}.error-banner{display:flex;gap:8px;padding:11px 13px;margin-bottom:18px;border-radius:8px;background:#fff0f0;color:#8b252b;font-size:12px}.browser-header{padding:17px 18px;border-bottom:1px solid rgba(0,0,0,.09)}.back{color:#6c5ce7;font-size:13px;font-weight:600}.server-title{display:flex;flex-direction:column;align-items:center;gap:2px}.server-title strong{font-size:14px}.server-title span{font-size:11px;color:#777}.path-bar{display:flex;gap:8px;padding:11px 14px;background:rgba(0,0,0,.025);border-bottom:1px solid rgba(0,0,0,.08)}.path-bar .up,.path-bar button{height:34px;display:grid;place-items:center;border:1px solid rgba(0,0,0,.13);background:rgba(255,255,255,.75);border-radius:7px}.path-bar .up{width:38px;color:#6c5ce7;font-weight:700}.path-bar input{height:34px;font-family:"SF Mono",Menlo,monospace;font-size:12px}.path-bar button{padding:0 16px;cursor:pointer;color:inherit}.entry-list{min-height:420px;max-height:620px;overflow:auto;padding:6px}.entry{display:flex;align-items:center;gap:11px;min-height:42px;padding:0 12px;border-radius:7px;font-size:13px}.entry.enabled:hover{background:rgba(108,92,231,.1)}.entry.disabled{opacity:.42}.icon{display:grid;place-items:center;width:25px;height:25px;font-size:11px}.icon.folder{color:#6c5ce7}.icon.file{font-family:"SF Mono",Menlo,monospace;color:#65656b}.file-action{margin-left:auto;font-size:11px;color:#777}.browser-footer{padding:10px 16px;border-top:1px solid rgba(0,0,0,.08);font-size:11px;color:#777}.error-page{text-align:center;padding:70px 50px}.error-symbol{display:grid;place-items:center;margin:0 auto 20px;width:52px;height:52px;border-radius:50%;background:#ffe5e5;color:#a32931;font-size:30px;font-weight:700}.error-page h1{font-size:24px}.error-page p{max-width:560px;margin:12px auto 24px;color:#696970;white-space:pre-wrap}.error-actions{display:flex;justify-content:center;gap:10px}@media(max-width:760px){.field-grid{grid-template-columns:1fr 1fr}.connections{grid-template-columns:1fr}.picker.wide{padding:18px}.form-footer{align-items:flex-end;gap:15px}}@media(prefers-color-scheme:dark){:root{background:#121214;color:#ededf0}body{background:radial-gradient(circle at top,#252529 0,#121214 58%)}.picker{background:rgba(30,30,33,.94);border-color:rgba(255,255,255,.1);box-shadow:0 18px 70px rgba(0,0,0,.42)}.page-header p,.connection-copy span,.form-footer span,.server-title span,.file-action,.browser-footer{color:#9a9aa1}.button.secondary,.connection-card,.new-connection form,.path-bar .up,.path-bar button,input{background:rgba(255,255,255,.055);border-color:rgba(255,255,255,.13)}.new-connection,.browser-header,.path-bar,.browser-footer{border-color:rgba(255,255,255,.09)}.path-bar{background:rgba(255,255,255,.025)}.error-banner{background:rgba(180,45,55,.18);color:#ffb8bd}.empty{border-color:rgba(255,255,255,.14)}}
"#;

#[cfg(test)]
mod tests {
    use super::{action_url, connections_page};

    #[test]
    fn action_urls_encode_remote_paths() {
        assert_eq!(
            action_url("browse", &[("path", "/home/me/My Notes")]),
            "mdiew://browse?path=%2Fhome%2Fme%2FMy+Notes"
        );
    }

    #[test]
    fn connection_page_keeps_local_cancel_path() {
        let html = connections_page(None);
        assert!(html.contains("mdiew://cancel"));
        assert!(html.contains("Connect new server"));
    }
}
