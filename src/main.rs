mod app;
mod searcher;
mod sync_app;
mod sync_state;
mod sync_syncer;
mod sync_watcher;
mod theme;

fn main() {
    #[cfg(target_os = "macos")]
    if std::env::var("WGPU_BACKEND").is_err() {
        unsafe { std::env::set_var("WGPU_BACKEND", "metal,gl"); }
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rust Seek")
            .with_inner_size([900.0, 620.0])
            .with_min_inner_size([600.0, 420.0])
            .with_icon(make_icon()),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native("Rust Seek", options, Box::new(|cc| {
        theme::apply_style(&cc.egui_ctx);
        cc.egui_ctx.set_visuals(theme::dark_visuals());
        let mut fonts = egui::FontDefinitions::default();
        let cjk_bytes: &[u8] = include_bytes!("../assets/NotoSansSC-Regular.otf");
        fonts.font_data.insert("cjk".to_owned(), egui::FontData::from_static(cjk_bytes).into());
        fonts.families.entry(egui::FontFamily::Proportional).or_default().push("cjk".to_owned());
        fonts.families.entry(egui::FontFamily::Monospace).or_default().push("cjk".to_owned());
        cc.egui_ctx.set_fonts(fonts);
        Ok(Box::new(app::App::default()))
    })) {
        eprintln!("rust-seek failed to start: {e}");
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

fn make_icon() -> egui::IconData {
    const S: usize = 32;
    let mut px = vec![0u8; S * S * 4];

    let set = |px: &mut Vec<u8>, x: i32, y: i32, r: u8, g: u8, b: u8, a: u8| {
        if x >= 0 && y >= 0 && (x as usize) < S && (y as usize) < S {
            let i = (y as usize * S + x as usize) * 4;
            let fa = a as f32 / 255.0;
            px[i]   = (r as f32 * fa) as u8;
            px[i+1] = (g as f32 * fa) as u8;
            px[i+2] = (b as f32 * fa) as u8;
            px[i+3] = a;
        }
    };

    for y in 0..S as i32 {
        for x in 0..S as i32 {
            let dx = x - 13; let dy = y - 13;
            let dist2 = dx*dx + dy*dy;
            if dist2 <= 121 && dist2 >= 64 {
                let t = (dist2 - 64) as f32 / 57.0;
                set(&mut px, x, y, (220.0 + t*20.0) as u8, (80.0 - t*20.0) as u8, 40, 255);
            } else if dist2 < 64 {
                set(&mut px, x, y, 180, 210, 255, 60);
            }
        }
    }
    for t in 0..12i32 {
        for dx in -1i32..=1 { for dy in -1i32..=1 {
            set(&mut px, 21+t+dx, 21+t+dy, 180, 60, 20, 255);
        }}
    }
    for y in 5i32..9 { for x in 6i32..10 {
        let dx = x-13; let dy = y-13; let d2 = dx*dx+dy*dy;
        if d2 <= 121 && d2 >= 64 { set(&mut px, x, y, 255, 200, 180, 200); }
    }}

    egui::IconData { rgba: px, width: S as u32, height: S as u32 }
}
