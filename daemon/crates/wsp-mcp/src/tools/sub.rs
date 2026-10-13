// SPDX-License-Identifier: AGPL-3.0-only
//! `run` with `on`: a sub-thread of the thread asking, on a machine of its own on a cloud. The host makes the machine
//! behind every check it runs; this opens the thread in the machine's folder with the preamble above the task and
//! answers with what the machine came up with. packages/host/src/verbs/threads.ts `subStart` is the rule.

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::named::{params, Workspace};
use super::said::turns;
use super::turn::{
    before_sending, begin_through, checked_start, files_from, follow, held_line, name_of_task, notify_of, opening, taken_name_after,
    turn_out, turn_refusal, turn_text, workspace_names, Picks, TurnOut,
};
use super::{Answer, Refused};
use crate::host::Host;
use crate::record::fill;

/// What a run answers: a turn's answer, and on a sub-thread's start what its machine came up with.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct RunOut {
    #[serde(flatten)]
    pub turn: TurnOut,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sub: Option<SubOut>,
}

/// A sub-thread's machine, branches and files sent, as the host answers its create: `SubOut`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SubOut {
    pub on: String,
    pub machine: String,
    pub size: SubSize,
    pub rate_usd_per_hour: f64,
    pub repo: String,
    pub base: SubBase,
    pub branch: String,
    pub sent: SubSent,
    pub not_sent: SubNotSent,
    pub delete_after_minutes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
#[serde(rename_all = "camelCase")]
pub struct SubSize {
    pub cpu: u64,
    pub mem_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct SubBase {
    pub branch: String,
    pub sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct SubSent {
    pub paths: Vec<String>,
    pub bytes: u64,
    pub missing: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct SubNotSent {
    pub uncommitted: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub why: Option<String>,
}

/// What the host answers a sub-thread's create with: the machine, and the folder, preamble and agent its start opens
/// the thread with (`SubStarted`).
#[derive(Deserialize)]
struct SubMade {
    workspace: Box<serde_json::value::RawValue>,
    sub: SubStarted,
}

#[derive(Deserialize)]
struct SubStarted {
    out: SubOut,
    folder: String,
    preamble: String,
    harness: String,
}

/// What a run with `on` asks of the host for the sub-thread's machine.
pub(super) struct SubAsk {
    pub(super) on: String,
    pub(super) size: Option<String>,
    pub(super) from: Option<String>,
    pub(super) branch: Option<String>,
    pub(super) agent: Option<String>,
}

/// A sub-thread's start: the picks read against the agent's lists, the machine made with every check the host runs
/// behind it, and the thread opened in its folder there with the preamble above the task, named as the machine is:
/// packages/host/src/verbs/threads.ts `subStart`.
#[allow(clippy::too_many_arguments)]
pub(super) async fn sub_run(
    host: Arc<Host>,
    ask: SubAsk,
    message: String,
    picks: Picks,
    notify: Option<Vec<String>>,
    title: Option<String>,
    files: Option<Vec<String>>,
    detach: Option<bool>,
) -> Result<Answer, Refused> {
    let client = host.client().await?;
    let read = async {
        let notify = notify_of(&client, notify.as_deref().unwrap_or_default()).await?;
        checked_start(&client, &message, ask.agent.as_deref(), &picks, None, None).await?;
        let name = taken_name_after(
            title.as_deref().map_or_else(|| name_of_task(&message), str::to_owned).as_str(),
            &workspace_names(&client).await?,
        );
        let mut sub = params([("on", Value::from(ask.on.as_str()))]);
        for (key, value) in [("size", &ask.size), ("from", &ask.from), ("branch", &ask.branch), ("harness", &ask.agent)] {
            if let Some(value) = value {
                sub.insert(key.to_owned(), Value::from(value.as_str()));
            }
        }
        let made: SubMade = client
            .request("workspaces.create", params([("project", Value::from("")), ("name", Value::from(name)), ("sub", Value::Object(sub))]))
            .await?;
        Ok((notify, made))
    }
    .await;
    let (notify, made) = before_sending(&client, read)?;
    let workspace: Workspace = super::workspace::read(&made.workspace, "workspaces.create")?;
    let attachments = files_from(files.as_deref().unwrap_or_default(), host.args().guest)?;
    let words = &turns().sub;
    let prompt = fill(&words.opening, &[("preamble", &made.sub.preamble), ("task", &message)]);
    let mut start = opening(&host, &workspace.id, &prompt, Some(made.sub.harness.clone()), Some(&made.sub.folder), notify);
    start.insert("title".to_owned(), Value::from(workspace.name.as_str()));
    if !attachments.is_empty() {
        start.insert("attachments".to_owned(), Value::from(attachments));
    }
    picks.wire(&mut start)?;
    let on = made.sub.out.on.to_lowercase();
    let opened = |thread_id: &str| {
        let short: String = thread_id.chars().take(8).collect();
        fill(&words.opened, &[("thread", &short), ("on", &on), ("folder", &made.sub.folder)])
    };
    if detach == Some(true) {
        let turn = begin_through(&host, client, &start).await?;
        let out = RunOut { turn: turn_out(&turn), sub: Some(made.sub.out.clone()) };
        return Ok(Answer::text(held_line(opened(&out.turn.thread_id), &turn), &out));
    }
    let turn = follow(&host, client, &start, &mut None).await?;
    if let Some(refused) = turn_refusal(&turn) {
        return Err(refused.into());
    }
    let out = RunOut { turn: turn_out(&turn), sub: Some(made.sub.out.clone()) };
    Ok(Answer::text(format!("{}\n{}", opened(&out.turn.thread_id), turn_text(&out.turn)), &out))
}
