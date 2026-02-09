#![deny(unsafe_op_in_unsafe_fn)]

use core::cell::OnceCell;
use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use notify_debouncer_mini::{new_debouncer, notify};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, ProtocolObject};
use objc2::{define_class, msg_send, sel, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate, NSBackingStoreType,
    NSMenu, NSMenuItem, NSModalResponseOK, NSOpenPanel, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{
    ns_string, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSRunLoopCommonModes,
    NSSize, NSString,
};
use objc2_web_kit::WKWebView;

use comrak::{markdown_to_html, Options};

const GITHUB_CSS: &str = include_str!("../assets/github-markdown.css");
const MERMAID_JS: &str = include_str!("../assets/mermaid.min.js");

/// The currently displayed file path. Protected by a Mutex for updates from open-file.
static FILE_PATH: OnceLock<Mutex<PathBuf>> = OnceLock::new();
static NEEDS_RELOAD: AtomicBool = AtomicBool::new(false);

fn get_file_path() -> PathBuf {
    FILE_PATH
        .get()
        .expect("FILE_PATH should be set")
        .lock()
        .unwrap()
        .clone()
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
    options.render.r#unsafe = true;
    options
}

fn render_markdown(markdown: &str) -> String {
    let options = comrak_options();
    let html_body = markdown_to_html(markdown, &options);

    format!(
        r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<style>
{GITHUB_CSS}

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
</style>
<script>
{MERMAID_JS}
</script>
</head>
<body>
<article class="markdown-body">
{html_body}
</article>
<script>
(function() {{
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
</script>
</body>
</html>"#
    )
}

fn load_and_render() -> String {
    let path = get_file_path();
    let markdown = std::fs::read_to_string(&path).unwrap_or_else(|e| {
        format!("# Error\n\nFailed to read `{}`: {}", path.display(), e)
    });
    render_markdown(&markdown)
}

// ── App Delegate ──────────────────────────────────────────────────────

struct AppDelegateIvars {
    window: OnceCell<Retained<NSWindow>>,
    web_view: OnceCell<Retained<WKWebView>>,
    debouncer: RefCell<Option<notify_debouncer_mini::Debouncer<notify::RecommendedWatcher>>>,
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
                let content_rect = NSRect::new(NSPoint::new(0., 0.), NSSize::new(800., 600.));
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

            let html = load_and_render();
            let html_ns = NSString::from_str(&html);
            unsafe { web_view.loadHTMLString_baseURL(&html_ns, None) };

            window.setContentView(Some(&web_view));
            window.center();

            let title = get_file_path()
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("mdview")
                .to_string();
            window.setTitle(&NSString::from_str(&title));
            window.makeKeyAndOrderFront(None);

            // Build menu bar.
            build_menu_bar(mtm);

            // Start file watcher.
            let debouncer = start_file_watcher();

            self.ivars()
                .window
                .set(window)
                .expect("window ivar should not already be set");
            self.ivars()
                .web_view
                .set(web_view)
                .expect("web_view ivar should not already be set");
            *self.ivars().debouncer.borrow_mut() = Some(debouncer);

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

            let html = load_and_render();

            let html_with_scroll_restore = html.replace(
                "</body>",
                r#"<script>
(function() {
    var savedY = sessionStorage.getItem('mdview_scrollY');
    if (savedY) {
        requestAnimationFrame(function() {
            window.scrollTo(0, parseInt(savedY));
            sessionStorage.removeItem('mdview_scrollY');
        });
    }
})();
</script>
</body>"#,
            );

            let save_js = NSString::from_str(
                "sessionStorage.setItem('mdview_scrollY', window.scrollY.toString())",
            );
            unsafe { web_view.evaluateJavaScript_completionHandler(&save_js, None) };

            let html_ns = NSString::from_str(&html_with_scroll_restore);
            unsafe { web_view.loadHTMLString_baseURL(&html_ns, None) };
        }

        #[unsafe(method(openDocument:))]
        #[allow(non_snake_case)]
        fn openDocument(&self, _sender: *mut AnyObject) {
            let mtm = self.mtm();
            let panel = NSOpenPanel::openPanel(mtm);
            panel.setAllowsMultipleSelection(false);

            let response = panel.runModal();
            if response == NSModalResponseOK {
                if let Some(url) = panel.URL() {
                    if let Some(path) = url.path() {
                        self.open_file(PathBuf::from(path.to_string()));
                    }
                }
            }
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
    }
);

impl AppDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars {
            window: OnceCell::new(),
            web_view: OnceCell::new(),
            debouncer: RefCell::new(None),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn open_file(&self, path: PathBuf) {
        let path = std::fs::canonicalize(&path).unwrap_or(path);

        // Update window title.
        if let Some(window) = self.ivars().window.get() {
            let title = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("mdview");
            window.setTitle(&NSString::from_str(title));
        }

        // Update the file path and reload.
        set_file_path(path);

        // Restart file watcher for the new file.
        let debouncer = start_file_watcher();
        *self.ivars().debouncer.borrow_mut() = Some(debouncer);

        // Re-render.
        if let Some(web_view) = self.ivars().web_view.get() {
            let html = load_and_render();
            let html_ns = NSString::from_str(&html);
            unsafe { web_view.loadHTMLString_baseURL(&html_ns, None) };
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
    let app_menu = NSMenu::initWithTitle(NSMenu::alloc(mtm), ns_string!("mdview"));
    unsafe {
        app_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("About mdview"),
            Some(sel!(orderFrontStandardAboutPanel:)),
            ns_string!(""),
        );
    }
    app_menu.addItem(&NSMenuItem::separatorItem(mtm));
    unsafe {
        app_menu.addItemWithTitle_action_keyEquivalent(
            ns_string!("Quit mdview"),
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
            ns_string!("Open..."),
            Some(sel!(openDocument:)),
            ns_string!("o"),
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

fn start_file_watcher() -> notify_debouncer_mini::Debouncer<notify::RecommendedWatcher> {
    let path = get_file_path();

    let mut debouncer = new_debouncer(Duration::from_millis(200), move |result| match result {
        Ok(_events) => {
            NEEDS_RELOAD.store(true, Ordering::Relaxed);
        }
        Err(err) => {
            eprintln!("File watch error: {err}");
        }
    })
    .expect("Failed to create file watcher");

    debouncer
        .watcher()
        .watch(&path, notify::RecursiveMode::NonRecursive)
        .unwrap_or_else(|e| eprintln!("Failed to watch file: {e}"));

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
    if args.len() < 2 {
        eprintln!("Usage: mdview <file.md>");
        std::process::exit(1);
    }

    let path = std::fs::canonicalize(&args[1]).unwrap_or_else(|_| PathBuf::from(&args[1]));
    set_file_path(path);

    let mtm = MainThreadMarker::new().unwrap();
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

    let delegate = AppDelegate::new(mtm);
    let object = ProtocolObject::from_ref(&*delegate);
    app.setDelegate(Some(object));

    app.run();
}
