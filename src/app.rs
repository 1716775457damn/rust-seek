use crate::searcher::{search_file, search_filename_with_size, SearchResult};
use crate::sync_app::SyncApp;
use crate::theme;
use eframe::egui;
use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};
use ignore::WalkBuilder;
use regex::RegexBuilder;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

// ── Tab ───────────────────────────────────────────────────────────────────────

#[derive(PartialEq, Clone, Copy)]
enum Tab { Search, Sync }

// ── SearchApp ─────────────────────────────────────────────────────────────────

enum SearchMsg {
    Result(SearchResult),
    Done(u128),
}

#[derive(PartialEq, Clone, Copy, serde::Serialize, serde::Deserialize)]
enum SearchMode { Text, Filename }

const MAX_HISTORY: usize = 20;
const COLLAPSE_THRESHOLD: usize = 5;
const MAX_RESULTS: usize = 2000;

#[derive(Default, serde::Serialize, serde::Deserialize)]
struct Prefs {
    path_history: Vec<String>,
    pattern_history: Vec<String>,
    last_path: String,
    last_mode: Option<SearchMode>,
}

impl Prefs {
    fn load() -> Self {
        prefs_path()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }
}

fn prefs_path() -> Option<std::path::PathBuf> {
    Some(dirs::data_local_dir()?.join("rust-seek").join("prefs.json"))
}

struct SearchApp {
    pattern: String,
    search_path: String,
    ignore_case: bool,
    fixed_string: bool,
    mode: SearchMode,
    results: Vec<SearchResult>,
    total_matches: usize,
    result_capped: bool,
    collapsed: HashSet<String>,
    expanded: HashSet<String>,
    status: String,
    live_count: usize,
    live_matches: usize,
    path_error: Option<String>,
    regex_error: Option<String>,
    last_pat: String,
    last_ic: bool,
    last_fs: bool,
    filter: String,
    filter_lc: String,
    pat_history_idx: Option<usize>,
    searching: bool,
    rx: Option<Receiver<SearchMsg>>,
    cancel: Option<Arc<AtomicBool>>,
    last_repaint: Instant,
    prefs: Prefs,
    needs_focus: bool,
}

impl Default for SearchApp {
    fn default() -> Self {
        let prefs = Prefs::load();
        let cwd = if prefs.last_path.is_empty() {
            std::env::current_dir().unwrap_or_default()
                .to_string_lossy().replace('\\', "/")
        } else {
            prefs.last_path.clone()
        };
        Self {
            pattern: String::new(),
            search_path: cwd,
            ignore_case: true,
            fixed_string: false,
            mode: prefs.last_mode.unwrap_or(SearchMode::Filename),
            results: Vec::new(),
            total_matches: 0,
            result_capped: false,
            collapsed: HashSet::new(),
            expanded: HashSet::new(),
            status: String::from("就绪"),
            live_count: 0,
            live_matches: 0,
            path_error: None,
            regex_error: None,
            last_pat: String::new(),
            last_ic: false,
            last_fs: false,
            filter: String::new(),
            filter_lc: String::new(),
            pat_history_idx: None,
            searching: false,
            rx: None,
            cancel: None,
            last_repaint: Instant::now(),
            prefs,
            needs_focus: true,
        }
    }
}

impl SearchApp {
    fn start_search(&mut self) {
        if self.pattern.is_empty() { return; }
        if !std::path::Path::new(&self.search_path).exists() {
            self.path_error = Some(format!("路径不存在: {}", self.search_path));
            return;
        }
        self.path_error = None;
        self.cancel_search();

        push_history(&mut self.prefs.path_history, self.search_path.clone());
        push_history(&mut self.prefs.pattern_history, self.pattern.clone());
        self.prefs.last_path = self.search_path.clone();
        self.prefs.last_mode = Some(self.mode);
        let prefs_clone = serde_json::to_string(&self.prefs).ok();
        thread::spawn(move || {
            if let (Some(p), Some(s)) = (prefs_path(), prefs_clone) {
                let _ = std::fs::create_dir_all(p.parent().unwrap());
                let _ = std::fs::write(p, s);
            }
        });

        let pat = if self.fixed_string { regex::escape(&self.pattern) } else { self.pattern.clone() };
        let re = match RegexBuilder::new(&pat)
            .case_insensitive(self.ignore_case)
            .unicode(true)
            .build()
        {
            Ok(r) => r,
            Err(e) => { self.status = format!("无效的正则: {e}"); return; }
        };

        self.results.clear();
        self.total_matches = 0;
        self.result_capped = false;
        self.collapsed.clear(); self.collapsed.shrink_to_fit();
        self.expanded.clear();  self.expanded.shrink_to_fit();
        self.filter.clear();    self.filter_lc.clear();
        self.live_count = 0;    self.live_matches = 0;
        self.searching = true;
        self.status = "搜索中…".to_string();

        let (tx, rx): (Sender<SearchMsg>, Receiver<SearchMsg>) = mpsc::channel();
        let cancelled = Arc::new(AtomicBool::new(false));
        self.rx = Some(rx);
        self.cancel = Some(cancelled.clone());

        let path = self.search_path.clone();
        let threads = num_cpus::get();
        let mode = self.mode;

        thread::spawn(move || {
            let start = Instant::now();
            let walker = WalkBuilder::new(&path)
                .hidden(true).git_ignore(false).ignore(false)
                .threads(threads).build_parallel();
            walker.run(|| {
                let tx = tx.clone();
                let re = re.clone();
                let cancelled = cancelled.clone();
                Box::new(move |entry| {
                    if cancelled.load(Ordering::Relaxed) { return ignore::WalkState::Quit; }
                    let entry = match entry { Ok(e) => e, Err(_) => return ignore::WalkState::Continue };
                    let result = match mode {
                        SearchMode::Filename => {
                            // Use metadata from DirEntry to avoid a redundant stat() call
                            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
                            search_filename_with_size(entry.path(), &re, size)
                        }
                        SearchMode::Text => {
                            if !entry.file_type().map_or(false, |ft| ft.is_file()) {
                                return ignore::WalkState::Continue;
                            }
                            search_file(entry.path(), &re, 10 * 1024 * 1024).ok().flatten()
                        }
                    };
                    if let Some(r) = result {
                        if tx.send(SearchMsg::Result(r)).is_err() { return ignore::WalkState::Quit; }
                    }
                    ignore::WalkState::Continue
                })
            });
            let _ = tx.send(SearchMsg::Done(start.elapsed().as_millis()));
        });
    }

    fn cancel_search(&mut self) {
        if let Some(c) = self.cancel.take() { c.store(true, Ordering::Relaxed); }
        self.rx = None;
        self.searching = false;
    }

    fn update(&mut self, ctx: &egui::Context) {
        // Drain results
        if self.rx.is_some() {
            let mut done = false;
            let mut got  = false;
            loop {
                match self.rx.as_ref().unwrap().try_recv() {
                    Ok(SearchMsg::Result(r)) => {
                        if self.results.len() < MAX_RESULTS {
                            self.live_matches += r.matches.len();
                            self.results.push(r);
                            self.live_count += 1;
                        } else if !self.result_capped {
                            self.result_capped = true;
                            if let Some(ref c) = self.cancel { c.store(true, Ordering::Relaxed); }
                        }
                        got = true;
                    }
                    Ok(SearchMsg::Done(ms)) => {
                        self.results.sort_unstable_by(|a, b| a.path.cmp(&b.path));
                        self.total_matches = self.live_matches;
                        self.status = if self.results.is_empty() {
                            format!("未找到结果 ({}ms)", ms)
                        } else if self.mode == SearchMode::Filename {
                            if self.result_capped {
                                format!("找到 {}+ 个文件（已截断）({}ms)", MAX_RESULTS, ms)
                            } else {
                                format!("找到 {} 个文件 ({}ms)", self.results.len(), ms)
                            }
                        } else {
                            format!("{} 处匹配，共 {} 个文件 ({}ms)", self.total_matches, self.results.len(), ms)
                        };
                        self.searching = false;
                        self.cancel = None;
                        done = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
            if done { self.rx = None; }
            if got {
                self.status = if self.mode == SearchMode::Filename {
                    format!("搜索中… 已找到 {} 个文件{}", self.live_count,
                        if self.result_capped { "（已达上限）" } else { "" })
                } else {
                    format!("搜索中… {} 处匹配 / {} 个文件", self.live_matches, self.live_count)
                };
                let now = Instant::now();
                if now.duration_since(self.last_repaint) >= Duration::from_millis(100) {
                    self.last_repaint = now;
                    ctx.request_repaint();
                }
            }
        }

        // Drag-and-drop folder onto window
        ctx.input(|i| {
            if let Some(dropped) = i.raw.dropped_files.first() {
                if let Some(ref p) = dropped.path {
                    self.search_path = p.to_string_lossy().replace('\\', "/");
                    self.path_error = None;
                }
            }
        });

        // Esc cancels
        if self.searching && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.cancel_search();
            self.status = "已取消".to_string();
        }

        // Toolbar
        egui::TopBottomPanel::top("search_toolbar")
            .frame(egui::Frame::side_top_panel(&ctx.style())
                .inner_margin(egui::Margin { left: 14, right: 14, top: 10, bottom: 8 }))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("路径").color(Color32::from_rgb(140, 155, 175)).size(12.0));
                let path_id = ui.make_persistent_id("path_popup");
                let path_resp = ui.add(
                    TextEdit::singleline(&mut self.search_path)
                        .desired_width(190.0)
                        .font(egui::TextStyle::Body)
                        .text_color(if self.path_error.is_some() {
                            Color32::from_rgb(248, 113, 113)
                        } else {
                            Color32::from_rgb(220, 228, 240)
                        })
                        .frame(true),
                );
                if path_resp.gained_focus() && !self.prefs.path_history.is_empty() {
                    ui.memory_mut(|m| m.open_popup(path_id));
                }
                egui::popup_below_widget(ui, path_id, &path_resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                    ui.set_min_width(260.0);
                    let history: Vec<String> = self.prefs.path_history.iter().take(10).cloned().collect();
                    for h in history {
                        if ui.selectable_label(false, &h).clicked() {
                            self.search_path = h;
                            ui.memory_mut(|m| m.close_popup());
                        }
                    }
                });
                if ui.add(egui::Button::new("📁").min_size(egui::vec2(28.0, 28.0))).clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        self.search_path = p.to_string_lossy().replace('\\', "/");
                        self.path_error = None;
                    }
                }

                ui.add(egui::Separator::default().vertical().spacing(6.0));

                let prev_mode = self.mode;
                ui.selectable_value(&mut self.mode, SearchMode::Filename, "🗂 文件名")
                    .on_hover_text("按文件/文件夹名搜索");
                ui.selectable_value(&mut self.mode, SearchMode::Text, "📄 文本")
                    .on_hover_text("搜索文件内容");
                if self.mode != prev_mode {
                    self.results.clear();
                    self.cancel_search();
                    self.status = "就绪".to_string();
                }

                ui.add(egui::Separator::default().vertical().spacing(6.0));

                let pat_id = ui.make_persistent_id("pat_popup");
                let pat_resp = ui.add(
                    TextEdit::singleline(&mut self.pattern)
                        .hint_text(if self.mode == SearchMode::Filename { "文件名…" } else { "关键词…" })
                        .desired_width(220.0)
                        .font(egui::TextStyle::Body)
                        .frame(true),
                );

                // Live regex validation
                if self.pattern != self.last_pat || self.ignore_case != self.last_ic || self.fixed_string != self.last_fs {
                    self.last_pat = self.pattern.clone();
                    self.last_ic  = self.ignore_case;
                    self.last_fs  = self.fixed_string;
                    let pat_check = if self.fixed_string { regex::escape(&self.pattern) } else { self.pattern.clone() };
                    self.regex_error = if self.pattern.is_empty() { None } else {
                        RegexBuilder::new(&pat_check).case_insensitive(self.ignore_case)
                            .build().err().map(|e| format!("{e}"))
                    };
                }

                if self.needs_focus { pat_resp.request_focus(); self.needs_focus = false; }

                if pat_resp.gained_focus() && !self.prefs.pattern_history.is_empty() {
                    ui.memory_mut(|m| m.open_popup(pat_id));
                }
                if pat_resp.has_focus() && !self.prefs.pattern_history.is_empty() {
                    let up   = ui.input(|i| i.key_pressed(egui::Key::ArrowUp));
                    let down = ui.input(|i| i.key_pressed(egui::Key::ArrowDown));
                    if up || down {
                        let len = self.prefs.pattern_history.len();
                        let idx = match self.pat_history_idx {
                            None    => if up { Some(0) } else { None },
                            Some(i) => if up { Some((i+1).min(len-1)) } else if i==0 { None } else { Some(i-1) },
                        };
                        self.pat_history_idx = idx;
                        if let Some(i) = idx { self.pattern = self.prefs.pattern_history[i].clone(); }
                    }
                }
                if pat_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.start_search();
                }
                egui::popup_below_widget(ui, pat_id, &pat_resp, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                    ui.set_min_width(260.0);
                    let history: Vec<String> = self.prefs.pattern_history.iter().take(10).cloned().collect();
                    for h in history {
                        if ui.selectable_label(false, &h).clicked() {
                            self.pattern = h;
                            self.pat_history_idx = None;
                            ui.memory_mut(|m| m.close_popup());
                        }
                    }
                });

                if self.searching {
                    if ui.add(egui::Button::new("⏹  取消")
                        .fill(Color32::from_rgb(127, 29, 29))
                        .min_size(egui::vec2(72.0, 28.0))
                    ).on_hover_text("也可按 Esc").clicked() {
                        self.cancel_search();
                        self.status = "已取消".to_string();
                    }
                } else {
                    if ui.add(egui::Button::new("🔍  搜索")
                        .fill(Color32::from_rgb(7, 89, 133))
                        .min_size(egui::vec2(72.0, 28.0))
                    ).clicked() { self.start_search(); }
                }

                ui.add(egui::Separator::default().vertical().spacing(6.0));
                ui.checkbox(&mut self.ignore_case, "Aa").on_hover_text("忽略大小写");
                ui.checkbox(&mut self.fixed_string, "F").on_hover_text("纯文本（不使用正则）");
            });

            if let Some(ref err) = self.path_error {
                ui.add_space(2.0);
                ui.label(RichText::new(format!("⚠  {err}")).color(Color32::from_rgb(248, 113, 113)).size(11.5));
            }
            if let Some(ref err) = self.regex_error {
                ui.add_space(2.0);
                ui.label(RichText::new(format!("正则错误: {err}")).color(Color32::from_rgb(251, 146, 60)).size(11.5));
            }
        });

        // Status bar
        egui::TopBottomPanel::bottom("search_status")
            .frame(egui::Frame::side_top_panel(&ctx.style())
                .inner_margin(egui::Margin { left: 14, right: 14, top: 5, bottom: 5 }))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if self.searching { ui.spinner(); ui.add_space(4.0); }
                let status_color = if self.status.contains("未找到") {
                    Color32::from_rgb(156, 163, 175)
                } else if self.status.contains("错误") {
                    Color32::from_rgb(248, 113, 113)
                } else {
                    Color32::from_rgb(148, 163, 184)
                };
                ui.label(RichText::new(&self.status).color(status_color).size(11.5));
            });
        });

        // Results
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.results.is_empty() && !self.searching {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("🔍  输入关键词后按回车搜索")
                        .color(Color32::from_rgb(75, 85, 100)).size(14.0));
                });
                return;
            }

            if !self.results.is_empty() {
                egui::Frame::new()
                    .inner_margin(egui::Margin { left: 4, right: 4, top: 4, bottom: 6 })
                    .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        if self.mode == SearchMode::Text {
                            if ui.small_button("▼ 全部展开").clicked() { self.collapsed.clear(); }
                            if ui.small_button("▶ 全部折叠").clicked() {
                                for r in &self.results { self.collapsed.insert(r.path.clone()); }
                            }
                            ui.add(egui::Separator::default().vertical().spacing(4.0));
                        }
                        if ui.small_button("📋 复制全部路径").clicked() {
                            let all = self.results.iter().map(|r| r.path.as_str()).collect::<Vec<_>>().join("\n");
                            ctx.copy_text(all);
                        }
                        if self.result_capped {
                            ui.add_space(6.0);
                            ui.label(RichText::new(format!("⚠  结果已截断至 {} 条", MAX_RESULTS))
                                .color(Color32::from_rgb(251, 191, 36)).size(11.5));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if !self.filter.is_empty() && ui.small_button("✕").clicked() {
                                self.filter.clear(); self.filter_lc.clear();
                            }
                            ui.add(TextEdit::singleline(&mut self.filter)
                                .hint_text("过滤结果…").desired_width(150.0)
                                .font(egui::TextStyle::Small));
                            ui.label(RichText::new("过滤:").color(Color32::from_rgb(120, 135, 155)).size(12.0));
                            let new_lc = self.filter.to_lowercase();
                            if new_lc != self.filter_lc { self.filter_lc = new_lc; }
                        });
                    });
                });
                ui.add(egui::Separator::default().spacing(0.0));
            }

            let mut toggle_collapse: Option<String> = None;
            let mut toggle_expand:   Option<String> = None;

            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                for result in &self.results {
                    if !self.filter_lc.is_empty() && !result.path_lc.contains(&self.filter_lc) { continue; }
                    let is_collapsed = self.collapsed.contains(&result.path);
                    let is_expanded  = self.expanded.contains(&result.path);
                    let shown = if self.mode == SearchMode::Text && !is_collapsed {
                        if is_expanded { result.matches.len() } else { result.matches.len().min(COLLAPSE_THRESHOLD) }
                    } else { 0 };
                    let has_more = self.mode == SearchMode::Text
                        && !is_collapsed
                        && result.matches.len() > COLLAPSE_THRESHOLD
                        && !is_expanded;
                    match render_result(ui, result, self.mode, is_collapsed, shown, has_more, ctx) {
                        RowAction::ToggleCollapse(p) => toggle_collapse = Some(p),
                        RowAction::ToggleExpand(p)   => toggle_expand   = Some(p),
                        RowAction::None => {}
                    }
                }
            });

            if let Some(p) = toggle_collapse {
                if self.collapsed.contains(&p) { self.collapsed.remove(&p); } else { self.collapsed.insert(p); }
            }
            if let Some(p) = toggle_expand { self.expanded.insert(p); }
        });
    } // end SearchApp::update
} // end impl SearchApp

// ── Top-level App ─────────────────────────────────────────────────────────────

pub struct App {
    tab:    Tab,
    search: SearchApp,
    sync:   SyncApp,
    is_dark: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            tab:    Tab::Search,
            search: SearchApp::default(),
            sync:   SyncApp::default(),
            is_dark: true,
        }
    }
}

impl eframe::App for App {
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.sync.on_exit();
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // T key toggles theme globally
        if ctx.input(|i| i.key_pressed(egui::Key::T)) {
            self.is_dark = !self.is_dark;
            ctx.set_visuals(if self.is_dark { theme::dark_visuals() } else { theme::light_visuals() });
        }

        // Tab bar + theme toggle
        egui::TopBottomPanel::top("tab_bar")
            .frame(egui::Frame::side_top_panel(&ctx.style())
                .inner_margin(egui::Margin { left: 10, right: 10, top: 6, bottom: 0 }))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.selectable_value(&mut self.tab, Tab::Search, "🔍 搜索");
                ui.selectable_value(&mut self.tab, Tab::Sync,   "🔄 同步");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let (icon, tip) = if self.is_dark { ("☀️", "切换浅色 (T)") } else { ("🌙", "切换深色 (T)") };
                    if ui.add(egui::Button::new(icon).min_size(egui::vec2(28.0, 24.0)))
                        .on_hover_text(tip).clicked()
                    {
                        self.is_dark = !self.is_dark;
                        ctx.set_visuals(if self.is_dark { theme::dark_visuals() } else { theme::light_visuals() });
                    }
                });
            });
        });

        // Update window title based on active tab
        let title = match self.tab {
            Tab::Search => if self.search.searching {
                "Rust Seek — 搜索中…".to_string()
            } else if !self.search.results.is_empty() {
                format!("Rust Seek — {}", self.search.status)
            } else {
                "Rust Seek".to_string()
            },
            Tab::Sync => "Rust Seek — 同步".to_string(),
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));

        // Route to active tab
        match self.tab {
            Tab::Search => self.search.update(ctx),
            Tab::Sync   => self.sync.update(ctx),
        }
    }
}

// ── Render helpers ────────────────────────────────────────────────────────────

enum RowAction { None, ToggleCollapse(String), ToggleExpand(String) }

fn render_result(
    ui: &mut Ui,
    result: &SearchResult,
    mode: SearchMode,
    is_collapsed: bool,
    shown_matches: usize,
    has_more: bool,
    ctx: &egui::Context,
) -> RowAction {
    let mut action = RowAction::None;

    ui.horizontal(|ui| {
        if mode == SearchMode::Text {
            let arrow = if is_collapsed { "▶" } else { "▼" };
            if ui.small_button(arrow).clicked() {
                action = RowAction::ToggleCollapse(result.path.clone());
            }
        }
        ui.label(result.icon);

        let link = if mode == SearchMode::Filename {
            let file_name = result.path.rsplit('/').next().unwrap_or(&result.path);
            let parent    = result.path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
            let l = ui.link(RichText::new(file_name).color(Color32::from_rgb(100, 180, 255)).strong());
            if !parent.is_empty() {
                ui.label(RichText::new(parent).color(Color32::DARK_GRAY).small());
            }
            if result.file_size > 0 {
                ui.label(RichText::new(&result.file_size_str).color(Color32::DARK_GRAY).small());
            }
            l
        } else {
            let l = ui.link(RichText::new(&result.path).color(Color32::from_rgb(100, 180, 255)).strong());
            if !is_collapsed {
                ui.label(RichText::new(format!("({} 处匹配)", result.matches.len())).color(Color32::GRAY).small());
            }
            l
        };

        if link.clicked() { let _ = open::that(&result.win_path); }

        if mode == SearchMode::Filename {
            if let Some(m) = result.matches.first() {
                ui.add_space(4.0);
                render_highlighted(ui, &m.line, &m.ranges, false);
            }
        }

        link.context_menu(|ui| {
            if ui.button("📂  在文件夹中显示").clicked() { reveal_in_explorer(&result.win_path); ui.close_menu(); }
            if ui.button("▶  打开").clicked() { let _ = open::that(&result.win_path); ui.close_menu(); }
            ui.separator();
            if ui.button("📋  复制路径").clicked() { ctx.copy_text(result.path.clone()); ui.close_menu(); }
            if ui.button("📋  复制文件夹路径").clicked() {
                let p = result.path.rsplit_once('/').map(|(p,_)| p).unwrap_or("").to_string();
                ctx.copy_text(p); ui.close_menu();
            }
        });
    });

    if mode == SearchMode::Text && !is_collapsed {
        let mut last_shown_line: Option<usize> = None;
        for m in result.matches.iter().take(shown_matches) {
            let before_line_num = m.line_num.saturating_sub(1);
            if let Some(ref before) = m.context_before {
                if last_shown_line != Some(before_line_num) {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{:>4}  ", before_line_num)).color(Color32::DARK_GRAY).monospace());
                        ui.label(RichText::new(truncate_display(before)).color(Color32::DARK_GRAY).monospace());
                    });
                }
            }
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.label(RichText::new(format!("{:>4}: ", m.line_num)).color(Color32::from_rgb(100, 200, 100)).monospace());
                render_highlighted(ui, &m.line, &m.ranges, true);
            });
            last_shown_line = Some(m.line_num);
            if let Some(ref after) = m.context_after {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{:>4}  ", m.line_num + 1)).color(Color32::DARK_GRAY).monospace());
                    ui.label(RichText::new(truncate_display(after)).color(Color32::DARK_GRAY).monospace());
                });
                last_shown_line = Some(m.line_num + 1);
            }
            ui.add_space(2.0);
        }
        if has_more {
            let remaining = result.matches.len() - shown_matches;
            if ui.small_button(RichText::new(format!("  ↓ 显示另外 {} 处匹配", remaining)).color(Color32::GRAY).small())
                .clicked()
            {
                action = RowAction::ToggleExpand(result.path.clone());
            }
        }
        ui.add(egui::Separator::default().spacing(6.0));
    }

    action
}

fn render_highlighted(ui: &mut egui::Ui, line: &str, ranges: &[(usize, usize)], monospace: bool) {
    use egui::text::{LayoutJob, TextFormat};
    use egui::FontId;

    let mut job = LayoutJob::default();
    let font = if monospace { FontId::monospace(14.0) } else { FontId::proportional(12.0) };
    let normal_color = if monospace { Color32::LIGHT_GRAY } else { Color32::GRAY };
    let fmt_normal    = TextFormat { font_id: font.clone(), color: normal_color, ..Default::default() };
    let fmt_highlight = TextFormat {
        font_id: font, color: Color32::BLACK,
        background: Color32::from_rgb(255, 200, 0), ..Default::default()
    };

    let chars: Vec<char> = line.chars().collect();
    let char_to_byte: Vec<usize> = line.char_indices().map(|(b, _)| b).collect();
    let total_chars = chars.len();

    let char_slice = |from: usize, to: usize| -> &str {
        let b_start = char_to_byte.get(from).copied().unwrap_or(line.len());
        let b_end   = char_to_byte.get(to).copied().unwrap_or(line.len());
        &line[b_start..b_end]
    };

    let mut cursor = 0usize;
    for &(start, end) in ranges {
        let start = start.min(total_chars);
        let end   = end.min(total_chars);
        if start > cursor { job.append(char_slice(cursor, start), 0.0, fmt_normal.clone()); }
        if start < end   { job.append(char_slice(start, end),    0.0, fmt_highlight.clone()); }
        cursor = end;
    }
    if cursor < total_chars { job.append(char_slice(cursor, total_chars), 0.0, fmt_normal); }
    ui.label(job);
}

fn truncate_display(s: &str) -> &str {
    if s.len() <= 200 { return s; }
    match s.char_indices().nth(200) { Some((i, _)) => &s[..i], None => s }
}

fn reveal_in_explorer(path: &str) {
    #[cfg(target_os = "windows")]
    { let _ = std::process::Command::new("explorer").arg(format!("/select,{}", path)).spawn(); }
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg("-R").arg(path).spawn(); }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    { if let Some(p) = std::path::Path::new(path).parent() { let _ = std::process::Command::new("xdg-open").arg(p).spawn(); } }
}

fn push_history(history: &mut Vec<String>, value: String) {
    history.retain(|h| h != &value);
    history.insert(0, value);
    history.truncate(MAX_HISTORY);
}
