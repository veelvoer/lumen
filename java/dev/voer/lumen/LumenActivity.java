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
    /** A window of this process was closed for good: its native side cannot be started again. */
    private static boolean destroyed = false;

    @Override
    protected void onCreate(Bundle b) {
        if (alive > 0 || destroyed) {
            // Android started a new window before the old one was gone (apps were cleared, then Home): the native
            // side of the old one is still in this process. End it; Android starts a clean process right away.
            android.os.Process.killProcess(android.os.Process.myPid());
        }
        alive++;
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
        getWindow().setFormat(android.graphics.PixelFormat.TRANSLUCENT);
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
    protected void onStart() {
        super.onStart();
        Widgets.resume();
    }

    @Override
    protected void onStop() {
        super.onStop();
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
