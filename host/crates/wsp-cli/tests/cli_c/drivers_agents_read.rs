// SPDX-License-Identifier: AGPL-3.0-only
//! The drivers of the `agents_read` unit: one per recorded helper of `agents-help.ts:1-298`.
#![allow(dead_code)]

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde_json::Value;
use wsp_cli::hosts::HostAim;
use wsp_cli::place_signin::BoxSignedIn;
use wsp_cli::verbs::agents_help::read as a;
use wsp_cli::verbs::client::{Again, BoxFuture, CliIo, Dialer, FlagValue, Flags, Frame, HostClient, Out, VerbContext};
use wsp_cli::watch::{Redraw, StopListener, WatchSignals};
use wsp_wire::types::agents_report::{AgentsReport, PluginBrings, PluginRow, ServerToolsAnswer};
use wsp_words::exit::HostError;

use super::common::{Env, Sinks};

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

fn opt(env: &Env, name: &str) -> Option<String> {
    env.str_arg(name)
}

pub fn const_aimed_both_line(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::AIMED_BOTH_LINE) })
}

pub fn const_project_unnamed_line(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::PROJECT_UNNAMED_LINE) })
}

pub fn const_tools_project_bare_line(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::TOOLS_PROJECT_BARE_LINE) })
}

pub fn const_plugin_kind_words(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::PLUGIN_KIND_WORDS) })
}

pub fn const_server_tool_columns(_env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::SERVER_TOOL_COLUMNS) })
}

pub fn aimed_usage(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::aimed_usage(&text(env, "tool"))) })
}

pub fn project_off_computer_line(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::project_off_computer_line(&text(env, "value"))) })
}

pub fn plugin_turned_words(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::plugin_turned_words(&text(env, "id"), env.flag("on"), &text(env, "agent"))) })
}

pub fn plugin_turned_line(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let row: PluginRow = arg(env, "row");
        ok(a::plugin_turned_line(&row))
    })
}

pub fn unset_variable_line(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::unset_variable_line(&text(env, "variable"))) })
}

pub fn not_header_line(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::not_header_line(&text(env, "pair"))) })
}

pub fn project_scope_line(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::project_scope_line(&text(env, "word"))) })
}

pub fn tools_added_line(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::tools_added_line(&text(env, "agent"), &text(env, "file"))) })
}

pub fn signed_in_line(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let answer = env.arg("answer").unwrap();
        let said = BoxSignedIn {
            signed_in: answer["signedIn"].as_bool().unwrap(),
            detail: answer.get("detail").and_then(|v| v.as_str()).map(String::from),
            said: answer.get("said").and_then(|v| v.as_str()).map(String::from),
        };
        ok(a::signed_in_line(&text(env, "what"), &text(env, "where"), &said))
    })
}

pub fn project_asked(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let value = env.arg("value").map(|v| match v {
            Value::Bool(b) => a::ProjectValue::Flag(*b),
            Value::String(s) => a::ProjectValue::Name(s.clone()),
            other => panic!("a project value is a flag or a name, not {other}"),
        });
        ok(a::project_asked(value.as_ref(), opt(env, "thread").as_deref(), opt(env, "on").as_deref(), &text(env, "usage"))?)
    })
}

pub fn tools_project(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let got = a::tools_project(opt(env, "value").as_deref(), opt(env, "thread").as_deref(), opt(env, "on").as_deref(), &text(env, "usage"))?;
        match got {
            Some(name) => ok(name),
            None => Ok(Value::Null),
        }
    })
}

pub fn agents_target(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let target = a::agents_target(env.client.as_ref(), opt(env, "thread").as_deref(), opt(env, "on").as_deref(), &text(env, "usage"), &text(env, "line"), opt(env, "project").as_deref(), env.cloud()).await?;
        ok(target)
    })
}

pub fn agents_report(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let report = a::agents_report(env.client.as_ref(), opt(env, "thread").as_deref(), opt(env, "on").as_deref(), &text(env, "usage"), &text(env, "line"), env.cloud()).await?;
        ok(report)
    })
}

fn report(env: &Env) -> AgentsReport {
    arg(env, "report")
}

pub fn report_facts(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::report_facts(&report(env))) })
}

pub fn agent_row_lines(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::agent_row_lines(&report(env))) })
}

pub fn skill_row_lines(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::skill_row_lines(&report(env))) })
}

pub fn server_row_lines(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::server_row_lines(&report(env))) })
}

pub fn plugin_row_lines(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::plugin_row_lines(&report(env))) })
}

pub fn brought_words(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let brings: PluginBrings = arg(env, "brings");
        ok(a::brought_words(&brings))
    })
}

pub fn tool_lines(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let answer: ServerToolsAnswer = arg(env, "answer");
        ok(a::tool_lines(&text(env, "name"), &answer))
    })
}

pub fn plugin_changed(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let ask: a::PluginChange = {
            let v = env.arg("ask").unwrap();
            a::PluginChange { agent: v["agent"].as_str().unwrap().into(), plugin: v["plugin"].as_str().unwrap().into(), on: v["on"].as_bool().unwrap() }
        };
        let row = a::plugin_changed(env.client.as_ref(), &ask, opt(env, "thread").as_deref(), opt(env, "on").as_deref(), &text(env, "usage"), &text(env, "line"), env.cloud()).await?;
        ok(row)
    })
}

pub fn server_tools_of(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let answer = a::server_tools_of(
            env.client.as_ref(),
            &text(env, "name"),
            &text(env, "agent"),
            opt(env, "thread").as_deref(),
            opt(env, "on").as_deref(),
            env.flag("refresh"),
            &text(env, "usage"),
            &text(env, "line"),
            opt(env, "project").as_deref(),
            env.cloud(),
        )
        .await?;
        ok(answer)
    })
}

pub fn server_changed(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let op = match text(env, "op").as_str() {
            "servers.add" => a::ServerOp::Add,
            "servers.remove" => a::ServerOp::Remove,
            "servers.toggle" => a::ServerOp::Toggle,
            other => panic!("no such op {other}"),
        };
        let body: Frame = arg(env, "body");
        let got = a::server_changed(env.client.as_ref(), op, &body, opt(env, "thread").as_deref(), opt(env, "on").as_deref(), &text(env, "usage"), &text(env, "line"), opt(env, "project").as_deref(), env.cloud()).await?;
        ok(got)
    })
}

pub fn server_values(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let vars: IndexMap<String, String> = arg(env, "env");
        let names = env.strs("names");
        let headers = env.strs("headers");
        ok(a::server_values(&vars, &names, &headers, &text(env, "usage"))?)
    })
}

pub fn server_command(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::server_command(opt(env, "line").as_deref(), &text(env, "usage"))?) })
}

pub fn server_scope(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let project = match env.arg("project") {
            None => a::ProjectAsked::default(),
            Some(p) => a::ProjectAsked { project: p["project"].as_bool().unwrap(), name: p.get("name").and_then(|v| v.as_str()).map(String::from) },
        };
        ok(a::server_scope(opt(env, "word").as_deref(), &text(env, "usage"), &project)?)
    })
}

pub fn add_tools(env: &Env) -> Answer<'_> {
    Box::pin(async move { ok(a::add_tools(env.client.as_ref(), &text(env, "agent")).await?) })
}

// ---- drawRows

/// The one dial of a case: every `client()` answers the case's first socket and says `dial`.
struct OneDial {
    client: Arc<dyn HostClient>,
    sinks: Arc<Sinks>,
}

#[async_trait]
impl Dialer for OneDial {
    async fn client(&self, _again: Option<Again>) -> Result<Arc<dyn HostClient>, HostError> {
        self.sinks.said.lock().unwrap().push("dial".to_string());
        Ok(self.client.clone())
    }
}

/// A terminal that redraws: raw writes go where the case's streamed text is, and signals are held so a frame can stop the
/// watch the way Ctrl-C would. Each `on` and `off` says its signal.
struct Stage {
    sinks: Arc<Sinks>,
    listeners: Mutex<Vec<StopListener>>,
}

impl Redraw for Stage {
    fn write(&self, text: &str) {
        self.sinks.streamed.lock().unwrap().push_str(text);
    }

    fn columns(&self) -> u16 {
        80
    }
}

impl WatchSignals for Stage {
    fn on(&self, signal: &'static str, listener: StopListener) {
        self.sinks.said.lock().unwrap().push(format!("on {signal}"));
        self.listeners.lock().unwrap().push(listener);
    }

    fn off(&self, signal: &'static str, _listener: &StopListener) {
        self.sinks.said.lock().unwrap().push(format!("off {signal}"));
    }
}

/// The case's `Sinks` with a redraw where the case has a terminal.
struct StageIo {
    sinks: Arc<Sinks>,
    redraw: Option<Arc<dyn Redraw>>,
}

#[async_trait]
impl CliIo for StageIo {
    fn log(&self, line: &str) {
        self.sinks.log(line)
    }

    fn error(&self, line: &str) {
        self.sinks.error(line)
    }

    fn stream(&self, text: &str) {
        self.sinks.stream(text)
    }

    fn redraw(&self) -> Option<Arc<dyn Redraw>> {
        self.redraw.clone()
    }

    async fn ask(&self, question: &str) -> Result<String, HostError> {
        self.sinks.ask(question).await
    }

    async fn ask_secret(&self, question: &str, variable: Option<&str>) -> Result<String, HostError> {
        self.sinks.ask_secret(question, variable).await
    }
}

pub fn draw_rows(env: &Env) -> Answer<'_> {
    Box::pin(async move {
        let flags_in = env.arg("flags").unwrap().as_object().unwrap().clone();
        let mut flags = Flags::new();
        for (k, v) in &flags_in {
            flags.insert(k.clone(), FlagValue::Bool(v.as_bool().unwrap()));
        }
        let json_on = flags_in.get("json").and_then(|v| v.as_bool()).unwrap_or(false);
        let terminal = env.arg("redraw").and_then(|v| v.as_bool()).unwrap_or(false);
        let stage = Arc::new(Stage { sinks: env.sinks.clone(), listeners: Mutex::new(Vec::new()) });
        let io: Arc<dyn CliIo> = Arc::new(StageIo { sinks: env.sinks.clone(), redraw: terminal.then(|| stage.clone() as Arc<dyn Redraw>) });
        let mut deps = env.deps.clone();
        deps.dialer = Arc::new(OneDial { client: env.client.clone(), sinks: env.sinks.clone() });
        deps.signals = terminal.then(|| stage.clone() as Arc<dyn WatchSignals>);
        let ctx = VerbContext { deps, args: Vec::new(), flags, io: io.clone(), out: Out::formatter(io, json_on), aim: HostAim::Here, usage: String::new() };

        let frames: Vec<(Value, Vec<String>)> = env
            .arg("frames")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|f| (f["value"].clone(), f["rows"].as_array().unwrap().iter().map(|r| r.as_str().unwrap().to_string()).collect()))
            .collect();
        let frame_op = opt(env, "frameOp");
        let stop_after = env.arg("stopAfter").and_then(|v| v.as_u64()).map(|n| n as usize);
        let calls = Arc::new(Mutex::new(0usize));
        let sinks = env.sinks.clone();
        let frame = move |client: Arc<dyn HostClient>| -> BoxFuture<'static, Result<a::Rows, HostError>> {
            let frames = frames.clone();
            let frame_op = frame_op.clone();
            let calls = calls.clone();
            let sinks = sinks.clone();
            let stage = stage.clone();
            Box::pin(async move {
                let n = {
                    let mut c = calls.lock().unwrap();
                    *c += 1;
                    *c
                };
                sinks.said.lock().unwrap().push(format!("frame {n}"));
                if let Some(op) = &frame_op {
                    client.request(op, Frame::new()).await?;
                }
                let (value, rows) = frames[(n - 1).min(frames.len() - 1)].clone();
                if Some(n) == stop_after {
                    let held: Vec<StopListener> = stage.listeners.lock().unwrap().clone();
                    for l in held {
                        l();
                    }
                }
                Ok(a::Rows { value, rows })
            })
        };
        let code = a::draw_rows(&ctx, &text(env, "words"), &frame).await?;
        ok(code)
    })
}
