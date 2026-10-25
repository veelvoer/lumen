#![allow(deprecated)]

mod anim;
mod appwidgets;
mod app;
mod bridge;
mod cards;
mod clock;
mod config;
mod drawer;
mod fonts;
mod glass;
mod home;
mod icons;
mod menu;
mod notes;
mod perf;
mod phosphor;
mod scroll;
mod platform;
mod settings;
mod theme;
mod update;
mod widgets;

pub use app::Lumen;
pub use config::{Config, Mode, Preset};
pub use platform::files_dir;
pub use settings::SettingsPage;

#[cfg(target_os = "android")]
mod entry {
    use super::*;

    struct Root(Lumen);

    impl eframe::App for Root {
        fn clear_color(&self, _: &egui::Visuals) -> [f32; 4] {
            [0.0, 0.0, 0.0, 0.0] // see-through, so the system wallpaper shows
        }

        fn ui(&mut self, ui: &mut egui::Ui, frame: &mut eframe::Frame) {
            if let Some(c) = frame.info().cpu_usage {
                self.0.note_cpu(c);
            }
            // a bug must never take the home screen down with it: skip the frame, reset what was in flight
            let l = &mut self.0;
            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| l.frame(ui))).is_err() {
                l.recover(ui.ctx());
            } else {
                l.panics = 0;
            }
        }
    }

    fn now_secs() -> u64 {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
    }

    #[unsafe(no_mangle)]
    fn android_main(app: android_activity::AndroidApp) {
        android_logger::init_once(android_logger::Config::default().with_max_level(log::LevelFilter::Info).with_tag("lumen"));
        platform::init(&app);
        let crash_dir = platform::files_dir();
        let _ = app::FILES_DIR.set(crash_dir.clone());
        std::panic::set_hook(Box::new(move |i| {
            log::error!("panic: {i}");
            let bt = std::backtrace::Backtrace::force_capture().to_string();
            let text = format!("rust panic: {i}\n{}", &bt[..bt.len().min(1200)]);
            let _ = std::fs::write(std::path::Path::new(&crash_dir).join("last-crash.txt"), text);
        }));

        let options = eframe::NativeOptions {
            android_app: Some(app),
            viewport: egui::ViewportBuilder::default().with_transparent(true),
            ..Default::default()
        };
        let dir = platform::files_dir();
        let r = eframe::run_native(
            "Lumen",
            options,
            Box::new(move |cc| {
                fonts::install(&cc.egui_ctx, 0);
                let mut l = Lumen::new(&cc.egui_ctx, dir);
                if let Some(gl) = cc.gl.as_ref() {
                    use eframe::glow::HasContext;
                    let renderer = unsafe { gl.get_parameter_string(eframe::glow::RENDERER) };
                    let version = unsafe { gl.get_parameter_string(eframe::glow::VERSION) };
                    log::info!("gl: {renderer} / {version}");
                    l.set_gpu(&cc.egui_ctx, &format!("{renderer} ({version})"));
                }
                if l.cfg.auto_update && l.cfg.last_check + 6 * 3600 < now_secs() {
                    let ctx = cc.egui_ctx.clone();
                    l.upd.check(move || ctx.request_repaint());
                    l.cfg.last_check = now_secs();
                    l.dirty = true;
                }
                Ok(Box::new(Root(l)))
            }),
        );
        if let Err(e) = r {
            log::error!("exit: {e}");
        }
    }
}

/// For tests and screenshots on a desktop.
pub fn install_fonts(ctx: &egui::Context) {
    fonts::install(ctx, 0);
}
