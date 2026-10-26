// SPDX-License-Identifier: AGPL-3.0-only
//! Drivers of `turns_target`: one per helper of `src/verbs/turns_help/target.rs` the probe `turns_target.probe.ts`
//! recorded. Each reads the case's arguments, calls the helper and answers it as the JSON the TS answered.
//!
//! A `RunTarget` carries a function, which the TS value loses in JSON; the recorded value is the target and, as `opened`, the
//! two lines that function says for `thread-1` with no folder (`bare`) and in `/root/work/lab` (`folder`), with HOME
//! `/root` (set here, as the probe sets it). The cloud setting of a case (`cloud: true`) is handed to the helpers that read it.
use std::time::Duration;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use wsp_cli::verbs::client::BoxFuture;
use wsp_cli::verbs::turns_help as t;
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::workspace_more::WorkspaceOut;
use wsp_words::exit::HostError;

use super::common::Env;
use super::drivers_turns_picks::done;

fn from<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).expect("the case's argument reads")
}

fn summary(target: &t::RunTarget) -> Value {
    let opened = |f: &t::Opened| json!({ "bare": f("thread-1", None), "folder": f("thread-1", Some("/root/work/lab")) });
    match target {
        t::RunTarget::Folder(f) => json!({ "here": f.here, "opened": opened(&f.opened) }),
        t::RunTarget::Fork(f) => serde_json::to_value(f).expect("a fork target serializes"),
        t::RunTarget::Workspace { workspace, opened: o } => {
            let mut out = json!({ "workspace": workspace });
            if let Some(f) = o {
                out["opened"] = opened(f);
            }
            out
        }
    }
}

pub fn run_target(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        std::env::set_var("HOME", "/root");
        let r#where: t::RunWhere = from(env.arg("where").expect("where"));
        let elsewhere = env.arg("elsewhere").and_then(|v| v.as_bool());
        let got = t::run_target(env.client.as_ref(), env.str_arg("ref").as_deref(), env.str_arg("callerCwd").as_deref(), &env.case.env, elsewhere, &r#where, env.case.cloud).await;
        done(env, got.map(|target| summary(&target)))
    })
}

pub fn project_of_folder(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let projects: Vec<Value> = from(env.arg("projects").expect("projects"));
        done(env, Ok::<_, HostError>(t::project_of_folder(&projects, &env.str_arg("folder").unwrap())))
    })
}

pub fn fork_for(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let project: t::ProjectPick = from(env.arg("project").expect("project"));
        let say = env.sinks.say();
        done(env, t::fork_for(env.client.as_ref(), &project, &env.str_arg("task").unwrap(), &say).await)
    })
}

pub fn sub_for(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let ask: t::SubAsk = from(env.arg("ask").expect("ask"));
        let say = env.sinks.say();
        let made = t::sub_for(env.client.as_ref(), &ask, &env.str_arg("task").unwrap(), env.str_arg("title").as_deref(), std::sync::Arc::new(say)).await;
        done(
            env,
            match made {
                Err(e) => Err(e),
                Ok(made) => {
                    let at_once = env.flag("stopAtOnce");
                    if at_once {
                        (made.stop)();
                    }
                    if env.flag("wait") || at_once {
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                    (made.stop)();
                    Ok(json!({ "workspace": made.workspace, "sub": made.sub }))
                }
            },
        )
    })
}

pub fn thread_here(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let row: Option<ThreadView> = env.arg("ref").filter(|v| v.is_object()).map(from);
        let word = env.str_arg("ref");
        let r#ref = match (&row, &word) {
            (Some(row), _) => Some(t::ThreadRef::Row(row)),
            (None, Some(word)) => Some(t::ThreadRef::Word(word)),
            (None, None) => None,
        };
        done(env, t::thread_here(env.client.as_ref(), r#ref).await)
    })
}

pub fn sub_deleting(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let thread: Option<ThreadView> = env.arg("thread").map(from);
        done(env, t::sub_deleting(env.client.as_ref(), thread.as_ref()).await)
    })
}

pub fn refuse_machine_thread(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move { done(env, t::refuse_machine_thread(env.client.as_ref(), &env.str_arg("ref").unwrap()).await.map(|()| Value::Null)) })
}

pub fn thread_at(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let got = t::thread_at_in(env.client.as_ref(), &env.str_arg("ref").unwrap(), &env.str_arg("line").unwrap(), env.flag("orComputer"), env.case.cloud).await;
        done(env, got.map(|(thread, workspace)| json!({ "thread": thread, "workspace": workspace })))
    })
}

fn pair(v: &Value) -> (ThreadView, WorkspaceOut) {
    (from(&v["thread"]), from(&v["workspace"]))
}

pub fn child_of(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let lead = pair(env.arg("lead").expect("lead"));
        let got = t::child_of(env.client.as_ref(), &lead, &env.str_arg("ref").unwrap(), &env.str_arg("line").unwrap(), env.case.cloud).await;
        done(env, got.map(|(thread, workspace)| json!({ "thread": thread, "workspace": workspace })))
    })
}

pub fn sharing_with(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let at = pair(env.arg("at").expect("at"));
        done(env, t::sharing_with(env.client.as_ref(), &at).await)
    })
}

pub fn thread_deleted(env: &Env) -> BoxFuture<'_, Result<Value, HostError>> {
    Box::pin(async move {
        let o: t::ThreadDeletedAsk = env.arg("o").map(from).unwrap_or_default();
        done(env, t::thread_deleted(env.client.as_ref(), &env.str_arg("threadId").unwrap(), &o).await)
    })
}
