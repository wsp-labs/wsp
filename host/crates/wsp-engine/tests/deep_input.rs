// SPDX-License-Identifier: AGPL-3.0-only
//! Deep input audit (wsp-labs/wsp-map#2448): input nested past what a walk can hold must be answered, never abort the host.
//! A stack overflow kills the whole process, so each case runs its body in a child copy of this test binary on a thread
//! with a 1 MiB stack, the size the host's threads would run it on, and fails when the child dies.
//! `DEEP_AUDIT_DEPTH` overrides a case's depth, which is how the depth each one dies at was found.

const CHILD: &str = "DEEP_AUDIT_CHILD";
const DEPTH: &str = "DEEP_AUDIT_DEPTH";
const HOST_THREAD_STACK: usize = 1024 * 1024;

/// Runs `body(depth)` in a child copy of this binary and fails when the child dies, as a stack overflow kills a host.
fn in_child(name: &str, depth: usize, body: fn(usize)) {
    let depth = std::env::var(DEPTH).ok().and_then(|d| d.parse().ok()).unwrap_or(depth);
    if std::env::var(CHILD).as_deref() == Ok(name) {
        std::thread::Builder::new()
            .stack_size(HOST_THREAD_STACK)
            .spawn(move || body(depth))
            .expect("a thread")
            .join()
            .expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().expect("this binary"))
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .env(DEPTH, depth.to_string())
        .output()
        .expect("the child starts");
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr);
        let tail: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).rev().take(3).collect();
        panic!(
            "{depth} deep: the whole process died ({}) instead of answering: {}",
            out.status,
            tail.into_iter().rev().collect::<Vec<_>>().join(" | ")
        );
    }
}

// ---- cases ----
//
// Each case: a `fn <case>_body(depth: usize)` that builds the input `depth` deep, calls the code under test and asserts
// the answer node gives for the same input (an answer, a refusal, or a failed call, which in Rust is an Err), then a
// #[test] that calls in_child with the test's exact name and a depth past where the code dies. The comment above the
// #[test] names the recursion (file:line), what reaches it from outside the host, the depth it dies at on a 1 MiB stack
// in a debug build, and what node answers at the test's depth.

#[path = "rest_common/mod.rs"]
mod common;

use wsp_engine::project_state::resolver::rewrite_cwd;

/// `depth` arrays inside one another: `[[[...]]]`.
fn arrays(depth: usize) -> String {
    format!("{}{}", "[".repeat(depth), "]".repeat(depth))
}

/// A current-thread runtime built on the calling (1 MiB) thread, so the futures run on that stack.
fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime").block_on(f)
}

// ---- project_state/resolver.rs: js_write (write_value) over a value the person's own file holds ----
//
// Recursion: `write_value` (crates/wsp-engine/src/project_state/resolver.rs:588), called by `js_write` from
// `rewrite_jsonl` (resolver.rs:245) for a line the move edited. The line is read by `JsReader` (iterative, keeps 3,500
// levels, nothing holds it lower: no line or file size cap), so a line nested up to 3,500 deep reaches `write_value`
// whole. What reaches it: a transcript line an agent harness wrote (a Claude Code `.jsonl` session file, which
// `ClaudeResolver::move_state` rewrites when the project folder moves) that has a `cwd` the move changes and a deeply
// nested field beside it. Node: JSON.parse and JSON.stringify both answer at this depth (stringify only throws past about
// 3,650 levels), so the line is rewritten and the count is 1.
// Node's answer at 3,400: the line is rewritten with its `cwd` moved, count 1.
fn rewrite_cwd_body(depth: usize) {
    let dir = tempfile::tempdir().expect("a folder");
    let file = dir.path().join("session.jsonl");
    let line = format!(r#"{{"cwd":"/work/old/app","deep":{}}}"#, arrays(depth));
    std::fs::write(&file, format!("{line}\n")).expect("the transcript");
    let changed = block_on(rewrite_cwd(file.to_str().unwrap(), "/work/old", "/work/new", None)).expect("node rewrites the line");
    assert_eq!(changed, 1);
    let want = format!(r#"{{"cwd":"/work/new/app","deep":{}}}"#, arrays(depth));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), format!("{want}\n"));
}

// Dies at 1,786 on a 1 MiB stack in a debug build.
#[test]
fn rewrite_cwd_survives_a_line_nested_3400_deep() {
    in_child("rewrite_cwd_survives_a_line_nested_3400_deep", 3_400, rewrite_cwd_body);
}

use wsp_engine::project_state::project_state_resolvers;

// ---- project_state/gemini.rs: the registry the person keeps, written back with `js_write(.., Some(2))` ----
//
// Recursion: `write_value` (resolver.rs:588) over the whole registry, called at gemini.rs:145 by `move_state`; also
// `registry.get("projects").clone()` (gemini.rs:108) and the Drop of the registry. What reaches it: Gemini CLI's own
// `projects.json` in the person's home (`~/.gemini/projects.json`), which `move_state` reads when a project folder moves;
// `JsReader` keeps 3,500 levels and no file size cap holds it lower. Node: JSON.parse and JSON.stringify(registry, null, 2)
// answer at 3,400 levels, so the move is made and the registry is written with `projects` renamed.
fn gemini_move_body(depth: usize) {
    let home = tempfile::tempdir().expect("a home");
    let file = home.path().join("projects.json");
    std::fs::write(&file, format!(r#"{{"projects":{{"/work/old/app":"slug"}},"deep":{}}}"#, arrays(depth))).unwrap();
    let resolver = project_state_resolvers().get("gemini").expect("the gemini resolver").clone();
    let moved = block_on(resolver.move_state(home.path().to_str().unwrap(), "/work/old", "/work/new")).expect("node moves the registry");
    assert_eq!(moved.len(), 1);
    assert_eq!(moved[0].changed, 1);
    assert!(std::fs::read_to_string(&file).unwrap().contains("\"/work/new/app\""));
}

// Dies at 1,781 on a 1 MiB stack in a debug build.
#[test]
fn gemini_move_state_survives_a_registry_nested_3400_deep() {
    in_child("gemini_move_state_survives_a_registry_nested_3400_deep", 3_400, gemini_move_body);
}

// ---- vault_cipher.rs: read_json with the recursion limit lifted ----
//
// Recursion: serde_json's `Value::deserialize` with `disable_recursion_limit()` (vault_cipher.rs:165) and then the Drop of
// that Value; the host's own bound is JSON_DEPTH = 2,000 (vault_cipher.rs:146), so 2,000 is the deepest header line that
// reaches it. What reaches it: the first line of a sealed image file (`open`, `sealed_image_of`, `header_of`), a file the
// person imports from another computer or a download. Node: JSON.parse reads the line at any depth, the line is not a
// header wsp wrote, so the call fails with the words below (one failed call, caught by the caller).
fn vault_header_body(depth: usize) {
    let mut sealed = arrays(depth).into_bytes();
    sealed.push(b'\n');
    sealed.extend_from_slice(&[0u8; 64]);
    let key = wsp_engine::vault_cipher::VaultKey::Private([7u8; 32]);
    let err = wsp_engine::vault_cipher::open(&sealed, &key).expect_err("a header that is not a header is refused");
    assert_eq!(err.to_string(), "this file is not a sealed wsp image: its header is not one wsp wrote");
}

// Dies at 447 on a 1 MiB stack in a debug build.
#[test]
fn vault_open_refuses_a_header_nested_2000_deep() {
    in_child("vault_open_refuses_a_header_nested_2000_deep", 2_000, vault_header_body);
}

// ---- project_state/resolver.rs: walk_files over a folder tree ----
//
// Recursion: `walk_files` (resolver.rs:691), one level per folder, reached by `files_under` and so by every resolver's
// `sessions` and `entries`. What reaches it: a folder tree under an agent's state in the person's home (a Claude Code
// `projects/<key>` folder, the Gemini `tmp/<slug>`); the only bound is the kernel's path length (4,096 bytes), which
// holds the depth near 2,000 folders. Node answers at this depth.
fn files_under_body(depth: usize) {
    let root = tempfile::tempdir().expect("a folder");
    let mut deep = root.path().to_path_buf();
    for _ in 0..depth {
        deep.push("d");
    }
    std::fs::create_dir_all(&deep).expect("a deep tree");
    std::fs::write(deep.join("s.jsonl"), "{}\n").unwrap();
    let found = wsp_engine::project_state::files_under(root.path().to_str().unwrap(), ".jsonl").expect("node walks it");
    assert_eq!(found.len(), 1);
}

// Dies at 405 on a 1 MiB stack in a debug build.
#[test]
fn files_under_walks_a_tree_1900_folders_deep() {
    in_child("files_under_walks_a_tree_1900_folders_deep", 1_900, files_under_body);
}

// ---- box_backend/request.rs: BoxView read from the provider's answer ----
//
// Recursion: three, one after the other on one answer. wsp-catalog's `parse_json` (json_parse.rs:138-197, refuses past
// 1,000 levels), `raw.get("sandbox").cloned()` (request.rs:721, `Value::clone`), and `serde_json::from_value` into
// `BoxView` (request.rs:35), whose `id` and `state` are read by `js_text` (request.rs:127), which deserializes the field
// as a `Value` and walks it with `js_string` (js_value.rs:12). What reaches it: the body of the box provider's HTTP answer
// to `GET /sandboxes/<id>`, a field that is an array nested 870 deep (the parser's own limit is 1,000). Node: `view`
// returns the parsed object as it is (JSON.parse reads any depth; nothing walks the field), so the call answers.
fn box_view_body(depth: usize) {
    use common::{reply_text, FakeFetch};
    use std::sync::Arc;
    use wsp_engine::box_backend::request::{BoxBackend, BoxBackendOptions};
    use wsp_engine::fetch::Fetch;
    let f = FakeFetch::fixed(vec![(
        "GET /api/v1/sandboxes/x",
        reply_text(200, &format!(r#"{{"sandbox":{{"id":{},"state":"running"}}}}"#, arrays(depth))),
    )]);
    let fetch: Arc<dyn Fetch> = f.clone();
    let boat = BoxBackend::new(BoxBackendOptions { api_key: "box_x".into(), fetch: Some(fetch), ..Default::default() });
    let view = block_on(boat.view("x")).expect("node answers with the object");
    assert_eq!(view.state, "running");
}

// Dies at 495 on a 1 MiB stack in a debug build.
#[test]
fn box_view_reads_an_answer_whose_id_is_nested_870_deep() {
    in_child("box_view_reads_an_answer_whose_id_is_nested_870_deep", 870, box_view_body);
}

/// `depth` arrays inside one another around a 7: `[[[7]]]`.
fn arrays_around_seven(depth: usize) -> String {
    format!("{}7{}", "[".repeat(depth - 1), "]".repeat(depth - 1))
}

// ---- box_backend: the views a provider's answer fills, at the deepest answer the JSON.parse port reads ----
//
// The port reads 1,000 levels. The answer `{"sandbox":{"id":<tree>}}` holds the tree at depth 2, so a tree of 998 levels
// is the deepest read (`name` is read as absent: it is not a string). Node: `String(id)` joins the nested arrays, so the id reads "7".
fn box_view_cap_body(depth: usize) {
    use common::{reply_text, FakeFetch};
    use std::sync::Arc;
    use wsp_engine::box_backend::request::{BoxBackend, BoxBackendOptions};
    use wsp_engine::fetch::Fetch;
    let f = FakeFetch::fixed(vec![(
        "GET /api/v1/sandboxes/x",
        reply_text(
            200,
            &format!(
                r#"{{"sandbox":{{"id":{},"name":{},"state":{}}}}}"#,
                arrays_around_seven(depth),
                arrays(depth),
                arrays_around_seven(depth)
            ),
        ),
    )]);
    let fetch: Arc<dyn Fetch> = f.clone();
    let boat = BoxBackend::new(BoxBackendOptions { api_key: "box_x".into(), fetch: Some(fetch), ..Default::default() });
    let view = block_on(boat.view("x")).expect("node answers with the object");
    assert_eq!((view.id.as_str(), view.state.as_str(), view.name), ("7", "7", None));
}

#[test]
fn box_view_reads_fields_nested_to_the_parse_cap() {
    in_child("box_view_reads_fields_nested_to_the_parse_cap", 998, box_view_cap_body);
}

// The listing of named snapshots reads each row through the same views: `name` and `status` are text, `error` and
// `createdAt` are read as absent when they are not a string.
fn named_snapshots_body(depth: usize) {
    use common::{reply_text, FakeFetch};
    use std::sync::Arc;
    use wsp_engine::box_backend::request::{BoxBackend, BoxBackendOptions};
    use wsp_engine::fetch::Fetch;
    let row =
        format!(r#"{{"name":{},"status":"ready","error":{},"createdAt":{}}}"#, arrays_around_seven(depth), arrays(depth), arrays(depth));
    let f = FakeFetch::fixed(vec![("GET /api/v1/named-snapshots", reply_text(200, &format!(r#"{{"snapshots":[{row}]}}"#)))]);
    let fetch: Arc<dyn Fetch> = f.clone();
    let boat = BoxBackend::new(BoxBackendOptions { api_key: "box_x".into(), fetch: Some(fetch), ..Default::default() });
    let rows = block_on(boat.named_snapshots()).expect("node answers with the rows");
    assert_eq!((rows[0].name.as_str(), rows[0].status.as_str(), rows[0].error.clone()), ("7", "ready", None));
}

#[test]
fn named_snapshots_read_fields_nested_to_the_parse_cap() {
    in_child("named_snapshots_read_fields_nested_to_the_parse_cap", 997, named_snapshots_body);
}

// One level past the port's cap, the answer is refused whole: that one call fails with the words the port gives for text
// it cannot read, and the host lives.
fn box_view_past_the_cap_body(depth: usize) {
    use common::{reply_text, FakeFetch};
    use std::sync::Arc;
    use wsp_engine::box_backend::request::{BoxBackend, BoxBackendOptions};
    use wsp_engine::fetch::Fetch;
    let f = FakeFetch::fixed(vec![("GET /api/v1/sandboxes/x", reply_text(200, &format!(r#"{{"sandbox":{{"id":{}}}}}"#, arrays(depth))))]);
    let fetch: Arc<dyn Fetch> = f.clone();
    let boat = BoxBackend::new(BoxBackendOptions { api_key: "box_x".into(), fetch: Some(fetch), ..Default::default() });
    let err = block_on(boat.view("x")).expect_err("past the cap the answer is refused");
    assert_eq!(err.to_string(), "Maximum call stack size exceeded");
}

#[test]
fn box_view_refuses_an_answer_nested_past_the_parse_cap() {
    in_child("box_view_refuses_an_answer_nested_past_the_parse_cap", 999, box_view_past_the_cap_body);
}
