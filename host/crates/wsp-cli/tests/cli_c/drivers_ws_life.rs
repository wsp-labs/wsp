// SPDX-License-Identifier: AGPL-3.0-only
//! The drivers of the `ws_life` unit: one per recorded helper, each reading the case's arguments, calling the Rust
//! helper and answering the JSON the TS value was written as. `workspaces-help.ts:311-598`.
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use wsp_cli::verbs::client::BoxFuture;
use wsp_cli::verbs::turns_help::Turn;
use wsp_cli::verbs::workspaces_help as w;
use wsp_wire::views::session::SessionView;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::{HostError, Kind};

use super::common::Env;

pub fn typed<T: DeserializeOwned>(env: &Env, name: &str) -> T {
    serde_json::from_value(env.arg(name).unwrap_or_else(|| panic!("the case names no {name}")).clone())
        .unwrap_or_else(|e| panic!("the recorded {name} reads: {e}"))
}

fn maybe<T: DeserializeOwned>(env: &Env, name: &str) -> Option<T> {
    env.arg(name).map(|v| serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("the recorded {name} reads: {e}")))
}

fn said<T: serde::Serialize>(value: T) -> Result<Value, HostError> {
    Ok(serde_json::to_value(value).expect("the value serializes"))
}

fn text(env: &Env, name: &str) -> String {
    env.str_arg(name).unwrap_or_else(|| panic!("the case names no {name}"))
}

pub fn rename_workspace(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::rename_workspace(env.client.as_ref(), &text(env, "ref"), &text(env, "name")).await?) })
}

pub fn renamed_workspace_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::renamed_workspace_line(&typed(env, "renamed"))) })
}

pub fn awake(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let tell = env.sinks.say();
        said(w::awake(env.client.as_ref(), &typed(env, "workspace"), &text(env, "action"), &tell).await?)
    })
}

pub fn no_draft_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::no_draft_line(env.str_arg("note").as_deref())) })
}

pub fn const_no_draft_fix(_env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { Ok(json!(w::NO_DRAFT_FIX)) })
}

pub fn committed(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let say = env.sinks.say();
        said(
            w::committed(
                env.client.as_ref(),
                &text(env, "workspaceId"),
                env.str_arg("message").as_deref(),
                &env.strs("files"),
                &say,
                env.str_arg("threadId").as_deref(),
            )
            .await?,
        )
    })
}

pub fn asked_to_fix(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let child: Option<w::ChildRef> = maybe(env, "child");
        said(
            w::asked_to_fix(
                env.client.as_ref(),
                &text(env, "workspaceId"),
                env.str_arg("check").as_deref(),
                child.as_ref(),
                &text(env, "threadId"),
            )
            .await?,
        )
    })
}

pub fn fix_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::fix_line(&text(env, "thread"), &typed(env, "asked"), env.str_arg("child").as_deref())) })
}

pub fn merged_in(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let tell = env.sinks.say();
        said(w::merged_in(env.client.as_ref(), &typed(env, "lead"), &typed(env, "child"), &tell).await?)
    })
}

pub fn started_from(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::started_from(env.client.as_ref(), &typed(env, "o")).await?) })
}

pub fn review_started(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::review_started(env.client.as_ref(), &typed(env, "o")).await?) })
}

pub fn review_posted(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        said(w::review_posted(env.client.as_ref(), &text(env, "workspaceId"), &typed(env, "edits"), &text(env, "threadId")).await?)
    })
}

fn echo(id: &str) -> String {
    format!("opened {id}")
}

pub fn started_lines(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let started: wsp_wire::views::harness::StartResult = typed(env, "started");
        let thread: Option<&(dyn Fn(&str) -> String + Send + Sync)> =
            if env.str_arg("thread").as_deref() == Some("echo") { Some(&echo) } else { None };
        said(w::started_lines(&started, thread))
    })
}

pub fn review_lines(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let started: wsp_wire::views::harness::StartResult = typed(env, "started");
        let thread: Option<&(dyn Fn(&str) -> String + Send + Sync)> =
            if env.str_arg("thread").as_deref() == Some("echo") { Some(&echo) } else { None };
        said(w::review_lines(&started, thread))
    })
}

pub fn thread_label(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::thread_label(&typed(env, "thread"))) })
}

pub fn thread_cwd(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let worktree: Option<wsp_wire::views::workspace_more::WorktreeFolder> = maybe(env, "worktree");
        said(w::thread_cwd(env.str_arg("cwd").as_deref(), worktree.as_ref()))
    })
}

pub fn folder_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::folder_of(&typed(env, "workspace"), env.case.env.get("HOME").map(String::as_str))) })
}

pub fn with_shared(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::with_shared(&text(env, "line"), &env.strs("shared"))) })
}

pub fn shared_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        Ok(match w::shared_of(&env.strs("shared")) {
            Some(ids) => json!({ "sharedWith": ids }),
            None => json!({}),
        })
    })
}

pub fn const_verdicts(_env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let mut table = Map::new();
        for (word, verdict) in w::VERDICTS {
            table.insert(word.to_string(), serde_json::to_value(verdict).expect("a verdict serializes"));
        }
        Ok(Value::Object(table))
    })
}

pub fn merged_pr(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        said(
            w::merged_pr(
                env.client.as_ref(),
                &text(env, "workspaceId"),
                maybe(env, "method"),
                env.flag("whenChecksPass"),
                &text(env, "threadId"),
            )
            .await?,
        )
    })
}

pub fn update_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::update_line(&text(env, "workspace"), &typed(env, "done"))) })
}

/// The turn a case names: its thread, and the refusal its result carries where it names one.
pub fn turn_of(env: &Env) -> Option<Turn> {
    let turn = env.arg("turn")?;
    let session: SessionView =
        serde_json::from_value(json!({ "id": "s_1", "workspaceId": "ws_1", "harness": "claude", "status": "failed" }))
            .expect("a session reads");
    let result: Option<TurnResult> =
        turn.get("refusal").map(|r| serde_json::from_value(json!({ "status": "completed", "refusal": r })).expect("a result reads"));
    Some(Turn {
        session,
        thread_id: turn["threadId"].as_str().expect("a thread id").to_string(),
        outcome: SessionStartOutcome::Started,
        result,
        reason: None,
        after_cut: false,
        notice: None,
    })
}

pub fn nap_after_dead_launch(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let turn = turn_of(env);
        Ok(match w::nap_after_dead_launch(env.client.as_ref(), &typed(env, "woken"), turn.as_ref()).await {
            Some(line) => json!(line),
            None => Value::Null,
        })
    })
}

/// What `errorOf` of the recorder writes for an error, the stamps in its order.
pub fn error_value(e: &HostError) -> Value {
    let mut o = Map::new();
    o.insert("message".into(), json!(e.message));
    if let Some(fix) = &e.fix {
        o.insert("fix".into(), json!(fix));
    }
    if let Some(kind) = &e.kind {
        o.insert("kind".into(), json!(kind.as_str()));
    }
    if let Some(errors) = &e.errors {
        o.insert("errors".into(), Value::Array(errors.clone()));
    }
    if let Some(warnings) = &e.warnings {
        o.insert("warnings".into(), Value::Array(warnings.clone()));
    }
    if let Some(left) = &e.left {
        o.insert("left".into(), Value::Array(left.clone()));
    }
    json!({ "error": o })
}

pub fn with_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let spec = env.arg("error").expect("an error");
        let message = spec["message"].as_str().expect("a message");
        let mut e = match spec.get("kind").and_then(Value::as_str) {
            Some(kind) => HostError::with_kind(message, Kind::parse(kind)),
            None => HostError::new(message),
        };
        e.fix = spec.get("fix").and_then(Value::as_str).map(String::from);
        e.errors = spec.get("errors").and_then(Value::as_array).cloned();
        Ok(error_value(&w::with_line(e, env.str_arg("line").as_deref())))
    })
}

pub fn stop(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::stop(env.client.as_ref(), &text(env, "ref"), env.str_arg("subagent").as_deref()).await?) })
}

pub fn stop_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::stop_line(&typed(env, "stopped"))) })
}

pub fn forget_thread(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        w::forget_thread(env.client.as_ref(), &typed(env, "thread"), env.flag("check")).await?;
        Ok(Value::Null)
    })
}

pub fn thread_forgot_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::thread_forgot_line(&typed(env, "thread"))) })
}

pub fn settle_threads(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let threads: Vec<wsp_wire::views::session::ThreadView> = typed(env, "threads");
        said(w::settle_threads(env.client.as_ref(), &threads, env.flag("finished"), maybe(env, "processes")).await?)
    })
}

pub fn restore_threads(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let threads: Vec<wsp_wire::views::session::ThreadView> = typed(env, "threads");
        said(w::restore_threads(env.client.as_ref(), &threads).await?)
    })
}

pub fn settled_lines(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::settled_lines(&typed(env, "answer"))) })
}

pub fn restored_lines(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::restored_lines(&typed(env, "answer"))) })
}

pub fn rename(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::rename(env.client.as_ref(), &typed(env, "thread"), &text(env, "title")).await?) })
}

pub fn rename_line(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { said(w::rename_line(&typed(env, "renamed"))) })
}
