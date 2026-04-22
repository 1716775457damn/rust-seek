//! Theme definitions — dark and light variants.
//! Both share the same spacing/typography; only Visuals differ.

use egui::{Color32, CornerRadius, Margin, Stroke, Style, TextStyle, FontId, Visuals};

pub fn apply_style(ctx: &egui::Context) {
    let mut style = (*ctx.style()).clone();
    style.text_styles = [
        (TextStyle::Small,     FontId::proportional(11.5)),
        (TextStyle::Body,      FontId::proportional(13.5)),
        (TextStyle::Button,    FontId::proportional(13.0)),
        (TextStyle::Heading,   FontId::proportional(16.0)),
        (TextStyle::Monospace, FontId::monospace(13.0)),
    ].into();
    style.spacing.item_spacing      = egui::vec2(8.0, 5.0);
    style.spacing.button_padding    = egui::vec2(10.0, 5.0);
    style.spacing.window_margin     = Margin::same(12);
    style.spacing.indent            = 18.0;
    style.spacing.interact_size     = egui::vec2(40.0, 28.0);
    style.spacing.scroll.bar_width  = 6.0;
    style.spacing.scroll.bar_inner_margin = 2.0;
    ctx.set_style(style);
}

/// Sky-400 accent — used by both themes.
pub const ACCENT_SEEK: Color32 = Color32::from_rgb(56, 189, 248);

pub fn dark_visuals() -> Visuals {
    let accent     = ACCENT_SEEK;
    let accent_dim = Color32::from_rgb(14, 116, 144);

    let mut v = Visuals::dark();
    v.window_fill      = Color32::from_rgb(20, 22, 28);
    v.panel_fill       = Color32::from_rgb(26, 28, 36);
    v.faint_bg_color   = Color32::from_rgb(32, 35, 44);
    v.extreme_bg_color = Color32::from_rgb(15, 16, 21);
    v.window_stroke    = Stroke::new(1.0, Color32::from_rgb(55, 60, 75));

    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(45, 50, 65));
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(50, 55, 70));

    widget_states(&mut v, accent, accent_dim,
        Color32::from_rgb(38, 42, 54),
        Color32::from_rgb(55, 60, 78),
        Color32::from_rgb(48, 54, 70),
        Color32::from_rgb(80, 90, 115),
        Color32::from_rgb(30, 34, 46),
        Color32::from_rgb(180, 190, 210),
    );

    v.selection.bg_fill  = Color32::from_rgba_unmultiplied(56, 189, 248, 55);
    v.selection.stroke   = Stroke::new(1.0, accent);
    v.hyperlink_color    = accent;
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };
    v
}

pub fn light_visuals() -> Visuals {
    let accent     = Color32::from_rgb(2, 132, 199);   // sky-600 — darker for light bg
    let accent_dim = Color32::from_rgb(186, 230, 253); // sky-200

    let mut v = Visuals::light();
    v.window_fill      = Color32::from_rgb(248, 250, 252); // slate-50
    v.panel_fill       = Color32::from_rgb(241, 245, 249); // slate-100
    v.faint_bg_color   = Color32::from_rgb(226, 232, 240); // slate-200
    v.extreme_bg_color = Color32::WHITE;
    v.window_stroke    = Stroke::new(1.0, Color32::from_rgb(203, 213, 225));

    v.widgets.noninteractive.bg_stroke = Stroke::new(1.0, Color32::from_rgb(203, 213, 225));
    v.widgets.noninteractive.fg_stroke = Stroke::new(1.0, Color32::from_rgb(203, 213, 225));

    widget_states(&mut v, accent, accent_dim,
        Color32::from_rgb(241, 245, 249),
        Color32::from_rgb(203, 213, 225),
        Color32::from_rgb(226, 232, 240),
        Color32::from_rgb(148, 163, 184),
        Color32::from_rgb(248, 250, 252),
        Color32::from_rgb(51, 65, 85),
    );

    v.selection.bg_fill  = Color32::from_rgba_unmultiplied(2, 132, 199, 40);
    v.selection.stroke   = Stroke::new(1.0, accent);
    v.hyperlink_color    = accent;
    v.override_text_color = Some(Color32::from_rgb(15, 23, 42)); // slate-900
    v.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 0.5 };
    v
}

fn widget_states(
    v: &mut Visuals,
    accent: Color32, accent_dim: Color32,
    inactive_fill: Color32, inactive_stroke_col: Color32,
    hovered_fill: Color32, hovered_stroke_col: Color32,
    open_fill: Color32,
    fg_col: Color32,
) {
    let cr = CornerRadius::same(6);

    v.widgets.inactive.bg_fill      = inactive_fill;
    v.widgets.inactive.bg_stroke    = Stroke::new(1.0, inactive_stroke_col);
    v.widgets.inactive.corner_radius = cr;
    v.widgets.inactive.fg_stroke    = Stroke::new(1.5, fg_col);

    v.widgets.hovered.bg_fill       = hovered_fill;
    v.widgets.hovered.bg_stroke     = Stroke::new(1.0, hovered_stroke_col);
    v.widgets.hovered.corner_radius  = cr;
    v.widgets.hovered.fg_stroke     = Stroke::new(1.5, Color32::from_rgb(15, 23, 42));
    v.widgets.hovered.expansion     = 1.0;

    v.widgets.active.bg_fill        = accent_dim;
    v.widgets.active.bg_stroke      = Stroke::new(1.0, accent);
    v.widgets.active.corner_radius   = cr;
    v.widgets.active.fg_stroke      = Stroke::new(2.0, Color32::from_rgb(15, 23, 42));

    v.widgets.open.bg_fill          = open_fill;
    v.widgets.open.bg_stroke        = Stroke::new(1.0, accent);
    v.widgets.open.corner_radius     = cr;
}
