//! Icons are loaded on two worker threads the moment they are first needed, masked to the chosen
//! shape in Rust, and kept as GPU textures. The UI thread never waits for Android.

use crate::android::{self, App};
use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions};
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

pub const ICON_PX: i32 = 168;

pub struct Icon {
    pub tex: TextureHandle,
    pub mono: bool,
}

struct Req {
    app: App,
    radius: f32,
    mono: bool,
    generation: u32,
}

struct Done {
    key: String,
    generation: u32,
    img: Option<(ColorImage, bool)>,
}

pub struct Icons {
    tex: HashMap<String, Icon>,
    pending: HashSet<String>,
    stale: HashSet<String>,
    tx: Sender<Req>,
    rx: Receiver<Done>,
    ctx: Context,
    radius: f32,
    mono: bool,
    generation: u32,
}

impl Icons {
    pub fn new(ctx: &Context, radius: f32, mono: bool) -> Icons {
        let (tx, rx_req) = channel::<Req>();
        let (tx_done, rx) = channel::<Done>();
        let rx_req = Arc::new(Mutex::new(rx_req));
        for _ in 0..2 {
            let (rx_req, tx_done, ctx) = (rx_req.clone(), tx_done.clone(), ctx.clone());
            std::thread::spawn(move || loop {
                let req = match rx_req.lock().map(|r| r.recv()) {
                    Ok(Ok(r)) => r,
                    _ => return,
                };
                let img = android::load_icon(&req.app, ICON_PX, req.mono).map(|raw| {
                    let mut rgba = raw.rgba;
                    if !raw.mono {
                        mask(&mut rgba, raw.size, req.radius);
                    }
                    (ColorImage::from_rgba_premultiplied([raw.size, raw.size], &rgba), raw.mono)
                });
                let _ = tx_done.send(Done { key: req.app.pkg.clone(), generation: req.generation, img });
                ctx.request_repaint();
            });
        }
        Icons { tex: HashMap::new(), pending: HashSet::new(), stale: HashSet::new(), tx, rx, ctx: ctx.clone(), radius, mono, generation: 0 }
    }

    /// Call once per frame: turns finished icons into textures.
    pub fn pump(&mut self) {
        while let Ok(d) = self.rx.try_recv() {
            if d.generation != self.generation {
                continue;
            }
            self.pending.remove(&d.key);
            self.stale.remove(&d.key);
            if let Some((img, mono)) = d.img {
                let tex = self.ctx.load_texture(format!("icon:{}", d.key), img, TextureOptions::LINEAR);
                self.tex.insert(d.key, Icon { tex, mono });
            }
        }
    }

    pub fn get(&mut self, app: &App) -> Option<&Icon> {
        if (!self.tex.contains_key(&app.pkg) || self.stale.contains(&app.pkg)) && self.pending.insert(app.pkg.clone()) {
            let _ = self.tx.send(Req { app: app.clone(), radius: self.radius, mono: self.mono, generation: self.generation });
        }
        self.tex.get(&app.pkg)
    }

    /// Shape or style changed: reload what is on screen, keep the old pictures until the new ones arrive.
    pub fn restyle(&mut self, radius: f32, mono: bool) {
        if (radius - self.radius).abs() < 0.001 && mono == self.mono {
            return;
        }
        self.radius = radius;
        self.mono = mono;
        self.generation += 1;
        self.pending.clear();
        self.stale = self.tex.keys().cloned().collect();
    }
}

/// Rounded square with corner radius `r` (0 = square, 1 = circle), anti-aliased.
fn mask(rgba: &mut [u8], size: usize, r: f32) {
    let r = r.clamp(0.0, 1.0);
    let half = size as f32 / 2.0;
    let rad = r * half;
    for y in 0..size {
        for x in 0..size {
            let (px, py) = ((x as f32 + 0.5 - half).abs(), (y as f32 + 0.5 - half).abs());
            let (qx, qy) = (px - (half - rad), py - (half - rad));
            let d = (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad;
            let a = (0.5 - d).clamp(0.0, 1.0);
            if a < 1.0 {
                let p = &mut rgba[(y * size + x) * 4..][..4];
                for c in p.iter_mut() {
                    *c = (*c as f32 * a) as u8;
                }
            }
        }
    }
}
