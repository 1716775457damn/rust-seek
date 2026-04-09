use anyhow::Result;
use memmap2::Mmap;
use regex::Regex;
use std::fs::File;
use std::path::Path;

pub struct Match {
    pub line_num: usize,
    pub line: String,
    pub ranges: Vec<(usize, usize)>,
}

pub struct SearchResult {
    pub path: String,
    pub matches: Vec<Match>,
}

pub fn search_file(path: &Path, pattern: &Regex, max_filesize: u64) -> Result<Option<SearchResult>> {
    let metadata = std::fs::metadata(path)?;
    if metadata.len() == 0 || metadata.len() > max_filesize {
        return Ok(None);
    }

    let content: String = if metadata.len() < 4096 {
        let bytes = std::fs::read(path)?;
        if bytes[..bytes.len().min(8192)].contains(&0) { return Ok(None); }
        match String::from_utf8(bytes) { Ok(s) => s, Err(_) => return Ok(None) }
    } else {
        let file = File::open(path)?;
        let mmap = unsafe { Mmap::map(&file)? };
        if mmap[..mmap.len().min(8192)].contains(&0) { return Ok(None); }
        match std::str::from_utf8(&mmap) { Ok(s) => s.to_owned(), Err(_) => return Ok(None) }
    };

    let mut matches = Vec::new();
    for (i, line) in content.lines().enumerate() {
        let ranges: Vec<(usize, usize)> = pattern.find_iter(line).map(|m| (m.start(), m.end())).collect();
        if !ranges.is_empty() {
            matches.push(Match { line_num: i + 1, line: line.to_string(), ranges });
        }
    }

    if matches.is_empty() { return Ok(None); }
    Ok(Some(SearchResult { path: path.to_string_lossy().replace('\\', "/"), matches }))
}
