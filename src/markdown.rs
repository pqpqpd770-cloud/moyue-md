use base64::Engine as _;
use base64::engine::general_purpose::STANDARD as B64;
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Images above this size stay as plain links instead of being inlined.
const MAX_INLINE_IMAGE: usize = 8 * 1024 * 1024;

#[derive(Serialize, Default, Clone)]
pub struct Stats {
    pub lines: usize,
    pub chars: usize,
    pub cjk: usize,
    pub words: usize,
    pub minutes: usize,
    pub headings: usize,
    pub images: usize,
    pub tables: usize,
    pub code_blocks: usize,
}

#[derive(Serialize)]
pub struct Rendered {
    pub html: String,
    pub stats: Stats,
}

/// Decode a file into text, honouring BOMs and falling back to GBK for legacy
/// Chinese documents that are not valid UTF-8.
pub fn decode(bytes: &[u8]) -> (String, String) {
    if bytes.starts_with(&[0xEF, 0xBB, 0xBF]) {
        return (
            String::from_utf8_lossy(&bytes[3..]).into_owned(),
            "UTF-8 BOM".into(),
        );
    }
    if bytes.starts_with(&[0xFF, 0xFE]) || bytes.starts_with(&[0xFE, 0xFF]) {
        let (text, _, _) = encoding_rs::UTF_16LE.decode(bytes);
        return (text.into_owned(), "UTF-16".into());
    }
    match String::from_utf8(bytes.to_vec()) {
        Ok(text) => (text, "UTF-8".into()),
        Err(_) => {
            let (text, _, had_errors) = encoding_rs::GBK.decode(bytes);
            (text.into_owned(), if had_errors { "GBK?".into() } else { "GBK".into() })
        }
    }
}

pub fn options() -> Options {
    Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_HEADING_ATTRIBUTES
        | Options::ENABLE_MATH
        | Options::ENABLE_DEFINITION_LIST
        | Options::ENABLE_SUPERSCRIPT
        | Options::ENABLE_SUBSCRIPT
}

pub fn render(source: &str, base: &Path) -> Rendered {
    let stats = stats(source);
    let parser = Parser::new_ext(source, options());

    let mut events: Vec<Event> = Vec::with_capacity(source.len() / 8 + 16);
    for event in parser {
        match event {
            Event::Start(Tag::Image {
                dest_url,
                title,
                id,
                link_type,
            }) => {
                let dest_url = match inline_image(&dest_url, base) {
                    Some(data_uri) => CowStr::from(data_uri),
                    None => dest_url,
                };
                events.push(Event::Start(Tag::Image {
                    dest_url,
                    title,
                    id,
                    link_type,
                }));
            }
            other => events.push(other),
        }
    }

    let mut raw = String::with_capacity(source.len() * 2 + 64);
    html::push_html(&mut raw, events.into_iter());
    let html = with_heading_ids(&raw);

    Rendered { html, stats }
}

pub fn stats(source: &str) -> Stats {
    let mut out = Stats {
        lines: source.lines().count(),
        ..Default::default()
    };
    let mut in_code = false;
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            if !in_code {
                out.code_blocks += 1;
            }
            in_code = !in_code;
            continue;
        }
        if in_code {
            continue;
        }
        let mut hashes = 0;
        for ch in trimmed.chars() {
            if ch == '#' {
                hashes += 1;
            } else {
                break;
            }
        }
        if (1..=6).contains(&hashes) && trimmed.chars().nth(hashes) == Some(' ') {
            out.headings += 1;
        }
        if trimmed.contains('|')
            && trimmed.matches('|').count() >= 2
            && trimmed.contains('-')
            && trimmed.chars().all(|c| "-:| \t".contains(c))
        {
            out.tables += 1;
        }
    }

    let mut latin_run = 0usize;
    for ch in source.chars() {
        if ch.is_whitespace() {
            continue;
        }
        out.chars += 1;
        if is_cjk(ch) {
            out.cjk += 1;
            latin_run = 0;
        } else if ch.is_alphanumeric() || ch == '\'' || ch == '-' || ch == '_' {
            latin_run += 1;
        } else {
            if latin_run > 0 {
                out.words += 1;
                latin_run = 0;
            }
        }
    }
    if latin_run > 0 {
        out.words += 1;
    }
    out.images = source.match_indices("![").count();

    // ~300 CJK chars or ~200 latin words per minute.
    let cjk_minutes = out.cjk as f64 / 300.0;
    let word_minutes = out.words as f64 / 200.0;
    out.minutes = (cjk_minutes + word_minutes).ceil().max(1.0) as usize;
    out
}

fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3000..=0x303F | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF |
        0xFF00..=0xFFEF | 0x20000..=0x2FA1F)
}

/// Add `id="sec-N"` to headings (unless the author already supplied one) so the
/// table of contents can jump around. The TOC labels themselves are read from the
/// rendered DOM in JavaScript.
fn with_heading_ids(html: &str) -> String {
    let bytes = html.as_bytes();
    let mut out = String::with_capacity(html.len() + 64);
    let mut cursor = 0usize;
    let mut index = 0usize;

    while let Some(found) = html[cursor..].find("<h") {
        let start = cursor + found;
        let level_ok = matches!(bytes.get(start + 2), Some(b'1'..=b'6'));
        let tag_ok = matches!(bytes.get(start + 3), Some(b'>') | Some(b' '));
        if !(level_ok && tag_ok) {
            out.push_str(&html[cursor..start + 2]);
            cursor = start + 2;
            continue;
        }
        index += 1;
        out.push_str(&html[cursor..start]);
        if bytes.get(start + 3) == Some(&b'>') {
            out.push_str(&html[start..start + 3]);
            out.push_str(&format!(" id=\"sec-{index}\""));
            out.push('>');
        } else {
            // Author-supplied attributes: keep them, only add an id if missing.
            let close = html[start..].find('>').map(|i| start + i).unwrap_or(start);
            let tag = &html[start..close];
            out.push_str(tag);
            if !tag.contains("id=") {
                out.push_str(&format!(" id=\"sec-{index}\""));
            }
            out.push('>');
            cursor = close + 1;
            continue;
        }
        cursor = start + 4;
    }
    out.push_str(&html[cursor..]);
    out
}

/// Turn a local image reference into a self-contained data URI. Remote and already
/// inlined sources are left untouched.
fn inline_image(url: &str, base: &Path) -> Option<String> {
    let trimmed = url.trim();
    let lower = trimmed.to_ascii_lowercase();
    if trimmed.is_empty()
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("data:")
        || lower.starts_with("//")
    {
        return None;
    }

    let decoded = percent_decode(trimmed);
    let path = resolve(base, &decoded);
    let bytes = std::fs::read(&path).ok()?;
    if bytes.len() > MAX_INLINE_IMAGE {
        return None;
    }
    let mime = mime_for(&path);
    Some(format!("data:{};base64,{}", mime, B64.encode(bytes)))
}

pub fn resolve(base: &Path, raw: &str) -> PathBuf {
    let cleaned = raw.split(['?', '#']).next().unwrap_or(raw).replace('\\', "/");
    let candidate = PathBuf::from(&cleaned);
    if candidate.is_absolute() {
        candidate
    } else {
        base.join(candidate)
    }
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hex = std::str::from_utf8(&bytes[i + 1..i + 3]).ok();
            if let Some(value) = hex.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(value);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mime_for(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" | "jpe" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "avif" => "image/avif",
        "apng" => "image/apng",
        _ => "application/octet-stream",
    }
}
