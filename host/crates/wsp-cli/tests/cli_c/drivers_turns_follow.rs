// SPDX-License-Identifier: AGPL-3.0-only
//! The drivers of `turns_follow.json`: each recorded helper of `turns_help/follow.rs` called with the case's arguments.
//! What the TS callbacks said is written down the same way here (`summary`, `queued ...`), so a callback heard out of
//! order or not at all fails the case's `said` lines.
use std::sync::Arc;

use serde_json::{json, Value};
use wsp_cli::verbs::client::{BoxFuture, Frame, HostClient};
use wsp_cli::verbs::turns_help::{self as t, FollowOn, Turn, Waited};
use wsp_wire::types::thread_read::ThreadMessage;
use wsp_wire::views::preferences::ThreadHead;
use wsp_wire::views::session::{SessionOrigin, ThreadView};
use wsp_wire::views::session_events_more::SessionEvent;
use wsp_words::exit::HostError;
use wsp_words::thread_read::{thread_messages, thread_reply_rows};

use super::common::Env;

type Out = Result<Value, HostError>;

fn frame(v: &Value) -> Frame {
    v.as_object().expect("an object").clone()
}

fn threads(env: &Env, name: &str) -> Vec<ThreadView> {
    env.arg(name)
        .and_then(|v| v.as_array())
        .expect("threads")
        .iter()
        .map(|v| serde_json::from_value(v.clone()).expect("a thread view"))
        .collect()
}

/// A whole number as JSON, the way JS writes a millisecond count.
fn count(n: f64) -> Value {
    if n.fract() == 0.0 && n >= 0.0 {
        Value::from(n as u64)
    } else {
        Value::from(n)
    }
}

fn origin(env: &Env) -> SessionOrigin {
    serde_json::from_value(env.arg("startedBy").cloned().expect("startedBy")).expect("an origin")
}

/// The turn as the recorder wrote it: the fields in a fixed order, the absent ones left out.
pub fn turn_json(t: &Turn) -> Value {
    let mut o = serde_json::Map::new();
    o.insert("session".into(), serde_json::to_value(&t.session).expect("session"));
    o.insert("threadId".into(), json!(t.thread_id));
    o.insert("outcome".into(), serde_json::to_value(t.outcome).expect("outcome"));
    if let Some(r) = &t.result {
        o.insert("result".into(), serde_json::to_value(r).expect("result"));
    }
    if let Some(r) = &t.reason {
        o.insert("reason".into(), json!(r));
    }
    if t.after_cut {
        o.insert("afterCut".into(), json!(true));
    }
    if let Some(n) = &t.notice {
        o.insert("notice".into(), json!(n));
    }
    Value::Object(o)
}

fn summary(t: &Turn) -> String {
    let outcome = serde_json::to_value(t.outcome).unwrap();
    let status =
        t.result.as_ref().map(|r| serde_json::to_value(r.status).unwrap().as_str().unwrap().to_string()).unwrap_or_else(|| "-".into());
    format!(
        "thread={} outcome={} result={} reason={} afterCut={}",
        t.thread_id,
        outcome.as_str().unwrap(),
        status,
        t.reason.clone().unwrap_or_else(|| "-".into()),
        t.after_cut
    )
}

fn waited_json(w: &Waited) -> Value {
    match w {
        Waited::Ended(e) => {
            let mut ended = serde_json::Map::new();
            ended.insert("threadId".into(), json!(e.thread_id));
            ended.insert("result".into(), serde_json::to_value(&e.result).expect("result"));
            if let Some(s) = &e.sub {
                ended.insert("sub".into(), serde_json::to_value(s).expect("sub"));
            }
            json!({ "ended": ended })
        }
        Waited::TimedOutMs(ms) => json!({ "timedOutMs": count(*ms) }),
    }
}

/// The `on` callbacks of the recorder's `follow` run, each saying what it heard into `said`.
fn heard(env: &Env) -> FollowOn {
    let said = env.sinks.clone();
    let (a, b, c, d, e, f) = (said.clone(), said.clone(), said.clone(), said.clone(), said.clone(), said.clone());
    FollowOn {
        queued: Some(Arc::new(move |q| {
            a.said.lock().unwrap().push(format!("queued {}", q.waits_for.clone().unwrap_or_else(|| "-".into())))
        })),
        starting: Some(Arc::new(move |s| b.said.lock().unwrap().push(format!("starting {} installs={}", s.harness, s.installs.is_some())))),
        steered: Some(Arc::new(move |s| c.said.lock().unwrap().push(format!("steered waiting={}", s.waiting == Some(true))))),
        started: Some(Arc::new(move |t| {
            let t = t.lock().unwrap();
            d.said.lock().unwrap().push(format!("started {} notice={}", summary(&t), t.notice.clone().unwrap_or_else(|| "-".into())))
        })),
        redialed: Some(Arc::new(move || e.said.lock().unwrap().push("redialed".into()))),
        event: Some(Arc::new(move |ev, t| {
            let kind = serde_json::to_value(ev).unwrap()["type"].as_str().unwrap().to_string();
            f.said.lock().unwrap().push(format!("event {kind} {}", summary(t)))
        })),
    }
}

pub fn start_detached(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let start = frame(env.arg("start").expect("start"));
        let queued: Option<Arc<dyn Fn(&wsp_wire::views::session_events::SessionQueuedEvent) + Send + Sync>> = if env.flag("queued") {
            let io = env.sinks.clone();
            Some(Arc::new(move |q| io.said.lock().unwrap().push(format!("queued {}", q.waits_for.clone().unwrap_or_else(|| "-".into())))))
        } else {
            None
        };
        let dialer = env.dialer.clone();
        let redial = move || -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> {
            let d = dialer.clone();
            Box::pin(async move { d.redial().await })
        };
        let turn =
            t::start_detached(env.client.clone(), &start, origin(env), queued, if env.flag("redial") { Some(&redial) } else { None })
                .await?;
        Ok(turn_json(&turn))
    })
}

pub fn follow(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let start = frame(env.arg("start").expect("start"));
        let dialer = env.dialer.clone();
        let redial = move || -> BoxFuture<'static, Result<Arc<dyn HostClient>, HostError>> {
            let d = dialer.clone();
            Box::pin(async move { d.redial().await })
        };
        let turn =
            t::follow(env.client.clone(), &start, origin(env), &heard(env), if env.flag("redial") { Some(&redial) } else { None }).await?;
        Ok(turn_json(&turn))
    })
}

pub fn first_ended(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let named = threads(env, "named");
        let timeout = env.arg("timeoutMs").and_then(|v| v.as_u64());
        Ok(waited_json(&t::first_ended(env.client.as_ref(), &named, timeout).await?))
    })
}

pub fn wait_through(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let named = threads(env, "named");
        let timeout = env.arg("timeoutMs").and_then(|v| v.as_u64());
        let io = env.sinks.clone();
        let redialed = move || io.said.lock().unwrap().push("redialed".into());
        Ok(waited_json(&t::wait_through(&env.deps, &named, timeout, Some(&redialed)).await?))
    })
}

pub fn restart_host(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(json!({ "running": t::restart_host(&env.deps).await? })) })
}

fn messages_json(m: &[ThreadMessage]) -> Value {
    serde_json::to_value(m).expect("messages")
}

pub fn read_thread(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let thread: ThreadView = serde_json::from_value(env.arg("thread").cloned().expect("thread")).expect("a thread view");
        let read = t::read_thread(env.client.as_ref(), &thread, env.flag("last")).await?;
        Ok(json!({ "threadId": read.thread_id, "messages": messages_json(&read.messages) }))
    })
}

pub fn read_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let thread_id = if env.flag("otherThread") { "t-other".to_string() } else { env.str_arg("threadId").expect("threadId") };
        let events: Vec<SessionEvent> = serde_json::from_value(env.arg("events").cloned().expect("events")).expect("events");
        let last = env.flag("last");
        let messages = if last { thread_reply_rows(&events, &thread_id) } else { thread_messages(&events, &thread_id) };
        let read = t::ThreadRead { thread_id, messages };
        Ok(Value::from(t::read_line(&read, last, &|_| 0)))
    })
}

pub fn thread_head(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let thread: ThreadView = serde_json::from_value(env.arg("thread").cloned().expect("thread")).expect("a thread view");
        Ok(serde_json::to_value(t::thread_head(env.client.as_ref(), &thread).await?).expect("a head"))
    })
}

pub fn head_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move {
        let head: ThreadHead = serde_json::from_value(env.arg("head").cloned().expect("head")).expect("a head");
        Ok(Value::from(t::head_line(&head, &|_| 0)))
    })
}

pub fn host_did_not_stop_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(Value::from(t::host_did_not_stop_line(env.arg("waitMs").and_then(|v| v.as_u64()).expect("waitMs")))) })
}

pub fn host_restarted_line(env: &Env) -> BoxFuture<'_, Out> {
    Box::pin(async move { Ok(Value::from(t::host_restarted_line(&env.strs("running")))) })
}
