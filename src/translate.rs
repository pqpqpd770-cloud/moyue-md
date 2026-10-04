//! Markdown-aware translation.
//!
//! The source is split into blocks: fenced code, front matter and indented code
//! pass through untouched, prose is sent to the public Google endpoint. Line
//! structure is kept when the service answers with the same number of lines,
//! which is what keeps headings, lists and tables readable.
//! Everything here runs off the UI thread.

use crate::markdown;
use crate::win;
use serde_json::Value;
use std::path::Path;

/// Stay well under any URL limit: percent-encoded CJK costs nine characters per
/// glyph, so this is roughly 480 Chinese characters per request.
const MAX_QUERY_CHARS: usize = 4400;
const TIMEOUT_MS: i32 = 20_000;

pub const LANGUAGES: [&str; 10] = [
    "zh-CN", "zh-TW", "en", "ja", "ko", "ru", "de", "fr", "es", "it",
];

pub struct Translated {
    pub markdown: String,
    pub html: String,
    pub target: String,
    pub blocks: usize,
    pub comments: usize,
}

pub fn target_for(text: &str) -> String {
    let mut cjk = 0usize;
    let mut other = 0usize;
    for ch in text.chars() {
        if ch.is_whitespace() {
            continue;
        }
        if is_cjk(ch) {
            cjk += 1;
        } else if ch.is_alphanumeric() {
            other += 1;
        }
    }
    if cjk + other == 0 {
        return "zh-CN".into();
    }
    if (cjk as f64) / ((cjk + other) as f64) > 0.2 {
        "en".into()
    } else {
        "zh-CN".into()
    }
}

/// `auto` picks a sensible direction; anything else is validated against the
/// list of languages we offer.
pub fn normalise_target(target: &str, source: &str) -> String {
    let wanted = target.trim();
    if wanted.is_empty() || wanted.eq_ignore_ascii_case("auto") {
        return target_for(source);
    }
    LANGUAGES
        .iter()
        .find(|code| code.eq_ignore_ascii_case(wanted))
        .map(|code| (*code).to_string())
        .unwrap_or_else(|| wanted.to_string())
}

pub fn is_cjk(ch: char) -> bool {
    matches!(ch as u32,
        0x3000..=0x303F | 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF)
}

/* -------------------------------------------------------------------- blocks */

struct Block {
    text: String,
    translate: bool,
    /// Fence language of a code block (lower-cased, arguments stripped).
    lang: Option<String>,
}

/// What the caller wants translated.
#[derive(Default, Clone, Copy)]
pub struct Options {
    /// Also translate comments inside fenced code blocks.
    pub comments: bool,
}

/// Split markdown into translatable prose and text that must be copied verbatim.
/// Concatenating the blocks gives back the original source.
fn split_blocks(source: &str) -> Vec<Block> {
    let mut blocks: Vec<Block> = Vec::new();
    let mut buffer = String::new();
    let mut verbatim = String::new();
    let mut fence: Option<(char, usize)> = None;
    let mut code_lang: Option<String> = None;
    let mut front_matter = false;
    let mut first = true;

    for line in source.split_inclusive('\n') {
        let bare = line.trim_end_matches(['\n', '\r']);
        let lead = bare.trim_start();
        let indent = bare.len() - lead.len();

        if front_matter {
            verbatim.push_str(line);
            if lead.trim_end() == "---" && verbatim.lines().count() > 1 {
                front_matter = false;
                blocks.push(Block {
                    text: std::mem::take(&mut verbatim),
                    translate: false,
                    lang: Some("yaml".into()),
                });
            }
            continue;
        }

        if let Some((marker, open_len)) = fence {
            verbatim.push_str(line);
            let run = lead.chars().take_while(|ch| *ch == marker).count();
            let rest = lead.chars().skip(run).all(|ch| ch.is_whitespace());
            if run >= open_len && rest && run >= 3 {
                fence = None;
                blocks.push(Block {
                    text: std::mem::take(&mut verbatim),
                    translate: false,
                    lang: code_lang.take(),
                });
            }
            continue;
        }

        let marker = lead.chars().next().unwrap_or(' ');
        let run = lead.chars().take_while(|ch| *ch == marker).count();
        let opens_fence = (marker == '`' || marker == '~') && run >= 3 && indent < 4;

        if opens_fence {
            push(&mut blocks, &mut buffer, true, None);
            push(&mut blocks, &mut verbatim, false, code_lang.take());
            fence = Some((marker, run));
            code_lang = fence_language(lead, marker, run);
            verbatim.push_str(line);
            first = false;
            continue;
        }

        if first && (lead.trim_end() == "---" || lead.trim_end() == "+++") {
            first = false;
            front_matter = true;
            verbatim.push_str(line);
            continue;
        }
        first = false;

        if bare.trim().is_empty() {
            push(&mut blocks, &mut buffer, true, None);
            verbatim.push_str(line);
            continue;
        }

        // Indented code after a blank line: leave it alone.
        if indent >= 4 && buffer.is_empty() {
            verbatim.push_str(line);
            continue;
        }

        push(&mut blocks, &mut verbatim, false, None);
        buffer.push_str(line);
    }

    push(&mut blocks, &mut verbatim, false, code_lang.take());
    push(&mut blocks, &mut buffer, true, None);
    blocks
}

fn push(blocks: &mut Vec<Block>, text: &mut String, translate: bool, lang: Option<String>) {
    if !text.is_empty() {
        blocks.push(Block {
            text: std::mem::take(text),
            translate,
            lang,
        });
    }
}

/// Pull the language out of an opening fence: "```rust ignore" -> "rust".
fn fence_language(lead: &str, marker: char, run: usize) -> Option<String> {
    let info = lead.chars().skip(run).collect::<String>();
    let info = info.trim().trim_start_matches(marker);
    let word = info
        .trim()
        .split(|ch: char| ch.is_whitespace() || ch == '{' || ch == '(' || ch == ',')
        .next()
        .unwrap_or_default()
        .trim_start_matches('.')
        .to_ascii_lowercase();
    if word.is_empty() {
        None
    } else {
        Some(word)
    }
}

/* ---------------------------------------------------------------- translate */

/* ------------------------------------------------------------- 代码注释 */

/// Comment syntax for one language family.
struct CommentStyle {
    /// (marker, must be at the start of the line or preceded by whitespace)
    line: &'static [(&'static str, bool)],
    /// (open, close)
    block: &'static [(&'static str, &'static str)],
}

const SLASH: CommentStyle = CommentStyle {
    line: &[("//", false)],
    block: &[("/*", "*/")],
};
const CSS_ONLY: CommentStyle = CommentStyle {
    line: &[],
    block: &[("/*", "*/")],
};
const PYTHON: CommentStyle = CommentStyle {
    line: &[("#", false)],
    block: &[("\"\"\"", "\"\"\""), ("'''", "'''")],
};
const HASH: CommentStyle = CommentStyle {
    line: &[("#", false)],
    block: &[],
};
const DASHDASH: CommentStyle = CommentStyle {
    line: &[("--", true)],
    block: &[],
};
const DASHDASH_BLOCK: CommentStyle = CommentStyle {
    line: &[("--", true)],
    block: &[("/*", "*/")],
};
const LUA: CommentStyle = CommentStyle {
    line: &[("--", true)],
    block: &[("--[[", "]]")],
};
const SEMI: CommentStyle = CommentStyle {
    line: &[(";", true)],
    block: &[],
};
const PERCENT: CommentStyle = CommentStyle {
    line: &[("%", true)],
    block: &[],
};
const HTML: CommentStyle = CommentStyle {
    line: &[],
    block: &[("<!--", "-->")],
};
const BATCH: CommentStyle = CommentStyle {
    line: &[("REM ", false), ("rem ", false), ("::", false)],
    block: &[],
};

fn comment_style(lang: &str) -> Option<&'static CommentStyle> {
    let style = match lang {
        "rust" | "rs" | "c" | "cpp" | "c++" | "cc" | "cxx" | "h" | "hpp" | "hh" | "hxx" | "m"
        | "mm" | "java" | "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "go" | "golang" | "cs"
        | "csharp" | "swift" | "kt" | "kotlin" | "php" | "scala" | "dart" | "groovy" | "less"
        | "scss" | "styl" | "sol" | "solidity" | "zig" | "v" | "glsl" | "hlsl" | "wgsl"
        | "json5" | "proto" | "thrift" | "vala" | "jsonc" => &SLASH,
        "css" => &CSS_ONLY,
        "py" | "python" | "python3" | "pyi" => &PYTHON,
        "sh" | "bash" | "zsh" | "fish" | "ksh" | "ps1" | "powershell" | "yaml" | "yml"
        | "toml" | "ini" | "cfg" | "conf" | "dockerfile" | "makefile" | "make" | "r"
        | "perl" | "pl" | "tcl" | "cmake" | "properties" | "env" | "nginx" | "graphql"
        | "gql" | "nix" | "crystal" | "coffee" | "rake" | "gemfile" => &HASH,
        "sql" => &DASHDASH_BLOCK,
        "lua" => &LUA,
        "hs" | "haskell" | "elm" | "ada" | "purescript" | "applescript" => &DASHDASH,
        "lisp" | "clj" | "clojure" | "elisp" | "scheme" | "asm" | "nasm" | "s" | "racket" => {
            &SEMI
        }
        "tex" | "latex" | "matlab" | "erlang" | "postscript" | "ps" | "prolog" => &PERCENT,
        "html" | "htm" | "xml" | "xhtml" | "svg" | "vue" | "svelte" | "markdown" | "md" => &HTML,
        "bat" | "cmd" | "batch" | "dosbatch" => &BATCH,
        _ => return None,
    };
    Some(style)
}

/// One translatable comment inside a code block.
struct CommentSlot {
    block: usize,
    line: usize,
    /// byte range of the comment text inside that line
    head: usize,
    tail: usize,
    /// the original comment ended with a space (keep it before a closing marker)
    pad: bool,
    text: String,
}

/// Find every comment in a code block, skipping anything inside string literals.
fn collect_comments(block_index: usize, code: &str, style: &CommentStyle) -> Vec<CommentSlot> {
    let mut slots = Vec::new();
    let mut pending_close: Option<&'static str> = None;

    for (line_index, line) in code.split('\n').enumerate() {
        if let Some(close) = pending_close {
            let text_start = line.len() - line.trim_start().len();
            match line.find(close) {
                Some(close_at) => {
                    pending_close = None;
                    push_slot(
                        &mut slots,
                        block_index,
                        line_index,
                        line,
                        text_start.max(0),
                        close_at,
                    );
                }
                None => push_slot(&mut slots, block_index, line_index, line, text_start, line.len()),
            }
            continue;
        }

        let Some((marker_at, text_start, closes)) = scan_for_comment(line, style) else {
            continue;
        };
        match closes {
            Some(close) => {
                // block comment opened on this line
                match line[text_start..].find(close) {
                    Some(offset) => {
                        let close_at = text_start + offset;
                        push_slot(
                            &mut slots,
                            block_index,
                            line_index,
                            line,
                            text_start,
                            close_at,
                        );
                    }
                    None => {
                        pending_close = Some(close);
                        push_slot(
                            &mut slots,
                            block_index,
                            line_index,
                            line,
                            text_start,
                            line.len(),
                        );
                    }
                }
            }
            None => {
                let _ = marker_at;
                push_slot(
                    &mut slots,
                    block_index,
                    line_index,
                    line,
                    text_start,
                    line.len(),
                );
            }
        }
    }
    slots
}

fn push_slot(
    slots: &mut Vec<CommentSlot>,
    block: usize,
    line: usize,
    source: &str,
    head: usize,
    tail: usize,
) {
    if head >= tail || tail > source.len() {
        return;
    }
    let text = &source[head..tail];
    let trimmed = text.trim();
    let pad = text.len() > trimmed.len() && text.ends_with(char::is_whitespace);
    // Skip decorative rules ("-----", "===") and empty comments: nothing to translate.
    if trimmed.chars().count() < 2
        || !trimmed
            .chars()
            .any(|ch| ch.is_alphanumeric() || is_cjk(ch))
    {
        return;
    }
    slots.push(CommentSlot {
        block,
        line,
        head,
        tail,
        pad,
        text: trimmed.to_string(),
    });
}

/// The first comment marker outside a string literal, with the byte offset where
/// its text starts and the closing marker when it opens a block comment.
fn scan_for_comment(line: &str, style: &CommentStyle) -> Option<(usize, usize, Option<&'static str>)> {
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let mut quote: Option<char> = None;
    let mut index = 0usize;

    while index < chars.len() {
        let (byte_at, ch) = chars[index];
        if let Some(open_quote) = quote {
            if ch == '\\' {
                index += 2;
                continue;
            }
            if ch == open_quote {
                quote = None;
            }
            index += 1;
            continue;
        }

        let rest = &line[byte_at..];
        for (open, close) in style.block {
            if rest.starts_with(open) {
                let text_start = byte_at + open.len();
                return Some((byte_at, skip_spaces(line, text_start), Some(*close)));
            }
        }
        for (marker, needs_space) in style.line {
            if rest.starts_with(marker) {
                if *needs_space && byte_at > 0 && !line.as_bytes()[byte_at - 1].is_ascii_whitespace()
                {
                    continue;
                }
                // "https://..." is not a comment
                if *marker == "//" && byte_at > 0 && line.as_bytes()[byte_at - 1] == b':' {
                    continue;
                }
                let text_start = byte_at + marker.len();
                return Some((byte_at, skip_spaces(line, text_start), None));
            }
        }

        if ch == '"' || ch == '\'' || ch == '`' {
            quote = Some(ch);
        }
        index += 1;
    }
    None
}

fn skip_spaces(line: &str, from: usize) -> usize {
    let mut at = from;
    for (offset, ch) in line[from..].char_indices() {
        if !ch.is_whitespace() {
            return from + offset;
        }
        at = from + offset + ch.len_utf8();
    }
    at
}

/// Put translated comments back into one code block. `translations` runs parallel
/// to `slots`; `None` means "keep the original text".
fn rebuild_block(
    code: &str,
    block_index: usize,
    slots: &[CommentSlot],
    translations: &[Option<String>],
) -> String {
    if !slots.iter().any(|slot| slot.block == block_index) {
        return code.to_string();
    }
    let mut lines: Vec<String> = code.split('\n').map(str::to_string).collect();
    for (position, slot) in slots.iter().enumerate() {
        if slot.block != block_index {
            continue;
        }
        let Some(Some(text)) = translations.get(position) else {
            continue;
        };
        if let Some(line) = lines.get_mut(slot.line) {
            if slot.tail <= line.len() && slot.head <= slot.tail {
                let gap = if slot.pad && !text.ends_with(' ') { " " } else { "" };
                *line = format!(
                    "{}{}{}{}",
                    &line[..slot.head],
                    text,
                    gap,
                    &line[slot.tail..]
                );
            }
        }
    }
    lines.join("\n")
}

/// Translate the collected comment texts in as few requests as possible.
/// Comments are single lines, so the service's line structure maps back 1:1.
fn translate_comments(
    slots: &[CommentSlot],
    target: &str,
    progress: &mut dyn FnMut(usize, usize),
    done: &mut usize,
    total: usize,
) -> (Vec<Option<String>>, usize) {
    let mut out: Vec<Option<String>> = vec![None; slots.len()];
    if slots.is_empty() {
        return (out, 0);
    }

    // Pack comment texts into requests, remembering which slots each one carries.
    let mut batches: Vec<Vec<usize>> = Vec::new();
    let mut current: Vec<usize> = Vec::new();
    let mut cost = 0usize;
    for (index, slot) in slots.iter().enumerate() {
        let size = encoded_len(&slot.text) + 1;
        if !current.is_empty() && cost + size > MAX_QUERY_CHARS {
            batches.push(std::mem::take(&mut current));
            cost = 0;
        }
        current.push(index);
        cost += size;
    }
    if !current.is_empty() {
        batches.push(current);
    }

    let mut translated_count = 0usize;
    for batch in &batches {
        progress(*done, total);
        let joined = batch
            .iter()
            .map(|index| slots[*index].text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        if let Ok(answer) = request(&joined, target) {
            let lines: Vec<&str> = answer.split('\n').collect();
            if lines.len() == batch.len() {
                for (position, slot_index) in batch.iter().enumerate() {
                    let text = lines[position].trim();
                    if !text.is_empty() {
                        out[*slot_index] = Some(text.to_string());
                        translated_count += 1;
                    }
                }
            }
        }
        *done += 1;
    }
    (out, translated_count)
}

pub fn translate_document(
    source: &str,
    target: &str,
    base: &Path,
    options: Options,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<Translated, String> {
    let target = normalise_target(target, source);
    let blocks = split_blocks(source);

    // Pass 1: prospection for comments inside code blocks, so they can be
    // translated alongside the prose instead of in a second round trip each.
    let mut slots: Vec<CommentSlot> = Vec::new();
    if options.comments {
        for (index, block) in blocks.iter().enumerate() {
            if block.translate {
                continue;
            }
            let Some(lang) = block.lang.as_deref() else {
                continue;
            };
            let Some(style) = comment_style(lang) else {
                continue;
            };
            slots.extend(collect_comments(index, &block.text, style));
        }
    }

    let prose_total = blocks
        .iter()
        .filter(|block| block.translate && !block.text.trim().is_empty())
        .count();
    let batch_count = if slots.is_empty() {
        0
    } else {
        let mut batches = 1usize;
        let mut cost = 0usize;
        for slot in &slots {
            let size = encoded_len(&slot.text) + 1;
            if cost + size > MAX_QUERY_CHARS {
                batches += 1;
                cost = 0;
            }
            cost += size;
        }
        batches
    };
    let total = prose_total + batch_count;

    let mut done = 0usize;
    // Comments first: they are short, so one or two requests cover the whole file.
    let (comment_translations, comment_count) =
        translate_comments(&slots, &target, progress, &mut done, total);

    let mut translated = String::with_capacity(source.len());
    for (index, block) in blocks.iter().enumerate() {
        if block.translate {
            if block.text.trim().is_empty() {
                translated.push_str(&block.text);
                continue;
            }
            progress(done, total);
            translated.push_str(&translate_block(&block.text, &target)?);
            done += 1;
            continue;
        }
        translated.push_str(&rebuild_block(
            &block.text,
            index,
            &slots,
            &comment_translations,
        ));
    }
    progress(total, total);

    let rendered = markdown::render(&translated, base);
    Ok(Translated {
        markdown: translated,
        html: rendered.html,
        target,
        blocks: prose_total,
        comments: comment_count,
    })
}

fn translate_block(text: &str, target: &str) -> Result<String, String> {
    let trailing = text.len() - text.trim_end_matches('\n').len();
    let mut out = String::with_capacity(text.len());
    for (index, chunk) in split_for_url(text).iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&request(chunk, target)?);
    }
    while out.ends_with('\n') {
        out.pop();
    }
    out.push_str(&"\n".repeat(trailing));
    Ok(out)
}

fn request(text: &str, target: &str) -> Result<String, String> {
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?client=gtx&sl=auto&tl={target}&dt=t&q={}",
        encode(text)
    );
    let body = win::http_get(&url, TIMEOUT_MS, "Accept: application/json\r\n")?;
    match parse_google(&body, text) {
        Ok(translated) => Ok(translated),
        Err(google_error) => match request_mymemory(text, target) {
            Ok(translated) => Ok(translated),
            Err(_) => Err(google_error),
        },
    }
}

fn parse_google(body: &str, original: &str) -> Result<String, String> {
    let value: Value =
        serde_json::from_str(body).map_err(|_| "翻译服务返回了无法解析的内容".to_string())?;
    let segments = value
        .get(0)
        .and_then(Value::as_array)
        .ok_or_else(|| "翻译服务没有返回结果".to_string())?;
    let mut out = String::new();
    for segment in segments {
        if let Some(piece) = segment.get(0).and_then(Value::as_str) {
            out.push_str(piece);
        }
    }
    let out = out.trim_end_matches('\n').to_string();
    if out.trim().is_empty() {
        return Err("翻译服务返回了空结果".into());
    }
    Ok(realign(&out, original))
}

/// When the service merges lines, fold the result onto one logical line so no
/// list marker is left hanging on its own.
fn realign(translated: &str, original: &str) -> String {
    let wanted = original.lines().count();
    let got = translated.lines().count();
    if wanted <= 1 || wanted == got {
        return translated.to_string();
    }
    translated.replace('\n', " ")
}

fn request_mymemory(text: &str, target: &str) -> Result<String, String> {
    let pair = if target.starts_with("zh") {
        "auto|zh-CN"
    } else {
        "auto|en"
    };
    let url = format!(
        "https://api.mymemory.translated.net/get?q={}&langpair={pair}",
        encode(text)
    );
    let body = win::http_get(&url, TIMEOUT_MS, "Accept: application/json\r\n")?;
    let value: Value =
        serde_json::from_str(&body).map_err(|_| "备用翻译服务返回异常".to_string())?;
    let out = value
        .get("responseData")
        .and_then(|data| data.get("translatedText"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim()
        .to_string();
    if out.is_empty() {
        return Err("备用翻译服务没有返回结果".into());
    }
    Ok(out)
}

/* --------------------------------------------------------------- url budget */

fn split_for_url(text: &str) -> Vec<String> {
    if encoded_len(text) <= MAX_QUERY_CHARS {
        return vec![text.to_string()];
    }
    let mut out: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_cost = 0usize;

    for line in text.split_inclusive('\n') {
        let cost = encoded_len(line);
        if cost > MAX_QUERY_CHARS {
            if !current.is_empty() {
                out.push(std::mem::take(&mut current));
                current_cost = 0;
            }
            out.extend(split_sentences(line));
            continue;
        }
        if current_cost + cost > MAX_QUERY_CHARS && !current.is_empty() {
            out.push(std::mem::take(&mut current));
            current_cost = 0;
        }
        current.push_str(line);
        current_cost += cost;
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn split_sentences(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in line.chars() {
        current.push(ch);
        let boundary = matches!(ch, '。' | '！' | '？' | '.' | '!' | '?' | '；' | ';');
        if boundary && encoded_len(&current) >= MAX_QUERY_CHARS / 2 {
            out.push(std::mem::take(&mut current));
        } else if encoded_len(&current) >= MAX_QUERY_CHARS {
            out.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn encoded_len(text: &str) -> usize {
    text.chars()
        .map(|ch| if unreserved(ch) { 1 } else { ch.len_utf8() * 3 })
        .sum()
}

fn unreserved(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~')
}

pub fn encode(text: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(text.len() * 2);
    for byte in text.as_bytes() {
        let ch = *byte as char;
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '~') {
            out.push(ch);
        } else {
            out.push('%');
            out.push(HEX[(byte >> 4) as usize] as char);
            out.push(HEX[(byte & 0x0F) as usize] as char);
        }
    }
    out
}
