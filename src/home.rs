//! The home screen: geometry, painting, and the touch machine (tap, swipe, long press, drag and drop).

use std::collections::HashMap;
use crate::app::*;
use crate::cards;
use crate::anim;
use crate::config::{Action, DockStyle, Item, Page, PageEffect, Preset, Search, Shape, WidgetKind};
use crate::fonts;
use crate::phosphor as ph;
use crate::platform::{self, Haptic};
use crate::widgets::{self, cr};
use egui::{pos2, vec2, Align2, Color32, Context, Frame, Painter, Pos2, Rect, Sense, Stroke, StrokeKind};

pub struct Geo {
    pub screen: Rect,
    pub top: f32,
    pub grid: Rect,
    pub cell: egui::Vec2,
    pub dock: Rect,
    pub search: Option<Rect>,
    pub dots_y: f32,
    pub dock_slot_w: f32,
}

impl Geo {
    pub fn cell_rect(&self, col: usize, row: usize, w: usize, h: usize) -> Rect {
        Rect::from_min_size(self.grid.min + vec2(col as f32 * self.cell.x, row as f32 * self.cell.y), vec2(w as f32 * self.cell.x, h as f32 * self.cell.y))
    }

    pub fn cell_at(&self, pos: Pos2, cols: usize, rows: usize) -> Option<(usize, usize)> {
        let rel = pos - self.grid.min;
        if rel.x < 0.0 || rel.y < 0.0 {
            return None;
        }
        let (c, r) = ((rel.x / self.cell.x) as usize, (rel.y / self.cell.y) as usize);
        (c < cols && r < rows).then_some((c, r))
    }

    /// Dock slot i when the dock holds n items: the group is centered.
    pub fn dock_slot(&self, i: usize, n: usize) -> Rect {
        let total = self.dock_slot_w * n as f32;
        Rect::from_min_size(pos2(self.dock.center().x - total / 2.0 + i as f32 * self.dock_slot_w, self.dock.top()), vec2(self.dock_slot_w, self.dock.height()))
    }
}

pub struct ZoneSpec {
    pub hover: Hover,
    pub rect: Rect,
    pub glyph: &'static str,
    pub label: &'static str,
}

impl Lumen {
    pub fn geo(&self, ctx: &Context, screen: Rect) -> Geo {
        let k = ctx.pixels_per_point();
        let (il, it, ir, ib) = (self.insets.left / k, self.insets.top / k, self.insets.right / k, self.insets.bottom / k);
        let c = &self.cfg;
        let icon = c.eff();
        let bottom = screen.bottom() - ib.max(14.0) - c.margin_bottom;
        let dock_label = if c.dock_labels { 16.0 * c.font_scale * c.label_size / 12.0 + 4.0 } else { 0.0 };
        let dock_h = match c.dock {
            DockStyle::Bare => c.dock_icon_size + 22.0,
            _ => c.dock_icon_size + 30.0,
        } + dock_label;
        let margin = if c.preset == Preset::Ios { 10.0 } else { 12.0 } + c.margin_side;
        let left = screen.left() + il.max(margin);
        let right = screen.right() - ir.max(margin);
        let (gl, gr) = (left, right);
        // on wide screens the bars stay a comfortable width
        let (left, right) = if right - left > 760.0 { let mid = (left + right) / 2.0; (mid - 380.0, mid + 380.0) } else { (left, right) };
        if c.preset == Preset::Ios && c.search == Search::Bottom {
            // SpringBoard metrics (iPhone 15, 393 x 852 pt), scaled to this screen: side margin 11, row pitch 97,
            // dock 94 high with a 38 radius, a small Search capsule between the grid and the dock, dots above it.
            let s = (screen.width() / 393.0).min(screen.height() / 852.0 * 1.02).clamp(0.8, 1.5);
            let dock_h = 94.0 * s;
            let dock_bottom = screen.bottom() - (ib * 0.5).max(8.0 * s) - c.margin_bottom;
            let dock = Rect::from_min_max(pos2(left, dock_bottom - dock_h), pos2(right, dock_bottom));
            let pill = Rect::from_center_size(pos2(screen.center().x, dock.top() - 14.0 * s - 18.0 * s), vec2(88.0 * s, 36.0 * s));
            let dots_y = pill.top() - 13.0 * s;
            let top = screen.top() + it + 14.0 * s + c.margin_top;
            let pitch = (97.0 * s).min((dots_y - 18.0 * s - top) / c.rows as f32);
            let grid = Rect::from_min_size(pos2(gl, top), vec2(gr - gl, pitch * c.rows as f32));
            let cell = vec2(grid.width() / c.cols as f32, pitch);
            let dock_slot_w = (dock.width() / c.dock_slots.max(1) as f32).min(100.0 * s);
            return Geo { screen, top: screen.top() + it, grid, cell, dock, search: Some(pill), dots_y, dock_slot_w };
        }
        let mut y = bottom;
        let mut search = None;

        if c.search == Search::Bottom && matches!(c.preset, Preset::Pixel | Preset::Custom) {
            // Pixel: the search pill sits under the dock
            search = Some(Rect::from_min_max(pos2(left + 4.0, y - 56.0), pos2(right - 4.0, y)));
            y -= 56.0 + 6.0;
        }
        let dock = Rect::from_min_max(pos2(left, y - dock_h), pos2(right, y));
        y = dock.top();
        if c.search == Search::Bottom && search.is_none() {
            search = Some(Rect::from_min_max(pos2(left + 28.0, y - 46.0 - 6.0), pos2(right - 28.0, y - 6.0)));
            y -= 58.0;
        }
        let dots_y = if c.page_dots && self.page_count() > 1 { y - 14.0 } else { y };
        if c.page_dots && self.page_count() > 1 {
            y -= 26.0;
        }
        let mut top = screen.top() + it + 6.0 + c.margin_top;
        if c.search == Search::Top {
            search = Some(Rect::from_min_size(pos2(left + 6.0, top), vec2(right - left - 12.0, 52.0)));
            top += 62.0;
        }
        let grid = Rect::from_min_max(pos2(gl, top), pos2(gr, (y - 4.0).max(top + 100.0)));
        let mut cell = vec2(grid.width() / c.cols as f32, grid.height() / c.rows as f32);
        let mut grid = grid;
        if c.preset == Preset::Pixel {
            // Pixel Launcher: rows are about 104 dp apart and the grid starts under the status bar
            let pitch = cell.y.min(104.0);
            cell.y = pitch;
            grid = Rect::from_min_size(grid.min, vec2(grid.width(), pitch * c.rows as f32));
        }
        let dock_slot_w = (dock.width() / c.dock_slots.max(1) as f32).min(110.0);
        let _ = icon;
        Geo { screen, top: screen.top() + it, grid, cell, dock, search, dots_y, dock_slot_w }
    }

    /// The square the icon is drawn in, for a one-cell item.
    pub fn icon_rect(&self, cell: Rect) -> Rect {
        let icon = self.cfg.eff();
        let label_h = if self.cfg.labels { 16.0 * self.cfg.font_scale * self.cfg.label_size / 12.0 } else { 0.0 };
        let total = icon + if self.cfg.labels { 6.0 + label_h } else { 0.0 };
        let top = cell.top() + ((cell.height() - total) / 2.0).max(2.0);
        Rect::from_min_size(pos2(cell.center().x - icon / 2.0, top), vec2(icon, icon))
    }

    pub fn dock_icon_rect(&self, slot: Rect) -> Rect {
        let icon = self.cfg.dock_icon_size;
        let up = if self.cfg.dock_labels { 8.0 } else { 0.0 };
        Rect::from_center_size(slot.center() - vec2(0.0, up), vec2(icon, icon))
    }

    fn zones(&self, geo: &Geo, d: &Drag) -> Vec<ZoneSpec> {
        if self.cfg.preset == Preset::Ios {
            return vec![];
        }
        let on_home = !matches!(d.from, Source::Drawer);
        let is_app = matches!(d.item, Item::App(_));
        let mut z: Vec<(Hover, &'static str, &'static str)> = vec![];
        if on_home {
            z.push((Hover::Remove, ph::X, "Remove"));
        }
        if is_app {
            z.push((Hover::Info, ph::INFO, "App info"));
            z.push((Hover::Uninstall, ph::TRASH, "Uninstall"));
        }
        let n = z.len().max(1) as f32;
        let w = (geo.screen.width() - 24.0) / n;
        z.into_iter()
            .enumerate()
            .map(|(i, (hover, glyph, label))| ZoneSpec { hover, glyph, label, rect: Rect::from_min_size(pos2(geo.screen.left() + 12.0 + i as f32 * w, geo.top + 6.0), vec2(w - 8.0, 52.0)) })
            .collect()
    }

    // ───────────────────────────── frame ─────────────────────────────

    pub fn home(&mut self, root: &mut egui::Ui, ctx: &Context) {
        let screen = ctx.content_rect();
        let mut geo = self.geo(ctx, screen);
        // icons shrink to fit when the screen or the grid is small
        let label_h = if self.cfg.labels { 6.0 + 16.0 * self.cfg.font_scale * self.cfg.label_size / 12.0 } else { 0.0 };
        let drawer_w = (screen.width() - 24.0) / self.cfg.drawer_cols.max(3) as f32;
        let fit = self.cfg.icon_size.min(geo.cell.y - label_h - 10.0).min(geo.cell.x - 10.0).min(drawer_w - 10.0).max(28.0);
        if (self.cfg.eff() - fit).abs() > 0.5 {
            self.cfg.icon_eff = fit;
            geo = self.geo(ctx, screen);
        }
        let dt = ctx.input(|i| i.stable_dt).clamp(0.001, 0.05);
        {
            let _s = crate::perf::span(crate::perf::Sec::Springs);
            self.step_springs(ctx, dt);
        }
        self.track_items(&geo);
        // the root ui must be claimed, everything is painted in layers
        egui::CentralPanel::default().frame(Frame::NONE).show(root, |_| {});
        let covered = self.drawer >= 0.995 && self.screen == Screen::Home;
        if !covered {
            let _s = crate::perf::span(crate::perf::Sec::Paint);
            self.paint_home(ctx, &geo, dt);
        }
        if self.screen == Screen::Home {
            {
                let _s = crate::perf::span(crate::perf::Sec::Touch);
                self.touch(ctx, &geo);
            }
            let _s = crate::perf::span(crate::perf::Sec::Overlays);
            if self.drawer > 0.001 {
                self.drawer_ui(ctx, screen);
            }
            if self.search_open {
                self.search_ui(ctx, screen);
            }
            if self.menu.is_some() {
                self.menu_ui(ctx, screen);
            }
            if self.widget_picker {
                self.widget_picker_ui(ctx, screen);
            }
            if self.dialog.is_some() {
                self.dialog_ui(ctx, screen);
            }
        }
        if self.whats_new.is_some() || self.notes_t.x > 0.01 {
            self.whats_new_ui(ctx, screen);
        }
        self.toast_ui(ctx, screen);

        if self.cfg.page_dots || self.has_live_widget() {
            let s = self.clock.sec as u64;
            let wait = if self.has_analog() && !self.light { 1 } else { 61u64.saturating_sub(s).max(1) };
            ctx.request_repaint_after(std::time::Duration::from_secs(wait));
        }
    }

    /// Gives new items a springy entrance and removed items a shrinking exit.
    fn track_items(&mut self, geo: &Geo) {
        if self.items_ready < 40 && self.cfg.seeded && !self.apps.is_empty() {
            self.items_ready += 1;
        }
        let ready = self.items_ready >= 40;
        let mut now: HashMap<String, (Item, Rect)> = HashMap::new();
        let page = self.page.round() as usize;
        for (pi, pg) in self.cfg.pages.iter().enumerate() {
            for pl in &pg.items {
                let (w, h) = pl.item.span(self.cfg.cols);
                let r = geo.cell_rect(pl.col, pl.row, w.min(self.cfg.cols), h.min(self.cfg.rows));
                let r = r.translate(vec2((pi as f32 - self.page) * geo.screen.width(), 0.0));
                now.insert(pl.item.key(), (pl.item.clone(), r));
            }
        }
        let n = self.cfg.dock_items.len();
        for (i, it) in self.cfg.dock_items.iter().enumerate() {
            now.insert(it.key(), (it.clone(), geo.dock_slot(i, n)));
        }
        for (key, (_, r)) in &now {
            if !self.item_sp.contains_key(key) {
                self.item_sp.insert(key.clone(), anim::Spring2::at(r.min));
                if ready {
                    self.scale_sp.insert(key.clone(), anim::Spring { x: 0.55, v: 0.0, target: 1.0 });
                }
            }
        }
        let dragging = if let Touch::Drag(d) = &self.touch { Some(d.item.key()) } else { None };
        let lifted = if let Touch::Lifted { item, .. } = &self.touch { Some(item.key()) } else { None };
        if ready {
            for (key, (item, r)) in &self.prev_items {
                if !now.contains_key(key) && dragging.as_deref() != Some(key.as_str()) && lifted.as_deref() != Some(key.as_str()) && self.edit_gone_ok(key) {
                    let on_page = r.right() > 0.0 && r.left() < geo.screen.width();
                    if on_page {
                        self.ghosts.push(Ghost { item: item.clone(), from: *r, to: Rect::from_center_size(r.center(), r.size() * 0.55), t: anim::Spring::at(0.0), icon_space: false });
                    }
                }
            }
        }
        let _ = page;
        self.prev_items = now;
    }

    /// Items that vanish because a drag picked them up are not "removed".
    fn edit_gone_ok(&self, key: &str) -> bool {
        !matches!(&self.touch, Touch::Drag(d) if d.item.key() == key)
    }

    /// One frame of every spring that is not following a finger.
    fn step_springs(&mut self, ctx: &Context, dt: f32) {
        let f = self.feel;
        let count = self.page_count().max(1);
        let mut busy = false;
        if !matches!(self.touch, Touch::Pager) {
            let mut sp = anim::Spring { x: self.page, v: self.page_v, target: self.page_target.clamp(0.0, (count - 1) as f32) };
            busy |= sp.step(dt, f.page);
            self.page = sp.x;
            self.page_v = sp.v;
        }
        if !matches!(self.touch, Touch::DrawerPull) {
            let mut sp = anim::Spring { x: self.drawer, v: self.drawer_v, target: if self.drawer_open { 1.0 } else { 0.0 } };
            busy |= sp.step(dt, f.drawer);
            self.drawer = sp.x;
            self.drawer_v = sp.v;
        }
        let mut sp = anim::Spring { x: self.folder_t, v: self.folder_v, target: if self.open_folder.is_some() || self.open_category.is_some() { 1.0 } else { 0.0 } };
        busy |= sp.step(dt, f.popup);
        self.folder_t = sp.x;
        self.folder_v = sp.v;
        if !matches!(self.touch, Touch::LibraryScroll) {
            busy |= self.lib_scroller.step(dt, self.lib_max);
        }
        self.notes_t.target = if self.whats_new.is_some() { 1.0 } else { 0.0 };
        busy |= self.notes_t.step(dt, f.popup);
        for g in self.ghosts.iter_mut() {
            g.t.target = 1.0;
            busy |= g.t.step(dt, f.item);
        }
        self.ghosts.retain(|g| g.t.x < 0.97 || g.t.moving());
        self.edit_t.target = if self.edit { 1.0 } else { 0.0 };
        busy |= self.edit_t.step(dt, f.press);
        self.resume_t.target = 0.0;
        busy |= self.resume_t.step(dt, f.screen);
        self.menu_t.target = if self.menu.is_some() { 1.0 } else { 0.0 };
        busy |= self.menu_t.step(dt, f.menu);
        if self.menu.is_none() && self.menu_t.x < 0.02 {
            self.menu_t.snap(0.0);
        }
        // press / lift scale of single items
        let press = match self.cfg.preset {
            Preset::Ios => 0.88,
            Preset::Pixel => 0.92,
            _ => 0.95,
        };
        let lifted = if let Touch::Lifted { item, .. } = &self.touch { Some(item.key()) } else { None };
        let pressed = self.press_key.clone();
        let mut keys: Vec<String> = self.scale_sp.keys().cloned().collect();
        keys.extend(lifted.iter().cloned());
        keys.extend(pressed.iter().cloned());
        keys.sort();
        keys.dedup();
        for k in keys {
            let target = if lifted.as_deref() == Some(k.as_str()) { 1.12 } else if pressed.as_deref() == Some(k.as_str()) { press } else { 1.0 };
            let sp = self.scale_sp.entry(k.clone()).or_insert_with(|| anim::Spring::at(1.0));
            sp.target = target;
            busy |= sp.step(dt, f.press);
            if !sp.moving() && target == 1.0 {
                self.scale_sp.remove(&k);
            }
        }
        if busy {
            ctx.request_repaint();
        }
    }

    fn has_live_widget(&self) -> bool {
        self.cfg.pages.iter().any(|p| p.items.iter().any(|i| matches!(i.item, Item::Widget { .. })))
    }

    fn has_analog(&self) -> bool {
        self.cfg.pages.iter().any(|p| p.items.iter().any(|i| matches!(i.item, Item::Widget { kind: WidgetKind::ClockAnalog, .. })))
    }

    fn overlay_blocks_home(&self) -> bool {
        self.drawer > 0.02 || self.menu.is_some() || self.dialog.is_some() || self.widget_picker || self.search_open
    }

    // ───────────────────────────── painting ─────────────────────────────

    fn paint_home(&mut self, ctx: &Context, geo: &Geo, dt: f32) {
        let pal = self.pal;
        let cfg = self.cfg.clone();
        let screen = geo.screen;
        let w = screen.width();
        let centre = screen.center().to_vec2();
        let page = self.page;
        let t_now = ctx.input(|i| i.time) as f32;
        let dragging_key = if let Touch::Drag(d) = &self.touch { Some(d.item.key()) } else { None };
        let edit = self.edit;
        let edit_amp = self.edit_t.x.clamp(0.0, 1.0);
        let live_clock = self.clock.clone();
        let aw_rc = self.aw.clone();
        let aw_guard = aw_rc.borrow();
        let live = cards::Live { clock: &live_clock, battery: self.battery, aw: Some(&*aw_guard), ppp: ctx.pixels_per_point(), now: ctx.input(|i| i.time) };
        let count = cfg.pages.len() + usize::from(cfg.app_library);

        // how far the home screen steps back while something opens over it
        let amount = self.drawer.clamp(0.0, 1.0).max(self.folder_t.clamp(0.0, 1.0)).max(self.screen_t.x.clamp(0.0, 1.0));
        let (recede, fade) = match cfg.preset {
            Preset::Ios => (0.08, 0.0),
            Preset::Pixel => (0.05, 0.5),
            Preset::Nothing => (0.0, 0.75),
            Preset::Custom => (0.04, 0.3),
        };
        let resume = self.resume_t.x.clamp(0.0, 1.0);
        let r = 1.0 - recede * amount + 0.06 * resume;
        let tf = |tx: f32, s: f32| {
            let s_total = r * s;
            egui::emath::TSTransform::new(centre * (1.0 - s_total) + vec2(tx * r, 0.0), s_total)
        };
        let base_alpha = (1.0 - fade * amount) * (1.0 - 0.4 * resume);

        let lib = if cfg.app_library { Some((self.lib_groups(), self.lib_layout(geo, 0.0))) } else { None };
        if let Some((_, (_, _, content))) = &lib {
            self.lib_max = (content + self.lib_scroller.offset.min(0.0) - (geo.screen.height() - geo.top) + 120.0).max(0.0);
        }

        // take the heavy parts out of self so the layer closures can use them freely
        let mut icons = self.icons.take().unwrap();
        let mut item_sp = std::mem::take(&mut self.item_sp);
        let scale_sp = std::mem::take(&mut self.scale_sp);
        let apps = std::mem::take(&mut self.apps);
        let by_pkg = std::mem::take(&mut self.by_pkg);
        let badges = std::mem::take(&mut self.badges);
        let feel = self.feel;
        let mut moving = false;
        let find = |pkg: &str| by_pkg.get(pkg).map(|&i| &apps[i]);
        let icon_rect = |cell: Rect| {
            let icon = cfg.eff();
            let label_h = if cfg.labels { 16.0 * cfg.font_scale * cfg.label_size / 12.0 } else { 0.0 };
            let total = icon + if cfg.labels { 6.0 + label_h } else { 0.0 };
            let top = cell.top() + ((cell.height() - total) / 2.0).max(2.0);
            Rect::from_min_size(pos2(cell.center().x - icon / 2.0, top), vec2(icon, icon))
        };
        // 1. the wallpaper dim
        layer(ctx, "wall", 0, screen, egui::emath::TSTransform::IDENTITY, 1.0, |p| {
            if pal.scrim.a() > 0 {
                p.rect_filled(screen, 0, pal.scrim);
            }
        });

        // 2. pages, each in its own transformed layer so the effect moves everything on it, text too
        for pi in 0..cfg.pages.len().max(1) {
            let d = pi as f32 - page;
            let (tx, s, a) = page_effect(if crate::perf::light() { PageEffect::Slide } else { cfg.page_effect }, d, w);
            let show = d.abs() < 1.0 && a > 0.003;
            layer(ctx, "page", pi, screen, tf(tx, s), if show { a * base_alpha } else { 0.0 }, |p| {
                if !show {
                    return;
                }
                let Some(pg) = cfg.pages.get(pi) else { return };
                let mut labels = widgets::Labels::default();
                for pl in &pg.items {
                    let key = pl.item.key();
                    let (sw, sh) = (pl.item.span(cfg.cols).0.min(cfg.cols), pl.item.span(cfg.cols).1.min(cfg.rows));
                    let target = geo.cell_rect(pl.col, pl.row, sw, sh);
                    let sp = item_sp.entry(key.clone()).or_insert_with(|| anim::Spring2::at(target.min));
                    moving |= sp.step_to(target.min, dt, feel.item);
                    let cell = Rect::from_min_size(sp.pos(), target.size());
                    let sc = scale_sp.get(&key).map(|s| s.x).unwrap_or(1.0);
                    let jig = jiggle(t_now, &key) * edit_amp;
                    paint_item(p, &pl.item, cell, &icon_rect, &cfg, &pal, &mut icons, &find, &badges, &live, sc, jig, 1.0, true, sc > 1.05, &mut labels);
                    if edit && dragging_key.as_deref() != Some(key.as_str()) {
                        paint_remove_badge(p, &pl.item, cell, &icon_rect, &cfg, &pal);
                    }
                }
                labels.flush(p, &cfg);
            });
        }

        // the App Library is the last page
        if let Some((groups, (search_r, cards_l, _))) = &lib {
            let pi = cfg.pages.len();
            let d = pi as f32 - page;
            let (tx, sc, a) = page_effect(if crate::perf::light() { PageEffect::Slide } else { cfg.page_effect }, d, w);
            let show = d.abs() < 1.0 && a > 0.003;
            layer(ctx, "page", pi, screen, tf(tx, sc), if show { a * base_alpha } else { 0.0 }, |p| {
                if show {
                    paint_library(p, geo, groups, *search_r, cards_l, &cfg, &pal, &mut icons, &find, &badges);
                }
            });
        }
        self.finish_home(ctx, geo, dt, icons, item_sp, scale_sp, apps, by_pkg, badges, &cfg, &pal, &live, edit, t_now, dragging_key, moving, base_alpha, &tf, count, page);
    }

    /// Dock, search pill, dots and the overlays that sit above the pages.
    #[allow(clippy::too_many_arguments)]
    fn finish_home(
        &mut self,
        ctx: &Context,
        geo: &Geo,
        dt: f32,
        mut icons: crate::icons::Icons,
        mut item_sp: std::collections::HashMap<String, anim::Spring2>,
        scale_sp: std::collections::HashMap<String, anim::Spring>,
        apps: std::rc::Rc<Vec<platform::App>>,
        by_pkg: std::collections::HashMap<String, usize>,
        badges: std::collections::HashMap<String, u32>,
        cfg: &crate::config::Config,
        pal: &crate::theme::Palette,
        live: &cards::Live,
        edit: bool,
        t_now: f32,
        dragging_key: Option<String>,
        mut moving: bool,
        base_alpha: f32,
        tf: &dyn Fn(f32, f32) -> egui::emath::TSTransform,
        count: usize,
        page: f32,
    ) {
        let screen = geo.screen;
        let edit_amp = self.edit_t.x.clamp(0.0, 1.0);
        let feel = self.feel;
        let find = |pkg: &str| by_pkg.get(pkg).map(|&i| &apps[i]);
        let dock_icon = |c: Rect| {
            let up = if cfg.dock_labels { 8.0 } else { 0.0 };
            Rect::from_center_size(c.center() - vec2(0.0, up), vec2(cfg.dock_icon_size, cfg.dock_icon_size))
        };

        // 3. dock: it slides away on the App Library page, like iOS
        let dock_vis = if cfg.app_library { 1.0 - (page - (cfg.pages.len() as f32 - 1.0)).clamp(0.0, 1.0) } else { 1.0 };
        layer(ctx, "dock", 0, screen, tf(0.0, 1.0), dock_vis * base_alpha, |p| {
            if dock_vis < 0.02 {
                return;
            }
            if cfg.dock != DockStyle::Bare {
                if cfg.dock == DockStyle::Glass && cfg.preset == Preset::Ios {
                    widgets::liquid_glass(p, geo.dock, cfg.dock_radius, pal);
                } else if cfg.dock == DockStyle::Glass {
                    widgets::glass(p, geo.dock, cfg.dock_radius, pal);
                } else {
                    p.rect_filled(geo.dock, cr(cfg.dock_radius), pal.sheet);
                }
            }
            let n = cfg.dock_items.len();
            let mut labels = widgets::Labels::default();
            for (i, it) in cfg.dock_items.iter().enumerate() {
                let slot = geo.dock_slot(i, n);
                let key = it.key();
                let sp = item_sp.entry(key.clone()).or_insert_with(|| anim::Spring2::at(slot.min));
                moving |= sp.step_to(slot.min, dt, feel.item);
                let cell = Rect::from_min_size(sp.pos(), slot.size());
                let sc = scale_sp.get(&key).map(|s| s.x).unwrap_or(1.0);
                let jig = jiggle(t_now, &key) * edit_amp;
                paint_item(p, it, cell, &dock_icon, cfg, pal, &mut icons, &find, &badges, live, sc, jig, 1.0, cfg.dock_labels, sc > 1.05, &mut labels);
                if edit {
                    paint_remove_badge(p, it, cell, &dock_icon, cfg, pal);
                }
            }
            labels.flush(p, cfg);
        });

        // 4. search pill, page dots, edit chrome
        layer(ctx, "top", 0, screen, tf(0.0, 1.0), base_alpha, |p| {
            if let Some(s) = geo.search {
                if cfg.search != Search::Off {
                    widgets::search_pill(p, s, "Search", cfg, pal, cfg.preset == Preset::Pixel);
                }
            }
            if cfg.page_dots && count > 1 {
                paint_dots(p, geo, cfg, pal, count, page);
            }
        });

        // restore what we borrowed
        self.icons = Some(icons);
        self.item_sp = item_sp;
        self.scale_sp = scale_sp;
        self.apps = apps;
        self.by_pkg = by_pkg;
        self.badges = badges;

        // 5. things above: popup, edit chrome, the dragged item
        self.paint_folder_popup(ctx, geo);
        if self.edit {
            let me = &*self;
            layer(ctx, "chrome", 0, screen, egui::emath::TSTransform::IDENTITY, 1.0, |p| me.paint_edit_chrome(p, geo));
        }
        self.paint_resize(ctx, geo);
        self.paint_drag(ctx, geo);
        self.paint_ghosts(ctx, geo);
        // keep the stacking order stable when pages come and go
        for name in ["dock", "top", "popup", "chrome", "resize", "drag", "ghost"] {
            ctx.move_to_top(egui::LayerId::new(egui::Order::Middle, egui::Id::new(("lumen-layer", name, 0usize))));
        }
        if moving {
            ctx.request_repaint();
        }
    }

    fn paint_edit_chrome(&self, p: &Painter, geo: &Geo) {
        let pal = &self.pal;
        let y = edit_chrome_y(geo);
        let plus = Rect::from_center_size(pos2(geo.screen.left() + 34.0, y), vec2(40.0, 40.0));
        p.circle_filled(plus.center(), 18.0, Color32::from_white_alpha(60));
        widgets::glyph_at(p, plus.center(), Align2::CENTER_CENTER, ph::PLUS, 20.0, Color32::WHITE);
        let gear = pos2(geo.screen.left() + 34.0 + 48.0, y);
        p.circle_filled(gear, 18.0, Color32::from_white_alpha(60));
        widgets::glyph_at(p, gear, Align2::CENTER_CENTER, ph::GEAR, 20.0, Color32::WHITE);
        let done = Rect::from_center_size(pos2(geo.screen.right() - 52.0, y), vec2(80.0, 36.0));
        p.rect_filled(done, cr(18.0), Color32::from_white_alpha(60));
        p.text(done.center(), Align2::CENTER_CENTER, "Done", fonts::body(&self.cfg, 16.0, true), Color32::WHITE);
        let _ = pal;
    }

    fn lib_groups(&self) -> Vec<(String, Option<crate::platform::Category>, Vec<String>)> {
        let mut groups = vec![];
        let sug: Vec<String> = self.suggested(4).into_iter().map(|a| a.pkg.clone()).collect();
        if !sug.is_empty() {
            groups.push(("Suggestions".to_string(), None, sug));
        }
        for cat in platform::Category::ALL {
            let mut v: Vec<&platform::App> = self.apps.iter().filter(|a| a.category == cat && !self.cfg.hidden.contains(&a.pkg)).collect();
            if v.is_empty() {
                continue;
            }
            v.sort_by(|a, b| self.cfg.launches.get(&b.pkg).cmp(&self.cfg.launches.get(&a.pkg)).then(a.key.cmp(&b.key)));
            groups.push((cat.name().to_string(), Some(cat), v.into_iter().map(|a| a.pkg.clone()).collect()));
        }
        groups
    }

    /// Rect of each library card, its four icon slots, and the total content height.
    fn lib_layout(&self, geo: &Geo, off: f32) -> (Rect, Vec<(Rect, Vec<Rect>)>, f32) {
        let w = geo.screen.width();
        let margin = 16.0;
        let gap = 14.0;
        let card = (w - 2.0 * margin - gap) / 2.0;
        let top = geo.top + 14.0;
        let search = Rect::from_min_size(pos2(margin + off, top - self.lib_scroller.offset.min(0.0)), vec2(w - 2.0 * margin, 44.0));
        let mut out = vec![];
        let groups = self.lib_groups();
        let start = top + 44.0 + 20.0;
        let pitch = card + 26.0 + gap;
        for i in 0..groups.len() {
            let (col, row) = (i % 2, i / 2);
            let r = Rect::from_min_size(pos2(margin + off + col as f32 * (card + gap), start + row as f32 * pitch - self.lib_scroller.offset), vec2(card, card));
            let pad = card * 0.09;
            let icon = (card - 2.0 * pad - card * 0.07) / 2.0;
            let slots: Vec<Rect> = (0..4)
                .map(|s| Rect::from_min_size(r.min + vec2(pad + (s % 2) as f32 * (icon + card * 0.07), pad + (s / 2) as f32 * (icon + card * 0.07)), vec2(icon, icon)))
                .collect();
            out.push((r, slots));
        }
        let rows = groups.len().div_ceil(2) as f32;
        let content = start + rows * pitch - geo.top;
        (search, out, content)
    }

    // folder / category popup
    fn popup_apps(&self) -> Option<(String, Vec<String>, bool)> {
        if let Some(id) = self.open_folder {
            for pg in &self.cfg.pages {
                for pl in &pg.items {
                    if let Item::Folder { id: fid, name, apps } = &pl.item {
                        if *fid == id {
                            return Some((name.clone(), apps.clone(), true));
                        }
                    }
                }
            }
            for it in &self.cfg.dock_items {
                if let Item::Folder { id: fid, name, apps } = it {
                    if *fid == id {
                        return Some((name.clone(), apps.clone(), true));
                    }
                }
            }
        }
        if let Some(cat) = self.open_category {
            let mut v: Vec<&platform::App> = self.apps.iter().filter(|a| a.category == cat && !self.cfg.hidden.contains(&a.pkg)).collect();
            v.sort_by(|a, b| a.key.cmp(&b.key));
            return Some((cat.name().to_string(), v.into_iter().map(|a| a.pkg.clone()).collect(), false));
        }
        None
    }

    pub fn popup_geo(&self, geo: &Geo, n: usize) -> (Rect, Rect, Vec<Rect>) {
        let cols = self.cfg.folder_cols.clamp(2, 5);
        let rows = n.div_ceil(cols).clamp(1, 5);
        let icon = self.cfg.eff();
        let cw = (icon + 34.0).max(86.0);
        let ch = icon + 36.0;
        let panel_w = cols as f32 * cw + 28.0;
        let title_h = 54.0;
        let panel_h = rows as f32 * ch + 28.0 + title_h;
        let center = pos2(geo.screen.center().x, geo.screen.center().y - 20.0);
        let panel = Rect::from_center_size(center, vec2(panel_w, panel_h));
        let title = Rect::from_min_size(panel.min + vec2(14.0, 6.0), vec2(panel_w - 28.0, title_h - 6.0));
        let cells = (0..n.min(cols * rows)).map(|i| Rect::from_min_size(panel.min + vec2(14.0 + (i % cols) as f32 * cw, title_h + 10.0 + (i / cols) as f32 * ch), vec2(cw, ch))).collect();
        (panel, title, cells)
    }

    fn paint_folder_popup(&mut self, ctx: &Context, geo: &Geo) {
        let t = self.folder_t;
        let popup = self.popup_apps();
        let Some((name, pkgs, _)) = popup else {
            if t <= 0.01 {
                self.folder_origin = None;
            }
            // keep the layer alive and empty so the stacking order stays put
            layer(ctx, "popup", 0, geo.screen, egui::emath::TSTransform::IDENTITY, 0.0, |_| {});
            return;
        };
        if t <= 0.001 {
            layer(ctx, "popup", 0, geo.screen, egui::emath::TSTransform::IDENTITY, 0.0, |_| {});
            return;
        }
        let pal = self.pal;
        let (panel, title, cells) = self.popup_geo(geo, pkgs.len());
        let origin = self.folder_origin.map(|r| r.center()).unwrap_or(panel.center());
        let s = 0.22 + 0.78 * t;
        let xf = egui::emath::TSTransform::new(origin.to_vec2() * (1.0 - s), s);
        let alpha = (t * 2.2).clamp(0.0, 1.0);
        let cfg = self.cfg.clone();
        let mut icons = self.icons.take().unwrap();
        let ios = cfg.preset == Preset::Ios;
        let tcol = if ios { Color32::WHITE } else { pal.text };
        let screen = geo.screen;
        // the dim sits in its own layer: it must not shrink with the panel
        layer(ctx, "popup-dim", 0, screen, egui::emath::TSTransform::IDENTITY, 1.0, |p| {
            p.rect_filled(screen, 0, Color32::from_black_alpha((t.clamp(0.0, 1.0) * if ios { 90.0 } else { 130.0 }) as u8));
        });
        layer(ctx, "popup", 0, screen, xf, alpha, |p| {
            let panel_fill = if ios { Color32::from_rgba_unmultiplied(70, 70, 80, 215) } else { pal.sheet };
            p.rect_filled(panel, cr(if ios { 34.0 } else { 36.0 }), panel_fill);
            p.text(title.center_top() + vec2(0.0, 12.0), Align2::CENTER_TOP, &name, fonts::body(&cfg, 20.0, true), tcol);
            for (pkg, cell) in pkgs.iter().zip(cells.iter()) {
                let Some(&ai) = self.by_pkg.get(pkg) else { continue };
                let ir = Rect::from_center_size(pos2(cell.center().x, cell.top() + 12.0 + cfg.eff() / 2.0), vec2(cfg.eff(), cfg.eff()));
                widgets::app_icon(p, ir, &self.apps[ai], &mut icons, &cfg, &pal, 1.0, 0.0, 1.0);
                if let Some(c) = self.badges.get(pkg) {
                    widgets::badge(p, ir, *c, &cfg, &pal);
                }
                if cfg.labels {
                    widgets::label(p, pos2(cell.center().x, ir.bottom() + 5.0), cfg.names.get(pkg).map(|s| s.as_str()).unwrap_or(&self.apps[ai].label), &cfg, tcol, cell.width() - 6.0, false, 1.0);
                }
            }
        });
        self.icons = Some(icons);
    }

    fn paint_drag(&mut self, ctx: &Context, geo: &Geo) {
        let Touch::Drag(d) = self.touch.clone() else {
            layer(ctx, "drag", 0, geo.screen, egui::emath::TSTransform::IDENTITY, 0.0, |_| {});
            return;
        };
        let pal = self.pal;
        let cfg = self.cfg.clone();
        let zones = self.zones(geo, &d);
        let hint: Option<Rect> = match &d.hover {
            Hover::Cell { page, col, row } if *page == self.page.round() as usize => {
                let (w, h) = d.item.span(cfg.cols);
                cfg.nearest_free(*page, *col, *row, w.min(cfg.cols), h.min(cfg.rows), None).map(|(c, r)| self.icon_rect_for(&d.item, geo.cell_rect(c, r, w, h).shrink(4.0)))
            }
            Hover::Merge(key) => self.rect_of_key(geo, key).map(|r| self.icon_rect(r).expand(8.0)),
            _ => None,
        };
        let merging = matches!(d.hover, Hover::Merge(_));
        let live_clock = self.clock.clone();
        let aw_rc = self.aw.clone();
        let aw_guard = aw_rc.borrow();
        let live = cards::Live { clock: &live_clock, battery: self.battery, aw: Some(&*aw_guard), ppp: ctx.pixels_per_point(), now: ctx.input(|i| i.time) };
        let mut icons = self.icons.take().unwrap();
        let center = d.pos - d.grab;
        let (sw, sh) = d.item.span(cfg.cols);
        let size = match &d.item {
            Item::Widget { .. } | Item::AppWidget { .. } => vec2(sw.min(cfg.cols) as f32 * geo.cell.x, sh.min(cfg.rows) as f32 * geo.cell.y),
            _ => vec2(cfg.eff(), cfg.eff()),
        };
        let whole = Rect::from_center_size(center, size * 1.1);
        let font = fonts::body(&cfg, 14.5, true);
        layer(ctx, "drag", 0, geo.screen, egui::emath::TSTransform::IDENTITY, 1.0, |p| {
            if let Some(r) = hint {
                if merging {
                    p.rect_filled(r, cr(widgets::plate_radius(&cfg, r.width())), Color32::from_white_alpha(70));
                } else {
                    p.rect_stroke(r, cr(widgets::plate_radius(&cfg, cfg.eff())), Stroke::new(1.5, pal.home_text.gamma_multiply(0.45)), StrokeKind::Inside);
                }
            }
            for z in &zones {
                let on = d.hover == z.hover;
                let bg = if on { pal.accent } else { Color32::from_black_alpha(120) };
                p.rect_filled(z.rect, cr(26.0), bg);
                let fg = if on { pal.on_accent } else { Color32::WHITE };
                let tw = widgets::text_width(p, z.label, &font);
                let start = z.rect.center().x - (tw + 30.0) / 2.0;
                widgets::glyph_at(p, pos2(start, z.rect.center().y), Align2::LEFT_CENTER, z.glyph, 20.0, fg);
                p.text(pos2(start + 30.0, z.rect.center().y), Align2::LEFT_CENTER, z.label, font.clone(), fg);
            }
            widgets::shadow(p, whole, widgets::plate_radius(&cfg, whole.width()), 90);
            let find = |pkg: &str| self.by_pkg.get(pkg).map(|&i| &self.apps[i]);
            let id = |c: Rect| Rect::from_center_size(c.center(), vec2(cfg.eff() * 1.1, cfg.eff() * 1.1));
            let mut labels = widgets::Labels::default();
            paint_item(p, &d.item, whole, &id, &cfg, &pal, &mut icons, &find, &self.badges, &live, 1.0, 0.0, 1.0, false, false, &mut labels);
            labels.flush(p, &cfg);
        });
        self.icons = Some(icons);
        ctx.request_repaint();
    }

    /// The icon (or widget) rectangle of an item that is on the page or in the dock now.
    fn key_icon_rect(&self, geo: &Geo, key: &str) -> Option<Rect> {
        let r = self.rect_of_key(geo, key)?;
        if self.cfg.dock_items.iter().any(|i| i.key() == key) {
            return Some(self.dock_icon_rect(r));
        }
        let widget = self.cfg.pages.iter().flat_map(|p| p.items.iter()).any(|p| p.item.key() == key && matches!(p.item, Item::Widget { .. } | Item::AppWidget { .. }));
        Some(if widget { r } else { self.icon_rect(r) })
    }

    fn paint_ghosts(&mut self, ctx: &Context, geo: &Geo) {
        if self.ghosts.is_empty() {
            layer(ctx, "ghost", 0, geo.screen, egui::emath::TSTransform::IDENTITY, 0.0, |_| {});
            return;
        }
        let cfg = self.cfg.clone();
        let pal = self.pal;
        let live_clock = self.clock.clone();
        let live = cards::Live { clock: &live_clock, battery: self.battery, aw: None, ppp: 1.0, now: 0.0 };
        let mut icons = self.icons.take().unwrap();
        let ghosts: Vec<(Item, Rect, f32, bool)> = self
            .ghosts
            .iter()
            .map(|g| {
                let t = g.t.x.clamp(0.0, 1.2);
                let lerp = |a: f32, b: f32| a + (b - a) * t;
                let r = Rect::from_min_max(pos2(lerp(g.from.min.x, g.to.min.x), lerp(g.from.min.y, g.to.min.y)), pos2(lerp(g.from.max.x, g.to.max.x), lerp(g.from.max.y, g.to.max.y)));
                (g.item.clone(), r, (1.0 - t * if g.icon_space { 0.9 } else { 1.0 }).clamp(0.0, 1.0), g.icon_space)
            })
            .collect();
        layer(ctx, "ghost", 0, geo.screen, egui::emath::TSTransform::IDENTITY, 1.0, |p| {
            let find = |pkg: &str| self.by_pkg.get(pkg).map(|&i| &self.apps[i]);
            let mut labels = widgets::Labels::default();
            for (item, rect, alpha, icon_space) in &ghosts {
                if *icon_space {
                    if let Item::App(pkg) = item {
                        if let Some(app) = find(pkg) {
                            widgets::app_icon(p, *rect, app, &mut icons, &cfg, &pal, 1.0, 0.0, *alpha);
                        }
                    }
                } else {
                    let ic = |c: Rect| {
                        let size = cfg.eff();
                        let label_h = if cfg.labels { 16.0 * cfg.font_scale * cfg.label_size / 12.0 } else { 0.0 };
                        let total = size + if cfg.labels { 6.0 + label_h } else { 0.0 };
                        let top = c.top() + ((c.height() - total) / 2.0).max(2.0);
                        Rect::from_min_size(pos2(c.center().x - size / 2.0, top), vec2(size, size))
                    };
                    paint_item(p, item, *rect, &ic, &cfg, &pal, &mut icons, &find, &self.badges, &live, 1.0, 0.0, *alpha, true, false, &mut labels);
                }
            }
            labels.flush(p, &cfg);
        });
        self.icons = Some(icons);
    }

    fn icon_rect_for(&self, item: &Item, cell: Rect) -> Rect {
        match item {
            Item::Widget { .. } | Item::AppWidget { .. } => cell,
            _ => self.icon_rect(cell),
        }
    }

    fn rect_of_key(&self, geo: &Geo, key: &str) -> Option<Rect> {
        let pi = self.page.round() as usize;
        if let Some(pg) = self.cfg.pages.get(pi) {
            for pl in &pg.items {
                if pl.item.key() == key {
                    let (w, h) = pl.item.span(self.cfg.cols);
                    return Some(geo.cell_rect(pl.col, pl.row, w, h));
                }
            }
        }
        let n = self.cfg.dock_items.len();
        self.cfg.dock_items.iter().position(|i| i.key() == key).map(|i| geo.dock_slot(i, n))
    }

    // ───────────────────────────── widget resize ─────────────────────────────

    /// (page, index, col, row, w, h) of the widget being resized.
    fn resize_target(&self) -> Option<(usize, usize, usize, usize, usize, usize)> {
        let key = self.resize.as_ref()?;
        for (pi, pg) in self.cfg.pages.iter().enumerate() {
            for (i, pl) in pg.items.iter().enumerate() {
                if pl.item.key() == *key {
                    let (w, h) = pl.item.span(self.cfg.cols);
                    return Some((pi, i, pl.col, pl.row, w.min(self.cfg.cols), h.min(self.cfg.rows)));
                }
            }
        }
        None
    }

    /// The four handle centres of the selected widget: left, top, right, bottom.
    fn resize_handles(&self, geo: &Geo) -> Option<(Rect, [Pos2; 4])> {
        let (pi, _, c, r, w, h) = self.resize_target()?;
        if pi != self.page.round() as usize {
            return None;
        }
        let rect = geo.cell_rect(c, r, w, h).shrink(3.0);
        Some((rect, [rect.left_center(), rect.center_top(), rect.right_center(), rect.center_bottom()]))
    }

    fn resize_drag(&mut self, geo: &Geo, edge: u8, pos: Pos2) {
        let Some((pi, idx, c, r, w, h)) = self.resize_target() else { return };
        let (cols, rows) = (self.cfg.cols as i32, self.cfg.rows as i32);
        let gx = ((pos.x - geo.grid.left()) / geo.cell.x).round() as i32;
        let gy = ((pos.y - geo.grid.top()) / geo.cell.y).round() as i32;
        let (mut nc, mut nr, mut nw, mut nh) = (c as i32, r as i32, w as i32, h as i32);
        match edge {
            0 => {
                let left = gx.clamp(0, c as i32 + w as i32 - 1);
                nw = c as i32 + w as i32 - left;
                nc = left;
            }
            1 => {
                let top = gy.clamp(0, r as i32 + h as i32 - 1);
                nh = r as i32 + h as i32 - top;
                nr = top;
            }
            2 => nw = (gx - c as i32).clamp(1, cols - c as i32),
            _ => nh = (gy - r as i32).clamp(1, rows - r as i32),
        }
        if (nc, nr, nw, nh) == (c as i32, r as i32, w as i32, h as i32) {
            return;
        }
        let key = self.resize.clone().unwrap_or_default();
        if !self.cfg.fits(pi, nc as usize, nr as usize, nw as usize, nh as usize, Some(&key)) {
            return;
        }
        let pl = &mut self.cfg.pages[pi].items[idx];
        pl.col = nc as usize;
        pl.row = nr as usize;
        if let Item::AppWidget { w, h, .. } = &mut pl.item {
            *w = nw as usize;
            *h = nh as usize;
        }
        self.dirty = true;
        self.haptic(Haptic::Tick);
    }

    fn paint_resize(&self, ctx: &Context, geo: &Geo) {
        let handles = self.resize_handles(geo);
        let pal = self.pal;
        layer(ctx, "resize", 0, geo.screen, egui::emath::TSTransform::IDENTITY, if handles.is_some() { 1.0 } else { 0.0 }, |p| {
            let Some((rect, hs)) = handles else { return };
            p.rect_stroke(rect.expand(2.0), cr(self.cfg.widget_radius + 2.0), Stroke::new(2.5, pal.accent), StrokeKind::Outside);
            for (i, c) in hs.iter().enumerate() {
                let size = if i % 2 == 0 { vec2(8.0, 30.0) } else { vec2(30.0, 8.0) };
                p.rect_filled(Rect::from_center_size(*c, size + vec2(4.0, 4.0)), cr(6.0), Color32::from_black_alpha(90));
                p.rect_filled(Rect::from_center_size(*c, size), cr(4.0), pal.accent);
            }
        });
    }

    // ───────────────────────────── hit testing ─────────────────────────────

    fn hit_at(&self, geo: &Geo, pos: Pos2) -> Hit {
        let w = geo.screen.width();
        if self.edit {
            if (pos - pos2(geo.screen.left() + 34.0, edit_chrome_y(geo))).length() < 26.0 {
                return Hit::EditAdd;
            }
            if (pos - pos2(geo.screen.left() + 34.0 + 48.0, edit_chrome_y(geo))).length() < 26.0 {
                return Hit::EditSettings;
            }
            if Rect::from_center_size(pos2(geo.screen.right() - 52.0, edit_chrome_y(geo)), vec2(90.0, 44.0)).contains(pos) {
                return Hit::EditDone;
            }
        }
        if let Some((_, pkgs, _)) = self.popup_apps() {
            let (_, _, cells) = self.popup_geo(geo, pkgs.len());
            for (pkg, cell) in pkgs.iter().zip(cells) {
                if cell.contains(pos) {
                    return Hit::FolderApp(self.open_folder.unwrap_or(0), pkg.clone());
                }
            }
            return Hit::None;
        }
        let page = self.page.round();
        if (self.page - page).abs() > 0.05 {
            return Hit::None;
        }
        let pi = page as usize;
        if self.cfg.app_library && pi == self.cfg.pages.len() {
            let (search, cards_l, _) = self.lib_layout(geo, 0.0);
            if search.contains(pos) {
                return Hit::Search;
            }
            let groups = self.lib_groups();
            for ((_, cat, pkgs), (rect, slots)) in groups.iter().zip(cards_l) {
                if !rect.contains(pos) {
                    continue;
                }
                for (s, slot) in slots.iter().enumerate() {
                    if slot.expand(4.0).contains(pos) {
                        if s == 3 && pkgs.len() > 4 {
                            return cat.map(Hit::LibraryGroup).unwrap_or(Hit::None);
                        }
                        if let Some(pkg) = pkgs.get(s) {
                            return Hit::LibraryApp(pkg.clone());
                        }
                    }
                }
                return cat.map(Hit::LibraryGroup).unwrap_or(Hit::None);
            }
            let _ = w;
            return Hit::None;
        }
        if geo.search.is_some_and(|s| s.contains(pos)) && self.cfg.search != Search::Off {
            return Hit::Search;
        }
        let n = self.cfg.dock_items.len();
        for (i, it) in self.cfg.dock_items.iter().enumerate() {
            if geo.dock_slot(i, n).contains(pos) {
                return Hit::Item(it.key());
            }
        }
        if let Some(pg) = self.cfg.pages.get(pi) {
            for pl in &pg.items {
                let (w, h) = pl.item.span(self.cfg.cols);
                if geo.cell_rect(pl.col, pl.row, w.min(self.cfg.cols), h.min(self.cfg.rows)).contains(pos) {
                    return Hit::Item(pl.item.key());
                }
            }
        }
        Hit::None
    }

    fn find_item(&self, key: &str) -> Option<(Item, Source)> {
        for (pi, pg) in self.cfg.pages.iter().enumerate() {
            for pl in &pg.items {
                if pl.item.key() == key {
                    return Some((pl.item.clone(), Source::Page { page: pi, col: pl.col, row: pl.row }));
                }
            }
        }
        self.cfg.dock_items.iter().position(|i| i.key() == key).map(|i| (self.cfg.dock_items[i].clone(), Source::Dock(i)))
    }

    // ───────────────────────────── touch machine ─────────────────────────────

    fn touch(&mut self, ctx: &Context, geo: &Geo) {
        let (pressed, down, pos, time, delta, vel) = ctx.input(|i| (i.pointer.primary_pressed(), i.pointer.primary_down(), i.pointer.latest_pos(), i.time, i.pointer.delta(), i.pointer.velocity()));
        let slop = 10.0;
        let long_press = 0.40;
        let w = geo.screen.width();
        let h = geo.screen.height();
        let count = self.page_count().max(1);
        if !matches!(self.touch, Touch::Idle | Touch::Spent) || self.edit {
            // frames follow the finger, not the speed at which touch events happen to arrive
            ctx.request_repaint();
        }
        if !matches!(self.touch, Touch::Down { .. }) {
            self.press_key = None;
        }
        // two fingers pulling together
        if self.screen == Screen::Home && !self.overlay_blocks_home() && matches!(self.touch, Touch::Idle | Touch::Down { .. }) {
            if let Some(m) = ctx.input(|i| i.multi_touch()) {
                if m.zoom_delta < 0.985 && self.cfg.pinch != Action::None {
                    self.touch = Touch::Spent;
                    let a = self.cfg.pinch;
                    self.run_action(ctx, a);
                }
            }
        }
        match self.touch.clone() {
            Touch::Idle => {
                if pressed && !self.overlay_blocks_home() {
                    if let Some(pos) = pos {
                        if let Some((_, hs)) = self.resize_handles(geo) {
                            if let Some(e) = hs.iter().position(|c| (pos - *c).length() < 34.0) {
                                self.touch = Touch::Resize(e as u8);
                                self.haptic(Haptic::Lift);
                                return;
                            }
                        }
                        if self.resize.is_some() {
                            // a press anywhere else ends the resizing
                            self.resize = None;
                        }
                        let hit = self.hit_at(geo, pos);
                        self.press_key = match (&hit, self.edit) {
                            (Hit::Item(k), false) => Some(k.clone()),
                            _ => None,
                        };
                        self.touch = Touch::Down { start: pos, t0: time, hit };
                    }
                }
            }
            Touch::Down { start, t0, hit } => {
                let pos = pos.unwrap_or(start);
                let moved = (pos - start).length();
                if !down {
                    self.tap(ctx, geo, start, hit, time);
                    self.touch = Touch::Idle;
                } else if moved > slop {
                    let d = pos - start;
                    let on_item = matches!(hit, Hit::Item(_) | Hit::FolderApp(..) | Hit::LibraryApp(_));
                    if self.edit && on_item && !self.cfg.lock_layout {
                        self.pick_up(ctx, geo, &hit, pos, start);
                    } else if self.open_folder.is_some() || self.open_category.is_some() {
                        self.touch = Touch::Spent;
                    } else if d.x.abs() > d.y.abs() {
                        self.touch = Touch::Pager;
                        self.page -= d.x / w;
                    } else if self.cfg.app_library && self.page.round() as usize == self.cfg.pages.len() && (self.lib_scroller.offset > 0.5 || d.y < 0.0) {
                        self.touch = Touch::LibraryScroll;
                    } else if d.y < 0.0 {
                        if self.cfg.swipe_up == Action::Drawer {
                            self.touch = Touch::DrawerPull;
                            self.drawer = (-d.y / (h * 0.55)).clamp(0.0, 1.0);
                        } else {
                            self.run_action(ctx, self.cfg.swipe_up);
                            self.touch = Touch::Spent;
                        }
                    } else {
                        let a = self.cfg.swipe_down;
                        self.run_action(ctx, a);
                        self.touch = Touch::Spent;
                    }
                } else if time - t0 > long_press {
                    self.long_press(ctx, geo, &hit, start);
                }
            }
            Touch::Lifted { start, item, from, rect } => {
                if !down {
                    self.touch = Touch::Idle;
                } else if pos.is_some_and(|p| (p - start).length() > slop) && !self.cfg.lock_layout {
                    self.menu = None;
                    let pos = pos.unwrap_or(start);
                    self.begin_drag(ctx, item, from, pos, start - rect.center(), time);
                }
            }
            Touch::Pager => {
                let max = (count - 1) as f32;
                let rubber = if self.page < 0.0 || self.page > max { 0.35 } else { 1.0 };
                self.page -= delta.x / w * rubber;
                if !down {
                    // carry the finger's speed into the spring so the page keeps moving the way it was flicked
                    let v = -vel.x / w;
                    self.page_v = v;
                    self.page_target = (self.page + v * 0.2).round().clamp(0.0, max);
                    self.touch = Touch::Idle;
                }
            }
            Touch::DrawerPull => {
                self.drawer = (self.drawer - delta.y / (h * 0.55)).clamp(0.0, 1.0);
                if !down {
                    let v = -vel.y / (h * 0.55);
                    self.drawer_v = v;
                    self.drawer_open = self.drawer + v * 0.18 > 0.5;
                    self.touch = Touch::Idle;
                }
            }
            Touch::LibraryScroll => {
                let max = self.lib_max;
                self.lib_scroller.drag_by(delta.y, max);
                if !down {
                    self.lib_scroller.release(-vel.y);
                    self.touch = Touch::Idle;
                }
            }
            Touch::Spent => {
                if !down {
                    self.touch = Touch::Idle;
                }
            }
            Touch::Resize(edge) => {
                if let Some(pos) = pos {
                    self.resize_drag(geo, edge, pos);
                }
                if !down {
                    self.touch = Touch::Idle;
                }
            }
            Touch::Drag(mut d) => {
                if let Some(pos) = pos {
                    d.pos = pos;
                }
                if !down {
                    // decide what is under the finger where it let go
                    self.update_drag(ctx, geo, &mut d, time);
                    self.drop_drag(ctx, geo, d, time);
                } else {
                    self.update_drag(ctx, geo, &mut d, time);
                    self.touch = Touch::Drag(d);
                    ctx.request_repaint();
                }
            }
        }
    }

    fn tap(&mut self, ctx: &Context, geo: &Geo, pos: Pos2, hit: Hit, time: f64) {
        if self.edit {
            match hit {
                Hit::EditDone => self.edit = false,
                Hit::EditAdd => self.widget_picker = true,
                Hit::EditSettings => self.open_settings(),
                Hit::Item(key) => {
                    // the remove badge sits on the top-left corner of the icon
                    if let Some(ir) = self.key_icon_rect(geo, &key) {
                        if (pos - ir.left_top()).length() < 24.0 {
                            self.remove_item(&key);
                        }
                    }
                }
                _ => {
                    self.edit = false;
                }
            }
            return;
        }
        match hit {
            Hit::Item(key) => {
                if let Some((item, _)) = self.find_item(&key) {
                    match item {
                        Item::App(pkg) => {
                            let from = self.key_icon_rect(geo, &key);
                            self.launch_at(ctx, &pkg, from);
                        }
                        Item::Folder { id, .. } => {
                            self.folder_origin = self.key_icon_rect(geo, &key);
                            self.open_folder = Some(id);
                            self.haptic(Haptic::Tick);
                        }
                        Item::Widget { kind, .. } => {
                            // a Lumen widget opens the app it stands for
                            let (action, data): (Option<&str>, Option<&str>) = match kind {
                                WidgetKind::SearchBar => (None, None),
                                WidgetKind::Calendar | WidgetKind::DotDate => (Some("android.intent.action.VIEW"), Some("content://com.android.calendar/time")),
                                WidgetKind::Battery | WidgetKind::DotBattery => (Some("android.intent.action.POWER_USAGE_SUMMARY"), None),
                                _ => (Some("android.intent.action.SHOW_ALARMS"), None),
                            };
                            match action {
                                None => self.open_search(),
                                Some(a) => {
                                    let (a, d) = (a.to_string(), data.map(String::from));
                                    crate::bridge::bg(move || platform::start_action(&a, d.as_deref()));
                                }
                            }
                        }
                        Item::AppWidget { wid, .. } => {
                            if let Some(r) = self.rect_of_key(geo, &key) {
                                let k = ctx.pixels_per_point();
                                let r = r.shrink(3.0);
                                self.aw.borrow().tap(wid, (pos.x - r.left()) * k, (pos.y - r.top()) * k);
                            }
                        }
                    }
                }
            }
            Hit::FolderApp(_, pkg) | Hit::LibraryApp(pkg) => self.launch(ctx, &pkg),
            Hit::LibraryGroup(cat) => self.open_category = Some(cat),
            Hit::Search => self.open_search(),
            Hit::None => {
                if self.open_folder.is_some() || self.open_category.is_some() {
                    self.open_folder = None;
                    self.open_category = None;
                } else if let Some((t, p)) = self.last_tap {
                    if time - t < 0.32 && (p - pos).length() < 60.0 {
                        let a = self.cfg.double_tap;
                        self.run_action(ctx, a);
                        self.last_tap = None;
                    } else {
                        self.last_tap = Some((time, pos));
                    }
                } else {
                    self.last_tap = Some((time, pos));
                }
            }
            _ => {}
        }
    }

    fn long_press(&mut self, ctx: &Context, geo: &Geo, hit: &Hit, start: Pos2) {
        self.haptic(Haptic::Lift);
        let time = ctx.input(|i| i.time);
        match hit {
            Hit::Item(key) => {
                let Some((item, from)) = self.find_item(key) else { return };
                let anchor = self.key_icon_rect(geo, key).unwrap_or(Rect::from_center_size(start, vec2(60.0, 60.0)));
                let target = match &item {
                    Item::App(p) => Target::App { pkg: p.clone(), on_home: true },
                    Item::Folder { id, .. } => Target::Folder(*id),
                    Item::Widget { id, .. } => Target::Widget(*id),
                    Item::AppWidget { id, .. } => Target::AppWidget(*id),
                };
                if self.edit {
                    return;
                }
                self.menu = Some(Menu { target, anchor });
                self.touch = Touch::Lifted { start, item, from, rect: anchor };
            }
            Hit::FolderApp(id, pkg) => {
                let (_, _, cells) = self.popup_geo(geo, self.popup_apps().map(|x| x.1.len()).unwrap_or(0));
                let idx = self.popup_apps().and_then(|x| x.1.iter().position(|p| p == pkg)).unwrap_or(0);
                let rect = cells.get(idx).copied().unwrap_or(Rect::from_center_size(start, vec2(60.0, 60.0)));
                let ir = Rect::from_center_size(pos2(rect.center().x, rect.top() + 12.0 + self.cfg.eff() / 2.0), vec2(self.cfg.eff(), self.cfg.eff()));
                let from = if self.open_folder.is_some() { Source::Folder(*id) } else { Source::Drawer };
                self.menu = Some(Menu { target: Target::App { pkg: pkg.clone(), on_home: false }, anchor: ir });
                self.touch = Touch::Lifted { start, item: Item::App(pkg.clone()), from, rect: ir };
            }
            Hit::LibraryApp(pkg) => {
                let r = Rect::from_center_size(start, vec2(self.cfg.eff(), self.cfg.eff()));
                self.menu = Some(Menu { target: Target::App { pkg: pkg.clone(), on_home: false }, anchor: r });
                self.touch = Touch::Lifted { start, item: Item::App(pkg.clone()), from: Source::Drawer, rect: r };
            }
            Hit::None | Hit::Search | Hit::LibraryGroup(_) => {
                if self.open_folder.is_some() || self.open_category.is_some() {
                    self.touch = Touch::Spent;
                    return;
                }
                match self.cfg.long_press_empty {
                    Action::Menu => self.menu = Some(Menu { target: Target::Home, anchor: Rect::from_center_size(start, vec2(10.0, 10.0)) }),
                    a => self.run_action(ctx, a),
                }
                self.touch = Touch::Spent;
            }
            _ => {}
        }
        let _ = time;
    }

    fn pick_up(&mut self, ctx: &Context, geo: &Geo, hit: &Hit, pos: Pos2, start: Pos2) {
        let time = ctx.input(|i| i.time);
        let (item, from, rect) = match hit {
            Hit::Item(key) => {
                let Some((item, from)) = self.find_item(key) else { return };
                let r = self.key_icon_rect(geo, key).unwrap_or(Rect::from_center_size(start, vec2(60.0, 60.0)));
                (item, from, r)
            }
            _ => return,
        };
        self.begin_drag(ctx, item, from, pos, start - rect.center(), time);
    }

    /// Takes the item off the layout and puts it on the finger.
    pub fn begin_drag(&mut self, ctx: &Context, item: Item, from: Source, pos: Pos2, grab: egui::Vec2, time: f64) {
        match &from {
            Source::Page { .. } | Source::Dock(_) => {
                self.cfg.remove_key(&item.key());
            }
            Source::Folder(_) => {
                if let Item::App(pkg) = &item {
                    self.cfg.remove_app(pkg);
                }
            }
            Source::Drawer => {}
        }
        self.open_folder = None;
        self.open_category = None;
        self.menu = None;
        if matches!(from, Source::Drawer) {
            self.drawer_open = false;
            self.close_drawer(ctx);
            self.search_open = false;
        }
        self.haptic(Haptic::Lift);
        self.dirty = true;
        self.touch = Touch::Drag(Drag { item, from, pos, grab, hover: Hover::None, hover_since: time, edge: None, cand: None });
    }

    fn update_drag(&mut self, _ctx: &Context, geo: &Geo, d: &mut Drag, time: f64) {
        let pos = d.pos;
        let w = geo.screen.width();
        let count = self.page_count();
        let pi = self.page.round() as usize;
        let on_page = pi < self.cfg.pages.len();

        // edge: switch pages after a short wait
        let dir = if pos.x < 30.0 { -1 } else if pos.x > w - 30.0 { 1 } else { 0 };
        match (dir, d.edge) {
            (0, _) => d.edge = None,
            (dir, Some((t0, old))) if dir == old => {
                if time - t0 > 0.6 {
                    let cur = self.page_target as i32;
                    let mut next = (cur + dir).max(0) as usize;
                    if next >= self.cfg.pages.len() {
                        let last_has_items = self.cfg.pages.last().is_some_and(|p| !p.items.is_empty());
                        if self.cfg.pages.len() < 8 && last_has_items && next == self.cfg.pages.len() {
                            self.cfg.pages.push(Page::default());
                        } else {
                            next = self.cfg.pages.len() - 1;
                        }
                    }
                    self.page_target = next.min(self.page_count() - 1) as f32;
                    d.edge = Some((time, dir));
                    self.haptic(Haptic::Tick);
                }
            }
            (dir, _) => d.edge = Some((time, dir)),
        }
        let _ = count;

        // what is under the finger
        let mut hover = Hover::None;
        for z in self.zones(geo, d) {
            if z.rect.contains(pos) {
                hover = z.hover;
            }
        }
        if hover == Hover::None {
            let n = self.cfg.dock_items.len();
            let is_widget = matches!(d.item, Item::Widget { .. } | Item::AppWidget { .. });
            if geo.dock.expand2(vec2(0.0, 18.0)).contains(pos) && !is_widget {
                // slot under the finger
                let total = geo.dock_slot_w * (n.max(1)) as f32;
                let left = geo.dock.center().x - total / 2.0;
                let idx = (((pos.x - left) / geo.dock_slot_w).floor().max(0.0) as usize).min(n);
                // dwell on an app to make a folder with it
                let target = self.cfg.dock_items.get(idx).filter(|_| (pos.x - (left + (idx as f32 + 0.5) * geo.dock_slot_w)).abs() < geo.dock_slot_w * 0.3);
                match target {
                    Some(t) if matches!(d.item, Item::App(_)) && matches!(t, Item::App(_) | Item::Folder { .. }) => {
                        let key = t.key();
                        if d.cand.as_deref() != Some(key.as_str()) {
                            d.cand = Some(key.clone());
                            d.hover_since = time;
                        }
                        hover = if time - d.hover_since > 0.35 { Hover::Merge(key) } else { Hover::Dock(idx) };
                    }
                    _ => {
                        d.cand = None;
                        hover = Hover::Dock(idx.min(self.cfg.dock_slots.saturating_sub(1)));
                    }
                }
            } else if on_page {
                if let Some((c, r)) = geo.cell_at(pos, self.cfg.cols, self.cfg.rows) {
                    let occupant = self.cfg.item_at(pi, c, r).map(|i| self.cfg.pages[pi].items[i].item.clone());
                    match occupant {
                        Some(t) if matches!(d.item, Item::App(_)) && matches!(t, Item::App(_) | Item::Folder { .. }) => {
                            let key = t.key();
                            if d.cand.as_deref() != Some(key.as_str()) {
                                d.cand = Some(key.clone());
                                d.hover_since = time;
                            }
                            hover = if time - d.hover_since > 0.35 { Hover::Merge(key) } else { Hover::Cell { page: pi, col: c, row: r } };
                        }
                        _ => {
                            d.cand = None;
                            hover = Hover::Cell { page: pi, col: c, row: r };
                        }
                    }
                } else {
                    d.cand = None;
                }
            }
        } else {
            d.cand = None;
        }
        if hover != d.hover {
            if matches!(hover, Hover::Merge(_)) {
                self.haptic(Haptic::Tick);
            }
            d.hover = hover;
        }
    }

    fn drop_drag(&mut self, ctx: &Context, geo: &Geo, d: Drag, _time: f64) {
        let _ = geo;
        self.touch = Touch::Idle;
        self.haptic(Haptic::Drop);
        self.dirty = true;
        let item = d.item.clone();
        let mut placed = false;
        match d.hover.clone() {
            Hover::Remove => {
                // folders and widgets are simply removed; apps stay in the drawer
                if let Item::AppWidget { wid, .. } = &item {
                    self.aw.borrow_mut().delete(*wid);
                }
                placed = true;
            }
            Hover::Info => {
                if let Item::App(p) = &item {
                    let p = p.clone();
                    crate::bridge::bg(move || platform::app_info(&p));
                }
            }
            Hover::Uninstall => {
                if let Item::App(p) = &item {
                    let p = p.clone();
                    crate::bridge::bg(move || platform::uninstall(&p));
                }
            }
            Hover::Merge(key) => {
                // the dragged icon flies into the folder
                if let Some(r) = self.rect_of_key(geo, &key) {
                    let to = self.key_icon_rect(geo, &key).unwrap_or(r);
                    let size = self.cfg.eff();
                    let from = Rect::from_center_size(d.pos - d.grab, vec2(size * 1.1, size * 1.1));
                    self.ghosts.push(Ghost { item: item.clone(), from, to: Rect::from_center_size(to.center(), to.size() * 0.35), t: anim::Spring::at(0.0), icon_space: true });
                }
                placed = self.merge_into(&key, &item)
            }
            Hover::Dock(idx) => {
                if !matches!(item, Item::Widget { .. } | Item::AppWidget { .. }) && self.cfg.dock_items.len() < self.cfg.dock_slots {
                    let at = idx.min(self.cfg.dock_items.len());
                    self.cfg.remove_app_if_app(&item);
                    self.cfg.dock_items.insert(at, item.clone());
                    placed = true;
                }
            }
            Hover::Cell { page, col, row } => {
                let (w, h) = item.span(self.cfg.cols);
                let (w, h) = (w.min(self.cfg.cols), h.min(self.cfg.rows));
                if let Some((c, r)) = self.cfg.nearest_free(page, col, row, w, h, None) {
                    self.cfg.remove_app_if_app(&item);
                    self.cfg.place(page, c, r, item.clone());
                    placed = true;
                }
            }
            Hover::None => {}
        }
        if placed && matches!(d.hover, Hover::Cell { .. } | Hover::Dock(_)) {
            // let the item fly from the finger into its cell instead of jumping
            let key = item.key();
            let dummy = Rect::from_min_size(Pos2::ZERO, geo.cell);
            let off = match d.hover {
                Hover::Dock(_) => self.dock_icon_rect(Rect::from_min_size(Pos2::ZERO, vec2(geo.dock_slot_w, geo.dock.height()))).center().to_vec2(),
                _ => match item {
                    Item::Widget { .. } | Item::AppWidget { .. } => vec2(geo.cell.x, geo.cell.y) * (item.span(self.cfg.cols).0 as f32 / 2.0).max(0.5),
                    _ => self.icon_rect(dummy).center().to_vec2(),
                },
            };
            let center = d.pos - d.grab;
            self.item_sp.insert(key.clone(), anim::Spring2::at(center - off));
            self.scale_sp.insert(key, anim::Spring { x: 1.1, v: 0.0, target: 1.0 });
        }
        let removed_on_purpose = matches!(d.hover, Hover::Remove);
        let acted = matches!(d.hover, Hover::Info | Hover::Uninstall);
        if !placed && !removed_on_purpose && !matches!(d.from, Source::Drawer) || (acted && !matches!(d.from, Source::Drawer)) {
            self.restore(&item, &d.from);
        }
        if removed_on_purpose {
            if let Item::Folder { .. } = item {
                // keep the apps reachable: they stay in the drawer
            }
        }
        self.cfg.tidy_pages();
        let n = self.page_count().max(1);
        self.page_target = self.page_target.clamp(0.0, (n - 1) as f32);
        // a drop from the drawer keeps the finger's page
        let _ = ctx;
    }

    fn restore(&mut self, item: &Item, from: &Source) {
        match from {
            Source::Page { page, col, row } => {
                let (w, h) = item.span(self.cfg.cols);
                if self.cfg.fits(*page, *col, *row, w.min(self.cfg.cols), h.min(self.cfg.rows), None) {
                    self.cfg.place(*page, *col, *row, item.clone());
                } else {
                    self.cfg.insert_auto(item.clone());
                }
            }
            Source::Dock(i) => {
                let at = (*i).min(self.cfg.dock_items.len());
                self.cfg.dock_items.insert(at, item.clone());
            }
            Source::Folder(id) => {
                if let Item::App(pkg) = item {
                    let mut done = false;
                    let mut add = |it: &mut Item| {
                        if let Item::Folder { id: fid, apps, .. } = it {
                            if fid == id {
                                apps.push(pkg.clone());
                                done = true;
                            }
                        }
                    };
                    for pg in self.cfg.pages.iter_mut() {
                        for pl in pg.items.iter_mut() {
                            add(&mut pl.item);
                        }
                    }
                    for it in self.cfg.dock_items.iter_mut() {
                        add(it);
                    }
                    if !done {
                        self.cfg.insert_auto(item.clone());
                    }
                }
            }
            Source::Drawer => {}
        }
    }

    /// Dropped on an app: both go into a new folder. Dropped on a folder: it goes in.
    fn merge_into(&mut self, key: &str, item: &Item) -> bool {
        let Item::App(dragged) = item else { return false };
        let name_for = |s: &Lumen, a: &str, b: &str| -> String {
            let (ca, cb) = (s.app(a).map(|x| x.category), s.app(b).map(|x| x.category));
            if ca.is_some() && ca == cb { ca.unwrap().name().to_string() } else { "Folder".to_string() }
        };
        let id = self.cfg.new_id();
        // find the target in pages or dock
        let target: Option<Item> = self.cfg.pages.iter().flat_map(|p| p.items.iter()).find(|p| p.item.key() == key).map(|p| p.item.clone()).or_else(|| self.cfg.dock_items.iter().find(|i| i.key() == key).cloned());
        let Some(target) = target else { return false };
        let new_item = match target {
            Item::Folder { id: fid, name, mut apps } => {
                apps.push(dragged.clone());
                Item::Folder { id: fid, name, apps }
            }
            Item::App(t) => {
                let name = name_for(self, &t, dragged);
                Item::Folder { id, name, apps: vec![t, dragged.clone()] }
            }
            _ => return false,
        };
        self.cfg.remove_app_if_app(item);
        for pg in self.cfg.pages.iter_mut() {
            for pl in pg.items.iter_mut() {
                if pl.item.key() == key {
                    pl.item = new_item.clone();
                    return true;
                }
            }
        }
        for it in self.cfg.dock_items.iter_mut() {
            if it.key() == key {
                *it = new_item.clone();
                return true;
            }
        }
        false
    }

    pub fn remove_item(&mut self, key: &str) {
        if let Some(Item::AppWidget { wid, .. }) = self.cfg.remove_key(key) {
            self.aw.borrow_mut().delete(wid);
        }
        self.cfg.tidy_pages();
        self.dirty = true;
    }
}

// ───────────────────────────── item painting (free functions to keep borrows apart) ─────────────────────────────

fn edit_chrome_y(geo: &Geo) -> f32 {
    (geo.top - 8.0).max(26.0)
}

type Find<'a> = dyn Fn(&str) -> Option<&'a platform::App> + 'a;

#[allow(clippy::too_many_arguments)]
fn paint_item<'a>(
    p: &Painter,
    item: &Item,
    cell: Rect,
    icon_rect: &dyn Fn(Rect) -> Rect,
    cfg: &crate::config::Config,
    pal: &crate::theme::Palette,
    icons: &mut crate::icons::Icons,
    find: &Find<'a>,
    badges: &std::collections::HashMap<String, u32>,
    live: &cards::Live,
    scale: f32,
    rot: f32,
    alpha: f32,
    labels_ok: bool,
    lifted: bool,
    labels: &mut widgets::Labels,
) {
    match item {
        Item::App(pkg) => {
            let Some(app) = find(pkg) else { return };
            let ir = icon_rect(cell);
            if lifted {
                widgets::shadow(p, ir, widgets::plate_radius(cfg, ir.width()), 70);
            }
            widgets::app_icon(p, ir, app, icons, cfg, pal, scale, rot, alpha);
            if let Some(c) = badges.get(pkg) {
                widgets::badge(p, Rect::from_center_size(ir.center(), ir.size() * scale), *c, cfg, pal);
            }
            if cfg.labels && labels_ok {
                labels.push(pos2(cell.center().x, ir.bottom() + 6.0 + (scale - 1.0) * ir.height() / 2.0), cfg.names.get(pkg).map(|s| s.as_str()).unwrap_or(&app.label), pal.home_text, cell.width() - 4.0, cfg.label_shadow, alpha);
            }
        }
        Item::Folder { name, apps, .. } => {
            let ir = icon_rect(cell);
            let list: Vec<&platform::App> = apps.iter().filter_map(|a| find(a)).collect();
            widgets::folder_icon(p, ir, &list, icons, cfg, pal, scale, rot);
            if cfg.labels && labels_ok {
                labels.push(pos2(cell.center().x, ir.bottom() + 6.0), name, pal.home_text, cell.width() - 4.0, cfg.label_shadow, alpha);
            }
        }
        Item::Widget { kind, .. } => {
            cards::paint(p, cell, *kind, cfg, pal, live);
        }
        Item::AppWidget { wid, .. } => {
            let r = cell.shrink(3.0);
            let uv = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
            let mut shown = false;
            if let Some(aw) = live.aw {
                aw.request(*wid, r.width() * live.ppp, r.height() * live.ppp, live.now);
                if let Some(tex) = aw.texture(*wid) {
                    p.image(tex, r, uv, Color32::WHITE.gamma_multiply(alpha));
                    shown = true;
                }
            }
            if !shown {
                p.rect_filled(r, cr(cfg.widget_radius), pal.card);
                widgets::glyph_at(p, r.center(), Align2::CENTER_CENTER, ph::SQUARES_FOUR, 28.0, pal.dim);
            }
        }
    }
}

fn paint_remove_badge(p: &Painter, item: &Item, cell: Rect, icon_rect: &dyn Fn(Rect) -> Rect, _cfg: &crate::config::Config, _pal: &crate::theme::Palette) {
    let ir = match item {
        Item::Widget { .. } | Item::AppWidget { .. } => cell.shrink(6.0),
        _ => icon_rect(cell),
    };
    let c = ir.left_top() + vec2(2.0, 2.0);
    p.circle_filled(c, 11.0, Color32::from_gray(90));
    p.rect_filled(Rect::from_center_size(c, vec2(11.0, 2.4)), cr(1.2), Color32::WHITE);
}

#[allow(dead_code)]
fn _shape_unused(_: Shape, _: Sense) {}

#[allow(clippy::too_many_arguments)]
fn paint_library<'a>(
    p: &Painter,
    geo: &Geo,
    groups: &[(String, Option<platform::Category>, Vec<String>)],
    search: Rect,
    cards_l: &[(Rect, Vec<Rect>)],
    cfg: &crate::config::Config,
    pal: &crate::theme::Palette,
    icons: &mut crate::icons::Icons,
    find: &Find<'a>,
    badges: &std::collections::HashMap<String, u32>,
) {
    let screen = geo.screen;
    p.rect_filled(screen, 0, Color32::from_black_alpha(70));
    let p = p.with_clip_rect(Rect::from_min_max(pos2(screen.left(), geo.top), screen.max));
    p.rect_filled(search, cr(22.0), Color32::from_white_alpha(46));
    widgets::glyph_at(&p, search.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, ph::MAGNIFYING_GLASS, 18.0, Color32::from_white_alpha(190));
    p.text(search.left_center() + vec2(44.0, 0.0), Align2::LEFT_CENTER, "App Library", fonts::body(cfg, 16.0, false), Color32::from_white_alpha(190));
    for ((name, _cat, pkgs), (rect, slots)) in groups.iter().zip(cards_l.iter()) {
        if rect.bottom() < geo.top || rect.top() > screen.bottom() {
            continue;
        }
        p.rect_filled(*rect, cr(24.0), pal.folder_bg);
        for s in 0..4usize {
            let Some(pkg) = pkgs.get(s) else { break };
            if s == 3 && pkgs.len() > 4 {
                // the fourth slot becomes a small group of the rest
                let slot = slots[3];
                let mini = slot.width() * 0.46;
                for (j, rest) in pkgs.iter().skip(3).take(4).enumerate() {
                    let r = Rect::from_min_size(slot.min + vec2((j % 2) as f32 * (mini + slot.width() * 0.08), (j / 2) as f32 * (mini + slot.width() * 0.08)), vec2(mini, mini));
                    if let Some(app) = find(rest) {
                        widgets::app_icon(&p, r, app, icons, cfg, pal, 1.0, 0.0, 1.0);
                    }
                }
                break;
            }
            if let Some(app) = find(pkg) {
                widgets::app_icon(&p, slots[s], app, icons, cfg, pal, 1.0, 0.0, 1.0);
                if let Some(c) = badges.get(pkg) {
                    if cfg.badges != crate::config::Badges::Off {
                        widgets::badge(&p, slots[s], *c, cfg, pal);
                    }
                }
            }
        }
        widgets::label(&p, pos2(rect.center().x, rect.bottom() + 6.0), name, cfg, Color32::WHITE, rect.width(), true, 1.0);
    }
}

/// Where a page is, how big, how visible, while it moves by `d` pages from the centre.
fn page_effect(e: PageEffect, d: f32, w: f32) -> (f32, f32, f32) {
    let a = d.abs().min(1.0);
    match e {
        PageEffect::Slide => (d * w, 1.0, 1.0),
        PageEffect::Parallax => (if d < 0.0 { d * w * 0.7 } else { d * w }, 1.0 - 0.04 * a, 1.0 - 0.65 * a),
        PageEffect::Zoom => (d * w * 0.55, 1.0 - 0.15 * a, 1.0 - a),
        PageEffect::Fade => (d * w * 0.2, 1.0, (1.0 - a * 1.1).max(0.0)),
        PageEffect::Stack => {
            if d < 0.0 {
                (d * w * 0.25, 1.0 - 0.08 * a, 1.0 - a)
            } else {
                (d * w, 1.0, 1.0)
            }
        }
    }
}

/// Each item sways a little differently, so the edit mode looks alive.
fn jiggle(t: f32, key: &str) -> f32 {
    // iOS: about two degrees, 7 or 8 swings a second, every icon a little out of step
    let phase = key.bytes().fold(0u32, |h, b| h.wrapping_mul(31).wrapping_add(b as u32)) as f32 * 0.37;
    let f = 1.0 + (phase.sin() * 0.08);
    ((t * 47.0 * f + phase).sin() * 0.034) + ((t * 31.0 + phase * 1.7).sin() * 0.006)
}

/// A transformed, faded painting layer. Layers are areas so their stacking order is stable.
fn layer(ctx: &Context, name: &str, idx: usize, screen: Rect, tf: egui::emath::TSTransform, alpha: f32, f: impl FnOnce(&Painter)) {
    let id = egui::Id::new(("lumen-layer", name, idx));
    ctx.set_transform_layer(egui::LayerId::new(egui::Order::Middle, id), tf);
    egui::Area::new(id).order(egui::Order::Middle).fixed_pos(screen.min).constrain(false).interactable(false).show(ctx, |ui| {
        ui.set_clip_rect(screen);
        let mut p = ui.painter().clone();
        p.set_opacity(alpha);
        f(&p);
    });
}

fn paint_dots(p: &Painter, geo: &Geo, cfg: &crate::config::Config, pal: &crate::theme::Palette, count: usize, page: f32) {
    use crate::config::DotsStyle;
    let y = geo.dots_y;
    if cfg.dots_style == DotsStyle::Numbers {
        let cur = (page.round() as usize + 1).min(count);
        p.text(pos2(geo.screen.center().x, y), Align2::CENTER_CENTER, format!("{cur} / {count}"), fonts::mono(12.0, false), pal.home_text.gamma_multiply(0.8));
        return;
    }
    let gap = if cfg.dots_style == DotsStyle::Bars { 20.0 } else { 14.0 };
    for i in 0..count {
        let x = geo.screen.center().x + (i as f32 - (count - 1) as f32 / 2.0) * gap;
        let near = (1.0 - (page - i as f32).abs()).clamp(0.0, 1.0); // 1 when this page is the current one, in between while moving
        let c = pos2(x, y);
        let dim = pal.home_text.gamma_multiply(0.42 + 0.58 * near);
        if cfg.app_library && i == count - 1 {
            widgets::glyph_at(p, c, Align2::CENTER_CENTER, ph::SQUARES_FOUR, 10.0 + 2.0 * near, dim);
        } else if cfg.dots_style == DotsStyle::Bars {
            let w = 8.0 + 8.0 * near;
            p.rect_filled(Rect::from_center_size(c, vec2(w, 3.0)), cr(1.5), if near > 0.5 && cfg.preset == Preset::Nothing { pal.accent } else { dim });
        } else if cfg.preset == Preset::Nothing {
            let sz = 4.0 + 3.0 * near;
            p.rect_filled(Rect::from_center_size(c, vec2(sz, sz)), cr(sz / 2.0), if near > 0.5 { pal.accent } else { dim });
        } else {
            p.circle_filled(c, 3.0 + 0.8 * near, dim);
        }
    }
}
