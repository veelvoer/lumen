//! Talks to the phone off the UI thread. Every JNI call can stall for a few milliseconds, and a few
//! milliseconds is a dropped frame, so all of them happen on one worker. The UI only reads a snapshot.

use crate::platform::{self, Battery, Haptic, Insets};
use std::collections::HashMap;
use std::sync::mpsc::{channel, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex, OnceLock};


static BG: OnceLock<Mutex<Sender<Cmd>>> = OnceLock::new();

/// Runs something that talks to Android (it can take milliseconds) off the drawing thread.
pub fn bg(f: impl FnOnce() + Send + 'static) {
    if let Some(tx) = BG.get().and_then(|m| m.lock().ok()) {
        let _ = tx.send(Cmd::Run(Box::new(f)));
    }
}
use std::time::{Duration, Instant};

#[derive(Clone, Default)]
pub struct Snapshot {
    pub insets: Insets,
    pub tz_offset_ms: i64,
    pub h24: bool,
    pub battery: Option<Battery>,
    pub badges: HashMap<String, u32>,
    pub dark: bool,
    pub dark_text: bool,
    pub refresh_hz: f32,
    pub default_home: bool,
    pub wallpaper_access: bool,
    pub wallpaper_live: bool,
    pub home_pressed: bool,
    pub settings_requested: bool,
}

enum Cmd {
    Run(Box<dyn FnOnce() + Send>),
    Blur(i32),
    Offset(f32),
    Haptic(Haptic),
    Bars { hide_status: bool, hide_nav: bool },
    Rotate(bool),
    LightBars(bool),
    HighRefresh(bool),
    Poke,
}

#[derive(Clone)]
pub struct Bridge {
    snap: Arc<Mutex<Snapshot>>,
    tx: Sender<Cmd>,
}

fn poll(s: &mut Snapshot, slow: bool) {
    s.insets = platform::insets();
    if platform::take_home_pressed() {
        s.home_pressed = true;
    }
    if platform::take_settings_requested() {
        s.settings_requested = true;
    }
    if slow {
        s.tz_offset_ms = platform::tz_offset_ms();
        s.h24 = platform::is_24h();
        s.battery = Some(platform::battery());
        s.badges = platform::badges();
        s.dark = platform::is_dark();
        s.dark_text = platform::wallpaper_dark_text();
        s.refresh_hz = platform::refresh_rate();
        s.default_home = platform::is_default_home();
        s.wallpaper_access = platform::wallpaper_access();
        s.wallpaper_live = platform::wallpaper_live();
    }
}

impl Bridge {
    pub fn new(ctx: &egui::Context) -> Bridge {
        let snap = Arc::new(Mutex::new(Snapshot::default()));
        {
            let mut s = snap.lock().unwrap();
            poll(&mut s, true);
        }
        let (tx, rx) = channel::<Cmd>();
        let _ = BG.set(Mutex::new(tx.clone()));
        let (s2, ctx2) = (snap.clone(), ctx.clone());
        std::thread::spawn(move || {
            let mut last_slow = Instant::now();
            let mut blur = -1;
            loop {
                match rx.recv_timeout(Duration::from_millis(350)) {
                    Ok(cmd) => match cmd {
                        Cmd::Run(f) => f(),
                        Cmd::Blur(px) => {
                            if px != blur {
                                blur = px;
                                platform::set_blur(px);
                            }
                        }
                        Cmd::Offset(x) => platform::set_wallpaper_offset(x),
                        Cmd::Haptic(h) => platform::haptic(h),
                        Cmd::Bars { hide_status, hide_nav } => platform::set_system_bars(hide_status, hide_nav),
                        Cmd::Rotate(on) => platform::set_rotation(on),
                        Cmd::LightBars(l) => platform::set_light_bars(l),
                        Cmd::HighRefresh(on) => platform::request_high_refresh(on),
                        Cmd::Poke => {}
                    },
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
                let slow = last_slow.elapsed() > Duration::from_secs(4);
                if slow {
                    last_slow = Instant::now();
                }
                let mut next = s2.lock().unwrap().clone();
                poll(&mut next, slow);
                let changed = {
                    let cur = s2.lock().unwrap();
                    (next.insets.top, next.insets.bottom, next.insets.left, next.insets.right) != (cur.insets.top, cur.insets.bottom, cur.insets.left, cur.insets.right)
                        || next.home_pressed
                        || next.settings_requested
                        || next.dark != cur.dark
                        || next.dark_text != cur.dark_text
                };
                // keep flags the UI has not consumed yet
                {
                    let mut cur = s2.lock().unwrap();
                    next.home_pressed |= cur.home_pressed;
                    next.settings_requested |= cur.settings_requested;
                    *cur = next;
                }
                if changed {
                    ctx2.request_repaint();
                }
            }
        });
        Bridge { snap, tx }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snap.lock().map(|s| s.clone()).unwrap_or_default()
    }

    pub fn take_home(&self) -> bool {
        self.snap.lock().map(|mut s| std::mem::take(&mut s.home_pressed)).unwrap_or(false)
    }

    pub fn take_settings(&self) -> bool {
        self.snap.lock().map(|mut s| std::mem::take(&mut s.settings_requested)).unwrap_or(false)
    }

    pub fn blur(&self, px: i32) {
        let _ = self.tx.send(Cmd::Blur(px));
    }

    pub fn wallpaper_offset(&self, x: f32) {
        let _ = self.tx.send(Cmd::Offset(x));
    }

    pub fn haptic(&self, h: Haptic) {
        let _ = self.tx.send(Cmd::Haptic(h));
    }

    pub fn bars(&self, hide_status: bool, hide_nav: bool) {
        let _ = self.tx.send(Cmd::Bars { hide_status, hide_nav });
    }

    /// Ask for the display's fastest refresh rate while Lumen is showing.
    pub fn high_refresh(&self, on: bool) {
        let _ = self.tx.send(Cmd::HighRefresh(on));
    }

    pub fn light_bars(&self, light: bool) {
        let _ = self.tx.send(Cmd::LightBars(light));
    }

    pub fn rotate(&self, on: bool) {
        let _ = self.tx.send(Cmd::Rotate(on));
    }

    #[allow(dead_code)]
    pub fn poke(&self) {
        let _ = self.tx.send(Cmd::Poke);
    }
}
