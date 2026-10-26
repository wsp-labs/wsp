// SPDX-License-Identifier: AGPL-3.0-only
//! `--door` keeps the tapes that went through one door: each door's count is what the tapes hold.

use serde_json::Value;
use std::path::Path;
use xtask::check::Options;
use xtask::tape::{self, Door, Tape};

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}

fn tape(id: &str, meta: &str) -> Tape {
    let text = format!("{meta}{{\"in\":{{\"id\":1,\"op\":\"init.get\"}}}}\n");
    Tape::parse(id, &text, id, 1).unwrap()
}

fn on(door: Option<Door>) -> Options {
    Options { door, ..Options::default() }
}

#[test]
fn a_door_keeps_only_its_own_tapes() {
    let ws = tape("a", "");
    let mcp = tape("b", "{\"meta\":{\"door\":\"mcp\"}}\n");
    let cli = tape("c", "{\"meta\":{\"door\":\"cli\"}}\n");
    for (door, want) in [(Door::Ws, [true, false, false]), (Door::Mcp, [false, true, false]), (Door::Cli, [false, false, true])] {
        let got = [&ws, &mcp, &cli].map(|t| on(Some(door)).keeps(t));
        assert_eq!(got, want, "{}", door.word());
    }
    assert!([&ws, &mcp, &cli].iter().all(|t| on(None).keeps(t)), "with no door every tape runs");
}

#[test]
fn a_door_and_a_text_both_hold() {
    let opts = Options { door: Some(Door::Mcp), only: Some("tools".into()), ..Options::default() };
    assert!(opts.keeps(&tape("hand/tools", "{\"meta\":{\"door\":\"mcp\"}}\n")));
    assert!(!opts.keeps(&tape("hand/other", "{\"meta\":{\"door\":\"mcp\"}}\n")));
    assert!(!opts.keeps(&tape("hand/tools", "")));
}

/// The door a tape file's own meta line names, read without the tape parser: a file with none went through the websocket.
fn door_in_file(path: &Path) -> &'static str {
    let text = std::fs::read_to_string(path).unwrap();
    let first: Value = serde_json::from_str(text.lines().next().unwrap()).unwrap();
    match first.pointer("/meta/door").and_then(Value::as_str) {
        Some("mcp") => "mcp",
        Some("cli") => "cli",
        _ => "ws",
    }
}

fn files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            files(&path, out);
        } else if path.extension().is_some_and(|e| e == "tape") {
            out.push(path);
        }
    }
}

#[test]
fn each_door_s_count_is_what_the_tapes_hold() {
    let mut paths = Vec::new();
    files(&root().join(tape::TAPES_DIR), &mut paths);
    let tapes = tape::load(root()).unwrap();
    for door in [Door::Ws, Door::Mcp, Door::Cli] {
        let held = paths.iter().filter(|p| door_in_file(p) == door.word()).count();
        let kept = tapes.iter().filter(|t| !t.id.starts_with("spec/") && on(Some(door)).keeps(t)).count();
        assert!(held > 0, "the repository holds no {} tape", door.word());
        assert_eq!(kept, held, "the {} door", door.word());
    }
    let all: usize = [Door::Ws, Door::Mcp, Door::Cli].iter().map(|d| tapes.iter().filter(|t| on(Some(*d)).keeps(t)).count()).sum();
    assert_eq!(all, tapes.len(), "every tape goes through exactly one door");
}
