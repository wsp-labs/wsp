// SPDX-License-Identifier: AGPL-3.0-only
//! The drivers of `turns_stream.json`: each recorded helper of `turns_help/stream.rs` called with the case's arguments.
use std::sync::Arc;

use indexmap::IndexMap;
use serde_json::{json, Map, Value};
use wsp_cli::verbs::client::{BoxFuture, CliIo, FlagValue, Flags, Frame, OpenedFn, Out as Printer, VerbContext};
use wsp_cli::verbs::turns_help::{self as t, Picks, SaidAbout, Turn};
use wsp_wire::types::sub_thread::SubOut;
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::session_events_more::SessionPermissionEvent;
use wsp_wire::wire::helpers::PermissionOption;
use wsp_wire::wire::runtime::SessionStartOutcome;
use wsp_words::exit::HostError;

use super::common::Env;

type Out = Result<Value, HostError>;

fn turn_of(v: &Value) -> Turn {
    Turn {
        session: serde_json::from_value(v["session"].clone()).expect("session"),
        thread_id: v["threadId"].as_str().expect("threadId").to_string(),
        outcome: serde_json::from_value::<SessionStartOutcome>(v["outcome"].clone()).expect("outcome"),
        result: v.get("result").map(|r| serde_json::from_value::<TurnResult>(r.clone()).expect("result")),
        reason: v.get("reason").and_then(|r| r.as_str()).map(String::from),
        after_cut: v.get("afterCut").and_then(|a| a.as_bool()).unwrap_or(false),
        notice: v.get("notice").and_then(|n| n.as_str()).map(String::from),
    }
}

fn turn_arg(env: &Env) -> Turn {
    turn_of(env.arg("turn").expect("turn"))
}

fn options_arg(env: &Env) -> Vec<PermissionOption> {
    serde_json::from_value(env.arg("options").cloned().expect("options")).expect("options")
}

/// The error as the recorder wrote it: `{error: {message, fix?, kind?}}`.
fn error_json(e: &HostError) -> Value {
    let mut o = Map::new();
    o.insert("message".into(), json!(e.message));
    if let Some(f) = &e.fix {
        o.insert("fix".into(), json!(f));
    }
    if let Some(k) = &e.kind {
        o.insert("kind".into(), json!(k.as_str()));
    }
    json!({ "error": o })
}

pub fn turn_failure(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(t::turn_failure(&turn_arg(env)).map(Value::from).unwrap_or(Value::Null)) })
}

pub fn turn_refusal(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(t::turn_refusal(&turn_arg(env)).map(|e| error_json(&e)).unwrap_or(Value::Null)) })
}

pub fn answer_keys_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(Value::from(t::answer_keys_line(&options_arg(env)))) })
}

pub fn answer_verbs_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(Value::from(t::answer_verbs_line(&env.str_arg("threadId").expect("threadId")))) })
}

fn ask_of(v: &Value) -> SessionPermissionEvent {
    let mut o =
        json!({ "workspaceId": "w", "sessionId": "s", "askId": "a", "toolName": v["toolName"], "input": v["input"], "options": [] });
    if let Some(d) = v.get("detail") {
        o["detail"] = d.clone();
    }
    serde_json::from_value(o).expect("a prompt")
}

pub fn answered_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let ask = ask_of(env.arg("ask").expect("ask"));
        Ok(Value::from(t::answered_line(&env.str_arg("threadId").expect("threadId"), &ask, &env.str_arg("said").expect("said"))))
    })
}

pub fn no_open_ask_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(Value::from(t::no_open_ask_line(&env.str_arg("threadId").expect("threadId")))) })
}

pub fn no_such_answer_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        Ok(Value::from(t::no_such_answer_line(&env.str_arg("threadId").expect("threadId"), &env.str_arg("verb").expect("verb"))))
    })
}

pub fn answer_roads(_env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        Ok(Value::Array(
            t::answer_roads()
                .iter()
                .map(|r| {
                    let mut o = Map::new();
                    o.insert("key".into(), json!(r.key));
                    o.insert("effect".into(), serde_json::to_value(r.effect).expect("effect"));
                    if let Some(d) = r.does {
                        o.insert("does".into(), json!(d));
                    }
                    if let Some(a) = &r.answer {
                        let mut v = Map::new();
                        v.insert("verb".into(), json!(a.verb));
                        v.insert("about".into(), json!(a.about));
                        v.insert("said".into(), json!(a.said));
                        if a.reasons {
                            v.insert("reasons".into(), json!(true));
                        }
                        o.insert("answer".into(), Value::Object(v));
                    }
                    Value::Object(o)
                })
                .collect(),
        ))
    })
}

pub fn answer_in_the_app(_env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(Value::from(t::ANSWER_IN_THE_APP)) })
}

pub fn answer_open_ask(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let key = if env.str_arg("road").as_deref() == Some("allow") { "y" } else { "n" };
        let road = t::answer_roads().iter().find(|r| r.key == key).expect("a road");
        let reason = env.str_arg("reason");
        let a = t::answer_open_ask(env.client.as_ref(), &env.str_arg("ref").expect("ref"), road, reason.as_deref()).await?;
        Ok(json!({ "threadId": a.thread_id, "askId": a.ask_id, "optionId": a.option_id, "line": a.line }))
    })
}

fn picks_of(env: &Env) -> Picks {
    let p = env.arg("picks");
    Picks {
        model: p.and_then(|p| p["model"].as_str()).map(String::from),
        effort: p.and_then(|p| p["effort"].as_str()).map(String::from),
        access: p.and_then(|p| p["access"].as_str()).map(String::from),
        fast: p.and_then(|p| p["fast"].as_bool()).unwrap_or(false),
    }
}

fn opened_line(id: &str, folder: Option<&str>) -> String {
    match folder {
        None => format!("thread {id}"),
        Some(f) => format!("opened {id} in {f}"),
    }
}

fn context(env: &Env) -> VerbContext {
    let json = env.flag("json");
    let io: Arc<dyn CliIo> = env.sinks.clone();
    let mut flags: Flags = IndexMap::new();
    if json {
        flags.insert("json".into(), FlagValue::Bool(true));
    }
    VerbContext {
        deps: env.deps.clone(),
        args: Vec::new(),
        flags,
        io: io.clone(),
        out: Printer::formatter(io, json),
        aim: wsp_cli::hosts::HostAim::Here,
        usage: "wsp <verb>".into(),
    }
}

fn turn_json(t: &Turn) -> Value {
    super::drivers_turns_follow::turn_json(t)
}

pub fn follow_verb(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let ctx = context(env);
        let start: Frame = env.arg("start").and_then(|v| v.as_object()).cloned().expect("start");
        let said = SaidAbout {
            opened: env.flag("opened").then(|| {
                Arc::new(|id: &str, folder: Option<&str>| opened_line(id, folder))
                    as Arc<dyn Fn(&str, Option<&str>) -> String + Send + Sync>
            }),
            spend: env.str_arg("spend"),
        };
        let io = env.sinks.clone();
        let on_turn = move |turn: &t::SharedTurn| {
            let turn = turn.lock().unwrap();
            io.said.lock().unwrap().push(format!(
                "onTurn thread={} outcome={} result={}",
                turn.thread_id,
                serde_json::to_value(&turn.outcome).unwrap().as_str().unwrap(),
                turn.result
                    .as_ref()
                    .map(|r| serde_json::to_value(&r.status).unwrap().as_str().unwrap().to_string())
                    .unwrap_or_else(|| "-".into())
            ));
        };
        let turn = t::follow_verb(&ctx, env.client.clone(), &start, env.flag("announce"), &picks_of(env), &said, Some(&on_turn)).await?;
        Ok(turn_json(&turn))
    })
}

pub fn detach_verb(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let ctx = context(env);
        let start: Frame = env.arg("start").and_then(|v| v.as_object()).cloned().expect("start");
        let sub: Option<SubOut> = env.arg("sub").map(|v| serde_json::from_value(v.clone()).expect("a sub-thread's answer"));
        let said = env.str_arg("said");
        let opened: OpenedFn<'_> = &|id, folder| opened_line(id, folder);
        t::detach_verb(
            &ctx,
            env.client.clone(),
            &start,
            &picks_of(env),
            if env.flag("opened") { Some(opened) } else { None },
            sub.as_ref(),
            said.as_deref(),
        )
        .await?;
        Ok(Value::Null)
    })
}

pub fn before_sending(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let client = env.client.clone();
        let read: BoxFuture<'_, Result<Value, HostError>> =
            Box::pin(async move { client.request("sessions.list", Frame::new()).await.map(Value::Object) });
        t::before_sending(env.client.as_ref(), read).await
    })
}

pub fn turn_view(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(serde_json::to_value(t::turn_view(&turn_arg(env))).expect("a turn view")) })
}
