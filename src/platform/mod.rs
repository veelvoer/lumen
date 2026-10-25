//! Everything that touches the phone. On Android it goes through JNI; on a desktop (for tests and
//! screenshots) a mock stands in, so the whole UI can be run and looked at without a device.

pub const PACKAGE: &str = "dev.voer.lumen";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Default)]
pub enum Category {
    Social,
    Entertainment,
    Games,
    Photo,
    Productivity,
    Travel,
    News,
    #[default]
    Utilities,
}

impl Category {
    pub const ALL: [Category; 8] = [
        Category::Social,
        Category::Entertainment,
        Category::Games,
        Category::Photo,
        Category::Productivity,
        Category::Travel,
        Category::News,
        Category::Utilities,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Category::Social => "Social",
            Category::Entertainment => "Entertainment",
            Category::Games => "Games",
            Category::Photo => "Photo & Video",
            Category::Productivity => "Productivity",
            Category::Travel => "Travel",
            Category::News => "News & Reading",
            Category::Utilities => "Utilities",
        }
    }

    /// From Android's ApplicationInfo.category, with a guess from the package name when the app says nothing.
    pub fn from_android(c: i32, pkg: &str) -> Category {
        match c {
            0 => return Category::Games,
            1 | 2 => return Category::Entertainment,
            3 => return Category::Photo,
            4 => return Category::Social,
            5 => return Category::News,
            6 => return Category::Travel,
            7 => return Category::Productivity,
            _ => {}
        }
        const GUESS: [(&str, Category); 24] = [
            ("whatsapp", Category::Social),
            ("instagram", Category::Social),
            ("facebook", Category::Social),
            ("telegram", Category::Social),
            ("twitter", Category::Social),
            ("snapchat", Category::Social),
            ("discord", Category::Social),
            ("messag", Category::Social),
            ("youtube", Category::Entertainment),
            ("spotify", Category::Entertainment),
            ("netflix", Category::Entertainment),
            ("music", Category::Entertainment),
            ("photos", Category::Photo),
            ("camera", Category::Photo),
            ("gallery", Category::Photo),
            ("maps", Category::Travel),
            ("uber", Category::Travel),
            ("docs", Category::Productivity),
            ("drive", Category::Productivity),
            ("gmail", Category::Productivity),
            ("calendar", Category::Productivity),
            ("office", Category::Productivity),
            ("news", Category::News),
            ("chrome", Category::Productivity),
        ];
        GUESS.iter().find(|(k, _)| pkg.contains(k)).map(|(_, c)| *c).unwrap_or(Category::Utilities)
    }
}

#[derive(Clone, Debug)]
pub struct App {
    pub pkg: String,
    pub cls: String,
    pub label: String,
    pub key: String, // lowercase label, for sorting and search
    pub category: Category,
    /// when the app was last installed or updated: icons are cached until it changes
    pub stamp: i64,
}

pub struct RawIcon {
    pub size: usize,
    pub rgba: Vec<u8>, // premultiplied
    pub mono: bool,    // only the alpha channel carries the glyph
}

#[derive(Clone, Copy, Default, Debug)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

/// What the phone is, for choosing how much to draw and for bug reports.
#[derive(Clone, Debug, Default)]
pub struct DeviceInfo {
    pub model: String,
    pub manufacturer: String,
    pub sdk: i32,
    pub ram_mb: i64,
    pub low_ram: bool,
    pub refresh_hz: f32,
}

/// A widget another app offers.
#[derive(Clone, Debug)]
pub struct WidgetProvider {
    pub pkg: String,
    pub cls: String,
    pub label: String,
    /// minimum size in dp
    pub w: i32,
    pub h: i32,
    pub configure: bool,
}

impl WidgetProvider {
    /// How many home screen cells it needs (Android's own rule: (dp + 30) / 70).
    pub fn cells(&self) -> (usize, usize) {
        let c = |dp: i32| (((dp + 30) as f32 / 70.0).ceil() as usize).clamp(1, 6);
        (c(self.w), c(self.h))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Battery {
    pub level: u8,
    pub charging: bool,
}

/// Material You colors of the phone (Android 12+). Shades: 0, 10, 50, 100, 200 ... 900, 1000.
#[derive(Clone, Copy, Debug)]
pub struct Dynamic {
    pub accent1: [[u8; 3]; 13],
    pub neutral1: [[u8; 3]; 13],
    pub neutral2: [[u8; 3]; 13],
}

pub const SHADE_100: usize = 3;
pub const SHADE_200: usize = 4;
pub const SHADE_700: usize = 9;
pub const SHADE_800: usize = 10;
pub const SHADE_900: usize = 11;

#[derive(Clone, Copy, Debug)]
pub enum Haptic {
    Lift,
    Drop,
    Tick,
}

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::*;

#[cfg(not(target_os = "android"))]
mod mock;
#[cfg(not(target_os = "android"))]
pub use mock::*;
