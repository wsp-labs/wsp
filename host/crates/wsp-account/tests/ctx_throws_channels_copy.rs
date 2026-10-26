// SPDX-License-Identifier: AGPL-3.0-only
//! The context's `threadFolder` throws through a copy channel's `pty.create` that names no folder (channels.ts:812).
//! The throw ends the send before any daemon `pty.create` goes out.
#![allow(dead_code, unused_imports)]
mod common;
include!("channels_copy/world.rs");

// channels.ts:812: `asked.cwd ?? (await ctx.threadFolder(entry, {}))` rejects, and the create is never sent.
#[test]
fn ctx_throws_thread_folder_ends_a_pty_create() {
    let mut world = scenarios()["c12 cwd is the frame's or the thread's folder"]["world"].clone();
    world["threadFolderError"] = json!("the thread's folder is not on this computer");
    let script = vec![json!({ "step": "open", "as": "a" }), json!({ "step": "send", "on": "a", "frame": { "op": "pty.create" } })];
    let (steps, fx) = run_one(world, script, Mode::Copy);
    assert_eq!(steps, json!([{ "open": "ok" }, { "op": "pty.create", "thrown": "the thread's folder is not on this computer" }]));
    assert!(!fx.to_string().contains("pty.create"), "no daemon pty.create after the throw: {fx}");
}
