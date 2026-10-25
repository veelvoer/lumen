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

    @Override
    protected void onCreate(Bundle b) {
        if (alive > 0) {
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

    /**
     * Swiping Lumen away in the app switcher destroys the activity but leaves the process behind, and native code
     * cannot start its window twice in one process: the next Home press showed a black screen. End the process so
     * Android starts a clean one.
     */
    @Override
    protected void onDestroy() {
        super.onDestroy();
        alive--;
        // finishing or reclaimed by the system, either way the native window cannot be started again here
        if (alive <= 0 && !isChangingConfigurations()) android.os.Process.killProcess(android.os.Process.myPid());
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
