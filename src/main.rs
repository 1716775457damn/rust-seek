mod app;
mod searcher;

fn main() {
    // On macOS, if the default Metal/wgpu backend fails (e.g. older Intel Mac,
    // Rosetta), fall back to OpenGL so the window actually appears.
    #[cfg(target_os = "macos")]
    if std::env::var("WGPU_BACKEND").is_err() {
        // Try Metal first; wgpu will fall back automatically on failure.
        // Setting this env var makes the fallback explicit and debuggable.
        unsafe { std::env::set_var("WGPU_BACKEND", "metal,gl"); }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rust Seek")
            .with_inner_size([900.0, 600.0])
            .with_min_inner_size([600.0, 400.0])
            .with_icon(make_icon()),
        ..Default::default()
    };
    if let Err(e) = eframe::run_native("Rust Seek", options, Box::new(|cc| {
        let mut fonts = egui::FontDefinitions::default();
        let cjk_bytes: &[u8] = include_bytes!("../assets/NotoSansSC-Regular.otf");
        fonts.font_data.insert(
            "cjk".to_owned(),
            egui::FontData::from_static(cjk_bytes).into(),
        );
        fonts.families
            .entry(egui::FontFamily::Proportional)
            .or_default()
            .push("cjk".to_owned());
        fonts.families
            .entry(egui::FontFamily::Monospace)
            .or_default()
            .push("cjk".to_owned());
        cc.egui_ctx.set_fonts(fonts);
        Ok(Box::new(app::App::default()))
    })) {
        eprintln!("rust-seek failed to start: {e}");
        // Show a visible error dialog on macOS instead of silently exiting
        #[cfg(target_os = "macos")]
        {
            let msg = format!("rust-seek failed to start:\n{e}");
            let _ = std::process::Command::new("osascript")
                .args(["-e", &format!("display alert \"Rust Seek\" message \"{msg}\"")])
                .spawn();
        }
        std::process::exit(1);
    }
}

/// 32×32 RGBA magnifying glass icon
fn make_icon() -> egui::IconData {
    const S: usize = 32;
    let mut px = vec![0u8; S * S * 4];

    let set = |px: &mut Vec<u8>, x: i32, y: i32, r: u8, g: u8, b: u8, a: u8| {
        if x >= 0 && y >= 0 && (x as usize) < S && (y as usize) < S {
            let i = (y as usize * S + x as usize) * 4;
            // Alpha-blend onto transparent background
            let fa = a as f32 / 255.0;
            px[i]   = (r as f32 * fa) as u8;
            px[i+1] = (g as f32 * fa) as u8;
            px[i+2] = (b as f32 * fa) as u8;
            px[i+3] = a;
        }
    };

    // Draw circle ring (magnifying glass lens): center (13,13), outer r=11, inner r=8
    for y in 0..S as i32 {
        for x in 0..S as i32 {
            let dx = x - 13;
            let dy = y - 13;
            let dist2 = dx * dx + dy * dy;
            if dist2 <= 121 && dist2 >= 64 {
                // Ring: orange-red gradient
                let t = (dist2 - 64) as f32 / (121.0 - 64.0);
                let r = (220.0 + t * 20.0) as u8;
                let g = (80.0 - t * 20.0) as u8;
                let b = (40.0) as u8;
                set(&mut px, x, y, r, g, b, 255);
            } else if dist2 < 64 {
                // Lens interior: light blue tint
                let alpha = 60u8;
                set(&mut px, x, y, 180, 210, 255, alpha);
            }
        }
    }

    // Handle: thick diagonal line from (21,21) to (29,29), width 3
    for t in 0..12i32 {
        let x = 21 + t;
        let y = 21 + t;
        for dx in -1i32..=1 {
            for dy in -1i32..=1 {
                set(&mut px, x + dx, y + dy, 180, 60, 20, 255);
            }
        }
    }

    // Highlight glint on lens (top-left arc)
    for y in 5i32..9 {
        for x in 6i32..10 {
            let dx = x - 13;
            let dy = y - 13;
            let dist2 = dx * dx + dy * dy;
            if dist2 <= 121 && dist2 >= 64 {
                set(&mut px, x, y, 255, 200, 180, 200);
            }
        }
    }

    egui::IconData { rgba: px, width: S as u32, height: S as u32 }
}
