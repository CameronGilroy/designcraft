//! The right dock: an expanded panel group (Properties · Pages · Layers) and a collapsed icon
//! column whose panels open as flyouts beside the dock. A workspace icon group opens as one
//! flyout, with a tab for each panel in the group. The flyout stays attached to that group's
//! icon; dragging its header pulls it off into a floating window.

use egui::{Color32, Sense, Stroke, vec2};

use crate::theme::{Tokens, semibold};
use crate::{DesignApp, icons, panels};

/// (id, label, icon) of the tabs in the expanded group.
pub const DOCK_TABS: &[(&str, &str, &str)] =
    &[("properties", "Properties", "panel-properties"), ("pages", "Pages", "panel-pages"), ("layers", "Layers", "panel-layers")];

/// Panels in the collapsed icon column.
pub const ICON_PANELS: &[(&str, &str, &str)] = &[
    ("history", "History", "panel-history"),
    ("stroke", "Stroke", "panel-stroke"),
    ("swatches", "Swatches", "panel-swatches"),
    ("color", "Color", "panel-color"),
    ("gradient", "Gradient", "tool-gradient"),
    ("effects", "Effects", "panel-effects"),
    ("paragraphStyles", "Paragraph Styles", "panel-pstyles"),
    ("characterStyles", "Character Styles", "panel-cstyles"),
    ("character", "Character", "panel-character"),
    ("typography", "Typography", "panel-typography"),
    ("glyphs", "Glyphs", "panel-character"),
    ("paragraph", "Paragraph", "panel-paragraph"),
    ("story", "Story", "panel-text-frame"),
    ("textWrap", "Text Wrap", "panel-wrap"),
    ("table", "Table", "panel-table"),
    ("align", "Align", "panel-align"),
    ("pathfinder", "Pathfinder", "tool-scissors"),
    ("flexLayout", "Flex Layout", "panel-flex"),
    ("links", "Links", "panel-links"),
    ("info", "Info", "panel-info"),
    ("preflight", "Preflight", "panel-preflight"),
    ("attributes", "Attributes", "panel-attributes"),
    ("conditions", "Conditional Text", "panel-conditions"),
    ("notes", "Notes", "panel-notes"),
    ("library", "Library", "panel-library"),
    ("hyperlinks", "Hyperlinks", "panel-hyperlinks"),
    ("bookmarks", "Bookmarks", "panel-bookmarks"),
    ("articles", "Articles", "panel-articles"),
    ("tags", "Tags", "panel-tags"),
    ("book", "Book", "panel-library"),
    ("states", "Object States", "panel-articles"),
    ("buttons", "Buttons and Forms", "panel-articles"),
    ("liquid", "Liquid Layout", "panel-pages"),
    ("media", "Media", "panel-articles"),
    ("transitions", "Page Transitions", "panel-pages"),
    ("trackChanges", "Track Changes", "panel-articles"),
    ("scripts", "Scripts", "panel-library"),
    ("dataMerge", "Data Merge", "panel-datamerge"),
];

/// Panels that only float (they open from a menu, not from the icon column).
pub const FLOATING_PANELS: &[(&str, &str)] = &[("tabs", "Tabs")];

/// The English name of panel `id`.
pub fn panel_label(id: &str) -> &'static str {
    DOCK_TABS.iter().chain(ICON_PANELS).map(|p| (p.0, p.1)).chain(FLOATING_PANELS.iter().copied()).find(|p| p.0 == id).map_or("Panel", |p| p.1)
}

pub fn show(app: &mut DesignApp, ui: &mut egui::Ui) {
    if crate::panel_docking::show(app, ui) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    // Expanded panel stack (outermost, 253 pt) — declared first so it sits at the far right.
    if app.ui.dock_expanded {
        egui::Panel::right("dock").default_size(253.0).size_range(220.0..=420.0).resizable(true).frame(egui::Frame::NONE.fill(t.panel)).show(
            ui,
            |ui| {
                if dock_header(ui, &t, "»").clicked() {
                    let _ = app.run("window.collapseToIcons", serde_json::json!({}));
                }
                // Tab strip (27 pt).
                let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width(), 27.0), Sense::hover());
                ui.painter().rect_filled(strip, 0.0, t.tab_strip);
                let mut x = strip.min.x;
                for (id, label, _) in DOCK_TABS {
                    let active = app.ui.dock_tab == *id;
                    let g = crate::rtl::plain(
                        ui.ctx(),
                        crate::i18n::tr(&app.ui.language, label),
                        semibold(11.5),
                        if active { t.text_strong } else { t.text_dim },
                    );
                    let w = g.size().x + 26.0;
                    let r = egui::Rect::from_min_size(egui::pos2(x, strip.min.y), vec2(w, 27.0));
                    if active {
                        ui.painter().rect_filled(r, 0.0, t.panel);
                    }
                    ui.painter().galley(egui::pos2(r.min.x + 13.0, r.center().y - g.size().y / 2.0), g, Color32::WHITE);
                    ui.painter().line_segment([r.right_top(), r.right_bottom()], Stroke::new(1.0, t.border));
                    let response = craft_ui::tabs::Tab::new(ui.id().with(("docktab", *id)), label, active).sense(Sense::click_and_drag()).show_at(
                        ui,
                        r,
                        |_, _| {},
                    );
                    if response.clicked() {
                        app.ui.dock_tab = id.to_string();
                    }
                    crate::panel_docking::legacy_tab(app, ui, id, &response);
                    x += w;
                }
                let menu_r = egui::Rect::from_min_size(egui::pos2(strip.max.x - 24.0, strip.min.y + 4.0), vec2(18.0, 18.0));
                let menu = ui.interact(menu_r, ui.id().with("dock-menu"), Sense::click());
                if menu.hovered() {
                    ui.painter().rect_filled(menu_r, 3.0, t.hover);
                }
                icons::paint(ui.painter(), menu_r, "menu", t.icon);
                let tab = app.ui.dock_tab.clone();
                egui::Popup::menu(&menu)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .show(|ui| crate::panel_docking::three_line_menu(app, ui, &tab));
                egui::Frame::NONE.inner_margin(egui::Margin { left: 10, right: 10, top: 4, bottom: 6 }).show(ui, |ui| {
                    // The Pages panel scrolls its parts itself and keeps its footer at the bottom.
                    if app.ui.dock_tab == "pages" {
                        panels::pages::show(app, ui);
                        return;
                    }
                    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| match app.ui.dock_tab.as_str() {
                        "layers" => panels::layers::show(app, ui),
                        _ => panels::properties::show(app, ui),
                    });
                });
            },
        );
    }
    // Collapsed icon strip (37 pt) to the left of the panel stack; a workspace's iconic groups
    // (Essentials Classic) show their names.
    let groups = app.ui.icon_groups.clone();
    let icon_rect = egui::Panel::right("icon_column")
        .exact_size(icon_column_width(app))
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel))
        .show(ui, |ui| {
            if dock_header(ui, &t, "«").clicked() {
                app.ui.dock_expanded = true;
            }
            ui.add_space(4.0);
            match &groups {
                Some(groups) => {
                    for (i, group) in groups.iter().enumerate() {
                        if i > 0 {
                            let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 7.0), Sense::hover());
                            ui.painter().line_segment([r.left_center(), r.right_center()], Stroke::new(1.0, t.border));
                        }
                        for (id, label, icon) in group.iter().filter_map(|id| panel_entry(id)) {
                            let on = app.ui.open_panel.as_deref() == Some(id) || app.ui.floating.iter().any(|(p, _)| p == id);
                            let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 26.0), Sense::click_and_drag());
                            if on {
                                ui.painter().rect_filled(r.shrink2(vec2(3.0, 1.0)), 3.0, t.tool_active);
                            } else if response.hovered() {
                                ui.painter().rect_filled(r.shrink2(vec2(3.0, 1.0)), 3.0, t.hover);
                            }
                            icons::paint(ui.painter(), egui::Rect::from_min_size(r.min + vec2(8.0, 4.0), vec2(18.0, 18.0)), icon, t.icon);
                            crate::rtl::paint(
                                ui.painter(),
                                r.left_center() + vec2(32.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                crate::i18n::tr(&app.ui.language, label),
                                semibold(11.5),
                                t.text,
                            );
                            remember_icon(ui, id, r);
                            icon_clicked(app, ui, id, &response);
                        }
                    }
                }
                None => {
                    ui.vertical_centered(|ui| {
                        for (id, label, icon) in ICON_PANELS {
                            let on = app.ui.open_panel.as_deref() == Some(*id) || app.ui.floating.iter().any(|(p, _)| p == id);
                            let response =
                                icons::button(ui, icon, 28.0, on, crate::i18n::tr(&app.ui.language, label)).interact(Sense::click_and_drag());
                            remember_icon(ui, id, response.rect);
                            icon_clicked(app, ui, id, &response);
                            ui.add_space(1.0);
                        }
                    });
                }
            }
        })
        .response
        .rect;
    // The flyout sits against whichever column is furthest left: the named icons, or History beside them.
    let bar = if side_column_width(app) > 0.0 {
        let history = history_column(app, ui, &t);
        if history.min.x < icon_rect.min.x { history } else { icon_rect }
    } else {
        icon_rect
    };
    remember_right_bar(ui.ctx(), bar);
}

/// Width of the icon column: icons only, or icons with names for a workspace's groups.
fn icon_column_width(app: &DesignApp) -> f32 {
    if app.ui.icon_groups.is_some() { 150.0 } else { 37.0 }
}

/// (id, label, icon) of any dock panel.
fn panel_entry(id: &str) -> Option<(&'static str, &'static str, &'static str)> {
    DOCK_TABS.iter().chain(ICON_PANELS).find(|p| p.0 == id).copied()
}

/// Panels in `groups` that share a column with `id`, in that column's order.
pub(crate) fn icon_group_ids(groups: &[Vec<String>], id: &str) -> Vec<&'static str> {
    groups
        .iter()
        .find(|group| group.iter().any(|panel| panel == id))
        .map(|group| group.iter().filter_map(|panel| panel_entry(panel).map(|entry| entry.0)).collect())
        .unwrap_or_default()
}

/// Width of the history icon column beside a workspace's named groups. Essentials has no groups
/// and lists History in the main icon column instead.
pub(crate) fn side_column_width(app: &DesignApp) -> f32 {
    if app.ui.icon_groups.as_ref().is_some_and(|groups| !groups.iter().flatten().any(|panel| panel == "history")) { 37.0 } else { 0.0 }
}

/// Raise a floating panel, or open and close its flyout.
fn toggle_open(app: &mut DesignApp, ui: &egui::Ui, id: &str) {
    if app.ui.floating.iter().any(|(panel, _)| panel == id) {
        ui.ctx().move_to_top(egui::LayerId::new(egui::Order::Middle, egui::Id::new(("floating_panel", id))));
    } else {
        let open = app.ui.open_panel.as_deref() == Some(id);
        app.ui.open_panel = if open { None } else { Some(id.to_string()) };
    }
}

fn history_column(app: &mut DesignApp, ui: &mut egui::Ui, t: &Tokens) -> egui::Rect {
    egui::Panel::right("history_column")
        .exact_size(37.0)
        .resizable(false)
        .frame(egui::Frame::NONE.fill(t.panel))
        .show(ui, |ui| {
            if dock_header(ui, t, "«").clicked() {
                toggle_open(app, ui, "history");
            }
            ui.add_space(4.0);
            ui.vertical_centered(|ui| {
                let on = app.ui.open_panel.as_deref() == Some("history") || app.ui.floating.iter().any(|(panel, _)| panel == "history");
                let response =
                    icons::button(ui, "panel-history", 28.0, on, crate::i18n::tr(&app.ui.language, "History")).interact(Sense::click_and_drag());
                remember_icon(ui, "history", response.rect);
                icon_clicked(app, ui, "history", &response);
            });
        })
        .response
        .rect
}

/// Where an icon was drawn, so its flyout can sit beside that row.
pub(crate) fn remember_icon(ui: &egui::Ui, id: &str, rect: egui::Rect) {
    ui.ctx().data_mut(|data| data.insert_temp(egui::Id::new(("icon-row", id)), rect));
}

/// The right icon bar for this frame, so a flyout and a drop can meet its left edge.
pub(crate) fn remember_right_bar(ctx: &egui::Context, rect: egui::Rect) {
    if rect.width() > 1.0 && rect.height() > 1.0 && rect.min.x.is_finite() && rect.min.y.is_finite() {
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("right-icon-bar"), rect));
    }
}

/// Icon bar drawn last frame (during a drag) or earlier this frame (when the flyout opens).
pub(crate) fn right_bar(ctx: &egui::Context) -> Option<egui::Rect> {
    ctx.data(|data| data.get_temp(egui::Id::new("right-icon-bar")))
}

/// The icon bar, plus a gutter on its left. Releasing a floating panel here docks it.
pub(crate) fn beside_bar_zone(bar: egui::Rect) -> egui::Rect {
    egui::Rect::from_min_max(egui::pos2(bar.min.x - 48.0, bar.min.y), bar.max)
}

/// Top of the flyout: the first icon in the group, or this panel's own icon.
fn flyout_y(ctx: &egui::Context, anchor: &str) -> f32 {
    ctx.data(|data| data.get_temp::<egui::Rect>(egui::Id::new(("icon-row", anchor))).map(|rect| rect.min.y))
        .or_else(|| right_bar(ctx).map(|rect| rect.min.y))
        .or_else(|| ctx.memory(|memory| memory.area_rect(egui::Id::new("icon_column")).map(|rect| rect.min.y)))
        .unwrap_or(120.0)
}

/// A click on a panel's icon: raise it when it floats, else open or close its flyout.
fn icon_clicked(app: &mut DesignApp, ui: &mut egui::Ui, id: &str, response: &egui::Response) {
    if response.clicked() {
        toggle_open(app, ui, id);
    }
    crate::panel_docking::legacy_tab(app, ui, id, response);
}

/// 11 pt dock header strip with a collapse chevron; clicking it reports a click on the response.
fn dock_header(ui: &mut egui::Ui, t: &Tokens, chevron: &str) -> egui::Response {
    let (r, response) = ui.allocate_exact_size(vec2(ui.available_width(), 11.0), Sense::click());
    ui.painter().rect_filled(r, 0.0, t.tab_strip);
    ui.painter().text(egui::pos2(r.max.x - 8.0, r.center().y), egui::Align2::RIGHT_CENTER, chevron, egui::FontId::proportional(9.0), t.text_dim);
    ui.painter().line_segment([r.left_bottom(), r.right_bottom()], Stroke::new(1.0, t.border));
    response
}

/// A panel opened from the icon column, attached beside that icon. It is not a free window.
///
/// A group of panels uses the header as its tab strip and lines up with the group's first icon.
/// Dragging the header pulls the panel off the dock. A later click on the canvas closes it;
/// the click that opened it does not, because that id was not yet remembered.
pub fn flyout(app: &mut DesignApp, ctx: &egui::Context) {
    let remembered = ctx.data(|data| data.get_temp::<String>(egui::Id::new("flyout-open-id")));
    let Some(id) = app.ui.open_panel.clone() else {
        if remembered.is_some() {
            ctx.data_mut(|data| data.remove::<String>(egui::Id::new("flyout-open-id")));
        }
        return;
    };
    let t = Tokens::get(ctx);
    let groups = app.ui.icon_groups.clone().unwrap_or_default();
    let members = icon_group_ids(&groups, &id);
    let grouped = members.len() > 1;
    let width = if grouped { 312.0 } else { 268.0 };
    let screen = ctx.content_rect();
    // Flush with the icon bar's left edge. The expanded dock, when it is open, stays to the right of that bar.
    let x = right_bar(ctx).map(|bar| (bar.min.x - width).max(screen.min.x)).unwrap_or_else(|| {
        let dock_w = if app.ui.dock_expanded { ctx.memory(|m| m.area_rect(egui::Id::new("dock")).map(|r| r.width())).unwrap_or(280.0) } else { 0.0 };
        (screen.max.x - icon_column_width(app) - side_column_width(app) - dock_w - width).max(screen.min.x)
    });
    let anchor = members.first().copied().unwrap_or(id.as_str());
    let y = flyout_y(ctx, anchor);
    let pos = egui::pos2(x, y);
    let mut open = true;
    let mut shown = id.clone();
    // Middle keeps the flyout above the page and the dock. Menus, dialogs and floating
    // windows are Foreground, so they stay above it.
    let area = egui::Area::new(egui::Id::new("panel_flyout")).order(egui::Order::Middle).fixed_pos(pos).show(ctx, |ui| {
        egui::Frame::popup(ui.style()).fill(t.panel).inner_margin(egui::Margin::same(0)).show(ui, |ui| {
            ui.set_width(width);
            ui.set_max_height((screen.max.y - pos.y - 12.0).max(80.0));
            let stay = if grouped { group_strip(app, ui, &t, width, &members, &mut shown) } else { single_strip(app, ui, &t, width, &shown, pos) };
            if !stay {
                open = false;
                return;
            }
            if app.ui.open_panel.is_none() {
                return;
            }
            let room = (screen.max.y - pos.y - 48.0).clamp(80.0, 640.0);
            // Each panel hugs its content. History sizes from its rows. The others scroll only
            // once the body is taller than the space left beside the icon.
            let (body_h, margin) =
                if shown == "history" { (history_flyout_body(app, room.min(480.0)), egui::Margin::ZERO) } else { (room, egui::Margin::same(10)) };
            egui::Frame::NONE.inner_margin(margin).show(ui, |ui| {
                scrolled_body(app, ui, &shown, body_h);
            });
        });
    });
    if !open {
        app.ui.open_panel = None;
    }
    let flyout_rect = area.response.rect;
    let click = ctx.input(|input| input.pointer.interact_pos().filter(|_| input.pointer.primary_clicked()));
    let on_canvas = click.is_some_and(|point| app.canvas_rect.is_some_and(|rect| rect.contains(point)) && !flyout_rect.contains(point));
    // After the body, so a menu drawn from this flyout (it can extend past the flyout) counts
    // as open. The click that dismisses that menu does not also dismiss the flyout.
    let popup_open = ctx.any_popup_open();
    if app.ui.open_panel.as_deref() == Some(id.as_str()) && remembered.as_deref() == Some(id.as_str()) && on_canvas && !popup_open {
        app.ui.open_panel = None;
    }
    if let Some(current) = app.ui.open_panel.clone() {
        ctx.data_mut(|data| data.insert_temp(egui::Id::new("flyout-open-id"), current));
    } else {
        ctx.data_mut(|data| data.remove::<String>(egui::Id::new("flyout-open-id")));
    }
}

/// Group header: one tab per panel, then collapse and the panel menu. `false` closes the flyout.
fn group_strip(app: &mut DesignApp, ui: &mut egui::Ui, t: &Tokens, width: f32, members: &[&str], shown: &mut String) -> bool {
    let (strip, _) = ui.allocate_exact_size(vec2(width, 28.0), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, t.tab_strip);
    let menu_r = egui::Rect::from_min_size(egui::pos2(strip.max.x - 22.0, strip.min.y + 5.0), vec2(18.0, 18.0));
    let collapse_r = egui::Rect::from_min_size(egui::pos2(strip.max.x - 44.0, strip.min.y + 5.0), vec2(18.0, 18.0));
    let tabs_left = strip.min.x + 2.0;
    let avail = (collapse_r.min.x - 2.0 - tabs_left).max(40.0);
    let font = semibold(11.5);
    let lang = app.ui.language.clone();
    let mut widths: Vec<f32> = members
        .iter()
        .map(|panel| {
            let text = crate::i18n::tr(&lang, panel_label(panel));
            let galley = crate::rtl::plain(ui.ctx(), text, font.clone(), t.text);
            galley.size().x + if *panel == shown.as_str() { 28.0 } else { 16.0 }
        })
        .collect();
    if widths.iter().copied().sum::<f32>() > avail {
        let each = avail / widths.len().max(1) as f32;
        widths.fill(each);
    }
    let mut rects = Vec::with_capacity(members.len());
    let mut x = tabs_left;
    let mut torn: Option<&str> = None;
    for (panel, tab_w) in members.iter().copied().zip(widths.iter().copied()) {
        let rect = egui::Rect::from_min_size(egui::pos2(x, strip.min.y), vec2(tab_w, strip.height()));
        let response = ui.interact(rect, ui.id().with(("flytab", panel)), Sense::click_and_drag());
        if response.clicked() {
            app.ui.open_panel = Some(panel.to_string());
            *shown = panel.to_string();
        }
        if response.drag_stopped() && response.total_drag_delta().is_some_and(|delta| delta.length() > 24.0) {
            torn = Some(panel);
        }
        rects.push(rect);
        x += tab_w;
    }
    if let Some(panel) = torn {
        let at = ui.ctx().pointer_latest_pos().unwrap_or(strip.min) - vec2(20.0, 10.0);
        float_panel(app, panel, at);
        return true;
    }
    for (panel, rect) in members.iter().copied().zip(rects) {
        let active = panel == shown.as_str();
        if active {
            ui.painter().rect_filled(rect, 0.0, t.panel);
            diamond(ui.painter(), rect.left_center() + vec2(8.0, 0.0), 3.0, t.accent);
        }
        let color = if active { t.text_strong } else { t.text_dim };
        let text_x = rect.min.x + if active { 16.0 } else { 6.0 };
        let label = crate::i18n::tr(&lang, panel_label(panel));
        let galley = crate::rtl::truncated(ui.ctx(), label, font.clone(), color, (rect.max.x - 4.0 - text_x).max(8.0));
        ui.painter().galley(egui::pos2(text_x, rect.center().y - galley.size().y / 2.0), galley, color);
    }
    ui.painter().line_segment([strip.left_bottom(), strip.right_bottom()], Stroke::new(1.0, t.border));
    let collapse = ui.interact(collapse_r, ui.id().with("flycollapse"), Sense::click());
    if collapse.hovered() {
        ui.painter().rect_filled(collapse_r, 3.0, t.hover);
    }
    ui.painter().text(collapse_r.center(), egui::Align2::CENTER_CENTER, "»", egui::FontId::proportional(12.0), t.text_dim);
    crate::panel_docking::header_menu(app, ui, menu_r, shown);
    !collapse.clicked()
}

/// One panel: title, menu, float, close. `false` closes the flyout.
fn single_strip(app: &mut DesignApp, ui: &mut egui::Ui, t: &Tokens, width: f32, id: &str, pos: egui::Pos2) -> bool {
    let label = crate::i18n::tr(&app.ui.language, panel_label(id)).to_owned();
    let (strip, _) = ui.allocate_exact_size(vec2(width, 26.0), Sense::hover());
    ui.painter().rect_filled(strip, 0.0, t.panel_darker);
    crate::rtl::paint(ui.painter(), strip.min + vec2(10.0, 13.0), egui::Align2::LEFT_CENTER, &label, semibold(12.0), t.text_strong);
    let close = egui::Rect::from_min_size(egui::pos2(strip.max.x - 22.0, strip.min.y + 4.0), vec2(18.0, 18.0));
    let float_r = egui::Rect::from_min_size(egui::pos2(strip.max.x - 44.0, strip.min.y + 4.0), vec2(18.0, 18.0));
    let menu_r = egui::Rect::from_min_size(egui::pos2(strip.max.x - 66.0, strip.min.y + 4.0), vec2(18.0, 18.0));
    crate::panel_docking::header_menu(app, ui, menu_r, id);
    let closed = ui.interact(close, ui.id().with("flyclose"), Sense::click()).clicked();
    ui.painter().text(close.center(), egui::Align2::CENTER_CENTER, "×", egui::FontId::proportional(15.0), t.text_dim);
    if closed {
        return false;
    }
    let fr = ui.interact(float_r, ui.id().with("flyfloat"), Sense::click()).on_hover_ui(|ui| {
        crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Float panel"));
    });
    float_glyph(ui.painter(), float_r, t.text_dim);
    let head = egui::Rect::from_min_max(strip.min, egui::pos2(menu_r.min.x - 2.0, strip.max.y));
    let hr = ui.interact(head, ui.id().with("flyhead"), Sense::drag());
    let torn = hr.drag_stopped() && hr.total_drag_delta().is_some_and(|delta| delta.length() > 24.0);
    if fr.clicked() || torn {
        let at = if torn { ui.ctx().pointer_latest_pos().unwrap_or(pos) - vec2(20.0, 10.0) } else { pos - vec2(30.0, -30.0) };
        float_panel(app, id, at);
    }
    true
}

/// Height of the History flyout body: one row per state, plus the footer, never past `max_h`.
fn history_flyout_body(app: &DesignApp, max_h: f32) -> f32 {
    let rows = app.session.active().map(|state| state.history.undo.len() + state.history.redo.len() + 1).unwrap_or(1);
    (rows as f32 * 22.0 + 36.0).clamp(72.0, max_h.max(72.0))
}

fn diamond(painter: &egui::Painter, center: egui::Pos2, radius: f32, color: Color32) {
    painter.add(egui::Shape::convex_polygon(
        vec![center + vec2(0.0, -radius), center + vec2(radius, 0.0), center + vec2(0.0, radius), center + vec2(-radius, 0.0)],
        color,
        Stroke::NONE,
    ));
}

/// A panel's contents at most `max_h` tall, scrolling past that. The Pages panel scrolls its
/// parts itself and keeps its footer at the bottom, so it gets the height instead.
fn scrolled_body(app: &mut DesignApp, ui: &mut egui::Ui, id: &str, max_h: f32) {
    if scrolls_itself(id) {
        let min = if id == "history" { 72.0 } else { 160.0 };
        ui.allocate_ui(vec2(ui.available_width(), max_h.clamp(min, 560.0)), |ui| panel_body(app, ui, id));
    } else {
        egui::ScrollArea::vertical().max_height(max_h).show(ui, |ui| panel_body(app, ui, id));
    }
}

/// Panels that lay out their own scrolling parts (the Pages panel: parents, pages, footer).
pub fn scrolls_itself(id: &str) -> bool {
    id == "pages" || id == "history"
}

/// The contents of an icon-column panel.
pub fn panel_body(app: &mut DesignApp, ui: &mut egui::Ui, id: &str) {
    match id {
        "properties" => panels::properties::show(app, ui),
        "pages" => panels::pages::show(app, ui),
        "layers" => panels::layers::show(app, ui),
        "swatches" => panels::swatches::show(app, ui),
        "paragraphStyles" => panels::styles::paragraph(app, ui),
        "characterStyles" => panels::styles::character(app, ui),
        "stroke" => panels::properties::stroke_panel(app, ui),
        "character" => panels::properties::character_panel(app, ui),
        "typography" => panels::typography::show(app, ui),
        "glyphs" => panels::glyphs::show(app, ui),
        "history" => panels::history::show(app, ui),
        "paragraph" => panels::properties::paragraph_panel(app, ui),
        "story" => panels::story::show(app, ui),
        "textWrap" => panels::properties::wrap_panel(app, ui),
        "align" => panels::properties::align_panel(app, ui),
        "pathfinder" => panels::properties::pathfinder_panel(app, ui),
        "flexLayout" => panels::properties::flex_panel(app, ui),
        "color" => panels::swatches::color_panel(app, ui),
        "gradient" => panels::swatches::gradient_panel(app, ui),
        "effects" => panels::properties::effects_panel(app, ui),
        "links" => panels::properties::links_panel(app, ui),
        "preflight" => panels::properties::preflight_panel(app, ui),
        "attributes" => panels::properties::attributes_panel(app, ui),
        "conditions" => panels::conditions::show(app, ui),
        "notes" => panels::notes::show(app, ui),
        "library" => panels::library::show(app, ui),
        "hyperlinks" => panels::interactive::hyperlinks(app, ui),
        "bookmarks" => panels::interactive::bookmarks(app, ui),
        "articles" => panels::interactive::articles(app, ui),
        "tags" => panels::interactive::tags(app, ui),
        "book" => panels::library::book(app, ui),
        "states" => panels::interactive::states(app, ui),
        "buttons" => panels::interactive::buttons(app, ui),
        "liquid" => panels::interactive::liquid(app, ui),
        "media" => panels::interactive::media(app, ui),
        "transitions" => panels::interactive::transitions(app, ui),
        "trackChanges" => panels::interactive::track_changes(app, ui),
        "scripts" => panels::library::scripts(app, ui),
        "dataMerge" => panels::datamerge::show(app, ui),
        "table" => panels::table::show(app, ui),
        "tabs" => panels::tabs::show(app, ui),
        _ => panels::properties::info_panel(app, ui),
    }
}

/// Panels that only float, outside the dock and the docking layout (the Tabs panel).
pub(crate) fn float_only(id: &str) -> bool {
    FLOATING_PANELS.iter().any(|p| p.0 == id)
}

/// Tear panel `id` off the dock into a floating window at `at`.
pub fn float_panel(app: &mut DesignApp, id: &str, at: egui::Pos2) {
    if float_only(id) {
        app.ui.floating.retain(|(p, _)| p != id);
        app.ui.floating.push((id.to_string(), [at.x, at.y]));
        return;
    }
    if let Some(Err(error)) = crate::panel_docking::command(app, "window.panel.float", &serde_json::json!({"panel":id,"x":at.x,"y":at.y})) {
        app.status(error);
    }
}

/// Put a floating panel on the left of the right icon bar, in line with its icon.
pub fn dock_panel(app: &mut DesignApp, id: &str) {
    if float_only(id) {
        app.ui.floating.retain(|(p, _)| p != id);
        return;
    }
    if let Err(error) = crate::panel_docking::attach_beside(app, id) {
        app.status(error);
    }
}

/// Floating panels: movable, resizable windows; "Dock" returns one to the icon column.
pub fn floating(app: &mut DesignApp, ctx: &egui::Context) {
    // A customized layout draws its own floating groups; float-only panels are drawn here.
    let customized = app.ui.docking.is_some();
    let t = Tokens::get(ctx);
    for (id, at) in app.ui.floating.clone() {
        if customized && !float_only(&id) {
            continue;
        }
        let label = crate::i18n::tr(&app.ui.language, panel_label(&id)).to_owned();
        let docks = !float_only(&id);
        let mut open = true;
        let mut dock = false;
        let mut w = egui::Window::new(label.clone())
            .id(egui::Id::new(("floating_panel", &id)))
            .title_bar(false)
            .default_pos(egui::pos2(at[0], at[1]))
            .default_width(if id == "tabs" { 560.0 } else { 256.0 })
            .resizable(true)
            .frame(egui::Frame::window(&ctx.global_style()).fill(t.panel).inner_margin(egui::Margin::same(0)));
        // The Tabs panel moves over the text frame it edits, at the column's width.
        if id == "tabs"
            && let Some((pos, width)) = app.ui.tabs_panel.place.take()
        {
            w = w.current_pos(pos).min_width(width).max_width(width);
        }
        let r = w.show(ctx, |ui| {
            // Header strip (drag it to move the panel): name, Dock, close.
            let (strip, _) = ui.allocate_exact_size(vec2(ui.available_width().max(200.0), 24.0), Sense::hover());
            ui.painter().rect_filled(strip, 0.0, t.panel_darker);
            crate::rtl::paint(ui.painter(), strip.min + vec2(10.0, 12.0), egui::Align2::LEFT_CENTER, &label, semibold(12.0), t.text_strong);
            let close = egui::Rect::from_min_size(egui::pos2(strip.max.x - 22.0, strip.min.y + 3.0), vec2(18.0, 18.0));
            if ui
                .interact(close, ui.id().with("fclose"), Sense::click_and_drag())
                .on_hover_ui(|ui| {
                    crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Close"));
                })
                .clicked()
            {
                open = false;
            }
            ui.painter().text(close.center(), egui::Align2::CENTER_CENTER, "×", egui::FontId::proportional(15.0), t.text_dim);
            let dock_r = egui::Rect::from_min_size(egui::pos2(strip.max.x - 44.0, strip.min.y + 3.0), vec2(18.0, 18.0));
            let menu_x = if docks { strip.max.x - 66.0 } else { strip.max.x - 44.0 };
            let menu_r = egui::Rect::from_min_size(egui::pos2(menu_x, strip.min.y + 3.0), vec2(18.0, 18.0));
            crate::panel_docking::header_menu(app, ui, menu_r, &id);
            if docks
                && ui
                    .interact(dock_r, ui.id().with("fdock"), Sense::click_and_drag())
                    .on_hover_ui(|ui| {
                        crate::rtl::label(ui, crate::i18n::tr(&app.ui.language, "Dock panel"));
                    })
                    .clicked()
            {
                dock = true;
            }
            if docks {
                dock_glyph(ui.painter(), dock_r, t.text_dim);
            }
            egui::Frame::NONE.inner_margin(egui::Margin::same(10)).show(ui, |ui| {
                scrolled_body(app, ui, &id, ctx.content_rect().height() - 160.0);
            });
        });
        // Remember where it was left.
        let dropped_on_bar = r.as_ref().is_some_and(|r| {
            r.response.drag_stopped()
                && ctx
                    .input(|input| input.pointer.interact_pos())
                    .is_some_and(|point| right_bar(ctx).is_some_and(|bar| beside_bar_zone(bar).contains(point)))
        });
        if let Some(r) = r
            && let Some(f) = app.ui.floating.iter_mut().find(|(p, _)| *p == id)
        {
            f.1 = [r.response.rect.min.x, r.response.rect.min.y];
        }
        if !open || dock || dropped_on_bar {
            dock_panel(app, &id);
        }
    }
}

/// Two overlapping boxes: "float this panel".
fn float_glyph(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let s = Stroke::new(1.0, c);
    p.rect_stroke(egui::Rect::from_min_size(r.min + vec2(3.5, 6.5), vec2(9.0, 8.0)), 0.0, s, egui::StrokeKind::Middle);
    p.rect_stroke(egui::Rect::from_min_size(r.min + vec2(6.5, 3.5), vec2(9.0, 8.0)), 0.0, s, egui::StrokeKind::Middle);
}

/// A box with a bar on its right: "back into the dock".
fn dock_glyph(p: &egui::Painter, r: egui::Rect, c: Color32) {
    let b = egui::Rect::from_min_size(r.min + vec2(3.5, 4.5), vec2(11.0, 9.0));
    p.rect_stroke(b, 0.0, Stroke::new(1.0, c), egui::StrokeKind::Middle);
    p.rect_filled(egui::Rect::from_min_max(egui::pos2(b.max.x - 3.5, b.min.y), b.max), 0.0, c);
}

#[cfg(test)]
mod visual_regressions {
    use super::*;

    fn scaled_input(size: egui::Vec2, scale: f32) -> egui::RawInput {
        let mut input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, size)), ..Default::default() };
        input.viewports.entry(egui::ViewportId::ROOT).or_default().native_pixels_per_point = Some(scale);
        input
    }

    /// Opt-in evidence for native screenshot review; ordinary test runs stay headless and fast.
    #[test]
    fn capture_dock_visual_fixtures() {
        let Some(directory) = std::env::var_os("CRAFT_UI_VISUAL_FIXTURES").map(std::path::PathBuf::from) else { return };
        std::fs::create_dir_all(&directory).unwrap();
        for brightness in crate::theme::Brightness::ALL {
            for width in [800.0, 1280.0] {
                for scale in [1.0, 1.5, 2.0] {
                    let mut app = DesignApp::new(designcraft_engine::Session::new(), Default::default());
                    app.ui.dock_expanded = true;
                    app.ui.dock_tab = "properties".into();
                    let mut ready = false;
                    let mut harness =
                        egui_kittest::Harness::builder().with_size(vec2(width, 640.0)).with_pixels_per_point(scale).wgpu().build_ui_state(
                            move |ui, app: &mut DesignApp| {
                                if !ready {
                                    crate::theme::install_fonts(ui.ctx(), "en");
                                    crate::theme::apply(ui.ctx(), &Tokens::for_brightness(brightness));
                                    ready = true;
                                    return;
                                }
                                show(app, ui);
                            },
                            app,
                        );
                    harness.input_mut().max_texture_side = Some(8192);
                    harness.run_steps(4);
                    let stem = format!("designcraft-dock-{}-{width}-{scale}x", brightness.id());
                    harness.render().unwrap().save(directory.join(format!("{stem}.png"))).unwrap();
                    let panel = egui::containers::panel::PanelState::load(&harness.ctx, egui::Id::new("dock")).unwrap().outer_rect;
                    let region = serde_json::json!({
                        "screen": [width, 640.0], "scale": scale,
                        "regions": {"dock-tabs": [panel.left(), panel.top(), panel.width(), 45.0]}
                    });
                    std::fs::write(directory.join(format!("{stem}.json")), serde_json::to_vec_pretty(&region).unwrap()).unwrap();
                }
            }
        }
    }

    fn luminance(c: Color32) -> f32 {
        let linear = |v: u8| {
            let v = f32::from(v) / 255.0;
            if v <= 0.04045 { v / 12.92 } else { ((v + 0.055) / 1.055).powf(2.4) }
        };
        0.2126 * linear(c.r()) + 0.7152 * linear(c.g()) + 0.0722 * linear(c.b())
    }

    #[test]
    fn selected_dock_tab_has_readable_rendered_contrast_in_every_theme() {
        for brightness in crate::theme::Brightness::ALL {
            for width in [800.0, 1280.0] {
                for scale in [1.0, 1.5, 2.0] {
                    let ctx = egui::Context::default();
                    crate::theme::install_fonts(&ctx, "en");
                    let tokens = Tokens::for_brightness(brightness);
                    crate::theme::apply(&ctx, &tokens);
                    let mut app = DesignApp::new(designcraft_engine::Session::new(), Default::default());
                    app.ui.dock_expanded = true;
                    app.ui.dock_tab = "properties".into();
                    let mut output = ctx.run_ui(scaled_input(vec2(width, 640.0), scale), |ui| show(&mut app, ui));
                    output.textures_delta.clear();
                    assert_eq!(ctx.content_rect().size(), vec2(width, 640.0));
                    assert_eq!(ctx.pixels_per_point(), scale);
                    let text = output
                        .shapes
                        .iter()
                        .find_map(|shape| match &shape.shape {
                            egui::Shape::Text(text) if text.galley.job.text == "Properties" => Some(text),
                            _ => None,
                        })
                        .expect("the active Properties tab is painted");
                    let color = text.galley.job.sections.first().expect("tab text has a format").format.color;
                    let a = luminance(color);
                    let b = luminance(tokens.panel);
                    let contrast = (a.max(b) + 0.05) / (a.min(b) + 0.05);
                    assert!(contrast >= 4.5, "{brightness:?} at {width}: selected tab contrast is {contrast:.2}");
                }
            }
        }
    }
}

#[cfg(test)]
mod chevron_tests {
    use super::*;
    use egui_kittest::Harness;

    fn click(harness: &mut Harness<'_, DesignApp>, at: egui::Pos2) {
        harness.hover_at(at);
        harness.run_steps(1);
        harness.drag_at(at);
        harness.run_steps(1);
        harness.drop_at(at);
        harness.run_steps(3);
    }

    /// The » / « chevron painted at the right end of a panel's 11 pt header.
    fn chevron(harness: &Harness<'_, DesignApp>, panel: &str) -> egui::Pos2 {
        let r = egui::containers::panel::PanelState::load(&harness.ctx, egui::Id::new(panel)).expect("the panel is laid out").outer_rect;
        egui::pos2(r.max.x - 10.0, r.min.y + 5.0)
    }

    /// Issue #1203: the header chevrons were painted text; clicking them did nothing.
    #[test]
    fn the_dock_chevron_collapses_and_expands_the_dock() {
        let mut app = DesignApp::new(designcraft_engine::Session::new(), Default::default());
        app.ui.dock_expanded = true;
        let mut ready = false;
        let mut harness = Harness::builder().with_size(vec2(1280.0, 640.0)).build_ui_state(
            move |ui, app: &mut DesignApp| {
                if !ready {
                    crate::theme::install_fonts(ui.ctx(), "en");
                    ready = true;
                    return;
                }
                show(app, ui);
            },
            app,
        );
        harness.run_steps(4);
        let at = chevron(&harness, "dock");
        click(&mut harness, at);
        assert!(!harness.state().ui.dock_expanded, "» collapses the dock to icons");
        let at = chevron(&harness, "icon_column");
        click(&mut harness, at);
        assert!(harness.state().ui.dock_expanded, "« expands the dock again");
    }
}
