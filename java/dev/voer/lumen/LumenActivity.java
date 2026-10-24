package dev.voer.lumen;

import android.app.NativeActivity;
import android.content.Intent;

/** The native activity, plus a flag for Rust: the Home button was pressed while the launcher was already showing. */
public final class LumenActivity extends NativeActivity {
    static volatile boolean homePressed = false;

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        if (intent != null && intent.hasCategory(Intent.CATEGORY_HOME)) homePressed = true;
    }
}
