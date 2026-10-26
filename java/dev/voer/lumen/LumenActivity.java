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

    /**
     * The home gesture (swipe up from an app): the system's recents component sends us the closing app and a reply
     * channel and shrinks the app toward whatever rectangle we answer with. Telling it where the app's icon is on
     * our home screen turns a pill over a bare wallpaper into the app flying into its icon.
     */
    private android.view.SurfaceView floatingIcon;

    private void dropFloatingIcon() {
        try {
            if (floatingIcon != null && floatingIcon.getParent() instanceof android.view.ViewGroup) ((android.view.ViewGroup) floatingIcon.getParent()).removeView(floatingIcon);
        } catch (Throwable ignored) {}
        floatingIcon = null;
    }

    @Override
    public void onEnterAnimationComplete() {
        super.onEnterAnimationComplete();
        android.util.Log.i("lumen-life", "enterAnimationComplete");
    }

    /** The app's icon, masked like Lumen masks it, w by h pixels. */
    private android.graphics.Bitmap iconFor(android.content.ComponentName cn, int w, int h, String shape, float radius) {
        try {
            android.graphics.drawable.Drawable d = getPackageManager().getActivityIcon(cn);
            android.graphics.Bitmap b = android.graphics.Bitmap.createBitmap(w, h, android.graphics.Bitmap.Config.ARGB_8888);
            android.graphics.Canvas c = new android.graphics.Canvas(b);
            android.graphics.Path path = new android.graphics.Path();
            android.graphics.RectF box = new android.graphics.RectF(0, 0, w, h);
            if ("Circle".equals(shape)) path.addOval(box, android.graphics.Path.Direction.CW);
            else {
                float r = "Squircle".equals(shape) ? 0.30f * w : "Rounded".equals(shape) ? radius * 0.5f * w : 0.06f * w;
                path.addRoundRect(box, r, r, android.graphics.Path.Direction.CW);
            }
            c.clipPath(path);
            if (d instanceof android.graphics.drawable.AdaptiveIconDrawable) {
                android.graphics.drawable.AdaptiveIconDrawable ad = (android.graphics.drawable.AdaptiveIconDrawable) d;
                int pad = w / 4;
                if (ad.getBackground() != null) { ad.getBackground().setBounds(-pad, -pad, w + pad, h + pad); ad.getBackground().draw(c); }
                if (ad.getForeground() != null) { ad.getForeground().setBounds(-pad, -pad, w + pad, h + pad); ad.getForeground().draw(c); }
            } else {
                c.drawColor(0xFFF1F1F1);
                int inset = w * 14 / 100;
                d.setBounds(inset, inset, w - inset, h - inset);
                d.draw(c);
            }
            return b;
        } catch (Throwable t) { return null; }
    }

    /**
     * The home gesture (swipe up from an app): the system's recents component sends us the closing app and a reply
     * channel and shrinks the app toward whatever rectangle we answer with. We tell it where the app's icon sits on our
     * home screen and hand over a small surface with that icon, which it cross-fades in: the app flies into its icon
     * instead of a pill over a bare wallpaper.
     */
    @SuppressWarnings("deprecation")
    private void answerGestureContract(Intent intent) {
        try {
            android.os.Bundle c = intent.getBundleExtra("gesture_nav_contract_v1");
            if (c == null) return;
            final android.content.ComponentName cn = c.getParcelable(Intent.EXTRA_COMPONENT_NAME);
            final android.os.Message cb = c.getParcelable("android.intent.extra.REMOTE_CALLBACK");
            if (cn == null || cb == null || cb.replyTo == null) return;
            String want = cn.getPackageName();
            android.graphics.RectF found = null;
            String shape = "Circle";
            float radius = 0.3f;
            java.io.BufferedReader r = new java.io.BufferedReader(new java.io.FileReader(new java.io.File(getFilesDir(), "icon-rects.txt")));
            for (String line; (line = r.readLine()) != null; ) {
                String[] p = line.split(" ");
                if (p.length == 3 && p[0].equals("style")) { shape = p[1]; radius = Float.parseFloat(p[2]); }
                else if (p.length == 5 && p[0].equals(want) && found == null) {
                    found = new android.graphics.RectF(Float.parseFloat(p[1]), Float.parseFloat(p[2]), Float.parseFloat(p[3]), Float.parseFloat(p[4]));
                }
            }
            r.close();
            if (found == null) return;
            final android.graphics.RectF pos = found;
            final String fshape = shape;
            final float frad = radius;
            dropFloatingIcon();
            final int w = Math.max(8, Math.round(pos.width())), h = Math.max(8, Math.round(pos.height()));
            final android.graphics.Bitmap bmp = android.os.Build.VERSION.SDK_INT >= 29 ? iconFor(cn, w, h, fshape, frad) : null;
            final Runnable reply = new Runnable() {
                @Override public void run() {
                    try {
                        android.os.Bundle res = new android.os.Bundle();
                        res.putParcelable("gesture_nav_contract_icon_position", pos);
                        if (floatingIcon != null && android.os.Build.VERSION.SDK_INT >= 29) res.putParcelable("gesture_nav_contract_surface_control", floatingIcon.getSurfaceControl());
                        android.os.Message m = android.os.Message.obtain();
                        m.copyFrom(cb);
                        m.setData(res);
                        cb.replyTo.send(m);
                        android.util.Log.i("lumen-life", "answered gesture contract for " + cn.getPackageName() + " " + pos + (floatingIcon != null ? " with icon surface" : ""));
                    } catch (Throwable t) { android.util.Log.i("lumen-life", "gesture reply failed: " + t); }
                }
            };
            if (bmp == null) { reply.run(); return; }
            final android.view.SurfaceView sv = new android.view.SurfaceView(this);
            sv.setZOrderOnTop(true);
            sv.getHolder().setFormat(android.graphics.PixelFormat.TRANSLUCENT);
            sv.getHolder().addCallback(new android.view.SurfaceHolder.Callback() {
                @Override public void surfaceCreated(android.view.SurfaceHolder holder) {
                    try {
                        android.graphics.Canvas cv = holder.lockHardwareCanvas();
                        cv.drawColor(0, android.graphics.PorterDuff.Mode.CLEAR);
                        cv.drawBitmap(bmp, 0, 0, null);
                        holder.unlockCanvasAndPost(cv);
                    } catch (Throwable ignored) {}
                    reply.run();
                }
                @Override public void surfaceChanged(android.view.SurfaceHolder holder, int f, int a, int b) {}
                @Override public void surfaceDestroyed(android.view.SurfaceHolder holder) {}
            });
            floatingIcon = sv;
            sv.postDelayed(new Runnable() { @Override public void run() { if (floatingIcon == sv) dropFloatingIcon(); } }, 1500);
            android.widget.FrameLayout.LayoutParams lp = new android.widget.FrameLayout.LayoutParams(w, h);
            lp.leftMargin = Math.round(pos.left);
            lp.topMargin = Math.round(pos.top);
            addContentView(sv, lp);
            // safety: if the surface never appears, answer without it so the animation is not left waiting
            sv.postDelayed(new Runnable() { @Override public void run() { if (sv.getHolder().getSurface() == null || !sv.getHolder().getSurface().isValid()) reply.run(); } }, 150);
        } catch (Throwable t) {
            android.util.Log.i("lumen-life", "gesture contract failed: " + t);
        }
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        answerGestureContract(intent);
        note(intent);
    }
}
