//! Everything the user can change, and the home screen layout, saved as one JSON file.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Nothing,
    Pixel,
    Ios,
    Custom,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    Auto,
    Dark,
    Light,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    Circle,
    Squircle,
    Rounded,
    Square,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Search {
    Off,
    Top,
    Bottom,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DrawerStyle {
    Grid,
    List,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DockStyle {
    Glass,
    Solid,
    Bare,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Badges {
    Off,
    Dot,
    Count,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum FontChoice {
    Preset,
    System,
    GoogleSans,
    Inter,
    Mono,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Action {
    None,
    Drawer,
    Search,
    Notifications,
    QuickSettings,
    LockScreen,
    Settings,
    Edit,
    Menu,
}

impl Action {
    pub const ALL: [(Action, &'static str); 9] = [
        (Action::None, "Nothing"),
        (Action::Drawer, "App drawer"),
        (Action::Search, "Search"),
        (Action::Notifications, "Notifications"),
        (Action::QuickSettings, "Quick settings"),
        (Action::LockScreen, "Lock screen"),
        (Action::Settings, "Lumen settings"),
        (Action::Edit, "Edit home screen"),
        (Action::Menu, "Home menu"),
    ];
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum AnimStyle {
    Auto,
    Smooth,
    Bouncy,
    Snappy,
    Off,
}

/// How much work Lumen does to look good. Auto picks from the phone and from how it is keeping up.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PerfMode {
    Auto,
    Full,
    Light,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum PageEffect {
    Slide,
    Parallax,
    Zoom,
    Fade,
    Stack,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum LaunchAnim {
    Zoom,
    Reveal,
    None,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sort {
    Alphabetical,
    MostUsed,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DotsStyle {
    Dots,
    Bars,
    Numbers,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum TimeFormat {
    Phone,
    H12,
    H24,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum SearchPos {
    Auto,
    Top,
    Bottom,
}

/// Things that live on the home screen besides apps.
#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum WidgetKind {
    /// Pixel "At a glance": the date, big
    Glance,
    /// Stacked hours over minutes
    ClockStack,
    ClockDigital,
    ClockAnalog,
    /// Nothing: dot-matrix clock card
    DotClock,
    DotDate,
    DotBattery,
    /// iOS: weekday and day number
    Calendar,
    Battery,
    SearchBar,
}

impl WidgetKind {
    pub const ALL: [WidgetKind; 10] = [
        WidgetKind::Glance,
        WidgetKind::ClockStack,
        WidgetKind::ClockDigital,
        WidgetKind::ClockAnalog,
        WidgetKind::DotClock,
        WidgetKind::DotDate,
        WidgetKind::DotBattery,
        WidgetKind::Calendar,
        WidgetKind::Battery,
        WidgetKind::SearchBar,
    ];

    pub fn name(self) -> &'static str {
        match self {
            WidgetKind::Glance => "At a glance",
            WidgetKind::ClockStack => "Clock (stacked)",
            WidgetKind::ClockDigital => "Clock (digital)",
            WidgetKind::ClockAnalog => "Clock (analog)",
            WidgetKind::DotClock => "Dot-matrix clock",
            WidgetKind::DotDate => "Dot-matrix date",
            WidgetKind::DotBattery => "Dot-matrix battery",
            WidgetKind::Calendar => "Calendar",
            WidgetKind::Battery => "Battery",
            WidgetKind::SearchBar => "Search bar",
        }
    }

    /// Width and height in grid cells.
    pub fn span(self, cols: usize) -> (usize, usize) {
        let w = |n: usize| n.min(cols);
        match self {
            WidgetKind::Glance => (w(cols), 1),
            WidgetKind::ClockStack => (2, 2),
            WidgetKind::ClockDigital => (w(4), 2),
            WidgetKind::ClockAnalog => (2, 2),
            WidgetKind::DotClock => (w(4), 2),
            WidgetKind::DotDate => (2, 2),
            WidgetKind::DotBattery => (2, 2),
            WidgetKind::Calendar => (2, 2),
            WidgetKind::Battery => (2, 2),
            WidgetKind::SearchBar => (w(4), 1),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub enum Item {
    App(String),
    Folder { id: u32, name: String, apps: Vec<String> },
    Widget { id: u32, kind: WidgetKind },
    /// a widget that belongs to another app; `wid` is Android's widget id
    AppWidget { id: u32, wid: i32, pkg: String, cls: String, w: usize, h: usize },
}

impl Item {
    /// Stable identity, also used to animate an item from where it was to where it is now.
    pub fn key(&self) -> String {
        match self {
            Item::App(p) => format!("app:{p}"),
            Item::Folder { id, .. } => format!("folder:{id}"),
            Item::Widget { id, .. } => format!("widget:{id}"),
            Item::AppWidget { id, .. } => format!("awidget:{id}"),
        }
    }

    pub fn span(&self, cols: usize) -> (usize, usize) {
        match self {
            Item::Widget { kind, .. } => kind.span(cols),
            Item::AppWidget { w, h, .. } => ((*w).clamp(1, cols), (*h).clamp(1, 8)),
            _ => (1, 1),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, PartialEq, Debug)]
pub struct Placed {
    pub item: Item,
    pub col: usize,
    pub row: usize,
}

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Page {
    pub items: Vec<Placed>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub preset: Preset,
    pub mode: Mode,
    pub accent: [u8; 3],
    /// Use the phone's Material You colors as the accent (Android 12+).
    pub dynamic: bool,
    pub shape: Shape,
    /// Corner roundness for `Shape::Rounded`, 0 = square, 1 = circle
    pub icon_radius: f32,
    /// dp
    pub icon_size: f32,
    /// icon size actually used: icon_size, shrunk when the screen is too small for it (not saved)
    #[serde(skip)]
    pub icon_eff: f32,
    pub cols: usize,
    pub rows: usize,
    pub dock_slots: usize,
    pub labels: bool,
    pub themed_icons: bool,
    pub font: FontChoice,
    pub font_scale: f32,
    pub search: Search,
    pub drawer: DrawerStyle,
    pub drawer_cols: usize,
    pub suggestions: bool,
    pub app_library: bool,
    pub dock: DockStyle,
    pub dock_radius: f32,
    pub wallpaper_dim: f32,
    pub wallpaper_scroll: bool,
    pub page_dots: bool,
    pub badges: Badges,
    pub swipe_up: Action,
    pub swipe_down: Action,
    pub double_tap: Action,
    pub lock_layout: bool,
    // motion
    pub anim_style: AnimStyle,
    pub anim_speed: f32,
    pub page_effect: PageEffect,
    pub launch_anim: LaunchAnim,
    pub blur: f32,
    pub haptics: bool,
    /// read the wallpaper (with permission) to make real Liquid Glass
    pub real_glass: bool,
    /// the one-time tip about wallpaper access has been shown
    pub glass_hinted: bool,
    /// highest refresh rate to ask for; 0 = the screen's best. Lowered on its own when 120 Hz cannot be held.
    pub refresh_cap: f32,
    pub perf_mode: PerfMode,
    /// Auto mode saw the phone struggle and switched to Light; remembered between starts
    pub light_learned: bool,
    // layout details
    pub margin_side: f32,
    pub margin_top: f32,
    pub margin_bottom: f32,
    pub dock_icon_size: f32,
    pub dock_labels: bool,
    pub dots_style: DotsStyle,
    pub widget_radius: f32,
    pub folder_cols: usize,
    // text
    pub label_size: f32,
    pub label_shadow: bool,
    pub time_format: TimeFormat,
    // drawer
    pub drawer_opacity: f32,
    pub drawer_sort: Sort,
    pub drawer_search: SearchPos,
    pub drawer_scrubber: bool,
    // behavior
    pub auto_add: bool,
    pub long_press_empty: Action,
    pub pinch: Action,
    pub hide_status_bar: bool,
    pub hide_nav_bar: bool,
    pub rotate: bool,
    pub auto_update: bool,
    pub last_check: u64,
    pub seeded: bool,
    /// the layout was already adapted to a big screen once
    pub adapted: bool,
    /// a tablet: presets keep a bigger grid
    pub big: bool,
    /// the last version whose patch notes were shown
    pub notes_seen: String,
    /// which one-time setting updates were applied
    pub migrated: u32,

    pub pages: Vec<Page>,
    pub dock_items: Vec<Item>,
    pub hidden: Vec<String>,
    /// every app seen so far, to notice new ones
    pub known: Vec<String>,
    pub names: HashMap<String, String>,
    pub launches: HashMap<String, u32>,
    pub next_id: u32,
}

impl Default for Config {
    fn default() -> Self {
        let mut c = Config {
            preset: Preset::Pixel,
            mode: Mode::Auto,
            accent: [138, 180, 248],
            dynamic: true,
            shape: Shape::Circle,
            icon_radius: 1.0,
            icon_size: 56.0,
            icon_eff: 0.0,
            cols: 5,
            rows: 5,
            dock_slots: 4,
            labels: true,
            themed_icons: false,
            font: FontChoice::Preset,
            font_scale: 1.0,
            search: Search::Bottom,
            drawer: DrawerStyle::Grid,
            drawer_cols: 5,
            suggestions: true,
            app_library: false,
            dock: DockStyle::Bare,
            dock_radius: 32.0,
            wallpaper_dim: 0.0,
            wallpaper_scroll: true,
            page_dots: true,
            badges: Badges::Dot,
            swipe_up: Action::Drawer,
            swipe_down: Action::Notifications,
            double_tap: Action::None,
            lock_layout: false,
            anim_style: AnimStyle::Auto,
            anim_speed: 1.0,
            page_effect: PageEffect::Slide,
            launch_anim: LaunchAnim::Zoom,
            blur: 1.0,
            haptics: true,
            real_glass: true,
            glass_hinted: false,
            refresh_cap: 0.0,
            perf_mode: PerfMode::Auto,
            light_learned: false,
            margin_side: 0.0,
            margin_top: 0.0,
            margin_bottom: 0.0,
            dock_icon_size: 56.0,
            dock_labels: false,
            dots_style: DotsStyle::Dots,
            widget_radius: 24.0,
            folder_cols: 3,
            label_size: 12.0,
            label_shadow: true,
            time_format: TimeFormat::Phone,
            drawer_opacity: 1.0,
            drawer_sort: Sort::Alphabetical,
            drawer_search: SearchPos::Auto,
            drawer_scrubber: true,
            auto_add: false,
            long_press_empty: Action::Menu,
            pinch: Action::None,
            hide_status_bar: false,
            hide_nav_bar: false,
            rotate: false,
            auto_update: true,
            last_check: 0,
            seeded: false,
            adapted: false,
            big: false,
            notes_seen: String::new(),
            migrated: 0,
            pages: vec![Page::default()],
            dock_items: vec![],
            hidden: vec![],
            known: vec![],
            names: HashMap::new(),
            launches: HashMap::new(),
            next_id: 1,
        };
        c.apply_preset(Preset::Pixel);
        c
    }
}

impl Config {
    /// Changes the look and the signature widgets. The user's apps stay, they are only repacked.
    pub fn apply_preset(&mut self, p: Preset) {
        self.preset = p;
        match p {
            Preset::Nothing => {
                self.mode = Mode::Dark;
                self.accent = [215, 25, 33]; // Nothing red
                self.dynamic = false;
                self.shape = Shape::Circle;
                self.icon_size = 54.0;
                self.cols = 4;
                self.rows = 6;
                self.dock_slots = 4;
                self.labels = true;
                self.themed_icons = true;
                self.search = Search::Off;
                self.drawer = DrawerStyle::Grid;
                self.drawer_cols = 4;
                self.suggestions = false;
                self.app_library = false;
                self.dock = DockStyle::Bare;
                self.font_scale = 0.92;
                self.wallpaper_dim = 0.0;
                self.page_dots = true;
                self.badges = Badges::Dot;
                self.swipe_down = Action::Notifications;
                self.swipe_up = Action::Drawer;
                self.dock_icon_size = 54.0;
                self.dots_style = DotsStyle::Bars;
                self.widget_radius = 30.0;
                self.label_size = 11.0;
                self.label_shadow = false;
                self.drawer_opacity = 1.0;
                self.drawer_scrubber = true;
                self.page_effect = PageEffect::Slide;
                self.launch_anim = LaunchAnim::Zoom;
                self.long_press_empty = Action::Menu;
                self.anim_style = AnimStyle::Auto;
            }
            Preset::Pixel => {
                self.mode = Mode::Auto;
                self.accent = [138, 180, 248];
                self.dynamic = true;
                self.shape = Shape::Circle;
                self.icon_size = 56.0;
                self.cols = 5;
                self.rows = 5;
                self.dock_slots = 5;
                self.labels = true;
                self.themed_icons = false;
                self.search = Search::Bottom;
                self.drawer = DrawerStyle::Grid;
                self.drawer_cols = 5;
                self.suggestions = true;
                self.app_library = false;
                self.dock = DockStyle::Bare;
                self.font_scale = 1.0;
                self.wallpaper_dim = 0.0;
                self.page_dots = true;
                self.badges = Badges::Dot;
                self.swipe_down = Action::Notifications;
                self.swipe_up = Action::Drawer;
                self.dock_icon_size = 56.0;
                self.dots_style = DotsStyle::Dots;
                self.widget_radius = 28.0;
                self.label_size = 12.0;
                self.label_shadow = true;
                self.drawer_opacity = 1.0;
                self.drawer_scrubber = true;
                self.page_effect = PageEffect::Parallax;
                self.launch_anim = LaunchAnim::Reveal;
                self.long_press_empty = Action::Menu;
                self.anim_style = AnimStyle::Auto;
            }
            Preset::Ios => {
                self.mode = Mode::Auto;
                self.accent = [10, 132, 255];
                self.dynamic = false;
                self.shape = Shape::Squircle;
                self.icon_size = 60.0;
                self.cols = 4;
                self.rows = 6;
                self.dock_slots = 4;
                self.labels = true;
                self.themed_icons = false;
                self.search = Search::Bottom;
                self.drawer = DrawerStyle::Grid;
                self.drawer_cols = 4;
                self.suggestions = false;
                self.app_library = true;
                self.dock = DockStyle::Glass;
                self.dock_radius = 38.0;
                self.font_scale = 0.92;
                self.wallpaper_dim = 0.0;
                self.page_dots = true;
                self.badges = Badges::Count;
                self.swipe_down = Action::Search;
                self.swipe_up = Action::None;
                self.dock_icon_size = 60.0;
                self.dots_style = DotsStyle::Dots;
                self.widget_radius = 22.0;
                self.label_size = 11.5;
                self.label_shadow = true;
                self.drawer_opacity = 1.0;
                self.drawer_scrubber = true;
                self.page_effect = PageEffect::Slide;
                self.launch_anim = LaunchAnim::Zoom;
                self.long_press_empty = Action::Edit;
                self.anim_style = AnimStyle::Auto;
            }
            Preset::Custom => return,
        }
        if self.big {
            self.cols = 7;
            self.rows = 5;
            self.icon_size = self.icon_size.max(64.0);
            self.dock_icon_size = self.dock_icon_size.max(64.0);
            self.dock_slots = 6;
            self.drawer_cols = 8;
        }
        self.dock_items.truncate(self.dock_slots);
        // swap the signature widgets
        let sig: &[WidgetKind] = match p {
            Preset::Nothing => &[WidgetKind::DotClock],
            Preset::Pixel => &[WidgetKind::Glance],
            Preset::Ios => &[WidgetKind::Calendar, WidgetKind::ClockAnalog],
            Preset::Custom => &[],
        };
        let all = [WidgetKind::Glance, WidgetKind::DotClock, WidgetKind::Calendar, WidgetKind::ClockAnalog];
        let mut items: Vec<Item> = self
            .reading_order()
            .into_iter()
            .filter(|it| !matches!(it, Item::Widget { kind, .. } if all.contains(kind) && !sig.contains(kind)))
            .collect();
        let mut front: Vec<Item> = vec![];
        for k in sig {
            if !items.iter().any(|it| matches!(it, Item::Widget { kind, .. } if kind == k)) {
                let id = self.new_id();
                front.push(Item::Widget { id, kind: *k });
            }
        }
        front.append(&mut items);
        self.repack(front);
    }

    pub fn new_id(&mut self) -> u32 {
        self.next_id += 1;
        self.next_id
    }

    pub fn path(dir: &str) -> PathBuf {
        PathBuf::from(dir).join("config.json")
    }

    pub fn load(dir: &str) -> Config {
        let mut c: Config = std::fs::read_to_string(Self::path(dir)).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default();
        if c.migrated < 1 {
            c.migrated = 1;
            c.drawer_scrubber = true;
        }
        if c.migrated < 4 {
            c.migrated = 4;
            if c.drawer_opacity < 1.0 && c.drawer_opacity > 0.9 {
                c.drawer_opacity = 1.0;
            }
        }
        if c.migrated < 3 {
            c.migrated = 3;
            if c.preset == Preset::Pixel {
                c.rows = 5;
                c.dock_slots = 5;
            }
        }
        if c.migrated < 2 {
            c.migrated = 2;
            if c.preset == Preset::Ios {
                c.search = Search::Bottom;
            }
        }
        c
    }

    pub fn save(&self, dir: &str) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = PathBuf::from(dir).join("config.json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(tmp, Self::path(dir));
            }
        }
    }

    // ───────────────────────────── layout ─────────────────────────────

    pub fn eff(&self) -> f32 {
        if self.icon_eff > 0.0 { self.icon_eff } else { self.icon_size }
    }

    pub fn per_page(&self) -> usize {
        self.cols * self.rows
    }

    fn span_of(&self, it: &Item) -> (usize, usize) {
        let (w, h) = it.span(self.cols);
        (w.min(self.cols), h.min(self.rows))
    }

    /// Is the area free on `page`? `ignore` is an item that is about to move and does not count.
    pub fn fits(&self, page: usize, col: usize, row: usize, w: usize, h: usize, ignore: Option<&str>) -> bool {
        if col + w > self.cols || row + h > self.rows {
            return false;
        }
        let Some(pg) = self.pages.get(page) else { return true };
        !pg.items.iter().any(|p| {
            if ignore.is_some_and(|k| p.item.key() == k) {
                return false;
            }
            let (pw, ph) = self.span_of(&p.item);
            col < p.col + pw && p.col < col + w && row < p.row + ph && p.row < row + h
        })
    }

    pub fn free_spot(&self, page: usize, w: usize, h: usize) -> Option<(usize, usize)> {
        for r in 0..self.rows {
            for c in 0..self.cols {
                if self.fits(page, c, r, w, h, None) {
                    return Some((c, r));
                }
            }
        }
        None
    }

    /// Nearest free spot to (col,row) on this page for something of size w x h.
    pub fn nearest_free(&self, page: usize, col: usize, row: usize, w: usize, h: usize, ignore: Option<&str>) -> Option<(usize, usize)> {
        let mut best: Option<(usize, (usize, usize))> = None;
        for r in 0..self.rows {
            for c in 0..self.cols {
                if self.fits(page, c, r, w, h, ignore) {
                    let d = c.abs_diff(col).pow(2) + r.abs_diff(row).pow(2);
                    if best.is_none_or(|(bd, _)| d < bd) {
                        best = Some((d, (c, r)));
                    }
                }
            }
        }
        best.map(|(_, p)| p)
    }

    pub fn item_at(&self, page: usize, col: usize, row: usize) -> Option<usize> {
        let pg = self.pages.get(page)?;
        pg.items.iter().position(|p| {
            let (w, h) = self.span_of(&p.item);
            col >= p.col && col < p.col + w && row >= p.row && row < p.row + h
        })
    }

    /// Puts an item on the first free spot of any page, adding a page if everything is full.
    pub fn insert_auto(&mut self, item: Item) {
        let (w, h) = self.span_of(&item);
        for pi in 0..self.pages.len() {
            if let Some((c, r)) = self.free_spot(pi, w, h) {
                self.pages[pi].items.push(Placed { item, col: c, row: r });
                return;
            }
        }
        self.pages.push(Page::default());
        self.pages.last_mut().unwrap().items.push(Placed { item, col: 0, row: 0 });
    }

    pub fn place(&mut self, page: usize, col: usize, row: usize, item: Item) {
        while self.pages.len() <= page {
            self.pages.push(Page::default());
        }
        self.pages[page].items.push(Placed { item, col, row });
    }

    /// Every item on the pages, page by page, top to bottom, left to right.
    pub fn reading_order(&self) -> Vec<Item> {
        let mut v: Vec<(usize, usize, usize, Item)> = vec![];
        for (pi, pg) in self.pages.iter().enumerate() {
            for p in &pg.items {
                v.push((pi, p.row, p.col, p.item.clone()));
            }
        }
        v.sort_by_key(|(pi, r, c, _)| (*pi, *r, *c));
        v.into_iter().map(|(_, _, _, i)| i).collect()
    }

    /// Lays exactly these items out again from the top-left, page after page. Used when the grid or the preset changes.
    pub fn repack(&mut self, items: Vec<Item>) {
        self.pages = vec![Page::default()];
        for it in items {
            self.insert_auto(it);
        }
        self.dock_items.truncate(self.dock_slots);
        self.tidy_pages();
    }

    pub fn tidy_pages(&mut self) {
        while self.pages.len() > 1 && self.pages.last().is_some_and(|p| p.items.is_empty()) {
            self.pages.pop();
        }
        if self.pages.is_empty() {
            self.pages.push(Page::default());
        }
    }

    pub fn all_apps_on_home(&self) -> Vec<&str> {
        fn add<'a>(v: &mut Vec<&'a str>, it: &'a Item) {
            match it {
                Item::App(p) => v.push(p.as_str()),
                Item::Folder { apps, .. } => v.extend(apps.iter().map(|s| s.as_str())),
                _ => {}
            }
        }
        let mut v = vec![];
        for pg in &self.pages {
            for p in &pg.items {
                add(&mut v, &p.item);
            }
        }
        for it in &self.dock_items {
            add(&mut v, it);
        }
        v
    }

    pub fn on_home(&self, pkg: &str) -> bool {
        self.all_apps_on_home().contains(&pkg)
    }

    /// Takes an app off the home screen wherever it is. A folder left with one app turns back into that app.
    pub fn remove_app(&mut self, pkg: &str) {
        fn prune(it: Item, pkg: &str) -> Option<Item> {
            match it {
                Item::App(p) if p == pkg => None,
                Item::Folder { id, name, mut apps } => {
                    apps.retain(|a| a != pkg);
                    match apps.len() {
                        0 => None,
                        1 => Some(Item::App(apps.remove(0))),
                        _ => Some(Item::Folder { id, name, apps }),
                    }
                }
                other => Some(other),
            }
        }
        for pg in self.pages.iter_mut() {
            pg.items = pg.items.drain(..).filter_map(|p| prune(p.item, pkg).map(|item| Placed { item, col: p.col, row: p.row })).collect();
        }
        self.dock_items = self.dock_items.drain(..).filter_map(|it| prune(it, pkg)).collect();
    }

    pub fn remove_app_if_app(&mut self, it: &Item) {
        if let Item::App(p) = it {
            self.remove_app(p);
        }
    }

    pub fn remove_key(&mut self, key: &str) -> Option<Item> {
        for pg in self.pages.iter_mut() {
            if let Some(i) = pg.items.iter().position(|p| p.item.key() == key) {
                return Some(pg.items.remove(i).item);
            }
        }
        if let Some(i) = self.dock_items.iter().position(|it| it.key() == key) {
            return Some(self.dock_items.remove(i));
        }
        None
    }

    /// Drop everything that is no longer installed.
    pub fn tidy(&mut self, installed: &dyn Fn(&str) -> bool) {
        let gone: Vec<String> = self.all_apps_on_home().into_iter().filter(|p| !installed(p)).map(String::from).collect();
        for g in gone {
            self.remove_app(&g);
        }
        self.hidden.retain(|p| installed(p));
        self.tidy_pages();
    }

    pub fn label<'a>(&'a self, app: &'a crate::platform::App) -> &'a str {
        self.names.get(&app.pkg).map(|s| s.as_str()).unwrap_or(&app.label)
    }

    pub fn dark(&self, system_dark: bool) -> bool {
        match self.mode {
            Mode::Dark => true,
            Mode::Light => false,
            Mode::Auto => system_dark,
        }
    }
}
