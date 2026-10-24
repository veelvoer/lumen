package dev.voer.lumen;

import android.accessibilityservice.AccessibilityService;
import android.view.accessibility.AccessibilityEvent;

/** Lets a double tap lock the screen without turning off fingerprint unlock. It reads nothing from the screen. */
public final class LockService extends AccessibilityService {
    static volatile LockService instance;

    boolean lock() { return performGlobalAction(GLOBAL_ACTION_LOCK_SCREEN); }

    @Override protected void onServiceConnected() { instance = this; }
    @Override public boolean onUnbind(android.content.Intent i) { instance = null; return super.onUnbind(i); }
    @Override public void onAccessibilityEvent(AccessibilityEvent e) {}
    @Override public void onInterrupt() {}
}
