//! The home screen: geometry, painting, and the touch machine (tap, swipe, long press, drag and drop).

use crate::app::*;
use crate::cards;
use crate::config::{Action, DockStyle, Item, Page, Preset, Search, Shape, WidgetKind};
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
        let side = 12.0f32;
        let icon = c.icon_size;
        let bottom = screen.bottom() - ib.max(14.0);
        let dock_h = match c.dock {
            DockStyle::Bare => icon + 22.0,
            _ => icon + 30.0,
        };
        let margin = if c.preset == Preset::Ios { 10.0 } else { side };
        let left = screen.left() + il.max(margin);
        let right = screen.right() - ir.max(margin);
        let mut y = bottom;
        let mut search = None;
        if c.search == Search::Bottom {
            if matches!(c.preset, Preset::Pixel | Preset::Custom) {
                // Pixel: the search pill sits under the dock
                search = Some(Rect::from_min_max(pos2(left + 4.0, y - 56.0), pos2(right - 4.0, y)));
                y -= 56.0 + 10.0;
            }
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
        let mut top = screen.top() + it + 6.0;
        if c.search == Search::Top {
            search = Some(Rect::from_min_size(pos2(left + 6.0, top), vec2(right - left - 12.0, 52.0)));
            top += 62.0;
        }
        let grid = Rect::from_min_max(pos2(left, top), pos2(right, (y - 4.0).max(top + 100.0)));
        let cell = vec2(grid.width() / c.cols as f32, grid.height() / c.rows as f32);
        let dock_slot_w = (dock.width() / c.dock_slots.max(1) as f32).min(110.0);
        Geo { screen, top: screen.top() + it, grid, cell, dock, search, dots_y, dock_slot_w }
    }

    /// The square the icon is drawn in, for a one-cell item.
    pub fn icon_rect(&self, cell: Rect) -> Rect {
        let icon = self.cfg.icon_size;
        let label_h = if self.cfg.labels { 16.0 * self.cfg.font_scale } else { 0.0 };
        let total = icon + if self.cfg.labels { 6.0 + label_h } else { 0.0 };
        let top = cell.top() + ((cell.height() - total) / 2.0).max(2.0);
        Rect::from_min_size(pos2(cell.center().x - icon / 2.0, top), vec2(icon, icon))
    }

    pub fn dock_icon_rect(&self, slot: Rect) -> Rect {
        let icon = self.cfg.icon_size;
        Rect::from_center_size(slot.center(), vec2(icon, icon))
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
        let geo = self.geo(ctx, screen);
        let dt = ctx.input(|i| i.stable_dt).min(0.05);
        let k = 1.0 - (-dt * 16.0).exp();

        // animation of pages, drawer and popups
        let count = self.page_count().max(1);
        if !matches!(self.touch, Touch::Pager) {
            let t = self.page_target.clamp(0.0, (count - 1) as f32);
            self.page += (t - self.page) * k;
            if (t - self.page).abs() < 0.002 {
                self.page = t;
            } else {
                ctx.request_repaint();
            }
        }
        if !matches!(self.touch, Touch::DrawerPull) {
            let t = if self.drawer_open { 1.0 } else { 0.0 };
            self.drawer += (t - self.drawer) * k;
            if (t - self.drawer).abs() < 0.003 {
                self.drawer = t;
            } else {
                ctx.request_repaint();
            }
        }
        let want_folder = if self.open_folder.is_some() || self.open_category.is_some() { 1.0 } else { 0.0 };
        self.folder_t += (want_folder - self.folder_t) * (1.0 - (-dt * 22.0).exp());
        if (want_folder - self.folder_t).abs() < 0.01 {
            self.folder_t = want_folder;
        } else {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().frame(Frame::NONE).show(root, |ui| {
            let p = ui.painter().clone();
            self.paint_home(ctx, &p, &geo, dt);
        });

        self.touch(ctx, &geo);

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
        self.toast_ui(ctx, screen);

        if self.cfg.page_dots || self.has_live_widget() {
            let s = self.clock.sec as u64;
            let wait = if self.has_analog() { 1 } else { 61u64.saturating_sub(s).max(1) };
            ctx.request_repaint_after(std::time::Duration::from_secs(wait));
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

    fn paint_home(&mut self, ctx: &Context, p: &Painter, geo: &Geo, dt: f32) {
        let pal = self.pal;
        p.rect_filled(geo.screen, 0, pal.scrim);
        if self.drawer > 0.0 {
            p.rect_filled(geo.screen, 0, Color32::from_black_alpha((self.drawer * 120.0) as u8));
        }
        let k = 1.0 - (-dt * 18.0).exp();
        let t = ctx.input(|i| i.time) as f32;
        let dragging_key = if let Touch::Drag(d) = &self.touch { Some(d.item.key()) } else { None };
        let lifted_key = if let Touch::Lifted { item, .. } = &self.touch { Some(item.key()) } else { None };
        let live = cards::Live { clock: &self.clock, battery: self.battery };
        let edit = self.edit;
        let mut moving = false;
        let w = geo.screen.width();

        // pages
        let Lumen { cfg, icons, anim, apps, by_pkg, badges, .. } = self;
        let icons = icons.as_mut().unwrap();
        let find = |pkg: &str| by_pkg.get(pkg).map(|&i| &apps[i]);
        let page = self.page;
        let icon_rect = |cell: Rect| {
            let icon = cfg.icon_size;
            let label_h = if cfg.labels { 16.0 * cfg.font_scale } else { 0.0 };
            let total = icon + if cfg.labels { 6.0 + label_h } else { 0.0 };
            let top = cell.top() + ((cell.height() - total) / 2.0).max(2.0);
            Rect::from_min_size(pos2(cell.center().x - icon / 2.0, top), vec2(icon, icon))
        };

        for (pi, pg) in cfg.pages.iter().enumerate() {
            let off = (pi as f32 - page) * w;
            if off.abs() >= w {
                continue;
            }
            for pl in &pg.items {
                let key = pl.item.key();
                let (sw, sh) = (pl.item.span(cfg.cols).0.min(cfg.cols), pl.item.span(cfg.cols).1.min(cfg.rows));
                let target = geo.cell_rect(pl.col, pl.row, sw, sh);
                let cur = anim.entry(key.clone()).or_insert(target.min);
                *cur += (target.min - *cur) * k;
                if (target.min - *cur).length() > 0.4 {
                    moving = true;
                } else {
                    *cur = target.min;
                }
                let cell = Rect::from_min_size(*cur + vec2(off, 0.0), target.size());
                let lifted = lifted_key.as_deref() == Some(key.as_str());
                let jig = if edit { (t * 14.0 + pl.col as f32 * 1.3 + pl.row as f32 * 0.7).sin() * 0.025 } else { 0.0 };
                paint_item(p, &pl.item, cell, &icon_rect, cfg, &pal, icons, &find, badges, &live, if lifted { 1.12 } else { 1.0 }, jig, 1.0, true, lifted);
                if edit && dragging_key.as_deref() != Some(key.as_str()) {
                    paint_remove_badge(p, &pl.item, cell, &icon_rect, cfg, &pal);
                }
            }
        }

        // dock (it slides away on the App Library page, like iOS)
        let dock_vis = if cfg.app_library { 1.0 - (page - (cfg.pages.len() as f32 - 1.0)).clamp(0.0, 1.0) } else { 1.0 };
        let dp = &{
            let mut q = p.clone();
            q.set_opacity(dock_vis);
            q
        };
        if dock_vis < 0.02 {
            // nothing to draw
        } else if cfg.dock != DockStyle::Bare {
            let fill_rect = geo.dock;
            if cfg.dock == DockStyle::Glass {
                widgets::glass(dp, fill_rect, cfg.dock_radius, &pal);
            } else {
                dp.rect_filled(fill_rect, cr(cfg.dock_radius), pal.sheet);
            }
        }
        let n = cfg.dock_items.len();
        for (i, it) in cfg.dock_items.iter().enumerate() {
            let slot = geo.dock_slot(i, n);
            let key = it.key();
            let target = slot.min;
            let cur = anim.entry(key.clone()).or_insert(target);
            *cur += (target - *cur) * k;
            if (target - *cur).length() > 0.4 {
                moving = true;
            } else {
                *cur = target;
            }
            let cell = Rect::from_min_size(*cur, slot.size());
            let lifted = lifted_key.as_deref() == Some(key.as_str());
            let jig = if edit { (t * 14.0 + i as f32 * 1.1).sin() * 0.025 } else { 0.0 };
            let dock_icon = |c: Rect| Rect::from_center_size(c.center(), vec2(cfg.icon_size, cfg.icon_size));
            paint_item(dp, it, cell, &dock_icon, cfg, &pal, icons, &find, badges, &live, if lifted { 1.12 } else { 1.0 }, jig, 1.0, false, lifted);
            if edit {
                paint_remove_badge(dp, it, cell, &dock_icon, cfg, &pal);
            }
        }

        // search pill
        if let Some(s) = geo.search {
            match cfg.search {
                Search::Off => {}
                _ => widgets::search_pill(p, s, if cfg.preset == Preset::Pixel { "Search apps, web and more" } else { "Search" }, cfg, &pal, cfg.preset == Preset::Pixel),
            }
        }

        // page dots
        let count = cfg.pages.len() + usize::from(cfg.app_library);
        if cfg.page_dots && count > 1 {
            let gap = 14.0;
            for i in 0..count {
                let x = geo.screen.center().x + (i as f32 - (count - 1) as f32 / 2.0) * gap;
                let on = (page - i as f32).abs() < 0.5;
                let c = pos2(x, geo.dots_y);
                if cfg.app_library && i == count - 1 {
                    // the App Library page gets a little grid mark, like iOS
                    widgets::glyph_at(p, c, Align2::CENTER_CENTER, ph::SQUARES_FOUR, 10.0, if on { pal.home_text } else { pal.home_text.gamma_multiply(0.45) });
                } else if cfg.preset == Preset::Nothing {
                    p.rect_filled(Rect::from_center_size(c, vec2(if on { 7.0 } else { 4.0 }, if on { 7.0 } else { 4.0 })), cr(4.0), if on { pal.accent } else { pal.home_text.gamma_multiply(0.4) });
                } else {
                    p.circle_filled(c, if on { 3.6 } else { 3.0 }, if on { pal.home_text } else { pal.home_text.gamma_multiply(0.45) });
                }
            }
        }

        // iOS App Library page
        if cfg.app_library {
            let off = (cfg.pages.len() as f32 - page) * w;
            if off.abs() < w {
                self.paint_library(ctx, p, geo, off);
            }
        }

        if self.edit {
            self.paint_edit_chrome(p, geo);
        }

        self.paint_folder_popup(ctx, p, geo);
        self.paint_drag(ctx, p, geo);

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
        let search = Rect::from_min_size(pos2(margin + off, top - self.lib_scroll.min(0.0)), vec2(w - 2.0 * margin, 44.0));
        let mut out = vec![];
        let groups = self.lib_groups();
        let start = top + 44.0 + 20.0;
        let pitch = card + 26.0 + gap;
        for i in 0..groups.len() {
            let (col, row) = (i % 2, i / 2);
            let r = Rect::from_min_size(pos2(margin + off + col as f32 * (card + gap), start + row as f32 * pitch - self.lib_scroll), vec2(card, card));
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

    fn paint_library(&mut self, _ctx: &Context, p: &Painter, geo: &Geo, off: f32) {
        let groups = self.lib_groups();
        let (search, cards_l, _) = self.lib_layout(geo, off);
        let pal = self.pal;
        p.rect_filled(Rect::from_min_max(pos2(geo.screen.left() + off, geo.screen.top()), pos2(geo.screen.right() + off, geo.screen.bottom())), 0, Color32::from_black_alpha(70));
        let p = p.with_clip_rect(Rect::from_min_max(pos2(geo.screen.left(), geo.top), geo.screen.max));
        p.rect_filled(search, cr(22.0), Color32::from_white_alpha(46));
        widgets::glyph_at(&p, search.left_center() + vec2(16.0, 0.0), Align2::LEFT_CENTER, ph::MAGNIFYING_GLASS, 18.0, Color32::from_white_alpha(190));
        p.text(search.left_center() + vec2(44.0, 0.0), Align2::LEFT_CENTER, "App Library", fonts::body(&self.cfg, 16.0, false), Color32::from_white_alpha(190));
        let Lumen { cfg, icons, apps, by_pkg, badges, .. } = self;
        let icons = icons.as_mut().unwrap();
        for ((name, _cat, pkgs), (rect, slots)) in groups.iter().zip(cards_l.iter()) {
            if rect.bottom() < geo.top || rect.top() > geo.screen.bottom() {
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
                        if let Some(&ai) = by_pkg.get(rest) {
                            widgets::app_icon(&p, r, &apps[ai], icons, cfg, &pal, 1.0, 0.0, 1.0);
                        }
                    }
                    break;
                }
                if let Some(&ai) = by_pkg.get(pkg) {
                    widgets::app_icon(&p, slots[s], &apps[ai], icons, cfg, &pal, 1.0, 0.0, 1.0);
                    if let Some(c) = badges.get(pkg) {
                        if cfg.badges != crate::config::Badges::Off {
                            widgets::badge(&p, slots[s], *c, cfg, &pal);
                        }
                    }
                }
            }
            widgets::label(&p, pos2(rect.center().x, rect.bottom() + 6.0), name, cfg, Color32::WHITE, rect.width(), true, 1.0);
        }
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
        let cols = 3usize;
        let rows = n.div_ceil(cols).clamp(1, 5);
        let icon = self.cfg.icon_size;
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

    fn paint_folder_popup(&mut self, _ctx: &Context, p: &Painter, geo: &Geo) {
        let Some((name, pkgs, _)) = self.popup_apps() else { return };
        let t = self.folder_t;
        if t <= 0.0 {
            return;
        }
        let pal = self.pal;
        let (panel, title, cells) = self.popup_geo(geo, pkgs.len());
        let sc = 0.86 + 0.14 * t;
        let c0 = panel.center();
        let tf = |r: Rect| Rect::from_center_size(c0 + (r.center() - c0) * sc, r.size() * sc);
        p.rect_filled(geo.screen, 0, Color32::from_black_alpha((t * 130.0) as u8));
        let ios = self.cfg.preset == Preset::Ios;
        let panel_fill = if ios { Color32::from_rgba_unmultiplied(70, 70, 80, 200) } else { pal.sheet };
        p.rect_filled(tf(panel), cr(if ios { 34.0 } else { 36.0 }), panel_fill.gamma_multiply(t));
        let tcol = if ios { Color32::WHITE } else { pal.text };
        p.text(tf(title).center_top() + vec2(0.0, 12.0), Align2::CENTER_TOP, name, fonts::body(&self.cfg, 20.0 * sc, true), tcol.gamma_multiply(t));
        let Lumen { cfg, icons, apps, by_pkg, badges, .. } = self;
        let icons = icons.as_mut().unwrap();
        for (pkg, cell) in pkgs.iter().zip(cells.iter()) {
            let Some(&ai) = by_pkg.get(pkg) else { continue };
            let cell = tf(*cell);
            let ir = Rect::from_center_size(pos2(cell.center().x, cell.top() + 12.0 + cfg.icon_size * sc / 2.0), vec2(cfg.icon_size * sc, cfg.icon_size * sc));
            widgets::app_icon(p, ir, &apps[ai], icons, cfg, &pal, 1.0, 0.0, t);
            if let Some(c) = badges.get(pkg) {
                widgets::badge(p, ir, *c, cfg, &pal);
            }
            if cfg.labels {
                widgets::label(p, pos2(cell.center().x, ir.bottom() + 5.0), cfg.names.get(pkg).map(|s| s.as_str()).unwrap_or(&apps[ai].label), cfg, tcol, cell.width() - 6.0, false, t);
            }
        }
    }

    fn paint_drag(&mut self, ctx: &Context, p: &Painter, geo: &Geo) {
        let Touch::Drag(d) = self.touch.clone() else { return };
        let pal = self.pal;
        // drop hint
        match &d.hover {
            Hover::Cell { page, col, row } if *page == self.page.round() as usize => {
                let (w, h) = d.item.span(self.cfg.cols);
                if let Some((c, r)) = self.cfg.nearest_free(*page, *col, *row, w.min(self.cfg.cols), h.min(self.cfg.rows), None) {
                    let cell = geo.cell_rect(c, r, w, h).shrink(4.0);
                    p.rect_stroke(self.icon_rect_for(&d.item, cell), cr(widgets::plate_radius(&self.cfg, self.cfg.icon_size)), Stroke::new(1.5, pal.home_text.gamma_multiply(0.45)), StrokeKind::Inside);
                }
            }
            Hover::Merge(key) => {
                if let Some(r) = self.rect_of_key(geo, key) {
                    let ir = self.icon_rect(r);
                    p.rect_filled(ir.expand(8.0), cr(widgets::plate_radius(&self.cfg, ir.width() + 16.0)), Color32::from_white_alpha(70));
                }
            }
            _ => {}
        }
        // zones
        for z in self.zones(geo, &d) {
            let on = d.hover == z.hover;
            let bg = if on { pal.accent } else { Color32::from_black_alpha(120) };
            p.rect_filled(z.rect, cr(26.0), bg);
            let fg = if on { pal.on_accent } else { Color32::WHITE };
            let font = fonts::body(&self.cfg, 14.5, true);
            let tw = widgets::text_width(p, z.label, &font);
            let start = z.rect.center().x - (tw + 30.0) / 2.0;
            widgets::glyph_at(p, pos2(start, z.rect.center().y), Align2::LEFT_CENTER, z.glyph, 20.0, fg);
            p.text(pos2(start + 30.0, z.rect.center().y), Align2::LEFT_CENTER, z.label, font, fg);
        }
        // the item on the finger
        let center = d.pos - d.grab;
        let (sw, sh) = d.item.span(self.cfg.cols);
        let size = match &d.item {
            Item::Widget { .. } => vec2(sw.min(self.cfg.cols) as f32 * geo.cell.x, sh.min(self.cfg.rows) as f32 * geo.cell.y),
            _ => vec2(self.cfg.icon_size, self.cfg.icon_size),
        };
        let rect = Rect::from_center_size(center, size * 1.1);
        widgets::shadow(p, rect, widgets::plate_radius(&self.cfg, rect.width()), 90);
        let live = cards::Live { clock: &self.clock, battery: self.battery };
        let Lumen { cfg, icons, apps, by_pkg, badges, .. } = self;
        let icons = icons.as_mut().unwrap();
        let find = |pkg: &str| by_pkg.get(pkg).map(|&i| &apps[i]);
        let whole = Rect::from_center_size(center, size * 1.1);
        let id = |c: Rect| Rect::from_center_size(c.center(), vec2(cfg.icon_size * 1.1, cfg.icon_size * 1.1));
        paint_item(p, &d.item, whole, &id, cfg, &pal, icons, &find, badges, &live, 1.0, 0.0, 1.0, false, false);
        ctx.request_repaint();
    }

    fn icon_rect_for(&self, item: &Item, cell: Rect) -> Rect {
        match item {
            Item::Widget { .. } => cell,
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

    // ───────────────────────────── hit testing ─────────────────────────────

    fn hit_at(&self, geo: &Geo, pos: Pos2) -> Hit {
        let w = geo.screen.width();
        if self.edit {
            if (pos - pos2(geo.screen.left() + 34.0, edit_chrome_y(geo))).length() < 26.0 {
                return Hit::EditAdd;
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
        match self.touch.clone() {
            Touch::Idle => {
                if pressed && !self.overlay_blocks_home() {
                    if let Some(pos) = pos {
                        let hit = self.hit_at(geo, pos);
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
                    } else if self.cfg.app_library && self.page.round() as usize == self.cfg.pages.len() && (self.lib_scroll > 0.0 || d.y < 0.0) {
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
                    self.page_target = (self.page - vel.x / w * 0.3).round().clamp(0.0, max);
                    self.touch = Touch::Idle;
                }
            }
            Touch::DrawerPull => {
                self.drawer = (self.drawer - delta.y / (h * 0.55)).clamp(0.0, 1.0);
                if !down {
                    self.drawer_open = self.drawer > 0.3 || vel.y < -500.0;
                    if self.drawer_open && self.drawer < 0.3 && vel.y > 0.0 {
                        self.drawer_open = false;
                    }
                    self.touch = Touch::Idle;
                }
            }
            Touch::LibraryScroll => {
                let (_, _, content) = self.lib_layout(geo, 0.0);
                let max = (content - (geo.screen.height() - geo.top) + 120.0).max(0.0);
                self.lib_scroll = (self.lib_scroll - delta.y).clamp(0.0, max);
                if !down {
                    self.touch = Touch::Idle;
                }
            }
            Touch::Spent => {
                if !down {
                    self.touch = Touch::Idle;
                }
            }
            Touch::Drag(mut d) => {
                if let Some(pos) = pos {
                    d.pos = pos;
                }
                if !down {
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
                Hit::Item(key) => {
                    // the remove badge sits on the top-left corner of the icon
                    if let Some(r) = self.rect_of_key(geo, &key) {
                        let ir = self.icon_rect_for(&self.find_item(&key).map(|x| x.0).unwrap_or(Item::App(String::new())), r);
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
                        Item::App(pkg) => self.launch(ctx, &pkg),
                        Item::Folder { id, .. } => {
                            self.open_folder = Some(id);
                            self.haptic(Haptic::Tick);
                        }
                        Item::Widget { kind, .. } => {
                            if kind == WidgetKind::SearchBar {
                                self.open_search();
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
                let rect = self.rect_of_key(geo, key).unwrap_or(Rect::from_center_size(start, vec2(60.0, 60.0)));
                let anchor = self.icon_rect_for(&item, rect);
                let target = match &item {
                    Item::App(p) => Target::App { pkg: p.clone(), on_home: true },
                    Item::Folder { id, .. } => Target::Folder(*id),
                    Item::Widget { id, .. } => Target::Widget(*id),
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
                let ir = Rect::from_center_size(pos2(rect.center().x, rect.top() + 12.0 + self.cfg.icon_size / 2.0), vec2(self.cfg.icon_size, self.cfg.icon_size));
                let from = if self.open_folder.is_some() { Source::Folder(*id) } else { Source::Drawer };
                self.menu = Some(Menu { target: Target::App { pkg: pkg.clone(), on_home: false }, anchor: ir });
                self.touch = Touch::Lifted { start, item: Item::App(pkg.clone()), from, rect: ir };
            }
            Hit::LibraryApp(pkg) => {
                let r = Rect::from_center_size(start, vec2(self.cfg.icon_size, self.cfg.icon_size));
                self.menu = Some(Menu { target: Target::App { pkg: pkg.clone(), on_home: false }, anchor: r });
                self.touch = Touch::Lifted { start, item: Item::App(pkg.clone()), from: Source::Drawer, rect: r };
            }
            Hit::None | Hit::Search | Hit::LibraryGroup(_) => {
                if self.open_folder.is_some() || self.open_category.is_some() {
                    self.touch = Touch::Spent;
                    return;
                }
                if self.cfg.preset == Preset::Ios {
                    self.edit = true;
                } else {
                    self.menu = Some(Menu { target: Target::Home, anchor: Rect::from_center_size(start, vec2(10.0, 10.0)) });
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
                let r = self.rect_of_key(geo, key).unwrap_or(Rect::from_center_size(start, vec2(60.0, 60.0)));
                let r = self.icon_rect_for(&item, r);
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
            let is_widget = matches!(d.item, Item::Widget { .. });
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
                placed = true;
            }
            Hover::Info => {
                if let Item::App(p) = &item {
                    platform::app_info(p);
                }
            }
            Hover::Uninstall => {
                if let Item::App(p) = &item {
                    platform::uninstall(p);
                }
            }
            Hover::Merge(key) => placed = self.merge_into(&key, &item),
            Hover::Dock(idx) => {
                if !matches!(item, Item::Widget { .. }) && self.cfg.dock_items.len() < self.cfg.dock_slots {
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
        self.cfg.remove_key(key);
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
                widgets::label(p, pos2(cell.center().x, ir.bottom() + 6.0 + (scale - 1.0) * ir.height() / 2.0), cfg.names.get(pkg).map(|s| s.as_str()).unwrap_or(&app.label), cfg, pal.home_text, cell.width() - 4.0, true, alpha);
            }
        }
        Item::Folder { name, apps, .. } => {
            let ir = icon_rect(cell);
            let list: Vec<&platform::App> = apps.iter().filter_map(|a| find(a)).collect();
            widgets::folder_icon(p, ir, &list, icons, cfg, pal, scale, rot);
            if cfg.labels && labels_ok {
                widgets::label(p, pos2(cell.center().x, ir.bottom() + 6.0), name, cfg, pal.home_text, cell.width() - 4.0, true, alpha);
            }
        }
        Item::Widget { kind, .. } => {
            cards::paint(p, cell, *kind, cfg, pal, live);
        }
    }
}

fn paint_remove_badge(p: &Painter, item: &Item, cell: Rect, icon_rect: &dyn Fn(Rect) -> Rect, _cfg: &crate::config::Config, _pal: &crate::theme::Palette) {
    let ir = match item {
        Item::Widget { .. } => cell.shrink(6.0),
        _ => icon_rect(cell),
    };
    let c = ir.left_top() + vec2(2.0, 2.0);
    p.circle_filled(c, 11.0, Color32::from_gray(90));
    p.rect_filled(Rect::from_center_size(c, vec2(11.0, 2.4)), cr(1.2), Color32::WHITE);
}

#[allow(dead_code)]
fn _shape_unused(_: Shape, _: Sense) {}
