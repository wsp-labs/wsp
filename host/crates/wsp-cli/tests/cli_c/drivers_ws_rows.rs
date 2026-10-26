// SPDX-License-Identifier: AGPL-3.0-only
//! The drivers of the `ws_rows` unit: one per recorded helper of `workspaces-help.ts:602-881`.
#![allow(dead_code)]

use std::sync::Arc;

use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::{json, Map, Value};
use wsp_cli::verbs::client::{BoxFuture, CliIo, Out};
use wsp_cli::verbs::workspaces_help::reads::ThreadRow;
use wsp_cli::verbs::workspaces_help::rows as w;
use wsp_wire::views::session::ThreadView;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_more::WorkspaceOut;
use wsp_wire::wire::capabilities::MachineSizeOffer;
use wsp_words::exit::HostError;
use wsp_words::workspace_state::StandsOn;

use super::common::Env;

type Answer<'a> = BoxFuture<'a, Result<Value, HostError>>;

fn read<T: DeserializeOwned>(v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("the case's {v} does not read: {e}"))
}

fn arg<T: DeserializeOwned>(env: &Env, name: &str) -> T {
    read(env.arg(name).unwrap_or_else(|| panic!("the case has no {name}")))
}

fn text(env: &Env, name: &str) -> String {
    env.str_arg(name).unwrap_or_else(|| panic!("the case has no {name}"))
}

fn ok<T: serde::Serialize>(v: T) -> Result<Value, HostError> {
    Ok(serde_json::to_value(v).expect("the result serializes"))
}

/// `{workspace, threads, on?}`: what a `Dropping` is as a case holds it and as the TS returned it.
fn dropping_of(env: &Env, name: &str) -> w::Dropping {
    let d = env.arg(name).unwrap_or_else(|| panic!("the case has no {name}"));
    w::Dropping {
        workspace: read(&d["workspace"]),
        threads: d["threads"].as_u64().expect("threads") as usize,
        on: d
            .get("on")
            .map(|on| StandsOn { name: on["name"].as_str().unwrap().to_string(), computer: on["computer"].as_str().unwrap().to_string() }),
    }
}

fn dropping_json(d: &w::Dropping) -> Value {
    let mut m = Map::new();
    m.insert("workspace".into(), serde_json::to_value(&d.workspace).expect("a workspace serializes"));
    m.insert("threads".into(), json!(d.threads));
    if let Some(on) = &d.on {
        m.insert("on".into(), json!({ "name": on.name, "computer": on.computer }));
    }
    Value::Object(m)
}

fn out_of(env: &Env) -> Out {
    Out::formatter(env.sinks.clone() as Arc<dyn CliIo>, env.flag("json"))
}

fn entries(m: &IndexMap<String, String>) -> Value {
    Value::Array(m.iter().map(|(k, v)| json!([k, v])).collect())
}

pub fn const_thread_head(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::THREAD_HEAD) })
}

pub fn const_no_project_yet(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::NO_PROJECT_YET) })
}

pub fn const_folder_width(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::FOLDER_WIDTH) })
}

pub fn const_title_width(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::TITLE_WIDTH) })
}

pub fn made_worktree(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let workspace: WorkspaceOut = arg(env, "workspace");
        match w::made_worktree(workspace.worktree.as_ref()) {
            Some(copy) => Ok(json!({ "path": copy.path })),
            None => Ok(Value::Null),
        }
    })
}

pub fn dropping(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let d = w::dropping(env.client.as_ref(), &text(env, "ref")).await?;
        Ok(dropping_json(&d))
    })
}

pub fn deleting(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let d = w::deleting(env.client.as_ref(), &text(env, "ref")).await?;
        Ok(dropping_json(&d))
    })
}

pub fn forget_question(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::forget_question(&dropping_of(env, "d"))) })
}

pub fn forgot_line(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::forgot_line(&dropping_of(env, "d"))) })
}

pub fn delete_question(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::delete_question(&dropping_of(env, "d"))) })
}

pub fn deleted_line(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::deleted_line(&dropping_of(env, "d"))) })
}

pub fn forget(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        w::forget(env.client.as_ref(), &dropping_of(env, "d")).await?;
        Ok(Value::Null)
    })
}

pub fn delete_workspace(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        w::delete_workspace(env.client.as_ref(), &dropping_of(env, "d")).await?;
        Ok(Value::Null)
    })
}

pub fn shortened_front(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::shortened_front(&text(env, "path"), arg::<usize>(env, "width"))) })
}

pub fn shortened_end(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::shortened_end(&text(env, "text"), arg::<usize>(env, "width"))) })
}

fn now_ms() -> f64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("the clock is past 1970").as_millis() as f64
}

pub fn thread_lines(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let t: ThreadRow = arg(env, "t");
        let home = text(env, "home");
        ok(w::thread_lines(&t, &env.str_arg("indent").unwrap_or_default(), Some(&home), now_ms()))
    })
}

pub fn disk_gone_lines(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let rows: Vec<ThreadRow> = arg(env, "rows");
        ok(w::disk_gone_lines(&rows))
    })
}

pub fn thread_tree(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let rows: Vec<ThreadRow> = arg(env, "rows");
        let tree = w::thread_tree(&rows);
        ok(tree
            .iter()
            .map(|n| {
                let index = rows.iter().position(|r| std::ptr::eq(r, n.row)).expect("a row of the listing");
                json!({ "index": index, "id": n.row.thread.id, "depth": n.depth })
            })
            .collect::<Vec<_>>())
    })
}

pub fn place_names(env: &Env) -> Answer<'_> {
    Box::pin(async move { Ok(entries(&w::place_names(env.client.as_ref()).await?)) })
}

pub fn projects_of(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::projects_of(env.client.as_ref()).await?) })
}

pub fn project_of(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::project_of(env.client.as_ref(), &text(env, "ref")).await?) })
}

pub fn project_line(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let p: ProjectView = arg(env, "p");
        let workspaces: Vec<WorkspaceOut> = arg(env, "workspaces");
        let held: Vec<ThreadView> =
            env.arg("threadsHeld").unwrap().as_array().unwrap().iter().map(|t| read::<ThreadRow>(t).thread).collect();
        let named: IndexMap<String, String> = env
            .arg("named")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|e| (e[0].as_str().unwrap().to_string(), e[1].as_str().unwrap().to_string()))
            .collect();
        ok(w::project_line(&p, &workspaces, &held, &named))
    })
}

pub fn create_for(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let project: w::ProjectPick = arg(env, "project");
        let asked = match env.arg("asked") {
            None => w::CreateAsked::default(),
            Some(a) => w::CreateAsked {
                size: a.get("size").and_then(|v| v.as_str()).map(String::from),
                agents: a.get("agents").map(read::<w::AgentsAsked>),
                parent: a.get("parent").and_then(|v| v.as_str()).map(String::from),
            },
        };
        let created = w::create_for(env.client.as_ref(), &out_of(env), &project, &text(env, "name"), &asked).await?;
        ok(created)
    })
}

pub fn projects_here(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(w::projects_here(env.client.as_ref()).await?) })
}

pub fn agents_asked(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let asked = w::agents_asked(env.arg("maxMachines"), env.arg("maxDepth"), env.str_arg("computer").as_deref())?;
        match asked {
            Some(a) => ok(a),
            None => Ok(Value::Null),
        }
    })
}

pub fn count_asked(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let n =
            w::count_asked(&text(env, "flagName"), env.arg("word").unwrap(), arg::<u64>(env, "least"), env.str_arg("computer").as_deref())?;
        ok(n)
    })
}

pub fn size_chosen(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let sizes: Vec<MachineSizeOffer> = arg(env, "sizes");
        ok(w::size_chosen(&sizes, &text(env, "word"))?)
    })
}

pub fn ssh_asked(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let got = w::ssh_asked(env.str_arg("name").as_deref(), env.str_arg("port").as_deref(), env.str_arg("keyPath").as_deref())?;
        let mut m = Map::new();
        if let Some(name) = got.name {
            m.insert("name".into(), json!(name));
        }
        if let Some(port) = got.port {
            m.insert("port".into(), json!(port));
        }
        if let Some(key) = got.key_path {
            m.insert("keyPath".into(), json!(key));
        }
        Ok(Value::Object(m))
    })
}
