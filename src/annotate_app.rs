use eframe::egui;
use egui::{Color32, ColorImage, Pos2, Rect, Stroke, TextureHandle, Vec2};

// ── Shape types ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
pub enum ShapeKind { Rect, Ellipse, Arrow, Pen }

#[derive(Clone)]
pub struct Annotation {
    pub kind:   ShapeKind,
    pub color:  Color32,
    pub width:  f32,
    pub filled: bool,
    /// Start and end in image-pixel coordinates
    pub p1: Pos2,
    pub p2: Pos2,
    /// Freehand pen points (image-pixel coords), only used for ShapeKind::Pen
    pub pen_points: Vec<Pos2>,
}

// ── Palette ───────────────────────────────────────────────────────────────────

const PALETTE: &[Color32] = &[
    Color32::from_rgb(239,  68,  68),  // red
    Color32::from_rgb(249, 115,  22),  // orange
    Color32::from_rgb(234, 179,   8),  // yellow
    Color32::from_rgb( 34, 197,  94),  // green
    Color32::from_rgb( 59, 130, 246),  // blue
    Color32::from_rgb(168,  85, 247),  // purple
    Color32::from_rgb(236,  72, 153),  // pink
    Color32::WHITE,
    Color32::BLACK,
];

// ── Main struct ───────────────────────────────────────────────────────────────

pub struct AnnotateApp {
    /// Raw RGBA pixels of the captured screenshot
    pixels: Option<Vec<u8>>,
    img_w: usize,
    img_h: usize,
    /// egui texture (re-uploaded when annotations change)
    texture: Option<TextureHandle>,
    texture_dirty: bool,

    annotations: Vec<Annotation>,
    undo_stack:  Vec<Vec<Annotation>>,

    // Current tool state
    tool:       ShapeKind,
    color:      Color32,
    stroke_w:   f32,
    filled:     bool,

    // In-progress drag
    drag_start: Option<Pos2>,   // image-pixel coords
    cur_drag:   Option<Pos2>,
    pen_points: Vec<Pos2>,

    status: String,
}

impl Default for AnnotateApp {
    fn default() -> Self {
        Self {
            pixels: None,
            img_w: 0, img_h: 0,
            texture: None,
            texture_dirty: false,
            annotations: Vec::new(),
            undo_stack: Vec::new(),
            tool: ShapeKind::Rect,
            color: PALETTE[0],
            stroke_w: 3.0,
            filled: false,
            drag_start: None,
            cur_drag: None,
            pen_points: Vec::new(),
            status: "点击「截图」开始".to_string(),
        }
    }
}

impl AnnotateApp {
    /// Take a full-screen screenshot and store raw RGBA pixels.
    pub fn capture(&mut self) {
        // Hide window briefly so it doesn't appear in the screenshot
        std::thread::sleep(std::time::Duration::from_millis(150));
        match screenshots::Screen::all() {
            Ok(screens) => {
                if let Some(screen) = screens.into_iter().next() {
                    match screen.capture() {
                        Ok(img) => {
                            self.img_w = img.width() as usize;
                            self.img_h = img.height() as usize;
                            self.pixels = Some(img.into_raw());
                            self.annotations.clear();
                            self.undo_stack.clear();
                            self.texture = None;
                            self.texture_dirty = true;
                            self.status = format!(
                                "截图完成 {}×{} — 选择工具开始标注",
                                self.img_w, self.img_h
                            );
                        }
                        Err(e) => { self.status = format!("截图失败: {e}"); }
                    }
                } else {
                    self.status = "未找到显示器".to_string();
                }
            }
            Err(e) => { self.status = format!("截图失败: {e}"); }
        }
    }

    fn undo(&mut self) {
        if let Some(prev) = self.undo_stack.pop() {
            self.annotations = prev;
            self.texture_dirty = true;
        }
    }

    /// Flatten annotations onto the base image and return RGBA bytes.
    fn flatten_to_rgba(&self) -> Option<Vec<u8>> {
        let base = self.pixels.as_ref()?;
        let mut out = base.clone();
        let w = self.img_w;
        let h = self.img_h;

        for ann in &self.annotations {
            match ann.kind {
                ShapeKind::Rect => {
                    let x0 = ann.p1.x.min(ann.p2.x) as i32;
                    let y0 = ann.p1.y.min(ann.p2.y) as i32;
                    let x1 = ann.p1.x.max(ann.p2.x) as i32;
                    let y1 = ann.p1.y.max(ann.p2.y) as i32;
                    let lw = ann.width as i32;
                    if ann.filled {
                        fill_rect(&mut out, w, h, x0, y0, x1, y1, ann.color);
                    } else {
                        for t in 0..lw {
                            draw_rect_outline(&mut out, w, h, x0+t, y0+t, x1-t, y1-t, ann.color);
                        }
                    }
                }
                ShapeKind::Ellipse => {
                    let cx = ((ann.p1.x + ann.p2.x) / 2.0) as i32;
                    let cy = ((ann.p1.y + ann.p2.y) / 2.0) as i32;
                    let rx = ((ann.p2.x - ann.p1.x).abs() / 2.0) as i32;
                    let ry = ((ann.p2.y - ann.p1.y).abs() / 2.0) as i32;
                    draw_ellipse(&mut out, w, h, cx, cy, rx, ry, ann.color, ann.width as i32, ann.filled);
                }
                ShapeKind::Arrow => {
                    draw_arrow(&mut out, w, h, ann.p1, ann.p2, ann.color, ann.width as i32);
                }
                ShapeKind::Pen => {
                    for pair in ann.pen_points.windows(2) {
                        draw_line_thick(&mut out, w, h, pair[0], pair[1], ann.color, ann.width as i32);
                    }
                }
            }
        }
        Some(out)
    }

    /// Save annotated image to a PNG file.
    pub fn save_png(&self, path: &std::path::Path) -> anyhow::Result<()> {
        let rgba = self.flatten_to_rgba().ok_or_else(|| anyhow::anyhow!("no image"))?;
        image::save_buffer(path, &rgba, self.img_w as u32, self.img_h as u32, image::ColorType::Rgba8)?;
        Ok(())
    }

    /// Copy annotated image to clipboard.
    pub fn copy_to_clipboard(&self) -> anyhow::Result<()> {
        let rgba = self.flatten_to_rgba().ok_or_else(|| anyhow::anyhow!("no image"))?;
        let img = arboard::ImageData {
            width:  self.img_w,
            height: self.img_h,
            bytes:  std::borrow::Cow::Owned(rgba),
        };
        arboard::Clipboard::new()?.set_image(img)?;
        Ok(())
    }

    pub fn update(&mut self, ctx: &egui::Context) {
        // Rebuild texture when annotations change
        if self.texture_dirty {
            if let Some(rgba) = self.flatten_to_rgba() {
                let ci = ColorImage::from_rgba_unmultiplied(
                    [self.img_w, self.img_h], &rgba,
                );
                self.texture = Some(ctx.load_texture("screenshot", ci, egui::TextureOptions::LINEAR));
            }
            self.texture_dirty = false;
        }

        // Toolbar
        egui::TopBottomPanel::top("ann_toolbar")
            .frame(egui::Frame::side_top_panel(&ctx.style())
                .inner_margin(egui::Margin { left: 10, right: 10, top: 8, bottom: 8 }))
            .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                // Capture button
                if ui.add(egui::Button::new("📷 截图")
                    .fill(Color32::from_rgb(6, 78, 59))
                    .min_size(egui::vec2(72.0, 28.0))
                ).clicked() { self.capture(); }

                ui.add(egui::Separator::default().vertical().spacing(6.0));

                // Tool selector
                ui.label(egui::RichText::new("工具:").color(Color32::from_rgb(140,155,175)).size(12.0));
                for (kind, label, tip) in [
                    (ShapeKind::Rect,    "▭ 矩形", "矩形框"),
                    (ShapeKind::Ellipse, "◯ 椭圆", "椭圆/圆形"),
                    (ShapeKind::Arrow,   "→ 箭头", "箭头"),
                    (ShapeKind::Pen,     "✏ 画笔", "自由绘制"),
                ] {
                    let selected = self.tool == kind;
                    let btn = egui::Button::new(label)
                        .fill(if selected { Color32::from_rgb(6,78,59) } else { Color32::from_rgb(38,42,54) })
                        .min_size(egui::vec2(64.0, 26.0));
                    if ui.add(btn).on_hover_text(tip).clicked() { self.tool = kind; }
                }

                ui.add(egui::Separator::default().vertical().spacing(6.0));

                // Fill toggle
                ui.checkbox(&mut self.filled, "填充").on_hover_text("填充形状内部");

                // Stroke width
                ui.label(egui::RichText::new("粗细:").color(Color32::from_rgb(140,155,175)).size(12.0));
                ui.add(egui::Slider::new(&mut self.stroke_w, 1.0..=20.0).show_value(false));
                ui.label(egui::RichText::new(format!("{:.0}", self.stroke_w)).size(12.0));

                ui.add(egui::Separator::default().vertical().spacing(6.0));

                // Color palette
                ui.label(egui::RichText::new("颜色:").color(Color32::from_rgb(140,155,175)).size(12.0));
                for &c in PALETTE {
                    let selected = self.color == c;
                    let (rect, resp) = ui.allocate_exact_size(
                        egui::vec2(22.0, 22.0), egui::Sense::click()
                    );
                    let painter = ui.painter();
                    painter.rect_filled(rect, 3.0, c);
                    if selected {
                        painter.rect_stroke(rect, 3.0, Stroke::new(2.5, Color32::WHITE), egui::StrokeKind::Outside);
                    }
                    if resp.clicked() { self.color = c; }
                }
                // Custom color picker
                ui.color_edit_button_srgba(&mut self.color);

                ui.add(egui::Separator::default().vertical().spacing(6.0));

                // Undo / clear / save / copy
                let has_ann = !self.annotations.is_empty();
                if ui.add_enabled(has_ann, egui::Button::new("↩ 撤销")
                    .min_size(egui::vec2(56.0, 26.0))
                ).clicked() { self.undo(); }

                if ui.add_enabled(has_ann, egui::Button::new("🗑 清空")
                    .fill(Color32::from_rgb(127,29,29))
                    .min_size(egui::vec2(56.0, 26.0))
                ).clicked() {
                    self.undo_stack.push(self.annotations.clone());
                    self.annotations.clear();
                    self.texture_dirty = true;
                }

                let has_img = self.pixels.is_some();
                if ui.add_enabled(has_img, egui::Button::new("📋 复制")
                    .min_size(egui::vec2(56.0, 26.0))
                ).on_hover_text("复制到剪贴板").clicked() {
                    match self.copy_to_clipboard() {
                        Ok(_)  => self.status = "已复制到剪贴板".to_string(),
                        Err(e) => self.status = format!("复制失败: {e}"),
                    }
                }

                if ui.add_enabled(has_img, egui::Button::new("💾 保存")
                    .min_size(egui::vec2(56.0, 26.0))
                ).clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PNG", &["png"])
                        .set_file_name("screenshot.png")
                        .save_file()
                    {
                        match self.save_png(&path) {
                            Ok(_)  => self.status = format!("已保存: {}", path.display()),
                            Err(e) => self.status = format!("保存失败: {e}"),
                        }
                    }
                }
            });

            // Status bar inside toolbar
            ui.add_space(2.0);
            ui.label(egui::RichText::new(&self.status)
                .color(Color32::from_rgb(148,163,184)).size(11.5));
        });

        // Canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            if self.texture.is_none() {
                ui.centered_and_justified(|ui| {
                    ui.label(egui::RichText::new("📷  点击「截图」按钮开始")
                        .color(Color32::from_rgb(75,85,100)).size(16.0));
                });
                return;
            }

            let tex = self.texture.as_ref().unwrap();
            let img_size = Vec2::new(self.img_w as f32, self.img_h as f32);
            let avail = ui.available_size();

            // Scale to fit while preserving aspect ratio
            let scale = (avail.x / img_size.x).min(avail.y / img_size.y).min(1.0);
            let disp_size = img_size * scale;

            let (canvas_rect, resp) = ui.allocate_exact_size(disp_size, egui::Sense::drag());
            let painter = ui.painter_at(canvas_rect);

            // Draw screenshot
            painter.image(tex.id(), canvas_rect, Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0,1.0)), Color32::WHITE);

            // Helper: screen pos → image pixel pos
            let to_img = |p: Pos2| -> Pos2 {
                Pos2::new(
                    ((p.x - canvas_rect.min.x) / scale).clamp(0.0, img_size.x),
                    ((p.y - canvas_rect.min.y) / scale).clamp(0.0, img_size.y),
                )
            };
            // Helper: image pixel pos → screen pos
            let to_screen = |p: Pos2| -> Pos2 {
                Pos2::new(canvas_rect.min.x + p.x * scale, canvas_rect.min.y + p.y * scale)
            };

            // Draw committed annotations
            for ann in &self.annotations {
                draw_annotation_egui(&painter, ann, scale, &to_screen);
            }

            // Handle drag input
            if resp.drag_started() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if self.tool == ShapeKind::Pen {
                        self.undo_stack.push(self.annotations.clone());
                        self.pen_points.clear();
                        self.pen_points.push(to_img(pos));
                    } else {
                        self.drag_start = Some(to_img(pos));
                    }
                }
            }
            if resp.dragged() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if self.tool == ShapeKind::Pen {
                        self.pen_points.push(to_img(pos));
                        ctx.request_repaint();
                    } else {
                        self.cur_drag = Some(to_img(pos));
                        ctx.request_repaint();
                    }
                }
            }
            if resp.drag_stopped() {
                if let Some(pos) = resp.interact_pointer_pos() {
                    if self.tool == ShapeKind::Pen {
                        self.pen_points.push(to_img(pos));
                        if self.pen_points.len() >= 2 {
                            self.annotations.push(Annotation {
                                kind: ShapeKind::Pen,
                                color: self.color,
                                width: self.stroke_w,
                                filled: false,
                                p1: *self.pen_points.first().unwrap(),
                                p2: *self.pen_points.last().unwrap(),
                                pen_points: self.pen_points.clone(),
                            });
                            self.texture_dirty = true;
                        }
                        self.pen_points.clear();
                    } else if let Some(start) = self.drag_start.take() {
                        let end = to_img(pos);
                        if (end - start).length() > 3.0 {
                            self.undo_stack.push(self.annotations.clone());
                            self.annotations.push(Annotation {
                                kind: self.tool,
                                color: self.color,
                                width: self.stroke_w,
                                filled: self.filled,
                                p1: start, p2: end,
                                pen_points: Vec::new(),
                            });
                            self.texture_dirty = true;
                        }
                        self.cur_drag = None;
                    }
                }
            }

            // Draw in-progress shape preview
            if self.tool == ShapeKind::Pen && self.pen_points.len() >= 2 {
                for pair in self.pen_points.windows(2) {
                    painter.line_segment(
                        [to_screen(pair[0]), to_screen(pair[1])],
                        Stroke::new(self.stroke_w * scale, self.color),
                    );
                }
            } else if let (Some(s), Some(e)) = (self.drag_start, self.cur_drag) {
                let preview = Annotation {
                    kind: self.tool, color: self.color,
                    width: self.stroke_w, filled: self.filled,
                    p1: s, p2: e, pen_points: Vec::new(),
                };
                draw_annotation_egui(&painter, &preview, scale, &to_screen);
            }
        });
    }
}

// ── egui painter helpers ──────────────────────────────────────────────────────

fn draw_annotation_egui(
    painter: &egui::Painter,
    ann: &Annotation,
    scale: f32,
    to_screen: &impl Fn(Pos2) -> Pos2,
) {
    let stroke = Stroke::new(ann.width * scale, ann.color);
    match ann.kind {
        ShapeKind::Rect => {
            let r = Rect::from_two_pos(to_screen(ann.p1), to_screen(ann.p2));
            if ann.filled {
                painter.rect_filled(r, 0.0, ann.color);
            } else {
                painter.rect_stroke(r, 0.0, stroke, egui::StrokeKind::Middle);
            }
        }
        ShapeKind::Ellipse => {
            let center = to_screen(Pos2::new(
                (ann.p1.x + ann.p2.x) / 2.0,
                (ann.p1.y + ann.p2.y) / 2.0,
            ));
            let radii = Vec2::new(
                (ann.p2.x - ann.p1.x).abs() / 2.0 * scale,
                (ann.p2.y - ann.p1.y).abs() / 2.0 * scale,
            );
            // egui 0.31 has no painter.ellipse — draw with path
            let n = 64usize;
            let points: Vec<Pos2> = (0..=n).map(|i| {
                let t = i as f32 / n as f32 * std::f32::consts::TAU;
                center + Vec2::new(radii.x * t.cos(), radii.y * t.sin())
            }).collect();
            if ann.filled {
                painter.add(egui::Shape::convex_polygon(points, ann.color, Stroke::NONE));
            } else {
                painter.add(egui::Shape::closed_line(points, stroke));
            }
        }
        ShapeKind::Arrow => {
            let s = to_screen(ann.p1);
            let e = to_screen(ann.p2);
            painter.arrow(s, e - s, stroke);
        }
        ShapeKind::Pen => {
            for pair in ann.pen_points.windows(2) {
                painter.line_segment([to_screen(pair[0]), to_screen(pair[1])], stroke);
            }
        }
    }
}

// ── Pixel-level drawing (for flatten/export) ──────────────────────────────────

#[inline]
fn set_pixel(buf: &mut [u8], w: usize, h: usize, x: i32, y: i32, c: Color32) {
    if x < 0 || y < 0 || x >= w as i32 || y >= h as i32 { return; }
    let i = (y as usize * w + x as usize) * 4;
    buf[i]   = c.r();
    buf[i+1] = c.g();
    buf[i+2] = c.b();
    buf[i+3] = 255;
}

fn draw_rect_outline(buf: &mut [u8], w: usize, h: usize, x0: i32, y0: i32, x1: i32, y1: i32, c: Color32) {
    for x in x0..=x1 { set_pixel(buf, w, h, x, y0, c); set_pixel(buf, w, h, x, y1, c); }
    for y in y0..=y1 { set_pixel(buf, w, h, x0, y, c); set_pixel(buf, w, h, x1, y, c); }
}

fn fill_rect(buf: &mut [u8], w: usize, h: usize, x0: i32, y0: i32, x1: i32, y1: i32, c: Color32) {
    for y in y0..=y1 { for x in x0..=x1 { set_pixel(buf, w, h, x, y, c); } }
}

fn draw_ellipse(buf: &mut [u8], w: usize, h: usize, cx: i32, cy: i32, rx: i32, ry: i32, c: Color32, lw: i32, filled: bool) {
    if rx <= 0 || ry <= 0 { return; }
    let steps = (2.0 * std::f32::consts::PI * rx.max(ry) as f32) as usize * 2;
    let mut prev: Option<(i32, i32)> = None;
    for i in 0..=steps {
        let t = i as f32 / steps as f32 * 2.0 * std::f32::consts::PI;
        let px = cx + (rx as f32 * t.cos()) as i32;
        let py = cy + (ry as f32 * t.sin()) as i32;
        if filled {
            // Fill by drawing horizontal spans (simple scanline)
            set_pixel(buf, w, h, px, py, c);
        } else {
            if let Some((ppx, ppy)) = prev {
                draw_line_thick_i(buf, w, h, ppx, ppy, px, py, c, lw);
            }
        }
        prev = Some((px, py));
    }
    if filled {
        // Scanline fill
        for y in (cy - ry)..=(cy + ry) {
            let dy = (y - cy) as f32 / ry as f32;
            if dy.abs() > 1.0 { continue; }
            let dx = (1.0 - dy * dy).sqrt() * rx as f32;
            let x0 = cx - dx as i32;
            let x1 = cx + dx as i32;
            for x in x0..=x1 { set_pixel(buf, w, h, x, y, c); }
        }
    }
}

fn draw_line_thick(buf: &mut [u8], w: usize, h: usize, p1: Pos2, p2: Pos2, c: Color32, lw: i32) {
    draw_line_thick_i(buf, w, h, p1.x as i32, p1.y as i32, p2.x as i32, p2.y as i32, c, lw);
}

fn draw_line_thick_i(buf: &mut [u8], w: usize, h: usize, x0: i32, y0: i32, x1: i32, y1: i32, c: Color32, lw: i32) {
    // Bresenham with thickness via perpendicular offset
    let dx = (x1 - x0).abs(); let dy = (y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx - dy;
    let (mut x, mut y) = (x0, y0);
    let half = lw / 2;
    loop {
        for ox in -half..=half { for oy in -half..=half {
            set_pixel(buf, w, h, x + ox, y + oy, c);
        }}
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 > -dy { err -= dy; x += sx; }
        if e2 <  dx { err += dx; y += sy; }
    }
}

fn draw_arrow(buf: &mut [u8], w: usize, h: usize, p1: Pos2, p2: Pos2, c: Color32, lw: i32) {
    draw_line_thick(buf, w, h, p1, p2, c, lw);
    // Arrowhead
    let dx = p2.x - p1.x; let dy = p2.y - p1.y;
    let len = (dx*dx + dy*dy).sqrt().max(1.0);
    let ux = dx / len; let uy = dy / len;
    let head = 18.0_f32;
    let angle = 0.4_f32;
    let ax1 = Pos2::new(p2.x - head*(ux*angle.cos() + uy*angle.sin()), p2.y - head*(-ux*angle.sin() + uy*angle.cos()));
    let ax2 = Pos2::new(p2.x - head*(ux*angle.cos() - uy*angle.sin()), p2.y - head*( ux*angle.sin() + uy*angle.cos()));
    draw_line_thick(buf, w, h, p2, ax1, c, lw);
    draw_line_thick(buf, w, h, p2, ax2, c, lw);
}
