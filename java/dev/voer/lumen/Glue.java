package dev.voer.lumen;

import android.app.Activity;
import android.view.WindowInsetsController;

/** Window calls that must run on the UI thread. Everything else lives in Rust. */
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
}
