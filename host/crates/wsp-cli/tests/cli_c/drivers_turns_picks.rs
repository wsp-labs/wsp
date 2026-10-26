// SPDX-License-Identifier: AGPL-3.0-only
//! Drivers of `turns_picks`: one per helper of `src/verbs/turns_help/picks.rs` the probe `turns_picks.probe.ts` recorded.
//! Each reads the case's arguments, calls the helper and answers it as the JSON the TS answered.
//!
//! The probe's own arguments, which the drivers mirror: `filesBase64` (file name to base64: written into the case's
//! folder), `sizes` (file name to a length the file is cut or grown to), `summary` (answer attachments as
//! `{ mediaType, name, length }`: a file at its cap is 14 MB of base64). A refusal that names the case's folder says
//! `<cwd>`, as the recorded ones do.
use std::fs;
use std::path::Path;

use base64::Engine;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_cli::verbs::client::{BoxFuture, FlagValue, Flags};
use wsp_cli::verbs::turns_help as t;
use wsp_wire::types::attachments::Attachment;
use wsp_wire::views::session::ThreadView;
use wsp_words::exit::HostError;

use super::common::Env;

/// What the case's folder is called in what the TS wrote down.
pub fn scrub_err(env: &Env, mut e: HostError) -> HostError {
    e.message = e.message.replace(&env.cwd, "<cwd>");
    e.fix = e.fix.map(|f| f.replace(&env.cwd, "<cwd>"));
    e
}

/// A value as JSON with the case's folder written `<cwd>`.
pub fn value_of<T: Serialize>(env: &Env, v: &T) -> Value {
    let text = serde_json::to_string(v).expect("a value serializes").replace(&env.cwd, "<cwd>");
    serde_json::from_str(&text).expect("it reads back")
}

/// What a driver answers: the value as JSON, or the refusal with the folder scrubbed.
pub fn done<T: Serialize>(env: &Env, r: Result<T, HostError>) -> Result<Value, HostError> {
    r.map(|v| value_of(env, &v)).map_err(|e| scrub_err(env, e))
}

/// Writes what the probe's `filesBase64` and `sizes` name into the case's folder, in that order, after `files`.
pub fn prepare(env: &Env) {
    let root = Path::new(&env.cwd);
    if let Some(Value::Object(m)) = env.arg("filesBase64") {
        for (name, b64) in m {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).expect("folder");
            fs::write(path, base64::engine::general_purpose::STANDARD.decode(b64.as_str().unwrap()).expect("base64")).expect("file");
        }
    }
    if let Some(Value::Object(m)) = env.arg("sizes") {
        for (name, size) in m {
            let path = root.join(name);
            fs::create_dir_all(path.parent().unwrap()).expect("folder");
            let file = fs::OpenOptions::new().append(true).create(true).open(&path).expect("file");
            file.set_len(size.as_u64().expect("a length")).expect("sized");
        }
    }
}

fn attachments_json(env: &Env, files: &[Attachment]) -> Value {
    let summary = env.flag("summary");
    Value::Array(
        files
            .iter()
            .map(|a| {
                if summary {
                    json!({ "mediaType": a.media_type, "name": a.name, "length": a.bytes.len() })
                } else {
                    json!({ "mediaType": a.media_type, "name": a.name, "bytes": a.bytes })
                }
            })
            .collect(),
    )
}

fn from<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).expect("the case's argument reads")
}

fn picks(env: &Env, name: &str) -> t::Picks {
    env.arg(name).map(from).unwrap_or_default()
}

fn flags_of(v: &Value) -> Flags {
    v.as_object()
        .expect("flags")
        .iter()
        .map(|(k, v)| {
            let value = match v {
                Value::String(s) => FlagValue::Str(s.clone()),
                Value::Bool(b) => FlagValue::Bool(*b),
                Value::Array(a) => FlagValue::List(a.iter().map(|s| s.as_str().unwrap().to_string()).collect()),
                other => panic!("no flag is {other}"),
            };
            (k.clone(), value)
        })
        .collect()
}

fn opening_at(v: &Value) -> t::OpeningAt {
    let o = v.as_object().expect("the target");
    if let Some(here) = o.get("here") {
        t::OpeningAt::Here(from(here))
    } else if let Some(fork) = o.get("fork") {
        t::OpeningAt::Fork(t::ForkTarget { fork: from(fork) })
    } else if let Some(forks) = o.get("forks") {
        t::OpeningAt::Forks(t::ThreadFork { forks: from(forks), branch: o.get("branch").and_then(|b| b.as_str()).map(String::from) })
    } else {
        t::OpeningAt::Workspace { id: o["id"].as_str().expect("a workspace id").to_string() }
    }
}

pub fn picks_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { done(env, t::picks_of(&picks(env, "picks"))) })
}

pub fn pick_flags(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let got = t::pick_flags(&flags_of(env.arg("flags").expect("flags")));
        // The TS answers `{ model?, effort?, access?, fast? }` with an empty string kept and only `fast: true`.
        let mut out = serde_json::Map::new();
        if let Some(v) = got.model {
            out.insert("model".into(), v.into());
        }
        if let Some(v) = got.effort {
            out.insert("effort".into(), v.into());
        }
        if let Some(v) = got.access {
            out.insert("access".into(), v.into());
        }
        if got.fast {
            out.insert("fast".into(), true.into());
        }
        Ok(Value::Object(out))
    })
}

pub fn files_from(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        prepare(env);
        let got = t::files_from(&env.strs("paths"), env.flag("elsewhere"));
        done(env, got.map(|files| attachments_json(env, &files)))
    })
}

pub fn checked_start(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let task = env.str_arg("task").unwrap();
        let got = t::checked_start(
            env.client.as_ref(),
            &task,
            env.str_arg("harness").as_deref(),
            &picks(env, "picks"),
            env.str_arg("workspaceId").as_deref(),
            env.str_arg("forkOf").as_deref(),
        )
        .await;
        done(env, got.map(|()| Value::Null))
    })
}

pub fn project_defaults_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { done(env, t::project_defaults_of(env.client.as_ref()).await) })
}

pub fn preferences_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { done(env, t::preferences_of(env.client.as_ref()).await) })
}

pub fn worktree_for(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        done(env, t::worktree_for(env.client.as_ref(), &env.str_arg("project").unwrap(), &env.str_arg("branch").unwrap()).await)
    })
}

pub fn opening_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        prepare(env);
        let at = opening_at(env.arg("at").expect("a target"));
        let opts: t::OpeningOpts = env.arg("opts").map(from).unwrap_or_default();
        done(env, t::opening_of(&env.case.env, &at, &env.str_arg("prompt").unwrap(), &opts).map(Value::Object))
    })
}

pub fn thread_fork(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let at: Option<t::AtWord> = env.arg("at").map(from);
        let got = t::thread_fork(
            env.client.as_ref(),
            &env.str_arg("ref").unwrap(),
            at.as_ref(),
            env.str_arg("branch").as_deref(),
            &env.str_arg("task").unwrap(),
            env.str_arg("harness").as_deref(),
            &picks(env, "picks"),
            env.cloud(),
        )
        .await;
        done(env, got)
    })
}

pub fn resume_asked(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let r#where: t::ResumeWhere = env.arg("where").map(from).unwrap_or_default();
        done(env, t::resume_asked(env.str_arg("id").as_deref(), env.flag("copy"), &r#where))
    })
}

pub fn replaced_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { done(env, t::replaced_of(env.client.as_ref(), env.str_arg("ref").as_deref()).await) })
}

pub fn notify_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { done(env, t::notify_of(env.client.as_ref(), &env.strs("refs")).await) })
}

pub fn message_to(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        prepare(env);
        let thread: ThreadView = from(env.arg("thread").expect("a thread"));
        done(
            env,
            t::message_to(&thread, &env.str_arg("prompt").unwrap(), &picks(env, "picks"), &env.strs("files"), env.flag("elsewhere"))
                .map(Value::Object),
        )
    })
}
