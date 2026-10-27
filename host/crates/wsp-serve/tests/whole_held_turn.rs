// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/held-turn.test.ts at 65f6b89c7, case for case. A turn whose agent replied while a command it
//! started is still running: the runtime holds the turn's idle clock open for that work the way it holds it open under a
//! question nobody has answered, since both say the turn is waiting on something its own process is not doing. The box
//! the turn's exec stream reads is the one fact, and this is what moves it.
#[macro_use]
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use common::*;
use serde_json::{json, Value};
use wsp_core::machine_exec::TurnWaiting;
use wsp_core::types::api::StartOptions;
use wsp_core::types::wiring::{LocalWiring, Platform};
use wsp_engine::copier::fake_copier;
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions};
use wsp_wire::types::place_word::HERE_PLACE_ID;

const ASK_ID: &str = "ask_1";

fn ask() -> Value {
    json!({
        "askId": ASK_ID,
        "toolName": "Bash",
        "toolUseId": "toolu_1",
        "input": "{\"command\":\"pnpm test\"}",
        "options": [
            { "id": "allow", "label": "Allow", "effect": "allow" },
            { "id": "deny", "label": "Deny", "effect": "deny" },
        ],
    })
}

struct Held {
    fx: Fx,
    driven: Rc<Driven>,
    /// The box the runtime handed this turn's exec factory, read as the stream's every poll reads it.
    waiting: Rc<RefCell<Option<TurnWaiting>>>,
    _root: tempfile::TempDir,
}

impl Held {
    fn waiting(&self) -> Option<bool> {
        self.waiting.borrow().as_ref().map(|w| w())
    }
}

/// beforeEach: a local backend over a folder, an exec stream that keeps the box it is handed, a driven adapter.
fn start() -> Held {
    let dir = tempfile::tempdir().expect("a folder");
    let root = std::fs::canonicalize(dir.path()).expect("a real path");
    let folder = root.display().to_string();
    let waiting: Rc<RefCell<Option<TurnWaiting>>> = Rc::default();
    let kept = waiting.clone();
    let (home, runs, exec_root) = (folder.clone(), format!("{folder}/runs"), folder.clone());
    let local = LocalWiring {
        backend: Arc::new(LocalBackend::new(LocalBackendOptions { root: root.clone(), env: None })),
        folder: folder.clone(),
        exec_stream: Rc::new(move |opts, is_waiting| {
            if let Some(w) = is_waiting.clone() {
                *kept.borrow_mut() = Some(w);
            }
            let mut local = wsp_core::local_exec::LocalExecOptions::new(exec_root.clone(), runs.clone());
            if let Some(opts) = opts {
                local.idle_ms = opts.idle_ms;
                local.deadline_ms = opts.deadline_ms;
                local.now = opts.now;
                local.poll_ms = opts.poll_ms;
                local.reading = opts.reading;
            }
            wsp_core::local_exec::local_exec_stream(local, is_waiting)
        }),
        home: Rc::new(move |_| format!("{home}/.claude")),
        home_dir: folder.clone(),
        roots_path: format!("{folder}/roots"),
        env: Rc::new(|| indexmap::IndexMap::from([("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()))])),
        daemon_road: None,
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: Some(Rc::new(fake_copier(None, None))),
        platform: if cfg!(target_os = "macos") { Platform::Darwin } else { Platform::Linux },
        close: None,
    };
    let driven = Driven::held_turn();
    let fx = runtime(&StubBackend::new(None)).adapter("claude", driven.factory()).with(move |o| o.local = Some(local)).build();
    Held { fx, driven, waiting, _root: dir }
}

async fn begin(h: &Held, prompt: &str) -> Rc<DTurn> {
    let rt = h.fx.rt.clone();
    let ws = create_on(&rt, CreateOn::named("mac").on(HERE_PLACE_ID), None).await.expect("a workspace").view.id;
    rt.sessions().start(&ws, StartOptions { prompt: prompt.into(), ..Default::default() }, None).await.expect("a start");
    let driven = h.driven.clone();
    wait!([driven] driven.len() == 1);
    h.driven.turn(0)
}

// "what holds a turn's idle clock" > "a background task the harness reports alive holds it, as an open prompt does, and the two are one box"
#[test]
fn a_background_task_the_harness_reports_alive_holds_it_as_an_open_prompt_does_and_the_two_are_one_box() {
    drive(|| async {
        let h = start();
        let emit = begin(&h, "run the gate in the background").await;
        // A turn working on its own: nothing outside its process is holding it, so its stream's quiet is its own.
        assert_eq!(h.waiting(), Some(false));

        emit.emit(json!({ "type": "turn.tasks", "running": 1 }));
        assert_eq!(h.waiting(), Some(true));
        emit.emit(json!({ "type": "turn.tasks", "running": 0 }));
        assert_eq!(h.waiting(), Some(false));

        // The prompt road is unchanged, and a task still running holds the box past the prompt's close: one box, two
        // reasons, so closing one does not say the turn is idle while the other stands.
        emit.emit(json!({ "type": "permission.ask", "ask": ask() }));
        assert_eq!(h.waiting(), Some(true));
        emit.emit(json!({ "type": "turn.tasks", "running": 1 }));
        emit.emit(json!({ "type": "permission.close", "askId": ASK_ID, "outcome": "allowed", "optionId": "allow" }));
        assert_eq!(h.waiting(), Some(true));
        emit.emit(json!({ "type": "turn.tasks", "running": 0 }));
        assert_eq!(h.waiting(), Some(false));
        h.fx.rt.close().await.expect("a close");
    });
}

// "what holds a turn's idle clock" > "a task the harness still lists under its agent's final reply holds the turn as before it"
#[test]
fn a_task_the_harness_still_lists_under_its_agents_final_reply_holds_the_turn_as_before_it() {
    drive(|| async {
        let h = start();
        let emit = begin(&h, "start the server and say where it is").await;

        emit.emit(json!({ "type": "turn.tasks", "running": 1 }));
        emit.emit(json!({ "type": "turn.tasks", "running": 1, "replied": { "status": "completed", "text": "It is on port 3000." } }));
        assert_eq!(h.waiting(), Some(true));
        emit.emit(json!({ "type": "turn.tasks", "running": 0 }));
        assert_eq!(h.waiting(), Some(false));
        h.fx.rt.close().await.expect("a close");
    });
}
