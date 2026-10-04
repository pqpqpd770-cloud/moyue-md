#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod markdown;
mod translate;
mod web;
mod win;
mod assoc;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, UNIX_EPOCH};

use serde_json::{Value, json};
use tao::dpi::{LogicalPosition, LogicalSize};
use tao::event::{Event, WindowEvent};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy};
use tao::window::{Fullscreen, Icon, Window, WindowBuilder};
use wry::{DragDropEvent, WebView, WebViewBuilder, WebViewBuilderExtWindows};

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;

const APP_TITLE: &str = "墨阅 · Markdown 预览器";
const ICON_RGBA: &[u8] = include_bytes!("../assets/icon_window.rgba");
const ICON_SIZE: u32 = 128;
const MD_EXT: [&str; 8] = ["md", "markdown", "mdown", "mkd", "mkdn", "mdtxt", "mdtext", "txt"];

enum UserEvent {
    Ipc(String),
    Changed,
    Drag(String, Vec<PathBuf>),
    /// A message produced by a background thread, forwarded straight to the page.
    Push(Value),
    Quit,
}

#[derive(Default)]
struct Watch {
    path: Option<PathBuf>,
    stamp: Option<(u128, u64)>,
}

impl Watch {
    fn stat(path: &Path) -> Option<(u128, u64)> {
        let meta = std::fs::metadata(path).ok()?;
        let millis = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_millis();
        Some((millis, meta.len()))
    }

    fn follow(&mut self, path: &Path) {
        self.path = Some(path.to_path_buf());
        self.stamp = Self::stat(path);
    }

    fn poll(&mut self) -> bool {
        let Some(path) = self.path.clone() else {
            return false;
        };
        let now = Self::stat(&path);
        if now != self.stamp {
            self.stamp = now;
            true
        } else {
            false
        }
    }
}

struct App {
    cfg: config::Config,
    path: Option<PathBuf>,
    theme: String,
    face: String,
    font_size: u32,
    toc: bool,
    watch: Arc<Mutex<Watch>>,
    proxy: EventLoopProxy<UserEvent>,
    /// Bumped for every translation request; workers stop as soon as a newer one
    /// arrives, so switching language does not stack up translations.
    translation_run: Arc<AtomicU64>,
    pending: Option<PathBuf>,
    /// Markdown source of the open document, kept so whole-document translation
    /// can work on the original text instead of the rendered DOM.
    source: String,
    rendered_html: String,
    ready: bool,
}

impl App {
    fn new(
        cfg: config::Config,
        watch: Arc<Mutex<Watch>>,
        proxy: EventLoopProxy<UserEvent>,
        pending: Option<PathBuf>,
    ) -> Self {
        Self {
            theme: cfg.theme(),
            face: cfg.face(),
            font_size: cfg.font_size(),
            toc: cfg.show_toc(),
            cfg,
            path: None,
            watch,
            proxy,
            translation_run: Arc::new(AtomicU64::new(0)),
            pending,
            source: String::new(),
            rendered_html: String::new(),
            ready: false,
        }
    }

    /// Copy text to the Windows clipboard and tell the page how it went.
    fn copy(&self, webview: &WebView, text: &str) {
        match win::set_clipboard(text) {
            Ok(()) => self.send(webview, json!({ "cmd": "copied", "ok": true })),
            Err(err) => {
                self.send(webview, json!({ "cmd": "copied", "ok": false }));
                self.toast(webview, err, "err");
            }
        }
    }

    /// Translate on a worker thread so the window never freezes.
    fn start_translation(
        &self,
        text: String,
        target: Option<String>,
        from_document: bool,
        comments: bool,
    ) {
        if text.trim().is_empty() {
            return;
        }
        let proxy = self.proxy.clone();
        let run = self.translation_run.fetch_add(1, Ordering::SeqCst) + 1;
        let generation = Arc::clone(&self.translation_run);
        let base = self
            .path
            .as_ref()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        thread::spawn(move || {
            let current = || generation.load(Ordering::SeqCst) == run;
            let wanted = target.unwrap_or_else(|| "auto".into());
            let resolved = translate::normalise_target(&wanted, &text);

            let _ = proxy.send_event(UserEvent::Push(json!({
                "cmd": "translation",
                "state": "start",
                "target": resolved,
                "fromDocument": from_document,
                "run": run,
            })));

            let mut progress = |done: usize, total: usize| {
                if !current() {
                    return;
                }
                let _ = proxy.send_event(UserEvent::Push(json!({
                    "cmd": "translation",
                    "state": "progress",
                    "done": done,
                    "total": total,
                    "run": run,
                })));
            };

            let options = translate::Options { comments };
            let outcome = translate::translate_document(&text, &wanted, &base, options, &mut progress);
            if !current() {
                // A newer request took over; drop this result silently.
                return;
            }
            let message = match outcome {
                Ok(result) => json!({
                    "cmd": "translation",
                    "state": "done",
                    "html": result.html,
                    "markdown": result.markdown,
                    "target": result.target,
                    "blocks": result.blocks,
                    "comments": result.comments,
                    "source": text,
                    "run": run,
                }),
                Err(err) => json!({
                    "cmd": "translation",
                    "state": "error",
                    "message": err,
                    "target": resolved,
                    "run": run,
                }),
            };
            let _ = proxy.send_event(UserEvent::Push(message));
        });
    }

    /* ------------------------------------------------------------- messaging */

    fn send(&self, webview: &WebView, message: Value) {
        let payload = serde_json::to_string(&message.to_string()).unwrap_or_else(|_| "\"{}\"".into());
        let js = format!("window.Moyue && window.Moyue.receive({payload});");
        let _ = webview.evaluate_script(&js);
    }

    fn toast(&self, webview: &WebView, text: impl Into<String>, kind: &str) {
        self.send(webview, json!({ "cmd": "toast", "text": text.into(), "kind": kind }));
    }

    fn push_state(&self, webview: &WebView) {
        self.send(
            webview,
            json!({
                "cmd": "state",
                "theme": self.theme,
                "face": self.face,
                "fontSize": self.font_size,
                "toc": self.toc,
                "recent": self.cfg.recent,
            }),
        );
    }

    /// The recent list changes as soon as a file is opened, so the page has to
    /// be told - otherwise the toolbar dropdown stays empty for the whole
    /// session.
    fn push_recent(&self, webview: &WebView) {
        self.send(
            webview,
            json!({ "cmd": "recent", "recent": self.cfg.recent }),
        );
    }

    /* ---------------------------------------------------------------- files */

    fn open(
        &mut self,
        webview: &WebView,
        window: &Window,
        path: &Path,
        keep_scroll: bool,
        quiet: bool,
    ) {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) => {
                if !quiet {
                    self.toast(webview, format!("打不开 {}：{}", file_name(path), err), "err");
                    self.cfg.drop_recent(&path.to_string_lossy());
                    self.cfg.save();
                    self.push_recent(webview);
                }
                return;
            }
        };

        if bytes.len() > 8 * 1024 * 1024 && !quiet {
            self.toast(
                webview,
                format!("文件较大（{:.1} MB），渲染可能需要几秒", bytes.len() as f64 / 1048576.0),
                "ok",
            );
        }

        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        let (text, encoding) = markdown::decode(&bytes);
        let rendered = markdown::render(&text, &base);

        self.path = Some(path.to_path_buf());
        self.source = text;
        self.rendered_html = rendered.html.clone();
        self.watch.lock().unwrap().follow(path);
        self.cfg.push_recent(&path.to_string_lossy());
        self.cfg.save();
        if !quiet {
            self.push_recent(webview);
        }
        window.set_title(&format!("{} · 墨阅", file_name(path)));

        self.send(
            webview,
            json!({
                "cmd": "render",
                "html": rendered.html,
                "keepScroll": keep_scroll,
                "meta": {
                    "name": file_name(path),
                    "path": path.to_string_lossy(),
                    "dir": base.to_string_lossy(),
                    "enc": encoding,
                    "size": bytes.len(),
                    "stats": rendered.stats,
                }
            }),
        );
    }

    fn reload(&mut self, webview: &WebView, window: &Window, quiet: bool) {
        let Some(path) = self.path.clone() else {
            if !quiet {
                self.toast(webview, "还没有打开文件", "err");
            }
            return;
        };
        self.open(webview, window, &path, true, quiet);
    }

    fn export(&mut self, webview: &WebView, window: &Window) {
        let Some(path) = self.path.clone() else {
            self.toast(webview, "先打开一个文件再导出", "err");
            return;
        };
        if self.rendered_html.is_empty() {
            self.toast(webview, "文档还没渲染完", "err");
            return;
        }

        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "document".into());
        let default_name = format!("{stem}.html");
        let dialog = rfd::FileDialog::new()
            .set_file_name(&default_name)
            .add_filter("网页", &["html"]);
        let dialog = match path.parent() {
            Some(dir) => dialog.set_directory(dir),
            None => dialog,
        };

        let Some(target) = dialog.save_file() else {
            return;
        };

        let page = web::standalone(
            &stem,
            &path.to_string_lossy(),
            &self.rendered_html,
            &self.theme,
            &self.face,
        );
        match std::fs::write(&target, page) {
            Ok(()) => {
                self.toast(webview, format!("已导出：{}", target.to_string_lossy()), "ok");
                let _ = window;
            }
            Err(err) => self.toast(webview, format!("导出失败：{err}"), "err"),
        }
    }

    fn open_link(&self, webview: &WebView, target: &str) {
        let trimmed = target.trim();
        if trimmed.is_empty() {
            return;
        }
        if trimmed.starts_with("http://") || trimmed.starts_with("https://") || trimmed.starts_with("mailto:") {
            if let Err(err) = open::that_detached(trimmed) {
                self.toast(webview, format!("打不开链接：{err}"), "err");
            }
            return;
        }
        if trimmed.starts_with("data:") {
            return;
        }

        let base = self
            .path
            .as_ref()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        let resolved = markdown::resolve(&base, trimmed);

        let is_md = resolved
            .extension()
            .map(|e| {
                let ext = e.to_string_lossy().to_ascii_lowercase();
                MD_EXT.contains(&ext.as_str())
            })
            .unwrap_or(false);

        if is_md {
            let _ = std::process::Command::new(std::env::current_exe().unwrap_or_default())
                .arg(&resolved)
                .spawn();
        } else if resolved.exists() {
            let _ = open::that_detached(&resolved);
        } else {
            self.toast(webview, format!("找不到：{}", resolved.to_string_lossy()), "err");
        }
    }

    /* --------------------------------------------------------------- chrome */

    fn save_window(&mut self, window: &Window) {
        let maximized = window.is_maximized();
        self.cfg.window.maximized = maximized;
        if !maximized {
            let scale = window.scale_factor();
            let size = window.inner_size().to_logical::<f64>(scale);
            if size.width > 200.0 && size.height > 150.0 {
                self.cfg.window.width = Some(size.width);
                self.cfg.window.height = Some(size.height);
            }
            if let Ok(position) = window.outer_position() {
                let point = position.to_logical::<f64>(scale);
                self.cfg.window.x = Some(point.x);
                self.cfg.window.y = Some(point.y);
            }
        }
        self.cfg.save();
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    // Development helper: render a file to stdout and exit (no window).
    if let Some(index) = args.iter().position(|a| a == "--dump") {
        let target = args.get(index + 1).cloned().unwrap_or_default();
        match std::fs::read(&target) {
            Ok(bytes) => {
                let (text, encoding) = markdown::decode(&bytes);
                let base = Path::new(&target).parent().unwrap_or(Path::new("."));
                let rendered = markdown::render(&text, base);
                println!("<!-- encoding: {encoding} -->");
                println!("{}", rendered.html);
                println!(
                    "<!-- stats: {} -->",
                    serde_json::to_string(&rendered.stats).unwrap()
                );
            }
            Err(err) => eprintln!("read failed: {err}"),
        }
        return;
    }

    let devtools = args.iter().any(|a| a == "--devtools");

    // Development helper: translate a string from the command line.
    if args.iter().any(|a| a == "--associate") {
        let exe = std::env::current_exe().unwrap_or_default();
        match assoc::register(&exe) {
            Ok(()) => {
                match assoc::current_default() {
                    Some(current) if !assoc::is_ours(&current) => {
                        println!("registered; Windows still prefers {}", assoc::describe(&current));
                    }
                    _ => println!("registered; .md now opens with MoyueMD"),
                };
            }
            Err(err) => eprintln!("failed: {err}"),
        }
        return;
    }

    if let Some(index) = args.iter().position(|a| a == "--translate") {
        // A path is more convenient than a quoted string for multi-line tests.
        let joined = args[index + 1..].join(" ");
        let text = std::fs::read_to_string(&joined).unwrap_or(joined);
        let wanted = std::env::var("MOYUE_TARGET").unwrap_or_else(|_| "auto".into());
        let target = translate::normalise_target(&wanted, &text);
        eprintln!("target: {target}");
        let mut progress = |done: usize, total: usize| eprintln!("chunk {done}/{total}");
        let options = translate::Options { comments: true };
        match translate::translate_document(&text, &wanted, Path::new("."), options, &mut progress) {
            Ok(result) => println!("{}", result.markdown),
            Err(err) => eprintln!("failed: {err}"),
        }
        return;
    }

    let pending = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .map(PathBuf::from)
        .filter(|p| p.is_file());

    let cfg = config::Config::load();
    let theme = cfg.theme();
    let watch = Arc::new(Mutex::new(Watch::default()));

    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    spawn_watcher(proxy.clone(), Arc::clone(&watch));

    let icon = Icon::from_rgba(ICON_RGBA.to_vec(), ICON_SIZE, ICON_SIZE).ok();

    // Size and place the window inside whatever monitors actually exist, so a
    // high-DPI laptop panel never ends up with a window taller than the screen.
    let mut virtual_rect = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for monitor in event_loop.available_monitors() {
        let scale = monitor.scale_factor();
        let position = monitor.position().to_logical::<f64>(scale);
        let size = monitor.size().to_logical::<f64>(scale);
        virtual_rect.0 = virtual_rect.0.min(position.x);
        virtual_rect.1 = virtual_rect.1.min(position.y);
        virtual_rect.2 = virtual_rect.2.max(position.x + size.width);
        virtual_rect.3 = virtual_rect.3.max(position.y + size.height);
    }
    let (area_x, area_y, area_right, area_bottom) = if virtual_rect.0 == f64::MAX {
        (0.0, 0.0, 1920.0, 1080.0)
    } else {
        virtual_rect
    };
    let area_w = (area_right - area_x).max(640.0);
    let area_h = (area_bottom - area_y).max(420.0);

    let (primary_x, primary_y, primary_w, primary_h) = match event_loop.primary_monitor() {
        Some(monitor) => {
            let scale = monitor.scale_factor();
            let position = monitor.position().to_logical::<f64>(scale);
            let size = monitor.size().to_logical::<f64>(scale);
            (position.x, position.y, size.width, size.height)
        }
        None => (area_x, area_y, area_w, area_h),
    };

    let default_w = 1080.0f64.min(primary_w * 0.86);
    let default_h = 740.0f64.min(primary_h * 0.84);
    let width = cfg
        .window
        .width
        .unwrap_or(default_w)
        .clamp(520.0, area_w.max(520.0));
    let height = cfg
        .window
        .height
        .unwrap_or(default_h)
        .clamp(340.0, area_h.max(340.0));

    let mut builder = WindowBuilder::new()
        .with_title(APP_TITLE)
        .with_window_icon(icon)
        .with_decorations(false)
        .with_inner_size(LogicalSize::new(width, height))
        .with_min_inner_size(LogicalSize::new(560.0, 360.0));

    let placement = match (cfg.window.x, cfg.window.y) {
        (Some(x), Some(y)) if x.is_finite() && y.is_finite() => {
            let x = x.clamp(area_x - width + 180.0, area_right - 180.0);
            let y = y.clamp(area_y, area_bottom - 90.0);
            LogicalPosition::new(x, y)
        }
        _ => LogicalPosition::new(
            primary_x + (primary_w - width) / 2.0,
            primary_y + (primary_h - height) / 2.0 - 16.0,
        ),
    };
    builder = builder.with_position(placement);
    let window = builder.build(&event_loop).expect("无法创建窗口");
    if cfg.window.maximized {
        window.set_maximized(true);
    }

    // Same as --paper in styles.css, so the first frame is already the right colour.
    let background = if theme == "light" {
        (250, 250, 248, 255)
    } else {
        (27, 29, 34, 255)
    };

    // Keep WebView2's cache out of whatever folder the exe happens to live in,
    // so the program really is a single portable file.
    let mut web_context = wry::WebContext::new(user_data_dir());

    let ipc_proxy = proxy.clone();
    let drag_proxy = proxy.clone();
    let webview = WebViewBuilder::new_with_web_context(&mut web_context)
        .with_html(web::page())
        .with_background_color(background)
        .with_default_context_menus(false)
        .with_browser_accelerator_keys(false)
        .with_hotkeys_zoom(false)
        // Lets the page declare its own title bar with `-webkit-app-region: drag`,
        // which is what makes the window draggable without native decorations.
        // wry's own defaults have to be repeated when this is set.
        .with_additional_browser_args(
            "--disable-features=msWebOOUI,msPdfOOUI,msSmartScreenProtection \
             --autoplay-policy=no-user-gesture-required \
             --enable-features=msWebView2EnableDraggableRegions",
        )
        .with_devtools(devtools)
        .with_navigation_handler(|url| url.starts_with("about:") || url.starts_with("data:"))
        .with_ipc_handler(move |request| {
            let _ = ipc_proxy.send_event(UserEvent::Ipc(request.body().clone()));
        })
        .with_drag_drop_handler(move |event| {
            let (kind, paths) = match event {
                DragDropEvent::Enter { paths, .. } => ("enter", paths),
                DragDropEvent::Over { .. } => ("over", Vec::new()),
                DragDropEvent::Drop { paths, .. } => ("drop", paths),
                DragDropEvent::Leave => ("leave", Vec::new()),
                _ => ("over", Vec::new()),
            };
            let _ = drag_proxy.send_event(UserEvent::Drag(kind.to_string(), paths));
            false
        })
        .build(&window)
        .expect("无法创建 WebView2 视图");

    let mut app = App::new(cfg, watch, proxy.clone(), pending);
    let hwnd = window_hwnd(&window);
    if let Some(hwnd) = hwnd {
        win::dress_window(hwnd, app.theme != "light", window.is_maximized());
        win::keep_maximized_client_area(hwnd);
    }

    event_loop.run(move |event, _target, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                app.save_window(&window);
                *control_flow = ControlFlow::Exit;
            }
            Event::WindowEvent {
                event: WindowEvent::Resized(_),
                ..
            } => {
                if let Some(hwnd) = hwnd {
                    win::dress_window(hwnd, app.theme != "light", window.is_maximized());
                }
                app.send(
                    &webview,
                    json!({ "cmd": "window", "maximized": window.is_maximized() }),
                );
            }
            Event::UserEvent(UserEvent::Ipc(raw)) => handle_ipc(&mut app, &webview, &window, &raw),
            Event::UserEvent(UserEvent::Push(message)) => app.send(&webview, message),
            Event::UserEvent(UserEvent::Quit) => {
                app.save_window(&window);
                *control_flow = ControlFlow::Exit;
            }
            Event::UserEvent(UserEvent::Changed) => {
                let changed = app.watch.lock().map(|mut w| w.poll()).unwrap_or(false);
                if changed {
                    app.reload(&webview, &window, true);
                }
            }
            Event::UserEvent(UserEvent::Drag(kind, paths)) => {
                app.send(&webview, json!({ "cmd": "drag", "state": kind }));
                if kind == "drop" {
                    if let Some(path) = paths.into_iter().next() {
                        app.open(&webview, &window, &path, false, false);
                    }
                }
            }
            Event::LoopDestroyed => {
                app.save_window(&window);
            }
            _ => {}
        }
    });
}

fn spawn_watcher(proxy: EventLoopProxy<UserEvent>, watch: Arc<Mutex<Watch>>) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(420));
            let active = watch.lock().map(|w| w.path.is_some()).unwrap_or(false);
            if active {
                let _ = proxy.send_event(UserEvent::Changed);
            }
        }
    });
}

fn handle_ipc(app: &mut App, webview: &WebView, window: &Window, raw: &str) {
    let Ok(message) = serde_json::from_str::<Value>(raw) else {
        return;
    };
    let cmd = message.get("cmd").and_then(Value::as_str).unwrap_or_default();
    let value = message.get("value");

    match cmd {
        "ready" => {
            app.ready = true;
            app.push_state(webview);
            match app.pending.take() {
                Some(path) => app.open(webview, window, &path, false, false),
                None => app.send(
                    webview,
                    json!({ "cmd": "empty", "recent": app.cfg.recent }),
                ),
            }
        }
        "open" => {
            let dialog = rfd::FileDialog::new()
                .add_filter("Markdown", &["md", "markdown", "mdown", "mkd", "txt"])
                .add_filter("所有文件", &["*"]);
            if let Some(path) = dialog.pick_file() {
                app.open(webview, window, &path, false, false);
            }
        }
        "openPath" => {
            if let Some(path) = value.and_then(Value::as_str) {
                app.open(webview, window, Path::new(path), false, false);
            }
        }
        "reload" => app.reload(webview, window, false),
        "theme" => {
            if let Some(next) = value.and_then(Value::as_str) {
                app.theme = next.to_string();
                app.cfg.theme = Some(next.to_string());
                app.cfg.save();
                if let Some(hwnd) = window_hwnd(window) {
                    win::dress_window(hwnd, app.theme != "light", window.is_maximized());
                }
            }
        }
        "face" => {
            if let Some(next) = value.and_then(Value::as_str) {
                app.face = if next == "serif" { "serif".into() } else { "sans".into() };
                app.cfg.face = Some(app.face.clone());
                app.cfg.save();
            }
        }
        "font" => {
            if let Some(size) = value.and_then(Value::as_u64) {
                app.font_size = (size as u32).clamp(12, 30);
                app.cfg.font_size = Some(app.font_size);
                app.cfg.save();
            }
        }
        "toc" => {
            if let Some(flag) = value.and_then(Value::as_bool) {
                app.toc = flag;
                app.cfg.show_toc = Some(flag);
                app.cfg.save();
            }
        }
        "export" => app.export(webview, window),
        "copy" => {
            if let Some(text) = value.and_then(Value::as_str) {
                app.copy(webview, text);
            }
        }
        "translate" => {
            let mut text = value
                .and_then(|v| v.get("text"))
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string();
            let target = value
                .and_then(|v| v.get("target"))
                .and_then(Value::as_str)
                .map(str::to_string);
            let from_document = value
                .and_then(|v| v.get("fromDocument"))
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let comments = value
                .and_then(|v| v.get("comments"))
                .and_then(Value::as_bool)
                .unwrap_or(true);
            if from_document && !app.source.is_empty() {
                text = app.source.clone();
            }
            if text.trim().is_empty() {
                app.toast(webview, "先选中要翻译的文字，或者打开一个文档", "err");
            } else {
                app.start_translation(text, target, from_document, comments);
            }
        }
        "clearRecent" => {
            app.cfg.recent.clear();
            app.cfg.save();
            app.push_recent(webview);
            if app.path.is_none() {
                app.send(webview, json!({ "cmd": "empty", "recent": app.cfg.recent }));
            } else {
                app.push_state(webview);
            }
        }
        "link" => {
            if let Some(target) = value.and_then(Value::as_str) {
                app.open_link(webview, target);
            }
        }
        "fullscreen" => {
            if window.fullscreen().is_some() {
                window.set_fullscreen(None);
            } else {
                window.set_fullscreen(Some(Fullscreen::Borderless(None)));
            }
        }
        "win" => match value.and_then(Value::as_str).unwrap_or_default() {
            "minimize" => window.set_minimized(true),
            "maximize" => window.set_maximized(!window.is_maximized()),
            "close" => {
                let _ = app.proxy.send_event(UserEvent::Quit);
            }
            _ => {}
        },
        "jsError" => {
            if let Some(text) = value.and_then(Value::as_str) {
                app.toast(webview, format!("界面出错：{text}"), "err");
            }
        }
        "associate" => {
            let exe = std::env::current_exe().unwrap_or_default();
            match assoc::register(&exe) {
                Ok(()) => {
                    let note = match assoc::current_default() {
                        Some(current) if !assoc::is_ours(&current) => format!(
                            "已注册到「打开方式」。Windows 目前仍指向 {}，点「⋯ → Windows 默认应用设置」改一下即可。",
                            assoc::describe(&current)
                        ),
                        _ => "已设为 .md 的默认打开程序。双击 Markdown 文件就会用墨阅打开。".to_string(),
                    };
                    app.toast(webview, note, "ok");
                }
                Err(err) => app.toast(webview, format!("注册失败：{err}"), "err"),
            }
        }
        "assocSettings" => {
            if let Err(err) = open::that_detached("ms-settings:defaultapps") {
                app.toast(webview, format!("打不开系统设置：{err}"), "err");
            }
        }
        _ => {}
    }
}

fn window_hwnd(window: &Window) -> Option<HWND> {
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(HWND(win32.hwnd.get() as *mut std::ffi::c_void)),
        _ => None,
    }
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string_lossy().to_string())
}

/// `%LOCALAPPDATA%\MoyueMD\webview` — WebView2 browser profile lives here.
fn user_data_dir() -> Option<PathBuf> {
    let base = std::env::var("LOCALAPPDATA")
        .or_else(|_| std::env::var("APPDATA"))
        .ok()?;
    let dir = PathBuf::from(base).join("MoyueMD").join("webview");
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}
