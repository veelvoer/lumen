//! The launcher: state, per-frame bookkeeping and the actions the UI can trigger.

use crate::anim::{self, Feel, Spring, Spring2};
use crate::bridge::{Bridge, Snapshot};
use crate::clock::ClockInfo;
use crate::config::{Action, Config, Item, Preset, Search};
use crate::icons::{self, Icons};
use crate::platform::{self, App, Battery, Category, Dynamic, Haptic, Insets};
use crate::theme::{self, Palette};
use crate::update::Updater;
use egui::{Context, Pos2, Rect, Vec2};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};

#[derive(PartialEq, Clone, Copy, Debug)]
pub enum Screen {
    Home,
    Settings,
}

/// Where a dragged item came from, so it can go back if the drop fails.
#[derive(Clone, Debug, PartialEq)]
pub enum Source {
    Page { page: usize, col: usize, row: usize },
    Dock(usize),
    Folder(u32),
    Drawer,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Hover {
    None,
    Cell { page: usize, col: usize, row: usize },
    Dock(usize),
    /// drop on this item to make a folder or add to it
    Merge(String),
    Remove,
    Info,
    Uninstall,
}

#[derive(Clone, Debug)]
pub struct Drag {
    pub item: Item,
    pub from: Source,
    pub pos: Pos2,
    /// finger position minus the center of the item when it was picked up
    pub grab: Vec2,
    pub hover: Hover,
    pub hover_since: f64,
    pub edge: Option<(f64, i32)>,
    /// the item the finger has been dwelling on (for making folders)
    pub cand: Option<String>,
}

#[derive(Clone, Debug)]
pub enum Touch {
    Idle,
    /// finger is down on the home screen, not decided yet
    Down { start: Pos2, t0: f64, hit: Hit },
    /// long press happened: item is lifted and its menu is open; moving turns it into a drag
    Lifted { start: Pos2, item: Item, from: Source, rect: Rect },
    Pager,
    DrawerPull,
    LibraryScroll,
    /// the swipe-down action fired; wait for the finger to lift
    Spent,
    /// dragging one edge of a selected widget: 0 left, 1 top, 2 right, 3 bottom
    Resize(u8),
    Drag(Drag),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Hit {
    None,
    /// an item on the current page or dock, by key
    Item(String),
    FolderApp(u32, String),
    LibraryApp(String),
    LibraryGroup(Category),
    Search,
    EditDone,
    EditAdd,
    EditSettings,
    EditRemove(String),
}

#[derive(Clone, Debug)]
pub enum Target {
    App { pkg: String, on_home: bool },
    Folder(u32),
    Widget(u32),
    AppWidget(u32),
    Home,
}

#[derive(Clone, Debug)]
pub struct Menu {
    pub target: Target,
    pub anchor: Rect,
}

#[derive(Clone, Debug)]
pub enum Dialog {
    RenameApp { pkg: String, text: String },
    RenameFolder { id: u32, text: String },
}

/// The config, shared cheaply: copying it for a frame costs a pointer, and changing it copies only if someone still holds an old copy.
#[derive(Clone)]
pub struct Cfg(std::rc::Rc<Config>);

impl Cfg {
    pub fn new(c: Config) -> Cfg {
        Cfg(std::rc::Rc::new(c))
    }
}

impl std::ops::Deref for Cfg {
    type Target = Config;
    fn deref(&self) -> &Config {
        &self.0
    }
}

impl std::ops::DerefMut for Cfg {
    fn deref_mut(&mut self) -> &mut Config {
        std::rc::Rc::make_mut(&mut self.0)
    }
}

/// An item that is leaving: it shrinks away (removed) or flies into a folder (merged).
pub struct Ghost {
    pub item: Item,
    pub from: Rect,
    pub to: Rect,
    pub t: Spring,
    /// from/to are the icon squares themselves, not whole grid cells
    pub icon_space: bool,
}

/// State of the letter scroller in the drawer.
#[derive(Default)]
pub struct Scrub {
    pub act: Spring,
    pub y: Spring,
    pub last: Option<usize>,
}

pub struct Lumen {
    pub cfg: Cfg,
    pub dir: String,
    pub pal: Palette,
    pub phone_colors: Option<Dynamic>,
    pub font_scripts: u8,
    pub glass: crate::glass::Glass,
    pub wallpaper_access: bool,
    pub wallpaper_live: bool,
    probed: bool,
    pub device: crate::platform::DeviceInfo,
    pub gpu: String,
    /// light mode is on: fewer effects, so weak phones stay smooth
    pub light: bool,
    last_offset_at: f64,
    last_hr: Option<bool>,
    scan_pending: bool,
    last_scan: std::time::Instant,
    /// 1 right after coming back from an app, then eases to 0: the home screen settles in
    pub resume_t: Spring,
    pub edit_t: Spring,
    last_launch: f64,
    pub system_dark: bool,
    pub wallpaper_dark_text: bool,
    pub apps: std::rc::Rc<Vec<App>>,
    pub by_pkg: HashMap<String, usize>,
    apps_rx: Receiver<Vec<App>>,
    pub icons: Option<Icons>,
    pub upd: Updater,
    pub version: String,
    pub screen: Screen,
    pub settings_stack: Vec<crate::settings::SettingsPage>,
    pub settings_view: Vec<crate::settings::SettingsPage>,
    pub dirty: bool,
    pub default_home: bool,

    // home
    pub page: f32,
    pub page_target: f32,
    pub drawer: f32,
    pub drawer_open: bool,
    pub touch: Touch,
    pub item_sp: HashMap<String, Spring2>,
    pub scale_sp: HashMap<String, Spring>,
    pub page_v: f32,
    pub drawer_v: f32,
    pub folder_v: f32,
    pub folder_origin: Option<Rect>,
    pub menu_t: Spring,
    pub screen_t: Spring,
    pub depth: Spring,
    pub feel: Feel,
    pub bridge: Bridge,
    pub press_key: Option<String>,
    pub vel_smooth: egui::Vec2,
    pub edit: bool,
    pub ghosts: Vec<Ghost>,
    pub prev_items: HashMap<String, (Item, Rect)>,
    pub items_ready: u32,
    /// key of the app widget whose size is being changed
    pub resize: Option<String>,
    pub open_folder: Option<u32>,
    pub open_category: Option<Category>,
    pub folder_t: f32,
    pub lib_max: f32,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub widget_picker: bool,
    /// patch notes being shown: (version, lines)
    pub whats_new: Option<Vec<(String, Vec<String>)>>,
    pub notes_t: Spring,
    pub search_t: Spring,
    pub search_from: Option<Rect>,
    pub picker_tab: u8,
    pub search_open: bool,
    pub query: String,
    pub focus_search: bool,
    pub last_tap: Option<(f64, Pos2)>,
    pub toast: Option<(String, f64)>,
    /// clicks right after a long press are not taps
    pub suppress_click: f64,
    pub drawer_scroll_to: Option<f32>,
    pub scrub: Scrub,
    pub scroller: crate::scroll::Scroller,
    pub aw: std::rc::Rc<std::cell::RefCell<crate::appwidgets::AppWidgets>>,
    pub lib_scroller: crate::scroll::Scroller,
    pub picker_scroller: crate::scroll::Scroller,
    pub notes_scroller: crate::scroll::Scroller,
    pub search_scroller: crate::scroll::Scroller,
    pub settings_scrollers: Vec<crate::scroll::Scroller>,

    // phone state, refreshed now and then
    pub insets: Insets,
    pub clock: ClockInfo,
    pub battery: Battery,
    pub badges: HashMap<String, u32>,
    was_focused: bool,
    last_offset: f32,
    last_blur: i32,
    last_bars: (bool, bool, bool),
    last_light: Option<bool>,
    pub perf: crate::perf::Perf,
    /// seconds one frame may take (1 / refresh rate)
    pub frame_budget: f32,
    style_key: Option<String>,
}

impl Lumen {
    pub fn new(ctx: &Context, dir: String) -> Lumen {
        let cfg = Config::load(&dir);
        let phone_colors = platform::dynamic_colors();
        let system_dark = platform::is_dark();
        let _ = (&cfg.mode, 0);
        let pal = theme::palette(&cfg, cfg.dark(system_dark), phone_colors, false);
        let mut l = Lumen {
            pal,
            phone_colors,
            font_scripts: 0,
            glass: Default::default(),
            wallpaper_access: false,
            wallpaper_live: false,
            probed: false,
            device: platform::device_info(),
            gpu: String::new(),
            light: false,
            last_offset_at: 0.0,
            last_hr: None,
            scan_pending: false,
            last_scan: std::time::Instant::now(),
            resume_t: Spring::at(0.0),
            edit_t: Spring::at(0.0),
            last_launch: -10.0,
            system_dark,
            wallpaper_dark_text: false,
            icons: Some(Icons::new(ctx, icons::Style { shape: cfg.shape, radius: cfg.icon_radius, mono: cfg.themed_icons, px: icons::px_for(cfg.icon_size.max(cfg.dock_icon_size), ctx.pixels_per_point()) }, dir.clone())),
            dir,
            apps: Default::default(),
            by_pkg: HashMap::new(),
            apps_rx: spawn_app_scan(ctx),
            upd: Updater::default(),
            version: platform::version_name(),
            screen: Screen::Home,
            settings_stack: vec![],
            settings_view: vec![],
            dirty: false,
            default_home: platform::is_default_home(),
            page: 0.0,
            page_target: 0.0,
            drawer: 0.0,
            drawer_open: false,
            touch: Touch::Idle,
            item_sp: HashMap::new(),
            scale_sp: HashMap::new(),
            page_v: 0.0,
            drawer_v: 0.0,
            folder_v: 0.0,
            folder_origin: None,
            menu_t: Spring::at(0.0),
            screen_t: Spring::at(0.0),
            depth: Spring::at(0.0),
            feel: anim::feel(&cfg, false),
            bridge: Bridge::new(ctx),
            press_key: None,
            vel_smooth: egui::Vec2::ZERO,
            edit: false,
            ghosts: vec![],
            prev_items: HashMap::new(),
            items_ready: 0,
            resize: None,
            open_folder: None,
            open_category: None,
            folder_t: 0.0,
            lib_max: 0.0,
            menu: None,
            dialog: None,
            widget_picker: false,
            whats_new: None,
            notes_t: Spring::at(0.0),
            search_t: Spring::at(0.0),
            search_from: None,
            picker_tab: 1,
            search_open: false,
            query: String::new(),
            focus_search: false,
            last_tap: None,
            toast: None,
            suppress_click: 0.0,
            drawer_scroll_to: None,
            scrub: Scrub::default(),
            scroller: Default::default(),
            aw: std::rc::Rc::new(std::cell::RefCell::new(crate::appwidgets::AppWidgets::new(ctx))),
            lib_scroller: Default::default(),
            picker_scroller: Default::default(),
            notes_scroller: Default::default(),
            search_scroller: Default::default(),
            settings_scrollers: vec![],
            insets: Insets::default(),
            clock: ClockInfo::default(),
            battery: Battery { level: 100, charging: false },
            badges: HashMap::new(),
            was_focused: true,
            last_offset: -1.0,
            last_blur: -1,
            last_bars: (true, true, true),
            last_light: None,
            perf: Default::default(),
            frame_budget: 1.0 / 60.0,
            style_key: None,
            cfg: Cfg::new(cfg),
        };
        l.restyle(ctx);
        l
    }

    /// Decides whether light mode is on, from the setting, the phone, and what Auto learned.
    pub fn update_light(&mut self) {
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(8);
        let weak = crate::perf::weak_device(&self.device, &self.gpu, cores);
        self.light = match self.cfg.perf_mode {
            crate::config::PerfMode::Full => false,
            crate::config::PerfMode::Light => true,
            crate::config::PerfMode::Auto => weak || self.cfg.light_learned,
        };
        crate::perf::set_light(self.light);
        if self.last_hr != Some(!self.light) {
            self.last_hr = Some(!self.light);
            self.bridge.high_refresh(!self.light);
        }
    }

    /// The name of the graphics chip, known once the window exists.
    pub fn set_gpu(&mut self, ctx: &Context, gpu: &str) {
        self.gpu = gpu.to_string();
        self.update_light();
        self.restyle(ctx);
    }

    pub fn app(&self, pkg: &str) -> Option<&App> {
        self.by_pkg.get(pkg).map(|&i| &self.apps[i])
    }

    pub fn label_of(&self, pkg: &str) -> String {
        self.app(pkg).map(|a| self.cfg.label(a).to_string()).unwrap_or_default()
    }

    pub fn dark(&self) -> bool {
        self.cfg.dark(self.system_dark)
    }

    /// Applies colors and icon style after any change in the config.
    pub fn restyle(&mut self, ctx: &Context) {
        self.pal = theme::palette(&self.cfg, self.dark(), self.phone_colors, self.wallpaper_dark_text);
        theme::apply(ctx, &self.cfg, &self.pal);
        let mut st = ctx.global_style().as_ref().clone();
        st.spacing.item_spacing = egui::vec2(8.0, 8.0);
        st.spacing.button_padding = egui::vec2(14.0, 10.0);
        st.spacing.interact_size.y = 40.0;
        st.spacing.slider_width = 150.0;
        ctx.set_global_style(st);
        if !ctx.input(|i| i.pointer.any_down()) {
            let style = icons::Style { shape: self.cfg.shape, radius: self.cfg.icon_radius, mono: self.cfg.themed_icons, px: { let px = icons::px_for(self.cfg.icon_size.max(self.cfg.dock_icon_size), ctx.pixels_per_point()); if self.light { px.min(112) } else { px } } };
            if let Some(ic) = self.icons.as_mut() {
                ic.restyle(style);
            }
        }
        self.update_light();
        self.feel = anim::feel(&self.cfg, self.light);
    }

    // ───────────────────────────── per-frame upkeep ─────────────────────────────

    pub fn tick(&mut self, ctx: &Context) {
        // a tablet gets a bigger grid and bigger icons once, before the first layout is made
        if !self.cfg.adapted {
            let r = ctx.content_rect();
            if r.width() > 1.0 {
                self.cfg.adapted = true;
                if r.width().min(r.height()) >= 600.0 {
                    self.cfg.big = true;
                    self.cfg.cols = 7;
                    self.cfg.rows = 5;
                    self.cfg.icon_size = 68.0;
                    self.cfg.dock_icon_size = 68.0;
                    self.cfg.dock_slots = 6;
                    self.cfg.drawer_cols = 8;
                    self.cfg.rotate = true;
                }
                self.dirty = true;
            }
        }
        if let Some(ic) = self.icons.as_mut() {
            ic.pump();
        }
        {
            let idle = matches!(self.touch, Touch::Idle) && self.drawer == 0.0 && !self.drawer_open;
            if let Some(ic) = self.icons.as_mut() {
                ic.prefetch_step(idle);
            }
        }
        self.aw.borrow_mut().pump();
        self.handle_widget_events(ctx);
        if let Ok(mut list) = self.apps_rx.try_recv() {
            list.dedup_by(|a, b| a.pkg == b.pkg);
            self.scan_pending = false;
            // nothing installed, removed or updated since last time: do no work at all
            let same = list.len() == self.apps.len() && list.iter().zip(self.apps.iter()).all(|(a, b)| a.pkg == b.pkg && a.stamp == b.stamp && a.label == b.label);
            if same {
                return self.tick_rest(ctx);
            }
            self.by_pkg = list.iter().enumerate().map(|(i, a)| (a.pkg.clone(), i)).collect();
            self.apps = std::rc::Rc::new(list);
            if !self.cfg.seeded {
                self.seed();
            } else {
                // apps that appeared since last time
                let fresh: Vec<String> = self.apps.iter().filter(|x| !self.cfg.known.contains(&x.pkg) && x.pkg != platform::PACKAGE).map(|x| x.pkg.clone()).collect();
                if !self.cfg.known.is_empty() && self.cfg.auto_add {
                    for pkg in &fresh {
                        self.cfg.insert_auto(Item::App(pkg.clone()));
                    }
                }
            }
            // load a font for a script only when an app name needs it
            let need = self.apps.iter().fold(0u8, |m, x| m | crate::fonts::scripts_in(&x.label));
            if need & !self.font_scripts != 0 {
                self.font_scripts |= need;
                crate::fonts::install(ctx, self.font_scripts);
            }
            self.cfg.known = self.apps.iter().map(|x| x.pkg.clone()).collect();
            // load every icon in the background now, so the drawer never waits for one
            if let Some(ic) = self.icons.as_mut() {
                ic.prefetch_all(&self.apps);
            }
            let known: std::collections::HashSet<&str> = self.apps.iter().map(|a| a.pkg.as_str()).collect();
            self.cfg.tidy(&|p| known.contains(p));
            self.dirty = true;
            ctx.request_repaint();
            self.check_notes();
        }
        self.tick_rest(ctx);
    }

    /// After an update, show what is new once. A fresh install shows nothing.
    fn check_notes(&mut self) {
        if self.cfg.notes_seen == self.version {
            return;
        }
        let seen = self.cfg.notes_seen.clone();
        self.cfg.notes_seen = self.version.clone();
        self.dirty = true;
        if !seen.is_empty() {
            let n = crate::notes::since(&seen, &self.version);
            if !n.is_empty() {
                self.whats_new = Some(n);
            }
        }
    }

    pub fn show_current_notes(&mut self) {
        let n = crate::notes::since("", &self.version);
        self.whats_new = Some(n.into_iter().take(1).collect());
    }

    fn tick_rest(&mut self, ctx: &Context) {
        // everything the phone tells us arrives from the background worker; nothing here can stall a frame
        let snap: Snapshot = self.bridge.snapshot();
        if snap.battery.is_some() {
            self.wallpaper_access = snap.wallpaper_access;
            self.wallpaper_live = snap.wallpaper_live;
        }
        // real Liquid Glass needs the wallpaper: only when allowed, not live, and the phone can afford it
        let want_glass = self.cfg.real_glass && !self.light && self.wallpaper_access && !self.wallpaper_live;
        if want_glass {
            self.glass.start(ctx);
            self.glass.poll();
        } else if self.glass.active() || self.glass.status == "on" {
            self.glass.reset();
            self.glass.status.clear();
        }
        if snap.battery.is_some() {
            self.default_home = snap.default_home;
        }
        if snap.refresh_hz >= 30.0 {
            self.frame_budget = 1.0 / snap.refresh_hz.min(144.0);
        }
        self.insets = snap.insets;
        self.clock = ClockInfo::now(snap.tz_offset_ms, match self.cfg.time_format {
            crate::config::TimeFormat::Phone => snap.h24,
            crate::config::TimeFormat::H24 => true,
            crate::config::TimeFormat::H12 => false,
        });
        if let Some(b) = snap.battery {
            self.battery = b;
        }
        if self.cfg.badges != crate::config::Badges::Off {
            self.badges = snap.badges.clone();
        }
        if snap.battery.is_some() && (snap.dark != self.system_dark || snap.dark_text != self.wallpaper_dark_text) {
            self.system_dark = snap.dark;
            self.wallpaper_dark_text = snap.dark_text;
            self.restyle(ctx);
        }
        // dark status bar icons only where the background is light: the settings sheet, or a light wallpaper
        let covered = (self.screen == Screen::Settings && self.screen_t.x > 0.5) || self.drawer > 0.5 || self.search_open;
        let want_light = if covered { self.pal.light } else { self.wallpaper_dark_text && self.cfg.preset != Preset::Ios };
        if self.last_light != Some(want_light) {
            self.last_light = Some(want_light);
            self.bridge.light_bars(want_light);
        }

        let focused = ctx.input(|i| i.focused);
        if focused && !self.was_focused {
            // coming back from an app: look for installed or removed apps, but not more than every 30 seconds and without hurry
            if !self.scan_pending && self.last_scan.elapsed() > std::time::Duration::from_secs(30) {
                self.scan_pending = true;
                self.last_scan = std::time::Instant::now();
                self.apps_rx = spawn_app_scan(ctx);
            }
            self.bridge.high_refresh(!self.light);
        }
        self.was_focused = focused;

        if self.bridge.take_home() {
            self.go_home(ctx);
        }
        if self.bridge.take_settings() {
            self.open_settings();
        }

        let back = ctx.input(|i| i.key_pressed(egui::Key::BrowserBack) || i.key_pressed(egui::Key::Escape));
        if back {
            self.back(ctx);
        }

        // keep the wallpaper in step with the pages, and frost it behind drawers and folders
        if self.cfg.wallpaper_scroll && self.screen == Screen::Home {
            let n = self.page_count().max(2) as f32 - 1.0;
            let off = (self.page / n).clamp(0.0, 1.0);
            let now = ctx.input(|i| i.time);
            if (off - self.last_offset).abs() > 0.004 && (!self.light || now - self.last_offset_at > 0.15) {
                self.last_offset = off;
                self.last_offset_at = now;
                self.bridge.wallpaper_offset(off);
            }
        }
        let k = ctx.pixels_per_point();
        let lib_amount = if self.cfg.app_library { (self.page - (self.cfg.pages.len() as f32 - 1.0)).clamp(0.0, 1.0) } else { 0.0 };
        let amount = self.drawer.clamp(0.0, 1.0).max(self.folder_t.clamp(0.0, 1.0) * 0.7).max(lib_amount * 0.8).max(if self.screen == Screen::Settings { self.screen_t.x.clamp(0.0, 1.0) } else { 0.0 });
        let px = if self.light { 0 } else { ((amount * self.cfg.blur * 28.0 * k) / 4.0).round() as i32 * 4 };
        if px != self.last_blur {
            self.last_blur = px;
            self.bridge.blur(px);
        }
        let bars = (self.cfg.hide_status_bar, self.cfg.hide_nav_bar, self.cfg.rotate);
        if bars != self.last_bars {
            self.last_bars = bars;
            self.bridge.bars(bars.0, bars.1);
            self.bridge.rotate(bars.2);
        }
    }

    /// The Home button: put everything away and go to the first page.
    pub fn go_home(&mut self, ctx: &Context) {
        self.menu = None;
        self.dialog = None;
        self.widget_picker = false;
        self.search_open = false;
        self.open_folder = None;
        self.open_category = None;
        self.edit = false;
        self.screen = Screen::Home;
        self.close_drawer(ctx);
        if matches!(self.touch, Touch::Idle | Touch::Spent) {
            self.page_target = 0.0;
        }
    }

    pub fn back(&mut self, ctx: &Context) {
        if self.whats_new.is_some() {
            self.whats_new = None;
        } else if self.dialog.is_some() {
            self.dialog = None;
        } else if self.menu.is_some() {
            self.menu = None;
        } else if self.widget_picker {
            self.widget_picker = false;
        } else if self.search_open {
            self.search_open = false;
            self.query.clear();
        } else if self.screen == Screen::Settings {
            if self.settings_stack.pop().is_none() {
                self.screen = Screen::Home;
            }
        } else if self.open_folder.is_some() || self.open_category.is_some() {
            self.open_folder = None;
            self.open_category = None;
        } else if self.drawer_open {
            self.close_drawer(ctx);
        } else if self.resize.is_some() {
            self.resize = None;
        } else if self.edit {
            self.edit = false;
        } else {
            self.page_target = 0.0;
        }
    }

    /// A widget from another app finished being added (or failed): put it on the home screen.
    fn handle_widget_events(&mut self, ctx: &Context) {
        let events: Vec<crate::appwidgets::Event> = {
            let aw = self.aw.borrow();
            let mut g = aw.events.lock().unwrap();
            std::mem::take(&mut *g)
        };
        for e in events {
            match e {
                crate::appwidgets::Event::Ready { wid, provider } => {
                    let (w, h) = provider.cells();
                    let (w, h) = (w.min(self.cfg.cols), h.min(self.cfg.rows));
                    let id = self.cfg.new_id();
                    let item = Item::AppWidget { id, wid, pkg: provider.pkg.clone(), cls: provider.cls.clone(), w, h };
                    let pi = (self.page.round() as usize).min(self.cfg.pages.len().saturating_sub(1));
                    if let Some((c, r)) = self.cfg.free_spot(pi, w, h) {
                        self.cfg.place(pi, c, r, item);
                    } else {
                        self.cfg.insert_auto(item);
                    }
                    self.dirty = true;
                    self.widget_picker = false;
                    self.edit = false;
                    self.toast(ctx, "Widget added");
                }
                crate::appwidgets::Event::Failed(msg) => self.toast(ctx, &msg),
            }
        }
    }

    /// Bakes the glass panels of this frame (iOS dock and search capsule) when the wallpaper is readable.
    pub fn bake_glass(&mut self, ctx: &Context, geo: &crate::home::Geo) {
        crate::glass::clear_ready();
        if !self.glass.active() || self.cfg.preset != Preset::Ios {
            return;
        }
        let count = self.page_count().max(2) as f32 - 1.0;
        let pan = if self.cfg.wallpaper_scroll { (self.page / count).clamp(0.0, 1.0) } else { 0.5 };
        let dark = self.dark();
        let tint = if dark { egui::Color32::from_rgba_unmultiplied(24, 24, 34, 46) } else { egui::Color32::from_rgba_unmultiplied(255, 255, 255, 60) };
        if self.cfg.dock == crate::config::DockStyle::Glass {
            self.glass.bake(ctx, "dock", geo.dock, self.cfg.dock_radius, geo.screen, pan, tint, dark);
        }
        if let Some(s) = geo.search {
            if s.width() < 160.0 {
                self.glass.bake(ctx, "pill", s, s.height() / 2.0, geo.screen, pan, tint, dark);
            }
        }
    }

    pub fn page_count(&self) -> usize {
        self.cfg.pages.len() + usize::from(self.cfg.app_library)
    }

    pub fn close_drawer(&mut self, ctx: &Context) {
        self.drawer_open = false;
        self.query.clear();
        self.scroller.reset();
        self.focus_search = false;
        ctx.memory_mut(|m| m.stop_text_input());
    }

    pub fn toast(&mut self, ctx: &Context, text: &str) {
        self.toast = Some((text.to_string(), ctx.input(|i| i.time) + 2.2));
    }

    // ───────────────────────────── first start ─────────────────────────────

    /// First start: the usual apps on the dock and first page so the home screen is not empty.
    pub fn reseed(&mut self) {
        self.seed();
    }

    fn seed(&mut self) {
        let mut dock: Vec<String> = vec![];
        for (action, data) in [
            ("android.intent.action.DIAL", None),
            ("android.intent.action.SENDTO", Some("smsto:")),
            ("android.intent.action.VIEW", Some("https://example.com")),
            ("android.media.action.IMAGE_CAPTURE", None),
        ] {
            if let Some(p) = platform::default_for(action, data) {
                if self.by_pkg.contains_key(&p) && !dock.contains(&p) {
                    dock.push(p);
                }
            }
        }
        const WELL_KNOWN: [&str; 20] = [
            "com.google.android.apps.photos", "com.google.android.apps.maps", "com.google.android.gm", "com.google.android.youtube",
            "com.android.vending", "com.android.settings", "com.google.android.calendar", "com.google.android.deskclock",
            "com.android.deskclock", "com.google.android.calculator", "com.spotify.music", "com.whatsapp",
            "org.mozilla.firefox", "com.android.chrome", "com.google.android.apps.messaging", "com.instagram.android",
            "com.google.android.apps.docs", "com.netflix.mediclient", "org.telegram.messenger", "com.discord",
        ];
        let mut list: Vec<String> = WELL_KNOWN.iter().filter(|p| self.by_pkg.contains_key(**p) && !dock.contains(&p.to_string())).map(|p| p.to_string()).collect();
        if list.len() < 10 {
            for a in self.apps.iter() {
                if list.len() >= 14 {
                    break;
                }
                if !list.contains(&a.pkg) && !dock.contains(&a.pkg) {
                    list.push(a.pkg.clone());
                }
            }
        }
        self.cfg.dock_items = dock.into_iter().take(self.cfg.dock_slots).map(Item::App).collect();
        let preset = self.cfg.preset;
        self.cfg.pages = vec![crate::config::Page::default()];
        let mut items: Vec<Item> = list.into_iter().map(Item::App).collect();
        // signature widgets first
        let kinds: &[crate::config::WidgetKind] = match preset {
            Preset::Nothing => &[crate::config::WidgetKind::DotClock],
            Preset::Pixel => &[crate::config::WidgetKind::Glance],
            Preset::Ios => &[crate::config::WidgetKind::Calendar, crate::config::WidgetKind::ClockAnalog],
            Preset::Custom => &[],
        };
        let mut front = vec![];
        for k in kinds {
            let id = self.cfg.new_id();
            front.push(Item::Widget { id, kind: *k });
        }
        front.append(&mut items);
        self.cfg.repack(front);
        self.cfg.seeded = true;
    }

    // ───────────────────────────── actions ─────────────────────────────

    pub fn launch(&mut self, ctx: &Context, pkg: &str) {
        self.launch_at(ctx, pkg, None);
    }

    /// Starts an app. With the icon's rectangle (in points) the system zooms out of the icon.
    pub fn launch_at(&mut self, ctx: &Context, pkg: &str, from: Option<Rect>) {
        // a second tap while an app is already opening must do nothing
        let now = ctx.input(|i| i.time);
        if now - self.last_launch < 0.12 {
            return;
        }
        self.last_launch = now;
        if pkg == platform::PACKAGE {
            self.open_settings();
            self.close_drawer(ctx);
            self.search_open = false;
            self.menu = None;
            return;
        }
        if let Some(a) = self.app(pkg) {
            log::info!("launch {pkg}");
            let k = ctx.pixels_per_point();
            match (from, self.cfg.launch_anim) {
                (Some(r), crate::config::LaunchAnim::Zoom | crate::config::LaunchAnim::Reveal) => {
                    let reveal = self.cfg.launch_anim == crate::config::LaunchAnim::Reveal;
                    let (a, rect) = (a.clone(), [(r.left() * k) as i32, (r.top() * k) as i32, (r.width() * k) as i32, (r.height() * k) as i32]);
                    std::thread::spawn(move || platform::launch_from(&a, rect, reveal));
                }
                _ => {
                    let a = a.clone();
                    std::thread::spawn(move || platform::launch(&a));
                }
            }
            *self.cfg.launches.entry(pkg.to_string()).or_default() += 1;
            self.dirty = true;
        }
        self.menu = None;
        self.open_folder = None;
        self.open_category = None;
        self.search_open = false;
        self.query.clear();
        self.close_drawer(ctx);
    }

    pub fn run_action(&mut self, ctx: &Context, a: Action) {
        match a {
            Action::None => {}
            Action::Drawer => self.drawer_open = true,
            Action::Search => self.open_search(),
            Action::Notifications => crate::bridge::bg(platform::expand_notifications),
            Action::QuickSettings => crate::bridge::bg(platform::expand_quick_settings),
            Action::LockScreen => {
                if !platform::lock_screen() {
                    platform::ask_lock_access();
                    self.toast(ctx, "Turn on Lumen in Accessibility to lock with a double tap");
                }
            }
            Action::Settings => self.open_settings(),
            Action::Edit => self.edit = true,
            Action::Menu => {
                let c = ctx.content_rect().center();
                self.menu = Some(Menu { target: Target::Home, anchor: Rect::from_center_size(c, egui::vec2(10.0, 10.0)) });
            }
        }
    }

    /// The search bar: Pixel and Nothing search inside the drawer, iOS has its own spotlight.
    pub fn open_search(&mut self) {
        if matches!(self.cfg.preset, Preset::Ios | Preset::Custom) || self.cfg.search == Search::Off && self.cfg.preset == Preset::Ios {
            self.search_open = true;
            self.query.clear();
        } else {
            self.drawer_open = true;
        }
        self.focus_search = true;
    }

    pub fn open_settings(&mut self) {
        self.screen = Screen::Settings;
        self.settings_stack.clear();
        self.menu = None;
        self.drawer_open = false;
        self.edit = false;
    }

    pub fn haptic(&self, h: Haptic) {
        if self.cfg.haptics {
            self.bridge.haptic(h);
        }
    }

    /// Apps by how often they were launched, then name. Used for suggestions.
    pub fn suggested(&self, n: usize) -> Vec<&App> {
        let mut v: Vec<&App> = self.apps.iter().filter(|a| self.cfg.launches.get(&a.pkg).copied().unwrap_or(0) > 0 && !self.cfg.hidden.contains(&a.pkg)).collect();
        v.sort_by_key(|a| std::cmp::Reverse(self.cfg.launches.get(&a.pkg).copied().unwrap_or(0)));
        if v.len() < n {
            for p in ["com.android.chrome", "com.google.android.apps.photos", "com.google.android.apps.maps", "com.google.android.youtube", "com.spotify.music", "com.whatsapp", "com.google.android.gm"] {
                if let Some(a) = self.app(p) {
                    if !v.iter().any(|x| x.pkg == a.pkg) {
                        v.push(a);
                    }
                }
            }
        }
        v.truncate(n);
        v
    }

    pub fn save_if_dirty(&mut self) {
        if self.dirty {
            self.cfg.save(&self.dir);
            self.dirty = false;
        }
    }

    pub fn frame(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        self.log_frame_times(&ctx);
        {
            let _s = crate::perf::span(crate::perf::Sec::Tick);
            self.tick(&ctx);
        }
        let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
        self.screen_t.target = if self.screen == Screen::Settings { 1.0 } else { 0.0 };
        let mut busy = self.screen_t.step(dt, self.feel.screen);
        busy |= self.depth.step(dt, self.feel.screen);
        if busy {
            ctx.request_repaint();
        }
        if self.screen_t.x < 0.999 || self.screen == Screen::Home {
            self.home(root, &ctx);
        }
        if self.screen == Screen::Settings || self.screen_t.x > 0.002 {
            let _s = crate::perf::span(crate::perf::Sec::Settings);
            crate::settings::show(self, root);
        }
        self.save_if_dirty();
    }
}

impl Lumen {
    /// Writes one line a second to the log when frames were slow, so jank can be found on a real phone.
    fn log_frame_times(&mut self, ctx: &Context) {
        let dt = ctx.input(|i| i.unstable_dt);
        self.perf.frame(dt, self.frame_budget);
        // the phone cannot keep up: lighten up on our own, and remember it
        if self.cfg.perf_mode == crate::config::PerfMode::Auto && !self.light && self.perf.slow_run >= 45 {
            self.cfg.light_learned = true;
            self.dirty = true;
            self.update_light();
            self.restyle(ctx);
            self.toast(ctx, "Switched to smooth mode so it keeps up");
        }
    }

    /// After a panic in a frame: drop whatever gesture or popup was in flight so the next frame starts clean.
    pub fn recover(&mut self) {
        log::error!("recovered from a panic in a frame");
        self.touch = Touch::Idle;
        self.menu = None;
        self.dialog = None;
        self.edit = false;
        self.press_key = None;
    }

    /// A plain-text report about this phone and this launcher, for bug reports.
    pub fn diagnostics(&self, ctx: &Context) -> String {
        let (pages, icons, px) = self.icons.as_ref().map(|i| i.stats()).unwrap_or((0, 0, 0));
        let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(0);
        let r = ctx.content_rect();
        let k = ctx.pixels_per_point();
        format!(
            "Lumen {}\nDevice: {} {} (Android API {})\nMemory: {} MB{}, {} cores\nScreen: {:.0}x{:.0} px, density {:.2}, {:.0} Hz\nGraphics: {}\nMode: {:?}, light is {}{}\nWallpaper: {}\nFrame time while moving: {:.1} ms (budget {:.1} ms)\nApps: {}, icons loaded {} ({} px, {} atlas pages)\nPreset: {:?}, grid {}x{}, pages {}",
            self.version,
            self.device.manufacturer,
            self.device.model,
            self.device.sdk,
            self.device.ram_mb,
            if self.device.low_ram { " (low-memory device)" } else { "" },
            cores,
            r.width() * k,
            r.height() * k,
            k,
            self.device.refresh_hz.max(1.0).max(1.0 / self.frame_budget.max(0.001)),
            if self.gpu.is_empty() { "unknown" } else { &self.gpu },
            self.cfg.perf_mode,
            if self.light { "on" } else { "off" },
            if self.cfg.light_learned { " (learned from slow frames)" } else { "" },
            if self.glass.active() { "glass active".to_string() } else { self.glass.status.clone() },
            self.perf.recent * 1000.0,
            self.frame_budget * 1000.0,
            self.apps.len(),
            icons,
            px,
            pages,
            self.cfg.preset,
            self.cfg.cols,
            self.cfg.rows,
            self.cfg.pages.len(),
        )
    }

    pub fn note_cpu(&mut self, c: f32) {
        self.perf.cpu(c);
    }
}

pub fn spawn_app_scan(ctx: &Context) -> Receiver<Vec<App>> {
    let (tx, rx) = channel();
    let ctx = ctx.clone();
    std::thread::spawn(move || {
        let _ = tx.send(platform::launcher_apps());
        ctx.request_repaint();
    });
    rx
}

pub fn rect_contains(r: Rect, p: Pos2) -> bool {
    r.contains(p)
}
