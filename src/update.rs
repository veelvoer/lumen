//! Updates from GitHub releases, no cable needed. Same idea as Hearth: look at the newest release,
//! download the APK, let Android's installer put it over the running version.

use crate::android;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const REPO: &str = "veelvoer/lumen";
const UA: &str = concat!("Lumen/", env!("CARGO_PKG_VERSION"));

#[derive(Clone, Debug, Default)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub apk_url: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct State {
    pub busy: bool,
    pub progress: Option<u8>,
    pub available: Option<Release>,
    pub checked: bool,
    pub message: String,
}

#[derive(Clone, Default)]
pub struct Updater(Arc<Mutex<State>>);

fn parts(v: &str) -> Vec<u64> {
    v.trim_start_matches('v').split('.').map(|p| p.parse().unwrap_or(0)).collect()
}

pub fn newer(a: &str, b: &str) -> bool {
    let (x, y) = (parts(a), parts(b));
    (0..3).map(|i| (x.get(i).copied().unwrap_or(0), y.get(i).copied().unwrap_or(0))).find(|(p, q)| p != q).is_some_and(|(p, q)| p > q)
}

/// Release notes as plain lines: no # or ** marks, bullets as dots.
pub fn plain(md: &str) -> Vec<String> {
    md.lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| {
            let l = l.trim_start_matches('#').trim_start();
            let l = if let Some(r) = l.strip_prefix("- ").or_else(|| l.strip_prefix("* ")) { format!("• {r}") } else { l.to_string() };
            l.replace("**", "").replace('`', "")
        })
        .collect()
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(20)))
        .user_agent(UA)
        .build()
        .into()
}

/// The API allows 60 looks an hour per network; the release page's redirect has no such limit.
fn latest_via_redirect() -> Result<Release, String> {
    let a: ureq::Agent = ureq::Agent::config_builder().max_redirects(0).http_status_as_error(false).timeout_global(Some(Duration::from_secs(15))).user_agent(UA).build().into();
    let r = a.get(format!("https://github.com/{REPO}/releases/latest")).call().map_err(|e| e.to_string())?;
    let loc = r.headers().get("location").and_then(|v| v.to_str().ok()).ok_or_else(|| format!("GitHub said {}", r.status()))?;
    let tag = loc.rsplit("/tag/").next().unwrap_or("").to_string();
    let v = tag.trim_start_matches('v').to_string();
    Ok(Release { apk_url: Some(format!("https://github.com/{REPO}/releases/download/{tag}/Lumen-{v}.apk")), version: v, notes: String::new() })
}

fn latest_via_api() -> Result<Release, String> {
    let mut r = agent().get(format!("https://api.github.com/repos/{REPO}/releases/latest")).header("Accept", "application/vnd.github+json").call().map_err(|e| e.to_string())?;
    let v: serde_json::Value = r.body_mut().read_json().map_err(|e| e.to_string())?;
    let tag = v["tag_name"].as_str().ok_or("no tag")?;
    let apk = v["assets"].as_array().and_then(|a| a.iter().find(|x| x["name"].as_str().is_some_and(|n| n.ends_with(".apk")))).and_then(|x| x["browser_download_url"].as_str()).map(String::from);
    Ok(Release { version: tag.trim_start_matches('v').into(), notes: v["body"].as_str().unwrap_or("").into(), apk_url: apk })
}

pub fn latest() -> Result<Release, String> {
    latest_via_api().or_else(|_| latest_via_redirect())
}

impl Updater {
    pub fn state(&self) -> State {
        self.0.lock().map(|s| s.clone()).unwrap_or_default()
    }

    fn with(&self, f: impl FnOnce(&mut State)) {
        if let Ok(mut s) = self.0.lock() {
            f(&mut s);
        }
    }

    /// Looks on GitHub in the background. `done` runs when the answer is in (to redraw).
    pub fn check(&self, done: impl Fn() + Send + 'static) {
        if self.state().busy {
            return;
        }
        self.with(|s| {
            s.busy = true;
            s.message.clear();
        });
        let me = self.clone();
        std::thread::spawn(move || {
            let cur = android::version_name();
            match latest() {
                Ok(r) => me.with(|s| {
                    s.available = newer(&r.version, &cur).then_some(r);
                    s.checked = true;
                    s.busy = false;
                }),
                Err(e) => me.with(|s| {
                    s.message = format!("Could not look for updates: {e}");
                    s.busy = false;
                }),
            }
            done();
        });
    }

    /// Downloads the new APK and hands it to Android's installer.
    pub fn install(&self, done: impl Fn() + Send + 'static) {
        let Some(r) = self.state().available else { return };
        let Some(url) = r.apk_url.clone() else {
            self.with(|s| s.message = "The new version has no APK file.".into());
            return;
        };
        if !android::can_install_packages() {
            self.with(|s| s.message = "Allow Lumen to install updates, then press Update again.".into());
            android::ask_install_permission();
            return;
        }
        self.with(|s| {
            s.busy = true;
            s.progress = Some(0);
            s.message = "Downloading…".into();
        });
        let me = self.clone();
        std::thread::spawn(move || {
            let res = download(&url, &me, &done);
            match res {
                Ok(path) => {
                    me.with(|s| s.message = "Installing…".into());
                    let r = android::install_apk(&path);
                    me.with(|s| s.message = match r { Ok(()) => "Confirm the update on the screen that opens.".into(), Err(e) => format!("The update did not work: {e}") });
                }
                Err(e) => me.with(|s| s.message = format!("The update did not work: {e}")),
            }
            me.with(|s| {
                s.busy = false;
                s.progress = None;
            });
            done();
        });
    }
}

fn download(url: &str, me: &Updater, done: &impl Fn()) -> Result<String, String> {
    use std::io::{Read, Write};
    let a: ureq::Agent = ureq::Agent::config_builder().timeout_global(Some(Duration::from_secs(300))).user_agent(UA).build().into();
    let mut resp = a.get(url).call().map_err(|e| e.to_string())?;
    let total: u64 = resp.headers().get("content-length").and_then(|v| v.to_str().ok()).and_then(|v| v.parse().ok()).unwrap_or(0);
    let path = format!("{}/lumen-update.apk", android::cache_dir());
    let mut f = std::fs::File::create(&path).map_err(|e| e.to_string())?;
    let mut rd = resp.body_mut().as_reader();
    let (mut buf, mut got, mut last) = (vec![0u8; 64 * 1024], 0u64, 0u8);
    loop {
        let n = rd.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        got += n as u64;
        if total > 0 {
            let p = (got * 100 / total) as u8;
            if p != last {
                last = p;
                me.with(|s| s.progress = Some(p));
                done();
            }
        }
    }
    if total > 0 && got != total {
        return Err("the download was cut short".into());
    }
    Ok(path)
}
