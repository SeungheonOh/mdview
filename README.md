# mdiew

A fast, native macOS markdown viewer built with Rust.

Renders markdown files using a `WKWebView` with GitHub-flavored styling, Mermaid diagram support, syntax highlighting, and live file watching.

## Features

- GitHub-flavored markdown (tables, task lists, footnotes, strikethrough, autolinks)
- Mermaid diagram rendering
- Syntax highlighting via syntect
- Live reload on file changes with scroll position preservation
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

### Keyboard shortcuts

| Shortcut | Action |
|----------|--------|
| Cmd+O | Open file |
| Cmd+F | Find in page |
| Cmd+E | Export to HTML |
| Cmd+Shift+E | Export to PDF |
| Cmd+R | Reload |

## Uninstall

```sh
make uninstall
```
