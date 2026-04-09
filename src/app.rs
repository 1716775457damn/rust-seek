use crate::searcher::{search_file, SearchResult};
use eframe::egui;
use egui::{Color32, RichText, ScrollArea, TextEdit, Ui};
use ignore::WalkBuilder;
use regex::RegexBuilder;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

enum SearchMsg {
    Result(SearchResult),
    Done(u128), // elapsed ms
}

pub struct App {
    pattern: String,
    search_path: String,
    ignore_case: bool,
    fixed_string: bool,
    results: Vec<SearchResult>,
    status: String,
    searching: bool,
    rx: Option<Receiver<SearchMsg>>,
}

impl Default for App {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            search_path: ".".to_string(),
            ignore_case: false,
            fixed_string: false,
            results: Vec::new(),
            status: String::from("Ready"),
            searching: false,
            rx: None,
        }
    }
}

impl App {
    fn start_search(&mut self) {
        if self.pattern.is_empty() || self.searching { return; }

        let pat = if self.fixed_string { regex::escape(&self.pattern) } else { self.pattern.clone() };
        let re = match RegexBuilder::new(&pat).case_insensitive(self.ignore_case).build() {
            Ok(r) => r,
            Err(e) => { self.status = format!("Invalid pattern: {e}"); return; }
        };

        self.results.clear();
        self.searching = true;
        self.status = "Searching…".to_string();

        let (tx, rx): (Sender<SearchMsg>, Receiver<SearchMsg>) = mpsc::channel();
        self.rx = Some(rx);

        let path = self.search_path.clone();
        let threads = num_cpus::get();

        thread::spawn(move || {
            let start = std::time::Instant::now();
            let walker = WalkBuilder::new(&path).hidden(false).threads(threads).build_parallel();
            walker.run(|| {
                let tx = tx.clone();
                let re = re.clone();
                Box::new(move |entry| {
                    let entry = match entry { Ok(e) => e, Err(_) => return ignore::WalkState::Continue };
                    if !entry.file_type().map_or(false, |ft| ft.is_file()) {
                        return ignore::WalkState::Continue;
                    }
                    if let Ok(Some(result)) = search_file(entry.path(), &re, 10 * 1024 * 1024) {
                        if tx.send(SearchMsg::Result(result)).is_err() {
                            return ignore::WalkState::Quit;
                        }
                    }
                    ignore::WalkState::Continue
                })
            });
            let _ = tx.send(SearchMsg::Done(start.elapsed().as_millis()));
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Drain background results
        if let Some(rx) = &self.rx {
            let mut done = false;
            loop {
                match rx.try_recv() {
                    Ok(SearchMsg::Result(r)) => { self.results.push(r); ctx.request_repaint(); }
                    Ok(SearchMsg::Done(ms)) => {
                        let total_matches: usize = self.results.iter().map(|r| r.matches.len()).sum();
                        self.status = format!("{} matches in {} files ({}ms)",
                            total_matches, self.results.len(), ms);
                        self.results.sort_by(|a, b| a.path.cmp(&b.path));
                        self.searching = false;
                        done = true;
                        break;
                    }
                    Err(_) => break,
                }
            }
            if done { self.rx = None; }
        }

        // Top search bar
        egui::TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                // Path picker
                ui.label("Path:");
                ui.add(TextEdit::singleline(&mut self.search_path).desired_width(180.0));
                if ui.button("📁").clicked() {
                    if let Some(p) = rfd::FileDialog::new().pick_folder() {
                        self.search_path = p.to_string_lossy().to_string();
                    }
                }

                ui.separator();

                // Pattern input — trigger search on Enter
                ui.label("Search:");
                let resp = ui.add(
                    TextEdit::singleline(&mut self.pattern)
                        .hint_text("pattern…")
                        .desired_width(220.0),
                );
                if resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.start_search();
                }

                if ui.button(if self.searching { "⏳" } else { "🔍 Search" }).clicked() {
                    self.start_search();
                }

                ui.separator();
                ui.checkbox(&mut self.ignore_case, "Aa");
                ui.checkbox(&mut self.fixed_string, "F");
            });
            ui.add_space(4.0);
        });

        // Status bar
        egui::TopBottomPanel::bottom("status").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new(&self.status).color(Color32::GRAY).small());
            });
        });

        // Results
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.results.is_empty() && !self.searching {
                ui.centered_and_justified(|ui| {
                    ui.label(RichText::new("Enter a pattern and press Search").color(Color32::GRAY));
                });
                return;
            }

            ScrollArea::vertical().auto_shrink(false).show(ui, |ui| {
                for result in &self.results {
                    render_result(ui, result);
                }
            });
        });
    }
}

fn render_result(ui: &mut Ui, result: &SearchResult) {
    let path = result.path.clone();
    ui.horizontal(|ui| {
        if ui.link(RichText::new(&result.path).color(Color32::from_rgb(100, 180, 255)).strong()).clicked() {
            // Open file with system default editor
            let _ = open::that(path.replace('/', "\\"));
        }
        ui.label(RichText::new(format!("({} matches)", result.matches.len())).color(Color32::GRAY).small());
    });

    for m in &result.matches {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            // Line number
            ui.label(RichText::new(format!("{:>4}: ", m.line_num)).color(Color32::from_rgb(100, 200, 100)).monospace());
            // Render line with highlighted matches
            let line = &m.line;
            let mut cursor = 0;
            for &(start, end) in &m.ranges {
                if start > cursor {
                    ui.label(RichText::new(&line[cursor..start]).monospace());
                }
                ui.label(RichText::new(&line[start..end]).monospace().color(Color32::BLACK).background_color(Color32::from_rgb(255, 200, 0)));
                cursor = end;
            }
            if cursor < line.len() {
                ui.label(RichText::new(&line[cursor..]).monospace());
            }
        });
    }

    ui.add(egui::Separator::default().spacing(8.0));
}
