// SPDX-License-Identifier: AGPL-3.0-only
//! A start that resumes a session the agent's store no longer holds. TS packages/protocol/src/lost-session.ts. Spec:
//! spec/port.md, start_after_loss.

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use futures::future::FutureExt;

use crate::pending::{lost_session_prompt, LOST_SESSION_NOTE};
use crate::port::{
    AdapterError, AdapterEvent, AfterLoss, DeltaKind, Finishes, LostSessionStart, OnEvent, TurnDelta, TurnResult, TurnStatus,
};

fn failed(words: String) -> TurnResult {
    TurnResult {
        status: TurnStatus::Failed,
        duration_ms: None,
        waited_ms: None,
        cost_usd: None,
        tokens: None,
        model: None,
        models: None,
        text: None,
        error: Some(words),
        unreached: None,
        refusal: None,
        limit: None,
    }
}

pub fn start_after_loss<S, L>(options: LostSessionStart, lost: String, launch: L) -> Result<AfterLoss<S>, AdapterError>
where
    S: Finishes + ?Sized + 'static,
    L: Fn(String, Option<String>, OnEvent) -> Result<Arc<S>, AdapterError> + 'static,
{
    let LostSessionStart { prompt, resume, seed, on_event } = options;
    let gone = Rc::new(Cell::new(false));
    let first_sink = {
        let gone = gone.clone();
        let on_event = on_event.clone();
        Arc::new(move |event: AdapterEvent| {
            if let AdapterEvent::TurnDone { result, .. } = &event {
                if result.error.as_deref() == Some(lost.as_str()) {
                    gone.set(true);
                    return;
                }
            }
            if gone.get() && matches!(event, AdapterEvent::SessionEnd { .. }) {
                return;
            }
            on_event(event);
        })
    };
    let first = launch(prompt.clone(), Some(resume), first_sink)?;
    let current = Arc::new(Mutex::new(first.clone()));
    let launch = Rc::new(launch);
    let finished = {
        let first = first.clone();
        let current = current.clone();
        async move {
            let result = first.finished().await;
            if !gone.get() {
                return result;
            }
            let history = match seed().await {
                Ok(history) => history,
                Err(words) => return failed(words),
            };
            let noted = Rc::new(Cell::new(false));
            let second_sink: OnEvent = Arc::new(move |event: AdapterEvent| {
                let note_for = match &event {
                    AdapterEvent::SessionStart { session_id, .. } if !noted.replace(true) => Some(session_id.clone()),
                    _ => None,
                };
                on_event(event);
                if let Some(session_id) = note_for {
                    on_event(AdapterEvent::TurnDelta(TurnDelta::new(session_id, DeltaKind::Note, LOST_SESSION_NOTE)));
                }
            });
            match launch(lost_session_prompt(&history, &prompt), None, second_sink) {
                Ok(second) => {
                    *current.lock().unwrap() = second.clone();
                    second.finished().await
                }
                Err(AdapterError(words)) => failed(words),
            }
        }
    };
    Ok(AfterLoss { first, now: current, finished: finished.boxed_local().shared() })
}
