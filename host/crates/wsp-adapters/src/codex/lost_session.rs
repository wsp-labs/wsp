// SPDX-License-Identifier: AGPL-3.0-only
//! A start that resumes a thread whose rollout the server does not hold: a second launch on a new thread handed the
//! thread so far, through the port's `start_after_loss`. TS adapter-codex/src/lost-session.ts:59-96. Spec:
//! spec/codex_adapter.md rule 8.

use std::cell::RefCell;
use std::sync::{Arc, Mutex};

use futures::future::LocalBoxFuture;

use super::turn::{CodexKept, CodexSession, SessionCore, Sessions};
use crate::port::{
    AdapterError, AdapterEvent, AfterLoss, AnswerOutcome, ExecFailure, Finishes, HarnessStartOptions, LostSessionStart, OnEvent,
    PermissionAnswer, SharedFuture, SteerOutcome, TaskStop, TurnResult,
};

/// What a server answers a resume of a thread its home holds no rollout for. TS lost-session.ts:77.
pub fn no_rollout_line(thread_id: &str) -> String {
    format!("codex could not open the thread: no rollout found for thread id {thread_id}")
}

impl Finishes for CodexSession {
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.core.finished()
    }
}

/// The session the caller holds for a seeded start: the first launch's id, and whichever launch runs now. TS lost-session.ts:86.
struct LostSession {
    first: Arc<CodexSession>,
    now: Arc<Mutex<Arc<CodexSession>>>,
    finished: SharedFuture<TurnResult>,
}

impl LostSession {
    fn current(&self) -> Arc<CodexSession> {
        self.now.lock().unwrap().clone()
    }
}

impl SessionCore for LostSession {
    fn local_id(&self) -> String {
        self.first.core.local_id()
    }
    fn thread_id(&self) -> String {
        self.current().core.thread_id()
    }
    fn command(&self) -> Option<String> {
        self.current().core.command()
    }
    fn run(&self) -> Option<String> {
        self.current().core.run()
    }
    fn pid(&self) -> Option<u32> {
        self.current().core.pid()
    }
    fn from(&self) -> Option<u64> {
        self.current().core.from()
    }
    fn finished(&self) -> SharedFuture<TurnResult> {
        self.finished.clone()
    }
    fn interrupt(&self) -> LocalBoxFuture<'static, ()> {
        self.current().core.interrupt()
    }
    fn kept(&self) -> Option<CodexKept> {
        self.current().core.kept()
    }
    fn steer(&self, prompt: String) -> LocalBoxFuture<'static, SteerOutcome> {
        self.current().core.steer(prompt)
    }
    fn answer(&self, ask_id: String, answer: PermissionAnswer) -> LocalBoxFuture<'static, Result<AnswerOutcome, ExecFailure>> {
        self.current().core.answer(ask_id, answer)
    }
    fn stop_task(&self, task: String) -> LocalBoxFuture<'static, TaskStop> {
        self.current().core.stop_task(task)
    }
}

/// The options a launch takes, copied field by field: a one-shot `prompt_after` cannot be copied, so the copy leaves it out.
fn copy_options(o: &HarnessStartOptions) -> HarnessStartOptions {
    HarnessStartOptions {
        prompt: o.prompt.clone(),
        resume: o.resume.clone(),
        copy: o.copy,
        cwd: o.cwd.clone(),
        model: o.model.clone(),
        effort: o.effort.clone(),
        permission_mode: o.permission_mode.clone(),
        context_window: o.context_window.clone(),
        fast: o.fast,
        title: o.title.clone(),
        images: o.images.clone(),
        fork: o.fork.clone(),
        mcp_servers: o.mcp_servers.clone(),
        server_values: o.server_values.clone(),
        off_servers: o.off_servers.clone(),
        seed: o.seed.clone(),
        limit_details: o.limit_details,
        prompt_after: None,
        version: o.version.clone(),
        keep: o.keep,
        on_event: o.on_event.clone(),
    }
}

/// A launch, and where it resumes with a seed (not a copy, not a fork), the second launch behind it; the answer reads
/// whichever runs now. Registered under the first's local id. TS lost-session.ts:81.
pub fn start_seeded(
    launch_turn: std::sync::Arc<dyn Fn(HarnessStartOptions) -> Result<CodexSession, AdapterError>>,
    sessions: &Sessions,
    options: HarnessStartOptions,
) -> Result<CodexSession, AdapterError> {
    let (Some(resume), Some(seed)) = (options.resume.clone(), options.seed.clone()) else {
        return launch_turn(options);
    };
    if options.copy || options.fork.is_some() {
        return launch_turn(options);
    }
    let base = copy_options(&options);
    let on_event: OnEvent = options.on_event.clone().unwrap_or_else(|| Arc::new(|_: AdapterEvent| {}));
    let prompt = options.prompt.clone();
    let first_options = RefCell::new(Some(options));
    let launch = move |prompt: String, resume: Option<String>, on_event: OnEvent| {
        let taken = first_options.borrow_mut().take();
        let mut next = match taken {
            Some(first) => first,
            None => {
                let mut fresh = copy_options(&base);
                fresh.seed = None;
                fresh
            }
        };
        next.prompt = prompt;
        next.resume = resume;
        next.on_event = Some(on_event);
        launch_turn(next).map(Arc::new)
    };
    let AfterLoss { first, now, finished } = crate::lost_session::start_after_loss(
        LostSessionStart { prompt, resume: resume.clone(), seed, on_event },
        no_rollout_line(&resume),
        launch,
    )?;
    let local = first.core.local_id();
    let session = CodexSession { core: Arc::new(LostSession { first, now, finished }) };
    crate::codex::turn::register(sessions, local, &session);
    Ok(session)
}
