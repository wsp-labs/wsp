// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/lost-session-line.test.ts at 65f6b89c7, case for case. The host's one log line when a resume finds
//! the agent's session gone and the turn falls back to a new one: what the machine holds for the lost session, read with
//! Codex's own reading in bash over a CODEX_HOME built here, so the next loss names its own cause. The adapter's start goes
//! through the shared start-after-loss, the road both real adapters take.
#[macro_use]
mod common;

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use common::comeback::{opts, within};
use common::*;
use futures::channel::oneshot;
use futures::future::LocalBoxFuture;
use futures::FutureExt as _;
use serde_json::{json, Value};
use wsp_adapters::codex::session_held::{parse_session_held, session_held_command};
use wsp_adapters::port::{
    start_after_loss, AdapterError, AdapterEvent, ExecFailure, ExecFailureKind, Finishes, HarnessAdapter, HarnessExec, HarnessSession,
    HarnessStartOptions, LostSessionStart, OnEvent, SharedFuture, TurnResult,
};
use wsp_core::console::capture;
use wsp_core::types::api::StartOptions;
use wsp_core::types::harness::{HarnessAdapterContext, HarnessAdapterFactory};
use wsp_threads::threads::launch::{seed_after_loss, LostAt, SessionHeld};

const FIRST: &str = "01a1424a-7c2e-7d31-9f00-5b2a1c3d4e5f";
const SECOND: &str = "01a1424b-fa29-7aa0-8c11-0d9e8f7a6b5c";
const OPENING: &str = "Remember the word lantern.";

fn lost_words() -> String {
    format!("codex could not open the thread: no rollout found for thread id {FIRST}")
}

/// A home whose index still names the first session's rollout, a paginated row whose file is gone.
fn lost_home() -> (tempfile::TempDir, String) {
    let home = tempfile::tempdir().expect("a folder");
    let named = format!("{}/sessions/2026/10/16/rollout-2026-10-16T01-19-52-{FIRST}.jsonl", home.path().display());
    let sql = format!(
        "create table threads (id text primary key, rollout_path text not null, title text not null, history_mode text not null default 'legacy');\n         insert into threads values ('{FIRST}', '{named}', '{OPENING}', 'paginated');"
    );
    let done = std::process::Command::new("sqlite3").arg(home.path().join("state_5.sqlite")).arg(sql).status().expect("sqlite3 runs");
    assert!(done.success(), "sqlite3 made the index");
    (home, named)
}

struct Launch {
    local_id: String,
    finished: SharedFuture<TurnResult>,
}

impl Finishes for Launch {
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
}

struct Losing {
    home: String,
    starts: Rc<RefCell<Vec<Option<String>>>>,
    lost: bool,
}

struct After {
    first_id: String,
    after: wsp_adapters::port::AfterLoss<Launch>,
}

struct Plain(Launch);

impl HarnessSession for Plain {
    fn local_id(&self) -> String {
        self.0.local_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.0.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

impl HarnessSession for After {
    fn local_id(&self) -> String {
        self.first_id.clone()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.after.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        async {}.boxed_local()
    }
}

fn ev(doc: Value) -> AdapterEvent {
    serde_json::from_value(doc).expect("an adapter event")
}

fn launch_with(
    starts: Rc<RefCell<Vec<Option<String>>>>,
    lost: bool,
) -> impl Fn(String, Option<String>, OnEvent) -> Result<Arc<Launch>, AdapterError> + Clone {
    move |_prompt, resume, on| {
        starts.borrow_mut().push(resume.clone());
        let n = starts.borrow().len();
        let session = resume.clone().unwrap_or_else(|| if n == 1 { FIRST.to_string() } else { SECOND.to_string() });
        let refused = lost && resume.is_some();
        let result: TurnResult = serde_json::from_value(if refused {
            json!({ "status": "failed", "error": lost_words(), "text": "" })
        } else {
            json!({ "status": "completed", "text": "ok" })
        })
        .expect("a result");
        let (tx, rx) = oneshot::channel();
        let (r, id) = (result.clone(), session.clone());
        tokio::task::spawn_local(async move {
            tokio::task::yield_now().await;
            if !refused {
                on(ev(json!({ "type": "session.start", "sessionId": id })));
            }
            on(ev(json!({ "type": "turn.done", "sessionId": id, "result": serde_json::to_value(&r).expect("a result") })));
            on(ev(json!({ "type": "session.end", "sessionId": id, "exitCode": 0, "sawResult": true })));
            let _ = tx.send(r);
        });
        let finished: LocalBoxFuture<'static, TurnResult> = async move { rx.await.expect("the turn was dropped") }.boxed_local();
        Ok(Arc::new(Launch { local_id: session, finished: finished.shared() }))
    }
}

impl HarnessAdapter for Losing {
    fn start(&self, o: HarnessStartOptions) -> Result<Arc<dyn HarnessSession>, AdapterError> {
        let launch = launch_with(self.starts.clone(), self.lost);
        let on = o.on_event.clone().expect("an event sink");
        let (Some(resume), Some(seed)) = (o.resume.clone(), o.seed.clone()) else {
            let l = launch(o.prompt.clone(), o.resume.clone(), on)?;
            return Ok(Arc::new(Plain(Launch { local_id: l.local_id.clone(), finished: l.finished.clone() })));
        };
        let after = start_after_loss(LostSessionStart { prompt: o.prompt.clone(), resume, seed, on_event: on }, lost_words(), launch)?;
        Ok(Arc::new(After { first_id: after.first.local_id.clone(), after }))
    }

    fn steers(&self) -> bool {
        false
    }

    fn holds_session(&self) -> bool {
        true
    }

    fn session_held(&self, session_id: String, exec: Arc<dyn HarnessExec>) -> Option<LocalBoxFuture<'static, Result<String, String>>> {
        let home = self.home.clone();
        Some(
            async move {
                let command = session_held_command(&home, &session_id)?;
                exec.exec(&command, None).await.map(|out| parse_session_held(&out)).map_err(|e| e.message)
            }
            .boxed_local(),
        )
    }
}

fn factory(home: &str, starts: &Rc<RefCell<Vec<Option<String>>>>, lost: bool) -> HarnessAdapterFactory {
    let (home, starts) = (home.to_string(), starts.clone());
    Arc::new(move |_ctx: HarnessAdapterContext| {
        Ok(Arc::new(Losing { home: home.clone(), starts: starts.clone(), lost }) as Arc<dyn HarnessAdapter>)
    })
}

struct Turns {
    lines: Vec<String>,
    starts: Vec<Option<String>>,
    named: String,
    workspace: String,
}

/// A runtime on a stub machine whose guest runs the store reading in bash here, and a thread's two turns on it.
async fn two_turns(lost: bool) -> Turns {
    let (home, named) = lost_home();
    let backend = StubBackend::new(None);
    let inner = backend.exec_impl_fn();
    backend.set_exec_impl(Rc::new(move |m, cmd, stdin| {
        if cmd.contains("/proc/sys/kernel/random/boot_id") {
            return Box::pin(async move {
                let out = tokio::process::Command::new("bash").arg("-c").arg(&cmd).output().await.expect("bash runs");
                Ok(exec_ok(&String::from_utf8_lossy(&out.stdout)))
            });
        }
        inner(m, cmd, stdin)
    }));
    let warned = Rc::new(capture());
    let starts = Rc::new(RefCell::new(vec![]));
    let fx = runtime(&backend).adapter("codex", factory(&home.path().display().to_string(), &starts, lost)).build();
    let ws = create_on(&fx.rt, CreateOn::golden("a"), None).await.expect("a workspace").view.id;
    let first = fx.rt.sessions().start(&ws, StartOptions { harness: Some("codex".into()), ..opts(OPENING) }, None).await.expect("a start");
    first.finished().await;
    let thread = j(&first.view())["threadId"].as_str().expect("a thread").to_string();
    fx.rt
        .sessions()
        .start(&ws, StartOptions { thread: Some(thread), harness: Some("codex".into()), ..opts("what was the word?") }, None)
        .await
        .expect("a start")
        .finished()
        .await;
    // The reading runs beside the turn, so its line lands a moment after; a line that never comes fails on the count.
    if lost {
        let w = warned.clone();
        within(5000, || {
            let w = w.clone();
            async move { w.lines().iter().any(|l| l.contains(&format!("session {FIRST}"))) }
        })
        .await;
    }
    let lines = warned.lines().into_iter().filter(|l| l.contains("was gone from")).collect();
    let out = Turns { lines, starts: starts.borrow().clone(), named, workspace: ws };
    fx.rt.close().await.expect("a close");
    out
}

// "a resume that finds the agent's session gone" > "logs one line naming what the machine holds for it: its rollout files, its index row and the path it names, the boot"
#[test]
fn logs_one_line_naming_what_the_machine_holds_for_it_its_rollout_files_its_index_row_and_the_path_it_names_the_boot() {
    drive(|| async {
        let t = two_turns(true).await;
        assert_eq!(t.starts, [None, Some(FIRST.to_string()), None]);
        assert_eq!(t.lines.len(), 1, "{:?}", t.lines);
        let line = &t.lines[0];
        let head = format!("session {FIRST} of thread ");
        let rest = line.strip_prefix(&head).expect("the line opens on the session");
        let (thread, rest) = rest.split_at(8);
        assert!(thread.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{thread}");
        let rest = rest.strip_prefix(&format!(" was gone from {}, so its turn started a new one: boot ", t.workspace)).expect("the words");
        let (boot, rest) = rest.split_once("; ").expect("a boot");
        assert!(boot == "unread" || (boot.len() == 36 && boot.chars().all(|c| c.is_ascii_hexdigit() || c == '-')), "{boot}");
        let _ = rest;
        assert!(line.contains("; rollout files: none; "));
        assert!(
            line.contains(&format!(
                "; index row: state_5.sqlite {{\"path\":\"{}\",\"mode\":\"paginated\"}}, the file it names is missing",
                t.named
            )),
            "{line}"
        );
        assert!(!line.contains(OPENING));
        assert!(!line.contains("what was the word?"));
    });
}

// "..." > "hands the seed over and logs why when the reading throws before it returns"
// (The TS reader throws synchronously where the command is built; a `SessionHeld` here answers `Err`, which is what a refused
// command guard comes to, so the log line reads the same.)
#[test]
fn hands_the_seed_over_and_logs_why_when_the_reading_throws_before_it_returns() {
    drive(|| async {
        let warned = Rc::new(capture());
        let exec: Arc<dyn HarnessExec> =
            Arc::new(|_: &str, _: Option<&wsp_adapters::port::Env>| -> LocalBoxFuture<'static, Result<String, ExecFailure>> {
                async { Err(ExecFailure { kind: ExecFailureKind::Other, message: "no machine".into() }) }.boxed_local()
            });
        let held: SessionHeld = Rc::new(|_, _| async { Err("threadId must be a plain slug".to_string()) }.boxed_local());
        let seed = seed_after_loss(
            Some(held),
            exec,
            LostAt { session_id: FIRST.into(), thread_id: SECOND.into(), workspace_id: "ws_a".into() },
            Rc::new(|| async { Ok("the thread so far".to_string()) }.boxed_local()),
        );
        assert_eq!(seed().await.expect("a seed"), "the thread so far");
        let w = warned.clone();
        wait!([w] w.lines().len() == 1);
        assert!(warned.lines()[0].ends_with(": not read: threadId must be a plain slug"), "{:?}", warned.lines());
    });
}

// "..." > "logs nothing when the resume opens the session"
#[test]
fn logs_nothing_when_the_resume_opens_the_session() {
    drive(|| async {
        let t = two_turns(false).await;
        assert_eq!(t.starts, [None, Some(FIRST.to_string())]);
        assert!(t.lines.is_empty(), "{:?}", t.lines);
    });
}
