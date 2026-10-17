// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's states as the web app's reference shots show them: the fixture's short-1 thread (claude, Sonnet 5,
//! High, Bypass, on the fixture's computer and branch) and the agents its host lists, read from harnesses.json, which
//! is that host's `harnesses.list` answer.
// Each test crate that includes this reads only some of it.
#![allow(dead_code)]

use std::sync::Arc;

use desktop_gpui::screens::composer::dock::{Pick, Prompt};
use desktop_gpui::screens::composer::model::{Agent, Drawer, File, Hold, Picks, Skill};
use desktop_gpui::screens::composer::{Banner, Bar, Composer, Popup};
use gpui_kit::{Context, Window};

pub const STATES: [&str; 28] = [
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
    "history",
    "hash-open",
    "dollar-open",
    "fast-open",
    "favourites-open",
    "pinned-picker",
    "queue-bar",
    "drawer-question",
    "drawer-limit",
    "drawer-slot",
    "drawer-tasks",
    "resume-banner",
    "paste-file",
    "prompt-dock",
];

/// This computer's name, which the web shots' host shows as its own.
pub fn computer() -> String {
    let mut name = [0u8; 256];
    // SAFETY: gethostname writes at most the buffer's length.
    let read = unsafe { libc::gethostname(name.as_mut_ptr().cast(), name.len()) };
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    if read == 0 {
        String::from_utf8_lossy(&name[..end]).into_owned()
    } else {
        String::new()
    }
}

/// The skill the web shots' fixture gives its agent.
pub fn skills() -> Vec<Skill> {
    vec![Skill {
        name: "tidy-imports".into(),
        description: Some("Sort and group a file's imports the way this repo does, and drop the unused ones.".into()),
    }]
}

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
        fast: None,
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

/// The prompt the thread waits on: run `rm -rf build`.
pub fn asked() -> Prompt {
    let pick = |id: &str, label: &str, effect: &str| Pick {
        id: id.into(),
        label: label.into(),
        description: String::new(),
        effect: effect.into(),
        mode: None,
    };
    Prompt {
        session: "s".into(),
        ask: "perm-1".into(),
        agent: Some("claude".into()),
        title: "Run a command".into(),
        note: None,
        code: Some("rm -rf build".into()),
        runs_in: None,
        fields: Vec::new(),
        file: None,
        cwd: None,
        options: vec![pick("allow", "Allow", "allow"), pick("deny", "Deny", "deny")],
        questions: None,
        asked: "Run a command: rm -rf build".into(),
    }
}

/// Puts a fresh composer into `state`.
pub fn stage(state: &str, c: &mut Composer, window: &mut Window, cx: &mut Context<Composer>) {
    let fresh = matches!(state, "banner" | "new-thread" | "fast-open" | "resume-banner");
    let mut picks = ran();
    if state == "drawer-question" {
        picks.access = Some("default".into());
    }
    c.set_catalog(agents(), if fresh { Picks::default() } else { picks }, Vec::new(), cx);
    c.set_skills(skills(), cx);
    c.set_turn(!fresh, false, cx);
    c.set_place(Some(computer()), Some("main".into()), cx);
    match state {
        "typing" => c.set_text("Look at why the transcript jumps\nwhen a long tool output lands\nand fix it.", window, cx),
        "mention-open" => {
            c.set_folder_files(FOLDER.map(String::from).to_vec(), cx);
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
                    resume: None,
                }),
                cx,
            )
        }
        "disabled" => c.set_hold(Some(Hold::HostDown), cx),
        "history" => c.set_text("Why does the host send the head twice on reconnect? (short-1, turn 10)", window, cx),
        "hash-open" => {
            c.set_text("#", window, cx);
            c.set_host_items(Vec::new(), None, cx);
        }
        "dollar-open" => c.set_text("$", window, cx),
        "fast-open" => c.toggle_popup(Popup::Reasoning, cx),
        "favourites-open" => {
            c.toggle_favourite("claude:claude-opus-5-5".into(), cx);
            c.toggle_popup(Popup::Model, cx);
            c.show_favourites(cx);
        }
        "pinned-picker" => {
            c.toggle_popup(Popup::Model, cx);
            c.set_rail("codex".into(), cx);
        }
        "queue-bar" => {
            c.set_turn(true, true, cx);
            c.set_text("Then check the scroll anchor too.", window, cx);
            c.submit(window, cx);
            c.open_bar(Bar::Queue, cx);
        }
        "drawer-question" | "drawer-slot" | "drawer-tasks" => {
            c.set_turn(true, true, cx);
            let drawer = match state {
                "drawer-question" => {
                    c.set_prompt(Some(asked()), window, cx);
                    c.fold_prompt(window, cx);
                    Drawer::default()
                }
                "drawer-slot" => Drawer { slot: Some(format!("{} is running 3 of 3 threads", computer())), ..Drawer::default() },
                _ => Drawer { tasks: Some(("Pin the scroll anchor before tool output mounts".into(), "1/4".into())), ..Drawer::default() },
            };
            c.set_drawer(drawer, cx);
        }
        "prompt-dock" => {
            c.set_turn(true, true, cx);
            c.set_prompt(Some(asked()), window, cx);
        }
        "drawer-limit" => c.set_drawer(
            Drawer { limit: Some(("Usage limit reached".into(), "Claude Code resumes at 02:00".into())), ..Drawer::default() },
            cx,
        ),
        "resume-banner" => c.set_banner(
            Some(Banner::resume("5f0c9a52-7b1e-4d0a-9c3e-2a6b8d4e1f07".into(), "Fix the flaky scroll test", false, "Claude Code")),
            cx,
        ),
        "paste-file" => {
            let text = (1..=250).map(|n| format!("line {n} of the pasted log, as a terminal printed it\n")).collect::<String>();
            let file = desktop_gpui::screens::composer::model::pasted_file(&text[..11892.min(text.len())], &[]);
            let mut file =
                file.unwrap_or(File { name: "pasted-text-1.txt".into(), media_type: "text/plain".into(), bytes: Arc::new(Vec::new()) });
            file.bytes = Arc::new(vec![b'x'; 11892]);
            c.attach(vec![file], cx);
        }
        _ => {}
    }
}
