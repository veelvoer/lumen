package dev.voer.lumen;

import android.app.Activity;
import android.appwidget.AppWidgetHost;
import android.appwidget.AppWidgetHostView;
import android.appwidget.AppWidgetManager;
import android.appwidget.AppWidgetProviderInfo;
import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.drawable.Drawable;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.view.MotionEvent;
import android.view.View;
import android.view.ViewGroup;
import android.widget.FrameLayout;
import android.widget.RemoteViews;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.Callable;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;

/**
 * Hosts other apps' widgets. Android wants real Views for them, but Lumen draws everything itself, so the
 * views live off screen: Rust asks for a bitmap now and then, and forwards taps back.
 */
public final class Widgets {
    private Widgets() {}

    static final int HOST_ID = 7421;
    static final int REQUEST_BIND = 7431;
    static final int REQUEST_CONFIGURE = 7432;

    private static AppWidgetHost host;
    private static AppWidgetManager mgr;
    private static Activity act;
    private static final Handler main = new Handler(Looper.getMainLooper());
    private static final Map<Integer, AppWidgetHostView> views = new HashMap<>();
    private static final Map<Integer, int[]> sizes = new HashMap<>();
    static volatile String result = "";

    private static <T> T ui(final Callable<T> c) {
        if (Looper.myLooper() == Looper.getMainLooper()) {
            try { return c.call(); } catch (Throwable t) { return null; }
        }
        FutureTask<T> f = new FutureTask<>(c);
        main.post(f);
        try { return f.get(3, TimeUnit.SECONDS); } catch (Throwable t) { return null; }
    }

    static void start(final Activity a) {
        if (host != null) return;
        act = a;
        mgr = AppWidgetManager.getInstance(a);
        host = new AppWidgetHost(a, HOST_ID);
        try { host.startListening(); } catch (Throwable ignored) {}
    }

    static void stop() {
        try { if (host != null) host.stopListening(); } catch (Throwable ignored) {}
    }

    static void resume() {
        try { if (host != null) host.startListening(); } catch (Throwable ignored) {}
    }

    /** One line per widget: pkg|class|label|minWidthDp|minHeightDp|needsConfigure */
    public static String providers() {
        if (mgr == null) return "";
        StringBuilder sb = new StringBuilder();
        try {
            List<AppWidgetProviderInfo> all = mgr.getInstalledProviders();
            for (AppWidgetProviderInfo i : all) {
                String label = String.valueOf(i.loadLabel(act.getPackageManager())).replace('|', ' ').replace('\n', ' ');
                float d = act.getResources().getDisplayMetrics().density;
                int w = Math.round(i.minWidth / d), h = Math.round(i.minHeight / d);
                sb.append(i.provider.getPackageName()).append('|').append(i.provider.getClassName()).append('|').append(label).append('|')
                  .append(w).append('|').append(h).append('|').append(i.configure != null ? 1 : 0).append('\n');
            }
        } catch (Throwable ignored) {}
        return sb.toString();
    }

    private static AppWidgetProviderInfo find(String pkg, String cls) {
        for (AppWidgetProviderInfo i : mgr.getInstalledProviders()) {
            if (i.provider.getPackageName().equals(pkg) && i.provider.getClassName().equals(cls)) return i;
        }
        return null;
    }

    public static Bitmap preview(final String pkg, final String cls, final int px) {
        return ui(new Callable<Bitmap>() {
            @Override public Bitmap call() {
                AppWidgetProviderInfo i = find(pkg, cls);
                if (i == null) return null;
                Drawable d = i.loadPreviewImage(act, 0);
                if (d == null) d = i.loadIcon(act, 0);
                if (d == null) return null;
                int w = Math.max(1, d.getIntrinsicWidth()), h = Math.max(1, d.getIntrinsicHeight());
                float s = Math.min(1f, (float) px / Math.max(w, h));
                int bw = Math.max(1, Math.round(w * s)), bh = Math.max(1, Math.round(h * s));
                Bitmap b = Bitmap.createBitmap(bw, bh, Bitmap.Config.ARGB_8888);
                Canvas c = new Canvas(b);
                d.setBounds(0, 0, bw, bh);
                d.draw(c);
                return b;
            }
        });
    }

    public static int allocate() {
        if (host == null) return -1;
        try { return host.allocateAppWidgetId(); } catch (Throwable t) { return -1; }
    }

    /** 1: bound. 0: Android is asking the user (the answer arrives through takeResult). -1: failed. */
    public static int bind(final int id, final String pkg, final String cls) {
        Integer r = ui(new Callable<Integer>() {
            @Override public Integer call() {
                try {
                    ComponentName cn = new ComponentName(pkg, cls);
                    if (mgr.bindAppWidgetIdIfAllowed(id, cn)) return 1;
                    Intent i = new Intent(AppWidgetManager.ACTION_APPWIDGET_BIND);
                    i.putExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, id);
                    i.putExtra(AppWidgetManager.EXTRA_APPWIDGET_PROVIDER, cn);
                    act.startActivityForResult(i, REQUEST_BIND);
                    return 0;
                } catch (Throwable t) { return -1; }
            }
        });
        return r == null ? -1 : r;
    }

    /** 1: a configure screen opened (answer through takeResult). 0: nothing to configure. */
    public static int configure(final int id) {
        Integer r = ui(new Callable<Integer>() {
            @Override public Integer call() {
                try {
                    AppWidgetProviderInfo i = mgr.getAppWidgetInfo(id);
                    if (i == null || i.configure == null) return 0;
                    host.startAppWidgetConfigureActivityForResult(act, id, 0, REQUEST_CONFIGURE, null);
                    return 1;
                } catch (Throwable t) { return 0; }
            }
        });
        return r == null ? 0 : r;
    }

    public static void delete(final int id) {
        ui(new Callable<Void>() {
            @Override public Void call() {
                sizes.remove(id);
                AppWidgetHostView v = views.remove(id);
                if (v != null && v.getParent() instanceof ViewGroup) ((ViewGroup) v.getParent()).removeView(v);
                try { host.deleteAppWidgetId(id); } catch (Throwable ignored) {}
                return null;
            }
        });
    }

    public static Bitmap render(final int id, final int w, final int h) {
        return ui(new Callable<Bitmap>() {
            @Override public Bitmap call() {
                AppWidgetHostView v = views.get(id);
                if (v == null) {
                    AppWidgetProviderInfo info = mgr.getAppWidgetInfo(id);
                    if (info == null) return null;
                    v = host.createView(act, id, info);
                    v.setAlpha(0.01f);
                    act.addContentView(v, new FrameLayout.LayoutParams(w, h));
                    views.put(id, v);
                }
                int[] last = sizes.get(id);
                if (last == null || last[0] != w || last[1] != h) {
                    // tell the widget how big it is, so it can pick the layout that fits
                    float d = act.getResources().getDisplayMetrics().density;
                    int wd = Math.round(w / d), hd = Math.round(h / d);
                    try { v.updateAppWidgetSize(null, wd, hd, wd, hd); } catch (Throwable ignored) {}
                    v.setLayoutParams(new FrameLayout.LayoutParams(w, h));
                    sizes.put(id, new int[]{w, h});
                }
                v.forceLayout();
                v.measure(View.MeasureSpec.makeMeasureSpec(w, View.MeasureSpec.EXACTLY), View.MeasureSpec.makeMeasureSpec(h, View.MeasureSpec.EXACTLY));
                v.layout(0, 0, w, h);
                Bitmap b = Bitmap.createBitmap(w, h, Bitmap.Config.ARGB_8888);
                v.draw(new Canvas(b));
                return b;
            }
        });
    }

    /** action: 0 down, 1 up, 2 move (Android's MotionEvent numbers) */
    public static void touch(final int id, final int action, final float x, final float y) {
        ui(new Callable<Void>() {
            @Override public Void call() {
                AppWidgetHostView v = views.get(id);
                if (v == null) return null;
                long t = SystemClock.uptimeMillis();
                MotionEvent e = MotionEvent.obtain(t, t, action, x, y, 0);
                v.dispatchTouchEvent(e);
                e.recycle();
                return null;
            }
        });
    }

    static void onActivityResult(int req, int res, Intent data) {
        int id = data != null ? data.getIntExtra(AppWidgetManager.EXTRA_APPWIDGET_ID, -1) : -1;
        boolean ok = res == Activity.RESULT_OK;
        if (req == REQUEST_BIND) result = "bind:" + id + ":" + (ok ? "ok" : "no");
        else if (req == REQUEST_CONFIGURE) result = "config:" + id + ":" + (ok ? "ok" : "no");
    }

    public static String takeResult() {
        String r = result;
        result = "";
        return r;
    }
}
