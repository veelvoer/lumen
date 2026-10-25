//! Desktop stand-in for the phone: a fixed list of apps with drawn icons.

use super::{App, Battery, Category, Dynamic, Haptic, Insets, RawIcon};
use std::collections::HashMap;

const APPS: [(&str, &str, [u8; 3], Category); 45] = [
    ("dev.voer.lumen", "Lumen", [20, 22, 30], Category::Utilities),
    ("com.android.settings", "Settings", [96, 125, 139], Category::Utilities),
    ("com.google.android.dialer", "Phone", [66, 133, 244], Category::Utilities),
    ("com.google.android.apps.messaging", "Messages", [26, 115, 232], Category::Social),
    ("com.android.chrome", "Chrome", [234, 67, 53], Category::Productivity),
    ("com.google.android.GoogleCamera", "Camera", [52, 168, 83], Category::Photo),
    ("com.google.android.apps.photos", "Photos", [251, 188, 5], Category::Photo),
    ("com.google.android.apps.maps", "Maps", [52, 168, 83], Category::Travel),
    ("com.google.android.gm", "Gmail", [219, 68, 55], Category::Productivity),
    ("com.google.android.youtube", "YouTube", [255, 0, 0], Category::Entertainment),
    ("com.spotify.music", "Spotify", [30, 215, 96], Category::Entertainment),
    ("com.whatsapp", "WhatsApp", [37, 211, 102], Category::Social),
    ("com.instagram.android", "Instagram", [193, 53, 132], Category::Social),
    ("com.android.vending", "Play Store", [0, 150, 136], Category::Utilities),
    ("com.google.android.calendar", "Calendar", [66, 133, 244], Category::Productivity),
    ("com.google.android.deskclock", "Clock", [38, 50, 56], Category::Utilities),
    ("com.google.android.calculator", "Calculator", [69, 90, 100], Category::Utilities),
    ("com.google.android.apps.docs", "Drive", [251, 188, 5], Category::Productivity),
    ("com.netflix.mediaclient", "Netflix", [229, 9, 20], Category::Entertainment),
    ("org.telegram.messenger", "Telegram", [42, 171, 238], Category::Social),
    ("com.discord", "Discord", [88, 101, 242], Category::Social),
    ("com.twitter.android", "X", [20, 20, 20], Category::Social),
    ("com.reddit.frontpage", "Reddit", [255, 69, 0], Category::Social),
    ("com.google.android.apps.fitness", "Fit", [0, 188, 212], Category::Utilities),
    ("com.google.android.keep", "Keep", [255, 193, 7], Category::Productivity),
    ("com.google.android.contacts", "Contacts", [66, 133, 244], Category::Utilities),
    ("com.android.documentsui", "Files", [33, 150, 243], Category::Utilities),
    ("com.google.android.apps.translate", "Translate", [66, 133, 244], Category::Utilities),
    ("com.amazon.mShop.android.shopping", "Amazon", [255, 153, 0], Category::Utilities),
    ("com.ubercab", "Uber", [20, 20, 20], Category::Travel),
    ("com.supercell.clashofclans", "Clash of Clans", [255, 160, 0], Category::Games),
    ("com.mojang.minecraftpe", "Minecraft", [90, 150, 60], Category::Games),
    ("com.king.candycrushsaga", "Candy Crush", [233, 30, 99], Category::Games),
    ("com.google.android.apps.news", "News", [66, 133, 244], Category::News),
    ("com.google.android.apps.podcasts", "Podcasts", [156, 39, 176], Category::Entertainment),
    ("com.slack", "Slack", [74, 21, 75], Category::Productivity),
    ("us.zoom.videomeetings", "Zoom", [45, 140, 255], Category::Productivity),
    ("com.google.android.apps.tasks", "Tasks", [66, 133, 244], Category::Productivity),
    ("com.google.android.apps.wallet", "Wallet", [33, 33, 33], Category::Utilities),
    ("com.google.android.apps.authenticator2", "Authenticator", [66, 133, 244], Category::Utilities),
    ("com.google.android.recorder", "Recorder", [244, 67, 54], Category::Utilities),
    ("com.google.android.apps.googleassistant", "Assistant", [66, 133, 244], Category::Utilities),
    ("com.bitwarden", "Bitwarden", [23, 93, 220], Category::Utilities),
    ("dev.voer.hearth", "Hearth", [255, 112, 67], Category::Productivity),
    ("org.mozilla.firefox", "Firefox", [255, 113, 57], Category::Productivity),
];

pub fn init_mock() {}
pub fn files_dir() -> String {
    let d = std::env::var("LUMEN_DIR").unwrap_or_else(|_| "/tmp/lumen-mock".into());
    let _ = std::fs::create_dir_all(&d);
    d
}
pub fn cache_dir() -> String {
    files_dir()
}
pub fn version_name() -> String {
    env!("CARGO_PKG_VERSION").into()
}

pub fn launcher_apps() -> Vec<App> {
    let mut v: Vec<App> = APPS.iter().map(|(p, l, _, c)| App { pkg: p.to_string(), cls: "Main".into(), label: l.to_string(), key: l.to_lowercase(), category: *c }).collect();
    v.sort_by(|a, b| a.key.cmp(&b.key));
    v
}

pub fn default_for(action: &str, _data: Option<&str>) -> Option<String> {
    Some(match action {
        "android.intent.action.DIAL" => "com.google.android.dialer",
        "android.intent.action.SENDTO" => "com.google.android.apps.messaging",
        "android.intent.action.VIEW" => "com.android.chrome",
        "android.media.action.IMAGE_CAPTURE" => "com.google.android.GoogleCamera",
        _ => return None,
    }
    .into())
}

pub fn launch(a: &App) {
    log::info!("launch {}", a.pkg);
}
pub fn uninstall(p: &str) {
    log::info!("uninstall {p}");
}
pub fn app_info(p: &str) {
    log::info!("app info {p}");
}
pub fn open_url(u: &str) {
    log::info!("open {u}");
}
pub fn search_web(q: &str) {
    log::info!("web search {q}");
}
pub fn open_wallpaper_picker() {}
pub fn open_system_settings() {}
pub fn is_default_home() -> bool {
    true
}
pub fn pick_default_home() {}
pub fn insets() -> Insets {
    Insets { left: 0.0, top: 72.0, right: 0.0, bottom: 63.0 }
}
pub fn is_24h() -> bool {
    true
}
pub fn local_millis() -> i64 {
    // a fixed moment keeps screenshots stable: Sat 24 Oct 2026, 23:47 local
    1_792_878_420_000 + 2 * 3_600_000
}
pub fn set_light_bars(_: bool) {}
pub fn can_install_packages() -> bool {
    true
}
pub fn ask_install_permission() {}
pub fn install_apk(_: &str) -> Result<(), String> {
    Ok(())
}
pub fn expand_notifications() {}
pub fn expand_quick_settings() {}
pub fn battery() -> Battery {
    Battery { level: 82, charging: false }
}
pub fn dynamic_colors() -> Option<Dynamic> {
    // a blue Material You palette
    let ramp = |base: [f32; 3]| {
        let l = [1.0, 0.99, 0.95, 0.9, 0.8, 0.7, 0.6, 0.5, 0.4, 0.3, 0.2, 0.1, 0.0f32];
        let mut o = [[0u8; 3]; 13];
        for (i, t) in l.iter().enumerate() {
            for c in 0..3 {
                o[i][c] = if *t >= 0.5 { (base[c] + (255.0 - base[c]) * (t - 0.5) * 2.0) as u8 } else { (base[c] * t * 2.0) as u8 };
            }
        }
        o
    };
    Some(Dynamic { accent1: ramp([66.0, 133.0, 244.0]), neutral1: ramp([120.0, 124.0, 132.0]), neutral2: ramp([112.0, 120.0, 140.0]) })
}
pub fn haptic(_: Haptic) {}
pub fn badges() -> HashMap<String, u32> {
    HashMap::from([("com.whatsapp".to_string(), 3), ("com.google.android.gm".to_string(), 12), ("org.telegram.messenger".to_string(), 1)])
}
pub fn badge_access() -> bool {
    true
}
pub fn ask_badge_access() {}
pub fn lock_screen() -> bool {
    true
}
pub fn lock_enabled() -> bool {
    true
}
pub fn ask_lock_access() {}
pub fn take_home_pressed() -> bool {
    false
}
pub fn set_wallpaper_offset(_: f32) {}
pub fn clipboard_get() -> String {
    std::fs::read_to_string(format!("{}/clip.txt", files_dir())).unwrap_or_default()
}
pub fn clipboard_set(s: &str) {
    let _ = std::fs::write(format!("{}/clip.txt", files_dir()), s);
}

pub fn load_icon(a: &App, px: i32, want_mono: bool) -> Option<RawIcon> {
    let (_, _, col, _) = APPS.iter().find(|(p, ..)| *p == a.pkg)?;
    let n = px as usize;
    let mut rgba = vec![0u8; n * n * 4];
    let c = n as f32 / 2.0;
    for y in 0..n {
        for x in 0..n {
            let (dx, dy) = (x as f32 + 0.5 - c, y as f32 + 0.5 - c);
            let d = (dx * dx + dy * dy).sqrt() / c;
            let o = (y * n + x) * 4;
            let mono_shape = (d > 0.28 && d < 0.5) || ((dx.abs() < c * 0.1) && dy.abs() < c * 0.42);
            if want_mono {
                if mono_shape {
                    rgba[o..o + 4].copy_from_slice(&[255, 255, 255, 255]);
                }
            } else {
                let shade = 1.0 - 0.25 * (y as f32 / n as f32);
                let ring = d > 0.28 && d < 0.5;
                let px = if ring { [255, 255, 255] } else { [(col[0] as f32 * shade) as u8, (col[1] as f32 * shade) as u8, (col[2] as f32 * shade) as u8] };
                rgba[o..o + 3].copy_from_slice(&px);
                rgba[o + 3] = 255;
            }
        }
    }
    Some(RawIcon { size: n, rgba, mono: want_mono })
}

pub fn is_dark() -> bool {
    std::env::var("LUMEN_LIGHT").is_err()
}

pub fn tz_offset_ms() -> i64 {
    2 * 3_600_000
}
pub fn set_blur(_: i32) {}
pub fn set_system_bars(_: bool, _: bool) {}
pub fn set_rotation(_: bool) {}
pub fn take_settings_requested() -> bool {
    false
}
pub fn launch_from(a: &App, _: [i32; 4], _: bool) {
    log::info!("launch {} with animation", a.pkg);
}

pub fn now_ms() -> i64 {
    1_792_878_420_000
}
