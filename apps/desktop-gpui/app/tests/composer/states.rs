// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's states as the web app's reference shots show them: the fixture's short-1 thread (claude, Sonnet 5,
//! High, Bypass, on the fixture's computer and branch) and the agents its host lists, read from harnesses.json, which
//! is that host's `harnesses.list` answer.
// Each test crate that includes this reads only some of it.
#![allow(dead_code)]

use std::sync::Arc;

use desktop_gpui::screens::composer::model::{Agent, File, Hold, Picks};
use desktop_gpui::screens::composer::{Banner, Composer, Popup};
use gpui_kit::{Context, Window};

pub const STATES: [&str; 14] = [
    "empty",
    "typing",
    "mention-open",
    "command-open",
    "model-picker-open",
    "effort-picker-open",
    "access-picker-open",
    "attachments",
    "stash-open",
    "running",
    "queued",
    "banner",
    "new-thread",
    "disabled",
];

pub fn agents() -> Vec<Agent> {
    let listed: Vec<serde_json::Value> = serde_json::from_str(include_str!("harnesses.json")).unwrap();
    listed.iter().map(|a| Agent::read(a).expect("a catalog entry reads")).collect()
}

/// short-1's agent and the choices it ran with.
pub fn ran() -> Picks {
    Picks {
        agent: Some("claude".into()),
        model: Some("claude-sonnet-5".into()),
        effort: Some("high".into()),
        window: None,
        access: Some("bypassPermissions".into()),
    }
}

/// The 24 px swatch and the note the web shots attach.
pub fn files() -> Vec<File> {
    let mut png = Vec::new();
    let swatch = image::RgbaImage::from_pixel(24, 24, image::Rgba([217, 119, 87, 255]));
    image::DynamicImage::ImageRgba8(swatch).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    vec![
        File { name: "swatch.png".into(), media_type: "image/png".into(), bytes: Arc::new(png) },
        File {
            name: "notes.txt".into(),
            media_type: "text/plain".into(),
            bytes: Arc::new(b"The transcript jumps when a long tool output lands.\n".to_vec()),
        },
    ]
}

/// The files the web shots' project holds, as fs.files lists them.
pub const FOLDER: [&str; 6] =
    ["README.md", "package.json", "src/scroll-anchor.ts", "src/tool-output.tsx", "src/transcript.ts", "test/transcript.test.ts"];

/// Puts a fresh composer into `state`.
pub fn stage(state: &str, c: &mut Composer, window: &mut Window, cx: &mut Context<Composer>) {
    let fresh = matches!(state, "banner" | "new-thread");
    c.set_catalog(agents(), if fresh { Picks::default() } else { ran() }, Vec::new(), cx);
    c.set_turn(!fresh, false, cx);
    c.set_place(Some("build-the-composer-and-pickers-2395".into()), Some("main".into()), cx);
    match state {
        "typing" => c.set_text("Look at why the transcript jumps\nwhen a long tool output lands\nand fix it.", window, cx),
        "mention-open" => {
            c.set_folder_files(FOLDER.map(String::from).to_vec(), window, cx);
            c.set_text("@", window, cx);
        }
        "command-open" => c.set_text("/", window, cx),
        "model-picker-open" => c.toggle_popup(Popup::Model, cx),
        "effort-picker-open" => c.toggle_popup(Popup::Reasoning, cx),
        "access-picker-open" => c.toggle_popup(Popup::Access, cx),
        "attachments" => c.attach(files(), cx),
        "stash-open" => {
            c.set_text("Try pinning the scroll anchor before the tool output mounts.", window, cx);
            c.stash_draft(window, cx);
            c.toggle_popup(Popup::Stash, cx);
        }
        "running" => c.set_turn(true, true, cx),
        "queued" => {
            c.set_turn(true, true, cx);
            c.set_text("Then check the scroll anchor too.", window, cx);
            c.submit(window, cx);
        }
        "banner" => {
            c.set_picks(Picks { model: Some("claude-sonnet-5".into()), ..Picks::default() }, cx);
            c.set_banner(
                Some(Banner {
                    fork: true,
                    line: "Fork of Look at why the transcript jumps when a long…".into(),
                    note: "in this folder".into(),
                }),
                cx,
            )
        }
        "disabled" => c.set_hold(Some(Hold::HostDown), cx),
        _ => {}
    }
}
