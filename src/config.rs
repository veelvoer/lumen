//! Everything the user can change, saved as one JSON file.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Preset {
    Nothing,
    Pixel,
    Ios,
    Custom,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum Clock {
    Off,
    Digital,
    DotMatrix,
    Glance,
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
pub enum Mode {
    Dark,
    Light,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    pub preset: Preset,
    pub mode: Mode,
    pub accent: [u8; 3],
    /// 0 = square, 1 = circle
    pub icon_radius: f32,
    /// dp
    pub icon_size: f32,
    pub cols: usize,
    pub rows: usize,
    pub labels: bool,
    pub mono_icons: bool,
    pub clock: Clock,
    pub search: Search,
    pub drawer: DrawerStyle,
    pub dock: DockStyle,
    pub dock_radius: f32,
    pub font_scale: f32,
    pub wallpaper_dim: f32,
    pub page_dots: bool,
    pub dock_apps: Vec<String>,
    pub pages: Vec<Vec<String>>,
    pub hidden: Vec<String>,
    pub auto_update: bool,
    pub last_check: u64,
    pub seeded: bool,
}

impl Default for Config {
    fn default() -> Self {
        let mut c = Config {
            preset: Preset::Pixel,
            mode: Mode::Dark,
            accent: [138, 180, 248],
            icon_radius: 1.0,
            icon_size: 56.0,
            cols: 5,
            rows: 6,
            labels: true,
            mono_icons: false,
            clock: Clock::Glance,
            search: Search::Bottom,
            drawer: DrawerStyle::Grid,
            dock: DockStyle::Glass,
            dock_radius: 32.0,
            font_scale: 1.0,
            wallpaper_dim: 0.0,
            page_dots: true,
            dock_apps: vec![],
            pages: vec![vec![]],
            hidden: vec![],
            auto_update: true,
            last_check: 0,
            seeded: false,
        };
        c.apply_preset(Preset::Pixel);
        c
    }
}

impl Config {
    /// Changes only the look. Apps and layout of the home screen stay as they are.
    pub fn apply_preset(&mut self, p: Preset) {
        self.preset = p;
        match p {
            Preset::Nothing => {
                self.mode = Mode::Dark;
                self.accent = [215, 25, 33]; // Nothing red
                self.icon_radius = 1.0;
                self.icon_size = 52.0;
                self.cols = 4;
                self.rows = 6;
                self.labels = true;
                self.mono_icons = true;
                self.clock = Clock::DotMatrix;
                self.search = Search::Off;
                self.drawer = DrawerStyle::List;
                self.dock = DockStyle::Bare;
                self.dock_radius = 0.0;
                self.font_scale = 0.95;
                self.wallpaper_dim = 0.35;
                self.page_dots = true;
            }
            Preset::Pixel => {
                self.mode = Mode::Dark;
                self.accent = [138, 180, 248];
                self.icon_radius = 1.0;
                self.icon_size = 56.0;
                self.cols = 5;
                self.rows = 6;
                self.labels = true;
                self.mono_icons = false;
                self.clock = Clock::Glance;
                self.search = Search::Bottom;
                self.drawer = DrawerStyle::Grid;
                self.dock = DockStyle::Glass;
                self.dock_radius = 36.0;
                self.font_scale = 1.0;
                self.wallpaper_dim = 0.0;
                self.page_dots = false;
            }
            Preset::Ios => {
                self.mode = Mode::Dark;
                self.accent = [10, 132, 255];
                self.icon_radius = 0.45; // continuous-corner look
                self.icon_size = 60.0;
                self.cols = 4;
                self.rows = 6;
                self.labels = true;
                self.mono_icons = false;
                self.clock = Clock::Off;
                self.search = Search::Bottom;
                self.drawer = DrawerStyle::Grid;
                self.dock = DockStyle::Glass;
                self.dock_radius = 38.0;
                self.font_scale = 0.92;
                self.wallpaper_dim = 0.0;
                self.page_dots = true;
            }
            Preset::Custom => {}
        }
    }

    pub fn path(dir: &str) -> PathBuf {
        PathBuf::from(dir).join("config.json")
    }

    pub fn load(dir: &str) -> Config {
        std::fs::read_to_string(Self::path(dir))
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &str) {
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let tmp = PathBuf::from(dir).join("config.json.tmp");
            if std::fs::write(&tmp, s).is_ok() {
                let _ = std::fs::rename(tmp, Self::path(dir));
            }
        }
    }

    /// Where an app lives now, so one app is never on the home screen twice.
    pub fn on_home(&self, pkg: &str) -> bool {
        self.pages.iter().flatten().any(|p| p == pkg) || self.dock_apps.iter().any(|p| p == pkg)
    }

    pub fn remove_everywhere(&mut self, pkg: &str) {
        for p in &mut self.pages {
            p.retain(|x| x != pkg);
        }
        self.dock_apps.retain(|x| x != pkg);
    }

    pub fn add_to_home(&mut self, pkg: &str, page: usize) {
        self.remove_everywhere(pkg);
        let per = self.cols * self.rows;
        let mut i = page.min(self.pages.len().saturating_sub(1));
        while self.pages[i].len() >= per {
            i += 1;
            if i == self.pages.len() {
                self.pages.push(vec![]);
            }
        }
        self.pages[i].push(pkg.to_string());
    }

    pub fn add_to_dock(&mut self, pkg: &str) {
        self.remove_everywhere(pkg);
        if self.dock_apps.len() >= 5 {
            self.dock_apps.pop();
        }
        self.dock_apps.push(pkg.to_string());
    }

    /// Drop empty trailing pages and apps that were uninstalled.
    pub fn tidy(&mut self, installed: &dyn Fn(&str) -> bool) {
        for p in &mut self.pages {
            p.retain(|x| installed(x));
        }
        self.dock_apps.retain(|x| installed(x));
        while self.pages.len() > 1 && self.pages.last().is_some_and(|p| p.is_empty()) {
            self.pages.pop();
        }
        if self.pages.is_empty() {
            self.pages.push(vec![]);
        }
    }
}
