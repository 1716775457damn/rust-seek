use anyhow::Result;
use encoding_rs::{BIG5, GBK, UTF_16BE, UTF_16LE};
use memmap2::Mmap;
use regex::Regex;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;
use unicode_normalization::UnicodeNormalization;

pub struct Match {
    pub line_num: usize,
    pub line: String,
    pub ranges: Vec<(usize, usize)>,
    pub context_before: Option<Arc<String>>,
    pub context_after: Option<Arc<String>>,
}

pub struct SearchResult {
    pub path: String,
    pub path_lc: String,
    pub win_path: String,
    pub icon: &'static str,
    pub matches: Vec<Match>,
    pub file_size: u64,
    pub file_size_str: String,
}

pub fn search_filename(path: &Path, pattern: &Regex) -> Option<SearchResult> {
    let name = path.file_name()?.to_string_lossy();
    let name_nfc = nfc(name.as_ref());
    let ranges: Vec<(usize, usize)> = pattern
        .find_iter(&name_nfc)
        .map(|m| (m.start(), m.end()))
        .collect();
    if ranges.is_empty() { return None; }
    let (display_path, win_path) = make_paths(path);
    let icon = file_icon(&display_path);
    // Reuse already-fetched metadata instead of a second stat() call
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Some(SearchResult {
        icon, win_path,
        path_lc: display_path.to_lowercase(),
        path: display_path,
        file_size_str: fmt_size(file_size),
        file_size,
        matches: vec![Match {
            line_num: 0, line: name_nfc, ranges,
            context_before: None, context_after: None,
        }],
    })
}

/// Variant that accepts pre-fetched file size to avoid a redundant stat() call
/// when the caller already has metadata (e.g. from WalkDir).
pub fn search_filename_with_size(path: &Path, pattern: &Regex, file_size: u64) -> Option<SearchResult> {
    let name = path.file_name()?.to_string_lossy();
    let name_nfc = nfc(name.as_ref());
    let ranges: Vec<(usize, usize)> = pattern
        .find_iter(&name_nfc)
        .map(|m| (m.start(), m.end()))
        .collect();
    if ranges.is_empty() { return None; }
    let (display_path, win_path) = make_paths(path);
    let icon = file_icon(&display_path);
    Some(SearchResult {
        icon, win_path,
        path_lc: display_path.to_lowercase(),
        path: display_path,
        file_size_str: fmt_size(file_size),
        file_size,
        matches: vec![Match {
            line_num: 0, line: name_nfc, ranges,
            context_before: None, context_after: None,
        }],
    })
}

const MAX_LINE_LEN: usize = 512;
const BINARY_CHECK_LEN: usize = 1024;

pub fn search_file(path: &Path, pattern: &Regex, max_filesize: u64) -> Result<Option<SearchResult>> {
    let metadata = std::fs::metadata(path)?;
    let file_len = metadata.len();
    if file_len == 0 || file_len > max_filesize { return Ok(None); }

    let matches = if file_len < 4096 {
        let bytes = std::fs::read(path)?;
        if is_binary(&bytes, bytes.len().min(BINARY_CHECK_LEN)) { return Ok(None); }
        let s = decode(&bytes);
        collect_matches(&s, pattern)
    } else {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        if is_binary(&mmap, BINARY_CHECK_LEN) { return Ok(None); }
        let s = decode(&mmap);
        collect_matches(&s, pattern)
    };

    if matches.is_empty() { return Ok(None); }
    let (display_path, win_path) = make_paths(path);
    let icon = file_icon(&display_path);
    Ok(Some(SearchResult {
        path_lc: display_path.to_lowercase(),
        path: display_path,
        win_path, icon, matches,
        file_size: file_len,
        file_size_str: fmt_size(file_len),
    }))
}

/// Compute display path (forward slash) and win_path (OS separator).
/// Applies NFC normalisation so macOS HFS+ NFD paths display correctly.
fn make_paths(path: &Path) -> (String, String) {
    let raw = path.to_string_lossy();
    let s = if raw.contains('\\') {
        raw.replace('\\', "/")
    } else {
        raw.into_owned()
    };
    let display = nfc(&s);
    let win_path = display.replace('/', std::path::MAIN_SEPARATOR_STR);
    (display, win_path)
}

/// NFD → NFC using the unicode-normalization crate.
///
/// macOS HFS+ stores filenames in NFD: each CJK character is decomposed into
/// a base codepoint + combining codepoints. Without NFC composition the
/// characters render as separate glyphs or question marks in egui.
/// unicode_normalization::nfc() handles all scripts correctly, not just CJK.
fn nfc(s: &str) -> String {
    if s.is_ascii() { return s.to_owned(); }
    s.nfc().collect()
}

fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{} B", bytes) }
    else if bytes < 1024 * 1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else { format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0) }
}

/// Detect binary files. UTF-16 files start with a BOM and contain many null
/// bytes — they are text, not binary, so we exempt them from the null-byte check.
fn is_binary(data: &[u8], check_len: usize) -> bool {
    let sample = &data[..data.len().min(check_len)];
    // UTF-16 LE/BE BOM — definitely text
    if sample.len() >= 2
        && ((sample[0] == 0xFF && sample[1] == 0xFE)
         || (sample[0] == 0xFE && sample[1] == 0xFF))
    {
        return false;
    }
    sample.contains(&0)
}

/// Detect encoding using only the first 4 KB, then decode the full content once.
/// Previously decoded the entire file twice (Big5 + GBK) for non-UTF-8 files.
fn decode(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    // UTF-16 LE BOM: FF FE
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let (cow, _, _) = UTF_16LE.decode(bytes);
        return std::borrow::Cow::Owned(cow.into_owned());
    }
    // UTF-16 BE BOM: FE FF
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let (cow, _, _) = UTF_16BE.decode(bytes);
        return std::borrow::Cow::Owned(cow.into_owned());
    }
    // UTF-8 BOM: EF BB BF
    if bytes.len() >= 3 && bytes[0] == 0xEF && bytes[1] == 0xBB && bytes[2] == 0xBF {
        return match std::str::from_utf8(&bytes[3..]) {
            Ok(s)  => std::borrow::Cow::Borrowed(s),
            Err(_) => std::borrow::Cow::Owned(String::from_utf8_lossy(&bytes[3..]).into_owned()),
        };
    }
    // Pure UTF-8 — zero copy
    if let Ok(s) = std::str::from_utf8(bytes) {
        return std::borrow::Cow::Borrowed(s);
    }
    // Non-UTF-8: probe only the first 4 KB to decide encoding,
    // then decode the full file once with the winner.
    const PROBE_LEN: usize = 4096;
    let probe = &bytes[..bytes.len().min(PROBE_LEN)];
    let (_, _, big5_err) = BIG5.decode(probe);
    let (_, _, gbk_err)  = GBK.decode(probe);

    let use_big5 = match (big5_err, gbk_err) {
        (false, true)  => true,
        (true,  false) => false,
        _ => {
            // Both clean or both errored on probe — count replacements in probe
            let (b5, _, _) = BIG5.decode(probe);
            let (gk, _, _) = GBK.decode(probe);
            let b5_bad = b5.chars().filter(|&c| c == '\u{FFFD}').count();
            let gk_bad = gk.chars().filter(|&c| c == '\u{FFFD}').count();
            #[cfg(target_os = "macos")]     { b5_bad <= gk_bad }
            #[cfg(not(target_os = "macos"))]{ b5_bad <  gk_bad }
        }
    };

    // Decode full file once with the chosen encoding
    let (cow, _, _) = if use_big5 { BIG5.decode(bytes) } else { GBK.decode(bytes) };
    std::borrow::Cow::Owned(cow.into_owned())
}

fn collect_matches(content: &str, pattern: &Regex) -> Vec<Match> {
    let mut matches = Vec::new();
    // Use lines() iterator — no pre-allocation of all line offsets.
    // Keep a sliding window of (prev_line, cur_line) to provide context
    // without storing the entire file's lines in memory.
    let mut prev_line: Option<String> = None;
    let mut pending: Option<(usize, String, Vec<(usize, usize)>)> = None;

    for (i, raw) in content.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);

        // Flush pending match now that we have its context_after
        if let Some((line_num, line_str, ranges)) = pending.take() {
            let context_after = if !line.is_empty() {
                Some(Arc::new(line.to_string()))
            } else { None };
            matches.push(Match {
                line_num,
                line: line_str,
                ranges,
                context_before: prev_line.as_ref().map(|s| Arc::new(s.clone())),
                context_after,
            });
        }

        if line.len() > MAX_LINE_LEN * 4 {
            prev_line = Some(line.to_string());
            continue;
        }

        let byte_ranges: Vec<(usize, usize)> = pattern.find_iter(line)
            .map(|m| (m.start(), m.end()))
            .collect();

        if !byte_ranges.is_empty() {
            let ranges = byte_ranges_to_char_ranges(line, &byte_ranges);
            let display = truncate(line);
            // Defer push until next iteration so we can fill context_after
            pending = Some((i + 1, display, ranges));
        }

        prev_line = Some(line.to_string());
    }

    // Flush last pending match (no context_after)
    if let Some((line_num, line_str, ranges)) = pending {
        matches.push(Match {
            line_num,
            line: line_str,
            ranges,
            context_before: prev_line.map(|s| Arc::new(s)),
            context_after: None,
        });
    }

    matches
}

/// Convert byte-offset pairs to char-offset pairs in one O(n) pass.
fn byte_ranges_to_char_ranges(s: &str, byte_ranges: &[(usize, usize)]) -> Vec<(usize, usize)> {
    if byte_ranges.is_empty() { return vec![]; }
    let mut result = Vec::with_capacity(byte_ranges.len());
    let mut ri = 0;
    let mut char_idx = 0usize;
    for (byte_idx, _ch) in s.char_indices() {
        while ri < byte_ranges.len() && byte_ranges[ri].0 == byte_idx {
            let (bs, be) = byte_ranges[ri];
            let char_len = s[bs..be].chars().count();
            result.push((char_idx, char_idx + char_len));
            ri += 1;
        }
        if ri >= byte_ranges.len() { break; }
        char_idx += 1;
    }
    result
}

fn truncate(line: &str) -> String {
    if line.len() > MAX_LINE_LEN {
        let end = line.char_indices().nth(MAX_LINE_LEN).map(|(i, _)| i).unwrap_or(line.len());
        line[..end].to_string() + "…"
    } else {
        line.to_string()
    }
}

pub fn file_icon(path: &str) -> &'static str {
    let ext_start = path.rfind('.').map(|i| i + 1).unwrap_or(path.len());
    let ext = &path[ext_start..];
    if ext.eq_ignore_ascii_case("exe") || ext.eq_ignore_ascii_case("msi") { return "⚙"; }
    if ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd")
    || ext.eq_ignore_ascii_case("ps1") || ext.eq_ignore_ascii_case("sh")  { return "⚙"; }
    if ext.eq_ignore_ascii_case("rs")  || ext.eq_ignore_ascii_case("py")
    || ext.eq_ignore_ascii_case("js")  || ext.eq_ignore_ascii_case("ts")
    || ext.eq_ignore_ascii_case("go")  || ext.eq_ignore_ascii_case("c")
    || ext.eq_ignore_ascii_case("cpp") || ext.eq_ignore_ascii_case("java")
    || ext.eq_ignore_ascii_case("cs")  || ext.eq_ignore_ascii_case("rb")  { return "📝"; }
    if ext.eq_ignore_ascii_case("toml")|| ext.eq_ignore_ascii_case("json")
    || ext.eq_ignore_ascii_case("yaml")|| ext.eq_ignore_ascii_case("yml")
    || ext.eq_ignore_ascii_case("xml") || ext.eq_ignore_ascii_case("ini")
    || ext.eq_ignore_ascii_case("cfg") || ext.eq_ignore_ascii_case("env") { return "🔧"; }
    if ext.eq_ignore_ascii_case("md")  || ext.eq_ignore_ascii_case("txt")
    || ext.eq_ignore_ascii_case("log")                                     { return "📄"; }
    if ext.eq_ignore_ascii_case("png") || ext.eq_ignore_ascii_case("jpg")
    || ext.eq_ignore_ascii_case("jpeg")|| ext.eq_ignore_ascii_case("gif")
    || ext.eq_ignore_ascii_case("svg") || ext.eq_ignore_ascii_case("ico")
    || ext.eq_ignore_ascii_case("bmp") || ext.eq_ignore_ascii_case("webp"){ return "🖼"; }
    if ext.eq_ignore_ascii_case("mp4") || ext.eq_ignore_ascii_case("mkv")
    || ext.eq_ignore_ascii_case("avi") || ext.eq_ignore_ascii_case("mov") { return "🎬"; }
    if ext.eq_ignore_ascii_case("mp3") || ext.eq_ignore_ascii_case("wav")
    || ext.eq_ignore_ascii_case("flac")|| ext.eq_ignore_ascii_case("ogg") { return "🎵"; }
    if ext.eq_ignore_ascii_case("zip") || ext.eq_ignore_ascii_case("rar")
    || ext.eq_ignore_ascii_case("7z")  || ext.eq_ignore_ascii_case("tar")
    || ext.eq_ignore_ascii_case("gz")                                      { return "📦"; }
    if ext.eq_ignore_ascii_case("pdf")                                     { return "📕"; }
    if ext.eq_ignore_ascii_case("doc") || ext.eq_ignore_ascii_case("docx"){ return "📘"; }
    if ext.eq_ignore_ascii_case("xls") || ext.eq_ignore_ascii_case("xlsx"){ return "📗"; }
    if ext.eq_ignore_ascii_case("ppt") || ext.eq_ignore_ascii_case("pptx"){ return "📙"; }
    if ext.eq_ignore_ascii_case("lnk")                                     { return "🔗"; }
    if ext.eq_ignore_ascii_case("db")  || ext.eq_ignore_ascii_case("sqlite")
    || ext.eq_ignore_ascii_case("sql")                                     { return "🗄"; }
    "📄"
}
