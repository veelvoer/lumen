//! Widgets that belong to other apps. Android draws them off screen as Views; a worker asks for a picture of
//! each one now and then and Lumen shows it as a texture. Taps are sent back to the real view.

use crate::platform::{self, WidgetProvider};
use egui::{ColorImage, Context, TextureHandle, TextureId, TextureOptions};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

enum Req {
    Render { wid: i32, w: i32, h: i32 },
    Preview { pkg: String, cls: String },
    Providers,
}

enum Done {
    Image { wid: i32, img: ColorImage },
    Preview { key: String, img: ColorImage },
    Providers(Vec<WidgetProvider>),
}

/// Something the add-a-widget flow finished.
pub enum Event {
    Ready { wid: i32, provider: WidgetProvider },
    Failed(String),
}

pub struct AppWidgets {
    tx: Sender<Req>,
    rx: Receiver<Done>,
    tex: HashMap<i32, TextureHandle>,
    previews: HashMap<String, TextureHandle>,
    preview_asked: RefCell<std::collections::HashSet<String>>,
    asked: RefCell<HashMap<i32, (f64, i32, i32)>>,
    pub providers: Option<Vec<WidgetProvider>>,
    providers_asked: bool,
    pub events: Arc<Mutex<Vec<Event>>>,
    ctx: Context,
}

impl AppWidgets {
    pub fn new(ctx: &Context) -> AppWidgets {
        let (tx, rx_req) = channel::<Req>();
        let (tx_done, rx) = channel::<Done>();
        let ctx2 = ctx.clone();
        std::thread::spawn(move || {
            while let Ok(req) = rx_req.recv() {
                match req {
                    Req::Render { wid, w, h } => {
                        if let Some(r) = platform::widget_render(wid, w, h) {
                            let hgt = r.rgba.len() / 4 / r.size.max(1);
                            if hgt > 0 {
                                let _ = tx_done.send(Done::Image { wid, img: ColorImage::from_rgba_premultiplied([r.size, hgt], &r.rgba) });
                            }
                        }
                    }
                    Req::Preview { pkg, cls } => {
                        if let Some(r) = platform::widget_preview(&pkg, &cls, 360) {
                            let hgt = r.rgba.len() / 4 / r.size.max(1);
                            if hgt > 0 {
                                let _ = tx_done.send(Done::Preview { key: format!("{pkg}/{cls}"), img: ColorImage::from_rgba_premultiplied([r.size, hgt], &r.rgba) });
                            }
                        }
                    }
                    Req::Providers => {
                        let mut v = platform::widget_providers();
                        v.sort_by(|a, b| a.pkg.cmp(&b.pkg).then(a.label.cmp(&b.label)));
                        let _ = tx_done.send(Done::Providers(v));
                    }
                }
                ctx2.request_repaint();
            }
        });
        AppWidgets { tx, rx, tex: HashMap::new(), previews: HashMap::new(), preview_asked: Default::default(), asked: Default::default(), providers: None, providers_asked: false, events: Arc::new(Mutex::new(vec![])), ctx: ctx.clone() }
    }

    pub fn pump(&mut self) {
        let mut n = 0;
        while n < 3 {
            let Ok(d) = self.rx.try_recv() else { break };
            n += 1;
            match d {
                Done::Image { wid, img } => match self.tex.get_mut(&wid) {
                    Some(t) => t.set(img, TextureOptions::LINEAR),
                    None => {
                        let t = self.ctx.load_texture(format!("awidget:{wid}"), img, TextureOptions::LINEAR);
                        self.tex.insert(wid, t);
                    }
                },
                Done::Preview { key, img } => {
                    let t = self.ctx.load_texture(format!("wpreview:{key}"), img, TextureOptions::LINEAR);
                    self.previews.insert(key, t);
                }
                Done::Providers(v) => self.providers = Some(v),
            }
        }
        if n == 3 {
            self.ctx.request_repaint();
        }
    }

    /// Ask for a fresh picture of a visible widget: right away the first time, then every second.
    pub fn request(&self, wid: i32, w_px: f32, h_px: f32, now: f64) {
        let (w, h) = (((w_px / 8.0).round() as i32 * 8).max(8), ((h_px / 8.0).round() as i32 * 8).max(8));
        let mut asked = self.asked.borrow_mut();
        let due = match asked.get(&wid) {
            None => true,
            Some((t, aw, ah)) => now - t > if crate::perf::light() { 2.5 } else { 1.0 } || *aw != w || *ah != h,
        };
        if due {
            asked.insert(wid, (now, w, h));
            let _ = self.tx.send(Req::Render { wid, w, h });
        }
    }

    /// After a tap the widget usually changes: look again soon.
    pub fn refresh_soon(&self, wid: i32) {
        if let Some(e) = self.asked.borrow_mut().get_mut(&wid) {
            e.0 -= 0.8;
        }
        self.ctx.request_repaint_after(Duration::from_millis(250));
    }

    pub fn texture(&self, wid: i32) -> Option<TextureId> {
        self.tex.get(&wid).map(|t| t.id())
    }

    pub fn forget(&mut self, wid: i32) {
        self.tex.remove(&wid);
        self.asked.borrow_mut().remove(&wid);
    }

    pub fn load_providers(&mut self) {
        if !self.providers_asked {
            self.providers_asked = true;
            let _ = self.tx.send(Req::Providers);
        }
    }

    pub fn preview(&self, p: &WidgetProvider) -> Option<(TextureId, [usize; 2])> {
        let key = format!("{}/{}", p.pkg, p.cls);
        if let Some(t) = self.previews.get(&key) {
            return Some((t.id(), t.size()));
        }
        if self.preview_asked.borrow_mut().insert(key) {
            let _ = self.tx.send(Req::Preview { pkg: p.pkg.clone(), cls: p.cls.clone() });
        }
        None
    }

    /// Adds a widget: allocates an id, asks Android to bind it (the user may be asked), opens its
    /// configuration if it has one, and reports through `events`.
    pub fn add(&self, provider: WidgetProvider) {
        let events = self.events.clone();
        let ctx = self.ctx.clone();
        std::thread::spawn(move || {
            let fail = |msg: &str| {
                if let Ok(mut e) = events.lock() {
                    e.push(Event::Failed(msg.to_string()));
                }
                ctx.request_repaint();
            };
            let wid = platform::widget_allocate();
            if wid < 0 {
                return fail("Android would not make a widget");
            }
            let wait_for = |prefix: &str| -> Option<bool> {
                for _ in 0..600 {
                    let r = platform::take_widget_result();
                    if r.starts_with(prefix) {
                        return Some(r.ends_with(":ok"));
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                None
            };
            match platform::widget_bind(wid, &provider.pkg, &provider.cls) {
                1 => {}
                0 => {
                    if wait_for("bind:") != Some(true) {
                        platform::widget_delete(wid);
                        return fail("The widget was not allowed");
                    }
                }
                _ => return fail("This widget cannot be added"),
            }
            if provider.configure && platform::widget_configure(wid) == 1 && wait_for("config:") != Some(true) {
                platform::widget_delete(wid);
                return fail("The widget was not set up");
            }
            if let Ok(mut e) = events.lock() {
                e.push(Event::Ready { wid, provider });
            }
            ctx.request_repaint();
        });
    }

    /// Sends a tap to the real view (down, a moment, up).
    pub fn tap(&self, wid: i32, x: f32, y: f32) {
        crate::bridge::bg(move || {
            platform::widget_touch(wid, 0, x, y);
            std::thread::sleep(Duration::from_millis(45));
            platform::widget_touch(wid, 1, x, y);
        });
        self.refresh_soon(wid);
    }

    pub fn delete(&mut self, wid: i32) {
        self.forget(wid);
        crate::bridge::bg(move || platform::widget_delete(wid));
    }
}
