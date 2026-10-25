//! Fonts: the presets bring their own (Google Sans for Pixel, Inter for iOS, DM Mono for Nothing's small
//! labels), the phone's fonts fill in other scripts, and Phosphor supplies every icon.

use crate::config::{Config, FontChoice, Preset};
use egui::{Context, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

pub const ICONS: &str = "icons";
pub const ICONS_FILL: &str = "icons-fill";

fn add(defs: &mut FontDefinitions, name: &str, data: &'static [u8]) {
    defs.font_data.insert(name.into(), Arc::new(FontData::from_static(data)));
}

pub const CJK: u8 = 1;
pub const ARABIC: u8 = 2;
pub const DEVANAGARI: u8 = 4;
pub const THAI: u8 = 8;
pub const HEBREW: u8 = 16;

/// Which scripts beyond the bundled Latin, Greek and Cyrillic a text needs.
pub fn scripts_in(text: &str) -> u8 {
    let mut m = 0;
    for c in text.chars() {
        let u = c as u32;
        m |= match u {
            0x2E80..=0x9FFF | 0xAC00..=0xD7AF | 0xF900..=0xFAFF | 0xFF00..=0xFFEF | 0x3040..=0x30FF => CJK,
            0x0600..=0x06FF | 0x0750..=0x077F | 0xFB50..=0xFDFF | 0xFE70..=0xFEFF => ARABIC,
            0x0900..=0x097F => DEVANAGARI,
            0x0E00..=0x0E7F => THAI,
            0x0590..=0x05FF => HEBREW,
            _ => 0,
        };
    }
    m
}

/// Bundled fonts always; the phone's big fonts for other scripts only when something on screen needs them
/// (a Chinese font is 16 MB: reading it on every start would slow every phone down for nobody).
pub fn install(ctx: &Context, scripts: u8) {
    let mut defs = FontDefinitions::default();
    add(&mut defs, "gs", include_bytes!("../assets/fonts/GoogleSans-Regular.ttf"));
    add(&mut defs, "gs-m", include_bytes!("../assets/fonts/GoogleSans-Medium.ttf"));
    add(&mut defs, "inter", include_bytes!("../assets/fonts/Inter-Regular.ttf"));
    add(&mut defs, "inter-s", include_bytes!("../assets/fonts/Inter-Semibold.ttf"));
    add(&mut defs, "mono", include_bytes!("../assets/fonts/DMMono-Regular.ttf"));
    add(&mut defs, "mono-m", include_bytes!("../assets/fonts/DMMono-Medium.ttf"));
    add(&mut defs, "ph", include_bytes!("../assets/Phosphor.ttf"));
    add(&mut defs, "ph-fill", include_bytes!("../assets/Phosphor-Fill.ttf"));

    let wanted: [(u8, &[&str]); 5] = [
        (CJK, &["/system/fonts/NotoSansCJK-Regular.ttc", "/system/fonts/NotoSansSC-Regular.otf", "/system/fonts/NotoSerifCJK-Regular.ttc"]),
        (ARABIC, &["/system/fonts/NotoSansArabic-Regular.ttf", "/system/fonts/NotoNaskhArabic-Regular.ttf"]),
        (DEVANAGARI, &["/system/fonts/NotoSansDevanagari-Regular.ttf"]),
        (THAI, &["/system/fonts/NotoSansThai-Regular.ttf"]),
        (HEBREW, &["/system/fonts/NotoSansHebrew-Regular.ttf"]),
    ];
    let mut system: Vec<String> = vec![];
    for (bit, paths) in wanted {
        if scripts & bit == 0 {
            continue;
        }
        if let Some((i, data)) = paths.iter().enumerate().find_map(|(i, p)| std::fs::read(p).ok().map(|d| (i, d))) {
            let name = format!("sys{bit}-{i}");
            defs.font_data.insert(name.clone(), Arc::new(FontData::from_owned(data)));
            system.push(name);
        }
    }

    let family = |defs: &mut FontDefinitions, name: &str, first: &str| {
        let mut v = vec![first.to_string()];
        v.extend(system.iter().cloned());
        defs.families.insert(FontFamily::Name(name.into()), v);
    };
    family(&mut defs, "gs", "gs");
    family(&mut defs, "gs-m", "gs-m");
    family(&mut defs, "inter", "inter");
    family(&mut defs, "inter-s", "inter-s");
    family(&mut defs, "mono", "mono");
    family(&mut defs, "mono-m", "mono-m");
    defs.families.insert(FontFamily::Name(ICONS.into()), vec!["ph".into()]);
    defs.families.insert(FontFamily::Name(ICONS_FILL.into()), vec!["ph-fill".into()]);
    let mut prop = vec!["gs".to_string()];
    prop.extend(system.iter().cloned());
    defs.families.insert(FontFamily::Proportional, prop);
    defs.families.insert(FontFamily::Monospace, vec!["mono".to_string()]);
    ctx.set_fonts(defs);
}

/// Which bundled font a config uses: (regular, medium) for body text and the same for labels.
fn names(cfg: &Config) -> (&'static str, &'static str, &'static str, &'static str) {
    match cfg.font {
        FontChoice::GoogleSans => ("gs", "gs-m", "gs", "gs-m"),
        FontChoice::Inter => ("inter", "inter-s", "inter", "inter-s"),
        FontChoice::Mono => ("mono", "mono-m", "mono", "mono-m"),
        FontChoice::System => ("gs", "gs-m", "gs", "gs-m"),
        FontChoice::Preset => match cfg.preset {
            Preset::Ios => ("inter", "inter-s", "inter", "inter-s"),
            Preset::Nothing => ("gs", "gs-m", "mono", "mono-m"),
            _ => ("gs", "gs-m", "gs", "gs-m"),
        },
    }
}

pub fn body(cfg: &Config, size: f32, medium: bool) -> FontId {
    let (r, m, _, _) = names(cfg);
    FontId::new(size, FontFamily::Name((if medium { m } else { r }).into()))
}

pub fn label(cfg: &Config, size: f32, medium: bool) -> FontId {
    let (_, _, r, m) = names(cfg);
    FontId::new(size, FontFamily::Name((if medium { m } else { r }).into()))
}

pub fn mono(size: f32, medium: bool) -> FontId {
    FontId::new(size, FontFamily::Name((if medium { "mono-m" } else { "mono" }).into()))
}

pub fn icon(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICONS.into()))
}

pub fn icon_fill(size: f32) -> FontId {
    FontId::new(size, FontFamily::Name(ICONS_FILL.into()))
}
