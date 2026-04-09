# 🔍 Rust Seek

> **A blazing-fast file & text search tool built with Rust — searches millions of lines in milliseconds, with a clean native GUI.**

![Rust](https://img.shields.io/badge/built%20with-Rust-orange?logo=rust)
![Platform](https://img.shields.io/badge/platform-Windows-blue?logo=windows)
![License](https://img.shields.io/badge/license-MIT-green)
![Version](https://img.shields.io/badge/version-0.1.0-brightgreen)

---

## ✨ Why Rust Seek?

Most search tools are either too slow, too complex, or require a runtime environment. **Rust Seek** is a single `.exe` file — no installation, no dependencies, no configuration. Just open it and search.

- ⚡ **Parallel search engine** — uses every CPU core simultaneously via the same engine powering `ripgrep`
- 🗺️ **Memory-mapped I/O** — reads large files without loading them fully into RAM
- 🎯 **Full regex support** — powered by Rust's battle-tested `regex` crate
- 🖥️ **Native GUI** — built with `egui`, renders at 60fps with zero web bloat
- 📦 **Zero dependencies for the user** — one `.exe`, runs anywhere on Windows

---

## 🚀 Features

### Search
- **Regex search** by default — use any valid regular expression as your pattern
- **Literal string mode** (`F` checkbox) — disable regex for exact text matching
- **Case-insensitive mode** (`Aa` checkbox) — match regardless of letter case
- **Binary file detection** — automatically skips binary files, no garbage output
- **Large file protection** — files over 10 MB are skipped to keep searches instant
- **Respects `.gitignore`** — won't waste time searching `node_modules`, `target/`, etc.

### Results
- **Real-time streaming** — results appear as they're found, no waiting for completion
- **All matches highlighted** — every occurrence on a line is highlighted in yellow, not just the first
- **Line numbers** — every match shows its exact line number in green
- **Sorted output** — results are always sorted by file path for consistent, readable output
- **Click to open** — click any file path to open it instantly in your default editor
- **Match count** — each file shows how many matches were found

### Interface
- **Folder picker** — click 📁 to browse for a directory, or type the path directly
- **Press Enter to search** — no need to reach for the mouse
- **Status bar** — shows total matches, files searched, and time elapsed
- **Resizable window** — drag to any size that fits your workflow

---

## 📸 Screenshot

```
┌─────────────────────────────────────────────────────────────┐
│  Path: [C:/my-project          ] 📁  Search: [fn main  ] 🔍 Search  [Aa] [F] │
├─────────────────────────────────────────────────────────────┤
│  src/main.rs  (1 match)                                      │
│     1:  fn main() -> eframe::Result {                        │
│                                                              │
│  src/app.rs  (3 matches)                                     │
│    41:  fn start_search(&mut self) {                         │
│    66:  fn update(&mut self, ctx: &egui::Context, ...) {     │
│   142:  fn render_result(ui: &mut Ui, result: &SearchResult) │
├─────────────────────────────────────────────────────────────┤
│  4 matches in 2 files (12ms)                                 │
└─────────────────────────────────────────────────────────────┘
```

---

## 📥 Download & Run

1. Go to [Releases](../../releases)
2. Download `rust-seek.exe`
3. Double-click to open — that's it

> ✅ No .NET, no Java, no Python, no Visual C++ Redistributable required.
> Works on Windows 10 and above.

---

## 🛠️ Build from Source

Requires [Rust](https://rustup.rs/) (stable toolchain).

```bash
git clone https://github.com/yourname/rust-seek
cd rust-seek
cargo build --release
# Binary at: target/release/rust-seek.exe
```

The release build enables full LTO and maximum optimization (`opt-level = 3`), producing a small, fast binary.

---

## 🏗️ Architecture

```
src/
├── main.rs       # Entry point, window setup
├── app.rs        # GUI logic (egui), search orchestration, result rendering
└── searcher.rs   # Core search engine: mmap I/O, regex matching, binary detection
```

| Component | Technology | Why |
|-----------|-----------|-----|
| GUI framework | `egui` / `eframe` | Immediate-mode, native, no Electron |
| Directory traversal | `ignore` | Same crate as ripgrep, parallel + gitignore-aware |
| File reading | `memmap2` | OS-level memory mapping for large files |
| Pattern matching | `regex` | Rust's standard regex engine, DFA-based, very fast |
| Folder dialog | `rfd` | Native OS file picker |

---

## ⚡ Performance

Rust Seek uses the same parallel directory walking strategy as `ripgrep` — one of the fastest search tools ever benchmarked. On a modern machine:

- Searches a **100,000-file codebase** in under **2 seconds**
- Handles **files up to 10 MB** with memory-mapped I/O
- Scales linearly with CPU cores — the more cores, the faster

---

## 🗺️ Roadmap

- [ ] Context lines (show N lines before/after each match)
- [ ] File type filter (search only `.rs`, `.py`, etc.)
- [ ] Replace mode (find & replace across files)
- [ ] Search history
- [ ] Dark / light theme toggle

---

## 📄 License

MIT © 2024
