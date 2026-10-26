// SPDX-License-Identifier: AGPL-3.0-only
//! The forms wsp-account shares with the runtime's context (create_runtime packet 0b): the error a refusal keeps its code
//! in when a host error is asked for, the viewed marks as the TS record per workspace (transcripts.ts:615,
//! workspaces.ts:881-891), and the runtime's state read where the TS keeps it (context.ts:156-166).
#![allow(dead_code, unused_imports)]
#[path = "support/base.rs"]
mod base;
#[path = "support/git.rs"]
mod git;
include!("workspaces_git/world.rs");

use indexmap::IndexMap;
use wsp_account::account::channels::ChannelsCtx;
use wsp_account::account::core_types::AccountError;
use wsp_account::projects::folders::FoldersCtx;
use wsp_core::context::{RuntimeState, StateCell};
use wsp_core::types::internal::DaemonRefusal;

#[test]
fn a_refusal_a_daemon_coded_keeps_its_code_when_a_host_error_is_asked_for() {
    let refused = AccountError::Refusal(DaemonRefusal { code: Some("no-host-cli".into()), message: "no host cli here".into() });
    let host: HostError = refused.into();
    assert_eq!(host.message, "no host cli here");
    assert_eq!(host.code.as_deref(), Some("no-host-cli"));
    assert_eq!(host.kind, None);

    let uncoded: HostError = AccountError::Refusal(DaemonRefusal { code: None, message: "nope".into() }).into();
    assert_eq!((uncoded.message.as_str(), uncoded.code), ("nope", None));
}

#[test]
fn a_host_error_goes_through_an_account_error_unchanged() {
    let said = HostError::with_kind("no such thing", Kind::NotFound);
    let back: HostError = AccountError::from(said.clone()).into();
    assert_eq!(back, said);
}

#[test]
fn viewed_marks_are_kept_as_the_ts_record_per_workspace_and_a_mark_keeps_its_place() {
    run(async {
        let w = world(json!({}));
        let mark = |path: &str, blob: Option<&str>| Some(ViewedMark { path: path.into(), blob: blob.map(str::to_owned) });
        for (path, blob) in [("a.ts", Some("b1")), ("b.ts", Some("b2")), ("c.ts", Some("b3")), ("a.ts", Some("b9")), ("b.ts", None)] {
            area::viewed(&w.ctx, "ws1", mark(path, blob), origin()).await.expect("a mark lands");
        }
        let held: IndexMap<String, String> = w.fake.viewed.borrow().get("ws1").cloned().expect("held");
        assert_eq!(held.into_iter().collect::<Vec<_>>(), vec![("a.ts".to_string(), "b9".to_string()), ("c.ts".to_string(), "b3".to_string())]);
    })
}

#[allow(dead_code)]
fn the_contexts_hand_the_viewed_marks_and_the_state_the_runtime_holds(git: &dyn GitCtx, channels: &dyn ChannelsCtx, folders: &dyn FoldersCtx) {
    let _: std::rc::Rc<std::cell::RefCell<IndexMap<String, IndexMap<String, String>>>> = git.viewed_marks();
    let _: StateCell<RuntimeState> = channels.state();
    let _: StateCell<RuntimeState> = folders.state();
}
