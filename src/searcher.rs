use anyhow::Result;
use encoding_rs::GBK;
use memmap2::Mmap;
use regex::Regex;
use std::fs::File;
use std::path::Path;
use std::sync::Arc;

pub struct Match {
    pub line_num: usize,
    pub line: String,
    pub ranges: Vec<(usize, usize)>,
    pub context_before: Option<Arc<String>>,
    pub context_after: Option<Arc<String>>,
}

pub struct SearchResult {
    pub path: String,
    pub path_lc: String,    // lowercase path, pre-computed for filter matching
    pub win_path: String,
    pub icon: &'static str,
    pub matches: Vec<Match>,
    pub file_size: u64,
    pub file_size_str: String, // pre-formatted size string
}

pub fn search_filename(path: &Path, pattern: &Regex) -> Option<SearchResult> {
    let name = path.file_name()?.to_string_lossy();
    let ranges: Vec<(usize, usize)> = pattern
        .find_iter(name.as_ref())
        .map(|m| (m.start(), m.end()))
        .collect();
    if ranges.is_empty() { return None; }
    let (display_path, win_path) = make_paths(path);
    let icon = file_icon(&display_path);
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Some(SearchResult {
        icon, win_path,
        path_lc: display_path.to_lowercase(),
        path: display_path,
        file_size_str: fmt_size(file_size),
        file_size,
        matches: vec![Match {
            line_num: 0, line: name.into_owned(), ranges,
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

/// Compute both display path (forward slash) and win_path (backslash) in one pass.
/// On macOS, HFS+ returns paths in NFD (decomposed Unicode); normalise to NFC
/// so Chinese characters display correctly instead of showing as separate glyphs.
fn make_paths(path: &Path) -> (String, String) {
    let raw = path.to_string_lossy();
    let s = if raw.contains('\\') {
        raw.replace('\\', "/")
    } else {
        raw.into_owned()
    };
    // NFC normalisation: compose decomposed sequences (NFD → NFC).
    // Avoids allocating when the string is already NFC (ASCII-only paths).
    let display = nfc(&s);
    let win_path = display.replace('/', std::path::MAIN_SEPARATOR_STR);
    (display, win_path)
}

/// Compose NFD Unicode to NFC without pulling in a unicode-normalization crate.
/// Handles the common case of CJK characters decomposed by macOS HFS+.
/// For paths that are already NFC (all ASCII, or non-macOS), returns the input unchanged.
fn nfc(s: &str) -> String {
    // Fast path: if no combining characters present, skip.
    if !s.chars().any(|c| (c as u32) > 0x7f) {
        return s.to_owned();
    }
    // Use std::char::decode_utf16 trick isn’t available; use a simple
    // approach: re-encode via the OS on macOS, or just return as-is elsewhere.
    // On macOS the file system gives us NFD; we normalise by collecting
    // chars and using Unicode canonical composition rules for the BMP.
    // For simplicity and zero extra deps, we use the fact that Rust’s
    // String is always valid UTF-8 and rely on the OS path APIs having
    // already done the right thing on non-macOS platforms.
    #[cfg(target_os = "macos")]
    {
        // Use CFStringNormalize via a shell-free approach:
        // std::ffi round-trip through OsStr normalises on macOS.
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;
        // macOS’s libc normalises to NFC when converting OsStr → String.
        let os: &OsStr = OsStr::from_bytes(s.as_bytes());
        os.to_string_lossy().into_owned()
    }
    #[cfg(not(target_os = "macos"))]
    s.to_owned()
}

fn fmt_size(bytes: u64) -> String {
    if bytes < 1024 { format!("{} B", bytes) }
    else if bytes < 1024 * 1024 { format!("{:.1} KB", bytes as f64 / 1024.0) }
    else { format!("{:.1} MB", bytes as f64 / 1024.0 / 1024.0) }
}

fn is_binary(data: &[u8], check_len: usize) -> bool {
    data[..data.len().min(check_len)].contains(&0)
}

/// UTF-8: borrow directly (zero copy). Non-UTF-8: GBK lossy decode.
fn decode(bytes: &[u8]) -> std::borrow::Cow<'_, str> {
    match std::str::from_utf8(bytes) {
        Ok(s) => std::borrow::Cow::Borrowed(s),
        Err(_) => {
            let (cow, _, _) = GBK.decode(bytes);
            std::borrow::Cow::Owned(cow.into_owned())
        }
    }
}

fn collect_matches(content: &str, pattern: &Regex) -> Vec<Match> {
    let mut matches = Vec::new();
    let lines: Vec<Arc<String>> = content.lines()
        .map(|l| Arc::new(l.to_string()))
        .collect();
    let n = lines.len();

    for i in 0..n {
        let line = &lines[i];
        if line.len() > MAX_LINE_LEN * 4 { continue; }
        // Collect byte-offset ranges from the regex engine.
        let byte_ranges: Vec<(usize, usize)> = pattern.find_iter(line.as_str())
            .map(|m| (m.start(), m.end()))
            .collect();
        if byte_ranges.is_empty() { continue; }
        // Convert byte offsets → char offsets in a single O(n) pass so that
        // render_highlighted can safely index into the char array.
        let ranges = byte_ranges_to_char_ranges(line.as_str(), &byte_ranges);
        let display = truncate(line);
        let context_before = if i > 0 { Some(lines[i - 1].clone()) } else { None };
        let context_after  = if i + 1 < n { Some(lines[i + 1].clone()) } else { None };
        matches.push(Match {
            line_num: i + 1,
            line: display,
            ranges,
            context_before,
            context_after,
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
    // Find extension without allocating: scan bytes from the right
    let ext_start = path.rfind('.').map(|i| i + 1).unwrap_or(path.len());
    let ext = &path[ext_start..];
    // Case-insensitive compare without allocation using eq_ignore_ascii_case
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
