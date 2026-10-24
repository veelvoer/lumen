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

    public static boolean takeHome() {
        boolean was = LumenActivity.homePressed;
        LumenActivity.homePressed = false;
        return was;
    }
}
