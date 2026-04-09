# 🔍 Rust Seek

> **A blazing-fast file & text search tool built with Rust — searches millions of files in milliseconds, with a clean native GUI.**

![Rust](https://img.shields.io/badge/built%20with-Rust-orange?logo=rust)
![Platform](https://img.shields.io/badge/platform-Windows-blue?logo=windows)
![License](https://img.shields.io/badge/license-MIT-green)
![Version](https://img.shields.io/badge/version-0.1.0-brightgreen)

---

## ✨ Why Rust Seek?

Most search tools are either too slow, too complex, or require a runtime. **Rust Seek** is a single `.exe` — no installation, no dependencies, no configuration. Just open it and search.

- ⚡ **Parallel search engine** — uses every CPU core via the same engine powering `ripgrep`
- 🗺️ **Memory-mapped I/O** — reads large files without loading them fully into RAM
- 🎯 **Full regex support** — powered by Rust's battle-tested `regex` crate
- 🖥️ **Native GUI** — built with `egui`, renders at 60fps with zero web bloat
- 🌏 **Chinese file support** — reads both UTF-8 and GBK/GB2312 encoded files
- 📦 **Zero dependencies for the user** — one `.exe`, runs anywhere on Windows 10+

---

## 🚀 Features

### Two Search Modes

| Mode | What it does |
|------|-------------|
| 🗂 **File Name** | Search files and folders by name across any directory, including system folders and `Program Files`. Finds `.exe`, `.lnk`, documents — anything. |
| 📄 **Text** | Search inside file contents with full regex support and context lines. Reads UTF-8 and GBK encoded files. |

Switching modes instantly clears results — no mixing of file and text results.

### Search
- **Regex by default** — use any valid regular expression as your pattern
- **Fixed string mode** (`F`) — disable regex for exact literal matching
- **Case-insensitive** (`Aa`) — on by default, toggle anytime
- **Live regex validation** — syntax errors shown instantly as you type, before you search
- **Binary file detection** — automatically skips binary files
- **Chinese encoding support** — searches GBK / GB2312 files, not just UTF-8
- **Context lines** — text mode shows the line before and after each match
- **Up to 2,000 results** — capped to keep the UI fast; warns when truncated

### Results
- **Real-time streaming** — results appear as they're found
- **Live progress** — status bar shows match count and file count while searching
- **All matches highlighted** — every occurrence on a line highlighted in yellow
- **Line numbers** — green line numbers for match lines, grey for context
- **Sorted output** — always sorted by file path
- **File size** — shown next to each result in file name mode
- **Collapse / expand** — fold individual files or all at once with one click
- **Show more** — files with many matches show the first 5, expandable on demand
- **Post-search filter** — type in the filter box to narrow results without re-searching
- **Copy all paths** — one click to copy every result path to clipboard

### Interface
- **Auto-focus** — search box is ready to type the moment you open the app
- **Drag & drop** — drag a folder onto the window to set the search path instantly
- **Folder picker** — click 📁 or drag a folder to set the path
- **Search history** — path and pattern history saved across restarts, click to reuse
- **Press Enter to search** — no need to reach for the mouse
- **Cancel anytime** — click ⏹ or press `Esc` to stop a running search immediately
- **Right-click menu** — on any result: open, reveal in Explorer, copy path, copy folder path
- **Reveal in Explorer** — opens Windows Explorer with the file selected and highlighted
- **Status bar** — total matches, files searched, and elapsed time

---

## 📸 Screenshot

```
┌──────────────────────────────────────────────────────────────────────┐
│ 路径: [C:/my-project  ] 📁  🗂文件名  📄文本  搜索: [fn main] 🔍搜索  Aa  F │
├──────────────────────────────────────────────────────────────────────┤
│ ▼ 全部展开  ▶ 全部折叠  📋 复制全部路径  过滤: [        ] ✕           │
├──────────────────────────────────────────────────────────────────────┤
│ ▼ 📝 src/app.rs  (3 处匹配)                                           │
│    41   fn start_search(&mut self) {                                  │
│    42:  fn start_search(&mut self) → starts parallel walk            │
│    43   let re = RegexBuilder::new(&pat)...                          │
│                                                                      │
│ ▼ 📝 src/main.rs  (1 处匹配)                                          │
│     0   mod app;                                                     │
│     1:  fn main() -> eframe::Result {                                │
│     2   let options = eframe::NativeOptions {                        │
├──────────────────────────────────────────────────────────────────────┤
│ ⏱ 4 处匹配，共 2 个文件 (8ms)                                         │
└──────────────────────────────────────────────────────────────────────┘
```

---

## 📥 Download & Run

1. Go to [Releases](../../releases)
2. Download `rust-seek.exe`
3. Double-click — that's it

> ✅ No .NET, no Java, no Python, no Visual C++ Redistributable required.  
> Works on Windows 10 and above.

---

## 🛠️ Build from Source

Requires [Rust](https://rustup.rs/) (stable toolchain).

```bash
git clone https://github.com/1716775457damn/rust-seek.git
cd rust-seek
cargo build --release
# Binary: target/release/rust-seek.exe
```

The release build uses full LTO and `opt-level = 3`, producing a small, fast single binary.

---

## 🏗️ Architecture

```
src/
├── main.rs       # Entry point, window setup, CJK font loading
├── app.rs        # GUI (egui): toolbar, results, history, drag-drop, context menus
└── searcher.rs   # Core engine: mmap I/O, regex, binary detection, GBK decoding
```

| Component | Crate | Why |
|-----------|-------|-----|
| GUI framework | `egui` / `eframe` | Immediate-mode, native, no Electron |
| Directory traversal | `ignore` | Same crate as ripgrep — parallel + gitignore-aware |
| File reading | `memmap2` | OS-level memory mapping, zero-copy on large files |
| Pattern matching | `regex` | DFA-based, Unicode-aware, very fast |
| Encoding detection | `encoding_rs` | UTF-8 + GBK/GB2312 fallback |
| Folder dialog | `rfd` | Native Windows file picker |
| Persistence | `serde_json` | Search history and preferences |

---

## ⚡ Performance

Rust Seek uses the same parallel directory walking strategy as `ripgrep`. On a modern machine:

- Searches a **100,000-file codebase** in under **2 seconds**
- Handles files up to **10 MB** with memory-mapped I/O — no full load into RAM
- Scales linearly with CPU cores
- Context strings shared via `Arc` — zero extra heap allocation per match
- Regex validation cached — no recompilation on every keystroke

---

## 🗺️ Roadmap

- [ ] Replace mode (find & replace across files)
- [ ] File type filter (search only `.rs`, `.py`, etc.)
- [ ] Dark / light theme toggle
- [ ] Export results to file

---

## 📄 License

MIT © 2025
