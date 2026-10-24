//! The launcher itself: state, gestures, home screen and app drawer.

use crate::android::{self, App, Insets};
use crate::config::{Clock, Config, DockStyle, DrawerStyle, Preset, Search};
use crate::icons::Icons;
use crate::theme::{self, Palette};
use crate::update::Updater;
use crate::widgets;
use eframe::egui::{
    self, pos2, vec2, Align2, Area, Color32, Context, CornerRadius, FontId, Frame, Id, Key, LayerId, Order, Pos2, Rect, Sense, Stroke,
    StrokeKind, UiBuilder, Vec2,
};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

#[derive(PartialEq, Clone, Copy)]
pub enum Screen {
    Home,
    Settings,
}

enum Gesture {
    None,
    Undecided(Vec2),
    Pager,
    Drawer,
    Shade,
}

#[derive(Clone)]
pub enum Target {
    App(String),
    Home,
}

pub struct Menu {
    pub target: Target,
    pub pos: Pos2,
    from_drawer: bool,
}

pub struct Lumen {
    pub cfg: Config,
    pub dir: String,
    pub pal: Palette,
    pub apps: Vec<App>,
    by_pkg: HashMap<String, usize>,
    apps_rx: Receiver<Vec<App>>,
    pub icons: Icons,
    pub upd: Updater,
    pub version: String,
    pub screen: Screen,
    pub dirty: bool,
    pub default_home: bool,

    page: f32,
    page_target: f32,
    drawer: f32,
    drawer_open: bool,
    gesture: Gesture,
    query: String,
    focus_search: bool,
    menu: Option<Menu>,
    insets: Insets,
    insets_at: Instant,
    was_focused: bool,
    clock: ClockInfo,
    clock_at: Option<Instant>,
    styled: Option<(crate::config::Mode, [u8; 3], Preset, u32)>,
}

#[derive(Default, Clone)]
struct ClockInfo {
    time: String,
    date: String,
    short_date: String,
    seconds: u64,
}

const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const MONTHS: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

fn clock_now() -> ClockInfo {
    let ms = android::local_millis();
    let days = ms.div_euclid(86_400_000);
    let secs = ms.rem_euclid(86_400_000) / 1000;
    let (mut h, m) = (secs / 3600, (secs % 3600) / 60);
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let mo = if mp < 10 { mp + 3 } else { mp - 9 };
    let wd = DAYS[(days + 4).rem_euclid(7) as usize];
    let mon = MONTHS[(mo - 1) as usize];
    let time = if android::is_24h() {
        format!("{h:02}:{m:02}")
    } else {
        let s = format!("{}:{m:02}", if h % 12 == 0 { 12 } else { h % 12 });
        h = 0;
        let _ = h;
        s
    };
    ClockInfo { time, date: format!("{wd}, {mon} {d}"), short_date: format!("{} {d} {}", wd.to_uppercase(), mon.to_uppercase()), seconds: (secs % 60) as u64 }
}

fn spawn_app_scan(ctx: &Context) -> Receiver<Vec<App>> {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(android::launcher_apps());
        ctx.request_repaint();
    });
    rx
}

impl Lumen {
    pub fn new(ctx: &Context, dir: String) -> Lumen {
        let cfg = Config::load(&dir);
        let pal = theme::palette(&cfg);
        let icons = Icons::new(ctx, cfg.icon_radius, cfg.mono_icons);
        let mut l = Lumen {
            pal,
            icons,
            dir,
            apps: vec![],
            by_pkg: HashMap::new(),
            apps_rx: spawn_app_scan(ctx),
            upd: Updater::default(),
            version: android::version_name(),
            screen: Screen::Home,
            dirty: false,
            default_home: android::is_default_home(),
            page: 0.0,
            page_target: 0.0,
            drawer: 0.0,
            drawer_open: false,
            gesture: Gesture::None,
            query: String::new(),
            focus_search: false,
            menu: None,
            insets: Insets::default(),
            insets_at: Instant::now() - Duration::from_secs(10),
            was_focused: true,
            clock: ClockInfo::default(),
            clock_at: None,
            styled: None,
            cfg,
        };
        l.restyle(ctx);
        l
    }

    pub fn app(&self, pkg: &str) -> Option<&App> {
        self.by_pkg.get(pkg).map(|&i| &self.apps[i])
    }

    /// Applies colors and icon style after any change in the config.
    pub fn restyle(&mut self, ctx: &Context) {
        self.pal = theme::palette(&self.cfg);
        theme::apply(ctx, &self.cfg, &self.pal);
        android::set_light_bars(self.pal.light);
        let mut st = ctx.global_style().as_ref().clone();
        st.spacing.item_spacing = vec2(8.0, 8.0);
        st.spacing.button_padding = vec2(14.0, 10.0);
        st.spacing.interact_size.y = 40.0;
        st.spacing.slider_width = 160.0;
        ctx.set_global_style(st);
        if !ctx.input(|i| i.pointer.any_down()) {
            self.icons.restyle(self.cfg.icon_radius, self.cfg.mono_icons);
        }
        self.styled = Some((self.cfg.mode, self.cfg.accent, self.cfg.preset, (self.cfg.wallpaper_dim * 100.0) as u32));
    }

    fn poll_apps(&mut self, ctx: &Context) {
        if let Ok(mut list) = self.apps_rx.try_recv() {
            list.dedup_by(|a, b| a.pkg == b.pkg);
            self.by_pkg = list.iter().enumerate().map(|(i, a)| (a.pkg.clone(), i)).collect();
            self.apps = list;
            if !self.cfg.seeded {
                self.seed();
            }
            let known: std::collections::HashSet<&str> = self.apps.iter().map(|a| a.pkg.as_str()).collect();
            self.cfg.tidy(&|p| known.contains(p));
            self.dirty = true;
            ctx.request_repaint();
        }
    }

    /// First start: put the usual apps on the dock and first page so the home screen is not empty.
    fn seed(&mut self) {
        let mut dock: Vec<String> = vec![];
        for (action, data) in [
            ("android.intent.action.DIAL", None),
            ("android.intent.action.SENDTO", Some("smsto:")),
            ("android.intent.action.VIEW", Some("https://example.com")),
            ("android.media.action.IMAGE_CAPTURE", None),
        ] {
            if let Some(p) = android::default_for(action, data) {
                if self.by_pkg.contains_key(&p) && !dock.contains(&p) {
                    dock.push(p);
                }
            }
        }
        const WELL_KNOWN: [&str; 16] = [
            "com.android.settings", "com.android.vending", "com.google.android.apps.maps", "com.google.android.gm", "com.google.android.apps.photos",
            "com.google.android.youtube", "com.google.android.deskclock", "com.android.deskclock", "com.google.android.calculator", "com.google.android.calendar",
            "com.spotify.music", "com.whatsapp", "org.mozilla.firefox", "com.android.chrome", "com.google.android.apps.messaging", "com.instagram.android",
        ];
        let mut page: Vec<String> = WELL_KNOWN.iter().filter(|p| self.by_pkg.contains_key(**p) && !dock.contains(&p.to_string())).map(|p| p.to_string()).collect();
        if page.len() < 8 {
            for a in &self.apps {
                if page.len() >= 12 {
                    break;
                }
                if !page.contains(&a.pkg) && !dock.contains(&a.pkg) {
                    page.push(a.pkg.clone());
                }
            }
        }
        self.cfg.dock_apps = dock;
        self.cfg.pages = vec![page];
        self.cfg.seeded = true;
    }

    fn refresh_insets(&mut self, ctx: &Context) {
        if self.insets_at.elapsed() > Duration::from_millis(700) {
            self.insets = android::insets();
            self.insets_at = Instant::now();
        }
        let _ = ctx;
    }

    fn inset_pts(&self, ctx: &Context) -> (f32, f32, f32, f32) {
        let k = ctx.pixels_per_point();
        (self.insets.left / k, self.insets.top / k, self.insets.right / k, self.insets.bottom / k)
    }

    pub fn open_settings(&mut self) {
        self.screen = Screen::Settings;
        self.menu = None;
        self.drawer_open = false;
    }

    fn close_drawer(&mut self, ctx: &Context) {
        self.drawer_open = false;
        self.query.clear();
        self.focus_search = false;
        ctx.memory_mut(|m| m.stop_text_input());
    }

    fn launch(&mut self, ctx: &Context, pkg: &str) {
        if let Some(a) = self.app(pkg) {
            android::launch(a);
        }
        self.menu = None;
        self.close_drawer(ctx);
        self.drawer = 0.0;
    }

    fn clock_info(&mut self) -> ClockInfo {
        let stale = self.clock_at.is_none_or(|t| t.elapsed() > Duration::from_secs(1));
        if stale {
            self.clock = clock_now();
            self.clock_at = Some(Instant::now());
        }
        self.clock.clone()
    }

    // ───────────────────────────── layout ─────────────────────────────

    fn layout(&self, ctx: &Context, screen: Rect) -> Layout {
        let (il, it, ir, ib) = self.inset_pts(ctx);
        let c = &self.cfg;
        let side = 14.0;
        let bottom = screen.bottom() - ib.max(10.0);
        let icon = c.icon_size;
        let dock_h = icon + if c.dock == DockStyle::Bare { 22.0 } else { 30.0 };
        let dock = Rect::from_min_size(pos2(screen.left() + il.max(side), bottom - dock_h), vec2(screen.width() - il.max(side) - ir.max(side), dock_h));
        let search_h = 52.0;
        let mut floor = dock.top() - 8.0;
        let mut search = None;
        let mut bottom_of_dock = dock;
        if c.search == Search::Bottom {
            if c.preset == Preset::Pixel {
                // under the dock, like Pixel
                let s = Rect::from_min_size(pos2(dock.left(), bottom - search_h), vec2(dock.width(), search_h));
                bottom_of_dock = dock.translate(vec2(0.0, -(search_h + 10.0)));
                floor = bottom_of_dock.top() - 8.0;
                search = Some(s);
            } else {
                let s = Rect::from_min_size(pos2(dock.left() + 24.0, dock.top() - search_h - 6.0), vec2(dock.width() - 48.0, 44.0));
                floor = s.top() - 4.0;
                search = Some(s);
            }
        }
        let dots_y = floor - 10.0;
        if c.page_dots && c.pages.len() > 1 {
            floor -= 22.0;
        }
        let mut top = screen.top() + it + 8.0;
        let mut search_top = None;
        if c.search == Search::Top {
            search_top = Some(Rect::from_min_size(pos2(screen.left() + il.max(side) + 6.0, top), vec2(screen.width() - il.max(side) - ir.max(side) - 12.0, 48.0)));
            top += 58.0;
        }
        let clock_h = match c.clock {
            Clock::Off => 0.0,
            Clock::Glance => 96.0,
            Clock::Digital => 150.0,
            Clock::DotMatrix => 168.0,
        };
        let clock = Rect::from_min_size(pos2(screen.left() + il.max(side) + 10.0, top), vec2(screen.width() - il.max(side) - ir.max(side) - 20.0, clock_h));
        let grid = Rect::from_min_max(pos2(screen.left() + il.max(8.0), clock.bottom() + 4.0), pos2(screen.right() - ir.max(8.0), floor));
        Layout { clock, grid, dock: bottom_of_dock, search: search.or(search_top), dots_y }
    }

    fn cell(&self, grid: Rect, idx: usize) -> Rect {
        let (cols, rows) = (self.cfg.cols.max(1), self.cfg.rows.max(1));
        let (cw, ch) = (grid.width() / cols as f32, (grid.height() / rows as f32).min(self.cfg.icon_size + 44.0 + 22.0));
        Rect::from_min_size(pos2(grid.left() + (idx % cols) as f32 * cw, grid.top() + (idx / cols) as f32 * ch), vec2(cw, ch))
    }

    fn dock_cell(&self, dock: Rect, idx: usize) -> Rect {
        let n = self.cfg.dock_apps.len().max(1);
        let w = dock.width() / n.max(4) as f32;
        let total = w * n as f32;
        Rect::from_min_size(pos2(dock.center().x - total / 2.0 + idx as f32 * w, dock.top()), vec2(w, dock.height()))
    }

    fn hit(&self, lay: &Layout, p: Pos2) -> Option<String> {
        for (i, pkg) in self.cfg.dock_apps.iter().enumerate() {
            if self.dock_cell(lay.dock, i).contains(p) {
                return Some(pkg.clone());
            }
        }
        let pi = self.page_target.round() as usize;
        if self.page_target.fract().abs() < 0.01 && (self.page - self.page_target).abs() < 0.05 {
            for (i, pkg) in self.cfg.pages.get(pi)?.iter().enumerate() {
                if self.cell(lay.grid, i).contains(p) {
                    return Some(pkg.clone());
                }
            }
        }
        None
    }

    // ───────────────────────────── frame ─────────────────────────────

    pub fn frame(&mut self, root: &mut egui::Ui) {
        let ctx = &root.ctx().clone();
        self.icons.pump();
        self.poll_apps(ctx);
        self.refresh_insets(ctx);

        let focused = ctx.input(|i| i.focused);
        if focused && !self.was_focused {
            self.apps_rx = spawn_app_scan(ctx);
            self.default_home = android::is_default_home();
        }
        self.was_focused = focused;

        let back = ctx.input(|i| i.key_pressed(Key::BrowserBack) || i.key_pressed(Key::Escape));
        if back {
            if self.menu.is_some() {
                self.menu = None;
            } else if self.screen == Screen::Settings {
                self.screen = Screen::Home;
            } else if self.drawer_open {
                self.close_drawer(ctx);
            } else {
                self.page_target = 0.0;
            }
        }

        match self.screen {
            Screen::Home => self.home(root),
            Screen::Settings => crate::settings::show(self, root),
        }

        if self.dirty {
            self.cfg.save(&self.dir);
            self.dirty = false;
        }
    }

    fn home(&mut self, root: &mut egui::Ui) {
        let ctx = &root.ctx().clone();
        let screen = ctx.content_rect();
        let lay = self.layout(ctx, screen);
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        let pal = self.pal;
        let npages = self.cfg.pages.len().max(1);

        let mut tap: Option<Pos2> = None;
        let mut long: Option<Pos2> = None;

        egui::CentralPanel::default().frame(Frame::NONE).show(root, |ui| {
            let resp = ui.interact(screen, Id::new("home-surface"), Sense::click_and_drag());
            let p = ui.painter().clone();
            p.rect_filled(screen, 0, pal.scrim);
            if self.drawer > 0.0 {
                p.rect_filled(screen, 0, Color32::from_black_alpha((self.drawer * 140.0) as u8));
            }

            // gestures
            let w = screen.width();
            if resp.drag_started() {
                self.gesture = Gesture::Undecided(Vec2::ZERO);
            }
            if resp.dragged() {
                let d = resp.drag_delta();
                match &mut self.gesture {
                    Gesture::Undecided(acc) => {
                        *acc += d;
                        if acc.length() > 12.0 {
                            let a = *acc;
                            self.gesture = if a.x.abs() > a.y.abs() {
                                self.page -= a.x / w;
                                Gesture::Pager
                            } else if a.y < 0.0 {
                                self.drawer = (-a.y / (screen.height() * 0.5)).clamp(0.0, 1.0);
                                Gesture::Drawer
                            } else {
                                android::expand_notifications();
                                Gesture::Shade
                            };
                        }
                    }
                    Gesture::Pager => {
                        let max = (npages - 1) as f32;
                        let rubber = if self.page < 0.0 || self.page > max { 0.35 } else { 1.0 };
                        self.page -= d.x / w * rubber;
                    }
                    Gesture::Drawer => self.drawer = (self.drawer - d.y / (screen.height() * 0.5)).clamp(0.0, 1.0),
                    _ => {}
                }
            }
            if resp.drag_stopped() {
                let v = ui.input(|i| i.pointer.velocity());
                match self.gesture {
                    Gesture::Pager => {
                        let t = (self.page - v.x / w * 0.3).round();
                        self.page_target = t.clamp(0.0, (npages - 1) as f32);
                    }
                    Gesture::Drawer => {
                        self.drawer_open = self.drawer > 0.3 || v.y < -500.0;
                        if self.drawer_open {
                            self.focus_search = false;
                        }
                    }
                    _ => {}
                }
                self.gesture = Gesture::None;
            }
            if resp.clicked() {
                tap = resp.interact_pointer_pos();
            }
            if resp.long_touched() || resp.secondary_clicked() {
                long = resp.interact_pointer_pos().or_else(|| ui.input(|i| i.pointer.hover_pos()));
            }

            // animation
            let k = 1.0 - (-dt * 16.0).exp();
            if !matches!(self.gesture, Gesture::Pager) {
                let t = self.page_target.clamp(0.0, (npages - 1) as f32);
                self.page += (t - self.page) * k;
                if (t - self.page).abs() < 0.002 {
                    self.page = t;
                } else {
                    ctx.request_repaint();
                }
            }
            if !matches!(self.gesture, Gesture::Drawer) {
                let t = if self.drawer_open { 1.0 } else { 0.0 };
                self.drawer += (t - self.drawer) * k;
                if (t - self.drawer).abs() < 0.003 {
                    self.drawer = t;
                } else {
                    ctx.request_repaint();
                }
            }

            self.paint_clock(ctx, &p, lay.clock);
            self.paint_pages(ctx, &p, &lay, screen);
            self.paint_dock(ctx, &p, &lay);
            if let Some(s) = lay.search {
                widgets::search_pill(&p, s, "Search apps", &pal, &self.cfg);
            }
            if self.cfg.page_dots && npages > 1 {
                for i in 0..npages {
                    let x = screen.center().x + (i as f32 - (npages - 1) as f32 / 2.0) * 14.0;
                    let on = (self.page - i as f32).abs() < 0.5;
                    p.circle_filled(pos2(x, lay.dots_y), if on { 3.6 } else { 2.8 }, if on { pal.text } else { pal.dim.gamma_multiply(0.6) });
                }
            }
        });

        // taps and long-presses on the home surface
        if let Some(pos) = tap {
            if self.menu.is_some() {
                self.menu = None;
            } else if lay.search.is_some_and(|s| s.contains(pos)) {
                self.drawer_open = true;
                self.focus_search = true;
            } else if let Some(pkg) = self.hit(&lay, pos) {
                self.launch(ctx, &pkg);
            }
        }
        if let Some(pos) = long {
            let target = self.hit(&lay, pos).map(Target::App).unwrap_or(Target::Home);
            self.menu = Some(Menu { target, pos, from_drawer: false });
        }

        if self.drawer > 0.001 {
            self.drawer_ui(ctx, screen);
        }
        if self.menu.is_some() {
            self.menu_ui(ctx, screen);
        }

        if self.cfg.clock != Clock::Off {
            let s = self.clock_info().seconds;
            ctx.request_repaint_after(Duration::from_secs(61 - s));
        }
    }

    fn paint_clock(&mut self, _ctx: &Context, p: &egui::Painter, r: Rect) {
        if self.cfg.clock == Clock::Off {
            return;
        }
        let ci = self.clock_info();
        let (pal, fs) = (self.pal, self.cfg.font_scale);
        match self.cfg.clock {
            Clock::Glance => {
                p.text(r.left_top() + vec2(4.0, 6.0), Align2::LEFT_TOP, &ci.date, FontId::proportional(34.0 * fs), pal.text);
                p.text(r.left_top() + vec2(5.0, 52.0), Align2::LEFT_TOP, &ci.time, FontId::proportional(16.0 * fs), pal.dim);
            }
            Clock::Digital => {
                p.text(r.center_top() + vec2(0.0, 10.0), Align2::CENTER_TOP, &ci.time, FontId::proportional(84.0 * fs), pal.text);
                p.text(r.center_top() + vec2(0.0, 108.0), Align2::CENTER_TOP, &ci.date, FontId::proportional(17.0 * fs), pal.dim);
            }
            Clock::DotMatrix => {
                let dot = ((r.width() - 8.0) / (5.0 * 4.0 + 3.0 + 1.6 * 4.0 + 4.0)).min(11.0 * fs.max(0.9));
                let w = widgets::dot_text_width(&ci.time, dot);
                widgets::dot_text(p, pos2(r.center().x - w / 2.0, r.top() + 12.0), &ci.time, dot, pal.text, pal.accent);
                p.text(r.center_top() + vec2(0.0, 12.0 + 7.0 * dot + 16.0), Align2::CENTER_TOP, &ci.short_date, FontId::monospace(13.0 * fs), pal.dim);
            }
            Clock::Off => {}
        }
    }

    fn paint_pages(&mut self, _ctx: &Context, p: &egui::Painter, lay: &Layout, screen: Rect) {
        let (pal, cfg) = (self.pal, self.cfg.clone());
        let pressed = _ctx.input(|i| if i.pointer.any_down() { i.pointer.press_origin() } else { None });
        for (pi, page) in cfg.pages.iter().enumerate() {
            let off = (pi as f32 - self.page) * screen.width();
            if off.abs() >= screen.width() {
                continue;
            }
            for (i, pkg) in page.iter().enumerate() {
                let Some(&ai) = self.by_pkg.get(pkg) else { continue };
                let cell = self.cell(lay.grid, i).translate(vec2(off, 0.0));
                let app = self.apps[ai].clone();
                let down = pressed.is_some_and(|o| cell.contains(o)) && matches!(self.gesture, Gesture::None);
                self.paint_cell(p, cell, &app, down, cfg.labels, &pal, &cfg);
            }
        }
    }

    fn paint_cell(&mut self, p: &egui::Painter, cell: Rect, app: &App, down: bool, labels: bool, pal: &Palette, cfg: &Config) {
        let icon = cfg.icon_size;
        let ic = Rect::from_center_size(pos2(cell.center().x, cell.top() + 10.0 + icon / 2.0), vec2(icon, icon));
        widgets::icon(p, ic, app, &mut self.icons, cfg, pal, if down { 0.9 } else { 1.0 });
        if labels {
            widgets::label(p, pos2(cell.center().x, ic.bottom() + 6.0), &app.label, cfg, pal, cell.width() - 6.0, cfg.preset == Preset::Ios);
        }
    }

    fn paint_dock(&mut self, ctx: &Context, p: &egui::Painter, lay: &Layout) {
        let (pal, cfg) = (self.pal, self.cfg.clone());
        match cfg.dock {
            DockStyle::Glass | DockStyle::Solid => {
                let r = CornerRadius::same(cfg.dock_radius.clamp(0.0, 80.0) as u8);
                let fill = if cfg.dock == DockStyle::Solid { pal.sheet } else { pal.glass };
                p.rect_filled(lay.dock, r, fill);
                p.rect_stroke(lay.dock, r, Stroke::new(1.0, pal.glass_line), StrokeKind::Inside);
            }
            DockStyle::Bare => {}
        }
        let pressed = ctx.input(|i| if i.pointer.any_down() { i.pointer.press_origin() } else { None });
        for (i, pkg) in cfg.dock_apps.iter().enumerate() {
            let Some(&ai) = self.by_pkg.get(pkg) else { continue };
            let cell = self.dock_cell(lay.dock, i);
            let ic = Rect::from_center_size(cell.center() - vec2(0.0, if cfg.dock == DockStyle::Bare && cfg.labels { 8.0 } else { 0.0 }), vec2(cfg.icon_size, cfg.icon_size));
            let down = pressed.is_some_and(|o| cell.contains(o)) && matches!(self.gesture, Gesture::None);
            let app = self.apps[ai].clone();
            widgets::icon(p, ic, &app, &mut self.icons, &cfg, &pal, if down { 0.9 } else { 1.0 });
            if cfg.dock == DockStyle::Bare && cfg.labels {
                widgets::label(p, pos2(cell.center().x, ic.bottom() + 4.0), &app.label, &cfg, &pal, cell.width() - 6.0, false);
            }
        }
    }

    // ───────────────────────────── drawer ─────────────────────────────

    fn drawer_ui(&mut self, ctx: &Context, screen: Rect) {
        let t = self.drawer;
        let ease = 1.0 - (1.0 - t).powi(3);
        let y = screen.top() + (1.0 - ease) * screen.height();
        let rect = Rect::from_min_size(pos2(screen.left(), y), screen.size());
        let (il, it, ir, ib) = self.inset_pts(ctx);
        let (pal, cfg) = (self.pal, self.cfg.clone());
        let round = if cfg.preset == Preset::Nothing { 0 } else { 28 };
        let mut close_drag: Option<(f32, bool)> = None;
        let mut launch: Option<String> = None;
        let mut long: Option<(String, Pos2)> = None;
        let mut first_hit: Option<String> = None;

        Area::new(Id::new("drawer")).order(Order::Foreground).fixed_pos(rect.min).constrain(false).show(ctx, |ui| {
            ui.set_clip_rect(screen);
            ui.painter().rect_filled(rect, CornerRadius { nw: round, ne: round, sw: 0, se: 0 }, pal.sheet);
            ui.allocate_rect(rect, Sense::click_and_drag());

            // grab area + search field
            let head = Rect::from_min_size(pos2(rect.left() + il.max(16.0), rect.top() + it.max(8.0)), vec2(rect.width() - il.max(16.0) - ir.max(16.0), 84.0));
            let handle = ui.interact(Rect::from_min_size(rect.min, vec2(rect.width(), head.bottom() - rect.top() + 4.0)), Id::new("drawer-handle"), Sense::drag());
            if handle.dragged() {
                close_drag = Some((handle.drag_delta().y, false));
            }
            if handle.drag_stopped() {
                let v = ui.input(|i| i.pointer.velocity().y);
                close_drag = Some((v, true));
            }
            ui.painter().rect_filled(Rect::from_center_size(pos2(rect.center().x, head.top() + 2.0), vec2(36.0, 4.0)), 2, pal.dim.gamma_multiply(0.5));
            ui.scope_builder(UiBuilder::new().max_rect(Rect::from_min_size(head.min + vec2(0.0, 14.0), vec2(head.width(), 52.0))), |ui| {
                let te = egui::TextEdit::singleline(&mut self.query)
                    .hint_text("Search apps")
                    .desired_width(f32::INFINITY)
                    .margin(vec2(18.0, 14.0))
                    .font(FontId::proportional(16.0 * cfg.font_scale))
                    .background_color(pal.card);
                let r = ui.add(te);
                if self.focus_search && t > 0.95 {
                    r.request_focus();
                    self.focus_search = false;
                }
                if r.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                    launch = first_hit.clone();
                }
            });

            let list_top = head.bottom() + 6.0;
            let body = Rect::from_min_max(pos2(rect.left() + il.max(8.0), list_top), pos2(rect.right() - ir.max(8.0), rect.bottom() - ib.max(8.0)));
            let q = self.query.trim().to_lowercase();
            let visible: Vec<usize> = (0..self.apps.len()).filter(|&i| {
                let a = &self.apps[i];
                if q.is_empty() { !cfg.hidden.contains(&a.pkg) } else { a.key.contains(&q) }
            }).collect();
            first_hit = visible.first().map(|&i| self.apps[i].pkg.clone());
            if ui.input(|i| i.key_pressed(Key::Enter)) && !q.is_empty() {
                launch = first_hit.clone();
            }

            ui.scope_builder(UiBuilder::new().max_rect(body), |ui| {
                egui::ScrollArea::vertical().auto_shrink(false).scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(ui, |ui| {
                    let w = body.width();
                    match cfg.drawer {
                        DrawerStyle::Grid => {
                            let cols = cfg.cols.max(3);
                            let cw = w / cols as f32;
                            let ch = cfg.icon_size + 44.0;
                            for row in visible.chunks(cols) {
                                let (rr, _) = ui.allocate_exact_size(vec2(w, ch), Sense::hover());
                                if !ui.is_rect_visible(rr) {
                                    continue;
                                }
                                for (c, &ai) in row.iter().enumerate() {
                                    let cell = Rect::from_min_size(rr.min + vec2(c as f32 * cw, 0.0), vec2(cw, ch));
                                    let app = self.apps[ai].clone();
                                    let resp = ui.interact(cell, Id::new(("d", &app.pkg)), Sense::click());
                                    let p = ui.painter_at(cell);
                                    self.paint_cell(&p, cell, &app, resp.is_pointer_button_down_on(), true, &pal, &cfg);
                                    if resp.clicked() {
                                        launch = Some(app.pkg.clone());
                                    }
                                    if resp.long_touched() || resp.secondary_clicked() {
                                        long = Some((app.pkg.clone(), resp.interact_pointer_pos().unwrap_or(cell.center())));
                                    }
                                }
                            }
                        }
                        DrawerStyle::List => {
                            let rh = 62.0;
                            let mut last = ' ';
                            for &ai in &visible {
                                let app = self.apps[ai].clone();
                                let letter = app.key.chars().next().map(|c| if c.is_alphabetic() { c.to_ascii_uppercase() } else { '#' }).unwrap_or('#');
                                if letter != last && q.is_empty() {
                                    last = letter;
                                    let (hr, _) = ui.allocate_exact_size(vec2(w, 34.0), Sense::hover());
                                    if ui.is_rect_visible(hr) {
                                        ui.painter().text(hr.left_center() + vec2(14.0, 6.0), Align2::LEFT_CENTER, letter.to_string(), FontId::monospace(13.0 * cfg.font_scale), pal.accent);
                                    }
                                }
                                let (rr, resp) = ui.allocate_exact_size(vec2(w, rh), Sense::click());
                                if !ui.is_rect_visible(rr) {
                                    continue;
                                }
                                let p = ui.painter_at(rr);
                                if resp.is_pointer_button_down_on() {
                                    p.rect_filled(rr, 10, pal.card);
                                }
                                let ic = Rect::from_center_size(pos2(rr.left() + 14.0 + 22.0, rr.center().y), vec2(44.0, 44.0));
                                widgets::icon(&p, ic, &app, &mut self.icons, &cfg, &pal, 1.0);
                                p.text(pos2(ic.right() + 16.0, rr.center().y), Align2::LEFT_CENTER, &app.label, FontId::proportional(17.0 * cfg.font_scale), pal.text);
                                if resp.clicked() {
                                    launch = Some(app.pkg.clone());
                                }
                                if resp.long_touched() || resp.secondary_clicked() {
                                    long = Some((app.pkg.clone(), resp.interact_pointer_pos().unwrap_or(rr.center())));
                                }
                            }
                        }
                    }
                    ui.add_space(24.0);
                });
            });
        });

        if let Some((dy, stopped)) = close_drag {
            if stopped {
                self.drawer_open = !(self.drawer < 0.7 || dy > 500.0);
                self.gesture = Gesture::None;
                if !self.drawer_open {
                    self.close_drawer(ctx);
                }
            } else {
                self.gesture = Gesture::Drawer;
                self.drawer = (self.drawer - dy / (screen.height() * 0.5)).clamp(0.0, 1.0);
            }
        }
        if let Some(pkg) = launch {
            self.launch(ctx, &pkg);
        }
        if let Some((pkg, pos)) = long {
            self.menu = Some(Menu { target: Target::App(pkg), pos, from_drawer: true });
        }
    }

    // ───────────────────────────── menu ─────────────────────────────

    fn menu_ui(&mut self, ctx: &Context, screen: Rect) {
        let Some(menu) = self.menu.as_ref() else { return };
        let (target, pos, from_drawer) = (menu.target.clone(), menu.pos, menu.from_drawer);
        let pal = self.pal;
        let mut close = false;
        let mut act: Option<MenuAct> = None;

        let bg = Area::new(Id::new("menu-bg")).order(Order::Tooltip).fixed_pos(screen.min).show(ctx, |ui| ui.allocate_rect(screen, Sense::click_and_drag()));
        if bg.inner.clicked() || bg.inner.dragged() {
            close = true;
        }

        let items: Vec<(&str, MenuAct)> = match &target {
            Target::Home => vec![("Settings", MenuAct::Settings), ("Wallpaper", MenuAct::Wallpaper), ("Phone settings", MenuAct::SystemSettings)],
            Target::App(pkg) => {
                let mut v = vec![];
                if self.cfg.on_home(pkg) {
                    v.push(("Remove from home", MenuAct::Remove));
                } else {
                    v.push(("Add to home", MenuAct::AddHome));
                }
                if !self.cfg.dock_apps.contains(pkg) {
                    v.push(("Add to dock", MenuAct::AddDock));
                }
                if from_drawer {
                    v.push(("Hide", MenuAct::Hide));
                }
                v.push(("App info", MenuAct::Info));
                v.push(("Uninstall", MenuAct::Uninstall));
                v
            }
        };
        let width = 210.0;
        let height = items.len() as f32 * 46.0 + 16.0;
        let p = pos2((pos.x - width / 2.0).clamp(12.0, screen.right() - width - 12.0), (pos.y - height - 24.0).max(screen.top() + 48.0).min(screen.bottom() - height - 12.0));
        Area::new(Id::new("menu")).order(Order::Tooltip).fixed_pos(p).show(ctx, |ui| {
            Frame::NONE.fill(pal.sheet).corner_radius(18).stroke(Stroke::new(1.0, pal.glass_line)).inner_margin(8).shadow(egui::epaint::Shadow { offset: [0, 6], blur: 24, spread: 0, color: Color32::from_black_alpha(110) }).show(ui, |ui| {
                ui.set_width(width - 16.0);
                ui.spacing_mut().item_spacing.y = 2.0;
                for (label, a) in items {
                    let danger = matches!(a, MenuAct::Uninstall);
                    let text = egui::RichText::new(label).size(15.5).color(if danger { Color32::from_rgb(240, 90, 90) } else { pal.text });
                    if ui.add_sized([width - 16.0, 42.0], egui::Button::new(text).fill(Color32::TRANSPARENT).stroke(Stroke::NONE).corner_radius(12)).clicked() {
                        act = Some(a);
                    }
                }
            });
        });

        if let Some(a) = act {
            close = true;
            let pkg = if let Target::App(p) = &target { p.clone() } else { String::new() };
            let page = self.page_target.round() as usize;
            match a {
                MenuAct::Settings => self.open_settings(),
                MenuAct::Wallpaper => android::open_wallpaper_picker(),
                MenuAct::SystemSettings => android::open_system_settings(),
                MenuAct::Remove => self.cfg.remove_everywhere(&pkg),
                MenuAct::AddHome => {
                    self.cfg.add_to_home(&pkg, page);
                    self.close_drawer(ctx);
                }
                MenuAct::AddDock => {
                    self.cfg.add_to_dock(&pkg);
                    self.close_drawer(ctx);
                }
                MenuAct::Hide => {
                    self.cfg.remove_everywhere(&pkg);
                    self.cfg.hidden.push(pkg);
                }
                MenuAct::Info => android::app_info(&pkg),
                MenuAct::Uninstall => android::uninstall(&pkg),
            }
            self.dirty = true;
        }
        if close {
            self.menu = None;
        }
    }
}

#[derive(Clone, Copy)]
enum MenuAct {
    Settings,
    Wallpaper,
    SystemSettings,
    Remove,
    AddHome,
    AddDock,
    Hide,
    Info,
    Uninstall,
}

pub struct Layout {
    clock: Rect,
    grid: Rect,
    dock: Rect,
    search: Option<Rect>,
    dots_y: f32,
}

#[allow(dead_code)]
fn _unused(_: LayerId) {}
