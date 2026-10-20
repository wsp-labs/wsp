// SPDX-License-Identifier: AGPL-3.0-only

use std::sync::Arc;

use gpui_kit::{Image, ImageFormat};

use super::*;
use crate::transcript::FileTile;

fn attachments() -> Vec<SessionEvent> {
    vec![base(
        "session.start",
        1,
        json!({"prompt": "look", "requestId": "q1", "attachments": [
            {"mediaType": "image/png", "bytes": 1300, "name": "shot.png"},
            {"mediaType": "text/plain", "bytes": 30, "name": "notes.txt"}
        ]}),
    )]
}

/// A 2 x 1 PNG, whole.
const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAIAAAABCAIAAAB7QOjdAAAAEElEQVR4nGP4z8DAwMDAAAAJ/QH/ZoWm8QAAAABJRU5ErkJggg==";

#[gpui_kit::test]
fn a_messages_files_draw_as_tiles_and_its_unheld_image_as_its_line(cx: &mut TestAppContext) {
    let (view, cx) = window(attachments(), cx);
    let tile = bounds(cx, "file-tile");
    assert_eq!(tile.size.height, px(56.));
    assert!(tile.size.width <= px(192.));
    // With no host to ask, the image is not held: its line, 2 px under the tiles' `mb-2`.
    let lines = bounds(cx, "file-lines");
    assert_eq!((lines.size.height, lines.top() - tile.bottom()), (px(16.), px(8.)));
    assert!(cx.debug_bounds("thumb").is_none());
    let key = view.read_with(cx, |v, _| v.rows()[0].key.clone());
    let bytes = unbase64(PNG).unwrap();
    assert_eq!(image_size(&bytes), Some((2., 1.)));
    view.update(cx, |v, cx| {
        let held = Shot::Held(Arc::new(Image::from_bytes(ImageFormat::Png, bytes)), Some((2., 1.)));
        v.images.insert(key, vec![held, Shot::File]);
        cx.notify();
    });
    cx.run_until_parked();
    let thumb = bounds(cx, "thumb");
    let tile = bounds(cx, "file-tile");
    assert_eq!((thumb.size, tile.left() - thumb.right()), (size(px(56.), px(56.)), px(6.)));
    assert!(cx.debug_bounds("file-lines").is_none(), "a held image needs no line");
    // The layer times its fade out on the wall clock.
    let shut = |cx: &mut VisualTestContext| {
        std::thread::sleep(std::time::Duration::from_millis(250));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
        !cx.update(|_, cx| crate::screens::dialogs::layer::is_open(super::IMAGE_PREVIEW, cx)) && cx.debug_bounds("image-dialog").is_none()
    };
    cx.simulate_click(thumb.center(), Modifiers::none());
    cx.run_until_parked();
    assert_eq!(cx.update(|_, cx| crate::screens::dialogs::layer::top(cx)), Some(super::IMAGE_PREVIEW), "the image opened off the layer");
    assert!(cx.debug_bounds("image-dialog").is_some() && cx.debug_bounds("dialogs-layer").is_some(), "a press opens the image whole");
    for _ in 0..3 {
        cx.executor().advance_clock(std::time::Duration::from_millis(16));
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    cx.simulate_keystrokes("escape");
    assert!(shut(cx), "Escape left the image open");
    cx.simulate_click(thumb.center(), Modifiers::none());
    cx.run_until_parked();
    cx.update(|window, cx| window.simulate_next_frame(cx));
    cx.simulate_click(gpui_kit::point(px(4.), px(4.)), Modifiers::none());
    assert!(shut(cx), "a press outside left the image open");
}

#[gpui_kit::test]
fn an_image_lets_go_of_its_pixels_once_its_row_leaves_the_kept_rows(cx: &mut TestAppContext) {
    let mut events = attachments();
    events.push(base("session.done", 2, json!({"result": {"status": "completed"}})));
    events.push(base("session.end", 3, json!({"exitCode": 0, "sawResult": true})));
    for n in 0..60u64 {
        let (turn, pos) = (format!("t{n}"), 10 + 4 * n);
        events.push(base("session.start", pos, json!({"prompt": format!("Question {n}"), "turnId": turn})));
        events.push(base("session.delta", pos + 1, json!({"kind": "text", "text": format!("Answer {n}."), "turnId": turn})));
        events.push(base("session.done", pos + 2, json!({"result": {"status": "completed"}, "turnId": turn})));
        events.push(base("session.end", pos + 3, json!({"exitCode": 0, "sawResult": true, "turnId": turn})));
    }
    let (view, cx) = window(events, cx);
    let key = view.read_with(cx, |v, _| v.rows()[0].key.clone());
    let image = Arc::new(Image::from_bytes(ImageFormat::Png, unbase64(PNG).unwrap()));
    view.update(cx, |v, cx| {
        v.images.insert(key.clone(), vec![Shot::Held(image.clone(), Some((2., 1.))), Shot::File]);
        cx.notify();
    });
    cx.run_until_parked();
    assert!(cx.debug_bounds("thumb").is_some(), "the image was not drawn");
    assert!(cx.update(|_, cx| image.is_asset_cached(cx)), "the drawn image was not decoded");
    view.update(cx, |v, cx| v.scroll_to_end(cx));
    for _ in 0..3 {
        cx.update(|window, cx| window.simulate_next_frame(cx));
        cx.run_until_parked();
    }
    assert!(view.read_with(cx, |v, _| !v.images.contains_key(&key)), "the row's attachments outlived its kept rows");
    assert!(!cx.update(|_, cx| image.is_asset_cached(cx)), "the image's decoded pixels outlived its row");
}

#[test]
fn an_unheld_attachment_reads_as_the_command_line_prints_it() {
    let tile = |mime: &str, bytes| FileTile {
        name: "a.txt".into(),
        bytes: Some(bytes),
        image: mime.starts_with("image/"),
        id: None,
        index: 0,
        mime: Some(mime.into()),
    };
    assert_eq!(attachment_line(&tile("image/png", 1_258_291)), "[image 1 MB png]");
    assert_eq!(attachment_line(&tile("image/jpeg", 900)), "[image 900 B jpeg]");
    assert_eq!(attachment_line(&tile("text/plain", 2_048)), "[file 2 KB a.txt]");
    assert_eq!(unbase64("aGk="), Some(b"hi".to_vec()));
    assert_eq!(unbase64("aGk"), Some(b"hi".to_vec()));
    assert_eq!(unbase64("a$=="), None);
}
