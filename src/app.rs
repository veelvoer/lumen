//! The launcher: state, per-frame bookkeeping and the actions the UI can trigger.

use crate::clock::ClockInfo;
use crate::config::{Action, Config, Item, Preset, Search};
use crate::icons::{self, Icons};
use crate::platform::{self, App, Battery, Category, Dynamic, Haptic, Insets};
use crate::theme::{self, Palette};
use crate::update::Updater;
use egui::{Context, Pos2, Rect, Vec2};
use std::collections::HashMap;
use std::sync::mpsc::{channel, Receiver};
use std::time::{Duration, Instant};

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
    EditRemove(String),
}

#[derive(Clone, Debug)]
pub enum Target {
    App { pkg: String, on_home: bool },
    Folder(u32),
    Widget(u32),
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

pub struct Lumen {
    pub cfg: Config,
    pub dir: String,
    pub pal: Palette,
    pub phone_colors: Option<Dynamic>,
    pub system_dark: bool,
    pub apps: Vec<App>,
    pub by_pkg: HashMap<String, usize>,
    apps_rx: Receiver<Vec<App>>,
    pub icons: Option<Icons>,
    pub upd: Updater,
    pub version: String,
    pub screen: Screen,
    pub settings_stack: Vec<crate::settings::SettingsPage>,
    pub dirty: bool,
    pub default_home: bool,

    // home
    pub page: f32,
    pub page_target: f32,
    pub drawer: f32,
    pub drawer_open: bool,
    pub touch: Touch,
    pub anim: HashMap<String, Pos2>,
    pub edit: bool,
    pub open_folder: Option<u32>,
    pub open_category: Option<Category>,
    pub folder_t: f32,
    pub lib_scroll: f32,
    pub menu: Option<Menu>,
    pub dialog: Option<Dialog>,
    pub widget_picker: bool,
    pub search_open: bool,
    pub query: String,
    pub focus_search: bool,
    pub last_tap: Option<(f64, Pos2)>,
    pub toast: Option<(String, f64)>,
    /// clicks right after a long press are not taps
    pub suppress_click: f64,
    pub drawer_scroll_to: Option<f32>,

    // phone state, refreshed now and then
    pub insets: Insets,
    insets_at: Instant,
    pub clock: ClockInfo,
    clock_at: Option<Instant>,
    pub battery: Battery,
    battery_at: Option<Instant>,
    pub badges: HashMap<String, u32>,
    badges_at: Option<Instant>,
    was_focused: bool,
    home_poll_at: Instant,
    last_offset: f32,
    style_key: Option<String>,
}

impl Lumen {
    pub fn new(ctx: &Context, dir: String) -> Lumen {
        let cfg = Config::load(&dir);
        let phone_colors = platform::dynamic_colors();
        let system_dark = platform::is_dark();
        let pal = theme::palette(&cfg, cfg.dark(system_dark), phone_colors);
        let mut l = Lumen {
            pal,
            phone_colors,
            system_dark,
            icons: Some(Icons::new(ctx, icons::Style { shape: cfg.shape, radius: cfg.icon_radius, mono: cfg.themed_icons })),
            dir,
            apps: vec![],
            by_pkg: HashMap::new(),
            apps_rx: spawn_app_scan(ctx),
            upd: Updater::default(),
            version: platform::version_name(),
            screen: Screen::Home,
            settings_stack: vec![],
            dirty: false,
            default_home: platform::is_default_home(),
            page: 0.0,
            page_target: 0.0,
            drawer: 0.0,
            drawer_open: false,
            touch: Touch::Idle,
            anim: HashMap::new(),
            edit: false,
            open_folder: None,
            open_category: None,
            folder_t: 0.0,
            lib_scroll: 0.0,
            menu: None,
            dialog: None,
            widget_picker: false,
            search_open: false,
            query: String::new(),
            focus_search: false,
            last_tap: None,
            toast: None,
            suppress_click: 0.0,
            drawer_scroll_to: None,
            insets: Insets::default(),
            insets_at: Instant::now() - Duration::from_secs(10),
            clock: ClockInfo::default(),
            clock_at: None,
            battery: Battery { level: 100, charging: false },
            battery_at: None,
            badges: HashMap::new(),
            badges_at: None,
            was_focused: true,
            home_poll_at: Instant::now(),
            last_offset: -1.0,
            style_key: None,
            cfg,
        };
        l.restyle(ctx);
        l
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
        self.pal = theme::palette(&self.cfg, self.dark(), self.phone_colors);
        theme::apply(ctx, &self.cfg, &self.pal);
        let mut st = ctx.global_style().as_ref().clone();
        st.spacing.item_spacing = egui::vec2(8.0, 8.0);
        st.spacing.button_padding = egui::vec2(14.0, 10.0);
        st.spacing.interact_size.y = 40.0;
        st.spacing.slider_width = 150.0;
        ctx.set_global_style(st);
        if !ctx.input(|i| i.pointer.any_down()) {
            let style = icons::Style { shape: self.cfg.shape, radius: self.cfg.icon_radius, mono: self.cfg.themed_icons };
            if let Some(ic) = self.icons.as_mut() {
                ic.restyle(style);
            }
        }
        platform::set_light_bars(self.pal.light);
    }

    // ───────────────────────────── per-frame upkeep ─────────────────────────────

    pub fn tick(&mut self, ctx: &Context) {
        if let Some(ic) = self.icons.as_mut() {
            ic.pump();
        }
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
        if self.insets_at.elapsed() > Duration::from_millis(700) {
            self.insets = platform::insets();
            self.insets_at = Instant::now();
        }
        if self.clock_at.is_none_or(|t| t.elapsed() > Duration::from_millis(500)) {
            self.clock = ClockInfo::now();
            self.clock_at = Some(Instant::now());
        }
        if self.battery_at.is_none_or(|t| t.elapsed() > Duration::from_secs(30)) {
            self.battery = platform::battery();
            self.battery_at = Some(Instant::now());
        }
        if self.cfg.badges != crate::config::Badges::Off && self.badges_at.is_none_or(|t| t.elapsed() > Duration::from_secs(3)) {
            self.badges = platform::badges();
            self.badges_at = Some(Instant::now());
        }

        let focused = ctx.input(|i| i.focused);
        if focused && !self.was_focused {
            self.apps_rx = spawn_app_scan(ctx);
            self.default_home = platform::is_default_home();
            let dark = platform::is_dark();
            if dark != self.system_dark {
                self.system_dark = dark;
                self.restyle(ctx);
            }
        }
        self.was_focused = focused;

        if self.home_poll_at.elapsed() > Duration::from_millis(150) {
            self.home_poll_at = Instant::now();
            if platform::take_home_pressed() {
                self.go_home(ctx);
            }
        }

        let back = ctx.input(|i| i.key_pressed(egui::Key::BrowserBack) || i.key_pressed(egui::Key::Escape));
        if back {
            self.back(ctx);
        }

        // keep the wallpaper in step with the pages
        if self.cfg.wallpaper_scroll && self.screen == Screen::Home {
            let n = self.page_count().max(2) as f32 - 1.0;
            let off = (self.page / n).clamp(0.0, 1.0);
            if (off - self.last_offset).abs() > 0.004 {
                self.last_offset = off;
                platform::set_wallpaper_offset(off);
            }
        }

        let key = format!("{:?}{:?}{:?}", self.cfg.preset, self.cfg.mode, self.system_dark);
        if self.style_key.as_deref() != Some(&key) {
            self.style_key = Some(key);
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
        if self.dialog.is_some() {
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
        } else if self.edit {
            self.edit = false;
        } else {
            self.page_target = 0.0;
        }
    }

    pub fn page_count(&self) -> usize {
        self.cfg.pages.len() + usize::from(self.cfg.app_library)
    }

    pub fn close_drawer(&mut self, ctx: &Context) {
        self.drawer_open = false;
        self.query.clear();
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
            for a in &self.apps {
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
        if let Some(a) = self.app(pkg) {
            platform::launch(a);
            *self.cfg.launches.entry(pkg.to_string()).or_default() += 1;
            self.dirty = true;
        }
        self.menu = None;
        self.open_folder = None;
        self.open_category = None;
        self.search_open = false;
        self.query.clear();
        self.close_drawer(ctx);
        self.drawer = 0.0;
    }

    pub fn run_action(&mut self, ctx: &Context, a: Action) {
        match a {
            Action::None => {}
            Action::Drawer => self.drawer_open = true,
            Action::Search => self.open_search(),
            Action::Notifications => platform::expand_notifications(),
            Action::QuickSettings => platform::expand_quick_settings(),
            Action::LockScreen => {
                if !platform::lock_screen() {
                    platform::ask_lock_access();
                    self.toast(ctx, "Turn on Lumen in Accessibility to lock with a double tap");
                }
            }
            Action::Settings => self.open_settings(),
            Action::Edit => self.edit = true,
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
        platform::haptic(h);
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
        self.tick(&ctx);
        match self.screen {
            Screen::Home => self.home(root, &ctx),
            Screen::Settings => crate::settings::show(self, root),
        }
        self.save_if_dirty();
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
