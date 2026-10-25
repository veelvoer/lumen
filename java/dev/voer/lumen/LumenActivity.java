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

    @Override
    protected void onCreate(Bundle b) {
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
        if (isFinishing()) android.os.Process.killProcess(android.os.Process.myPid());
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
