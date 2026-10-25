//! Scrolling that behaves like Android's: your finger drags the content one to one, letting go flings it
//! with a speed limit and a steady slow-down, and the ends give a little before pulling back.

use egui::{Context, Pos2, Rect, ScrollArea, Ui};

/// points per second; a hard flick on a phone is 5000 to 8000 pixels per second
const MAX_FLING: f32 = 3200.0;
/// how fast a fling slows down: the speed is multiplied by e^(-FRICTION * seconds)
const FRICTION: f32 = 3.3;
const SLOP: f32 = 6.0;

#[derive(Default, Clone, Debug)]
pub struct Scroller {
    pub offset: f32,
    pub vel: f32,
    /// finger is down inside the area
    holding: bool,
    /// and has moved far enough to scroll
    pub moved: bool,
    /// the press landed on a moving fling (it only stops it, it is not a tap)
    suppress: bool,
    /// settings pages hold sliders: a drag that starts on a widget must not scroll
    pub respect_widgets: bool,
    /// presses that start here belong to something else (the letter scroller)
    pub exclude: Option<Rect>,
    /// glide to this offset (letter scroller); any touch on the list cancels it
    pub glide: Option<f32>,
    /// lists made of egui widgets cannot draw past their ends: no bounce there
    pub hard_ends: bool,
    pub holding_now: bool,
    start: f32,
    start_x: f32,
    last: f32,
    /// how far the content can scroll, learned from the last frame
    pub max: f32,
    samples: Vec<(f64, f32)>,
}

impl Scroller {
    /// False when the finger scrolled or only stopped a fling: then a release is not a tap.
    pub fn tap_ok(&self) -> bool {
        !self.moved && !self.suppress
    }

    pub fn scroll_to(&mut self, offset: f32) {
        self.glide = Some(offset);
    }

    pub fn jump_to(&mut self, offset: f32) {
        self.offset = offset;
        self.vel = 0.0;
    }

    pub fn reset(&mut self) {
        *self = Scroller::default();
    }

    /// The finger moved by `dy` points (positive = down). Past either end the content gives, but resists.
    pub fn drag_by(&mut self, dy: f32, max: f32) {
        let over = if self.offset < 0.0 { -self.offset } else if self.offset > max { self.offset - max } else { 0.0 };
        let moving_out = (self.offset <= 0.0 && dy > 0.0) || (self.offset >= max && dy < 0.0);
        let k = if moving_out { 0.55 / (1.0 + over / 90.0) } else { 1.0 };
        self.offset -= dy * k;
        self.vel = 0.0;
    }

    /// The finger lifted while moving at `v` points per second (positive = content should move up).
    pub fn release(&mut self, v: f32) {
        self.vel = v.clamp(-MAX_FLING, MAX_FLING);
    }

    /// One frame of fling and bounce. True while still moving.
    pub fn step(&mut self, dt: f32, max: f32) -> bool {
        let max = max.max(0.0);
        let dt = dt.clamp(0.0, 0.05);
        if let Some(t) = self.glide {
            // critically damped glide, keeps its speed when the target moves under it
            let t = t.clamp(0.0, max);
            let w = std::f32::consts::TAU / 0.30;
            let mut left = dt;
            while left > 0.0 {
                let h = left.min(1.0 / 240.0);
                let a = -w * w * (self.offset - t) - 2.0 * w * self.vel;
                self.vel += a * h;
                self.offset += self.vel * h;
                left -= h;
            }
            if (self.offset - t).abs() < 0.4 && self.vel.abs() < 8.0 {
                self.offset = t;
                self.vel = 0.0;
                self.glide = None;
                return false;
            }
            return true;
        }
        if self.offset < 0.0 || self.offset > max {
            // beyond an end: a critically damped spring pulls it back, keeping any speed it arrived with
            let bound = if self.offset < 0.0 { 0.0 } else { max };
            let w = std::f32::consts::TAU / 0.42;
            let mut left = dt;
            while left > 0.0 {
                let h = left.min(1.0 / 240.0);
                let a = -w * w * (self.offset - bound) - 2.0 * w * self.vel;
                self.vel += a * h;
                self.offset += self.vel * h;
                left -= h;
            }
            if (self.offset - bound).abs() < 0.3 && self.vel.abs() < 5.0 {
                self.offset = bound;
                self.vel = 0.0;
                return false;
            }
            return true;
        }
        if self.vel.abs() < 6.0 {
            self.vel = 0.0;
            return false;
        }
        self.offset += self.vel * dt;
        self.vel *= (-FRICTION * dt).exp();
        true
    }

    /// Drives itself from the pointer: press inside `area`, drag, let go. Returns true while it needs more frames.
    pub fn update(&mut self, ctx: &Context, area: Rect, max: f32, enabled: bool, dt: f32) -> bool {
        let (pressed, down, pos, time) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.latest_pos(), i.time));
        if pressed && enabled && pos.is_some_and(|p| area.contains(p) && !self.exclude.is_some_and(|e| e.contains(p))) {
            self.holding = true;
            self.glide = None;
            self.moved = false;
            self.suppress = self.vel.abs() > 30.0;
            let y = pos.map(|p: Pos2| p.y).unwrap_or(0.0);
            self.start = y;
            self.start_x = pos.map(|p: Pos2| p.x).unwrap_or(0.0);
            self.last = y;
            self.samples.clear();
            self.vel = 0.0;
        }
        let mut busy = false;
        if self.holding {
            if down && enabled {
                if let Some(p) = pos {
                    if !self.moved {
                        let (dy, dx) = ((p.y - self.start).abs(), (p.x - self.start_x).abs());
                        if dx > SLOP && dx > dy {
                            // sideways: a slider or a page swipe, not ours
                            self.holding = false;
                        } else if dy > SLOP && !(self.respect_widgets && ctx.egui_is_using_pointer()) {
                            self.moved = true;
                            self.last = p.y; // no jump when the scroll begins
                        }
                    }
                    if self.moved {
                        self.drag_by(p.y - self.last, max);
                        self.last = p.y;
                        self.samples.push((time, p.y));
                        self.samples.retain(|(t, _)| time - *t < 0.09);
                    }
                }
                ctx.request_repaint();
                busy = true;
            } else {
                self.holding = false;
                if self.moved {
                    let v = match (self.samples.first(), self.samples.last()) {
                        (Some(a), Some(b)) if b.0 - a.0 > 0.01 && time - b.0 < 0.08 => -(b.1 - a.1) / (b.0 - a.0) as f32,
                        _ => 0.0,
                    };
                    self.release(v);
                }
            }
        }
        self.holding_now = self.holding;
        if !self.holding {
            busy |= self.step(dt, max);
        }
        if self.hard_ends {
            if self.offset < 0.0 || self.offset > max {
                self.offset = self.offset.clamp(0.0, max);
                if !self.holding {
                    self.vel = 0.0;
                }
            }
        }
        if busy {
            ctx.request_repaint();
        }
        busy
    }
}

/// A normal egui scroll area whose position and momentum come from a [`Scroller`], so every list in
/// Lumen flings and bounces the same way.
pub fn show<R>(ui: &mut Ui, sc: &mut Scroller, salt: impl std::hash::Hash + std::fmt::Debug, body: Rect, enabled: bool, add: impl FnOnce(&mut Ui) -> R) -> R {
    let ctx = ui.ctx().clone();
    let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
    let max = sc.max;
    let tag = format!("{salt:?}");
    // only settings pages hold sliders; elsewhere a press on a big touch area must still be able to scroll
    sc.respect_widgets = tag.contains("settings");
    sc.hard_ends = true;
    sc.update(&ctx, body, max, enabled, dt);
    let out = ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink(false)
        .scroll_source(egui::scroll_area::ScrollSource::NONE)
        .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
        .vertical_scroll_offset(sc.offset.max(0.0))
        .show(ui, add);
    sc.max = (out.content_size.y - out.inner_rect.height()).max(0.0);
    if sc.offset > sc.max && !sc.holding {
        // the content shrank (a section closed)
        sc.offset = sc.max;
    }
    out.inner
}
