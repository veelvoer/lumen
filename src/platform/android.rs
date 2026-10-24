//! The only place that talks to Android (through JNI). Everything else is plain Rust.

use super::PACKAGE;
use jni::errors::Result as JResult;
use jni::objects::{GlobalRef, JObject, JString, JValue};
use jni::{JNIEnv, JavaVM};
use super::{App, Battery, Category, Dynamic, Haptic, Insets, RawIcon};
use std::collections::HashMap;
use std::sync::OnceLock;

static VM: OnceLock<JavaVM> = OnceLock::new();
static ACTIVITY: OnceLock<GlobalRef> = OnceLock::new();


pub fn init(app: &android_activity::AndroidApp) {
    let vm = unsafe { JavaVM::from_raw(app.vm_as_ptr() as *mut _) }.expect("vm");
    let act = unsafe { JObject::from_raw(app.activity_as_ptr() as *mut _) };
    if let Ok(env) = vm.attach_current_thread() {
        if let Ok(g) = env.new_global_ref(&act) {
            let _ = ACTIVITY.set(g);
        }
    }
    let _ = VM.set(vm);
}

/// Runs `f` on the calling thread with a JNI env and the activity. Swallows Java exceptions.
fn with<R>(f: impl FnOnce(&mut JNIEnv, &JObject) -> JResult<R>) -> Option<R> {
    let vm = VM.get()?;
    let act = ACTIVITY.get()?;
    let mut env = vm.attach_current_thread().ok()?;
    let r = env.with_local_frame(64, |env| f(env, act.as_obj()));
    if env.exception_check().unwrap_or(false) {
        let _ = env.exception_clear();
        return None;
    }
    r.ok()
}

fn jstr(env: &mut JNIEnv, s: &JObject) -> String {
    if s.is_null() {
        return String::new();
    }
    let js = JString::from(unsafe { JObject::from_raw(s.as_raw()) });
    env.get_string(&js).map(|s| s.into()).unwrap_or_default()
}

fn package_manager<'a>(env: &mut JNIEnv<'a>, act: &JObject) -> JResult<JObject<'a>> {
    env.call_method(act, "getPackageManager", "()Landroid/content/pm/PackageManager;", &[])?.l()
}

pub fn files_dir() -> String {
    with(|env, act| {
        let f = env.call_method(act, "getFilesDir", "()Ljava/io/File;", &[])?.l()?;
        let p = env.call_method(&f, "getAbsolutePath", "()Ljava/lang/String;", &[])?.l()?;
        Ok(jstr(env, &p))
    })
    .unwrap_or_else(|| "/data/local/tmp".into())
}

pub fn cache_dir() -> String {
    with(|env, act| {
        let f = env.call_method(act, "getCacheDir", "()Ljava/io/File;", &[])?.l()?;
        let p = env.call_method(&f, "getAbsolutePath", "()Ljava/lang/String;", &[])?.l()?;
        Ok(jstr(env, &p))
    })
    .unwrap_or_else(|| "/data/local/tmp".into())
}

pub fn version_name() -> String {
    with(|env, act| {
        let pm = package_manager(env, act)?;
        let pkg = env.new_string(PACKAGE)?;
        let info = env.call_method(&pm, "getPackageInfo", "(Ljava/lang/String;I)Landroid/content/pm/PackageInfo;", &[JValue::Object(&pkg), JValue::Int(0)])?.l()?;
        let v = env.get_field(&info, "versionName", "Ljava/lang/String;")?.l()?;
        Ok(jstr(env, &v))
    })
    .unwrap_or_else(|| env!("CARGO_PKG_VERSION").into())
}

pub fn launcher_apps() -> Vec<App> {
    let mut out: Vec<App> = with(|env, act| {
        let pm = package_manager(env, act)?;
        let main = env.new_string("android.intent.action.MAIN")?;
        let cat = env.new_string("android.intent.category.LAUNCHER")?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&main)])?;
        env.call_method(&intent, "addCategory", "(Ljava/lang/String;)Landroid/content/Intent;", &[JValue::Object(&cat)])?;
        let list = env.call_method(&pm, "queryIntentActivities", "(Landroid/content/Intent;I)Ljava/util/List;", &[JValue::Object(&intent), JValue::Int(0)])?.l()?;
        let n = env.call_method(&list, "size", "()I", &[])?.i()?;
        let mut v = Vec::with_capacity(n as usize);
        for i in 0..n {
            let r = env.with_local_frame(16, |env| -> JResult<Option<App>> {
                let ri = env.call_method(&list, "get", "(I)Ljava/lang/Object;", &[JValue::Int(i)])?.l()?;
                let ai = env.get_field(&ri, "activityInfo", "Landroid/content/pm/ActivityInfo;")?.l()?;
                let pkg = env.get_field(&ai, "packageName", "Ljava/lang/String;")?.l()?;
                let cls = env.get_field(&ai, "name", "Ljava/lang/String;")?.l()?;
                let appinfo = env.get_field(&ai, "applicationInfo", "Landroid/content/pm/ApplicationInfo;")?.l()?;
                let cat = env.get_field(&appinfo, "category", "I")?.i()?;
                let label = env.call_method(&ri, "loadLabel", "(Landroid/content/pm/PackageManager;)Ljava/lang/CharSequence;", &[JValue::Object(&pm)])?.l()?;
                let label = env.call_method(&label, "toString", "()Ljava/lang/String;", &[])?.l()?;
                let (pkg, cls, label) = (jstr(env, &pkg), jstr(env, &cls), jstr(env, &label));
                if pkg == PACKAGE {
                    return Ok(None);
                }
                let key = label.to_lowercase();
                let category = Category::from_android(cat, &pkg);
                Ok(Some(App { pkg, cls, label, key, category }))
            });
            if let Ok(Some(a)) = r {
                v.push(a);
            }
            if env.exception_check().unwrap_or(false) {
                let _ = env.exception_clear();
            }
        }
        Ok(v)
    })
    .unwrap_or_default();
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out.dedup_by(|a, b| a.pkg == b.pkg && a.cls == b.cls);
    out
}

/// The package that answers a common intent (dialer, browser, ...), used to seed the first home screen.
pub fn default_for(action: &str, data: Option<&str>) -> Option<String> {
    with(|env, act| {
        let pm = package_manager(env, act)?;
        let a = env.new_string(action)?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&a)])?;
        if let Some(d) = data {
            let ds = env.new_string(d)?;
            let uri = env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&ds)])?.l()?;
            env.call_method(&intent, "setData", "(Landroid/net/Uri;)Landroid/content/Intent;", &[JValue::Object(&uri)])?;
        }
        let ri = env.call_method(&pm, "resolveActivity", "(Landroid/content/Intent;I)Landroid/content/pm/ResolveInfo;", &[JValue::Object(&intent), JValue::Int(0x10000)])?.l()?;
        if ri.is_null() {
            return Ok(None);
        }
        let ai = env.get_field(&ri, "activityInfo", "Landroid/content/pm/ActivityInfo;")?.l()?;
        let pkg = env.get_field(&ai, "packageName", "Ljava/lang/String;")?.l()?;
        let p = jstr(env, &pkg);
        Ok(if p == "android" || p == PACKAGE { None } else { Some(p) })
    })
    .flatten()
}

fn start(env: &mut JNIEnv, act: &JObject, intent: &JObject) -> JResult<()> {
    env.call_method(intent, "addFlags", "(I)Landroid/content/Intent;", &[JValue::Int(0x10000000)])?;
    env.call_method(act, "startActivity", "(Landroid/content/Intent;)V", &[JValue::Object(intent)])?;
    Ok(())
}

fn uri<'a>(env: &mut JNIEnv<'a>, s: &str) -> JResult<JObject<'a>> {
    let s = env.new_string(s)?;
    env.call_static_method("android/net/Uri", "parse", "(Ljava/lang/String;)Landroid/net/Uri;", &[JValue::Object(&s)])?.l()
}

pub fn launch(a: &App) {
    with(|env, act| {
        let main = env.new_string("android.intent.action.MAIN")?;
        let cat = env.new_string("android.intent.category.LAUNCHER")?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&main)])?;
        env.call_method(&intent, "addCategory", "(Ljava/lang/String;)Landroid/content/Intent;", &[JValue::Object(&cat)])?;
        let (p, c) = (env.new_string(&a.pkg)?, env.new_string(&a.cls)?);
        let cn = env.new_object("android/content/ComponentName", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&p), JValue::Object(&c)])?;
        env.call_method(&intent, "setComponent", "(Landroid/content/ComponentName;)Landroid/content/Intent;", &[JValue::Object(&cn)])?;
        env.call_method(&intent, "addFlags", "(I)Landroid/content/Intent;", &[JValue::Int(0x00200000)])?; // RESET_TASK_IF_NEEDED
        start(env, act, &intent)
    });
}

fn view_intent(action: &str, data: &str) {
    with(|env, act| {
        let a = env.new_string(action)?;
        let u = uri(env, data)?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;Landroid/net/Uri;)V", &[JValue::Object(&a), JValue::Object(&u)])?;
        start(env, act, &intent)
    });
}

pub fn uninstall(pkg: &str) {
    view_intent("android.intent.action.DELETE", &format!("package:{pkg}"));
}

pub fn app_info(pkg: &str) {
    view_intent("android.settings.APPLICATION_DETAILS_SETTINGS", &format!("package:{pkg}"));
}

pub fn open_url(url: &str) {
    view_intent("android.intent.action.VIEW", url);
}

pub fn open_wallpaper_picker() {
    with(|env, act| {
        let a = env.new_string("android.intent.action.SET_WALLPAPER")?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&a)])?;
        let title = env.new_string("Wallpaper")?;
        let chooser = env.call_static_method("android/content/Intent", "createChooser", "(Landroid/content/Intent;Ljava/lang/CharSequence;)Landroid/content/Intent;", &[JValue::Object(&intent), JValue::Object(&title)])?.l()?;
        start(env, act, &chooser)
    });
}

pub fn open_system_settings() {
    with(|env, act| {
        let a = env.new_string("android.settings.SETTINGS")?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&a)])?;
        start(env, act, &intent)
    });
}

pub fn is_default_home() -> bool {
    with(|env, act| {
        let pm = package_manager(env, act)?;
        let a = env.new_string("android.intent.action.MAIN")?;
        let cat = env.new_string("android.intent.category.HOME")?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&a)])?;
        env.call_method(&intent, "addCategory", "(Ljava/lang/String;)Landroid/content/Intent;", &[JValue::Object(&cat)])?;
        let ri = env.call_method(&pm, "resolveActivity", "(Landroid/content/Intent;I)Landroid/content/pm/ResolveInfo;", &[JValue::Object(&intent), JValue::Int(0x10000)])?.l()?;
        if ri.is_null() {
            return Ok(false);
        }
        let ai = env.get_field(&ri, "activityInfo", "Landroid/content/pm/ActivityInfo;")?.l()?;
        let pkg = env.get_field(&ai, "packageName", "Ljava/lang/String;")?.l()?;
        Ok(jstr(env, &pkg) == PACKAGE)
    })
    .unwrap_or(false)
}

/// Opens the system's "choose your home app" screen.
pub fn pick_default_home() {
    with(|env, act| {
        let a = env.new_string("android.settings.HOME_SETTINGS")?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&a)])?;
        start(env, act, &intent)
    });
}

/// System bars + display cutout, in physical pixels.
pub fn insets() -> Insets {
    with(|env, act| {
        let win = env.call_method(act, "getWindow", "()Landroid/view/Window;", &[])?.l()?;
        let decor = env.call_method(&win, "getDecorView", "()Landroid/view/View;", &[])?.l()?;
        let wi = env.call_method(&decor, "getRootWindowInsets", "()Landroid/view/WindowInsets;", &[])?.l()?;
        if wi.is_null() {
            return Ok(Insets::default());
        }
        let sb = env.call_static_method("android/view/WindowInsets$Type", "systemBars", "()I", &[])?.i()?;
        let dc = env.call_static_method("android/view/WindowInsets$Type", "displayCutout", "()I", &[])?.i()?;
        let ins = env.call_method(&wi, "getInsets", "(I)Landroid/graphics/Insets;", &[JValue::Int(sb | dc)])?.l()?;
        let g = |env: &mut JNIEnv, n: &str| env.get_field(&ins, n, "I").and_then(|v| v.i()).map(|v| v as f32);
        Ok(Insets { left: g(env, "left")?, top: g(env, "top")?, right: g(env, "right")?, bottom: g(env, "bottom")? })
    })
    .unwrap_or_default()
}

pub fn is_24h() -> bool {
    with(|env, act| env.call_static_method("android/text/format/DateFormat", "is24HourFormat", "(Landroid/content/Context;)Z", &[JValue::Object(act)])?.z()).unwrap_or(true)
}

/// Unix time in milliseconds shifted to the local time zone.
pub fn local_millis() -> i64 {
    with(|env, _| {
        let now = env.call_static_method("java/lang/System", "currentTimeMillis", "()J", &[])?.j()?;
        let tz = env.call_static_method("java/util/TimeZone", "getDefault", "()Ljava/util/TimeZone;", &[])?.l()?;
        let off = env.call_method(&tz, "getOffset", "(J)I", &[JValue::Long(now)])?.i()?;
        Ok(now + off as i64)
    })
    .unwrap_or_else(|| {
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
    })
}

/// Light status/navigation bar icons for light themes. Runs on the UI thread inside Glue.
pub fn set_light_bars(light: bool) {
    with(|env, act| {
        // Native threads only see system classes, so ask the app's own class loader for ours.
        let loader = env.call_method(act, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
        let name = env.new_string("dev.voer.lumen.Glue")?;
        let cls = env.call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&name)])?.l()?;
        let cls = jni::objects::JClass::from(cls);
        env.call_static_method(&cls, "setLightBars", "(Landroid/app/Activity;Z)V", &[JValue::Object(act), JValue::Bool(light as u8)])?;
        Ok(())
    });
}

pub fn can_install_packages() -> bool {
    with(|env, act| {
        let pm = package_manager(env, act)?;
        env.call_method(&pm, "canRequestPackageInstalls", "()Z", &[])?.z()
    })
    .unwrap_or(false)
}

pub fn ask_install_permission() {
    view_intent("android.settings.MANAGE_UNKNOWN_APP_SOURCES", &format!("package:{PACKAGE}"));
}

/// Hands an APK to Android's installer. The user confirms once; the receiver in Java opens that prompt.
pub fn install_apk(path: &str) -> Result<(), String> {
    let r = with(|env, act| {
        let pm = package_manager(env, act)?;
        let pi = env.call_method(&pm, "getPackageInstaller", "()Landroid/content/pm/PackageInstaller;", &[])?.l()?;
        let params = env.new_object("android/content/pm/PackageInstaller$SessionParams", "(I)V", &[JValue::Int(1)])?;
        let id = env.call_method(&pi, "createSession", "(Landroid/content/pm/PackageInstaller$SessionParams;)I", &[JValue::Object(&params)])?.i()?;
        let session = env.call_method(&pi, "openSession", "(I)Landroid/content/pm/PackageInstaller$Session;", &[JValue::Int(id)])?.l()?;
        let len = std::fs::metadata(path).map(|m| m.len() as i64).unwrap_or(-1);
        let name = env.new_string("lumen.apk")?;
        let out = env.call_method(&session, "openWrite", "(Ljava/lang/String;JJ)Ljava/io/OutputStream;", &[JValue::Object(&name), JValue::Long(0), JValue::Long(len)])?.l()?;
        let data = std::fs::read(path).map_err(|_| jni::errors::Error::JavaException)?;
        for chunk in data.chunks(256 * 1024) {
            let arr = env.byte_array_from_slice(chunk)?;
            env.call_method(&out, "write", "([BII)V", &[JValue::Object(&arr), JValue::Int(0), JValue::Int(chunk.len() as i32)])?;
        }
        env.call_method(&session, "fsync", "(Ljava/io/OutputStream;)V", &[JValue::Object(&out)])?;
        env.call_method(&out, "close", "()V", &[])?;
        let cls = env.new_string("dev.voer.lumen.InstallReceiver")?;
        let pkg = env.new_string(PACKAGE)?;
        let intent = env.new_object("android/content/Intent", "()V", &[])?;
        env.call_method(&intent, "setClassName", "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;", &[JValue::Object(&pkg), JValue::Object(&cls)])?;
        // FLAG_MUTABLE | FLAG_UPDATE_CURRENT: Android adds the status extras to this intent.
        let pending = env.call_static_method("android/app/PendingIntent", "getBroadcast", "(Landroid/content/Context;ILandroid/content/Intent;I)Landroid/app/PendingIntent;", &[JValue::Object(act), JValue::Int(id), JValue::Object(&intent), JValue::Int(0x02000000 | 0x08000000)])?.l()?;
        let sender = env.call_method(&pending, "getIntentSender", "()Landroid/content/IntentSender;", &[])?.l()?;
        env.call_method(&session, "commit", "(Landroid/content/IntentSender;)V", &[JValue::Object(&sender)])?;
        Ok(())
    });
    r.ok_or_else(|| "Android refused the install session".to_string())
}

/// Loads an app icon as RGBA. `px` is the side of the square bitmap.
pub fn load_icon(a: &App, px: i32, want_mono: bool) -> Option<RawIcon> {
    with(|env, act| {
        let pm = package_manager(env, act)?;
        let (p, c) = (env.new_string(&a.pkg)?, env.new_string(&a.cls)?);
        let cn = env.new_object("android/content/ComponentName", "(Ljava/lang/String;Ljava/lang/String;)V", &[JValue::Object(&p), JValue::Object(&c)])?;
        let dr = env.call_method(&pm, "getActivityIcon", "(Landroid/content/ComponentName;)Landroid/graphics/drawable/Drawable;", &[JValue::Object(&cn)])?.l()?;

        let cfg = env.get_static_field("android/graphics/Bitmap$Config", "ARGB_8888", "Landroid/graphics/Bitmap$Config;")?.l()?;
        let bmp = env.call_static_method("android/graphics/Bitmap", "createBitmap", "(IILandroid/graphics/Bitmap$Config;)Landroid/graphics/Bitmap;", &[JValue::Int(px), JValue::Int(px), JValue::Object(&cfg)])?.l()?;
        let canvas = env.new_object("android/graphics/Canvas", "(Landroid/graphics/Bitmap;)V", &[JValue::Object(&bmp)])?;

        let adaptive = env.is_instance_of(&dr, "android/graphics/drawable/AdaptiveIconDrawable")?;
        let draw = |env: &mut JNIEnv, d: &JObject, l: i32, t: i32, r: i32, b: i32| -> JResult<()> {
            env.call_method(d, "setBounds", "(IIII)V", &[JValue::Int(l), JValue::Int(t), JValue::Int(r), JValue::Int(b)])?;
            env.call_method(d, "draw", "(Landroid/graphics/Canvas;)V", &[JValue::Object(&canvas)])?;
            Ok(())
        };
        let mut mono = false;
        if adaptive {
            // Layers are 108dp with the middle 72dp visible: draw them 1.5x so the visible part fills the bitmap.
            let pad = px / 4;
            if want_mono && android_sdk() >= 33 {
                let m = env.call_method(&dr, "getMonochrome", "()Landroid/graphics/drawable/Drawable;", &[])?.l()?;
                if !m.is_null() {
                    draw(env, &m, -pad, -pad, px + pad, px + pad)?;
                    mono = true;
                }
            }
            if !mono {
                for layer in ["getBackground", "getForeground"] {
                    let d = env.call_method(&dr, layer, "()Landroid/graphics/drawable/Drawable;", &[])?.l()?;
                    if !d.is_null() {
                        draw(env, &d, -pad, -pad, px + pad, px + pad)?;
                    }
                }
            }
        } else {
            // Old-style icons come with their own shape: put them on a plain plate.
            env.call_method(&canvas, "drawColor", "(I)V", &[JValue::Int(0xFFF1F1F1u32 as i32)])?;
            let inset = px * 14 / 100;
            draw(env, &dr, inset, inset, px - inset, px - inset)?;
        }

        let n = (px * px * 4) as i32;
        let buf = env.call_static_method("java/nio/ByteBuffer", "allocateDirect", "(I)Ljava/nio/ByteBuffer;", &[JValue::Int(n)])?.l()?;
        env.call_method(&bmp, "copyPixelsToBuffer", "(Ljava/nio/Buffer;)V", &[JValue::Object(&buf)])?;
        let jb = jni::objects::JByteBuffer::from(unsafe { JObject::from_raw(buf.as_raw()) });
        let ptr = env.get_direct_buffer_address(&jb)?;
        let mut rgba = unsafe { std::slice::from_raw_parts(ptr, n as usize) }.to_vec();
        env.call_method(&bmp, "recycle", "()V", &[])?;

        if mono {
            // keep only the alpha: the glyph gets tinted when drawn
            for px in rgba.chunks_exact_mut(4) {
                let al = px[3];
                px.copy_from_slice(&[al, al, al, al]);
            }
        } else if want_mono {
            // no monochrome layer: fall back to a grayscale of the colored icon
            for px in rgba.chunks_exact_mut(4) {
                let g = ((px[0] as u32 * 30 + px[1] as u32 * 59 + px[2] as u32 * 11) / 100) as u8;
                px[0] = g;
                px[1] = g;
                px[2] = g;
            }
        }
        Ok(RawIcon { size: px as usize, rgba, mono })
    })
}

fn android_sdk() -> i32 {
    static SDK: OnceLock<i32> = OnceLock::new();
    *SDK.get_or_init(|| {
        with(|env, _| env.get_static_field("android/os/Build$VERSION", "SDK_INT", "I")?.i()).unwrap_or(26)
    })
}

/// Pulls down the notification shade (needs the EXPAND_STATUS_BAR permission, which is not dangerous).
pub fn expand_notifications() {
    with(|env, act| {
        let name = env.new_string("statusbar")?;
        let sb = env.call_method(act, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[JValue::Object(&name)])?.l()?;
        if sb.is_null() {
            return Ok(());
        }
        env.call_method(&sb, "expandNotificationsPanel", "()V", &[])?;
        Ok(())
    });
}

fn glue<'a>(env: &mut JNIEnv<'a>, act: &JObject, name: &str) -> JResult<jni::objects::JClass<'a>> {
    // Native threads only see system classes, so ask the app's own class loader for ours.
    let loader = env.call_method(act, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])?.l()?;
    let n = env.new_string(name)?;
    let cls = env.call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&n)])?.l()?;
    Ok(jni::objects::JClass::from(cls))
}

pub fn expand_quick_settings() {
    with(|env, act| {
        let name = env.new_string("statusbar")?;
        let sb = env.call_method(act, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[JValue::Object(&name)])?.l()?;
        if !sb.is_null() {
            env.call_method(&sb, "expandSettingsPanel", "()V", &[])?;
        }
        Ok(())
    });
}

pub fn battery() -> Battery {
    with(|env, act| {
        let f = env.new_string("android.intent.action.BATTERY_CHANGED")?;
        let filter = env.new_object("android/content/IntentFilter", "(Ljava/lang/String;)V", &[JValue::Object(&f)])?;
        let none = JObject::null();
        let i = env.call_method(act, "registerReceiver", "(Landroid/content/BroadcastReceiver;Landroid/content/IntentFilter;)Landroid/content/Intent;", &[JValue::Object(&none), JValue::Object(&filter)])?.l()?;
        let get = |env: &mut JNIEnv, k: &str, d: i32| -> JResult<i32> {
            let k = env.new_string(k)?;
            env.call_method(&i, "getIntExtra", "(Ljava/lang/String;I)I", &[JValue::Object(&k), JValue::Int(d)])?.i()
        };
        let (l, sc, plugged) = (get(env, "level", 0)?, get(env, "scale", 100)?, get(env, "plugged", 0)?);
        Ok(Battery { level: (l * 100 / sc.max(1)).clamp(0, 100) as u8, charging: plugged != 0 })
    })
    .unwrap_or(Battery { level: 100, charging: false })
}

pub fn dynamic_colors() -> Option<Dynamic> {
    if android_sdk() < 31 {
        return None;
    }
    with(|env, act| {
        const SHADES: [&str; 13] = ["0", "10", "50", "100", "200", "300", "400", "500", "600", "700", "800", "900", "1000"];
        let mut ramp = |env: &mut JNIEnv, name: &str| -> JResult<[[u8; 3]; 13]> {
            let mut out = [[0u8; 3]; 13];
            for (i, sh) in SHADES.iter().enumerate() {
                let id = env.get_static_field("android/R$color", format!("system_{name}_{sh}"), "I")?.i()?;
                let c = env.call_method(act, "getColor", "(I)I", &[JValue::Int(id)])?.i()? as u32;
                out[i] = [(c >> 16) as u8, (c >> 8) as u8, c as u8];
            }
            Ok(out)
        };
        Ok(Dynamic { accent1: ramp(env, "accent1")?, neutral1: ramp(env, "neutral1")?, neutral2: ramp(env, "neutral2")? })
    })
}

pub fn haptic(kind: Haptic) {
    with(|env, act| {
        let cls = glue(env, act, "dev.voer.lumen.Glue")?;
        let k = match kind { Haptic::Lift => 0, Haptic::Drop => 1, Haptic::Tick => 2 };
        env.call_static_method(&cls, "haptic", "(Landroid/app/Activity;I)V", &[JValue::Object(act), JValue::Int(k)])?;
        Ok(())
    });
}

pub fn set_wallpaper_offset(x: f32) {
    with(|env, act| {
        let cls = glue(env, act, "dev.voer.lumen.Glue")?;
        env.call_static_method(&cls, "wallpaperOffset", "(Landroid/app/Activity;F)V", &[JValue::Object(act), JValue::Float(x)])?;
        Ok(())
    });
}

/// Unread notification counts per package, from the notification listener (empty until the user allows it).
pub fn badges() -> HashMap<String, u32> {
    with(|env, act| {
        let cls = glue(env, act, "dev.voer.lumen.Glue")?;
        let s = env.call_static_method(&cls, "badges", "()Ljava/lang/String;", &[])?.l()?;
        let text = jstr(env, &s);
        Ok(text.lines().filter_map(|l| l.split_once('=')).filter_map(|(p, n)| Some((p.to_string(), n.parse().ok()?))).collect())
    })
    .unwrap_or_default()
}

fn secure_setting_has(key: &str, needle: &str) -> bool {
    with(|env, act| {
        let cr = env.call_method(act, "getContentResolver", "()Landroid/content/ContentResolver;", &[])?.l()?;
        let k = env.new_string(key)?;
        let v = env.call_static_method("android/provider/Settings$Secure", "getString", "(Landroid/content/ContentResolver;Ljava/lang/String;)Ljava/lang/String;", &[JValue::Object(&cr), JValue::Object(&k)])?.l()?;
        Ok(jstr(env, &v).contains(needle))
    })
    .unwrap_or(false)
}

pub fn badge_access() -> bool {
    secure_setting_has("enabled_notification_listeners", PACKAGE)
}

pub fn ask_badge_access() {
    view_intent_plain("android.settings.ACTION_NOTIFICATION_LISTENER_SETTINGS");
}

pub fn lock_enabled() -> bool {
    secure_setting_has("enabled_accessibility_services", &format!("{PACKAGE}/"))
}

pub fn ask_lock_access() {
    view_intent_plain("android.settings.ACCESSIBILITY_SETTINGS");
}

/// Locks the screen through the accessibility service (the only way that keeps fingerprint unlock working).
pub fn lock_screen() -> bool {
    with(|env, act| {
        let cls = glue(env, act, "dev.voer.lumen.Glue")?;
        env.call_static_method(&cls, "lock", "()Z", &[])?.z()
    })
    .unwrap_or(false)
}

/// True once after the Home button was pressed while Lumen was already open.
pub fn take_home_pressed() -> bool {
    with(|env, act| {
        let cls = glue(env, act, "dev.voer.lumen.Glue")?;
        env.call_static_method(&cls, "takeHome", "()Z", &[])?.z()
    })
    .unwrap_or(false)
}

fn view_intent_plain(action: &str) {
    with(|env, act| {
        let a = env.new_string(action)?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&a)])?;
        start(env, act, &intent)
    });
}

pub fn search_web(q: &str) {
    with(|env, act| {
        let a = env.new_string("android.intent.action.WEB_SEARCH")?;
        let intent = env.new_object("android/content/Intent", "(Ljava/lang/String;)V", &[JValue::Object(&a)])?;
        let k = env.new_string("query")?;
        let v = env.new_string(q)?;
        env.call_method(&intent, "putExtra", "(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;", &[JValue::Object(&k), JValue::Object(&v)])?;
        start(env, act, &intent)
    })
    .or_else(|| {
        let enc: String = q.bytes().map(|b| if b.is_ascii_alphanumeric() { (b as char).to_string() } else { format!("%{b:02X}") }).collect();
        open_url(&format!("https://www.google.com/search?q={enc}"));
        Some(())
    });
}

pub fn clipboard_get() -> String {
    with(|env, act| {
        let n = env.new_string("clipboard")?;
        let cm = env.call_method(act, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[JValue::Object(&n)])?.l()?;
        let clip = env.call_method(&cm, "getPrimaryClip", "()Landroid/content/ClipData;", &[])?.l()?;
        if clip.is_null() {
            return Ok(String::new());
        }
        let item = env.call_method(&clip, "getItemAt", "(I)Landroid/content/ClipData$Item;", &[JValue::Int(0)])?.l()?;
        let t = env.call_method(&item, "coerceToText", "(Landroid/content/Context;)Ljava/lang/CharSequence;", &[JValue::Object(act)])?.l()?;
        let t = env.call_method(&t, "toString", "()Ljava/lang/String;", &[])?.l()?;
        Ok(jstr(env, &t))
    })
    .unwrap_or_default()
}

pub fn clipboard_set(text: &str) {
    with(|env, act| {
        let n = env.new_string("clipboard")?;
        let cm = env.call_method(act, "getSystemService", "(Ljava/lang/String;)Ljava/lang/Object;", &[JValue::Object(&n)])?.l()?;
        let (l, t) = (env.new_string("Lumen")?, env.new_string(text)?);
        let clip = env.call_static_method("android/content/ClipData", "newPlainText", "(Ljava/lang/CharSequence;Ljava/lang/CharSequence;)Landroid/content/ClipData;", &[JValue::Object(&l), JValue::Object(&t)])?.l()?;
        env.call_method(&cm, "setPrimaryClip", "(Landroid/content/ClipData;)V", &[JValue::Object(&clip)])?;
        Ok(())
    });
}

/// Is the phone in dark mode right now?
pub fn is_dark() -> bool {
    with(|env, act| {
        let res = env.call_method(act, "getResources", "()Landroid/content/res/Resources;", &[])?.l()?;
        let cfg = env.call_method(&res, "getConfiguration", "()Landroid/content/res/Configuration;", &[])?.l()?;
        let ui = env.get_field(&cfg, "uiMode", "I")?.i()?;
        Ok(ui & 0x30 == 0x20)
    })
    .unwrap_or(true)
}
