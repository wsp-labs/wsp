// SPDX-License-Identifier: AGPL-3.0-only
//! packages/runtime/test/steer-lines.test.ts at 65f6b89c7, case for case. The finished lines a child sends its lead when a
//! message is steered into the child's last words: the runtime and the real Claude adapter, fed the lines Claude Code 2.1.280
//! printed for each case (recorded 2026-10-08, trimmed to the fields the adapter reads), and 2.1.295 for a message a woken agent
//! takes at a tool's end (recorded 2026-10-09). A turn sends each reply's line once: one held over background work goes as it is
//! given, one held for a steered message alone goes at the turn's end, and a slash command's answer is never the reply. Each
//! test names the TS test above it. The TS `memoryStore()` is the sqlite store here.
mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::fed::{fed_runs, init, lifecycle, result, said, task_done, tasks, FedRuns, Run};
use common::*;
use serde_json::Value;
use wsp_adapters::claude::adapter::{create_claude_adapter, AdapterDeps};
use wsp_adapters::port::{ExecStreamFactory, HarnessAdapter};
use wsp_core::types::api::{StartOptions, SteerAsk};
use wsp_core::types::harness::HarnessAdapterFactory;
use wsp_core::types::wiring::SessionHandle;
use wsp_words::exit::HostError;
use wsp_words::words::thread::still_running_line;

const REPORT: &str = "Built the parser; 40 tests pass.";
const ANSWER: &str = "BANANA";
const LINE: &str = "thread 1234abcd finished (completed): reply with the single word BANANA";
const COST: &str = "Total cost:            $0.0128\nTotal duration (API):  5s";

/// A reply given over running background work, as its line carries it.
fn held(reply: &str) -> String {
    format!("{reply}\n\n{}", still_running_line(1.0))
}

/// The end of a turn whose line promised another and had nothing to add under the reply: its outcome alone.
fn is_outcome(line: &str) -> bool {
    let Some(rest) = line.strip_prefix("thread ") else { return false };
    let Some((word, rest)) = rest.split_once(" finished (completed") else { return false };
    word.chars().all(|c| c.is_alphanumeric() || c == '_')
        && !word.is_empty()
        && rest.strip_suffix(')').is_some_and(|inner| !inner.contains(')'))
}

/// One text a test expects in the list a line list read: a text, or the shape of an outcome.
enum Line {
    Text(String),
    Outcome,
}

fn assert_lines(got: &[String], want: &[Line]) {
    assert_eq!(got.len(), want.len(), "{got:?}");
    for (g, w) in got.iter().zip(want) {
        match w {
            Line::Text(t) => assert_eq!(g, t),
            Line::Outcome => assert!(is_outcome(g), "{g}"),
        }
    }
}

struct Setup {
    rt: Fx,
    backend: Arc<StubBackend>,
    state: State,
    fed: Rc<FedRuns>,
    factory: Arc<dyn ExecStreamFactory>,
    kid_ws: String,
    lead_thread: String,
    kid: Rc<dyn SessionHandle>,
    run: Rc<Run>,
}

fn host(backend: &Arc<StubBackend>, state: &State, factory: &Arc<dyn ExecStreamFactory>) -> Fx {
    let exec = factory.clone();
    let adapter: HarnessAdapterFactory = Arc::new(move |_ctx| {
        let mut deps = AdapterDeps::new(exec.clone(), "/root/.claude-cfg");
        deps.result_exit_ms = Some(60_000);
        let adapter = create_claude_adapter(deps).map_err(|e| HostError::new(e.0))?;
        Ok(Arc::new(adapter) as Arc<dyn HarnessAdapter>)
    });
    runtime(backend).store(&state.store).adapter("claude", adapter).build()
}

async fn history(rt: &Fx, ws: &str) -> Vec<Value> {
    rt.rt.sessions().history(ws, None).await.expect("a history").iter().map(j).collect()
}

async fn setup() -> Setup {
    let (factory, fed) = fed_runs();
    let fed = Rc::new(fed);
    let backend = StubBackend::new(None);
    let state = sqlite_state();
    let rt = host(&backend, &state, &factory);
    let lead_ws = create_on(&rt.rt, CreateOn::golden("lead"), None).await.expect("a workspace").view.id;
    let lead =
        rt.rt.sessions().start(&lead_ws, StartOptions { prompt: "orchestrate".into(), ..Default::default() }, None).await.expect("a start");
    until(|| async { fed.run_of("orchestrate").is_some() }).await;
    let lead_run = fed.run_of("orchestrate").expect("the run");
    lead_run.push(&[init(), said("msg_0", "waiting"), result("waiting", 1, None)]);
    until(|| async { history(&rt, &lead_ws).await.iter().any(|e| e["type"] == "session.done") }).await;
    lead_run.end(Some(0));
    lead.finished().await;
    let lead_thread = lead.view().thread_id.expect("a thread");
    let kid_ws = create_on(&rt.rt, CreateOn::golden("kid"), None).await.expect("a workspace").view.id;
    let opts = StartOptions { prompt: "build the parser".into(), notify: Some(vec![lead_thread.clone()]), ..Default::default() };
    let kid = rt.rt.sessions().start(&kid_ws, opts, None).await.expect("a start");
    until(|| async { fed.run_of("build the parser").is_some() }).await;
    let run = fed.run_of("build the parser").expect("the run");
    run.push(&[init()]);
    until(|| async { history(&rt, &kid_ws).await.iter().any(|e| e["type"] == "session.start") }).await;
    Setup { rt, backend, state, fed, factory, kid_ws, lead_thread, kid, run }
}

fn strip_head(text: &str) -> String {
    // `e.text.replace(/^thread \w+ finished \([^)]*\): /, "")`
    let Some(rest) = text.strip_prefix("thread ") else { return text.to_string() };
    let Some((word, rest)) = rest.split_once(" finished (") else { return text.to_string() };
    if word.is_empty() || !word.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return text.to_string();
    }
    match rest.split_once("): ") {
        Some((inner, tail)) if !inner.contains(')') => tail.to_string(),
        _ => text.to_string(),
    }
}

impl Setup {
    async fn steer(&self, prompt: &str) -> String {
        let ask = SteerAsk { prompt: prompt.into(), request_id: None, attachments: None };
        let taken = self.rt.rt.sessions().steer(&self.kid.id(), ask, None).await.expect("a steer");
        assert_eq!(j(&taken), serde_json::json!({ "outcome": "accepted" }));
        self.fed.steered("build the parser", self.run.writes().len() - 1)
    }
    /// The lines the child's turns sent its lead, by their words past the thread's head.
    async fn to_lead(&self, on: &Fx) -> Vec<String> {
        history(on, &self.kid_ws)
            .await
            .iter()
            .filter(|e| e["type"] == "session.notify" && e["notify"] == self.lead_thread.as_str())
            .map(|e| strip_head(e["text"].as_str().unwrap_or_default()))
            .collect()
    }
    async fn lead_lines(&self) -> Vec<String> {
        self.to_lead(&self.rt).await
    }
    /// The child's turn is over on the transcript; its run is ended after, as the CLI exits once its input shuts.
    async fn over(&self, code: Option<i32>) {
        until(|| async { history(&self.rt, &self.kid_ws).await.iter().any(|e| e["type"] == "session.done") }).await;
        self.run.end(code);
        self.kid.finished().await;
        sleep_ms(50).await;
    }
    /// The host goes down with the child's run still printing, and a second host on the same store reads it again.
    async fn restart(&self) -> Fx {
        self.rt.rt.close().await.expect("a close");
        let second = host(&self.backend, &self.state, &self.factory);
        second.rt.sessions().list(Some(self.kid_ws.clone()), None).await.expect("a list");
        second
    }
    async fn ended_on(&self, on: &Fx) {
        until(|| async { history(on, &self.kid_ws).await.iter().any(|e| e["type"] == "session.end") }).await;
        sleep_ms(50).await;
    }
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "a plain steer sends one line, the answer, and not the reply before it"
#[test]
fn a_plain_steer_sends_one_line_the_answer_and_not_the_reply_before_it() {
    drive(|| async {
        let s = setup().await;
        let uuid = s.steer(LINE).await;
        s.run.push(&[
            lifecycle(&uuid, "queued"),
            said("msg_1", REPORT),
            result(REPORT, 1, None),
            lifecycle(&uuid, "started"),
            init(),
            said("msg_2", ANSWER),
            result(ANSWER, 1, None),
            lifecycle(&uuid, "completed"),
        ]);
        s.over(Some(0)).await;
        assert_eq!(s.lead_lines().await, [ANSWER]);
        s.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "a process that dies after the answer, before the CLI completes the message, still sends the answer's line"
#[test]
fn a_process_that_dies_after_the_answer_before_the_cli_completes_the_message_still_sends_the_answers_line() {
    drive(|| async {
        let s = setup().await;
        let uuid = s.steer(LINE).await;
        s.run.push(&[
            lifecycle(&uuid, "queued"),
            said("msg_1", REPORT),
            result(REPORT, 1, None),
            lifecycle(&uuid, "started"),
            init(),
            said("msg_2", ANSWER),
            result(ANSWER, 1, None),
        ]);
        sleep_ms(50).await;
        s.run.end(Some(137));
        s.kid.finished().await;
        sleep_ms(50).await;
        assert_eq!(s.lead_lines().await, [ANSWER]);
        s.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "a steer into a reply held over background work sends that reply's line once and the answer's once"
#[test]
fn a_steer_into_a_reply_held_over_background_work_sends_that_replys_line_once_and_the_answers_once() {
    drive(|| async {
        let s = setup().await;
        s.run.push(&[tasks(&["bk1"]), said("msg_1", REPORT), result(REPORT, 3, None)]);
        until(|| async { s.lead_lines().await.len() == 1 }).await;
        let uuid = s.steer(LINE).await;
        s.run.push(&[
            lifecycle(&uuid, "queued"),
            lifecycle(&uuid, "started"),
            init(),
            said("msg_2", ANSWER),
            result(ANSWER, 1, None),
            lifecycle(&uuid, "completed"),
            tasks(&[]),
        ]);
        until(|| async { s.lead_lines().await.len() == 2 }).await;
        // The CLI exits with the work done and its agent not woken again: the reply it was holding is the turn's end.
        s.run.end(Some(0));
        s.kid.finished().await;
        sleep_ms(50).await;
        let got = s.lead_lines().await;
        assert_lines(&got, &[Line::Text(held(REPORT)), Line::Text(held(ANSWER)), Line::Outcome]);
        s.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "the same, with the background work over before the answer: the answer's line goes at the end, once"
#[test]
fn the_same_with_the_background_work_over_before_the_answer_the_answers_line_goes_at_the_end_once() {
    drive(|| async {
        let s = setup().await;
        s.run.push(&[tasks(&["bk1"]), said("msg_1", REPORT), result(REPORT, 3, None)]);
        until(|| async { s.lead_lines().await.len() == 1 }).await;
        let uuid = s.steer(LINE).await;
        s.run.push(&[
            tasks(&[]),
            lifecycle(&uuid, "queued"),
            lifecycle(&uuid, "started"),
            init(),
            said("msg_2", ANSWER),
            result(ANSWER, 1, None),
            lifecycle(&uuid, "completed"),
        ]);
        s.over(Some(0)).await;
        assert_lines(&s.lead_lines().await, &[Line::Text(held(REPORT)), Line::Text(ANSWER.into())]);
        s.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "a steer the woken agent takes at a tool's end, completed before its result as 2.1.295 prints it, sends the answer's line at the end, once"
#[test]
fn a_steer_the_woken_agent_takes_at_a_tools_end_completed_before_its_result_as_2_1_295_prints_it_sends_the_answers_line_at_the_end_once() {
    drive(|| async {
        let s = setup().await;
        s.run.push(&[tasks(&["bk1"]), said("msg_1", REPORT), result(REPORT, 2, None)]);
        until(|| async { s.lead_lines().await.len() == 1 }).await;
        s.run.push(&[tasks(&[]), task_done("bk1"), init()]);
        let uuid = s.steer(LINE).await;
        s.run.push(&[
            lifecycle(&uuid, "queued"),
            lifecycle(&uuid, "started"),
            said("msg_2", ANSWER),
            lifecycle(&uuid, "completed"),
            result(ANSWER, 2, Some("task-notification")),
        ]);
        s.over(Some(0)).await;
        assert_eq!(j(&s.kid.finished().await)["text"], ANSWER);
        assert_lines(&s.lead_lines().await, &[Line::Text(held(REPORT)), Line::Text(ANSWER.into())]);
        s.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "/cost steered into the child's last words leaves the agent's report as the line, and the cost as a row of its own"
#[test]
fn cost_steered_into_the_childs_last_words_leaves_the_agents_report_as_the_line_and_the_cost_as_a_row_of_its_own() {
    drive(|| async {
        let s = setup().await;
        let uuid = s.steer("/cost").await;
        let synthetic = said("ac793b79", COST).replace("claude-haiku-4-5-20251001", "<synthetic>");
        s.run.push(&[
            lifecycle(&uuid, "queued"),
            said("msg_1", REPORT),
            result(REPORT, 1, None),
            lifecycle(&uuid, "started"),
            init(),
            synthetic,
            result(COST, 0, None),
            lifecycle(&uuid, "completed"),
        ]);
        s.over(Some(0)).await;
        assert_eq!(j(&s.kid.finished().await)["text"], REPORT);
        assert_eq!(s.lead_lines().await, [REPORT]);
        let rows: Vec<String> = history(&s.rt, &s.kid_ws)
            .await
            .iter()
            .filter(|e| e["type"] == "session.delta" && e["kind"] == "text")
            .map(|e| e["text"].as_str().unwrap_or_default().to_string())
            .collect();
        assert!(rows.iter().any(|t| t == COST), "{rows:?}");
        s.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "/cost steered over a reply already told for its background work sends the reply no second time, only the outcome the line promised"
#[test]
fn cost_steered_over_a_reply_already_told_for_its_background_work_sends_the_reply_no_second_time_only_the_outcome_the_line_promised() {
    drive(|| async {
        let s = setup().await;
        s.run.push(&[tasks(&["bk1"]), said("msg_1", REPORT), result(REPORT, 3, None)]);
        until(|| async { s.lead_lines().await.len() == 1 }).await;
        let uuid = s.steer("/cost").await;
        let synthetic = said("ac793b79", COST).replace("claude-haiku-4-5-20251001", "<synthetic>");
        s.run.push(&[
            tasks(&[]),
            lifecycle(&uuid, "queued"),
            lifecycle(&uuid, "started"),
            init(),
            synthetic,
            result(COST, 0, None),
            lifecycle(&uuid, "completed"),
        ]);
        s.over(Some(0)).await;
        assert_eq!(j(&s.kid.finished().await)["text"], REPORT);
        assert_lines(&s.lead_lines().await, &[Line::Text(held(REPORT)), Line::Outcome]);
        s.rt.rt.close().await.expect("a close");
    });
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "a reply told over background work is told once across a host restart that reads its run again from the start, and its end sends the outcome"
#[test]
fn a_reply_told_over_background_work_is_told_once_across_a_host_restart_that_reads_its_run_again_from_the_start_and_its_end_sends_the_outcome(
) {
    drive(|| async {
        let s = setup().await;
        s.run.push(&[tasks(&["bk1"]), said("msg_1", REPORT), result(REPORT, 3, None)]);
        until(|| async { s.lead_lines().await.len() == 1 }).await;
        let second = s.restart().await;
        // The work ends and the CLI exits without waking its agent: the reply it held, already told, is the turn's end.
        s.run.push(&[tasks(&[])]);
        sleep_ms(50).await;
        s.run.end(Some(0));
        s.ended_on(&second).await;
        assert_lines(&s.to_lead(&second).await, &[Line::Text(held(REPORT)), Line::Outcome]);
        // The second host read the run to its end: a row the restart had settled would carry no done.
        let done: Vec<String> = history(&second, &s.kid_ws)
            .await
            .iter()
            .filter(|e| e["type"] == "session.done")
            .map(|e| e["result"]["text"].as_str().unwrap_or_default().to_string())
            .collect();
        assert_eq!(done, [REPORT]);
        second.rt.close().await.expect("a close");
    });
}

// "a child's finished lines into its lead, with a message steered into the child's last words" > "a steer whose reply the first host held is answered once on the second, its line sent at the end"
#[test]
fn a_steer_whose_reply_the_first_host_held_is_answered_once_on_the_second_its_line_sent_at_the_end() {
    drive(|| async {
        let s = setup().await;
        let uuid = s.steer(LINE).await;
        s.run.push(&[lifecycle(&uuid, "queued"), said("msg_1", REPORT), result(REPORT, 1, None)]);
        sleep_ms(50).await;
        assert!(s.lead_lines().await.is_empty());
        let second = s.restart().await;
        s.run.push(&[lifecycle(&uuid, "started"), init(), said("msg_2", ANSWER), result(ANSWER, 1, None), lifecycle(&uuid, "completed")]);
        until(|| async { !s.to_lead(&second).await.is_empty() }).await;
        s.run.end(Some(0));
        s.ended_on(&second).await;
        assert_eq!(s.to_lead(&second).await, [ANSWER]);
        second.rt.close().await.expect("a close");
    });
}
