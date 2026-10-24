#![allow(deprecated)]
mod android;
mod app;
mod config;
mod icons;
mod settings;
mod theme;
mod update;
mod widgets;

use eframe::egui;

struct Root(app::Lumen);

impl eframe::App for Root {
    fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // see-through, so the system wallpaper shows
    }

    fn ui(&mut self, ui: &mut egui::Ui, _: &mut eframe::Frame) {
        self.0.frame(ui);
    }
}

/// Uses the phone's own fonts when they exist (they cover every language), egui's built-in ones otherwise.
fn fonts(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let candidates = [
        "/system/fonts/GoogleSans-Regular.ttf",
        "/system/fonts/Roboto-Regular.ttf",
        "/system/fonts/RobotoStatic-Regular.ttf",
        "/system/fonts/Roboto[wdth,wght].ttf",
    ];
    let fallbacks = ["/system/fonts/NotoSansCJK-Regular.ttc", "/system/fonts/NotoSansSC-Regular.otf", "/system/fonts/NotoSansArabic-Regular.ttf", "/system/fonts/NotoSansDevanagari-Regular.ttf"];
    if let Some(data) = candidates.iter().find_map(|p| std::fs::read(p).ok()) {
        defs.font_data.insert("system".into(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        defs.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "system".into());
    }
    for (i, p) in fallbacks.iter().enumerate() {
        if let Ok(data) = std::fs::read(p) {
            let name = format!("fallback{i}");
            defs.font_data.insert(name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
            defs.families.entry(egui::FontFamily::Proportional).or_default().push(name);
        }
    }
    ctx.set_fonts(defs);
}

#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("lumen"));
    std::panic::set_hook(Box::new(|i| log::error!("panic: {i}")));
    android::init(&app);

    let options = eframe::NativeOptions {
        android_app: Some(app),
        viewport: egui::ViewportBuilder::default().with_transparent(true),
        ..Default::default()
    };
    let dir = android::files_dir();
    let r = eframe::run_native(
        "Lumen",
        options,
        Box::new(move |cc| {
            fonts(&cc.egui_ctx);
            let l = app::Lumen::new(&cc.egui_ctx, dir);
            if l.cfg.auto_update && l.cfg.last_check + 6 * 3600 < now_secs() {
                let ctx = cc.egui_ctx.clone();
                l.upd.check(move || ctx.request_repaint());
            }
            let mut root = Root(l);
            root.0.cfg.last_check = now_secs();
            root.0.dirty = true;
            Ok(Box::new(root))
        }),
    );
    if let Err(e) = r {
        log::error!("exit: {e}");
    }
}

fn now_secs() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}
