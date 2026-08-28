# mdiew

A fast, native macOS markdown viewer built with Rust.

Renders markdown files using a `WKWebView` with GitHub-flavored styling, Mermaid diagrams, LaTeX math, syntax highlighting, and live file watching.

## Features

- GitHub-flavored markdown (tables, task lists, footnotes, strikethrough, autolinks)
- Mermaid diagram rendering
- Offline LaTeX math rendering via KaTeX
- Syntax highlighting via syntect
- Live reload on file changes with scroll position preservation
- Integrated local and SSH remote file pickers
- Saved remote servers and per-server directory memory
- macOS Keychain storage for SSH passwords and key passphrases
- On-demand remote connections with local file caching
- Find in page (Cmd+F)
- Export to HTML/PDF
- Open as .app bundle with Finder integration

## Installation

### Homebrew

```sh
brew tap SeungheonOh/mdiew https://github.com/SeungheonOh/mdiew
brew install --cask mdiew
```

### Install script (from GitHub Releases)

```sh
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/SeungheonOh/mdiew/releases/latest/download/mdiew-installer.sh | sh
```

### Build from source

Requires Rust and macOS.

```sh
git clone https://github.com/SeungheonOh/mdiew.git
cd mdiew
cargo build --release
```

The binary will be at `target/release/mdiew`.

### Install as .app bundle

```sh
make install
```

This builds the release binary, creates `mdiew.app` in `/Applications`, and registers it with Launch Services.

To also set mdiew as the default viewer for `.md` files:

```sh
make default
```

Requires [duti](https://github.com/moretension/duti) (`brew install duti`).

## Usage

```sh
mdiew README.md
```

Or open any `.md` file from Finder after installing the .app bundle.

### Remote files

Choose **File → Open Remote…** to connect to a machine over SSH. Add a server using
the same host, username, port, and optional identity file you would use with the
system `ssh` command. Existing aliases and options from `~/.ssh/config` continue to
apply.

mdiew verifies new servers before saving them, remembers the last directory used on
each server, and stores passwords or key passphrases in macOS Keychain. Remote files
are copied into `~/Library/Caches/mdiew/remote-files` before rendering. Connections
are opened only while listing a directory, opening a file, or reloading; mdiew does
not leave an SSH control connection running.

Saved connection details are stored in
`~/Library/Application Support/mdiew/remote-connections.json`. Removing a server
from the remote picker also removes its mdiew-managed Keychain credentials.

### LaTeX math

Use single dollar signs for inline math and double dollar signs for display math:

```markdown
Euler's identity is $e^{i\pi} + 1 = 0$.

$$
\int_{-\infty}^{\infty} e^{-x^2}\,dx = \sqrt{\pi}
$$
```

GitLab-style math code is also supported with ``$`...`$`` for inline math and fenced `math` blocks for display math.

### Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+O | Open file |
| File → Open Remote… | Open remote file over SSH |
| Cmd+F | Find in page |
| Cmd+E | Export to HTML |
| Cmd+Shift+E | Export to PDF |
| Cmd+R | Reload |

## Uninstall

```sh
make uninstall
```
