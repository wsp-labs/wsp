// SPDX-License-Identifier: AGPL-3.0-only
//! The Appearance type sample drawn with this computer's own text system, which on a Mac is made on the main thread
//! alone: a binary of its own with no test harness, so `main` is that thread.

use desktop_gpui::screens::settings::model::{At, Data, GroupId};
use desktop_gpui::screens::settings::pages::appearance::type_sample_code;
use desktop_gpui::screens::settings::SettingsScreen;
use desktop_gpui::ui;
use gpui_kit::{px, AppContext as _, TextRun};
use serde_json::Value;

/// The type card's code block drawn with this computer's own text system, the preferences picking `fonts`: its height,
/// and the height its code takes in `font` at `size`, shaped to the block's own width.
fn drawn_and_shaped(fonts: Value, font: &str, size: f32) -> (f32, f32) {
    let text_system = gpui_kit::platform::current_platform(true).text_system();
    let mut cx = gpui_kit::TestAppContext::build_with_text_system(gpui_kit::TestDispatcher::new(0), None, text_system);
    cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(ui::Side::Light, cx);
    });
    let mut preferences = serde_json::json!({ "theme": "light", "sidebarMode": "list", "terminalSize": "app", "terminalZoom": {},
        "access": {}, "projectLook": {}, "projectOrder": [], "labs": false });
    preferences.as_object_mut().unwrap().extend(fonts.as_object().unwrap().clone());
    let data = Data { preferences: serde_json::from_value(preferences).ok(), ..Data::default() };
    let runtime = std::sync::Arc::new(tokio::runtime::Builder::new_current_thread().build().unwrap());
    let (_, cx) = cx.add_window_view(|window, cx| {
        let view = cx.new(|cx| SettingsScreen::with_data(data, At::Group(GroupId::Appearance), runtime, window, cx));
        gpui_kit::component::Root::new(view, window, cx)
    });
    cx.simulate_resize(gpui_kit::size(px(1280.), px(2400.)));
    cx.run_until_parked();
    let block = cx.debug_bounds("type-sample-code").expect("the code block");
    // `padding: 0.8rem 0.9rem`.
    let wrap = block.size.width - px(14.4 * 2.);
    let code = type_sample_code();
    let lines = cx.update(|window, _| {
        let run = TextRun {
            len: code.len(),
            font: gpui_kit::font(font.to_owned()),
            color: gpui_kit::black(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = window.text_system().shape_text(code.into(), px(size), &[run], Some(wrap), None).expect("the code shapes");
        shaped.iter().map(|line| line.wrap_boundaries().len() + 1).sum::<usize>()
    });
    (f32::from(block.size.height), lines as f32 * size * 1.375 + 12.8 * 2.)
}

/// The first of `faces` this computer has.
fn installed(faces: &[&'static str]) -> &'static str {
    let names = gpui_kit::platform::current_platform(true).text_system().all_font_names();
    faces.iter().copied().find(|f| names.iter().any(|n| n == f)).unwrap_or_else(|| panic!("none of {faces:?} is installed"))
}

fn the_code_takes_the_code_font_and_code_size_picked() {
    let mono = installed(&["Liberation Mono", "Menlo", "Courier New", "DejaVu Sans Mono"]);
    let serif = installed(&["Liberation Serif", "Times New Roman", "Times", "DejaVu Serif"]);
    let picks = |face: &str, size: u32| serde_json::json!({ "codeFont": face, "codeSize": size });
    let (narrow, narrow_shaped) = drawn_and_shaped(picks(mono, 15), mono, 15.);
    let (wide, wide_shaped) = drawn_and_shaped(picks(serif, 15), serif, 15.);
    assert!(narrow_shaped != wide_shaped, "{mono} and {serif} wrap the line apart at this width: {narrow_shaped} and {wide_shaped}");
    assert!((narrow - narrow_shaped).abs() < 2., "drawn {narrow} px, {mono} at 15 px wraps to {narrow_shaped} px");
    assert!((wide - wide_shaped).abs() < 2., "drawn {wide} px, {serif} at 15 px wraps to {wide_shaped} px");
    let (big, big_shaped) = drawn_and_shaped(picks(mono, 22), mono, 22.);
    assert!((big - big_shaped).abs() < 2., "drawn {big} px, {mono} at 22 px wraps to {big_shaped} px");
    assert!(big > narrow, "a larger Code size draws a taller block");
}

fn main() {
    the_code_takes_the_code_font_and_code_size_picked();
    println!("test the_code_takes_the_code_font_and_code_size_picked ... ok");
}
