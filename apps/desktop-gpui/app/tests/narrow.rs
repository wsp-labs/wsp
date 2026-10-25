// SPDX-License-Identifier: AGPL-3.0-only
//! Settings at today's app's narrow widths, each piece's boxes at 600, 760 and 1280 px against the `max-sm:` (under
//! 640) and `max-md:` (under 768) rules the web writes for it: a row's slot under its words, the row select's width,
//! an alert dialog as a sheet at the foot, the page column's sides and top, Restore defaults as its glyph, the theme
//! grid, the Computers list's columns, its line and its limit band, the Remove line, and Activity's views as a select.
//! Drawn from records given, with no host behind.

use std::sync::Arc;

use desktop_gpui::screens::dialogs::layer;
use desktop_gpui::screens::settings::model::{At, Data, GroupId};
use desktop_gpui::screens::settings::page::DialogBuild;
use desktop_gpui::screens::settings::{SettingsScreen, DIALOG};
use desktop_gpui::ui::grid::{LIST_COLUMNS, LIST_COLUMNS_NARROW};
use desktop_gpui::ui::select::{Select, SelectItem, SelectState};
use desktop_gpui::ui::{self, glyph_frame, AlertDialog, Button, Card, Glyph, Grid, GridCell, GridRow, PickRow, Row, Side, Still, Switch};
use gpui_kit::component::Root;
use gpui_kit::{div, prelude::*, px, size, AnyElement, Bounds, Context, Entity, Pixels, TestAppContext, VisualTestContext, Window};
use serde_json::json;

const WIDTHS: [f32; 3] = [600., 760., 1280.];
const HEIGHT: f32 = 1400.;

fn init(cx: &mut TestAppContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
}

type Draw = Box<dyn Fn(&mut Window, &mut Context<Lone>) -> AnyElement>;

struct Lone(Draw);

impl Render for Lone {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().p(px(24.)).child((self.0)(window, cx))
    }
}

/// `piece` alone in a window as the app wraps it, at the first width.
fn lone(cx: &mut TestAppContext, piece: impl Fn(&mut Window, &mut Context<Lone>) -> AnyElement + 'static) -> &mut VisualTestContext {
    init(cx);
    let view = cx.new(|_| Lone(Box::new(piece)));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
    cx
}

/// The window at `width`, drawn again until what a probe measured has settled.
fn at(cx: &mut VisualTestContext, width: f32) {
    cx.simulate_resize(size(px(width), px(HEIGHT)));
    cx.run_until_parked();
    cx.update(|window, _| window.refresh());
    cx.run_until_parked();
}

fn bounds(cx: &mut VisualTestContext, selector: &'static str, width: f32) -> Bounds<Pixels> {
    cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn at {width} px"))
}

#[track_caller]
fn near(what: &str, got: Pixels, want: f32) {
    let got = f32::from(got);
    assert!((got - want).abs() < 0.5, "{what}: {got} px, the web writes {want} px");
}

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap())
}

/// Settings standing on `at`, drawn off `data` in a window as the app draws it.
fn settings(cx: &mut TestAppContext, data: Data, at: At) -> (Entity<SettingsScreen>, &mut VisualTestContext) {
    init(cx);
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::with_data(data, at, runtime(), window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    (screen.unwrap(), cx)
}

/// Preferences as the host sends them, `theme` on the given side.
fn preferences(theme: &str) -> Data {
    let p = json!({ "theme": theme, "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {}, "access": {},
        "projectLook": {}, "projectOrder": [], "labs": false });
    Data { preferences: serde_json::from_value(p).ok(), ..Data::default() }
}

fn place(extra: serde_json::Value) -> wsp_host_client::protocol::PlaceView {
    let mut base = json!({ "id": "box1", "kind": "computer", "name": "box1", "default": false });
    for (k, v) in extra.as_object().unwrap() {
        base[k] = v.clone();
    }
    serde_json::from_value(base).unwrap()
}

#[gpui_kit::test]
fn a_row_stands_its_control_under_its_words_under_640(cx: &mut TestAppContext) {
    let cx = lone(cx, |_, _| {
        let control = div().debug_selector(|| "control-box".into()).w(px(120.)).h(px(30.));
        Card::new("card").item(Row::new("r", "Theme").note("How wsp looks.").control(control)).into_any_element()
    });
    for width in WIDTHS {
        at(cx, width);
        let (row, title, control) = (bounds(cx, "row", width), bounds(cx, "title", width), bounds(cx, "control-box", width));
        let words_bottom = bounds(cx, "note", width).bottom();
        if width < 640. {
            near("the control under the words, past SPLIT_CLASS's gap-3", control.top() - words_bottom, 12.);
            near("the control at the words' left edge, not set right", control.left(), f32::from(title.left()));
        } else {
            near("the control at the row's right inset", row.right() - control.right(), 20.);
            assert!(control.top() < title.bottom(), "the control stands beside the words at {width} px: {control:?} against {title:?}");
        }
    }
}

/// With no editor installed the web hands the row a null control, so its empty slot still stands, under the words and
/// past the gap below 640 px.
#[gpui_kit::test]
fn the_editor_row_keeps_its_empty_slot_with_no_editor_installed(cx: &mut TestAppContext) {
    let data = Data { editors: Some(Vec::new()), ..preferences("light") };
    let (_, cx) = settings(cx, data, At::Group(GroupId::General));
    for width in WIDTHS {
        at(cx, width);
        bounds(cx, "editor-slot", width);
    }
}

#[gpui_kit::test]
fn a_led_rows_slot_stands_under_its_words_not_its_glyph(cx: &mut TestAppContext) {
    let cx = lone(cx, |_, cx| {
        let control = div().debug_selector(|| "control-box".into()).w(px(120.)).h(px(30.));
        Card::new("card").item(Row::new("r", "Claude Code").lead(glyph_frame(Glyph::Terminal, cx)).control(control)).into_any_element()
    });
    at(cx, 600.);
    let (row, title, control) = (bounds(cx, "row", 600.), bounds(cx, "title", 600.), bounds(cx, "control-box", 600.));
    near("LED_SLOT_INDENT's pl-11 past the card inset", control.left() - row.left(), 20. + 44.);
    near("the control's left is the title's", control.left(), f32::from(title.left()));
}

#[gpui_kit::test]
fn a_pick_rows_slot_stands_under_its_name_72_px_in_under_640(cx: &mut TestAppContext) {
    let cx = lone(cx, |_, _| {
        let slot = div().debug_selector(|| "control-box".into()).w(px(120.)).h(px(30.));
        Card::new("card")
            .item(PickRow::new("p", "Claude Code", true).glyph(Glyph::Terminal).note("Signed in").slot(slot))
            .into_any_element()
    });
    for width in WIDTHS {
        at(cx, width);
        let (row, slot, note) = (bounds(cx, "row", width), bounds(cx, "control-box", width), bounds(cx, "note", width));
        if width < 640. {
            near("the slot under the words, past the label's gap-3", slot.top() - note.bottom(), 12.);
            near("PickRow's max-sm:pl-[72px] past the card inset", slot.left() - row.left(), 20. + 72.);
        } else {
            near("the slot at the row's right inset", row.right() - slot.right(), 20.);
            assert!(slot.top() < note.bottom(), "the slot stands beside the words at {width} px: {slot:?} against {note:?}");
        }
    }
}

#[gpui_kit::test]
fn the_switch_grows_to_a_20_px_thumb_under_640(cx: &mut TestAppContext) {
    let cx = lone(cx, |_, _| Switch::new("s", true).into_any_element());
    for width in WIDTHS {
        at(cx, width);
        let switch = bounds(cx, "switch", width);
        // `[--thumb-size:--spacing(5)] sm:[--thumb-size:--spacing(4)]`, the track `thumb*2-2` by `thumb+2`.
        let thumb = if width < 640. { 20. } else { 16. };
        near(&format!("the track's width at {width} px"), switch.size.width, thumb * 2. - 2.);
        near(&format!("the track's height at {width} px"), switch.size.height, thumb + 2.);
    }
}

#[gpui_kit::test]
fn the_row_select_is_144_wide_under_640_and_176_over(cx: &mut TestAppContext) {
    init(cx);
    let state = cx.new(|_| SelectState::new(vec![SelectItem::new(1u8, "System"), SelectItem::new(2u8, "Light")], Some(&1)));
    let view = cx.new(move |_| Lone(Box::new(move |_, _| Select::new(&state).into_any_element())));
    let (_, cx) = cx.add_window_view(|window, cx| Root::new(view, window, cx));
    for width in WIDTHS {
        at(cx, width);
        let want = if width < 640. { 144. } else { 176. };
        near(&format!("the select at {width} px (SELECT_WIDTH, max-sm:w-36)"), bounds(cx, "select", width).size.width, want);
    }
}

#[gpui_kit::test]
fn an_alert_dialog_is_a_sheet_at_the_windows_foot_under_640(cx: &mut TestAppContext) {
    let (screen, cx) = settings(cx, preferences("light"), At::Group(GroupId::General));
    // Every move of the layer held at its end, so the popup stands where it comes to rest.
    cx.update(|_, cx| cx.set_global(Still(true)));
    at(cx, 1280.);
    cx.update(|_, cx| {
        screen.update(cx, |s, cx| {
            s.open_dialog(
                DialogBuild::alert(|_, _, _| {
                    AlertDialog::new("Revoke phone?")
                        .layered()
                        .sentence("It signs phone out of wsp.")
                        .act(Button::outline("cancel", "Cancel").neutral_ring())
                        .act(Button::destructive("confirm", "Revoke"))
                        .into_any_element()
                }),
                cx,
            )
        })
    });
    for width in WIDTHS {
        at(cx, width);
        assert_eq!(cx.update(|_, cx| layer::top(cx)), Some(DIALOG), "the alert stands on the window's dialogs layer");
        let dialog = bounds(cx, "alert-dialog", width);
        let (cancel, confirm) = (bounds(cx, "dialog-act-0", width), bounds(cx, "dialog-act-1", width));
        if width < 640. {
            near("the sheet's width, the window's", dialog.size.width, width);
            near("the sheet's left", dialog.left(), 0.);
            near("the sheet's foot on the window's", dialog.bottom(), HEIGHT);
            // flex-col-reverse gap-2 px-5 pt-3 pb-4: Revoke over Cancel, each the sheet's width less its sides.
            near("Revoke over Cancel, gap-2 apart", cancel.top() - confirm.bottom(), 8.);
            near("pb-4 under Cancel", dialog.bottom() - cancel.bottom(), 16.);
            near("Cancel across the sheet less px-5", cancel.size.width, width - 40.);
        } else {
            near("max-w-110", dialog.size.width, 440.);
            near("centred across the window", dialog.left(), (width - 440.) / 2.);
            assert!(dialog.bottom() < px(HEIGHT - 100.), "centred down the window too: {dialog:?}");
            near("the acts side by side", confirm.top(), f32::from(cancel.top()));
            near("pb-4 and the popup's 1 px border under the acts", dialog.bottom() - cancel.bottom(), 17.);
        }
    }
}

#[gpui_kit::test]
fn the_page_column_keeps_16_px_sides_and_a_24_px_top_under_640(cx: &mut TestAppContext) {
    // Account's one card, so the card the selector names is the column's first.
    let (_, cx) = settings(cx, preferences("light"), At::Group(GroupId::Account));
    for width in WIDTHS {
        at(cx, width);
        let (page, head) = (bounds(cx, "settings-page", width), bounds(cx, "card", width));
        let (sides, top): (f32, f32) = if width < 640. { (16., 24.) } else { (32., 28.) };
        // `mx-auto max-w-[760px]` under border-box sizing: the column's side, or past it, centred, once the page is wider.
        let side = sides.max((f32::from(page.size.width) - (760. - 2. * sides)) / 2.);
        near(&format!("the column's side at {width} px"), head.left() - page.left(), side);
        near(&format!("the column's top at {width} px"), head.top() - page.top(), top);
    }
}

#[gpui_kit::test]
fn restore_defaults_is_its_glyph_alone_under_640(cx: &mut TestAppContext) {
    let (_, cx) = settings(cx, preferences("light"), At::Group(GroupId::Appearance));
    for width in WIDTHS {
        at(cx, width);
        let restore = bounds(cx, "restore-defaults", width);
        if width < 640. {
            // An xs button's px-2 either side of the 14 px glyph, inside its 1 px edge.
            near("Restore defaults as its glyph", restore.size.width, 8. + 14. + 8. + 2.);
        } else {
            assert!(restore.size.width > px(80.), "Restore defaults carries its words at {width} px: {restore:?}");
        }
    }
}

#[gpui_kit::test]
fn the_theme_grid_takes_two_columns_and_8_px_gaps_under_640(cx: &mut TestAppContext) {
    let (_, cx) = settings(cx, preferences("light"), At::Group(GroupId::Appearance));
    for width in WIDTHS {
        at(cx, width);
        at(cx, width);
        let (paper, linen, frost) =
            (bounds(cx, "theme-option-paper", width), bounds(cx, "theme-option-linen", width), bounds(cx, "theme-option-frost", width));
        let picker = bounds(cx, "theme-picker", width);
        let (system, light) = (bounds(cx, "theme-mode-system", width), bounds(cx, "theme-mode-light", width));
        let gap = if width < 640. { 8. } else { 12. };
        near(&format!("the theme gap at {width} px"), linen.left() - paper.right(), gap);
        near(&format!("the mode gap at {width} px"), light.left() - system.right(), gap);
        if width < 640. {
            near("two columns across the picker", paper.size.width, (f32::from(picker.size.width) - 8.) / 2.);
            assert!(frost.top() > paper.bottom(), "the third theme starts the second line: {frost:?} under {paper:?}");
        } else {
            assert!(paper.size.width >= px(208.), "a column at least 13rem at {width} px: {paper:?}");
        }
    }
}

#[gpui_kit::test]
fn the_computers_list_drops_cores_and_memory_under_768(cx: &mut TestAppContext) {
    let here = place(json!({ "id": "here", "kind": "computer", "name": "here", "shape": { "cpu": 8, "memMb": 16384 } }));
    let data = Data { places: vec![here], places_read: true, ..preferences("light") };
    let (_, cx) = settings(cx, data, At::Group(GroupId::Computers));
    for width in WIDTHS {
        at(cx, width);
        let row = bounds(cx, "grid-row", width);
        let threads = bounds(cx, "place-threads", width);
        if width < 768. {
            assert!(cx.debug_bounds("place-cores").is_none(), "Cores stands down at {width} px");
            assert!(cx.debug_bounds("place-memory").is_none(), "Memory stands down at {width} px");
            // `minmax(0,1fr) auto 72px 14px`: the threads' cell as wide as its figure, then the state, the chevron.
            near("the threads' column against the state and the chevron", row.right() - threads.right(), 16. + 72. + 16. + 14. + 20.);
            assert!(threads.size.width < px(20.), "the threads' column is as wide as its one figure: {threads:?}");
        } else {
            let (cores, memory) = (bounds(cx, "place-cores", width), bounds(cx, "place-memory", width));
            near("Cores' column", cores.size.width, 56.);
            near("Memory's column", memory.size.width, 72.);
            near("Threads' column", threads.size.width, 64.);
        }
    }
}

#[gpui_kit::test]
fn a_rows_second_line_spans_the_narrow_templates_columns(cx: &mut TestAppContext) {
    let cx = lone(cx, |_, _| {
        let line = vec![
            GridCell::new(div().debug_selector(|| "plot".into()).w_full().h(px(18.)), 3, 2),
            GridCell::new(div().debug_selector(|| "figures".into()).w_full().h(px(18.)), 3, 2),
        ];
        let row = GridRow::new("r", LIST_COLUMNS)
            .narrow(LIST_COLUMNS_NARROW)
            .cell(div().child("here"))
            .wide(div().child("8"))
            .wide(div().child("16 GB"))
            .cell(div().child("0"))
            .cell(div().child("Ready"))
            .cell(div())
            .line(line);
        Grid::new("g").row(row).into_any_element()
    });
    for width in WIDTHS {
        at(cx, width);
        let figures = bounds(cx, "figures", width);
        let want = if width < 768. { 72. + 16. + 14. } else { 64. + 16. + 72. + 16. + 14. };
        near(&format!("the figures' span at {width} px"), figures.size.width, want);
    }
}

#[gpui_kit::test]
fn the_limit_band_folds_to_two_columns_under_768(cx: &mut TestAppContext) {
    let cloud = place(json!({ "id": "boat", "kind": "provider", "name": "box",
        "plan": { "key": "box_20", "trial": false, "machinesAtOnce": 4, "running": 1, "sizes": ["2x4"] },
        "starts": { "minute": { "used": 0, "limit": 5 }, "hour": { "used": 1, "limit": 30 }, "day": { "used": 3, "limit": 100 } } }));
    let data = Data { places: vec![cloud], places_read: true, ..preferences("light") };
    let (_, cx) = settings(cx, data, At::Computer("boat".into()));
    for width in WIDTHS {
        at(cx, width);
        let (machines, minute, hour) =
            (bounds(cx, "limit-machines", width), bounds(cx, "limit-minute", width), bounds(cx, "limit-hour", width));
        near("the second column beside the first", minute.top(), f32::from(machines.top()));
        if width < 768. {
            assert!(hour.top() > machines.bottom(), "the third limit starts the second line at {width} px: {hour:?}");
            near("gap-y-4 between the lines", hour.top() - machines.bottom(), 16.);
            near("the second line's first column under the first", hour.left(), f32::from(machines.left()));
        } else {
            near("four across one line", hour.top(), f32::from(machines.top()));
        }
    }
}

#[gpui_kit::test]
fn the_remove_line_stacks_its_words_over_its_button_under_640(cx: &mut TestAppContext) {
    let here = place(json!({ "id": "here", "kind": "computer", "name": "here" }));
    let data = Data { places: vec![here, place(json!({}))], places_read: true, ..preferences("light") };
    let (_, cx) = settings(cx, data, At::Computer("box1".into()));
    for width in WIDTHS {
        at(cx, width);
        let (line, button, note) = (bounds(cx, "remove-line", width), bounds(cx, "remove-button", width), bounds(cx, "remove-note", width));
        if width < 640. {
            near("the button under the words, gap-3", button.top() - note.bottom(), 12.);
            near("the button across the line's inset", button.size.width, f32::from(line.size.width) - 40.);
        } else {
            near("the button at the line's right inset", line.right() - button.right(), 20.);
            assert!(button.top() < note.bottom(), "the button beside the words at {width} px: {button:?} against {note:?}");
        }
    }
}

#[gpui_kit::test]
fn activitys_views_are_a_select_under_640(cx: &mut TestAppContext) {
    let (_, cx) = settings(cx, preferences("light"), At::Group(GroupId::Activity));
    for width in WIDTHS {
        at(cx, width);
        let narrow = cx.debug_bounds("activity-views-narrow");
        if width < 640. {
            let select = narrow.unwrap_or_else(|| panic!("the views' select at {width} px"));
            near("the views' select at SELECT_WIDTH's max-sm:w-36", select.size.width, 144.);
            assert!(cx.debug_bounds("segmented").is_none(), "no segments at {width} px");
        } else {
            assert!(narrow.is_none(), "no select at {width} px");
            assert!(cx.debug_bounds("segmented").is_some(), "the views' segments at {width} px");
        }
    }
}
