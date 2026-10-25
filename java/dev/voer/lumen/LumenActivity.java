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
        Intent i = getIntent();
        if (i != null && i.hasCategory(Intent.CATEGORY_LAUNCHER) && !i.hasCategory(Intent.CATEGORY_HOME)) settingsRequested = true;
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        note(intent);
    }
}
