//! App icons, built to be cheap on weak phones:
//!  * they are loaded on background threads at low priority, nothing on the drawing thread waits for Android;
//!  * what Android hands over is cached on disk, so the next start needs no Java at all;
//!  * they live in a few big atlas textures, so a whole screen of icons is one or two draw calls;
//!  * their pixel size follows the screen density instead of a fixed large size.

use crate::config::Shape;
use crate::platform::{self, App};
use egui::{pos2, ColorImage, Context, Rect, TextureHandle, TextureId, TextureOptions};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

/// Bigger atlas pages hold more icons per texture, so a screen of icons needs fewer texture switches (draw calls)
const PAGE_BIG: usize = 2048;
const PAGE_SMALL: usize = 1024;

#[derive(Clone, Copy)]
pub struct Icon {
    pub tex: TextureId,
    pub uv: Rect,
    /// only the glyph, to be tinted and put on a plate
    pub mono: bool,
    /// one texel in uv units: the atlas keeps a transparent texel around every picture
    pub texel: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Style {
    pub shape: Shape,
    pub radius: f32,
    pub mono: bool,
    /// side of the icon bitmaps in pixels
    pub px: i32,
}

struct Req {
    app: App,
    style: Style,
    generation: u32,
}

struct Done {
    key: String,
    generation: u32,
    img: Option<(ColorImage, bool)>,
}

#[derive(Clone, Copy)]
struct Slot {
    page: usize,
    x: usize,
    y: usize,
    mono: bool,
}

pub struct Icons {
    pages: Vec<TextureHandle>,
    slots: HashMap<String, Slot>,
    next_slot: usize,
    pending: HashSet<String>,
    stale: HashSet<String>,
    /// apps whose icon Android could not give us: not asked again until the style changes
    failed: HashMap<String, u8>,
    queue: VecDeque<App>,
    /// the first two atlas cells hold a plain plate and a thin ring in the icon shape, so plates batch with icons
    plate_ready: bool,
    tx: Sender<Req>,
    rx: Receiver<Done>,
    ctx: Context,
    style: Style,
    generation: u32,
    /// side of one atlas page in pixels
    page_px: usize,
}

/// Rounds the icon pixel size to a multiple of 8, no bigger than needed for crisp drawing.
pub fn px_for(icon_dp: f32, ppp: f32) -> i32 {
    (((icon_dp * ppp * 1.12) / 8.0).ceil() as i32 * 8).clamp(64, 192)
}

fn hash(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

fn cache_file(dir: &std::path::Path, app: &App, px: i32, mono: bool) -> PathBuf {
    dir.join(hash(&format!("{}|{}|{}|{}|{}|v3", app.pkg, app.cls, app.stamp, px, mono)))
}

fn read_cache(path: &std::path::Path, px: i32) -> Option<platform::RawIcon> {
    let b = std::fs::read(path).ok()?;
    if b.len() < 2 || b[0] != 1 {
        return None;
    }
    let rgba = b[2..].to_vec();
    (rgba.len() == (px * px * 4) as usize).then_some(platform::RawIcon { size: px as usize, rgba, mono: b[1] == 1 })
}

fn write_cache(path: &std::path::Path, r: &platform::RawIcon) {
    let mut b = Vec::with_capacity(2 + r.rgba.len());
    b.push(1);
    b.push(u8::from(r.mono));
    b.extend_from_slice(&r.rgba);
    let tmp = path.with_extension("tmp");
    if std::fs::write(&tmp, b).is_ok() {
        let _ = std::fs::rename(tmp, path);
    }
}

impl Icons {
    pub fn new(ctx: &Context, style: Style, cache_dir: String, big_pages: bool) -> Icons {
        let dir = PathBuf::from(cache_dir).join("icon-cache");
        let _ = std::fs::create_dir_all(&dir);
        let (tx, rx_req) = channel::<Req>();
        let (tx_done, rx) = channel::<Done>();
        let rx_req = Arc::new(Mutex::new(rx_req));
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4);
        for _ in 0..if cores > 4 { 2 } else { 1 } {
            let (rx_req, tx_done, ctx, dir) = (rx_req.clone(), tx_done.clone(), ctx.clone(), dir.clone());
            std::thread::spawn(move || {
                // loading icons must never compete with drawing
                platform::background_thread();
                loop {
                    let req = match rx_req.lock().map(|r| r.recv()) {
                        Ok(Ok(r)) => r,
                        _ => return,
                    };
                    let path = cache_file(&dir, &req.app, req.style.px, req.style.mono);
                    let raw = std::panic::catch_unwind(|| {
                        read_cache(&path, req.style.px).or_else(|| {
                            let r = platform::load_icon(&req.app, req.style.px, req.style.mono)?;
                            write_cache(&path, &r);
                            Some(r)
                        })
                    })
                    .unwrap_or(None);
                    let img = raw.map(|raw| {
                        let mut rgba = raw.rgba;
                        if !raw.mono {
                            mask(&mut rgba, raw.size, req.style);
                        }
                        (ColorImage::from_rgba_premultiplied([raw.size, raw.size], &rgba), raw.mono)
                    });
                    let _ = tx_done.send(Done { key: req.app.pkg.clone(), generation: req.generation, img });
                    ctx.request_repaint();
                }
            });
        }
        Icons { pages: vec![], slots: HashMap::new(), next_slot: 2, plate_ready: false, pending: HashSet::new(), stale: HashSet::new(), failed: HashMap::new(), queue: VecDeque::new(), tx, rx, ctx: ctx.clone(), style, generation: 0, page_px: if big_pages { PAGE_BIG } else { PAGE_SMALL } }
    }

    fn cell(&self) -> usize {
        self.style.px as usize + 2
    }

    fn per_row(&self) -> usize {
        self.page_px / self.cell()
    }

    fn per_page(&self) -> usize {
        self.per_row() * self.per_row()
    }

    fn alloc(&mut self) -> (usize, usize, usize) {
        let idx = self.next_slot;
        self.next_slot += 1;
        let (pp, pr) = (self.per_page(), self.per_row());
        let (page, local) = (idx / pp, idx % pp);
        while self.pages.len() <= page {
            let n = self.pages.len();
            let blank = ColorImage::filled([self.page_px, self.page_px], egui::Color32::TRANSPARENT);
            self.pages.push(self.ctx.load_texture(format!("icons{n}"), blank, TextureOptions::LINEAR));
        }
        let c = self.cell();
        (page, (local % pr) * c, (local / pr) * c)
    }

    fn cell_pos(&mut self, idx: usize) -> (usize, usize, usize) {
        let (pp, pr) = (self.per_page(), self.per_row());
        let (page, local) = (idx / pp, idx % pp);
        while self.pages.len() <= page {
            let n = self.pages.len();
            let blank = ColorImage::filled([self.page_px, self.page_px], egui::Color32::TRANSPARENT);
            self.pages.push(self.ctx.load_texture(format!("icons{n}"), blank, TextureOptions::LINEAR));
        }
        let c = self.cell();
        (page, (local % pr) * c, (local / pr) * c)
    }

    fn make_plates(&mut self) {
        let size = self.style.px as usize;
        let half = size as f32 / 2.0;
        let mut fill = vec![0u8; size * size * 4];
        let mut ring = vec![0u8; size * size * 4];
        for y in 0..size {
            for x in 0..size {
                let d = distance(self.style, half, x as f32 + 0.5 - half, y as f32 + 0.5 - half);
                let a_fill = (0.5 - d).clamp(0.0, 1.0);
                let a_inner = (0.5 - (d + 1.6)).clamp(0.0, 1.0);
                let o = (y * size + x) * 4;
                let f = (a_fill * 255.0) as u8;
                fill[o..o + 4].copy_from_slice(&[f, f, f, f]);
                let r = ((a_fill - a_inner).max(0.0) * 255.0) as u8;
                ring[o..o + 4].copy_from_slice(&[r, r, r, r]);
            }
        }
        for (i, data) in [fill, ring].into_iter().enumerate() {
            let (page, x, y) = self.cell_pos(i);
            let img = ColorImage::from_rgba_premultiplied([size, size], &data);
            self.pages[page].set_partial([x + 1, y + 1], img, TextureOptions::LINEAR);
        }
        self.plate_ready = true;
    }

    /// (plate, ring) shapes to tint, once they are in the atlas.
    pub fn plates(&self) -> Option<(Icon, Icon)> {
        if !self.plate_ready || self.pages.is_empty() {
            return None;
        }
        let px = self.style.px as f32;
        let c = self.cell();
        let (pp, pr) = (self.per_page(), self.per_row());
        let at = |idx: usize| {
            let (page, local) = (idx / pp, idx % pp);
            let (x, y) = (((local % pr) * c + 1) as f32, ((local / pr) * c + 1) as f32);
            Icon { tex: self.pages[page].id(), uv: Rect::from_min_max(pos2(x / self.page_px as f32, y / self.page_px as f32), pos2((x + px) / self.page_px as f32, (y + px) / self.page_px as f32)), mono: false, texel: 1.0 / self.page_px as f32 }
        };
        Some((at(0), at(1)))
    }

    /// Call once per frame: puts finished icons into the atlas. A handful per frame keeps swipes smooth.
    pub fn pump(&mut self) {
        if !self.plate_ready {
            self.make_plates();
        }
        let mut uploaded = 0;
        while uploaded < 6 {
            let Ok(d) = self.rx.try_recv() else { return };
            if d.generation != self.generation {
                continue;
            }
            self.pending.remove(&d.key);
            self.stale.remove(&d.key);
            if d.img.is_none() {
                // a load can fail while Android is busy starting or stopping an app: try again a couple of times
                *self.failed.entry(d.key.clone()).or_insert(0) += 1;
            }
            if let Some((img, mono)) = d.img {
                let slot = match self.slots.get(&d.key) {
                    Some(s) => *s,
                    None => {
                        let (page, x, y) = self.alloc();
                        Slot { page, x, y, mono }
                    }
                };
                self.pages[slot.page].set_partial([slot.x + 1, slot.y + 1], img, TextureOptions::LINEAR);
                self.slots.insert(d.key, Slot { mono, ..slot });
                uploaded += 1;
            }
        }
        self.ctx.request_repaint();
    }

    pub fn get(&mut self, app: &App) -> Option<Icon> {
        if (!self.slots.contains_key(&app.pkg) || self.stale.contains(&app.pkg)) && self.failed.get(&app.pkg).copied().unwrap_or(0) < 3 && self.pending.insert(app.pkg.clone()) {
            let _ = self.tx.send(Req { app: app.clone(), style: self.style, generation: self.generation });
        }
        let s = self.slots.get(&app.pkg)?;
        let px = self.style.px as f32;
        let (x, y) = ((s.x + 1) as f32, (s.y + 1) as f32);
        let uv = Rect::from_min_max(pos2(x / self.page_px as f32, y / self.page_px as f32), pos2((x + px) / self.page_px as f32, (y + px) / self.page_px as f32));
        Some(Icon { tex: self.pages[s.page].id(), uv, mono: s.mono, texel: 1.0 / self.page_px as f32 })
    }

    /// Queue every app for loading, a few at a time and only while nothing is moving.
    pub fn prefetch_all(&mut self, apps: &[App]) {
        self.queue = apps.iter().filter(|a| !self.slots.contains_key(&a.pkg)).cloned().collect();
    }

    pub fn prefetch_step(&mut self, idle: bool) {
        if !idle {
            return;
        }
        let mut n = 0;
        while self.pending.len() < 3 && n < 2 {
            let Some(a) = self.queue.pop_front() else { break };
            self.get(&a);
            n += 1;
        }
        if !self.queue.is_empty() {
            self.ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
    }

    /// Shape, color style or size changed: reload what is on screen, keep the old pictures until the new ones arrive.
    pub fn restyle(&mut self, style: Style) {
        if style == self.style {
            return;
        }
        if style.px != self.style.px {
            // different bitmap size: the atlas layout changes, start a new one
            self.pages.clear();
            self.slots.clear();
            self.next_slot = 2;
        } else {
            self.stale = self.slots.keys().cloned().collect();
        }
        self.plate_ready = false;
        self.style = style;
        self.generation += 1;
        self.pending.clear();
        self.failed.clear();
    }

    /// True when Android could not produce this app's icon (a letter is drawn instead).
    pub fn failed(&self, pkg: &str) -> bool {
        self.failed.get(pkg).copied().unwrap_or(0) >= 3
    }

    /// Back from an app: give every icon that failed another chance.
    pub fn retry_failed(&mut self) {
        self.failed.clear();
    }

    pub fn stats(&self) -> (usize, usize, i32) {
        (self.pages.len(), self.slots.len(), self.style.px)
    }

    pub fn pending_count(&self) -> usize {
        self.pending.len()
    }
}

/// Signed distance (in pixels, negative inside) from the edge of the icon shape. x and y are pixel offsets from the center.
fn distance(style: Style, half: f32, x: f32, y: f32) -> f32 {
    let (px, py) = (x.abs(), y.abs());
    match style.shape {
        Shape::Circle => (px * px + py * py).sqrt() - half,
        Shape::Squircle => {
            // iOS style continuous corner: a superellipse of order 5
            let n = 5.0f32;
            let r = ((px / half).powf(n) + (py / half).powf(n)).powf(1.0 / n);
            (r - 1.0) * half
        }
        Shape::Rounded | Shape::Square => {
            let rad = if style.shape == Shape::Square { half * 0.12 } else { style.radius.clamp(0.0, 1.0) * half };
            let (qx, qy) = (px - (half - rad), py - (half - rad));
            (qx.max(0.0).powi(2) + qy.max(0.0).powi(2)).sqrt() + qx.max(qy).min(0.0) - rad
        }
    }
}

/// Cuts the icon to the shape, anti-aliased.
fn mask(rgba: &mut [u8], size: usize, style: Style) {
    let half = size as f32 / 2.0;
    for y in 0..size {
        for x in 0..size {
            let d = distance(style, half, x as f32 + 0.5 - half, y as f32 + 0.5 - half);
            let a = (0.5 - d).clamp(0.0, 1.0);
            if a < 1.0 {
                for c in rgba[(y * size + x) * 4..][..4].iter_mut() {
                    *c = (*c as f32 * a) as u8;
                }
            }
        }
    }
}
