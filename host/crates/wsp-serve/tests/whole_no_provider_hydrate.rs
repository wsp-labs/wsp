// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/no-provider-hydrate.test.ts at 65f6b89c7, case for case. A host started without its provider key
//! serves. The records it holds are a provider's, and nothing can be asked about their machines until the key is back, so
//! each is left at the word it was left with and every road on one answers the sentence that names what is missing.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::{json, Value};
use wsp_core::types::wiring::{LocalWiring, Platform};
use wsp_engine::copier::fake_copier;
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
use wsp_engine::no_provider_backend::NoProviderBackend;
use wsp_store::keys::new_place_key_pair;
use wsp_words::words::thread::NO_PROVIDER_LINE;

/// A state a host with a provider wrote: one running cloud workspace, read again by a host with no key.
async fn keyless() -> (State, String) {
    let state = sqlite_state();
    let first = runtime(&StubBackend::new(None)).store(&state.store).build();
    let ws = create_on(&first.rt, CreateOn::golden("first"), None).await.expect("a workspace").view.id;
    first.rt.close().await.expect("a close");
    (state, ws)
}

fn none() -> Arc<dyn wsp_engine::machine::MachineBackend> {
    Arc::new(NoProviderBackend::new(None))
}

// "a host started without its provider" > "serves, keeps every cloud record listed at the word it was left with, and answers the provider's sentence on each"
#[test]
fn serves_keeps_every_cloud_record_listed_at_the_word_it_was_left_with_and_answers_the_providers_sentence_on_each() {
    drive(|| async {
        let (state, ws) = keyless().await;
        let fx = over(none()).store(&state.store).build();
        // The lists and the meter read the records this host holds, so a record dropped here is one nobody can see.
        let listed: Vec<Value> = fx.rt.workspaces().list(None).await.expect("a list").iter().map(j).collect();
        assert_matches(&json!(listed), json!([{ "id": ws, "name": "first", "phase": "running", "kind": "cloud" }]));
        assert_matches(&j(&fx.rt.workspaces().resolve("first", None).await.expect("a workspace")), json!({ "id": ws }));
        // Every road on that record says the one thing that is true about it: the key is what is missing.
        let refused = fx.rt.workspaces().nap(&ws, None).await.expect_err("a refusal");
        assert!(refused.message.contains(NO_PROVIDER_LINE), "{}", refused.message);
        // The record is left exactly as it was: nothing is called gone for want of a key.
        assert_matches(
            &state.store.get("workspaces", &ws).await.expect("a read").expect("a record"),
            json!({ "id": ws, "phase": "running" }),
        );
        fx.rt.close().await.expect("a close");
    });
}

// "a host started without its provider" > "reads a project of a folder with no origin, which it fills at every start, without waiting on its own start"
#[test]
fn reads_a_project_of_a_folder_with_no_origin_which_it_fills_at_every_start_without_waiting_on_its_own_start() {
    drive(|| async {
        let root = tempfile::tempdir().expect("a folder");
        let root_path = std::fs::canonicalize(root.path()).expect("a real path");
        let folder = root_path.join("work").join("site");
        std::fs::create_dir_all(&folder).expect("the folder");
        let state = sqlite_state();
        state
            .store
            .put(
                "projects",
                "pr_1",
                json!({ "id": "pr_1", "name": "site", "computer": "here", "source": { "kind": "folder", "path": folder }, "path": folder, "remote": "", "defaultBranch": "", "createdAt": "2026-09-01T00:00:00.000Z" }),
            )
            .await
            .expect("a put");
        let dir = root_path.display().to_string();
        let (home, exec_root, runs) = (dir.clone(), dir.clone(), format!("{dir}/runs"));
        let local = LocalWiring {
            backend: Arc::new(LocalBackend::new(LocalBackendOptions { root: root_path.clone(), env: None })),
            folder: dir.clone(),
            exec_stream: Rc::new(move |opts, waiting| {
                let mut local = wsp_core::local_exec::LocalExecOptions::new(exec_root.clone(), runs.clone());
                if let Some(opts) = opts {
                    local.idle_ms = opts.idle_ms;
                    local.deadline_ms = opts.deadline_ms;
                    local.now = opts.now;
                    local.poll_ms = opts.poll_ms;
                    local.reading = opts.reading;
                }
                wsp_core::local_exec::local_exec_stream(local, waiting)
            }),
            home: Rc::new(move |_| format!("{home}/.claude")),
            home_dir: dir.clone(),
            roots_path: format!("{dir}/roots"),
            env: Rc::new(|| {
                indexmap::IndexMap::from([("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()))])
            }),
            daemon_road: None,
            here_daemon: None,
            restart_daemon: None,
            sys_samples: None,
            copier: Some(Rc::new(fake_copier(None, None))),
            platform: if cfg!(target_os = "macos") { Platform::Darwin } else { Platform::Linux },
            close: None,
        };
        let fx = over(none())
            .store(&state.store)
            .with(move |o| {
                o.local = Some(local);
                o.place_links = Some(place_wiring(new_place_key_pair()));
            })
            .build();
        let rt = fx.rt.clone();
        let read =
            tokio::time::timeout(std::time::Duration::from_millis(2_000), async move { rt.workspaces().list(None).await.map(|_| ()) })
                .await;
        assert!(matches!(read, Ok(Ok(()))), "still waiting");
        fx.rt.close().await.expect("a close");
    });
}

// "a host started without its provider" > "reads a builder another host left as it is, rather than failing the whole start on it"
#[test]
fn reads_a_builder_another_host_left_as_it_is_rather_than_failing_the_whole_start_on_it() {
    drive(|| async {
        let (state, _) = keyless().await;
        state
            .store
            .put("builders", "m_b1", json!({ "id": "m_b1", "kind": "sandbox", "createdAt": jiff::Timestamp::now().to_string(), "firstLife": true, "baseTemplate": "t", "setupSha": "s" }))
            .await
            .expect("a put");
        let fx = over(none()).store(&state.store).build();
        fx.rt.workspaces().list(None).await.expect("a list");
        // Still there for the host that has the key: a record nothing could be asked about is never dropped.
        assert_matches(&state.store.get("builders", "m_b1").await.expect("a read").expect("a builder"), json!({ "id": "m_b1" }));
        fx.rt.close().await.expect("a close");
    });
}
