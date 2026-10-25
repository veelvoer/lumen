//! A tiny profiler: how long each part of a frame takes, summed up when a burst of animation ends, so slow
//! spots can be found on a real phone, and a rolling frame time the launcher uses to lighten itself.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering::Relaxed};
use std::time::Instant;

#[derive(Clone, Copy)]
pub enum Sec {
    Tick = 0,
    Springs,
    Paint,
    Touch,
    Overlays,
    Settings,
}

const NAMES: [&str; 6] = ["tick", "springs", "paint", "touch", "overlays", "settings"];
static MICROS: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];

pub struct Span(Sec, Instant);

pub fn span(s: Sec) -> Span {
    Span(s, Instant::now())
}

impl Drop for Span {
    fn drop(&mut self) {
        MICROS[self.0 as usize].fetch_add(self.1.elapsed().as_micros() as u32, Relaxed);
    }
}

#[derive(Default)]
pub struct Perf {
    frames: u32,
    dt_sum: f32,
    dt_max: f32,
    slow: u32,
    cpu_sum: f32,
    cpu_max: f32,
    cpu_n: u32,
    sec: [f32; 6],
    /// Smoothed frame time while things move (seconds). 0 until there is data.
    pub recent: f32,
    pub slow_run: u32,
    pub fast_run: u32,
}

impl Perf {
    /// Call once per frame with the time since the previous frame.
    pub fn frame(&mut self, dt: f32, budget: f32) {
        if dt > 0.0 && dt < 0.2 {
            self.frames += 1;
            self.dt_sum += dt;
            self.dt_max = self.dt_max.max(dt);
            if dt > budget * 1.35 {
                self.slow += 1;
                self.slow_run += 1;
                self.fast_run = 0;
            } else {
                self.fast_run += 1;
                self.slow_run = 0;
            }
            self.recent = if self.recent == 0.0 { dt } else { self.recent * 0.92 + dt * 0.08 };
            for (i, m) in MICROS.iter().enumerate() {
                self.sec[i] += m.swap(0, Relaxed) as f32 / 1000.0;
            }
        } else {
            for m in MICROS.iter() {
                m.store(0, Relaxed);
            }
            if self.frames >= 4 {
                let n = self.frames as f32;
                let parts: Vec<String> = NAMES.iter().zip(self.sec.iter()).filter(|(_, v)| **v > 0.0).map(|(n2, v)| format!("{n2} {:.2}", v / n)).collect();
                log::info!(
                    "burst: {} frames, dt avg {:.1} max {:.1} slow {}, cpu avg {:.1} max {:.1}, ms/frame [{}]",
                    self.frames,
                    self.dt_sum / n * 1000.0,
                    self.dt_max * 1000.0,
                    self.slow,
                    if self.cpu_n > 0 { self.cpu_sum / self.cpu_n as f32 * 1000.0 } else { 0.0 },
                    self.cpu_max * 1000.0,
                    parts.join(", ")
                );
            }
            self.frames = 0;
            self.dt_sum = 0.0;
            self.dt_max = 0.0;
            self.slow = 0;
            self.cpu_sum = 0.0;
            self.cpu_max = 0.0;
            self.cpu_n = 0;
            self.sec = [0.0; 6];
            self.slow_run = 0;
            self.fast_run = 0;
        }
    }

    pub fn cpu(&mut self, c: f32) {
        self.cpu_sum += c;
        self.cpu_max = self.cpu_max.max(c);
        self.cpu_n += 1;
    }
}

static LIGHT: AtomicBool = AtomicBool::new(false);

/// True while Lumen is in its light mode (no shadows, no blur, fewer effects).
pub fn light() -> bool {
    LIGHT.load(Relaxed)
}

pub fn set_light(on: bool) {
    LIGHT.store(on, Relaxed);
}

/// Is this a phone that should start in light mode? Little memory, few cores, or an old or small GPU.
pub fn weak_device(dev: &crate::platform::DeviceInfo, gpu: &str, cores: usize) -> bool {
    let g = gpu.to_lowercase();
    const WEAK_GPUS: [&str; 14] = ["powervr", "mali-400", "mali-450", "mali-t", "mali-g31", "mali-g51", "mali-g52 mc1", "mali-g52 mc2", "adreno (tm) 3", "adreno (tm) 4", "adreno (tm) 5", "adreno (tm) 610", "adreno (tm) 611", "adreno (tm) 612"];
    dev.low_ram || (dev.ram_mb > 0 && dev.ram_mb < 5000) || cores <= 4 || WEAK_GPUS.iter().any(|w| g.contains(w)) || (dev.sdk > 0 && dev.sdk < 28)
}
