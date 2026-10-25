package dev.voer.lumen;

import android.app.Activity;
import android.app.WallpaperManager;
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
                a.getWindow().setDecorFitsSystemWindows(false);
                WindowInsetsController c = a.getWindow().getInsetsController();
                if (c == null) return;
                int mask = WindowInsetsController.APPEARANCE_LIGHT_STATUS_BARS | WindowInsetsController.APPEARANCE_LIGHT_NAVIGATION_BARS;
                c.setSystemBarsAppearance(light ? mask : 0, mask);
            }
        });
    }

    /** 0 = lift (long press), 1 = drop, 2 = tick */
    public static void haptic(final Activity a, final int kind) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                View v = a.getWindow().getDecorView();
                int c = kind == 0 ? HapticFeedbackConstants.LONG_PRESS : kind == 1 ? HapticFeedbackConstants.CONTEXT_CLICK : HapticFeedbackConstants.CLOCK_TICK;
                v.performHapticFeedback(c);
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
                WindowInsetsController c = a.getWindow().getInsetsController();
                if (c == null) return;
                c.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
                if (hideStatus) c.hide(android.view.WindowInsets.Type.statusBars()); else c.show(android.view.WindowInsets.Type.statusBars());
                if (hideNav) c.hide(android.view.WindowInsets.Type.navigationBars()); else c.show(android.view.WindowInsets.Type.navigationBars());
            }
        });
    }

    public static void rotate(final Activity a, final boolean free) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                a.setRequestedOrientation(free ? android.content.pm.ActivityInfo.SCREEN_ORIENTATION_FULL_SENSOR : android.content.pm.ActivityInfo.SCREEN_ORIENTATION_PORTRAIT);
            }
        });
    }

    /** Picks the display mode with the highest refresh rate at the current resolution. */
    public static void highRefresh(final Activity a) {
        a.runOnUiThread(new Runnable() {
            @Override public void run() {
                try {
                    android.view.Display d = a.getDisplay();
                    if (d == null) return;
                    android.view.Display.Mode cur = d.getMode();
                    android.view.Display.Mode best = cur;
                    for (android.view.Display.Mode m : d.getSupportedModes()) {
                        if (m.getPhysicalWidth() == cur.getPhysicalWidth() && m.getPhysicalHeight() == cur.getPhysicalHeight() && m.getRefreshRate() > best.getRefreshRate()) best = m;
                    }
                    android.view.WindowManager.LayoutParams lp = a.getWindow().getAttributes();
                    lp.preferredDisplayModeId = best.getModeId();
                    lp.preferredRefreshRate = best.getRefreshRate();
                    a.getWindow().setAttributes(lp);
                } catch (Throwable ignored) {}
            }
        });
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
