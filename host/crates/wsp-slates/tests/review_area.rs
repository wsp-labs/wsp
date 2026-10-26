// SPDX-License-Identifier: AGPL-3.0-only
//! Review findings in the slate area, each pinned to the TS at 65f6b89c7.
#![allow(unused_imports, dead_code)]
mod support;

use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{temp_root, FakeHost, MemStore, THREAD};
use wsp_slates::slates::{SlateTarget, Slates, StateParams, WriteParams};
use wsp_wire::views::workspace::Caller;

struct Rig {
    root: tempfile::TempDir,
    host: Rc<FakeHost>,
    slates: Rc<dyn Slates>,
}

fn rig(prefix: &str) -> Rig {
    support::own_group();
    let root = temp_root(prefix);
    let host = FakeHost::new(root.path(), MemStore::new());
    let slates = host.slates();
    Rig { root, host, slates }
}

fn agent() -> Caller {
    support::as_thread(THREAD, THREAD)
}

fn target() -> SlateTarget {
    SlateTarget { thread_id: Some(THREAD.into()), turn_token: None }
}

// slate-runs.ts:557-563 save() calls writeOwn, which throws when the state folder cannot be written, and set() calls it
// for a kept secret (:588), so slates.ts:1212's state() rejects and the person's typing fails where they can see it. The
// port's Secrets::save drops the error, so the typing reads as kept and the secret is gone after a restart.
#[test]
fn a_kept_secret_the_host_cannot_save_refuses_the_typing() {
    support::local(async {
        let rig = rig("wsp-review-secret-save-");
        let s = &rig.slates;
        // A folder where the secrets file goes: the rename fails as a read-only or full state folder would.
        std::fs::create_dir_all(rig.root.path().join("state/slates.secrets.json/held")).unwrap();
        s.write(
            WriteParams {
                text: Some(r#"<slate><secret name="token" keep /><column><input id="tok" label="Token" value={$token} kind="password" /></column></slate>"#.into()),
                ..Default::default()
            },
            Some(&agent()),
        )
        .await
        .expect("the agent's write");
        let typed = s
            .state(
                StateParams {
                    target: target(),
                    values: Some(IndexMap::from([("$token".to_string(), json!("hunter2"))])),
                    ..Default::default()
                },
                None,
            )
            .await;
        assert!(typed.is_err(), "the TS rejects the typing when the kept secret cannot be saved: {:?}", typed.map(|a| a.version));
    });
}
