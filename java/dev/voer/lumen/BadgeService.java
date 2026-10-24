package dev.voer.lumen;

import android.service.notification.NotificationListenerService;
import android.service.notification.StatusBarNotification;

import java.util.HashMap;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;

/** Counts the notifications of every app so the launcher can show badges. Needs the user's permission. */
public final class BadgeService extends NotificationListenerService {
    static final Map<String, Integer> counts = new ConcurrentHashMap<>();

    private void recount() {
        try {
            Map<String, Integer> now = new HashMap<>();
            StatusBarNotification[] all = getActiveNotifications();
            if (all != null) {
                for (StatusBarNotification n : all) {
                    if (n.isOngoing() || (n.getNotification().flags & android.app.Notification.FLAG_GROUP_SUMMARY) != 0) continue;
                    now.merge(n.getPackageName(), 1, Integer::sum);
                }
            }
            counts.clear();
            counts.putAll(now);
        } catch (Throwable ignored) {}
    }

    @Override public void onListenerConnected() { recount(); }
    @Override public void onNotificationPosted(StatusBarNotification sbn) { recount(); }
    @Override public void onNotificationRemoved(StatusBarNotification sbn) { recount(); }
    @Override public void onListenerDisconnected() { counts.clear(); }
}
