// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-processes.ts at 65f6b89c7: the ops that read or end what a thread runs,
//! Activity's own, and settle and delete, which meet the processes a thread still runs before they take it. Spec:
//! spec/serve_helpers.md, H25.
use serde::Serialize;
use serde_json::Value;
use wsp_core::types::api::{DeleteThreadAsk, SettleAsk, ThreadDeleted};
use wsp_threads::activity::{ActivityAsk, EndAsk};
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::HostError;

use crate::serve::served::{DynRuntime, Payload};

/// `activity.read`, `activity.end`, `sessions.settle` or `sessions.delete`. serve-processes.ts:9.
pub fn is_processes_request(msg: &RuntimeRequest) -> bool {
    matches!(
        msg.op,
        RuntimeOp::ActivityRead { .. }
            | RuntimeOp::ActivityEnd { .. }
            | RuntimeOp::SessionsSettle { .. }
            | RuntimeOp::SessionsDelete { .. }
    )
}

/// `answerProcesses(rt, msg, origin)`: activity off `activity()`, settle and delete off `sessions()`.
/// serve-processes.ts:11-24.
pub async fn answer_processes(rt: &DynRuntime, msg: &RuntimeRequest, origin: Option<Caller>) -> Result<Payload, HostError> {
    match &msg.op {
        RuntimeOp::ActivityRead { computer, thread, .. } => {
            let ask = ActivityAsk { computer: computer.clone(), thread: thread.clone(), thread_ids: None };
            let activity = rt.activity().read(ask, origin).await?;
            let mut out = Payload::new();
            out.insert("activity".into(), to_value(&activity)?);
            Ok(out)
        }
        RuntimeOp::ActivityEnd { computer, pid, started_at, .. } => {
            let ask = EndAsk { computer: computer.clone(), pid: u32::try_from(pid.get()).unwrap_or(u32::MAX), started_at: started_at.0 };
            spread(&rt.activity().end(ask, origin).await?)
        }
        RuntimeOp::SessionsSettle { thread_ids, finished, processes, .. } => {
            let ask = SettleAsk { finished: finished.filter(|finished| *finished), processes: *processes };
            spread(&rt.sessions().settle(thread_ids.clone(), origin, ask).await?)
        }
        RuntimeOp::SessionsDelete { thread_id, processes, check, .. } => {
            let ask = DeleteThreadAsk { processes: *processes, check: check.filter(|check| *check) };
            deleted(&rt.sessions().delete(thread_id, origin, ask).await?)
        }
        _ => Err(HostError::new(format!("{} is not answered here", msg.op.name()))),
    }
}

/// `sessions.delete`'s answer: `machine` is `"deleted"` only where the machine went, each optional field only where given.
fn deleted(answer: &ThreadDeleted) -> Result<Payload, HostError> {
    let mut out = Payload::new();
    out.insert("workspaceId".into(), Value::from(answer.workspace_id.clone()));
    if let Some(worktree) = &answer.worktree {
        out.insert("worktree".into(), Value::from(worktree.clone()));
    }
    out.insert("threads".into(), Value::from(answer.threads));
    if let Some(ended) = &answer.ended {
        out.insert("ended".into(), to_value(ended)?);
    }
    if answer.machine_deleted {
        out.insert("machine".into(), Value::from("deleted"));
    }
    if let Some(on) = &answer.on {
        out.insert("on".into(), Value::from(on.clone()));
    }
    if let Some(question) = &answer.question {
        out.insert("question".into(), Value::from(question.clone()));
    }
    Ok(out)
}

fn to_value<T: Serialize>(answer: &T) -> Result<Value, HostError> {
    serde_json::to_value(answer).map_err(|e| HostError::new(e.to_string()))
}

/// The answer's fields beside id and ok, as the TS spreads them.
fn spread<T: Serialize>(answer: &T) -> Result<Payload, HostError> {
    match to_value(answer)? {
        Value::Object(fields) => Ok(fields),
        _ => Err(HostError::new("the answer is not an object")),
    }
}
