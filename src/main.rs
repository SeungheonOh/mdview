#![deny(unsafe_op_in_unsafe_fn)]

mod remote;
mod remote_picker;

use core::cell::{Cell, OnceCell};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSMenu, NSMenuItem, NSModalResponseOK, NSOpenPanel, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSRunLoopCommonModes, NSSize,
    NSString, ns_string,
};
use objc2_web_kit::{
    WKNavigationAction, WKNavigationActionPolicy, WKNavigationDelegate, WKWebView,
};

use comrak::plugins::syntect::SyntectAdapterBuilder;
use comrak::{Options, markdown_to_html_with_plugins, options::Plugins};

const GITHUB_CSS: &str = include_str!("../assets/github-markdown.css");
const KATEX_CSS: &str = include_str!("../assets/katex.min.css");
const KATEX_JS: &str = include_str!("../assets/katex.min.js");
const MERMAID_JS: &str = include_str!("../assets/mermaid.min.js");

/// The currently displayed file path. Protected by a Mutex for updates from open-file.
static FILE_PATH: OnceLock<Mutex<PathBuf>> = OnceLock::new();
static NEEDS_RELOAD: AtomicBool = AtomicBool::new(false);

fn get_file_path() -> Option<PathBuf> {
    FILE_PATH.get().map(|m| m.lock().unwrap().clone())
}

fn set_file_path(path: PathBuf) {
    match FILE_PATH.get() {
        Some(mutex) => {
            *mutex.lock().unwrap() = path;
        }
        None => {
            FILE_PATH.set(Mutex::new(path)).ok();
        }
    }
}

fn comrak_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.footnotes = true;
    options.extension.description_lists = true;
    options.extension.multiline_block_quotes = true;
    options.extension.math_dollars = true;
    options.extension.math_code = true;
    options.render.r#unsafe = true;
    options
}

fn build_syntax_set() -> syntect::parsing::SyntaxSet {
    let mut builder = syntect::parsing::SyntaxSet::load_defaults_newlines().into_builder();
    let lean_syntax = include_str!("../assets/lean.sublime-syntax");
    let syntax_def = syntect::parsing::syntax_definition::SyntaxDefinition::load_from_str(
        lean_syntax,
        true,
        Some("lean"),
    )
    .expect("failed to parse lean.sublime-syntax");
    builder.add(syntax_def);
    builder.build()
}

fn render_markdown(markdown: &str) -> String {
    let options = comrak_options();

    let ss = build_syntax_set();
    let adapter = SyntectAdapterBuilder::new().syntax_set(ss).css().build();
    let mut plugins = Plugins::default();
    plugins.render.codefence_syntax_highlighter = Some(&adapter);
    let html_body = markdown_to_html_with_plugins(markdown, &options, &plugins);

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
{GITHUB_CSS}
{KATEX_CSS}

body {{
    margin: 0;
    background-color: #ffffff;
}}
@media (prefers-color-scheme: dark) {{
    body {{
        background-color: #0d1117;
    }}
}}

.markdown-body {{
    box-sizing: border-box;
    min-width: 200px;
    max-width: 980px;
    margin: 0 auto;
    padding: 45px;
}}

@media (max-width: 767px) {{
    .markdown-body {{
        padding: 15px;
    }}
}}

.katex-display {{
    overflow-x: auto;
    overflow-y: hidden;
    padding: 0.25em 0;
}}

/* Find bar */
#mdiew-find-bar {{
    display: none;
    position: fixed;
    top: 12px;
    right: 12px;
    z-index: 10000;
    align-items: center;
    gap: 4px;
    padding: 6px 8px;
    background: rgba(246,246,246,0.92);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(0,0,0,0.12);
    border-radius: 8px;
    box-shadow: 0 2px 12px rgba(0,0,0,0.12);
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
    font-size: 13px;
}}
@media (prefers-color-scheme: dark) {{
    #mdiew-find-bar {{
        background: rgba(50,50,50,0.92);
        border-color: rgba(255,255,255,0.1);
    }}
}}
#mdiew-find-input {{
    width: 200px;
    padding: 4px 8px;
    border: 1px solid rgba(0,0,0,0.15);
    border-radius: 5px;
    outline: none;
    font-size: 13px;
    background: white;
    color: #333;
}}
@media (prefers-color-scheme: dark) {{
    #mdiew-find-input {{
        background: rgba(30,30,30,0.9);
        border-color: rgba(255,255,255,0.15);
        color: #eee;
    }}
}}
#mdiew-find-input:focus {{
    border-color: #6C5CE7;
    box-shadow: 0 0 0 2px rgba(108,92,231,0.3);
}}
.mdiew-find-btn {{
    padding: 3px 8px;
    border: 1px solid rgba(0,0,0,0.12);
    border-radius: 5px;
    background: rgba(255,255,255,0.8);
    color: #333;
    font-size: 12px;
    cursor: pointer;
    line-height: 1.4;
}}
@media (prefers-color-scheme: dark) {{
    .mdiew-find-btn {{
        background: rgba(60,60,60,0.8);
        border-color: rgba(255,255,255,0.1);
        color: #ddd;
    }}
}}
.mdiew-find-btn:hover {{
    background: rgba(108,92,231,0.15);
}}

/* Mermaid diagram containers */
.mermaid {{
    cursor: pointer;
    position: relative;
}}
.mermaid:hover {{
    outline: 2px solid rgba(108,92,231,0.4);
    outline-offset: 4px;
    border-radius: 4px;
}}
.mermaid::after {{
    content: 'Click to zoom';
    position: absolute;
    top: 4px;
    right: 8px;
    font-size: 11px;
    color: #666;
    background: rgba(255,255,255,0.85);
    padding: 2px 6px;
    border-radius: 4px;
    opacity: 0;
    transition: opacity 0.15s;
    pointer-events: none;
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
}}
@media (prefers-color-scheme: dark) {{
    .mermaid::after {{
        background: rgba(30,30,30,0.85);
        color: #aaa;
    }}
}}
.mermaid:hover::after {{
    opacity: 1;
}}

/* Mermaid zoom overlay */
#mermaid-overlay {{
    display: none;
    position: fixed;
    top: 0; left: 0; right: 0; bottom: 0;
    z-index: 9999;
    background: rgba(255,255,255,0.95);
    backdrop-filter: blur(8px);
    -webkit-backdrop-filter: blur(8px);
}}
@media (prefers-color-scheme: dark) {{
    #mermaid-overlay {{
        background: rgba(13,17,23,0.95);
    }}
}}
#mermaid-overlay-viewport {{
    width: 100%;
    height: 100%;
    overflow: hidden;
    cursor: grab;
}}
#mermaid-overlay-viewport:active {{
    cursor: grabbing;
}}
#mermaid-overlay-content {{
    transform-origin: 0 0;
    display: inline-block;
    padding: 40px;
}}
#mermaid-overlay-controls {{
    position: fixed;
    top: 12px;
    right: 12px;
    display: flex;
    gap: 4px;
    z-index: 10001;
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
}}
.mermaid-ctrl-btn {{
    padding: 6px 12px;
    border: 1px solid rgba(0,0,0,0.12);
    border-radius: 6px;
    background: rgba(246,246,246,0.92);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    color: #333;
    font-size: 14px;
    cursor: pointer;
    line-height: 1.2;
    box-shadow: 0 1px 4px rgba(0,0,0,0.08);
    user-select: none;
}}
@media (prefers-color-scheme: dark) {{
    .mermaid-ctrl-btn {{
        background: rgba(50,50,50,0.92);
        border-color: rgba(255,255,255,0.1);
        color: #ddd;
    }}
}}
.mermaid-ctrl-btn:hover {{
    background: rgba(108,92,231,0.15);
}}
#mermaid-overlay-zoom-level {{
    position: fixed;
    bottom: 16px;
    left: 50%;
    transform: translateX(-50%);
    font-family: -apple-system, BlinkMacSystemFont, sans-serif;
    font-size: 12px;
    color: #888;
    z-index: 10001;
    user-select: none;
}}

/* Syntax highlighting — GitHub colors */
pre.syntax-highlighting {{ background: transparent; }}
pre.syntax-highlighting .comment {{ color: #6a737d; font-style: italic; }}
pre.syntax-highlighting .string {{ color: #032f62; }}
pre.syntax-highlighting .constant {{ color: #005cc5; }}
pre.syntax-highlighting .keyword,
pre.syntax-highlighting .storage {{ color: #d73a49; }}
pre.syntax-highlighting .entity {{ color: #6f42c1; }}
pre.syntax-highlighting .support {{ color: #005cc5; }}
pre.syntax-highlighting .variable {{ color: #e36209; }}
pre.syntax-highlighting .punctuation {{ color: inherit; }}
@media (prefers-color-scheme: dark) {{
    pre.syntax-highlighting .comment {{ color: #8b949e; }}
    pre.syntax-highlighting .string {{ color: #a5d6ff; }}
    pre.syntax-highlighting .constant {{ color: #79c0ff; }}
    pre.syntax-highlighting .keyword,
    pre.syntax-highlighting .storage {{ color: #ff7b72; }}
    pre.syntax-highlighting .entity {{ color: #d2a8ff; }}
    pre.syntax-highlighting .support {{ color: #79c0ff; }}
    pre.syntax-highlighting .variable {{ color: #ffa657; }}
}}
</style>
<script>
{MERMAID_JS}
</script>
<script>
{KATEX_JS}
</script>
</head>
<body>
<article class="markdown-body">
{html_body}
</article>
<script>
(function() {{
    document.querySelectorAll('[data-math-style]').forEach(function(mathEl) {{
        var displayMode = mathEl.dataset.mathStyle === 'display';
        var source = mathEl.textContent;
        var pre = mathEl.parentElement && mathEl.parentElement.tagName === 'PRE'
            ? mathEl.parentElement
            : null;
        var container = document.createElement(pre ? 'div' : 'span');
        container.className = displayMode ? 'mdiew-math-display' : 'mdiew-math-inline';

        katex.render(source, container, {{
            displayMode: displayMode,
            throwOnError: false
        }});

        (pre || mathEl).replaceWith(container);
    }});

    // Convert comrak's mermaid code blocks into mermaid-renderable divs.
    document.querySelectorAll('pre > code.language-mermaid').forEach(function(codeEl) {{
        var pre = codeEl.parentElement;
        var div = document.createElement('div');
        div.className = 'mermaid';
        div.textContent = codeEl.textContent;
        pre.parentElement.replaceChild(div, pre);
    }});

    // Detect dark mode and set mermaid theme accordingly.
    var isDark = window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches;
    mermaid.initialize({{
        startOnLoad: true,
        theme: isDark ? 'dark' : 'default'
    }});
}})();

    // Persist scroll position across live reloads.
    var savedY = sessionStorage.getItem('mdiew_scrollY');
    if (savedY) {{
        requestAnimationFrame(function() {{
            window.scrollTo(0, parseInt(savedY));
        }});
    }}
    window.addEventListener('scroll', function() {{
        sessionStorage.setItem('mdiew_scrollY', window.scrollY.toString());
    }});
</script>

<!-- Mermaid zoom overlay -->
<div id="mermaid-overlay">
    <div id="mermaid-overlay-controls">
        <button class="mermaid-ctrl-btn" id="mermaid-zoom-in" title="Zoom in">+</button>
        <button class="mermaid-ctrl-btn" id="mermaid-zoom-out" title="Zoom out">&minus;</button>
        <button class="mermaid-ctrl-btn" id="mermaid-zoom-reset" title="Reset zoom">1:1</button>
        <button class="mermaid-ctrl-btn" id="mermaid-zoom-fit" title="Fit to screen">Fit</button>
        <button class="mermaid-ctrl-btn" id="mermaid-close" title="Close (Esc)">&times;</button>
    </div>
    <div id="mermaid-overlay-viewport">
        <div id="mermaid-overlay-content"></div>
    </div>
    <div id="mermaid-overlay-zoom-level"></div>
</div>
<script>
(function() {{
    var overlay = document.getElementById('mermaid-overlay');
    var viewport = document.getElementById('mermaid-overlay-viewport');
    var content = document.getElementById('mermaid-overlay-content');
    var zoomLabel = document.getElementById('mermaid-overlay-zoom-level');

    var scale = 1;
    var translateX = 0;
    var translateY = 0;
    var isDragging = false;
    var dragStartX = 0;
    var dragStartY = 0;
    var dragStartTX = 0;
    var dragStartTY = 0;

    function updateTransform() {{
        content.style.transform = 'translate(' + translateX + 'px, ' + translateY + 'px) scale(' + scale + ')';
        zoomLabel.textContent = Math.round(scale * 100) + '%';
    }}

    function fitToScreen() {{
        var svg = content.querySelector('svg');
        if (!svg) return;
        var svgW = svg.getBoundingClientRect().width / scale;
        var svgH = svg.getBoundingClientRect().height / scale;
        var vw = viewport.clientWidth - 80;
        var vh = viewport.clientHeight - 80;
        scale = Math.min(vw / svgW, vh / svgH, 2);
        translateX = (viewport.clientWidth - svgW * scale) / 2;
        translateY = (viewport.clientHeight - svgH * scale) / 2;
        updateTransform();
    }}

    function openOverlay(mermaidEl) {{
        var svg = mermaidEl.querySelector('svg');
        if (!svg) return;
        content.innerHTML = '';
        var clone = svg.cloneNode(true);
        clone.style.maxWidth = 'none';
        clone.style.width = '';
        clone.style.height = '';
        content.appendChild(clone);
        overlay.style.display = 'block';
        scale = 1;
        translateX = 0;
        translateY = 0;
        updateTransform();
        // Slight delay so layout settles, then fit
        requestAnimationFrame(function() {{ fitToScreen(); }});
    }}

    function closeOverlay() {{
        overlay.style.display = 'none';
        content.innerHTML = '';
    }}

    // Click on mermaid diagrams to open overlay
    document.addEventListener('click', function(e) {{
        var el = e.target.closest('.mermaid');
        if (el && !overlay.contains(e.target)) {{
            e.preventDefault();
            openOverlay(el);
        }}
    }});

    // Close button
    document.getElementById('mermaid-close').addEventListener('click', closeOverlay);

    // Zoom controls
    document.getElementById('mermaid-zoom-in').addEventListener('click', function() {{
        var cx = viewport.clientWidth / 2;
        var cy = viewport.clientHeight / 2;
        var factor = 1.25;
        translateX = cx - (cx - translateX) * factor;
        translateY = cy - (cy - translateY) * factor;
        scale *= factor;
        updateTransform();
    }});
    document.getElementById('mermaid-zoom-out').addEventListener('click', function() {{
        var cx = viewport.clientWidth / 2;
        var cy = viewport.clientHeight / 2;
        var factor = 0.8;
        translateX = cx - (cx - translateX) * factor;
        translateY = cy - (cy - translateY) * factor;
        scale = Math.max(0.1, scale * factor);
        updateTransform();
    }});
    document.getElementById('mermaid-zoom-reset').addEventListener('click', function() {{
        scale = 1;
        translateX = (viewport.clientWidth - content.scrollWidth) / 2;
        translateY = (viewport.clientHeight - content.scrollHeight) / 2;
        updateTransform();
    }});
    document.getElementById('mermaid-zoom-fit').addEventListener('click', fitToScreen);

    // Scroll to zoom (centered on mouse)
    viewport.addEventListener('wheel', function(e) {{
        e.preventDefault();
        var rect = viewport.getBoundingClientRect();
        var mx = e.clientX - rect.left;
        var my = e.clientY - rect.top;
        var factor = e.deltaY < 0 ? 1.1 : 0.9;
        var newScale = Math.max(0.1, Math.min(10, scale * factor));
        var ratio = newScale / scale;
        translateX = mx - (mx - translateX) * ratio;
        translateY = my - (my - translateY) * ratio;
        scale = newScale;
        updateTransform();
    }}, {{ passive: false }});

    // Drag to pan
    viewport.addEventListener('mousedown', function(e) {{
        if (e.target.closest('#mermaid-overlay-controls')) return;
        isDragging = true;
        dragStartX = e.clientX;
        dragStartY = e.clientY;
        dragStartTX = translateX;
        dragStartTY = translateY;
    }});
    window.addEventListener('mousemove', function(e) {{
        if (!isDragging) return;
        translateX = dragStartTX + (e.clientX - dragStartX);
        translateY = dragStartTY + (e.clientY - dragStartY);
        updateTransform();
    }});
    window.addEventListener('mouseup', function() {{
        isDragging = false;
    }});

    // Escape to close
    document.addEventListener('keydown', function(e) {{
        if (overlay.style.display === 'block') {{
            if (e.key === 'Escape') {{
                closeOverlay();
                e.stopPropagation();
            }}
            // Keyboard zoom: +/- keys
            if (e.key === '=' || e.key === '+') {{
                document.getElementById('mermaid-zoom-in').click();
            }}
            if (e.key === '-') {{
                document.getElementById('mermaid-zoom-out').click();
            }}
            if (e.key === '0') {{
                fitToScreen();
            }}
        }}
    }});
}})();
</script>

<!-- Find bar -->
<div id="mdiew-find-bar">
    <input id="mdiew-find-input" type="text" placeholder="Find..." autocomplete="off" spellcheck="false">
    <button class="mdiew-find-btn" id="mdiew-find-prev" title="Previous (⇧⏎)">&lsaquo;</button>
    <button class="mdiew-find-btn" id="mdiew-find-next" title="Next (⏎)">&rsaquo;</button>
    <button class="mdiew-find-btn" id="mdiew-find-close" title="Close (Esc)">&times;</button>
</div>
<script>
(function() {{
    var bar = document.getElementById('mdiew-find-bar');
    var input = document.getElementById('mdiew-find-input');
    var nextBtn = document.getElementById('mdiew-find-next');
    var prevBtn = document.getElementById('mdiew-find-prev');
    var closeBtn = document.getElementById('mdiew-find-close');

    function doFind(backwards) {{
        var text = input.value;
        if (!text) return;
        // window.find(string, caseSensitive, backwards, wrapAround)
        window.find(text, false, backwards, true);
    }}

    function openFindBar() {{
        bar.style.display = 'flex';
        input.focus();
        input.select();
    }}

    function closeFindBar() {{
        bar.style.display = 'none';
        input.value = '';
        // Clear selection
        window.getSelection().removeAllRanges();
    }}

    // Expose for native menu
    window.mdiewOpenFind = openFindBar;

    input.addEventListener('keydown', function(e) {{
        if (e.key === 'Enter') {{
            e.preventDefault();
            doFind(e.shiftKey);
        }} else if (e.key === 'Escape') {{
            closeFindBar();
        }}
    }});

    nextBtn.addEventListener('click', function() {{ doFind(false); }});
    prevBtn.addEventListener('click', function() {{ doFind(true); }});
    closeBtn.addEventListener('click', closeFindBar);

    // ⌘F from keyboard (in case menu doesn't catch it)
    document.addEventListener('keydown', function(e) {{
        if ((e.metaKey || e.ctrlKey) && e.key === 'f') {{
            e.preventDefault();
            openFindBar();
        }}
        // ⌘G / ⌘⇧G for next/prev
        if ((e.metaKey || e.ctrlKey) && e.key === 'g') {{
            e.preventDefault();
            doFind(e.shiftKey);
        }}
        if (e.key === 'Escape' && bar.style.display === 'flex') {{
            closeFindBar();
        }}
    }});
}})();
</script>
</body>
</html>"#
    )
}

fn load_and_render() -> Option<String> {
    match get_file_path() {
        Some(path) => {
            let markdown = match std::fs::read_to_string(&path) {
                Ok(s) if s.is_empty() => return None, // Skip: file is mid-save (atomic rename)
                Ok(s) => s,
                Err(_) => return None, // Skip: file doesn't exist momentarily
            };
            Some(render_markdown(&markdown))
        }
        None => Some(render_markdown("*Open a file with* **File → Open** *(⌘O)*")),
    }
}

// ── App Delegate ──────────────────────────────────────────────────────

struct AppDelegateIvars {
    window: OnceCell<Retained<NSWindow>>,
    web_view: OnceCell<Retained<WKWebView>>,
    debouncer: RefCell<Option<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>>>,
    remote_document: RefCell<Option<remote::RemoteDocument>>,
    picker_open: Cell<bool>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = AppDelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        #[allow(non_snake_case)]
        unsafe fn applicationDidFinishLaunching(&self, _notification: &NSNotification) {
            let mtm = self.mtm();

            let window = {
                let content_rect = NSRect::new(NSPoint::new(0., 0.), NSSize::new(1200., 900.));
                let style = NSWindowStyleMask::Closable
                    | NSWindowStyleMask::Resizable
                    | NSWindowStyleMask::Titled
                    | NSWindowStyleMask::Miniaturizable;
                unsafe {
                    NSWindow::initWithContentRect_styleMask_backing_defer(
                        NSWindow::alloc(mtm),
                        content_rect,
                        style,
                        NSBackingStoreType::Buffered,
                        false,
                    )
                }
            };

            let web_view =
                unsafe { WKWebView::initWithFrame(WKWebView::alloc(mtm), NSRect::ZERO) };
            let navigation_delegate = ProtocolObject::from_ref(self);
            unsafe { web_view.setNavigationDelegate(Some(navigation_delegate)) };

            let html = load_and_render()
                .unwrap_or_else(|| render_markdown("*Open a file with* **File → Open** *(⌘O)*"));
            let html_ns = NSString::from_str(&html);
            unsafe { web_view.loadHTMLString_baseURL(&html_ns, None) };

            window.setContentView(Some(&web_view));
            window.center();

            let title = get_file_path()
                .as_deref()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("mdiew")
                .to_string();
            window.setTitle(&NSString::from_str(&title));
            window.makeKeyAndOrderFront(None);

            // Build menu bar.
            build_menu_bar(mtm);

            // Start file watcher (only if a file was provided via CLI).
            if let Some(path) = get_file_path() {
                let debouncer = start_file_watcher(&path);
                *self.ivars().debouncer.borrow_mut() = Some(debouncer);
            }

            self.ivars()
                .window
                .set(window)
                .expect("window ivar should not already be set");
            self.ivars()
                .web_view
                .set(web_view)
                .expect("web_view ivar should not already be set");

            start_reload_timer(self);
        }

        #[unsafe(method(applicationShouldTerminateAfterLastWindowClosed:))]
        #[allow(non_snake_case)]
        unsafe fn applicationShouldTerminateAfterLastWindowClosed(
            &self,
            _sender: &NSApplication,
        ) -> bool {
            true
        }

        #[unsafe(method(application:openFile:))]
        #[allow(non_snake_case)]
        unsafe fn application_openFile(
            &self,
            _sender: &NSApplication,
            filename: &NSString,
        ) -> bool {
            let path = PathBuf::from(filename.to_string());
            if path.exists() {
                self.open_file(path);
                true
            } else {
                false
            }
        }
    }

    unsafe impl WKNavigationDelegate for AppDelegate {
        #[unsafe(method(webView:decidePolicyForNavigationAction:decisionHandler:))]
        #[allow(non_snake_case)]
        unsafe fn webView_decidePolicyForNavigationAction_decisionHandler(
            &self,
            _web_view: &WKWebView,
            navigation_action: &WKNavigationAction,
            decision_handler: &block2::DynBlock<dyn Fn(WKNavigationActionPolicy)>,
        ) {
            let absolute_url = unsafe { navigation_action.request() }
                .URL()
                .and_then(|url| url.absoluteString())
                .map(|url| url.to_string());
            if let Some(absolute_url) = absolute_url
                && absolute_url.starts_with("mdiew://")
            {
                decision_handler.call((WKNavigationActionPolicy::Cancel,));
                self.handle_remote_action(&absolute_url);
                return;
            }
            decision_handler.call((WKNavigationActionPolicy::Allow,));
        }
    }

    impl AppDelegate {
        #[unsafe(method(checkReload:))]
        #[allow(non_snake_case)]
        fn checkReload(&self, _timer: *mut AnyObject) {
            if !NEEDS_RELOAD.swap(false, Ordering::Relaxed) {
                return;
            }
            let Some(web_view) = self.ivars().web_view.get() else {
                return;
            };

            // If the file is unreadable or empty (mid-save), re-flag and retry next tick.
            let Some(html) = load_and_render() else {
                NEEDS_RELOAD.store(true, Ordering::Relaxed);
                return;
            };

            let html_ns = NSString::from_str(&html);
            unsafe { web_view.loadHTMLString_baseURL(&html_ns, None) };
        }

        #[unsafe(method(openDocument:))]
        #[allow(non_snake_case)]
        fn openDocument(&self, _sender: *mut AnyObject) {
            let mtm = self.mtm();
            let panel = NSOpenPanel::openPanel(mtm);
            panel.setAllowsMultipleSelection(false);

            let response = panel.runModal();
            if response == NSModalResponseOK
                && let Some(url) = panel.URL()
                && let Some(path) = url.path()
            {
                self.open_file(PathBuf::from(path.to_string()));
            }
        }

        #[unsafe(method(openRemoteDocument:))]
        #[allow(non_snake_case)]
        fn openRemoteDocument(&self, _sender: *mut AnyObject) {
            self.show_remote_connections(None);
        }

        #[unsafe(method(reloadDocument:))]
        #[allow(non_snake_case)]
        fn reloadDocument(&self, _sender: *mut AnyObject) {
            self.reload_current_document();
        }

        #[unsafe(method(zoomIn:))]
        #[allow(non_snake_case)]
        fn zoomIn(&self, _sender: *mut AnyObject) {
            if let Some(web_view) = self.ivars().web_view.get() {
                let js = NSString::from_str(
                    "document.body.style.zoom = (parseFloat(document.body.style.zoom || 1) + 0.1).toString()"
                );
                unsafe { web_view.evaluateJavaScript_completionHandler(&js, None) };
            }
        }

        #[unsafe(method(zoomOut:))]
        #[allow(non_snake_case)]
        fn zoomOut(&self, _sender: *mut AnyObject) {
            if let Some(web_view) = self.ivars().web_view.get() {
                let js = NSString::from_str(
                    "document.body.style.zoom = Math.max(0.5, (parseFloat(document.body.style.zoom || 1) - 0.1)).toString()"
                );
                unsafe { web_view.evaluateJavaScript_completionHandler(&js, None) };
            }
        }

        #[unsafe(method(resetZoom:))]
        #[allow(non_snake_case)]
        fn resetZoom(&self, _sender: *mut AnyObject) {
            if let Some(web_view) = self.ivars().web_view.get() {
                let js = NSString::from_str("document.body.style.zoom = '1'");
                unsafe { web_view.evaluateJavaScript_completionHandler(&js, None) };
            }
        }

        #[unsafe(method(performFind:))]
        #[allow(non_snake_case)]
        fn performFind(&self, _sender: *mut AnyObject) {
            if let Some(web_view) = self.ivars().web_view.get() {
                let js = NSString::from_str("window.mdiewOpenFind()");
                unsafe { web_view.evaluateJavaScript_completionHandler(&js, None) };
            }
        }
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars {
            window: OnceCell::new(),
            web_view: OnceCell::new(),
            debouncer: RefCell::new(None),
            remote_document: RefCell::new(None),
            picker_open: Cell::new(false),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn open_file(&self, path: PathBuf) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);

        // Update window title.
        if let Some(window) = self.ivars().window.get() {
            let title = path.file_name().and_then(|n| n.to_str()).unwrap_or("mdiew");
            window.setTitle(&NSString::from_str(title));
        }

        // Restart file watcher for the new file.
        let debouncer = start_file_watcher(&path);

        // Update the file path and reload.
        set_file_path(path);
        *self.ivars().remote_document.borrow_mut() = None;
        self.ivars().picker_open.set(false);
        *self.ivars().debouncer.borrow_mut() = Some(debouncer);

        // Re-render.
        if let Some(web_view) = self.ivars().web_view.get()
            && let Some(html) = load_and_render()
        {
            let html_ns = NSString::from_str(&html);
            unsafe { web_view.loadHTMLString_baseURL(&html_ns, None) };
        }
    }

    fn load_html(&self, html: &str) {
        if let Some(web_view) = self.ivars().web_view.get() {
            let html = NSString::from_str(html);
            unsafe { web_view.loadHTMLString_baseURL(&html, None) };
        }
    }

    fn show_remote_connections(&self, error: Option<&str>) {
        self.ivars().picker_open.set(true);
        self.load_html(&remote_picker::connections_page(error));
        if let Some(window) = self.ivars().window.get() {
            window.setTitle(ns_string!("Open Remote File"));
        }
    }

    fn show_remote_directory(&self, connection: &remote::Connection, path: &str) {
        match remote::list_directory(connection, path) {
            Ok(listing) => {
                let _ = remote::remember_directory(&connection.id, &listing.path);
                self.load_html(&remote_picker::directory_page(connection, &listing));
            }
            Err(error) => {
                let retry =
                    remote_picker::action_url("browse", &[("id", &connection.id), ("path", path)]);
                self.load_html(&remote_picker::error_page(&error, &retry));
            }
        }
    }

    fn handle_remote_action(&self, absolute_url: &str) {
        let Ok(url) = url::Url::parse(absolute_url) else {
            return;
        };
        let action = url.host_str().unwrap_or_default();
        let parameters: HashMap<String, String> = url.query_pairs().into_owned().collect();

        match action {
            "connections" => self.show_remote_connections(None),
            "cancel" => self.restore_document(),
            "connect" => {
                if let Some(connection) = parameters
                    .get("id")
                    .and_then(|id| remote::find_connection(id))
                {
                    let path = connection.last_directory.clone();
                    self.show_remote_directory(&connection, &path);
                }
            }
            "browse" => {
                if let (Some(connection), Some(path)) = (
                    parameters
                        .get("id")
                        .and_then(|id| remote::find_connection(id)),
                    parameters.get("path"),
                ) {
                    self.show_remote_directory(&connection, path);
                }
            }
            "open" => {
                if let (Some(connection), Some(path)) = (
                    parameters
                        .get("id")
                        .and_then(|id| remote::find_connection(id)),
                    parameters.get("path"),
                ) {
                    self.open_remote_file(&connection, path);
                }
            }
            "add" => self.add_remote_connection(&parameters),
            "remove" => {
                if let Some(id) = parameters.get("id") {
                    if let Err(error) = remote::remove_connection(id) {
                        self.show_remote_connections(Some(&error.to_string()));
                    } else {
                        self.show_remote_connections(None);
                    }
                }
            }
            _ => {}
        }
    }

    fn add_remote_connection(&self, parameters: &HashMap<String, String>) {
        let port = parameters
            .get("port")
            .and_then(|port| port.parse::<u16>().ok())
            .unwrap_or(22);
        let connection = remote::Connection::new(
            parameters.get("nickname").cloned().unwrap_or_default(),
            parameters.get("host").cloned().unwrap_or_default(),
            parameters.get("username").cloned(),
            port,
            parameters.get("identity_file").cloned(),
        );
        let mut connection = match connection {
            Ok(connection) => connection,
            Err(error) => {
                self.show_remote_connections(Some(&error));
                return;
            }
        };

        match remote::list_directory(&connection, "~") {
            Ok(listing) => {
                connection.last_directory = listing.path.clone();
                if let Err(error) = remote::save_connection(connection.clone()) {
                    self.show_remote_connections(Some(&error.to_string()));
                    return;
                }
                self.load_html(&remote_picker::directory_page(&connection, &listing));
            }
            Err(error) => {
                remote::clear_rejected_credentials(&connection.id);
                self.show_remote_connections(Some(&error));
            }
        }
    }

    fn open_remote_file(&self, connection: &remote::Connection, path: &str) {
        match remote::download_file(connection, path) {
            Ok(document) => {
                set_file_path(document.cached_path.clone());
                *self.ivars().remote_document.borrow_mut() = Some(document);
                *self.ivars().debouncer.borrow_mut() = None;
                self.ivars().picker_open.set(false);
                if let Some(window) = self.ivars().window.get() {
                    let file_name = PathBuf::from(path)
                        .file_name()
                        .and_then(|name| name.to_str())
                        .unwrap_or("Remote file")
                        .to_string();
                    window.setTitle(&NSString::from_str(&format!(
                        "{file_name} — {}",
                        connection.nickname
                    )));
                }
                if let Some(html) = load_and_render() {
                    self.load_html(&html);
                }
            }
            Err(error) => {
                let retry =
                    remote_picker::action_url("open", &[("id", &connection.id), ("path", path)]);
                self.load_html(&remote_picker::error_page(&error, &retry));
            }
        }
    }

    fn restore_document(&self) {
        self.ivars().picker_open.set(false);
        if let Some(html) = load_and_render() {
            self.load_html(&html);
        }
        if let Some(window) = self.ivars().window.get() {
            if let Some(document) = self.ivars().remote_document.borrow().as_ref() {
                let title = PathBuf::from(&document.remote_path)
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("Remote file")
                    .to_string();
                window.setTitle(&NSString::from_str(&title));
            } else {
                let title = get_file_path()
                    .as_deref()
                    .and_then(|path| path.file_name())
                    .and_then(|name| name.to_str())
                    .unwrap_or("mdiew")
                    .to_string();
                window.setTitle(&NSString::from_str(&title));
            }
        }
    }

    fn reload_current_document(&self) {
        if self.ivars().picker_open.get() {
            return;
        }
        let remote_document = self.ivars().remote_document.borrow().clone();
        if let Some(document) = remote_document {
            match remote::refresh_document(&document) {
                Ok(refreshed) => {
                    *self.ivars().remote_document.borrow_mut() = Some(refreshed);
                    if let Some(html) = load_and_render() {
                        self.load_html(&html);
                    }
                }
                Err(error) => {
                    let retry = remote_picker::action_url(
                        "open",
                        &[
                            ("id", &document.connection_id),
                            ("path", &document.remote_path),
                        ],
                    );
                    self.ivars().picker_open.set(true);
                    self.load_html(&remote_picker::error_page(&error, &retry));
                }
            }
        } else if let Some(html) = load_and_render() {
            self.load_html(&html);
        }
    }
}

// ── Menu Bar ──────────────────────────────────────────────────────────

fn build_menu_bar(mtm: MainThreadMarker) {
    let menu_bar = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!(""));

    // App menu.
    let app_menu_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            ns_string!(""),
            None,
            ns_string!(""),
        )
    };
    let app_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("mdiew"));
    unsafe {
        app_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("About mdiew"),
            Some(sel!(orderFrontStandardAboutPanel:)),
            ns_string!(""),
        );
    }
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    unsafe {
        app_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Quit mdiew"),
            Some(sel!(terminate:)),
            ns_string!("q"),
        );
    }
    app_menu_item.setSubmenu(Some(&app_menu));
    menu_bar.addItem(&app_menu_item);

    // File menu.
    let file_menu_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            ns_string!("File"),
            None,
            ns_string!(""),
        )
    };
    let file_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("File"));
    unsafe {
        file_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Open Local..."),
            Some(sel!(openDocument:)),
            ns_string!("o"),
        );
        file_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Open Remote..."),
            Some(sel!(openRemoteDocument:)),
            ns_string!(""),
        );
    }
    file_menu.addItem(&NSMenuItem::separatorItem(mtm));
    unsafe {
        file_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Reload"),
            Some(sel!(reloadDocument:)),
            ns_string!("r"),
        );
    }
    file_menu.addItem(&NSMenuItem::separatorItem(mtm));
    unsafe {
        file_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Close Window"),
            Some(sel!(performClose:)),
            ns_string!("w"),
        );
    }
    file_menu_item.setSubmenu(Some(&file_menu));
    menu_bar.addItem(&file_menu_item);

    // Edit menu.
    let edit_menu_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            ns_string!("Edit"),
            None,
            ns_string!(""),
        )
    };
    let edit_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("Edit"));
    unsafe {
        edit_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Copy"),
            Some(sel!(copy:)),
            ns_string!("c"),
        );
        edit_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Select All"),
            Some(sel!(selectAll:)),
            ns_string!("a"),
        );
    }
    edit_menu.addItem(&NSMenuItem::separatorItem(mtm));
    unsafe {
        edit_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Find\u{2026}"),
            Some(sel!(performFind:)),
            ns_string!("f"),
        );
    }
    edit_menu_item.setSubmenu(Some(&edit_menu));
    menu_bar.addItem(&edit_menu_item);

    // View menu.
    let view_menu_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            ns_string!("View"),
            None,
            ns_string!(""),
        )
    };
    let view_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("View"));
    unsafe {
        view_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Zoom In"),
            Some(sel!(zoomIn:)),
            ns_string!("+"),
        );
        view_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Zoom Out"),
            Some(sel!(zoomOut:)),
            ns_string!("-"),
        );
        view_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Actual Size"),
            Some(sel!(resetZoom:)),
            ns_string!("0"),
        );
    }
    view_menu_item.setSubmenu(Some(&view_menu));
    menu_bar.addItem(&view_menu_item);

    let app = NSApplication::sharedApplication(mtm);
    app.setMainMenu(Some(&menu_bar));
}

// ── File Watcher ──────────────────────────────────────────────────────

fn start_file_watcher(path: &Path) -> notify_debouncer_mini::Debouncer<notify::RecommendedWatcher> {
    // Watch the parent directory instead of the file directly.
    // Many editors (vim, VS Code, etc.) save via write-to-temp + atomic rename,
    // which replaces the inode and breaks a direct file watch.
    let file_name = path
        .file_name()
        .expect("watched path must have a filename")
        .to_os_string();
    let parent = path
        .parent()
        .expect("watched path must have a parent directory")
        .to_path_buf();

    let mut debouncer = new_debouncer(
        Duration::from_millis(200),
        move |result: Result<Vec<notify_debouncer_mini::DebouncedEvent>, notify::Error>| {
            match result {
                Ok(events) => {
                    let relevant = events
                        .iter()
                        .any(|e| e.path.file_name() == Some(&file_name));
                    if relevant {
                        NEEDS_RELOAD.store(true, Ordering::Relaxed);
                    }
                }
                Err(err) => {
                    eprintln!("File watch error: {err}");
                }
            }
        },
    )
    .expect("Failed to create file watcher");

    debouncer
        .watcher()
        .watch(&parent, notify::RecursiveMode::NonRecursive)
        .unwrap_or_else(|e| eprintln!("Failed to watch directory: {e}"));

    debouncer
}

// ── Reload Timer ──────────────────────────────────────────────────────

fn start_reload_timer(delegate: &AppDelegate) {
    use objc2_foundation::{NSRunLoop, NSTimer};

    let timer = unsafe {
        NSTimer::timerWithTimeInterval_target_selector_userInfo_repeats(
            0.2,
            delegate,
            sel!(checkReload:),
            None,
            true,
        )
    };

    let run_loop = NSRunLoop::currentRunLoop();
    unsafe { run_loop.addTimer_forMode(&timer, NSRunLoopCommonModes) };
}

// ── Main ──────────────────────────────────────────────────────────────

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if std::env::var_os("MDIEW_SSH_CONNECTION_ID").is_some() {
        std::process::exit(remote::run_askpass());
    }
    if args.len() >= 2 {
        let path = std::fs::canonicalize(&args[1]).unwrap_or_else(|_| PathBuf::from(&args[1]));
        set_file_path(path);
    }

    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let delegate = AppDelegate::new(mtm);
    let object = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(object));

    app.run();
}

#[cfg(test)]
mod tests {
    use super::{KATEX_CSS, render_markdown};

    #[test]
    fn renders_dollar_and_code_math_nodes() {
        let html = render_markdown(
            "Inline $x^2$ and $`y^2`$.\n\n$$\nx + y\n$$\n\n```math\n\\sum_n n\n```",
        );

        assert!(html.contains("<span data-math-style=\"inline\">x^2</span>"));
        assert!(html.contains("<code data-math-style=\"inline\">y^2</code>"));
        assert!(html.contains("class=\"language-math\" data-math-style=\"display\""));
        assert!(html.contains("katex.render(source, container"));
    }

    #[test]
    fn embeds_katex_fonts_for_offline_rendering() {
        assert!(KATEX_CSS.contains("data:font/woff2;base64,"));
        assert!(!KATEX_CSS.contains("url(fonts/"));
    }
}
