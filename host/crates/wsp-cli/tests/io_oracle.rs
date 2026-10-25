// SPDX-License-Identifier: AGPL-3.0-only
//! `verbs/io.rs` against what `packages/host/src/verbs/io.ts` answered (tests/fixtures/io.json, io_cloud.json,
//! written by "the io helpers" of oracle/cli.oracle.ts). Every test collects all its mismatches, each named by its
//! case, and panics once.

#[path = "cli_a_common/mod.rs"]
mod common;

use std::collections::BTreeSet;
use std::fmt::Display;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Arc;

use serde_json::{json, Value};

use common::{fixture, RecordingIo};
use wsp_cli::cloud_text::cloud_text;
use wsp_cli::verbs::client::{CallToolResult, FlagRow, FlagValue, Flags, InputKind, ToolInput};
use wsp_cli::verbs::io::{self, NotifyLength, TurnOut};
use wsp_cli::verbs::turns_help::{Ended, Turn, Waited};
use wsp_words::exit::{verb_failure, HostError, Kind};
use wsp_wire::views::project_bundle::{HostFolderListing, ProjectPlan};
use wsp_wire::views::session::{SessionView, ThreadView};
use wsp_wire::views::session_events::TurnResult;
use wsp_wire::views::sub_row::SubResult;
use wsp_wire::wire::runtime::SessionStartOutcome;

// ---------------------------------------------------------------------------------------------------------------
// The harness: every case checked, every mismatch named.
// ---------------------------------------------------------------------------------------------------------------

#[derive(Default)]
struct Misses(Vec<String>);

impl Misses {
    fn case(&mut self, name: impl Display, f: impl FnOnce() -> Result<(), String>) {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(Ok(())) => {}
            Ok(Err(why)) => self.0.push(format!("{name}: {why}")),
            Err(panic) => self.0.push(format!("{name}: panicked: {}", panic_text(&*panic))),
        }
    }

    fn miss(&mut self, name: impl Display, why: impl Display) {
        self.0.push(format!("{name}: {why}"));
    }

    fn done(self) {
        assert!(self.0.is_empty(), "{} case(s) wrong:\n{}", self.0.len(), self.0.join("\n"));
    }
}

fn panic_text(panic: &(dyn std::any::Any + Send)) -> String {
    panic.downcast_ref::<String>().cloned().or_else(|| panic.downcast_ref::<&str>().map(|s| (*s).to_owned())).unwrap_or_else(|| "?".to_owned())
}

fn same(got: &Value, want: &Value) -> Result<(), String> {
    if got == want {
        Ok(())
    } else {
        Err(format!("got {got}\n    want {want}"))
    }
}

/// Equal as the text a client reads, keys in order.
fn same_in_order(got: &Value, want: &Value) -> Result<(), String> {
    let (got, want) = (serde_json::to_string(got).unwrap(), serde_json::to_string(want).unwrap());
    if got == want {
        Ok(())
    } else {
        Err(format!("got {got}\n    want {want}"))
    }
}

fn same_text(got: &str, want: &str) -> Result<(), String> {
    if got == want {
        Ok(())
    } else {
        Err(format!("got {got:?}\n    want {want:?}"))
    }
}

fn same_lines(got: &[String], want: &Value) -> Result<(), String> {
    same(&json!(got), want)
}

/// The failure object the oracle's `caught` wrote: `verbFailure(e)` plus the kind and fix stamped on the error.
fn failure(e: &HostError) -> Value {
    json!({
        "error": e.message,
        "class": e.class().as_str(),
        "exit": e.exit_code(),
        "kind": e.kind.as_ref().map(Kind::as_str),
        "fix": e.fix,
    })
}

/// `{ok}` or `{err}` as the oracle's `caught` writes it.
fn settled<T>(r: Result<T, HostError>, ok: impl FnOnce(T) -> Value) -> Value {
    match r {
        Ok(v) => json!({ "ok": ok(v) }),
        Err(e) => json!({ "err": failure(&e) }),
    }
}

/// A whole-number f64 as the integer JSON writes it, where JS has only one number type.
fn num(n: f64) -> Value {
    if n.is_finite() && n.fract() == 0.0 && n.abs() < 9e15 {
        json!(n as i64)
    } else {
        json!(n)
    }
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_owned()).collect()
}

fn plan() -> ProjectPlan {
    serde_json::from_value(fixture("io", false)["plan"].clone()).unwrap_or_else(|e| panic!("the fixture plan does not parse: {e}"))
}

fn empty_plan() -> ProjectPlan {
    serde_json::from_value(fixture("io", false)["empty"].clone()).unwrap_or_else(|e| panic!("the fixture's empty plan does not parse: {e}"))
}

/// A `ToolInput` as the oracle's `field()` writes it, its describe cut for the cloud given.
fn input_json(i: &ToolInput, cloud: bool) -> Value {
    let kind = match &i.kind {
        InputKind::String => json!({ "type": "string" }),
        InputKind::Boolean => json!({ "type": "boolean" }),
        InputKind::Number => json!({ "type": "number" }),
        InputKind::Integer { min } => json!({ "type": "integer", "min": min }),
        InputKind::StringList { min } => json!({ "type": "stringList", "min": min }),
        InputKind::Enum(options) => json!({ "type": "enum", "options": options }),
        InputKind::Literal(v) => json!({ "type": "literal", "value": v }),
        InputKind::Json => json!({ "type": "json" }),
    };
    json!({ "name": i.name, "kind": kind, "optional": i.optional, "describe": i.describe.as_deref().map(|d| cloud_text(d, cloud)) })
}

/// The oracle gave a held start's `capped` no `placeId` and a wait's `sub` the key `sha`, which the wire shapes need
/// and name otherwise; both are filled the same way into what goes in and what is expected out.
fn filled(v: &Value) -> Value {
    match v {
        Value::Object(o) if o.get("capped").is_some_and(|c| c.get("place").is_some() && c.get("placeId").is_none()) => {
            let mut out = serde_json::Map::new();
            for (k, item) in o {
                if k == "capped" {
                    let c = item.as_object().unwrap();
                    out.insert(k.clone(), json!({ "placeId": "place-mini", "place": c["place"], "running": c["running"], "atOnce": c["atOnce"] }));
                } else {
                    out.insert(k.clone(), filled(item));
                }
            }
            Value::Object(out)
        }
        Value::Object(o) if o.contains_key("sha") && o.contains_key("branch") => {
            json!({ "branch": o["branch"], "pushedSha": o["sha"], "commits": o["commits"], "unpushed": 0, "changed": 0, "readAt": "2026-10-25T00:00:00.000Z" })
        }
        Value::Object(o) => Value::Object(o.iter().map(|(k, item)| (k.clone(), filled(item))).collect()),
        Value::Array(a) => Value::Array(a.iter().map(filled).collect()),
        other => other.clone(),
    }
}

fn parsed<T: serde::de::DeserializeOwned>(what: &str, v: &Value) -> T {
    serde_json::from_value(v.clone()).unwrap_or_else(|e| panic!("the fixture's {what} does not parse: {e}\n{v}"))
}

fn turn_of(v: &Value) -> Turn {
    Turn {
        session: parsed::<SessionView>("session", &v["session"]),
        thread_id: v["threadId"].as_str().unwrap().to_owned(),
        outcome: parsed::<SessionStartOutcome>("outcome", &v["outcome"]),
        result: v.get("result").map(|r| parsed::<TurnResult>("result", r)),
        reason: None,
        after_cut: v.get("afterCut") == Some(&json!(true)),
        notice: None,
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The plan.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn secrets_chosen_takes_the_plans_default_ticks_plus_keep_less_cut() {
    // The oracle sorted the TS Set; a BTreeSet is sorted, so the order here is the sort, not the Set's insertion order.
    let fx = fixture("io", false);
    let plan = plan();
    let mut m = Misses::default();
    for c in fx["chosen"].as_array().unwrap() {
        let (keep, cut) = (strings(&c["keep"]), strings(&c["cut"]));
        m.case(format!("keep {keep:?} cut {cut:?}"), || {
            same(&settled(io::secrets_chosen(&plan, &keep, &cut), |s| json!(s.into_iter().collect::<Vec<_>>())), &c["out"])
        });
    }
    let empty = empty_plan();
    m.case("a plan with no secrets, keep x", || {
        same(&settled(io::secrets_chosen(&empty, &["x".to_owned()], &[]), |s| json!(s.into_iter().collect::<Vec<_>>())), &fx["chosenEmpty"])
    });
    m.done();
}

#[test]
fn agents_chosen_takes_the_default_or_each_named_agent_that_travels() {
    // The oracle sorted the TS Set; a BTreeSet is sorted, so the order here is the sort, not the Set's insertion order.
    let fx = fixture("io", false);
    let plan = plan();
    let mut m = Misses::default();
    for c in fx["agents"].as_array().unwrap() {
        let named: Option<Vec<String>> = if c["named"].is_null() { None } else { Some(strings(&c["named"])) };
        m.case(format!("named {named:?}"), || {
            same(&settled(io::agents_chosen(&plan, named.as_deref()), |s| json!(s.into_iter().collect::<Vec<_>>())), &c["out"])
        });
    }
    let empty = empty_plan();
    m.case("a plan with no agents, named claude", || {
        same(&settled(io::agents_chosen(&empty, Some(&["claude".to_owned()])), |s| json!(s.into_iter().collect::<Vec<_>>())), &fx["agentsEmpty"])
    });
    m.done();
}

#[test]
fn plan_lines_are_the_oracles_lines() {
    let fx = fixture("io", false);
    let (plan, empty) = (plan(), empty_plan());
    let set = |words: &[&str]| words.iter().map(|w| (*w).to_owned()).collect::<BTreeSet<String>>();
    let mut m = Misses::default();
    // chosen[0] and agents[0] of the same fixture say what the defaults are.
    m.case("planLines full (the default ticks and agents)", || same_lines(&io::plan_lines(&plan, &set(&[".git/config"]), &set(&["claude", "opencode"]), None), &fx["planLines"]["full"]));
    m.case("planLines ticked (every secret ticked, no agent)", || {
        same_lines(&io::plan_lines(&plan, &set(&[".env", "deploy/key.pem", ".git/config"]), &set(&[]), None), &fx["planLines"]["ticked"])
    });
    m.case("planLines empty", || same_lines(&io::plan_lines(&empty, &set(&[]), &set(&[]), None), &fx["planLines"]["empty"]));
    m.done();
}

#[test]
fn folder_lines_are_the_oracles_lines() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    m.case("folderLines full", || {
        let listing: HostFolderListing = parsed("listing", &fx["listing"]);
        same_lines(&io::folder_lines(&listing), &fx["folderLines"]["full"])
    });
    m.case("folderLines empty", || {
        let listing: HostFolderListing = parsed("empty listing", &fx["emptyListing"]);
        same_lines(&io::folder_lines(&listing), &fx["folderLines"]["empty"])
    });
    m.done();
}

// ---------------------------------------------------------------------------------------------------------------
// The flags.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn timeout_flag_reads_seconds_above_zero_as_milliseconds() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    for c in fx["flags"]["timeout"].as_array().unwrap() {
        let value = c["value"].as_str();
        m.case(format!("--timeout {value:?}"), || same(&settled(io::timeout_flag(value), |ms| ms.map_or(Value::Null, num)), &c["out"]));
    }
    m.done();
}

#[test]
fn agents_flag_splits_on_commas_and_holds_each_to_the_catalog() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    for c in fx["flags"]["agents"].as_array().unwrap() {
        let value = c["value"].as_str();
        m.case(format!("--agents {value:?}"), || same(&settled(io::agents_flag(value), |a| json!(a)), &c["out"]));
    }
    m.done();
}

#[test]
fn scheme_flag_takes_light_or_dark() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    for c in fx["flags"]["scheme"].as_array().unwrap() {
        let value = c["value"].as_str();
        m.case(format!("--scheme {value:?}"), || same(&settled(io::scheme_flag(value), |s| serde_json::to_value(s).unwrap()), &c["out"]));
    }
    m.done();
}

#[test]
fn project_folders_refuses_the_first_relative_folder() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    for c in fx["flags"]["projectFolders"].as_array().unwrap() {
        let folders = strings(&c["folders"]);
        m.case(format!("project folders {folders:?}"), || same(&settled(io::project_folders(&folders), |f| json!(f)), &c["out"]));
    }
    m.done();
}

#[test]
fn projects_flag_resolves_each_project_against_the_folder_the_line_runs_in() {
    // The oracle resolved against the TS process's own folder, which the fixture shows in its first entry; the relative
    // entries only hold for that folder, so it is read back off the fixture and the absolute one is compared as is.
    let fx = fixture("io", false);
    let want = fx["projectsFlag"][0]["projects"].as_array().unwrap().clone();
    let oracle_cwd = want[0].as_str().unwrap().strip_suffix("/a").expect("the first project is the relative a").to_owned();
    let list = |words: &[&str]| -> Flags {
        let mut flags = Flags::new();
        flags.insert("project".to_owned(), FlagValue::List(words.iter().map(|w| (*w).to_owned()).collect()));
        flags
    };
    let mut m = Misses::default();
    m.case("--project a /b ../c in the oracle's folder", || same(&json!(io::projects_flag(&list(&["a", "/b", "../c"]), &oracle_cwd)), &json!(want)));
    m.case("the absolute entry stays as it is", || {
        let got = io::projects_flag(&list(&["a", "/b", "../c"]), &oracle_cwd).ok_or("None for a list")?;
        same_text(&got[1], want[1].as_str().unwrap())
    });
    m.case("--project a /b ../c in /root", || same(&json!(io::projects_flag(&list(&["a", "/b", "../c"]), "/root")), &json!(["/root/a", "/b", "/c"])));
    m.case("--project given once as a word is not a list", || {
        let mut flags = Flags::new();
        flags.insert("project".to_owned(), FlagValue::Str("a".to_owned()));
        same(&json!(io::projects_flag(&flags, "/root")), &fx["projectsFlag"][1].get("projects").cloned().unwrap_or(Value::Null))
    });
    m.case("no --project", || same(&json!(io::projects_flag(&Flags::new(), "/root")), &fx["projectsFlag"][2].get("projects").cloned().unwrap_or(Value::Null)));
    m.done();
}

#[test]
fn reason_flag_is_one_string_flag() {
    let fx = fixture("io", false);
    let table = io::reason_flag();
    let mut m = Misses::default();
    m.case("REASON_FLAG rows", || {
        let rows: Vec<&str> = table.0.iter().map(|(name, _)| *name).collect();
        same(&json!(rows), &json!(fx["reasonFlag"].as_array().unwrap().iter().map(|r| r[0].clone()).collect::<Vec<_>>()))
    });
    m.case("REASON_FLAG reason is {type: string}", || {
        let row = table.get("reason").ok_or("no reason row")?;
        if *row == FlagRow::STRING {
            Ok(())
        } else {
            Err(format!("got {row:?}"))
        }
    });
    m.done();
}

#[test]
fn with_replace_hint_adds_the_hint_to_an_exists_error_and_keeps_the_rest() {
    // The oracle's `same` (the very Error object came back) is a JS identity; only message and kind are compared.
    let fx = fixture("io", false);
    let mut m = Misses::default();
    for c in fx["replaced"].as_array().unwrap() {
        let message = c["message"].as_str().unwrap();
        m.case(format!("withReplaceHint {message:?} kind {}", c["kind"]), || {
            let mut e = HostError::new(message);
            e.kind = c["kind"].as_str().map(Kind::parse);
            let r = io::with_replace_hint(e);
            same(&json!({ "message": r.message, "kind": r.kind.as_ref().map(Kind::as_str) }), &json!({ "message": c["out"]["message"], "kind": c["out"]["kind"] }))
        });
    }
    m.done();
}

// ---------------------------------------------------------------------------------------------------------------
// The one confirmation.
// ---------------------------------------------------------------------------------------------------------------

#[tokio::test]
async fn confirmed_at_asks_once_at_a_terminal_and_refuses_off_one() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    for c in fx["confirms"].as_array().unwrap() {
        let (yes, tty, answer) = (c["yes"].as_bool().unwrap(), c["tty"].as_bool().unwrap(), c["answer"].as_str().unwrap().to_owned());
        let name = format!("confirmedAt yes {yes} tty {tty} answer {answer:?}");
        let io = Arc::new(RecordingIo { tty, answer, ..RecordingIo::default() });
        let at = io.clone();
        let joined = tokio::spawn(async move { io::confirmed_at(&*at, yes, "Remove the worktree for b in p?\nThe branch stays.", "b").await }).await;
        let got = match joined {
            Ok(r) => r,
            Err(e) => {
                m.miss(&name, format!("panicked: {e}"));
                continue;
            }
        };
        let out = match got {
            Ok(b) => json!({ "ok": b }),
            Err(e) => json!({ "err": serde_json::to_value(verb_failure(&e)).unwrap() }),
        };
        m.case(format!("{name}: the value"), || same(&out, &c["out"]));
        m.case(format!("{name}: what was asked"), || same_lines(&io.asked(), &c["asked"]));
        m.case(format!("{name}: what went to stderr"), || same_lines(&io.errors(), &c["errors"]));
    }
    m.done();
}

// ---------------------------------------------------------------------------------------------------------------
// The tool door's answers.
// ---------------------------------------------------------------------------------------------------------------

#[test]
fn turn_out_is_the_turns_view_or_its_refusal() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    for (n, c) in fx["turnOuts"].as_array().unwrap().iter().enumerate() {
        let c = filled(c);
        let name = format!("turn {n} ({} {})", c["turn"]["outcome"], c["turn"].get("result").map_or("no result".to_owned(), |r| r["status"].to_string()));
        m.case(name, || {
            let out = io::turn_out(&turn_of(&c["turn"]));
            match (&out, c["out"].get("ok"), c["out"].get("err")) {
                (Ok(got), Some(want), _) => same_in_order(&serde_json::to_value(got).unwrap(), want),
                (Err(e), _, Some(want)) => same(&failure(e), want),
                (got, ..) => Err(format!("got {got:?}\n    want {}", c["out"])),
            }
        });
    }
    m.done();
}

#[test]
fn detached_out_names_the_thread_and_what_holds_a_start_back() {
    let fx = fixture("io", false);
    let opened = |id: &str, folder: Option<&str>| format!("opened {id} in {}", folder.unwrap_or("-"));
    let mut m = Misses::default();
    for (n, c) in fx["turnOuts"].as_array().unwrap().iter().enumerate() {
        let c = filled(c);
        let turn = turn_of(&c["turn"]);
        let name = format!("turn {n} ({})", c["turn"]["outcome"]);
        m.case(format!("{name} detached"), || same_in_order(&serde_json::to_value::<CallToolResult>(io::detached_out(&turn, None, None, None)).unwrap(), &c["detached"]));
        m.case(format!("{name} detached with a way to open it and the line a wake left"), || {
            let got = io::detached_out(&turn, Some(&opened), None, Some("the disk was gone"));
            same_in_order(&serde_json::to_value(got).unwrap(), &c["detachedOpened"])
        });
    }
    m.done();
}

#[test]
fn turn_text_and_noticed_follow_the_oracle() {
    // The oracle's inputs hold their keys in no fixed order, so noticed is compared as values, not as text.
    let fx = fixture("io", false);
    let mut m = Misses::default();
    for (n, c) in fx["texts"].as_array().unwrap().iter().enumerate() {
        let out: TurnOut = parsed("turn out", &c["out"]);
        m.case(format!("text {n}: turnText"), || same_text(&io::turn_text(&out), c["text"].as_str().unwrap()));
        m.case(format!("text {n}: noticed said"), || same(&serde_json::to_value(io::noticed(out.clone(), Some("said"))).unwrap(), &c["noticed"]));
        m.case(format!("text {n}: noticed nothing"), || same(&serde_json::to_value(io::noticed(out.clone(), None)).unwrap(), &c["kept"]));
    }
    m.done();
}

#[test]
fn wait_answer_is_the_end_under_the_notify_line_or_the_timeout() {
    let fx = fixture("io", false);
    let thread = |id: &str, thread_id: Option<&str>| -> ThreadView {
        let mut v = json!({ "id": id, "workspaceId": "ws-1", "harness": "claude", "startedBy": "agent", "status": "running", "title": "", "sessionId": "", "turns": 0, "ran": false });
        if let Some(t) = thread_id {
            v["threadId"] = json!(t);
        }
        parsed("named thread", &v)
    };
    let mut m = Misses::default();
    for (n, c) in fx["waits"].as_array().unwrap().iter().enumerate() {
        let c = filled(c);
        let length = c["length"].as_str().unwrap().to_owned();
        m.case(format!("wait {n} ({length})"), || {
            let named = [thread("t-1", Some("t-1")), thread("s-2", None)];
            let waited = match c["waited"].get("ended") {
                Some(e) => Waited::Ended(Ended {
                    thread_id: e["threadId"].as_str().unwrap().to_owned(),
                    result: parsed::<TurnResult>("result", &e["result"]),
                    sub: e.get("sub").map(|s| parsed::<SubResult>("sub", s)),
                }),
                None => Waited::TimedOutMs(c["waited"]["timedOutMs"].as_f64().unwrap()),
            };
            let (value, line) = io::wait_answer(&named, &waited, if length == "whole" { NotifyLength::Whole } else { NotifyLength::Tail });
            same_in_order(&serde_json::to_value(&value).unwrap(), &c["out"]["value"])?;
            same_text(&line, c["out"]["line"].as_str().unwrap())
        });
    }
    m.done();
}

#[test]
fn as_json_and_as_text_wrap_the_structured_value() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    m.case("asJson", || {
        let got = io::as_json(json!({ "a": 1, "b": { "c": ["x", "\u{85}"] } }));
        same_in_order(&serde_json::to_value(got).unwrap(), &fx["asJson"])
    });
    m.case("asText", || same_in_order(&serde_json::to_value(io::as_text("line", json!({ "a": 1 }))).unwrap(), &fx["asText"]));
    m.done();
}

#[test]
fn the_words_a_start_and_an_import_say() {
    let fx = fixture("io", false);
    let mut m = Misses::default();
    m.case("SEND_MEETS", || same_text(&io::send_meets(), fx["sendMeets"].as_str().unwrap()));
    m.case("CAP_MEETS", || same_text(&io::cap_meets(), fx["capMeets"].as_str().unwrap()));
    m.case("PLAN_ONLY", || same_text(io::PLAN_ONLY, fx["planOnly"].as_str().unwrap()));
    m.case("IMPORT_NOW", || same_text(io::IMPORT_NOW, fx["importNow"].as_str().unwrap()));
    m.done();
}

// ---------------------------------------------------------------------------------------------------------------
// The tool inputs.
// ---------------------------------------------------------------------------------------------------------------

fn inputs_match(m: &mut Misses, what: &str, got: &[ToolInput], key: &str) {
    for cloud in [false, true] {
        let fx = fixture("io", cloud);
        let want = fx[key].as_array().unwrap();
        m.case(format!("{what} (cloud {cloud}): how many"), || same(&json!(got.len()), &json!(want.len())));
        for (i, w) in want.iter().enumerate() {
            m.case(format!("{what} {} (cloud {cloud})", w["name"]), || {
                let got = got.get(i).ok_or("missing")?;
                same(&input_json(got, cloud), w)
            });
        }
    }
}

#[test]
fn pick_inputs_are_model_effort_and_access() {
    let mut m = Misses::default();
    match catch_unwind(io::pick_inputs) {
        Ok(got) => inputs_match(&mut m, "PICK_INPUTS", &got, "pickInputs"),
        Err(p) => m.miss("PICK_INPUTS", format!("panicked: {}", panic_text(&*p))),
    }
    m.done();
}

#[test]
fn send_inputs_are_model_and_effort() {
    let mut m = Misses::default();
    match catch_unwind(io::send_inputs) {
        Ok(got) => inputs_match(&mut m, "SEND_INPUTS", &got, "sendInputs"),
        Err(p) => m.miss("SEND_INPUTS", format!("panicked: {}", panic_text(&*p))),
    }
    m.done();
}

#[test]
fn every_shared_tool_input_is_the_field_the_oracle_recorded() {
    let table: Vec<(&'static str, fn(&'static str) -> ToolInput)> = vec![
        ("WorkspaceIn", io::workspace_in),
        ("ThreadIn", io::thread_in),
        ("AgentIn", io::agent_in),
        ("NotifyIn", io::notify_in),
        ("RunProjectIn", io::run_project_in),
        ("BesideIn", io::beside_in),
        ("BranchIn", io::branch_in),
        ("OnIn", io::on_in),
        ("SubSizeIn", io::sub_size_in),
        ("SubFromIn", io::sub_from_in),
        ("RunCwdIn", io::run_cwd_in),
        ("ExecCwdIn", io::exec_cwd_in),
        ("CwdIn", io::cwd_in),
        ("DetachIn", io::detach_in),
        ("ResumeIn", io::resume_in),
        ("CopyIn", io::copy_in),
        ("TitleIn", io::title_in),
        ("ForkIn", io::fork_in),
        ("AtIn", io::at_in),
        ("ReplacesIn", io::replaces_in),
        ("FilesIn", io::files_in),
        ("FastIn", io::fast_in),
        ("ConfirmIn", io::confirm_in),
        ("SizeIn", io::size_in),
        ("MaxMachinesIn", io::max_machines_in),
        ("MaxDepthIn", io::max_depth_in),
        ("PROJECT_FOLDERS", io::project_folders_in),
        ("WEIGH_BY_FOLDERS", io::weigh_by_folders_in),
    ];
    let mut m = Misses::default();
    for cloud in [false, true] {
        let fx = fixture("io", cloud);
        let shared = fx["shared"].as_array().unwrap();
        m.case(format!("shared (cloud {cloud}): the names, in the oracle's order"), || {
            same(&json!(table.iter().map(|(n, _)| *n).collect::<Vec<_>>()), &json!(shared.iter().map(|s| s["name"].clone()).collect::<Vec<_>>()))
        });
        for want in shared {
            let name = want["name"].as_str().unwrap();
            m.case(format!("{name} (cloud {cloud})"), || {
                let (_, make) = table.iter().find(|(n, _)| *n == name).ok_or("no function for it in this test")?;
                same(&input_json(&make(table.iter().find(|(n, _)| *n == name).unwrap().0), cloud), want)
            });
        }
    }
    m.done();
}
