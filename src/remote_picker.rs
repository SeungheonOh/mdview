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
      <h1>Open Remote</h1>
      <p>Choose a saved SSH server or connect a new one.</p>
    </div>
    <a class="quiet-button" href="mdiew://cancel">Cancel</a>
  </header>
  {error}
  <div class="connections">{connection_cards}</div>
  <details class="new-connection">
    <summary>
      <span class="add-icon">+</span>
      <span class="summary-copy"><strong>Connect New Server</strong><small>Add an SSH host or config alias</small></span>
      <span class="chevron">›</span>
    </summary>
    <form action="mdiew://add" method="get">
      <div class="field-grid">
        <label><span>Host or SSH alias</span><input name="host" placeholder="server.example.com" required autofocus></label>
        <label><span>Name</span><input name="nickname" placeholder="Optional"></label>
      </div>
      <details class="advanced-options">
        <summary>Connection options</summary>
        <div class="field-grid advanced-grid">
          <label><span>Username</span><input name="username" placeholder="From SSH config"></label>
          <label><span>Port</span><input name="port" type="number" min="1" max="65535" value="22" required></label>
          <label class="full"><span>Identity file</span><input name="identity_file" placeholder="e.g. ~/.ssh/id_ed25519"></label>
        </div>
      </details>
      <div class="form-footer">
        <span>Credentials use macOS Keychain.</span>
        <button class="button primary" type="submit">Connect</button>
      </div>
    </form>
  </details>
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
    <a class="quiet-button" href="mdiew://cancel">Cancel</a>
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
    format!(
        r#"<div class="connection-card">
  <a class="connection-main" href="{connect_url}">
    <span class="server-icon"><i></i><i></i></span>
    <span class="connection-copy">
      <strong>{nickname}</strong>
      <span>{destination}{port}</span>
      <span class="directory">{directory}</span>
    </span>
    <span class="chevron">›</span>
  </a>
  <a class="remove" title="Remove server" href="{remove_url}" onclick="return confirm('Remove this saved server and its Keychain credentials?')">×</a>
</div>"#,
        nickname = html_escape(&connection.nickname),
        destination = html_escape(&connection.destination()),
        port = if connection.port == 22 {
            String::new()
        } else {
            format!(" · {}", connection.port)
        },
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
:root{color-scheme:light dark;font-family:-apple-system,BlinkMacSystemFont,"SF Pro Text",sans-serif;background:#f4f4f5;color:#202124}*{box-sizing:border-box}body{margin:0;min-height:100vh;background:#f4f4f5}a{color:inherit;text-decoration:none}.picker{width:min(720px,calc(100% - 40px));margin:40px auto;background:#fff;border:1px solid #d9d9dc;border-radius:12px;box-shadow:0 12px 40px rgba(0,0,0,.1);overflow:hidden}.picker.wide{width:min(680px,calc(100% - 40px));padding:22px}.page-header,.browser-header{display:flex;align-items:center;justify-content:space-between;gap:20px}.page-header{margin-bottom:18px}.page-header h1{font-size:21px;line-height:1.2;margin:0 0 4px}.page-header p{margin:0;color:#6f7076;font-size:12px}.quiet-button{color:#65666c;font-size:12px;padding:6px 8px;border-radius:6px}.quiet-button:hover{background:#eeeef0}.button{display:inline-flex;align-items:center;justify-content:center;border-radius:7px;padding:8px 14px;border:1px solid #ceced2;font-size:12px;font-weight:600;white-space:nowrap}.button.primary{background:#5b55d6;color:#fff;border-color:#5b55d6}.connections{border:1px solid #dedee1;border-radius:9px;overflow:hidden}.connection-card{position:relative;background:#fff}.connection-card+.connection-card{border-top:1px solid #e4e4e6}.connection-main{display:flex;align-items:center;gap:11px;padding:11px 44px 11px 12px;min-height:62px}.connection-main:hover{background:#f5f4ff}.server-icon{position:relative;width:30px;height:30px;border-radius:7px;background:#eeecff;flex:0 0 auto}.server-icon i{position:absolute;width:5px;height:5px;border-radius:50%;background:#655ddb;left:8px}.server-icon i:first-child{top:8px}.server-icon i:last-child{bottom:8px}.server-icon:after{content:"";position:absolute;width:1px;height:9px;background:#655ddb;left:10px;top:11px}.connection-copy{display:flex;flex-direction:column;gap:2px;min-width:0}.connection-copy strong{font-size:13px;font-weight:600}.connection-copy span{font-size:11px;color:#76777d;white-space:nowrap;overflow:hidden;text-overflow:ellipsis}.connection-copy .directory{color:#929399}.chevron{margin-left:auto;color:#aaaab0;font-size:20px}.remove{position:absolute;right:9px;top:19px;display:grid;place-items:center;width:24px;height:24px;border-radius:5px;color:#929399;font-size:17px}.remove:hover{background:#ffeaea;color:#a53d45}.new-connection{margin-top:10px;border:1px solid #dedee1;border-radius:9px;background:#fff;overflow:hidden}.new-connection>summary{display:flex;align-items:center;gap:11px;padding:11px 12px;cursor:pointer;list-style:none;min-height:60px}.new-connection>summary::-webkit-details-marker{display:none}.new-connection>summary:hover{background:#f7f7f8}.new-connection[open]>summary{border-bottom:1px solid #e2e2e4}.new-connection[open]>summary .chevron{transform:rotate(90deg)}.add-icon{display:grid;place-items:center;width:30px;height:30px;border-radius:7px;background:#eeeeef;color:#55565b;font-size:20px}.summary-copy{display:flex;flex-direction:column;gap:2px}.summary-copy strong{font-size:13px}.summary-copy small{color:#818289;font-size:11px}.new-connection form{padding:14px}.field-grid{display:grid;grid-template-columns:2fr 1fr;gap:10px}.field-grid label{display:flex;flex-direction:column;gap:5px}.field-grid label.full{grid-column:1/-1}.field-grid span{font-size:10px;color:#707178;font-weight:600}input{width:100%;height:33px;border:1px solid #cacacf;border-radius:6px;padding:0 9px;background:#fff;color:inherit;font:inherit;font-size:12px;outline:none}input:focus{border-color:#625bdd;box-shadow:0 0 0 2px rgba(98,91,221,.13)}.advanced-options{margin-top:10px}.advanced-options>summary{font-size:11px;color:#6560c9;cursor:pointer;list-style:none}.advanced-options>summary:before{content:"›";display:inline-block;margin-right:5px}.advanced-options[open]>summary:before{transform:rotate(90deg)}.advanced-grid{grid-template-columns:2fr 1fr;margin-top:10px;padding:10px;background:#f7f7f8;border-radius:7px}.form-footer{display:flex;align-items:center;justify-content:space-between;margin-top:12px}.form-footer span{font-size:10px;color:#85868c}.form-footer button{cursor:pointer}.empty{padding:20px;text-align:center;color:#7a7b80;font-size:12px}.error-banner{display:flex;gap:7px;padding:9px 11px;margin-bottom:12px;border-radius:7px;background:#fff0f0;color:#8b252b;font-size:11px}.browser-header{padding:14px 16px;border-bottom:1px solid #dedee1}.back{color:#5e58d2;font-size:12px;font-weight:600}.server-title{display:flex;flex-direction:column;align-items:center;gap:1px}.server-title strong{font-size:13px}.server-title span{font-size:10px;color:#7a7b80}.path-bar{display:flex;gap:7px;padding:9px 11px;background:#f7f7f8;border-bottom:1px solid #dedee1}.path-bar .up,.path-bar button{height:31px;display:grid;place-items:center;border:1px solid #cacacf;background:#fff;border-radius:6px}.path-bar .up{width:34px;color:#5e58d2;font-weight:700}.path-bar input{height:31px;font-family:"SF Mono",Menlo,monospace;font-size:11px}.path-bar button{padding:0 13px;cursor:pointer;color:inherit}.entry-list{min-height:380px;max-height:600px;overflow:auto;padding:5px}.entry{display:flex;align-items:center;gap:9px;min-height:37px;padding:0 10px;border-radius:6px;font-size:12px}.entry.enabled:hover{background:#f1f0ff}.entry.disabled{opacity:.38}.icon{display:grid;place-items:center;width:22px;height:22px;font-size:10px}.icon.folder{color:#5e58d2}.icon.file{font-family:"SF Mono",Menlo,monospace;color:#65666b}.file-action{margin-left:auto;font-size:10px;color:#85868c}.browser-footer{padding:9px 13px;border-top:1px solid #dedee1;font-size:10px;color:#85868c}.error-page{text-align:center;padding:60px 42px}.error-symbol{display:grid;place-items:center;margin:0 auto 17px;width:44px;height:44px;border-radius:50%;background:#ffe5e5;color:#a32931;font-size:24px;font-weight:700}.error-page h1{font-size:21px}.error-page p{max-width:520px;margin:10px auto 21px;color:#6f7076;font-size:12px;white-space:pre-wrap}.error-actions{display:flex;justify-content:center;gap:8px}@media(max-width:620px){.field-grid{grid-template-columns:1fr}.field-grid label.full{grid-column:auto}.picker.wide{padding:16px}}@media(prefers-color-scheme:dark){:root,body{background:#161618;color:#ededf0}.picker{background:#252528;border-color:#3c3c40;box-shadow:0 14px 44px rgba(0,0,0,.4)}.page-header p,.connection-copy span,.form-footer span,.server-title span,.file-action,.browser-footer,.summary-copy small{color:#9a9aa1}.quiet-button:hover,.connection-main:hover,.new-connection>summary:hover{background:#303034}.connections,.connection-card,.new-connection{background:#252528;border-color:#414146}.connection-card+.connection-card,.new-connection[open]>summary,.browser-header,.path-bar,.browser-footer{border-color:#3d3d41}.server-icon{background:#35324f}.add-icon{background:#36363a;color:#c7c7cc}.new-connection form{background:#222225}.advanced-grid,.path-bar{background:#202023}input,.path-bar .up,.path-bar button{background:#2b2b2e;border-color:#4a4a50}.error-banner{background:rgba(180,45,55,.18);color:#ffb8bd}}
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
        assert!(html.contains("Connect New Server"));
    }
}
