package dev.voer.lumen;

import android.app.Activity;
import android.app.WallpaperManager;
import android.os.Build;
import android.view.HapticFeedbackConstants;
import android.view.View;
import android.view.WindowInsetsController;

import java.util.Map;

/** Calls that must run on the UI thread, plus the few values Rust reads from the Java side. */
public final class Glue {
    private Glue() {}

    public static void setLightBars(final Activity a, final boolean light) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                try {
                    if (Build.VERSION.SDK_INT >= 30) {
                        a.getWindow().setDecorFitsSystemWindows(false);
                        WindowInsetsController c = a.getWindow().getInsetsController();
                        if (c == null) return;
                        int mask = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                        c.setSystemBarsAppearance(light ? mask : 0, mask);
                    } else {
                        // older Android: the same thing with the system UI flags
                        View d = a.getWindow().getDecorView();
                        int f = View.SYSTEM_UI_FLAG_LAYOUT_STABLE | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION;
                        if (light) {
                            if (Build.VERSION.SDK_INT >= 23) f |= View.SYSTEM_UI_FLAG_LIGHT_STATUS_BAR;
                            if (Build.VERSION.SDK_INT >= 26) f |= View.SYSTEM_UI_FLAG_LIGHT_NAVIGATION_BAR;
                        }
                        d.setSystemUiVisibility(f);
                        a.getWindow().setStatusBarColor(0);
                        a.getWindow().setNavigationBarColor(0);
                    }
                } catch (Throwable ignored) {}
            }
        });
    }

    /** 0 = lift (long press), 1 = drop, 2 = tick */
    public static void haptic(final Activity a, final int kind) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                try {
                    View v = a.getWindow().getDecorView();
                    int c = kind == 0 ? HapticFeedbackConstants.LONG_PRESS : kind == 1 ? (Build.VERSION.SDK_INT >= 23 ? HapticFeedbackConstants.CONTEXT_CLICK : HapticFeedbackConstants.VIRTUAL_KEY) : (Build.VERSION.SDK_INT >= 21 ? HapticFeedbackConstants.CLOCK_TICK : HapticFeedbackConstants.KEYBOARD_TAP);
                    v.performHapticFeedback(c);
                } catch (Throwable ignored) {}
            }
        });
    }

    public static void wallpaperOffset(Activity a, float x) {
        try {
            WallpaperManager.getInstance(a).setWallpaperOffsets(a.getWindow().getDecorView().getWindowToken(), x, 0.5f);
        } catch (Throwable ignored) {}
    }

    /** "package=count" per line */
    public static String badges() {
        StringBuilder sb = new StringBuilder();
        for (Map.Entry<String, Integer> e : BadgeService.counts.entrySet()) {
            if (e.getValue() > 0) sb.append(e.getKey()).append('=').append(e.getValue()).append('\n');
        }
        return sb.toString();
    }

    public static boolean lock() {
        LockService s = LockService.instance;
        return s != null && s.lock();
    }

    /** Frosted wallpaper behind the launcher (Android 12+), in pixels. 0 turns it off. */
    public static void setBlur(final Activity a, final int px) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                try { a.getWindow().setBackgroundBlurRadius(px); } catch (Throwable ignored) {}
            }
        });
    }

    public static void bars(final Activity a, final boolean hideStatus, final boolean hideNav) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                try {
                    if (Build.VERSION.SDK_INT >= 30) {
                        WindowInsetsController c = a.getWindow().getInsetsController();
                        if (c == null) return;
                        c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                        if (hideStatus) c.hide(android.view.WindowInsets.Type.statusBars()); else c.show(android.view.WindowInsets.Type.statusBars());
                        if (hideNav) c.hide(android.view.WindowInsets.Type.navigationBars()); else c.show(android.view.WindowInsets.Type.navigationBars());
                    } else {
                        View d = a.getWindow().getDecorView();
                        int f = d.getSystemUiVisibility();
                        f = hideStatus ? (f | View.SYSTEM_UI_FLAG_FULLSCREEN) : (f & ~View.SYSTEM_UI_FLAG_FULLSCREEN);
                        f = hideNav ? (f | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION | View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY) : (f & ~View.SYSTEM_UI_FLAG_HIDE_NAVIGATION);
                        d.setSystemUiVisibility(f);
                    }
                } catch (Throwable ignored) {}
            }
        });
    }

    public static void rotate(final Activity a, final boolean free) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                try {
                    // tablets always rotate: a portrait lock on a landscape-first screen is unusable
                    boolean tablet = a.getResources().getConfiguration().smallestScreenWidthDp >= 600;
                    a.setRequestedOrientation(free || tablet ? android.content.pm.ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR : android.content.pm.ActivityInfo.SCREEN_ORIENTATION_PORTRAIT);
                } catch (Throwable ignored) {}
            }
        });
    }

    /** Picks the display mode with the highest refresh rate at the current resolution. */
    public static void highRefresh(final Activity a, final boolean on) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                try {
                    android.view.Display d = Build.VERSION.SDK_INT >= 30 ? a.getDisplay() : a.getWindowManager().getDefaultDisplay();
                    if (d == null) return;
                    android.view.Display.Mode cur = d.getMode();
                    android.view.Display.Mode best = cur;
                    for (android.view.Display.Mode m : d.getSupportedModes()) {
                        if (m.getPhysicalWidth() == cur.getPhysicalWidth() && m.getPhysicalHeight() == cur.getPhysicalHeight() && m.getRefreshRate() > best.getRefreshRate()) best = m;
                    }
                    android.view.WindowManager.LayoutParams lp = a.getWindow().getAttributes();
                    // off: let the system choose again
                    lp.preferredDisplayModeId = on ? best.getModeId() : 0;
                    lp.preferredRefreshRate = on ? best.getRefreshRate() : 0f;
                    a.getWindow().setAttributes(lp);
                } catch (Throwable ignored) {}
            }
        });
    }

    static volatile String wallpaperError = "";

    /** The system wallpaper scaled to at most maxPx on its long side, or null when Android does not let us read it. */
    public static android.graphics.Bitmap wallpaper(Activity a, int maxPx) {
        try {
            WallpaperManager wm = WallpaperManager.getInstance(a);
            android.graphics.drawable.Drawable d = wm.getDrawable();
            if (d == null) { wallpaperError = "no drawable"; return null; }
            int w = Math.max(1, d.getIntrinsicWidth()), h = Math.max(1, d.getIntrinsicHeight());
            float s = Math.min(1f, (float) maxPx / Math.max(w, h));
            int bw = Math.max(1, Math.round(w * s)), bh = Math.max(1, Math.round(h * s));
            android.graphics.Bitmap b = android.graphics.Bitmap.createBitmap(bw, bh, android.graphics.Bitmap.Config.ARGB_8888);
            android.graphics.Canvas c = new android.graphics.Canvas(b);
            d.setBounds(0, 0, bw, bh);
            d.draw(c);
            wallpaperError = "";
            return b;
        } catch (Throwable t) {
            wallpaperError = String.valueOf(t);
            return null;
        }
    }

    public static boolean wallpaperAccess(Activity a) {
        try {
            if (Build.VERSION.SDK_INT >= 30) return android.os.Environment.isExternalStorageManager();
            return a.checkSelfPermission(android.Manifest.permission.READ_EXTERNAL_STORAGE) == android.content.pm.PackageManager.PERMISSION_GRANTED;
        } catch (Throwable t) { return false; }
    }

    public static void requestWallpaperAccess(final Activity a) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                try {
                    if (Build.VERSION.SDK_INT >= 30) {
                        android.content.Intent i = new android.content.Intent(android.provider.Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, android.net.Uri.parse("package:" + a.getPackageName()));
                        i.addFlags(android.content.Intent.FLAG_ACTIVITY_NEW_TASK);
                        a.startActivity(i);
                    } else {
                        a.requestPermissions(new String[]{android.Manifest.permission.READ_EXTERNAL_STORAGE}, 1);
                    }
                } catch (Throwable ignored) {}
            }
        });
    }

    public static boolean wallpaperLive(Activity a) {
        try { return WallpaperManager.getInstance(a).getWallpaperInfo() != null; } catch (Throwable t) { return false; }
    }

    public static String wallpaperError() { return wallpaperError; }

    /** The wallpaper's main colors ("argb,argb,argb"), which Android hands out without any permission. */
    public static String wallpaperColors(Activity a) {
        if (android.os.Build.VERSION.SDK_INT < 27) return "";
        try {
            android.app.WallpaperColors c = WallpaperManager.getInstance(a).getWallpaperColors(WallpaperManager.FLAG_SYSTEM);
            if (c == null) return "";
            int p = c.getPrimaryColor().toArgb();
            int s = c.getSecondaryColor() != null ? c.getSecondaryColor().toArgb() : p;
            int t = c.getTertiaryColor() != null ? c.getTertiaryColor().toArgb() : s;
            return p + "," + s + "," + t;
        } catch (Throwable t) { return ""; }
    }

    /** Why Android ended Lumen's last few processes (crash, native crash, ANR, killed...), newest first. */
    public static String exitInfo(Activity a) {
        if (android.os.Build.VERSION.SDK_INT < 30) return "";
        try {
            android.app.ActivityManager am = (android.app.ActivityManager) a.getSystemService(android.content.Context.ACTIVITY_SERVICE);
            java.util.List<android.app.ApplicationExitInfo> l = am.getHistoricalProcessExitReasons(a.getPackageName(), 0, 6);
            StringBuilder sb = new StringBuilder();
            for (android.app.ApplicationExitInfo e : l) {
                int r = e.getReason();
                String name = r == 4 ? "crash" : r == 5 ? "native crash" : r == 6 ? "not responding" : r == 3 ? "slow start" : r == 2 ? "signal" : r == 10 ? "user" : r == 16 ? "update" : r == 13 ? "system" : r == 12 ? "memory" : "other " + r;
                if (r == 16 || r == 10) continue;
                sb.append(new java.text.SimpleDateFormat("MM-dd HH:mm:ss", java.util.Locale.US).format(new java.util.Date(e.getTimestamp()))).append(' ').append(name);
                String d = e.getDescription();
                if (d != null && d.length() > 0) sb.append(" (").append(d.length() > 90 ? d.substring(0, 90) : d).append(')');
                sb.append('\n');
            }
            return sb.toString();
        } catch (Throwable t) { return ""; }
    }

    public static boolean takeSettings() {
        boolean was = LumenActivity.settingsRequested;
        LumenActivity.settingsRequested = false;
        return was;
    }

    public static boolean takeHome() {
        boolean was = LumenActivity.homePressed;
        LumenActivity.homePressed = false;
        return was;
    }
}
