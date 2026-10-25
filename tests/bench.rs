//! How much CPU one frame costs, split into "our code + egui layout" and "tessellation", without any GPU.
//! Run: cargo test --release --test bench -- --nocapture

use egui::{pos2, vec2, Context, Event, PointerButton, RawInput, Rect};
use lumen::{Config, Lumen, Preset};
use std::time::Instant;

struct Rig {
    ctx: Context,
    l: Lumen,
    t: f64,
    run_ms: f64,
    tess_ms: f64,
    frames: u32,
    shapes: usize,
    verts: usize,
}

impl Rig {
    fn new(preset: Preset) -> Rig {
        let dir = format!("/tmp/lumen-bench-{:?}-{}", preset, std::process::id());
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let mut cfg = Config::default();
        cfg.apply_preset(preset);
        cfg.save(&dir);
        let ctx = Context::default();
        lumen::install_fonts(&ctx);
        let l = Lumen::new(&ctx, dir);
        let mut r = Rig { ctx, l, t: 0.0, run_ms: 0.0, tess_ms: 0.0, frames: 0, shapes: 0, verts: 0 };
        for _ in 0..40 {
            r.frame(vec![]);
            std::thread::sleep(std::time::Duration::from_millis(8));
        }
        r.reset();
        r
    }

    fn reset(&mut self) {
        self.run_ms = 0.0;
        self.tess_ms = 0.0;
        self.frames = 0;
        self.shapes = 0;
        self.verts = 0;
    }

    fn frame(&mut self, events: Vec<Event>) {
        self.t += 1.0 / 60.0;
        let mut input = RawInput::default();
        input.screen_rect = Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(412.0, 915.0)));
        input.time = Some(self.t);
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(2.625);
        input.events = events;
        let t0 = Instant::now();
        let l = &mut self.l;
        let out = self.ctx.run_ui(input, |ui| l.frame(ui));
        let t1 = Instant::now();
        let prims = self.ctx.tessellate(out.shapes, out.pixels_per_point);
        let t2 = Instant::now();
        self.run_ms += (t1 - t0).as_secs_f64() * 1000.0;
        self.tess_ms += (t2 - t1).as_secs_f64() * 1000.0;
        self.frames += 1;
        self.shapes += prims.len();
        self.verts += prims.iter().map(|p| match &p.primitive { egui::epaint::Primitive::Mesh(m) => m.vertices.len(), _ => 0 }).sum::<usize>();
    }

    fn report(&self, what: &str) {
        let n = self.frames.max(1) as f64;
        println!("{what:28} run {:.2} ms  tess {:.2} ms  total {:.2} ms   ({} meshes, {} vertices per frame)", self.run_ms / n, self.tess_ms / n, (self.run_ms + self.tess_ms) / n, self.shapes / self.frames.max(1) as usize, self.verts / self.frames.max(1) as usize);
    }
}

fn ptr(pos: egui::Pos2, down: bool) -> Vec<Event> {
    vec![Event::PointerMoved(pos), Event::PointerButton { pos, button: PointerButton::Primary, pressed: down, modifiers: Default::default() }]
}

#[test]
fn bench() {
    for (p, name) in [(Preset::Pixel, "pixel"), (Preset::Ios, "ios"), (Preset::Nothing, "nothing")] {
        let mut r = Rig::new(p);
        for _ in 0..120 {
            r.frame(vec![]);
        }
        r.report(&format!("{name} home (still)"));
        r.reset();

        // swipe the pager: press, move left a bit each frame
        r.frame(ptr(pos2(300.0, 400.0), true));
        for i in 0..60 {
            r.frame(vec![Event::PointerMoved(pos2(300.0 - i as f32 * 4.0, 400.0))]);
        }
        r.report(&format!("{name} page drag"));
        r.frame(ptr(pos2(60.0, 400.0), false));
        for _ in 0..40 {
            r.frame(vec![]);
        }
        r.reset();

        r.l.drawer_open = true;
        for _ in 0..90 {
            r.frame(vec![]);
        }
        r.report(&format!("{name} drawer open"));
        r.reset();
        // scroll the drawer
        r.frame(ptr(pos2(200.0, 600.0), true));
        for i in 0..40 {
            r.frame(vec![Event::PointerMoved(pos2(200.0, 600.0 - i as f32 * 8.0))]);
        }
        r.report(&format!("{name} drawer scroll"));
        println!();
    }
}
