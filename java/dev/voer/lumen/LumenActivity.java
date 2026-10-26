package dev.voer.lumen;

import android.app.NativeActivity;
import android.content.Intent;
import android.os.Bundle;

/**
 * The native activity, plus two flags for Rust: the Home button was pressed while the launcher was already
 * showing, and Lumen was opened from an app icon (which always means "show me the settings").
 */
public final class LumenActivity extends NativeActivity {
    static volatile boolean homePressed = false;
    static volatile boolean settingsRequested = false;

    private static void note(Intent intent) {
        if (intent == null) return;
        if (intent.hasCategory(Intent.CATEGORY_HOME)) homePressed = true;
        else if (intent.hasCategory(Intent.CATEGORY_LAUNCHER)) settingsRequested = true;
    }

    /** Live activities in this process. Native code cannot start its window twice in one process. */
    private static int alive = 0;
    private static android.graphics.drawable.Drawable windowBg;

    /**
     * The opaque window shows its background the instant it appears, before Lumen has drawn a frame. A black one
     * flashes black; the wallpaper (cropped like Lumen draws it at the first page) makes that moment invisible.
     */
    private void applyWindowBackground() {
        try {
            if (windowBg == null) {
                android.graphics.Bitmap w = Glue.wallpaper(this, 1280);
                if (w == null) return;
                android.util.DisplayMetrics dm = getResources().getDisplayMetrics();
                float sa = (float) dm.widthPixels / dm.heightPixels, wa = (float) w.getWidth() / w.getHeight();
                int cw, ch, x0 = 0, y0 = 0;
                if (wa > sa) { ch = w.getHeight(); cw = Math.round(ch * sa); }
                else { cw = w.getWidth(); ch = Math.round(cw / sa); y0 = (w.getHeight() - ch) / 2; }
                cw = Math.max(1, Math.min(cw, w.getWidth()));
                ch = Math.max(1, Math.min(ch, w.getHeight() - y0));
                windowBg = new android.graphics.drawable.BitmapDrawable(getResources(), android.graphics.Bitmap.createBitmap(w, x0, y0, cw, ch));
            }
            getWindow().setBackgroundDrawable(windowBg);
        } catch (Throwable ignored) {}
    }
    private boolean ownWallpaper = false;
    /** A window of this process was closed for good: its native side cannot be started again. */
    private static boolean destroyed = false;

    /**
     * Ends this process and asks for a fresh home screen a moment later, so the user sees one quick restart
     * instead of whatever Android shows when the home app dies. Also notes it for the report.
     */
    private void restartCleanly() {
        try {
            java.io.FileWriter w = new java.io.FileWriter(new java.io.File(getFilesDir(), "last-restart.txt"));
            w.write(new java.text.SimpleDateFormat("MM-dd HH:mm:ss", java.util.Locale.US).format(new java.util.Date()) + " stale window, restarted");
            w.close();
        } catch (Throwable ignored) {}
        try {
            Intent home = new Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME).setPackage(getPackageName()).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
            android.os.Bundle opts = null;
            if (android.os.Build.VERSION.SDK_INT >= 34) {
                opts = android.app.ActivityOptions.makeBasic().setPendingIntentCreatorBackgroundActivityStartMode(android.app.ActivityOptions.MODE_BACKGROUND_ACTIVITY_START_ALLOWED).toBundle();
            }
            android.app.PendingIntent pi = android.app.PendingIntent.getActivity(this, 7, home, android.app.PendingIntent.FLAG_UPDATE_CURRENT | android.app.PendingIntent.FLAG_IMMUTABLE, opts);
            ((android.app.AlarmManager) getSystemService(ALARM_SERVICE)).set(android.app.AlarmManager.RTC, System.currentTimeMillis() + 300, pi);
        } catch (Throwable ignored) {}
        android.os.Process.killProcess(android.os.Process.myPid());
    }

    @Override
    protected void onCreate(Bundle b) {
        if (alive > 0 || destroyed) {
            // Android started a new window before the old one was gone (apps were cleared, then Home): the native
            // side of the old one is still in this process. End it; Android starts a clean process right away.
            restartCleanly();
        }
        alive++;
        // Rust leaves this file when it can paint the wallpaper itself (see glass::Backdrop)
        ownWallpaper = new java.io.File(getFilesDir(), "own-wallpaper").exists();
        if (ownWallpaper) {
            int theme = getResources().getIdentifier("Theme.Lumen.Opaque", "style", getPackageName());
            if (theme != 0) setTheme(theme); else ownWallpaper = false;
        }
        final java.io.File crashFile = new java.io.File(getFilesDir(), "last-crash.txt");
        final Thread.UncaughtExceptionHandler prev = Thread.getDefaultUncaughtExceptionHandler();
        Thread.setDefaultUncaughtExceptionHandler((t, e) -> {
            try {
                java.io.StringWriter sw = new java.io.StringWriter();
                e.printStackTrace(new java.io.PrintWriter(sw));
                java.io.FileWriter w = new java.io.FileWriter(crashFile);
                w.write("java crash in " + t.getName() + ": " + sw);
                w.close();
            } catch (Throwable ignored) {}
            if (prev != null) prev.uncaughtException(t, e);
        });
        super.onCreate(b);
        // NativeActivity asks for an opaque RGB_565 window; the wallpaper can only show through a translucent one
        if (!ownWallpaper) getWindow().setFormat(android.graphics.PixelFormat.TRANSLUCENT);
        else applyWindowBackground();
        Widgets.start(this);
        Intent i = getIntent();
        if (i != null && i.hasCategory(Intent.CATEGORY_LAUNCHER) && !i.hasCategory(Intent.CATEGORY_HOME)) settingsRequested = true;
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        alive--;
        // Do not end the process here: Android may be about to start the home screen again, and killing the process
        // underneath that is what looked like a crash. Remember it instead; the next window kills the stale process
        // while it starts, and Android then restarts it cleanly.
        if (!isChangingConfigurations()) destroyed = true;
    }

    @Override
    protected void onResume() {
        super.onResume();
        android.util.Log.i("lumen-life", "onResume");
    }

    @Override
    protected void onPause() {
        super.onPause();
        android.util.Log.i("lumen-life", "onPause");
    }

    @Override
    public void surfaceCreated(android.view.SurfaceHolder holder) {
        android.util.Log.i("lumen-life", "surfaceCreated");
        super.surfaceCreated(holder);
    }

    @Override
    public void surfaceChanged(android.view.SurfaceHolder holder, int format, int w, int h) {
        android.util.Log.i("lumen-life", "surfaceChanged " + w + "x" + h);
        super.surfaceChanged(holder, format, w, h);
    }

    @Override
    public void surfaceDestroyed(android.view.SurfaceHolder holder) {
        android.util.Log.i("lumen-life", "surfaceDestroyed");
        super.surfaceDestroyed(holder);
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        android.util.Log.i("lumen-life", "focus " + hasFocus);
        super.onWindowFocusChanged(hasFocus);
    }

    @Override
    protected void onStart() {
        super.onStart();
        android.util.Log.i("lumen-life", "onStart");
        Widgets.resume();
    }

    @Override
    protected void onStop() {
        super.onStop();
        android.util.Log.i("lumen-life", "onStop");
        Widgets.stop();
    }

    @Override
    protected void onActivityResult(int requestCode, int resultCode, Intent data) {
        super.onActivityResult(requestCode, resultCode, data);
        Widgets.onActivityResult(requestCode, resultCode, data);
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        note(intent);
    }
}
