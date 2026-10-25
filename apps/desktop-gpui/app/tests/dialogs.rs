// SPDX-License-Identifier: AGPL-3.0-only
//! Settings' dialogs as Base UI keeps them: focus moves into one as it opens and Tab stays inside it, so no key reaches
//! the page under the scrim, focus goes back to the button that opened it, and a press on the scrim closes a `Dialog`
//! but leaves an `AlertDialog` open. Drawn from records given, with no host behind.

use std::sync::Arc;

use desktop_gpui::screens::dialogs::layer;
use desktop_gpui::screens::settings::model::{At, Data, GroupId};
use desktop_gpui::screens::settings::page::DialogBuild;
use desktop_gpui::screens::settings::{Ctx, SettingsScreen, DIALOG};
use desktop_gpui::ui::{self, AlertDialog, Side};
use gpui_kit::component::Root;
use gpui_kit::{
    point, px, size, AnyElement, App, AppContext, Entity, FocusHandle, IntoElement, KeyDownEvent, KeyUpEvent, Keystroke, Modifiers,
    TestAppContext, VisualTestContext, Window,
};
use serde_json::json;

fn runtime() -> Arc<tokio::runtime::Runtime> {
    Arc::new(tokio::runtime::Builder::new_multi_thread().worker_threads(1).enable_all().build().unwrap())
}

/// Settings on Devices with two devices to revoke, laptop and phone, in a window as the app draws it.
fn devices(cx: &mut TestAppContext) -> (Entity<SettingsScreen>, &mut VisualTestContext) {
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(Side::Light, cx);
    });
    let device = |id: &str, name: &str| {
        let at = "2026-10-17T00:00:00Z";
        serde_json::from_value(json!({ "id": id, "name": name, "createdAt": at, "lastSeenAt": at })).unwrap()
    };
    let data = Data { devices: Some(vec![device("d1", "laptop"), device("d2", "phone")]), ..Data::default() };
    let mut screen = None;
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::with_data(data, At::Group(GroupId::Devices), runtime(), window, cx));
        screen = Some(view.clone());
        Root::new(view, window, cx)
    });
    cx.simulate_resize(size(px(1280.), px(800.)));
    cx.run_until_parked();
    (screen.unwrap(), cx)
}

/// A key pressed and let go, GPUI's keyboard click landing on the release, and the next frame delivered, which a test
/// window has no loop for.
fn press(cx: &mut VisualTestContext, key: &str) {
    let keystroke = Keystroke::parse(key).unwrap();
    cx.simulate_event(KeyDownEvent { keystroke: keystroke.clone(), is_held: false, prefer_character_input: false });
    cx.simulate_event(KeyUpEvent { keystroke });
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.run_until_parked();
}

fn focused(cx: &mut VisualTestContext) -> Option<FocusHandle> {
    cx.update(|window, cx| window.focused(cx))
}

/// Whether the Settings dialog stands on the window's dialogs layer.
fn open_on_layer(cx: &mut VisualTestContext) -> bool {
    cx.update(|_, cx| layer::is_open(DIALOG, cx))
}

/// How many notices stand in the window's toasts.
fn said(cx: &mut VisualTestContext) -> usize {
    let toasts = cx.update(|window, cx| ui::toast::of(window, cx).expect("ui::init registers the stack"));
    toasts.read_with(cx, |t, _| t.standing().len())
}

fn in_dialog(cx: &mut VisualTestContext) -> bool {
    cx.update(|window, cx| gpui_kit::base::active_focus_trap(window, cx).is_some())
}

#[gpui_kit::test]
fn focus_moves_into_a_dialog_stays_there_and_goes_back(cx: &mut TestAppContext) {
    let (_, cx) = devices(cx);
    // The two Revokes are the window's last tab stops. With nothing focused a test window's keys reach no key context,
    // so Root's Shift-Tab is called as it runs it.
    cx.update(|window, cx| {
        window.focus_prev(cx);
        window.focus_prev(cx);
    });
    let revoke = focused(cx).expect("Shift-Tab reaches laptop's Revoke");
    press(cx, "enter");
    assert!(open_on_layer(cx) && cx.debug_bounds("alert-dialog").is_some(), "Enter on Revoke opens its dialog");
    let first = focused(cx);
    assert!(first.is_some() && first.as_ref() != Some(&revoke), "focus moves off the Revoke behind the scrim as the dialog opens");
    cx.simulate_keystrokes("tab");
    assert_ne!(focused(cx), first, "Tab moves to the dialog's next button");
    cx.simulate_keystrokes("tab");
    assert_eq!(focused(cx), first, "and from its last button back to its first, as Cancel and Revoke are all it holds");
    cx.simulate_keystrokes("shift-tab");
    assert!(in_dialog(cx), "Shift-Tab stays inside it too");
    press(cx, "enter");
    // Focus is on the dialog's Revoke, which with no host behind says none answered; phone's Revoke would open a dialog.
    assert_eq!(said(cx), 1, "Enter lands on the dialog's Revoke, not on phone's on the page");
    assert!(open_on_layer(cx));
    cx.simulate_keystrokes("escape");
    assert!(!open_on_layer(cx), "Escape closes an alert dialog");
    cx.run_until_parked();
    assert_eq!(focused(cx), Some(revoke), "focus goes back to the Revoke that opened it");
}

fn revoke_laptop(_: &Ctx, _: &mut Window, _: &mut App) -> AnyElement {
    AlertDialog::new("Revoke laptop?").layered().into_any_element()
}

#[gpui_kit::test]
fn a_press_on_the_scrim_closes_a_dialog_but_not_an_alert_dialog(cx: &mut TestAppContext) {
    let (screen, cx) = devices(cx);
    let row = cx.debug_bounds("settings-keybindings").expect("the sidebar's Keybindings row is drawn");
    let open = |cx: &mut VisualTestContext, dialog: DialogBuild| {
        cx.update(|_, cx| screen.update(cx, |s, cx| s.open_dialog(dialog, cx)));
        cx.run_until_parked();
    };

    open(cx, DialogBuild::alert(revoke_laptop));
    cx.simulate_click(point(px(640.), px(20.)), Modifiers::none());
    assert!(open_on_layer(cx), "a press on the scrim leaves an alert dialog open");
    cx.simulate_click(row.center(), Modifiers::none());
    assert!(open_on_layer(cx));
    assert_eq!(screen.read_with(cx, |s, _| s.at().clone()), At::Group(GroupId::Devices), "the press reaches nothing under the scrim");
    cx.update(|_, cx| screen.update(cx, |s, cx| s.close_dialog(cx)));
    cx.run_until_parked();
    assert!(!open_on_layer(cx), "its own Cancel closes it");

    open(cx, DialogBuild::dialog(revoke_laptop));
    cx.simulate_click(point(px(640.), px(20.)), Modifiers::none());
    assert!(!open_on_layer(cx), "a press on the scrim closes a plain dialog");
}
