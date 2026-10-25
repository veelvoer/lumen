//! Animation: springs with velocity, tuned per preset so each one moves like the real thing.
//! iOS is bouncy, Pixel is smooth and decisive, Nothing is quick and mechanical.

use crate::config::{AnimStyle, Config, Preset};

#[derive(Clone, Copy, Debug, Default)]
pub struct Spring {
    pub x: f32,
    pub v: f32,
    pub target: f32,
}

#[derive(Clone, Copy, Debug)]
pub struct Tune {
    /// seconds the spring takes to roughly arrive; 0 means no animation at all
    pub response: f32,
    /// 1 = no overshoot, below 1 bounces
    pub damping: f32,
}

impl Spring {
    pub fn at(x: f32) -> Spring {
        Spring { x, v: 0.0, target: x }
    }

    pub fn snap(&mut self, x: f32) {
        self.x = x;
        self.v = 0.0;
        self.target = x;
    }

    /// Moves one frame. Returns true while it is still moving.
    pub fn step(&mut self, dt: f32, t: Tune) -> bool {
        if t.response <= 0.0 {
            self.snap(self.target);
            return false;
        }
        let w = std::f32::consts::TAU / t.response;
        let (k, c) = (w * w, 2.0 * t.damping * w);
        let mut left = dt.clamp(0.0, 0.05);
        while left > 0.0 {
            let h = left.min(1.0 / 240.0);
            let a = k * (self.target - self.x) - c * self.v;
            self.v += a * h;
            self.x += self.v * h;
            left -= h;
        }
        if (self.target - self.x).abs() < 0.0006 && self.v.abs() < 0.02 {
            self.x = self.target;
            self.v = 0.0;
            false
        } else {
            true
        }
    }

    pub fn moving(&self) -> bool {
        (self.target - self.x).abs() > 0.0006 || self.v.abs() > 0.02
    }
}

/// A point that follows a target with a spring on each axis.
#[derive(Clone, Copy, Debug)]
pub struct Spring2 {
    pub x: Spring,
    pub y: Spring,
}

impl Spring2 {
    pub fn at(p: egui::Pos2) -> Spring2 {
        Spring2 { x: Spring::at(p.x), y: Spring::at(p.y) }
    }

    pub fn pos(&self) -> egui::Pos2 {
        egui::pos2(self.x.x, self.y.x)
    }

    pub fn step_to(&mut self, target: egui::Pos2, dt: f32, t: Tune) -> bool {
        self.x.target = target.x;
        self.y.target = target.y;
        let a = self.x.step(dt, t);
        let b = self.y.step(dt, t);
        a || b
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Feel {
    pub page: Tune,
    pub drawer: Tune,
    pub popup: Tune,
    pub item: Tune,
    pub press: Tune,
    pub menu: Tune,
    pub screen: Tune,
}

fn t(response: f32, damping: f32) -> Tune {
    Tune { response, damping }
}

pub fn resolve_style(cfg: &Config) -> AnimStyle {
    match cfg.anim_style {
        AnimStyle::Auto => match cfg.preset {
            Preset::Ios => AnimStyle::Bouncy,
            Preset::Nothing => AnimStyle::Snappy,
            _ => AnimStyle::Smooth,
        },
        s => s,
    }
}

pub fn feel(cfg: &Config) -> Feel {
    let base = match resolve_style(cfg) {
        AnimStyle::Off => return Feel { page: t(0.0, 1.0), drawer: t(0.0, 1.0), popup: t(0.0, 1.0), item: t(0.0, 1.0), press: t(0.0, 1.0), menu: t(0.0, 1.0), screen: t(0.0, 1.0) },
        AnimStyle::Bouncy => Feel { page: t(0.44, 1.0), drawer: t(0.46, 0.9), popup: t(0.42, 0.72), item: t(0.38, 0.7), press: t(0.22, 0.7), menu: t(0.34, 0.7), screen: t(0.44, 0.92) },
        AnimStyle::Snappy => Feel { page: t(0.26, 1.0), drawer: t(0.28, 1.0), popup: t(0.22, 1.0), item: t(0.22, 0.95), press: t(0.12, 1.0), menu: t(0.18, 1.0), screen: t(0.26, 1.0) },
        _ => Feel { page: t(0.38, 0.96), drawer: t(0.42, 0.92), popup: t(0.34, 0.86), item: t(0.30, 0.82), press: t(0.18, 0.9), menu: t(0.28, 0.84), screen: t(0.40, 0.96) },
    };
    let s = cfg.anim_speed.clamp(0.25, 4.0);
    let f = |x: Tune| Tune { response: x.response / s, damping: x.damping };
    Feel { page: f(base.page), drawer: f(base.drawer), popup: f(base.popup), item: f(base.item), press: f(base.press), menu: f(base.menu), screen: f(base.screen) }
}
