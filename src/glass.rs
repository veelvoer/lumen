//! Liquid Glass, as close as a launcher can get without being able to read the screen behind it.
//!
//! Apple's glass is a lens: nearly clear in the middle, and along the rim it bends the background
//! (stronger the closer to the edge), splits it into colors a little, and catches light from one side.
//! We cannot read what is drawn behind our window, but with the user's permission we can read the wallpaper
//! itself. Each glass panel is then "baked" once into a small texture: the blurred wallpaper under it, refracted
//! along the rim with a tiny color split, lit by a Blinn-Phong highlight on the curved edge. The bake only
//! reruns when the panel or the wallpaper position under it changes.

use crate::platform;
use egui::{pos2, vec2, Color32, ColorImage, Context, Rect, TextureHandle, TextureId, TextureOptions};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};

/// A wallpaper copy small enough to sample on the CPU, sharp and blurred.
pub struct Small {
    pub w: usize,
    pub h: usize,
    pub sharp: Vec<u8>, // RGB
    pub blur: Vec<u8>,  // RGB
}

fn box_down(src: &[u8], sw: usize, sh: usize, dw: usize, dh: usize) -> Vec<u8> {
    let mut out = vec![0u8; dw * dh * 3];
    for y in 0..dh {
        let (y0, y1) = (y * sh / dh, ((y + 1) * sh / dh).max(y * sh / dh + 1).min(sh));
        for x in 0..dw {
            let (x0, x1) = (x * sw / dw, ((x + 1) * sw / dw).max(x * sw / dw + 1).min(sw));
            let (mut r, mut g, mut b, mut n) = (0u32, 0u32, 0u32, 0u32);
            for yy in y0..y1 {
                for xx in x0..x1 {
                    let o = (yy * sw + xx) * 4;
                    // the picture comes premultiplied; wallpapers are opaque so this is the color
                    r += src[o] as u32;
                    g += src[o + 1] as u32;
                    b += src[o + 2] as u32;
                    n += 1;
                }
            }
            let o = (y * dw + x) * 3;
            out[o] = (r / n.max(1)) as u8;
            out[o + 1] = (g / n.max(1)) as u8;
            out[o + 2] = (b / n.max(1)) as u8;
        }
    }
    out
}

fn blur_rgb(src: &[u8], w: usize, h: usize, radius: usize, passes: usize) -> Vec<u8> {
    let mut a = src.to_vec();
    let mut b = src.to_vec();
    for _ in 0..passes {
        // horizontal
        for y in 0..h {
            for c in 0..3 {
                let mut sum = 0u32;
                let mut cnt = 0u32;
                for x in 0..=radius.min(w - 1) {
                    sum += a[(y * w + x) * 3 + c] as u32;
                    cnt += 1;
                }
                for x in 0..w {
                    b[(y * w + x) * 3 + c] = (sum / cnt) as u8;
                    let add = x + radius + 1;
                    if add < w {
                        sum += a[(y * w + add) * 3 + c] as u32;
                        cnt += 1;
                    }
                    if x >= radius {
                        sum -= a[(y * w + x - radius) * 3 + c] as u32;
                        cnt -= 1;
                    }
                }
            }
        }
        // vertical
        for x in 0..w {
            for c in 0..3 {
                let mut sum = 0u32;
                let mut cnt = 0u32;
                for y in 0..=radius.min(h - 1) {
                    sum += b[(y * w + x) * 3 + c] as u32;
                    cnt += 1;
                }
                for y in 0..h {
                    a[(y * w + x) * 3 + c] = (sum / cnt) as u8;
                    let add = y + radius + 1;
                    if add < h {
                        sum += b[(add * w + x) * 3 + c] as u32;
                        cnt += 1;
                    }
                    if y >= radius {
                        sum -= b[((y - radius) * w + x) * 3 + c] as u32;
                        cnt -= 1;
                    }
                }
            }
        }
    }
    a
}

pub fn make_small(rgba: &[u8], w: usize, h: usize) -> Small {
    let sw = 160usize;
    let sh = ((sw * h) / w.max(1)).clamp(40, 480);
    let sharp = box_down(rgba, w, h, sw, sh);
    let blur = blur_rgb(&sharp, sw, sh, 7, 3);
    Small { w: sw, h: sh, sharp, blur }
}

fn sample(img: &[u8], w: usize, h: usize, u: f32, v: f32) -> [f32; 3] {
    let x = (u * w as f32 - 0.5).clamp(0.0, w as f32 - 1.001);
    let y = (v * h as f32 - 0.5).clamp(0.0, h as f32 - 1.001);
    let (x0, y0) = (x as usize, y as usize);
    let (fx, fy) = (x - x0 as f32, y - y0 as f32);
    let px = |xx: usize, yy: usize, c: usize| img[(yy * w + xx) * 3 + c] as f32;
    let mut out = [0.0; 3];
    for c in 0..3 {
        let top = px(x0, y0, c) * (1.0 - fx) + px(x0 + 1, y0, c) * fx;
        let bot = px(x0, y0 + 1, c) * (1.0 - fx) + px(x0 + 1, y0 + 1, c) * fx;
        out[c] = top * (1.0 - fy) + bot * fy;
    }
    out
}

/// Where in the wallpaper (0..1) the screen looks, for a wallpaper of this shape. `pan` is 0..1 across the pages.
pub fn uv_window(wall_aspect: f32, screen: Rect, pan: f32) -> Rect {
    let sa = screen.width() / screen.height();
    if wall_aspect > sa {
        let vw = sa / wall_aspect;
        let u0 = (1.0 - vw) * pan.clamp(0.0, 1.0);
        Rect::from_min_max(pos2(u0, 0.0), pos2(u0 + vw, 1.0))
    } else {
        let vh = wall_aspect / sa;
        let v0 = (1.0 - vh) * 0.5;
        Rect::from_min_max(pos2(0.0, v0), pos2(1.0, v0 + vh))
    }
}

struct Baked {
    tex: TextureHandle,
    key: (i32, i32, i32, i32, i32, u32),
    uv: Rect,
}

thread_local! {
    static TINT: std::cell::Cell<[f32; 3]> = const { std::cell::Cell::new([190.0, 190.0, 210.0]) };
    static READY: RefCell<HashMap<&'static str, (TextureId, Rect)>> = RefCell::new(HashMap::new());
}

/// The wallpaper's main colors (from Android, no permission needed), so frosted panels pick up its hue.
pub fn set_colors(cs: &[[u8; 3]]) {
    if cs.is_empty() {
        return;
    }
    let (a, b) = (cs[0], cs.get(1).copied().unwrap_or(cs[0]));
    let mix = |i: usize| a[i] as f32 * 0.65 + b[i] as f32 * 0.35;
    TINT.with(|t| t.set([mix(0), mix(1), mix(2)]));
}

/// The veil color of a frosted panel: the wallpaper's hue, pulled toward white.
pub fn veil(light: bool) -> [f32; 3] {
    let t = TINT.with(|t| t.get());
    let w = if light { 0.72 } else { 0.55 };
    [t[0] + (255.0 - t[0]) * w, t[1] + (255.0 - t[1]) * w, t[2] + (255.0 - t[2]) * w]
}

/// The baked texture for a named panel, if there is one (the painting code asks for it by name).
pub fn lookup(name: &str) -> Option<(TextureId, Rect)> {
    READY.with(|r| r.borrow().get(name).copied())
}

pub fn clear_ready() {
    READY.with(|r| r.borrow_mut().clear());
}

#[derive(Default)]
pub struct Glass {
    pub small: Option<Small>,
    pub aspect: f32,
    rx: Option<Receiver<(Small, f32)>>,
    cache: HashMap<&'static str, Baked>,
    asked: bool,
    pub status: String,
}

impl Glass {
    /// Reads the wallpaper once, off the drawing thread.
    pub fn start(&mut self, ctx: &Context) {
        if self.asked {
            return;
        }
        self.asked = true;
        let (tx, rx) = channel();
        self.rx = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            platform::background_thread();
            match platform::wallpaper_image(1024) {
                Ok((w, h, rgba)) => {
                    let _ = tx.send((make_small(&rgba, w, h), w as f32 / h as f32));
                }
                Err(e) => log::info!("wallpaper not readable: {e}"),
            }
            ctx.request_repaint();
        });
    }

    pub fn poll(&mut self) {
        if let Some(rx) = &self.rx {
            if let Ok((s, a)) = rx.try_recv() {
                self.small = Some(s);
                self.aspect = a;
                self.cache.clear();
                self.status = "on".into();
            }
        }
    }

    pub fn active(&self) -> bool {
        self.small.is_some()
    }

    pub fn reset(&mut self) {
        self.small = None;
        self.rx = None;
        self.asked = false;
        self.cache.clear();
        clear_ready();
    }

    /// Bakes (or reuses) the glass for `rect` (points). `tint` is mixed over the backdrop, `dark` picks the light direction's strength.
    pub fn bake(&mut self, ctx: &Context, name: &'static str, rect: Rect, radius: f32, screen: Rect, pan: f32, tint: Color32, dark: bool) {
        let Some(sm) = &self.small else { return };
        let uvw = uv_window(self.aspect, screen, pan);
        let ppp = ctx.pixels_per_point();
        let s = (ppp * 0.42).clamp(0.9, 1.6);
        let (w, h) = (((rect.width() * s).round() as usize).max(8), ((rect.height() * s).round() as usize).max(8));
        // rerun only when something visible changed: size, shape, or the wallpaper shifted by about a pixel
        let key = (w as i32, h as i32, (radius * 4.0) as i32, (uvw.min.x * sm.w as f32 * 3.0 * 1.0).round() as i32, (uvw.min.y * sm.h as f32 * 3.0).round() as i32, tint.to_array().iter().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(*b as u32)) ^ u32::from(dark) ^ (rect.min.x as u32).wrapping_mul(7) ^ (rect.min.y as u32).wrapping_mul(13));
        if let Some(b) = self.cache.get(name) {
            if b.key == key {
                let (id, uv) = (b.tex.id(), b.uv);
                READY.with(|r| r.borrow_mut().insert(name, (id, uv)));
                return;
            }
        }
        let img = render(sm, w, h, rect, radius, screen, uvw, s, tint, dark);
        let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
        let tex = match self.cache.remove(name) {
            Some(mut b) if b.tex.size() == [w, h] => {
                b.tex.set(img, TextureOptions::LINEAR);
                b.key = key;
                b.tex
            }
            _ => ctx.load_texture(format!("glass:{name}"), img, TextureOptions::LINEAR),
        };
        let id = tex.id();
        self.cache.insert(name, Baked { tex, key, uv });
        READY.with(|r| r.borrow_mut().insert(name, (id, uv)));
    }
}

#[allow(clippy::too_many_arguments)]
fn render(sm: &Small, w: usize, h: usize, rect: Rect, radius: f32, screen: Rect, uvw: Rect, s: f32, tint: Color32, dark: bool) -> ColorImage {
    let mut px = vec![Color32::TRANSPARENT; w * h];
    let half = vec2(rect.width(), rect.height()) / 2.0;
    let r = radius.min(half.x.min(half.y));
    let inner = half - vec2(r, r);
    let band = (half.x.min(half.y) * 0.7).min(30.0).max(7.0);
    let light = vec2(-0.5, -0.86).normalized();
    let tint = [tint.r() as f32, tint.g() as f32, tint.b() as f32, tint.a() as f32 / 255.0];
    let to_uv = |p: egui::Pos2| ((uvw.min.x + (p.x - screen.min.x) / screen.width() * uvw.width()), (uvw.min.y + (p.y - screen.min.y) / screen.height() * uvw.height()));
    for y in 0..h {
        for x in 0..w {
            // point in screen points, relative to the panel center
            let lp = vec2((x as f32 + 0.5) / s - half.x, (y as f32 + 0.5) / s - half.y);
            let q = vec2(lp.x.abs() - inner.x, lp.y.abs() - inner.y);
            let outside = vec2(q.x.max(0.0), q.y.max(0.0));
            let d = outside.length() + q.x.max(q.y).min(0.0) - r; // negative inside, in points
            let cover = (0.5 - d * s).clamp(0.0, 1.0);
            if cover <= 0.0 {
                continue;
            }
            // outward normal of the shape
            let n = if outside.length() > 0.0 {
                vec2(outside.x * lp.x.signum(), outside.y * lp.y.signum()).normalized()
            } else if q.x > q.y {
                vec2(lp.x.signum(), 0.0)
            } else {
                vec2(0.0, lp.y.signum())
            };
            let t = (-d / band).clamp(0.0, 1.0); // 0 on the rim, 1 deep inside
            let edge = (1.0 - t).powf(2.2);
            let bend = edge * band * 0.18; // frosted: only a hint of lensing at the rim // how far the picture is pulled in, in points
            let center = rect.center();
            let at = |k: f32, img: &[u8]| {
                let p = center + lp + n * bend * k;
                let (u, v) = to_uv(p);
                sample(img, sm.w, sm.h, u, v)
            };
            let mix = 0.94;
            let (mut cr, mut cg, mut cb);
            if edge > 0.02 {
                let (a, b, c) = (at(1.07, &sm.sharp), at(1.0, &sm.sharp), at(0.93, &sm.sharp));
                let (ab, bb, cbb) = (at(1.07, &sm.blur), at(1.0, &sm.blur), at(0.93, &sm.blur));
                cr = a[0] * (1.0 - mix) + ab[0] * mix;
                cg = b[1] * (1.0 - mix) + bb[1] * mix;
                cb = c[2] * (1.0 - mix) + cbb[2] * mix;
            } else {
                let b = at(1.0, &sm.sharp);
                let bb = at(1.0, &sm.blur);
                cr = b[0] * (1.0 - mix) + bb[0] * mix;
                cg = b[1] * (1.0 - mix) + bb[1] * mix;
                cb = b[2] * (1.0 - mix) + bb[2] * mix;
            }
            // a touch more color, a touch lighter (glass gathers light)
            let lum = cr * 0.3 + cg * 0.59 + cb * 0.11;
            let sat = 1.35;
            cr = lum + (cr - lum) * sat;
            cg = lum + (cg - lum) * sat;
            cb = lum + (cb - lum) * sat;
            let top = 1.0 - ((y as f32 + 0.5) / h as f32);
            let lift = (if dark { 14.0 } else { 20.0 }) + 12.0 * top * top;
            cr += lift;
            cg += lift;
            cb += lift;
            // tint
            cr = cr * (1.0 - tint[3]) + tint[0] * tint[3];
            cg = cg * (1.0 - tint[3]) + tint[1] * tint[3];
            cb = cb * (1.0 - tint[3]) + tint[2] * tint[3];
            // light on the curved rim: a bright crescent where the edge faces the light, a faint one opposite
            let facing = n.x * light.x + n.y * light.y;
            let spec = facing.max(0.0).powf(3.0) * edge.powf(1.2) * 70.0 + (-facing).max(0.0).powf(3.0) * edge.powf(1.2) * 30.0;
            let fres = edge.powf(4.0) * 30.0;
            // a soft shade on the lower inside, so the panel looks thick
            let shade = (1.0 - facing.max(0.0)) * (1.0 - t).powf(1.2) * if dark { 0.06 } else { 0.04 };
            let f = |c: f32| (((c + spec + fres) * (1.0 - shade)).clamp(0.0, 255.0) * cover) as u8;
            px[y * w + x] = Color32::from_rgba_premultiplied(f(cr), f(cg), f(cb), (255.0 * cover) as u8);
        }
    }
    ColorImage { size: [w, h], source_size: vec2(w as f32, h as f32), pixels: px }
}
