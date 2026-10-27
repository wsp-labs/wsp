// SPDX-License-Identifier: AGPL-3.0-only
//! Ruling 3 of round 2 of #2448: every tree the engine reads from outside is at most 500 levels deep, and a tree of nested
//! objects (a clone of one dies at 628 levels on a 1 MiB debug stack) at that depth is read, cloned where the code clones
//! it and dropped without the host's stack giving out. Each case runs in a child copy of this binary on a 1 MiB thread.
#[path = "rest_common/mod.rs"]
mod common;

use wsp_engine::project_state::project_state_resolvers;
use wsp_engine::project_state::resolver::rewrite_cwd;

const CHILD: &str = "CLONE_AT_CAP_CHILD";

fn in_child(name: &str, body: fn()) {
    if std::env::var(CHILD).as_deref() == Ok(name) {
        std::thread::Builder::new().stack_size(1024 * 1024).spawn(body).unwrap().join().expect("the body held");
        return;
    }
    let out = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", name, "--nocapture", "--test-threads=1"])
        .env(CHILD, name)
        .output()
        .unwrap();
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "the whole process died ({}): {}",
        out.status,
        stderr.lines().rev().take(2).collect::<Vec<_>>().join(" | ")
    );
}

fn block_on<F: std::future::Future>(f: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread().enable_all().build().expect("a runtime").block_on(f)
}

/// `{"a":{"a":...1}}`, `n` objects deep.
fn objects(n: usize) -> String {
    format!("{}1{}", "{\"a\":".repeat(n), "}".repeat(n))
}

fn rewrite_cwd_body() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("session.jsonl");
    std::fs::write(&file, format!("{{\"cwd\":\"/work/old/app\",\"deep\":{}}}\n", objects(499))).unwrap();
    assert_eq!(block_on(rewrite_cwd(file.to_str().unwrap(), "/work/old", "/work/new", None)).expect("a line at the cap is rewritten"), 1);
    assert!(std::fs::read_to_string(&file).unwrap().contains("/work/new/app"));
}

#[test]
fn a_session_line_of_500_object_levels_is_rewritten() {
    in_child("a_session_line_of_500_object_levels_is_rewritten", rewrite_cwd_body);
}

fn gemini_body() {
    let home = tempfile::tempdir().unwrap();
    let file = home.path().join("projects.json");
    std::fs::write(&file, format!(r#"{{"projects":{{"/work/old/app":"slug"}},"deep":{}}}"#, objects(499))).unwrap();
    let resolver = project_state_resolvers().get("gemini").expect("the gemini resolver").clone();
    let moved =
        block_on(resolver.move_state(home.path().to_str().unwrap(), "/work/old", "/work/new")).expect("a registry at the cap is moved");
    assert_eq!(moved[0].changed, 1);
}

#[test]
fn a_gemini_registry_of_500_object_levels_is_moved() {
    in_child("a_gemini_registry_of_500_object_levels_is_moved", gemini_body);
}

fn vault_body() {
    let hash = "a".repeat(64);
    let mut sealed = format!(
        r#"{{"format":"wsp-vault-1","cipher":"aes-256-gcm","to":"key","salt":"AA==","nonce":"AA==","ephemeral":"AA==","image":{{"name":"n","version":1,"hash":"{hash}","recipeHash":"r","logins":[],"sealedAt":"t","sealedFrom":"f"}},"extra":{}}}"#,
        objects(498)
    )
    .into_bytes();
    sealed.push(b'\n');
    assert_eq!(wsp_engine::vault_cipher::sealed_image_of(&sealed).expect("a header at the cap is read").name, "n");
}

#[test]
fn a_sealed_header_of_500_object_levels_is_read() {
    in_child("a_sealed_header_of_500_object_levels_is_read", vault_body);
}

fn box_view_body() {
    use common::{reply_text, FakeFetch};
    use std::sync::Arc;
    use wsp_engine::box_backend::request::{BoxBackend, BoxBackendOptions};
    use wsp_engine::fetch::Fetch;
    let f = FakeFetch::fixed(vec![(
        "GET /api/v1/sandboxes/x",
        reply_text(200, &format!(r#"{{"sandbox":{{"id":"x","state":"running","extra":{},"name":{}}}}}"#, objects(498), objects(498))),
    )]);
    let fetch: Arc<dyn Fetch> = f.clone();
    let boat = BoxBackend::new(BoxBackendOptions { api_key: "box_x".into(), fetch: Some(fetch), ..Default::default() });
    assert_eq!(block_on(boat.view("x")).expect("an answer at the cap").state, "running");
}

#[test]
fn a_box_answer_of_500_object_levels_is_read() {
    in_child("a_box_answer_of_500_object_levels_is_read", box_view_body);
}

fn solari_body() {
    use common::{reply_text, FakeFetch};
    use std::sync::Arc;
    use wsp_engine::fetch::Fetch;
    use wsp_engine::machine::MachineBackend;
    use wsp_engine::solari_backend::{SolariBackend, SolariBackendOptions};
    let row = format!(r#"{{"templateId":"t","name":"n","status":{},"createdAt":"c"}}"#, objects(497));
    let f = FakeFetch::fixed(vec![("GET /templates", reply_text(200, &format!(r#"{{"templates":[{row}]}}"#)))]);
    let fetch: Arc<dyn Fetch> = f.clone();
    let solari = SolariBackend::new(SolariBackendOptions { api_key: "k".into(), fetch: Some(fetch), ..Default::default() });
    assert_eq!(block_on(solari.list_templates()).expect("an answer at the cap").len(), 1);
}

#[test]
fn a_provider_answer_of_500_object_levels_is_read() {
    in_child("a_provider_answer_of_500_object_levels_is_read", solari_body);
}
