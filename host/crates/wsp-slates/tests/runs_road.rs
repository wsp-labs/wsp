// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_road (spec/runs_road.md): this computer's road and the `then` reshape, against real processes. Every
//! test runs in its own process group (`support::own_group`), so a group kill can never reach the runner.
mod support;

use std::cell::RefCell;
use std::process::Command;
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::json;
use support::{local, own_group, sleep_ms, test_clock, until};
use wsp_slates::slate_runs::{kill_group, local_road, reshape_answer, reshape_result, ReshapeAnswer, ReshapeStart, RoadEnd, RoadEvents, RoadStart, Stream};

/// What a road told: each line with its stream, and every end.
#[derive(Default)]
struct Told {
    lines: RefCell<Vec<(Stream, String)>>,
    ends: RefCell<Vec<RoadEnd>>,
}

impl RoadEvents for Told {
    fn line(&self, stream: Stream, text: &str) {
        self.lines.borrow_mut().push((stream, text.to_string()));
    }
    fn end(&self, end: RoadEnd) {
        self.ends.borrow_mut().push(end);
    }
}

fn env() -> IndexMap<String, String> {
    IndexMap::from([("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()))])
}

fn start(cmd: &str, cwd: &std::path::Path) -> RoadStart {
    RoadStart { cmd: cmd.into(), args: vec![], cwd: cwd.display().to_string(), env: env(), stdin: None, timeout_s: 60.0, stream: false }
}

/// Every pid whose process group is `pgid`, by ps.
fn in_group(pgid: u32) -> Vec<u32> {
    let out = Command::new("ps").args(["-A", "-o", "pid=,pgid="]).output().unwrap();
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| {
            let mut it = l.split_whitespace().map(|n| n.parse::<u32>().ok());
            match (it.next().flatten(), it.next().flatten()) {
                (Some(pid), Some(g)) if g == pgid => Some(pid),
                _ => None,
            }
        })
        .collect()
}

async fn ended(told: &Told, ms: u64) -> RoadEnd {
    assert!(until(ms, || !told.ends.borrow().is_empty()).await, "no end within {ms} ms");
    told.ends.borrow()[0].clone()
}

fn scrub_none() -> Rc<dyn Fn(&str) -> String> {
    Rc::new(|s: &str| s.to_string())
}

// runs_road Behaviour 1: slate-runs.ts:301
#[test]
fn the_command_is_bash_c_with_the_values_as_positional_parameters() {
    own_group();
    let dir = support::temp_root("road-argv-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let mut o = start(r#"printf '%s|' "$0" "$1" "$2""#, dir.path());
        o.args = vec!["a b; touch p1".into(), "$(touch p2)".into()];
        road.start(o, told.clone());
        let end = ended(&told, 10_000).await;
        assert_eq!(end.out, "bash|a b; touch p1|$(touch p2)|");
        assert_eq!((end.code, end.timed_out, end.cut), (Some(0), false, false));
        assert!(!dir.path().join("p1").exists() && !dir.path().join("p2").exists());
    });
}

// runs_road Behaviour 1: the environment is exactly the one given, nothing inherited (slate-runs.ts:301)
#[test]
fn the_environment_is_exactly_the_one_given() {
    own_group();
    std::env::set_var("WSP_LEAK_CHECK", "leaked");
    let dir = support::temp_root("road-env-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let mut o = start("env", dir.path());
        o.env.insert("KEPT".into(), "yes".into());
        road.start(o, told.clone());
        let end = ended(&told, 10_000).await;
        assert!(end.out.contains("KEPT=yes\n"), "{}", end.out);
        assert!(!end.out.contains("WSP_LEAK_CHECK"), "{}", end.out);
        assert!(!end.out.contains("HOME="), "{}", end.out);
    });
}

// runs_road Behaviour 1: no stdin is /dev/null; a given one is written then closed (slate-runs.ts:301, 322-325)
#[test]
fn stdin_is_dev_null_unless_given_and_closed_after_it() {
    own_group();
    let dir = support::temp_root("road-stdin-");
    local(async {
        let road = local_road(test_clock());
        let none = Rc::new(Told::default());
        road.start(start("cat; echo end", dir.path()), none.clone());
        assert_eq!(ended(&none, 10_000).await.out, "end\n");
        let given = Rc::new(Told::default());
        let mut o = start("cat; echo; echo end", dir.path());
        o.stdin = Some("one\ntwo".into());
        road.start(o, given.clone());
        assert_eq!(ended(&given, 10_000).await.out, "one\ntwo\nend\n");
    });
}

// runs_road Behaviour 2: lines as they arrive, the rest at the end, \r kept (slate-runs.ts:315-318, 339-344)
#[test]
fn a_stream_run_tells_each_line_as_it_comes_and_the_rest_at_the_end() {
    own_group();
    let dir = support::temp_root("road-lines-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let mut o = start(r"printf 'a\r\nb\n'; printf 'oops\n' >&2; sleep 0.2; printf 'tail'", dir.path());
        o.stream = true;
        road.start(o, told.clone());
        let end = ended(&told, 10_000).await;
        assert_eq!(*told.lines.borrow(), vec![(Stream::Out, "a\r".to_string()), (Stream::Out, "b".to_string()), (Stream::Err, "oops".to_string()), (Stream::Out, "tail".to_string())]);
        assert_eq!(end.out, "a\r\nb\ntail");
        assert_eq!(end.err, "oops\n");
    });
}

// runs_road Behaviour 2: UTF-8 decoded across chunk edges (slate-runs.ts:306, 316)
#[test]
fn a_character_split_across_two_writes_is_one_line() {
    own_group();
    let dir = support::temp_root("road-utf8-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let mut o = start(r"printf '\303'; sleep 0.2; printf '\251\n'", dir.path());
        o.stream = true;
        road.start(o, told.clone());
        let end = ended(&told, 10_000).await;
        assert_eq!(*told.lines.borrow(), vec![(Stream::Out, "é".to_string())]);
        assert_eq!(end.out, "é\n");
    });
}

// runs_road Behaviour 2: no stream asked, no line told (slate-runs.ts:315)
#[test]
fn a_run_not_streaming_tells_no_line() {
    own_group();
    let dir = support::temp_root("road-nostream-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        road.start(start("echo a; echo b", dir.path()), told.clone());
        assert_eq!(ended(&told, 10_000).await.out, "a\nb\n");
        assert!(told.lines.borrow().is_empty());
    });
}

// runs_road Behaviour 2: one 2 MiB cap shared by out and err, cut set past it (slate-runs.ts:307-314)
#[test]
fn out_and_err_share_one_2_mib_cap_and_say_they_were_cut() {
    own_group();
    let dir = support::temp_root("road-cap-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        road.start(start("head -c 1500000 /dev/zero | tr '\\0' a; head -c 1500000 /dev/zero | tr '\\0' b >&2", dir.path()), told.clone());
        let end = ended(&told, 20_000).await;
        assert!(end.cut);
        assert_eq!(end.out.len() + end.err.len(), 2_097_152);
        assert!(end.out.bytes().all(|b| b == b'a') && end.err.bytes().all(|b| b == b'b'));
        let under = Rc::new(Told::default());
        road.start(start("head -c 2097152 /dev/zero | tr '\\0' a", dir.path()), under.clone());
        let end = ended(&under, 20_000).await;
        assert_eq!((end.out.len(), end.cut), (2_097_152, false));
    });
}

// runs_road Behaviour 2, pilot finding: a streamed line grows only from capped output
#[test]
fn a_streamed_line_never_grows_past_the_cap() {
    own_group();
    let dir = support::temp_root("road-longline-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let mut o = start("head -c 3000000 /dev/zero | tr '\\0' b", dir.path());
        o.stream = true;
        road.start(o, told.clone());
        let end = ended(&told, 20_000).await;
        assert!(end.cut);
        let lines = told.lines.borrow();
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].1.len(), 2_097_152);
    });
}

// runs_road Behaviour 3, 4: the deadline kills the whole group and the end says timed out (slate-runs.ts:329-332)
#[test]
fn the_deadline_kills_the_group_and_ends_timed_out() {
    own_group();
    let dir = support::temp_root("road-deadline-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let mut o = start("echo $$; sleep 30 & sleep 31 & wait", dir.path());
        o.timeout_s = 1.0;
        let at = std::time::Instant::now();
        road.start(o, told.clone());
        let end = ended(&told, 5_000).await;
        assert!(at.elapsed().as_millis() < 5_000);
        assert!(end.timed_out);
        assert_eq!(end.code, None);
        let pgid: u32 = end.out.trim().parse().unwrap();
        assert!(until(3_000, || in_group(pgid).is_empty()).await, "left {:?}", in_group(pgid));
    });
}

// runs_road Behaviour 5: the kill takes every process of the group, and the end still comes (slate-runs.ts:358)
#[test]
fn a_kill_takes_the_whole_group_and_the_end_still_comes_once() {
    own_group();
    let dir = support::temp_root("road-kill-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let mut o = start("echo $$; sleep 300 & sleep 301 & bash -c 'sleep 302' & wait", dir.path());
        o.stream = true;
        let started = road.start(o, told.clone());
        assert!(until(5_000, || !told.lines.borrow().is_empty()).await);
        let pgid: u32 = told.lines.borrow()[0].1.parse().unwrap();
        assert_eq!(started.pid, Some(pgid));
        assert!(until(3_000, || in_group(pgid).len() >= 4).await);
        (started.kill)();
        let end = ended(&told, 5_000).await;
        assert_eq!((end.code, end.signal.as_deref(), end.timed_out), (None, Some("SIGKILL"), false));
        assert!(until(3_000, || in_group(pgid).is_empty()).await);
        sleep_ms(1_200).await;
        assert_eq!(told.ends.borrow().len(), 1);
    });
}

// runs_road Behaviour 4: a grandchild that left the group holding the pipes ends the run 1 s after the exit (slate-runs.ts:348-356)
#[test]
fn a_setsid_grandchild_holding_the_pipes_ends_the_run_after_the_grace() {
    own_group();
    let dir = support::temp_root("road-grace-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let at = std::time::Instant::now();
        road.start(start("setsid sh -c 'echo $$ > held.pid; exec sleep 6' & sleep 0.3; echo leader", dir.path()), told.clone());
        let end = ended(&told, 5_000).await;
        let took = at.elapsed().as_millis();
        assert!((1_000..4_000).contains(&took), "took {took} ms");
        assert_eq!(end.code, Some(0));
        assert!(end.out.starts_with("leader\n"));
        if let Ok(pid) = std::fs::read_to_string(dir.path().join("held.pid")) {
            unsafe { libc::kill(pid.trim().parse::<i32>().unwrap(), libc::SIGKILL) };
        }
        sleep_ms(300).await;
        assert_eq!(told.ends.borrow().len(), 1);
    });
}

// runs_road Decisions 1: a spawn that fails ends with node's message and no code (slate-runs.ts:347)
#[test]
fn a_spawn_that_fails_ends_with_the_error_and_no_code() {
    own_group();
    let dir = support::temp_root("road-spawnfail-");
    local(async {
        let road = local_road(test_clock());
        let told = Rc::new(Told::default());
        let mut o = start("true", dir.path());
        o.cwd = dir.path().join("nowhere").display().to_string();
        road.start(o, told.clone());
        let end = ended(&told, 5_000).await;
        assert_eq!(end.error.as_deref(), Some("spawn bash ENOENT"));
        assert_eq!((end.code, end.out.as_str(), end.err.as_str(), end.timed_out), (None, "", "", false));
    });
}

// runs_road Behaviour 6, rule 6.36: a kill of a group that is gone is ignored
#[test]
fn kill_group_ignores_a_group_that_is_gone() {
    own_group();
    let mut child = Command::new("true").spawn().unwrap();
    let pid = child.id();
    child.wait().unwrap();
    assert!(in_group(pid).is_empty());
    kill_group(pid);
}

// runs_road Behaviour 7, 8: a then reshapes stdin into JSON (slate-runs.ts:246-294)
#[test]
fn a_then_reads_the_input_on_stdin_and_answers_its_json() {
    own_group();
    let dir = support::temp_root("road-then-");
    local(async {
        let o = ReshapeStart { cmd: "cat".into(), input: " {\"a\":[1,\"x\"]}\n".into(), cwd: dir.path().display().to_string(), env: env(), timeout_s: 60.0, scrub: scrub_none() };
        assert_eq!(reshape_result(o, test_clock()).done.await, ReshapeAnswer::Json(json!({"a": [1, "x"]})));
    });
}

// runs_road Behaviour 8: each failure of a then, said as the TS says it (slate-runs.ts:283-294; slate-runs.test.ts:407-412)
#[test]
fn a_then_that_fails_says_why_with_its_stderr() {
    own_group();
    let dir = support::temp_root("road-then-fail-");
    local(async {
        let run = |cmd: &str, timeout_s: f64| ReshapeStart { cmd: cmd.into(), input: "raw\n".into(), cwd: dir.path().display().to_string(), env: env(), timeout_s, scrub: scrub_none() };
        assert_eq!(
            reshape_result(run("cat >/dev/null; echo cannot read it >&2; exit 4", 60.0), test_clock()).done.await,
            ReshapeAnswer::Failed { why: "then exited with 4".into(), exit: Some(4), err: "cannot read it\n".into() }
        );
        assert_eq!(reshape_result(run("echo not json", 60.0), test_clock()).done.await, ReshapeAnswer::Failed { why: "then printed no JSON: not json".into(), exit: Some(0), err: "".into() });
        assert_eq!(reshape_result(run("cat >/dev/null", 60.0), test_clock()).done.await, ReshapeAnswer::Failed { why: "then printed no JSON".into(), exit: Some(0), err: "".into() });
        let at = std::time::Instant::now();
        assert_eq!(reshape_result(run("sleep 30", 1.0), test_clock()).done.await, ReshapeAnswer::Failed { why: "then timed out after 1 s".into(), exit: Some(124), err: "".into() });
        assert!(at.elapsed().as_millis() < 5_000);
    });
}

// runs_road Behaviour 8: the out and err a then answers are scrubbed before the parse
#[test]
fn a_then_parses_its_out_after_the_scrub() {
    own_group();
    let dir = support::temp_root("road-then-scrub-");
    local(async {
        let scrub: Rc<dyn Fn(&str) -> String> = Rc::new(|s: &str| s.replace("tok_9f8e", "[secret:token]"));
        let o = ReshapeStart { cmd: "echo '{\"got\":\"tok_9f8e\"}'; echo 'err tok_9f8e' >&2".into(), input: "".into(), cwd: dir.path().display().to_string(), env: env(), timeout_s: 60.0, scrub: scrub.clone() };
        assert_eq!(reshape_result(o, test_clock()).done.await, ReshapeAnswer::Json(json!({"got": "[secret:token]"})));
        let failing = ReshapeStart { cmd: "echo 'err tok_9f8e' >&2; exit 2".into(), input: "".into(), cwd: dir.path().display().to_string(), env: env(), timeout_s: 60.0, scrub };
        assert_eq!(reshape_result(failing, test_clock()).done.await, ReshapeAnswer::Failed { why: "then exited with 2".into(), exit: Some(2), err: "err [secret:token]\n".into() });
    });
}

// runs_road Behaviour 8: first match wins, in the TS order: error, timeout, code, parse (slate-runs.ts:283-294)
#[test]
fn reshape_answer_says_the_first_reason_in_order() {
    let scrub = |s: &str| s.replace("sekrit", "[secret:s]");
    let base = RoadEnd { code: Some(0), out: "{}".into(), err: "e sekrit".into(), ..Default::default() };
    let err = "e [secret:s]".to_string();
    assert_eq!(
        reshape_answer(&RoadEnd { error: Some("spawn bash EACCES sekrit".into()), timed_out: true, code: None, ..base.clone() }, 60.0, &scrub),
        ReshapeAnswer::Failed { why: "then could not start: spawn bash EACCES [secret:s]".into(), exit: None, err: err.clone() }
    );
    assert_eq!(reshape_answer(&RoadEnd { timed_out: true, code: Some(3), ..base.clone() }, 2.5, &scrub), ReshapeAnswer::Failed { why: "then timed out after 2.5 s".into(), exit: Some(124), err: err.clone() });
    assert_eq!(reshape_answer(&RoadEnd { code: None, signal: Some("SIGKILL".into()), ..base.clone() }, 60.0, &scrub), ReshapeAnswer::Failed { why: "then exited with a signal".into(), exit: None, err: err.clone() });
    assert_eq!(reshape_answer(&base, 60.0, &scrub), ReshapeAnswer::Json(json!({})));
    let long = format!("  {}\nsecond line", "x".repeat(130));
    assert_eq!(reshape_answer(&RoadEnd { out: long, ..base.clone() }, 60.0, &scrub), ReshapeAnswer::Failed { why: format!("then printed no JSON: {}", "x".repeat(120)), exit: Some(0), err });
}

// runs_road Behaviour 8: err is cut to its 64 KiB tail from a line start (run_output_tail)
#[test]
fn reshape_answer_keeps_the_tail_of_a_long_stderr() {
    let err: String = (0..20_000).map(|i| format!("line {i:05}\n")).collect();
    let answer = reshape_answer(&RoadEnd { code: Some(1), err: err.clone(), ..Default::default() }, 60.0, &|s: &str| s.to_string());
    let ReshapeAnswer::Failed { why, exit, err: kept } = answer else { panic!("not failed") };
    assert_eq!((why.as_str(), exit), ("then exited with 1", Some(1)));
    assert!(kept.len() <= 65_536 && kept.starts_with("line ") && kept.ends_with("line 19999\n"));
}
