pub const TEMPLATE: &str = include_str!("../web/index.html");
pub const CSS: &str = include_str!("../web/styles.css");
pub const JS: &str = include_str!("../web/app.js");
pub const HIGHLIGHT: &str = include_str!("../web/highlight.js");

pub fn page() -> String {
    TEMPLATE
        .replace("/*__CSS__*/", CSS)
        .replace("/*__HL__*/", HIGHLIGHT)
        .replace("/*__JS__*/", JS)
}

/// Build a self-contained HTML file from the already rendered document.
/// `theme` is "light" / "dark", `face` is "sans" / "serif" (正文黑体或宋体).
pub fn standalone(title: &str, source: &str, body: &str, theme: &str, face: &str) -> String {
    let stamp = timestamp();
    let theme_class = if theme == "light" { "theme-light" } else { "theme-dark" };
    let face = if face == "serif" { "serif" } else { "sans" };
    format!(
        r#"<!doctype html>
<html lang="zh-CN" class="{theme_class}" data-face="{face}">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>
{css}
html, body {{ height: auto; overflow: auto; }}
#view {{ overflow: visible; }}
#page {{ min-height: 0; padding-top: 32px; padding-bottom: 120px; }}
.export-head {{
  display: flex; flex-wrap: wrap; align-items: baseline; gap: 2px 16px;
  margin: 0 0 3em; padding-bottom: 12px;
  border-bottom: 1px solid var(--rule);
  font-family: var(--ui); font-size: 12px; line-height: 1.6; color: var(--ink-3);
}}
.export-head b {{ color: var(--ink-2); font-size: 12.5px; font-weight: 600; }}
.export-head .grow {{ flex: 1 1 auto; }}
.export-head .src {{ overflow-wrap: anywhere; }}
</style>
</head>
<body>
<main id="view"><article id="page">
<header class="export-head"><b>{title}</b><span class="grow"></span><span class="src">{source}</span><span>导出于 {stamp}</span></header>
<div id="doc">{body}</div>
</article></main>
<script>{highlight}</script>
<script>window.moyueDecorate(document.getElementById('doc'));</script>
</body>
</html>
"#,
        title = escape(title),
        css = CSS,
        theme_class = theme_class,
        face = face,
        body = body,
        source = escape(source),
        stamp = stamp,
        highlight = HIGHLIGHT,
    )
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn timestamp() -> String {
    // No chrono dependency: ask the OS for local time.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let _ = stamp;
    match local_time() {
        Some(s) => s,
        None => fallback_time(stamp),
    }
}

fn local_time() -> Option<String> {
    let out = std::process::Command::new("cmd")
        .args(["/c", "echo", "%date% %time%"])
        .creation_flags_no_window()
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
    if text.is_empty() || text.contains('%') {
        None
    } else {
        Some(text)
    }
}

fn fallback_time(seconds: u64) -> String {
    format!("unix {seconds}")
}

trait NoWindow {
    fn creation_flags_no_window(&mut self) -> &mut Self;
}

impl NoWindow for std::process::Command {
    fn creation_flags_no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            self.creation_flags(0x0800_0000);
        }
        self
    }
}
