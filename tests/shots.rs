//! Renders the launcher on a desktop and writes PNGs to /tmp/lumen-shots, so every preset can be looked at
//! without a phone. Run: cargo test --test shots -- --nocapture

use egui::{vec2, Event, PointerButton, Pos2};
use egui_kittest::Harness;
use lumen::{Config, Lumen, Preset};

const OUT: &str = "/tmp/lumen-shots";

fn make(preset: Preset, setup: impl Fn(&mut Config)) -> Harness<'static, Option<Lumen>> {
    make_sized(preset, setup, vec2(412.0, 915.0), 2.625)
}

fn make_sized(preset: Preset, setup: impl Fn(&mut Config), size: egui::Vec2, ppp: f32) -> Harness<'static, Option<Lumen>> {
    let dir = format!("/tmp/lumen-test-{:?}-{}", preset, std::process::id());
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let mut cfg = Config::default();
    cfg.apply_preset(preset);
    setup(&mut cfg);
    cfg.save(&dir);
    let mut h = Harness::builder()
        .with_size(size)
        .with_pixels_per_point(ppp)
        .with_context_setup(|ctx| lumen::install_fonts(ctx))
        .wgpu()
        .build_ui_state(
            move |ui, state: &mut Option<Lumen>| {
                let l = state.get_or_insert_with(|| Lumen::new(ui.ctx(), dir.clone()));
                paint_wallpaper(ui, preset);
                l.frame(ui);
            },
            None,
        );
    settle(&mut h);
    h
}

fn paint_wallpaper(ui: &egui::Ui, preset: Preset) {
    let r = ui.ctx().content_rect();
    let (a, b) = match preset {
        Preset::Nothing => (egui::Color32::from_rgb(38, 38, 40), egui::Color32::from_rgb(4, 4, 4)),
        Preset::Pixel => (egui::Color32::from_rgb(70, 110, 170), egui::Color32::from_rgb(20, 32, 70)),
        _ => (egui::Color32::from_rgb(250, 140, 80), egui::Color32::from_rgb(90, 50, 170)),
    };
    let mut m = egui::Mesh::default();
    m.colored_vertex(r.left_top(), a);
    m.colored_vertex(r.right_top(), a);
    m.colored_vertex(r.right_bottom(), b);
    m.colored_vertex(r.left_bottom(), b);
    m.add_triangle(0, 1, 2);
    m.add_triangle(0, 2, 3);
    ui.ctx().layer_painter(egui::LayerId::background()).add(egui::Shape::mesh(m));
}

fn settle(h: &mut Harness<'_, Option<Lumen>>) {
    for _ in 0..24 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(15));
    }
}

fn shot(h: &mut Harness<'_, Option<Lumen>>, name: &str) {
    std::fs::create_dir_all(OUT).unwrap();
    let img = h.render().expect("render");
    img.save(format!("{OUT}/{name}.png")).unwrap();
    println!("wrote {OUT}/{name}.png");
}

fn lumen<'a>(h: &'a mut Harness<'_, Option<Lumen>>) -> &'a mut Lumen {
    h.state_mut().as_mut().unwrap()
}

fn touch_down(h: &Harness<'_, Option<Lumen>>, p: Pos2) {
    h.event(Event::PointerMoved(p));
    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Default::default() });
}
fn touch_move(h: &Harness<'_, Option<Lumen>>, p: Pos2) {
    h.event(Event::PointerMoved(p));
}
fn touch_up(h: &Harness<'_, Option<Lumen>>, p: Pos2) {
    h.event(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Default::default() });
}

#[test]
fn home_screens() {
    for (p, name) in [(Preset::Nothing, "nothing"), (Preset::Pixel, "pixel"), (Preset::Ios, "ios")] {
        let mut h = make(p, |_| {});
        shot(&mut h, &format!("{name}-home"));
    }
}

#[test]
fn drawers_and_more() {
    for (p, name) in [(Preset::Nothing, "nothing"), (Preset::Pixel, "pixel")] {
        let mut h = make(p, |_| {});
        lumen(&mut h).drawer_open = true;
        settle(&mut h);
        shot(&mut h, &format!("{name}-drawer"));
    }
    let mut h = make(Preset::Ios, |_| {});
    lumen(&mut h).page_target = 1.0;
    settle(&mut h);
    shot(&mut h, "ios-library");
    lumen(&mut h).page_target = 0.0;
    lumen(&mut h).edit = true;
    settle(&mut h);
    shot(&mut h, "ios-edit");
}

#[test]
fn settings_pages() {
    for (p, name) in [(Preset::Nothing, "nothing"), (Preset::Pixel, "pixel"), (Preset::Ios, "ios")] {
        let mut h = make(p, |_| {});
        lumen(&mut h).open_settings();
        settle(&mut h);
        shot(&mut h, &format!("{name}-settings"));
    }
}

#[test]
fn drag_and_drop() {
    let mut h = make(Preset::Pixel, |_| {});
    // press and hold on an icon, move it, look at it mid-drag, drop it
    let start = Pos2::new(120.0, 250.0);
    touch_down(&h, start);
    for _ in 0..8 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    shot(&mut h, "pixel-lift");
    touch_move(&h, Pos2::new(250.0, 330.0));
    for _ in 0..4 {
        h.step();
    }
    touch_move(&h, Pos2::new(260.0, 335.0));
    h.step();
    shot(&mut h, "pixel-drag");
    touch_up(&h, Pos2::new(260.0, 335.0));
    settle(&mut h);
    shot(&mut h, "pixel-dropped");
}

#[test]
fn folders_menu_picker_search() {
    // merge two apps into a folder by dragging one onto the other and waiting
    let mut h = make(Preset::Ios, |_| {});
    let a = Pos2::new(60.0, 285.0); // Photos
    let b = Pos2::new(150.0, 285.0); // Maps
    touch_down(&h, a);
    for _ in 0..8 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    shot(&mut h, "ios-menu");
    touch_move(&h, Pos2::new(100.0, 290.0));
    h.step();
    touch_move(&h, b);
    for _ in 0..10 {
        h.step();
        std::thread::sleep(std::time::Duration::from_millis(60));
    }
    shot(&mut h, "ios-merge-hover");
    touch_up(&h, b);
    settle(&mut h);
    shot(&mut h, "ios-folder-made");
    // open the folder
    touch_down(&h, b);
    h.step();
    touch_up(&h, b);
    for _ in 0..10 {
        h.step();
    }
    shot(&mut h, "ios-folder-open");

    let mut h = make(Preset::Pixel, |_| {});
    lumen(&mut h).widget_picker = true;
    settle(&mut h);
    shot(&mut h, "pixel-widgets");

    let mut h = make(Preset::Ios, |_| {});
    lumen(&mut h).search_open = true;
    lumen(&mut h).query = "ma".into();
    settle(&mut h);
    shot(&mut h, "ios-search");

    let mut h = make(Preset::Pixel, |c| c.mode = lumen::Mode::Light);
    shot(&mut h, "pixel-light-home");
}

#[test]
fn settings_subpages() {
    use lumen::SettingsPage as P;
    for (page, name) in [(P::Motion, "motion"), (P::Layout, "layout"), (P::Behavior, "behavior"), (P::Drawer, "drawer")] {
        let mut h = make(Preset::Ios, |_| {});
        lumen(&mut h).open_settings();
        lumen(&mut h).settings_stack.push(page);
        settle(&mut h);
        shot(&mut h, &format!("settings-{name}"));
    }
}

static FRAME_NS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
static FRAMES: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Measures what one launcher frame costs on this machine. Run with --release.
#[test]
fn perf() {
    use std::sync::atomic::Ordering::Relaxed;
    for (p, name) in [(Preset::Pixel, "pixel"), (Preset::Ios, "ios"), (Preset::Nothing, "nothing")] {
        let dir = format!("/tmp/lumen-perf-{name}-{}", std::process::id());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = Config::default();
        cfg.apply_preset(p);
        cfg.save(&dir);
        let mut h = Harness::builder()
            .with_size(vec2(412.0, 915.0))
            .with_pixels_per_point(2.625)
            .with_context_setup(|ctx| lumen::install_fonts(ctx))
            .build_ui_state(
                move |ui, state: &mut Option<Lumen>| {
                    let t = std::time::Instant::now();
                    let l = state.get_or_insert_with(|| Lumen::new(ui.ctx(), dir.clone()));
                    l.frame(ui);
                    FRAME_NS.fetch_add(t.elapsed().as_nanos() as u64, Relaxed);
                    FRAMES.fetch_add(1, Relaxed);
                },
                None,
            );
        for _ in 0..30 {
            h.step();
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        for (label, setup) in [("home", 0), ("drawer", 1), ("drag", 2)] {
            if setup == 1 {
                lumen(&mut h).drawer_open = true;
                for _ in 0..30 { h.step(); }
            }
            if setup == 2 {
                lumen(&mut h).drawer_open = false;
                for _ in 0..30 { h.step(); }
                touch_down(&h, Pos2::new(120.0, 250.0));
                for _ in 0..10 { h.step(); std::thread::sleep(std::time::Duration::from_millis(50)); }
                touch_move(&h, Pos2::new(250.0, 330.0));
            }
            FRAME_NS.store(0, Relaxed);
            FRAMES.store(0, Relaxed);
            for _ in 0..120 {
                h.step();
            }
            let n = FRAMES.load(Relaxed).max(1);
            println!("{name} {label}: {:.2} ms/frame over {n} frames", FRAME_NS.load(Relaxed) as f64 / n as f64 / 1e6);
        }
    }
}

#[test]
fn transitions() {
    let mut h = make(Preset::Ios, |_| {});
    lumen(&mut h).open_settings();
    for _ in 0..2 {
        h.step();
    }
    shot(&mut h, "ios-settings-opening");
    let mut h = make(Preset::Ios, |_| {});
    lumen(&mut h).page_target = 1.0;
    for _ in 0..1 {
        h.step();
    }
    h.step();
    shot(&mut h, "ios-page-moving");
}

#[test]
fn small_and_odd_screens() {
    for (w, h, ppp, name) in [(360.0, 760.0, 2.0, "360x760"), (320.0, 570.0, 1.5, "320x570"), (411.0, 914.0, 1.75, "g22-like"), (800.0, 1280.0, 1.5, "tablet-ish")] {
        for (p, pn) in [(Preset::Pixel, "pixel"), (Preset::Ios, "ios")] {
            let mut h2 = make_sized(p, |_| {}, vec2(w, h), ppp);
            shot(&mut h2, &format!("size-{name}-{pn}"));
        }
    }
}
