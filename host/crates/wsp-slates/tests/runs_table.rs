// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_table (spec/runs_table.md): slate-runs.test.ts's table cases, run for real in a temp folder, and the
//! large-module pilot's review findings (#2440, 6396255198) as exact tests. Tests that start processes run in their
//! own process group.
mod support;

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::{json, Value};
use support::{local, own_group, sleep_ms, test_clock, until, ManualClock, THREAD};
use wsp_core::clock::Clock;
use wsp_slates::slate_runs::{
    create_slate_runs, run_key, ApproveScope, CmdRunDecl, Outcome, Reshape, ReshapeAnswer, ReshapeStart, RoadEnd, RoadEvents, RoadStart,
    RoadStarted, RunApprovals, RunBy, RunInput, RunInputs, RunRecord, RunRoad, RunStart, RunStartAnswer, RunState, SlateRuns,
    SlateRunsDeps, Stream, TimerDecl, BACKED_OFF,
};
use wsp_wire::types::js_number::JsNumber;

type Hook = Rc<dyn Fn(&str, &RunRecord)>;

/// The runs table's deps, recording what it was told.
struct Deps {
    env: RefCell<IndexMap<String, String>>,
    records: RefCell<Vec<(String, RunRecord)>>,
    lines: RefCell<Vec<String>>,
    timers: RefCell<Vec<String>>,
    clock: Rc<dyn Clock>,
    road: RefCell<Option<Rc<dyn RunRoad>>>,
    owned: RefCell<Vec<(String, u32, String, Rc<RefCell<bool>>)>>,
    own_on: bool,
    secrets_file: Option<PathBuf>,
    on_timer: RefCell<Option<Rc<dyn Fn(&str)>>>,
    on_record: RefCell<Option<Hook>>,
    host_refused: RefCell<Option<String>>,
    dir: RefCell<Option<Result<String, String>>>,
    approvals: Option<Rc<dyn RunApprovals>>,
}

impl SlateRunsDeps for Deps {
    fn env(&self) -> IndexMap<String, String> {
        self.env.borrow().clone()
    }
    fn on_record(&self, _thread_id: &str, run: &str, record: RunRecord) {
        self.records.borrow_mut().push((run.to_string(), record.clone()));
        let hook = self.on_record.borrow().clone();
        if let Some(h) = hook {
            h(run, &record);
        }
    }
    fn on_line(&self, _thread_id: &str, _run: &str, line: &str, _stream: Stream) {
        self.lines.borrow_mut().push(line.to_string());
    }
    fn on_timer(&self, _thread_id: &str, run: &str) {
        self.timers.borrow_mut().push(run.to_string());
        let hook = self.on_timer.borrow().clone();
        if let Some(h) = hook {
            h(run);
        }
    }
    fn approvals(&self) -> Option<Rc<dyn RunApprovals>> {
        self.approvals.clone()
    }
    fn secrets_file(&self) -> Option<PathBuf> {
        self.secrets_file.clone()
    }
    fn dir(&self, _thread_id: &str) -> Option<Result<String, String>> {
        self.dir.borrow().clone()
    }
    fn road(&self, _thread_id: &str, _on: &str) -> Option<Rc<dyn RunRoad>> {
        self.road.borrow().clone()
    }
    fn own(&self, thread_id: &str, pid: u32, kind: &str) -> Option<Rc<dyn Fn()>> {
        if !self.own_on {
            return None;
        }
        let released = Rc::new(RefCell::new(false));
        self.owned.borrow_mut().push((thread_id.to_string(), pid, kind.to_string(), released.clone()));
        Some(Rc::new(move || *released.borrow_mut() = true))
    }
    fn host_refused(&self, _thread_id: &str, _run: &str) -> Option<String> {
        self.host_refused.borrow().clone()
    }
    fn clock(&self) -> Rc<dyn Clock> {
        self.clock.clone()
    }
}

struct H {
    deps: Rc<Deps>,
    runs: Rc<dyn SlateRuns>,
    dir: PathBuf,
}

fn deps(dir: &Path, clock: Rc<dyn Clock>) -> Deps {
    Deps {
        env: RefCell::new(IndexMap::from([
            ("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())),
            ("HOME".to_string(), dir.display().to_string()),
        ])),
        records: RefCell::new(vec![]),
        lines: RefCell::new(vec![]),
        timers: RefCell::new(vec![]),
        clock,
        road: RefCell::new(None),
        owned: RefCell::new(vec![]),
        own_on: false,
        secrets_file: None,
        on_timer: RefCell::new(None),
        on_record: RefCell::new(None),
        host_refused: RefCell::new(None),
        dir: RefCell::new(None),
        approvals: None,
    }
}

fn harness_with(dir: &Path, d: Deps) -> H {
    let deps = Rc::new(d);
    let runs = create_slate_runs(deps.clone());
    H { deps, runs, dir: dir.to_path_buf() }
}

fn harness(dir: &Path) -> H {
    harness_with(dir, deps(dir, test_clock()))
}

fn cmd(text: &str) -> CmdRunDecl {
    CmdRunDecl { cmd: text.into(), ..Default::default() }
}

fn env_of(pairs: &[(&str, &str)]) -> IndexMap<String, Value> {
    pairs.iter().map(|(k, v)| (k.to_string(), json!(v))).collect()
}

fn inputs(i: RunInputs) -> Rc<dyn Fn() -> RunInputs> {
    Rc::new(move || i.clone())
}

impl H {
    fn req(&self, run: &str, decl: CmdRunDecl, i: RunInputs, by: RunBy) -> RunStart {
        RunStart {
            thread_id: THREAD.into(),
            run: run.into(),
            decl,
            by,
            folder: self.dir.display().to_string(),
            inputs: inputs(i),
            last: None,
            asleep: false,
        }
    }
    fn start(&self, run: &str, decl: CmdRunDecl, i: RunInputs, by: RunBy) -> RunStartAnswer {
        self.runs.start(self.req(run, decl, i, by))
    }
    fn last(&self) -> RunRecord {
        self.deps.records.borrow().last().unwrap().1.clone()
    }
    fn of(&self, run: &str) -> Vec<RunRecord> {
        self.deps.records.borrow().iter().filter(|(r, _)| r == run).map(|(_, rec)| rec.clone()).collect()
    }
    /// The first record of `run` in one of `states` written from index `from` on.
    async fn until_from(&self, from: usize, run: &str, states: &[RunState], ms: u64) -> RunRecord {
        let found = || {
            self.deps.records.borrow()[from..]
                .iter()
                .find(|(r, rec)| r == run && rec.state().is_some_and(|s| states.contains(&s)))
                .map(|(_, rec)| rec.clone())
        };
        assert!(until(ms, || found().is_some()).await, "{run} reached none of {states:?}; records {:?}", self.deps.records.borrow());
        found().unwrap()
    }
    /// Starts it, approves it for the thread, and answers its finished record.
    async fn run_allowed(&self, run: &str, decl: CmdRunDecl, i: RunInputs) -> RunRecord {
        let from = self.deps.records.borrow().len();
        let first = self.start(run, decl, i, RunBy::Person);
        if first.outcome == Outcome::Held {
            self.runs.approve(THREAD, run, ApproveScope::Always);
        }
        self.until_from(from, run, &[RunState::Done, RunState::Failed, RunState::Cancelled], 20_000).await
    }
}

fn keys(r: &RunRecord) -> Vec<String> {
    r.0.keys().cloned().collect()
}

fn s(r: &RunRecord, k: &str) -> Option<String> {
    r.0.get(k).and_then(|v| v.as_str()).map(String::from)
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

fn hostile(d: &Path) -> String {
    let d = d.display();
    format!("x\"; touch {d}/p1; echo \"$(touch {d}/p2)`touch {d}/p3`' && touch {d}/p4 #")
}

mod values_reach_a_command_as_data {
    use super::*;

    // runs_table Behaviour 6: slate-runs.test.ts:83
    #[test]
    fn an_env_value_with_shell_metacharacters_is_one_word_and_runs_nothing() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let value = hostile(dir.path());
            let decl = CmdRunDecl { env: Some(env_of(&[("ID", "$id")])), ..cmd("printf '%s' \"$ID\"; echo; echo $ID") };
            let r = h
                .run_allowed(
                    "check",
                    decl,
                    RunInputs { env: Some(IndexMap::from([("ID".into(), RunInput::Value(json!(value)))])), ..Default::default() },
                )
                .await;
            assert_eq!(r.state(), Some(RunState::Done));
            assert_eq!(s(&r, "out").unwrap(), format!("{value}\n{value}\n"));
            for p in ["p1", "p2", "p3", "p4"] {
                assert!(!dir.path().join(p).exists());
            }
        });
    }

    // runs_table Behaviour 6: slate-runs.test.ts:92
    #[test]
    fn a_positional_parameter_and_stdin_carry_it_the_same_way() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let value = hostile(dir.path());
            let decl = CmdRunDecl {
                args: Some(vec![json!("$a"), json!("'x'")]),
                stdin: Some(json!("$body")),
                ..cmd("printf '%s|' \"$1\" \"$2\"; cat")
            };
            let i = RunInputs {
                args: Some(vec![RunInput::Value(json!(value)), RunInput::Value(json!("two words"))]),
                stdin: Some(RunInput::Value(json!(value))),
                ..Default::default()
            };
            let r = h.run_allowed("args", decl, i).await;
            assert_eq!(s(&r, "out").unwrap(), format!("{value}|two words|{value}"));
            for p in ["p1", "p2", "p3", "p4"] {
                assert!(!dir.path().join(p).exists());
            }
        });
    }

    // runs_table Behaviour 6: slate-runs.test.ts:103
    #[test]
    fn null_is_unset_a_list_is_json_and_the_run_sees_the_login_environment_with_no_wsp() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let d = deps(dir.path(), test_clock());
            for (k, v) in [
                ("WSP_HOST_TOKEN", "host-token"),
                ("WSP_TURN", "turn"),
                ("WSP_HOST_URL", "http://x"),
                ("WSP_HOST_KEY", "k"),
                ("KEPT", "yes"),
            ] {
                d.env.borrow_mut().insert(k.into(), v.into());
            }
            let h = harness_with(dir.path(), d);
            let decl =
                CmdRunDecl { env: Some(env_of(&[("GONE", "$g"), ("LIST", "$l")])), ..cmd("echo \"${GONE-unset} $LIST $KEPT\"; pwd; env") };
            let i = RunInputs {
                env: Some(IndexMap::from([
                    ("GONE".into(), RunInput::Value(Value::Null)),
                    ("LIST".into(), RunInput::Value(json!([1, "a"]))),
                ])),
                ..Default::default()
            };
            let r = h.run_allowed("env", decl, i).await;
            let out = s(&r, "out").unwrap();
            let mut lines = out.split('\n');
            assert_eq!(lines.next(), Some("unset [1,\"a\"] yes"));
            assert_eq!(lines.next(), Some(dir.path().display().to_string().as_str()));
            assert!(!out.contains("WSP_"));
            assert!(!out.contains("host-token"));
        });
    }

    // runs_table Behaviour 6: slate-runs.test.ts:115
    #[test]
    fn a_cwd_is_read_against_the_thread_s_folder_and_a_missing_one_fails_with_why() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            std::fs::create_dir(dir.path().join("notes")).unwrap();
            let a = h.run_allowed("a", CmdRunDecl { cwd: Some("notes".into()), ..cmd("pwd") }, RunInputs::default()).await;
            assert_eq!(s(&a, "out").unwrap(), format!("{}\n", dir.path().join("notes").display()));
            let b = h.run_allowed("b", CmdRunDecl { cwd: Some("nowhere".into()), ..cmd("pwd") }, RunInputs::default()).await;
            assert_eq!(b.state(), Some(RunState::Failed));
            assert_eq!(s(&b, "why").unwrap(), format!("the folder {} does not exist", dir.path().join("nowhere").display()));
            assert_eq!(keys(&b), ["state", "why", "exit", "runs", "endedAt"]);
            assert_eq!(b.0["exit"], Value::Null);
        });
    }

    // runs_table Behaviour 6: an absolute cwd wins and `..` folds, as path.resolve does (pilot finding)
    #[test]
    fn an_absolute_cwd_wins_and_dot_dot_folds() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            std::fs::create_dir(dir.path().join("a")).unwrap();
            let abs = h
                .run_allowed(
                    "abs",
                    CmdRunDecl { cwd: Some(dir.path().join("a").display().to_string()), ..cmd("pwd") },
                    RunInputs::default(),
                )
                .await;
            assert_eq!(s(&abs, "out").unwrap(), format!("{}\n", dir.path().join("a").display()));
            let up = h.run_allowed("up", CmdRunDecl { cwd: Some("a/../a/.".into()), ..cmd("pwd") }, RunInputs::default()).await;
            assert_eq!(s(&up, "out").unwrap(), format!("{}\n", dir.path().join("a").display()));
        });
    }

    // runs_table Behaviour 6: an env name that is not one fails before anything starts (slate-runs.ts:751)
    #[test]
    fn an_env_name_that_is_not_a_name_fails_the_run() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let decl = CmdRunDecl { env: Some(env_of(&[("1X", "$x")])), ..cmd("true") };
            let r = h
                .run_allowed(
                    "bad",
                    decl,
                    RunInputs { env: Some(IndexMap::from([("1X".into(), RunInput::Value(json!("v")))])), ..Default::default() },
                )
                .await;
            assert_eq!(s(&r, "why").unwrap(), "1X is not a name an environment variable can have");
            assert_eq!(r.runs(), 0);
        });
    }
}

mod secrets {
    use super::*;
    const TOKEN: &str = "vrcl_Ab+9/zz&q=1 ünï_0123456789";

    // runs_table Behaviour 6-8, runs_secrets 7: slate-runs.test.ts:127
    #[test]
    fn reach_a_run_through_env_and_stdin_and_are_scrubbed_from_out_err_lines_and_the_record() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let handle = h.runs.secrets().set(THREAD, "token", TOKEN, false);
            assert_eq!((handle.secret, handle.set, handle.len), (true, true, TOKEN.encode_utf16().count() as i64));
            let script = [
                "echo \"plain $TOKEN\"",
                "printf '%s' \"$TOKEN\" | base64 -w0; echo",
                "printf 'user:%s' \"$TOKEN\" | base64 -w0; echo",
                "python3 -c 'import os,urllib.parse; print(urllib.parse.quote(os.environ[\"TOKEN\"], safe=\"-_.!~*()\\x27\"))'",
                "printf '%s' \"$TOKEN\" | base64 -w0 | tr '+/' '-_' | tr -d '='; echo",
                "echo \"err $TOKEN\" >&2",
                "printf 'stdin '; cat; echo",
            ]
            .join("; ");
            let decl =
                CmdRunDecl { env: Some(env_of(&[("TOKEN", "$token")])), stdin: Some(json!("$token")), stream: Some(true), ..cmd(&script) };
            let i = RunInputs {
                env: Some(IndexMap::from([("TOKEN".into(), RunInput::Secret("token".into()))])),
                stdin: Some(RunInput::Secret("token".into())),
                ..Default::default()
            };
            let r = h.run_allowed("leak", decl, i).await;
            assert_eq!(r.state(), Some(RunState::Done));
            let b64 = "dnJjbF9BYis5L3p6JnE9MSDDvG7Dr18wMTIzNDU2Nzg5";
            let forms = [
                TOKEN.to_string(),
                b64.to_string(),
                b64.replace('+', "-").replace('/', "_"),
                "vrcl_Ab%2B9%2Fzz%26q%3D1%20%C3%BCn%C3%AF_0123456789".to_string(),
            ];
            let everything = serde_json::to_string(&(
                h.deps.records.borrow().iter().map(|(_, r)| r.clone()).collect::<Vec<_>>(),
                h.deps.lines.borrow().clone(),
            ))
            .unwrap();
            for f in &forms {
                assert!(!everything.contains(f.as_str()), "{f} leaked");
                assert!(!everything.contains(&f[4..16]), "{} leaked", &f[4..16]);
            }
            assert!(!everything.contains("dXNlcjp2cmNsX0FiKzkvenomcT0xIMO8bsOvXzAxMjM0NTY3ODk="));
            let out = s(&r, "out").unwrap();
            assert!(out.contains("plain [secret:token]"));
            assert!(out.contains("stdin [secret:token]"));
            assert_eq!(out.split('\n').nth(1), Some("[secret:token]"));
            assert_eq!(s(&r, "err").unwrap(), "err [secret:token]\n");
            assert!(h.deps.lines.borrow().contains(&"plain [secret:token]".to_string()));
        });
    }

    // runs_table Behaviour 6: slate-runs.test.ts:162
    #[test]
    fn are_refused_as_a_positional_parameter_which_ps_can_read() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.runs.secrets().set(THREAD, "token", TOKEN, false);
            let decl = CmdRunDecl { args: Some(vec![json!("$token")]), ..cmd("echo \"$1\"") };
            let r =
                h.run_allowed("argv", decl, RunInputs { args: Some(vec![RunInput::Secret("token".into())]), ..Default::default() }).await;
            assert_eq!(r.state(), Some(RunState::Failed));
            assert_eq!(s(&r, "why").unwrap(), "a secret reaches a command through env or stdin, never as an argument, which ps can read");
            assert!(r.0.get("out").is_none());
        });
    }

    // runs_table Behaviour 16: slate-runs.test.ts:170
    #[test]
    fn the_sheet_shows_dots_never_the_value() {
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.runs.secrets().set(THREAD, "token", TOKEN, false);
            let decl = CmdRunDecl {
                env: Some(env_of(&[("TOKEN", "$token"), ("OTHER", "$other"), ("NIL", "$nil"), ("N", "$n")])),
                stdin: Some(json!("$s")),
                ..cmd("printf 'T=%s' \"$TOKEN\" > .env")
            };
            let i = RunInputs {
                env: Some(IndexMap::from([
                    ("TOKEN".into(), RunInput::Secret("token".into())),
                    ("OTHER".into(), RunInput::Secret("unset".into())),
                    ("NIL".into(), RunInput::Value(Value::Null)),
                    ("N".into(), RunInput::Value(json!(1.5))),
                ])),
                stdin: Some(RunInput::Value(json!("first\nsecond"))),
                ..Default::default()
            };
            let a = h.start("save", decl, i, RunBy::Person);
            assert_eq!(a.outcome, Outcome::Held);
            assert!(!format!("{a:?}").contains(TOKEN));
            let ask = a.ask.unwrap();
            assert_eq!(
                ask.env,
                IndexMap::from([
                    ("TOKEN".to_string(), "••••".to_string()),
                    ("OTHER".into(), "•••• (not filled)".into()),
                    ("NIL".into(), "(unset)".into()),
                    ("N".into(), "1.5".into())
                ])
            );
            assert_eq!(ask.stdin.as_deref(), Some("first"));
            assert_eq!(
                serde_json::to_value(&ask).unwrap().as_object().unwrap().keys().cloned().collect::<Vec<_>>(),
                ["run", "key", "cmd", "env", "args", "stdin", "folder", "on", "timeout"]
            );
        });
    }
}

mod consent {
    use super::*;

    // runs_table Behaviour 3, 11: slate-runs.test.ts:205
    #[test]
    fn holds_until_approved_once_is_not_remembered_always_is_and_an_edited_command_is_held() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let decl = CmdRunDecl { env: Some(env_of(&[("ID", "$id")])), ..cmd("echo \"$ID\"") };
            let id = |v: &str| RunInputs { env: Some(IndexMap::from([("ID".into(), RunInput::Value(json!(v)))])), ..Default::default() };
            let held = h.start("check", decl.clone(), id("1"), RunBy::Person);
            assert_eq!(held.outcome, Outcome::Held);
            assert_eq!(serde_json::to_string(&held.record).unwrap(), r#"{"state":"held","why":"needs your approval","runs":0}"#);
            let ask = held.ask.unwrap();
            assert_eq!(
                (ask.cmd.as_str(), ask.env["ID"].as_str(), ask.folder.as_str(), ask.timeout.clone()),
                ("echo \"$ID\"", "1", dir.path().to_str().unwrap(), JsNumber::from(60i64))
            );
            assert_eq!(h.runs.held(THREAD).iter().map(|a| a.run.clone()).collect::<Vec<_>>(), ["check"]);

            let from = h.deps.records.borrow().len();
            assert_eq!(h.runs.approve(THREAD, "check", ApproveScope::Once).map(|a| a.outcome), Some(Outcome::Running));
            assert_eq!(s(&h.until_from(from, "check", &[RunState::Done], 10_000).await, "out").unwrap(), "1\n");
            assert_eq!(h.start("check", decl.clone(), id("2"), RunBy::Person).outcome, Outcome::Held);

            let from = h.deps.records.borrow().len();
            h.runs.approve(THREAD, "check", ApproveScope::Always);
            assert_eq!(s(&h.until_from(from, "check", &[RunState::Done], 10_000).await, "out").unwrap(), "2\n");
            let from = h.deps.records.borrow().len();
            assert_eq!(h.start("check", decl.clone(), id("3"), RunBy::Reaction).outcome, Outcome::Running);
            assert_eq!(s(&h.until_from(from, "check", &[RunState::Done], 10_000).await, "out").unwrap(), "3\n");

            let edits = [
                CmdRunDecl { cmd: "echo \"$ID\"; curl evil.example".into(), ..decl.clone() },
                CmdRunDecl { env: Some(env_of(&[("ID", "$other")])), ..decl.clone() },
                CmdRunDecl { env: Some(env_of(&[("ID", "$id"), ("MORE", "$x")])), ..decl.clone() },
                CmdRunDecl { cwd: Some("..".into()), ..decl.clone() },
                CmdRunDecl { stdin: Some(json!("$id")), ..decl.clone() },
            ];
            for edited in edits {
                assert_eq!(h.start("check", edited, id("4"), RunBy::Reaction).outcome, Outcome::Held);
            }
            assert_eq!(h.runs.allowed(THREAD), [run_key(&decl)]);
            assert_eq!(h.runs.key(&decl), run_key(&decl));
        });
    }

    // runs_table Behaviour 3, 11: slate-runs.test.ts:235
    #[test]
    fn a_confirm_asks_every_time_don_t_ends_it_cancelled_and_a_revoke_asks_again() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let sure = CmdRunDecl { confirm: Some("Delete the preview?".into()), ..cmd("true") };
            h.start("sure", sure.clone(), RunInputs::default(), RunBy::Person);
            let from = h.deps.records.borrow().len();
            h.runs.approve(THREAD, "sure", ApproveScope::Always);
            h.until_from(from, "sure", &[RunState::Done], 10_000).await;
            let again = h.start("sure", sure, RunInputs::default(), RunBy::Person);
            assert_eq!(again.outcome, Outcome::Held);
            assert_eq!(again.ask.unwrap().confirm.as_deref(), Some("Delete the preview?"));
            h.runs.deny(THREAD, "sure");
            let last = h.last();
            assert_eq!(keys(&last), ["state", "why", "exit", "runs", "endedAt"]);
            assert_eq!(
                (last.state(), s(&last, "why").unwrap().as_str(), last.runs()),
                (Some(RunState::Cancelled), "you said not to run it", 1)
            );

            let plain = cmd("true");
            h.start("plain", plain.clone(), RunInputs::default(), RunBy::Person);
            let from = h.deps.records.borrow().len();
            h.runs.approve(THREAD, "plain", ApproveScope::Always);
            h.until_from(from, "plain", &[RunState::Done], 10_000).await;
            h.runs.revoke(THREAD, &h.runs.key(&plain));
            assert_eq!(h.start("plain", plain, RunInputs::default(), RunBy::Person).outcome, Outcome::Held);
        });
    }

    // runs_table Behaviour 3e, 15: slate-runs.test.ts:253
    #[test]
    fn reactions_and_timers_start_a_run_at_most_12_times_a_minute_a_press_is_not_counted() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let decl = cmd("true");
            h.start("r", decl.clone(), RunInputs::default(), RunBy::Person);
            let from = h.deps.records.borrow().len();
            h.runs.approve(THREAD, "r", ApproveScope::Always);
            h.until_from(from, "r", &[RunState::Done], 10_000).await;
            for i in 0..12 {
                let from = h.deps.records.borrow().len();
                assert_eq!(
                    h.start("r", decl.clone(), RunInputs::default(), if i % 2 == 0 { RunBy::Reaction } else { RunBy::Timer }).outcome,
                    Outcome::Running
                );
                h.until_from(from, "r", &[RunState::Done], 10_000).await;
            }
            let over = h.start("r", decl.clone(), RunInputs::default(), RunBy::Reaction);
            assert_eq!(
                (over.outcome, s(&over.record, "why").unwrap().as_str()),
                (Outcome::Held, "started 12 times in a minute; press to run it again")
            );
            assert!(over.ask.is_none());
            let from = h.deps.records.borrow().len();
            assert_eq!(h.start("r", decl.clone(), RunInputs::default(), RunBy::Person).outcome, Outcome::Running);
            h.until_from(from, "r", &[RunState::Done], 10_000).await;
            h.runs.release(THREAD);
            assert_eq!(h.start("r", decl, RunInputs::default(), RunBy::Reaction).outcome, Outcome::Running);
        });
    }

    // runs_table Behaviour 3e, 15: slate-runs.test.ts:270
    #[test]
    fn the_person_s_starts_have_a_budget_of_their_own_12_a_minute_which_no_press_or() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let decl = cmd("true");
            h.start("r", decl.clone(), RunInputs::default(), RunBy::Person);
            let from = h.deps.records.borrow().len();
            h.runs.approve(THREAD, "r", ApproveScope::Always);
            h.until_from(from, "r", &[RunState::Done], 10_000).await;
            for _ in 1..12 {
                let from = h.deps.records.borrow().len();
                assert_eq!(h.start("r", decl.clone(), RunInputs::default(), RunBy::Person).outcome, Outcome::Running);
                h.until_from(from, "r", &[RunState::Done], 10_000).await;
            }
            h.runs.release(THREAD);
            let over = h.start("r", decl, RunInputs::default(), RunBy::Person);
            assert_eq!(
                serde_json::to_string(&over.record).unwrap(),
                r#"{"state":"held","why":"pressed 12 times in a minute; it can start again in a minute","runs":12}"#
            );
        });
    }

    // runs_table Edge case: the budget is spent before the approval check (slate-runs.ts:898-902)
    #[test]
    fn a_start_held_for_approval_still_spends_the_budget() {
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            for _ in 0..12 {
                assert_eq!(h.start("r", cmd("true"), RunInputs::default(), RunBy::Reaction).outcome, Outcome::Held);
            }
            let over = h.start("r", cmd("true"), RunInputs::default(), RunBy::Reaction);
            assert_eq!(s(&over.record, "why").unwrap(), "started 12 times in a minute; press to run it again");
            assert!(h.runs.held(THREAD).is_empty());
        });
    }

    // runs_table Behaviour 3b: a stored host run of a thread now away fails before anything asks (slate-runs.ts:891-892)
    #[test]
    fn a_host_run_the_host_refuses_fails_before_it_asks() {
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            *h.deps.host_refused.borrow_mut() = Some("$x runs on the host, which is elsewhere now".into());
            let a = h.start("x", CmdRunDecl { on: Some("host".into()), ..cmd("true") }, RunInputs::default(), RunBy::Person);
            assert_eq!(a.outcome, Outcome::Failed);
            assert_eq!(keys(&a.record), ["state", "why", "exit", "runs", "endedAt"]);
            assert_eq!(s(&a.record, "why").unwrap(), "$x runs on the host, which is elsewhere now");
            let thread = h.start("y", cmd("true"), RunInputs::default(), RunBy::Person);
            assert_eq!(thread.outcome, Outcome::Held);
        });
    }
}

mod process_lifetime {
    use super::*;

    // runs_table Behaviour 10, 12: slate-runs.test.ts:305
    #[test]
    fn a_cancel_kills_the_whole_process_group_and_writes_cancelled() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.start(
                "long",
                CmdRunDecl { stream: Some(true), ..cmd("echo $$; sleep 300 & sleep 301 & bash -c 'sleep 302' & wait") },
                RunInputs::default(),
                RunBy::Person,
            );
            h.runs.approve(THREAD, "long", ApproveScope::Always);
            assert!(until(5_000, || !h.deps.lines.borrow().is_empty()).await);
            let pgid: u32 = h.deps.lines.borrow()[0].parse().unwrap();
            assert!(until(3_000, || in_group(pgid).len() >= 4).await);
            h.runs.cancel(THREAD, "long");
            let last = h.last();
            assert_eq!(keys(&last), ["state", "why", "exit", "runs", "startedAt", "ms", "endedAt"]);
            assert_eq!((last.state(), s(&last, "why").unwrap().as_str(), last.runs()), (Some(RunState::Cancelled), "cancelled", 1));
            assert!(until(3_000, || in_group(pgid).is_empty()).await);
            let count = h.deps.records.borrow().len();
            sleep_ms(1_500).await;
            assert_eq!(h.deps.records.borrow().len(), count, "the killed run's end wrote a record");
        });
    }

    // runs_table Behaviour 3c, 6: slate-runs.test.ts:326
    #[test]
    fn a_start_while_it_runs_restarts_it_with_once_it_is_a_no_op() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let decl = CmdRunDecl { stream: Some(true), ..cmd("echo $$; sleep 300") };
            h.start("x", decl.clone(), RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "x", ApproveScope::Always);
            assert!(until(5_000, || h.deps.lines.borrow().len() == 1).await);
            let old: u32 = h.deps.lines.borrow()[0].parse().unwrap();
            let restarted = h.start("x", decl.clone(), RunInputs::default(), RunBy::Person);
            assert_eq!((restarted.outcome, restarted.record.runs()), (Outcome::Running, 2));
            assert!(until(3_000, || in_group(old).is_empty()).await);
            let noop = h.start("x", CmdRunDecl { once: Some(true), ..decl }, RunInputs::default(), RunBy::Person);
            assert_eq!((noop.outcome, noop.record.state(), noop.record.runs()), (Outcome::Noop, Some(RunState::Running), 2));
            h.runs.stop_all(THREAD, true, None);
        });
    }

    // runs_table Behaviour 8: slate-runs.test.ts:339
    #[test]
    fn a_timeout_ends_the_run_failed_with_exit_124_and_takes_its_group() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let at = std::time::Instant::now();
            let r = h
                .run_allowed("slow", CmdRunDecl { timeout: Some(JsNumber::from(1i64)), ..cmd("echo $$; sleep 30") }, RunInputs::default())
                .await;
            assert_eq!(
                (r.state(), r.0["exit"].clone(), s(&r, "why").unwrap().as_str()),
                (Some(RunState::Failed), json!(124), "timed out after 1 s")
            );
            assert!(at.elapsed().as_millis() < 5_000);
            let pgid: u32 = s(&r, "out").unwrap().trim().parse().unwrap();
            assert!(until(3_000, || in_group(pgid).is_empty()).await);
        });
    }

    // runs_table Behaviour 8: slate-runs.test.ts:348
    #[test]
    fn a_non_zero_exit_fails_with_its_code_and_a_json_out_is_parsed() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let bad = h.run_allowed("bad", cmd("echo nope >&2; exit 3"), RunInputs::default()).await;
            assert_eq!(keys(&bad), ["state", "why", "exit", "err", "out", "startedAt", "endedAt", "ms", "runs"]);
            assert_eq!(
                (bad.state(), bad.0["exit"].clone(), s(&bad, "why").unwrap().as_str(), s(&bad, "err").unwrap().as_str()),
                (Some(RunState::Failed), json!(3), "exited with 3", "nope\n")
            );
            let ok = h.run_allowed("json", cmd("echo '{\"id\": 7}'"), RunInputs::default()).await;
            assert_eq!(keys(&ok), ["state", "exit", "err", "json", "out", "startedAt", "endedAt", "ms", "runs"]);
            assert_eq!(ok.0["json"], json!({"id": 7}));
            let null = h.run_allowed("null", cmd("echo null"), RunInputs::default()).await;
            assert_eq!(null.0.get("json"), Some(&Value::Null));
            let blank = h.run_allowed("blank", cmd("echo '  '"), RunInputs::default()).await;
            assert!(blank.0.get("json").is_none());
        });
    }

    // runs_table Behaviour 8: slate-runs.test.ts:354
    #[test]
    fn keeps_the_64_kb_tail_at_a_line_start_and_cuts_past_2_mb() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let a40 = "a".repeat(40);
            let r = h
                .run_allowed(
                    "big",
                    cmd(&format!("awk 'BEGIN {{ for (i = 1; i <= 4000; i++) printf \"line %05d {a40}\\n\", i }}'")),
                    RunInputs::default(),
                )
                .await;
            let out = s(&r, "out").unwrap();
            assert!(out.len() <= 64 * 1024);
            assert!(out.starts_with("line "));
            assert!(out.trim_end().ends_with(&format!("line 04000 {a40}")));
            assert!(r.0.get("cut").is_none());
            let huge = h.run_allowed("huge", cmd("head -c 3000000 /dev/zero | tr '\\0' 'b' | fold -w 99"), RunInputs::default()).await;
            assert!(s(&huge, "out").unwrap().len() <= 64 * 1024);
            assert_eq!(huge.0.get("cut"), Some(&json!(true)));
            assert_eq!(keys(&huge).last().map(String::as_str), Some("cut"));
        });
    }

    // runs_table Behaviour 13: slate-runs.test.ts:366
    #[test]
    fn a_fifth_run_waits_for_one_of_four_to_end() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            for i in 0..4 {
                h.start(&format!("s{i}"), cmd("sleep 0.5"), RunInputs::default(), RunBy::Person);
                h.runs.approve(THREAD, &format!("s{i}"), ApproveScope::Always);
            }
            h.start("fifth", cmd("echo fifth"), RunInputs::default(), RunBy::Person);
            let from = h.deps.records.borrow().len();
            let busy = h.runs.approve(THREAD, "fifth", ApproveScope::Always).unwrap();
            assert_eq!(serde_json::to_string(&busy.record).unwrap(), r#"{"state":"held","why":"4 runs are already running","runs":0}"#);
            assert!(busy.ask.is_none());
            assert_eq!(s(&h.until_from(from, "fifth", &[RunState::Done], 10_000).await, "out").unwrap(), "fifth\n");
        });
    }

    // runs_table Behaviour 12: slate-runs.test.ts:377 (the stop_all half; the records half is runs_parts')
    #[test]
    fn stop_all_quiet_drops_completions_and_writes_nothing() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.start("q", cmd("sleep 300"), RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "q", ApproveScope::Always);
            let count = h.deps.records.borrow().len();
            h.runs.stop_all(THREAD, true, None);
            sleep_ms(1_500).await;
            assert_eq!(h.deps.records.borrow().len(), count);
            assert_eq!(h.last().state(), Some(RunState::Running));
        });
    }

    // runs_table Behaviour 12: stop_all with a why writes it on each running and held run, and not on a done one
    #[test]
    fn stop_all_with_a_why_writes_it_on_running_and_held_runs_only() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.run_allowed("done", cmd("true"), RunInputs::default()).await;
            h.start("held", cmd("true; echo held"), RunInputs::default(), RunBy::Person);
            h.start("long", cmd("sleep 300"), RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "long", ApproveScope::Always);
            let from = h.deps.records.borrow().len();
            h.runs.stop_all(THREAD, false, Some("the slate was cleared"));
            let after: Vec<(String, String)> =
                h.deps.records.borrow()[from..].iter().map(|(r, rec)| (r.clone(), s(rec, "why").unwrap())).collect();
            assert_eq!(
                after,
                [("held".to_string(), "the slate was cleared".to_string()), ("long".to_string(), "the slate was cleared".to_string())]
            );
            assert!(h.runs.held(THREAD).is_empty());
        });
    }
}

mod then {
    use super::*;

    // runs_table Behaviour 8: slate-runs.test.ts:395 (the run half; the key lines are runs_parts')
    #[test]
    fn reshapes_the_output_into_json_keeps_out_raw_and_shows_on_the_sheet() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let shape = "python3 -c 'import json,sys; rows=[l[2:].split(\" | \") for l in sys.stdin.read().splitlines() if l.startswith(\"- \")]; print(json.dumps({\"items\":[{\"id\":int(a),\"from\":b,\"subject\":c} for a,b,c in rows]}))'";
            let decl = CmdRunDecl { then: Some(shape.into()), ..cmd("printf '# Inbox\\n- 1 | ann | Hello\\n- 2 | bo | Invoice\\n'") };
            let held = h.start("inbox", decl.clone(), RunInputs::default(), RunBy::Person);
            assert_eq!((held.outcome, held.ask.unwrap().then.as_deref()), (Outcome::Held, Some(shape)));
            assert_ne!(h.runs.key(&decl), h.runs.key(&cmd(&decl.cmd)));
            assert_ne!(h.runs.key(&decl), h.runs.key(&CmdRunDecl { then: Some("cat".into()), ..decl.clone() }));
            let from = h.deps.records.borrow().len();
            h.runs.approve(THREAD, "inbox", ApproveScope::Always);
            let r = h.until_from(from, "inbox", &[RunState::Done, RunState::Failed], 20_000).await;
            assert_eq!((r.state(), r.0["exit"].clone()), (Some(RunState::Done), json!(0)));
            assert_eq!(s(&r, "out").unwrap(), "# Inbox\n- 1 | ann | Hello\n- 2 | bo | Invoice\n");
            assert_eq!(
                r.0["json"],
                json!({"items": [{"id": 1, "from": "ann", "subject": "Hello"}, {"id": 2, "from": "bo", "subject": "Invoice"}]})
            );
        });
    }

    // runs_table Behaviour 8: slate-runs.test.ts:407
    #[test]
    fn a_failing_reshape_fails_the_run_with_its_stderr_and_one_that_prints_no_json_says_so() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let bad = h
                .run_allowed(
                    "bad",
                    CmdRunDecl { then: Some("cat >/dev/null; echo cannot read it >&2; exit 4".into()), ..cmd("echo raw") },
                    RunInputs::default(),
                )
                .await;
            assert_eq!(keys(&bad), ["state", "why", "exit", "err", "out", "startedAt", "endedAt", "ms", "runs"]);
            assert_eq!(
                (
                    s(&bad, "why").unwrap().as_str(),
                    bad.0["exit"].clone(),
                    s(&bad, "out").unwrap().as_str(),
                    s(&bad, "err").unwrap().as_str()
                ),
                ("then exited with 4", json!(4), "raw\n", "cannot read it\n")
            );
            let prose =
                h.run_allowed("prose", CmdRunDecl { then: Some("echo not json".into()), ..cmd("echo raw") }, RunInputs::default()).await;
            assert_eq!(
                (s(&prose, "why").unwrap().as_str(), s(&prose, "out").unwrap().as_str()),
                ("then printed no JSON: not json", "raw\n")
            );
            let failed =
                h.run_allowed("failed", CmdRunDecl { then: Some("echo '{}'".into()), ..cmd("exit 2") }, RunInputs::default()).await;
            assert_eq!((failed.state(), s(&failed, "why").unwrap().as_str()), (Some(RunState::Failed), "exited with 2"));
        });
    }

    // runs_table Behaviour 8: slate-runs.test.ts:414
    #[test]
    fn reads_the_output_scrubbed_and_never_sees_a_secret() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.runs.secrets().set(THREAD, "token", "tok_9f8e7d6c5b4a", false);
            let decl = CmdRunDecl {
                env: Some(env_of(&[("T", "$token")])),
                then: Some("python3 -c 'import json,sys; print(json.dumps({\"got\": sys.stdin.read().strip()}))'".into()),
                ..cmd("echo \"$T\"")
            };
            let r = h
                .run_allowed(
                    "leak",
                    decl,
                    RunInputs { env: Some(IndexMap::from([("T".into(), RunInput::Secret("token".into()))])), ..Default::default() },
                )
                .await;
            assert_eq!(r.0["json"], json!({"got": "[secret:token]"}));
        });
    }
}

mod a_box_that_naps {
    use super::*;

    // runs_table Behaviour 3a: slate-runs.test.ts:422
    #[test]
    fn a_tick_while_it_naps_is_held_saying_so_its_last_result_kept_and_nothing_launched() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let first = h.run_allowed("feed", cmd("echo one"), RunInputs::default()).await;
            assert_eq!((first.state(), s(&first, "out").unwrap().as_str()), (Some(RunState::Done), "one\n"));
            let held = h.runs.start(RunStart {
                by: RunBy::Timer,
                asleep: true,
                ..h.req("feed", cmd("echo one"), RunInputs::default(), RunBy::Timer)
            });
            assert_eq!(held.outcome, Outcome::Held);
            assert_eq!(keys(&held.record), ["exit", "out", "err", "endedAt", "ms", "state", "why", "runs"]);
            assert_eq!(
                (s(&held.record, "why").unwrap().as_str(), s(&held.record, "out").unwrap().as_str(), held.record.0["exit"].clone()),
                ("the box was asleep, so this tick did not wake it; press to run it now", "one\n", json!(0))
            );
            assert!(h.runs.held(THREAD).is_empty());
            let from = h.deps.records.borrow().len();
            assert_eq!(
                h.runs.start(RunStart { asleep: true, ..h.req("feed", cmd("echo one"), RunInputs::default(), RunBy::Person) }).outcome,
                Outcome::Running
            );
            let again = h.until_from(from, "feed", &[RunState::Done], 10_000).await;
            assert_eq!(again.runs(), 2);
        });
    }

    // runs_table Behaviour 3a: a tick on a napping box while it runs is a no-op
    #[test]
    fn a_tick_while_it_naps_and_runs_is_a_no_op() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.start("feed", cmd("sleep 300"), RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "feed", ApproveScope::Always);
            let count = h.deps.records.borrow().len();
            let tick = h.runs.start(RunStart { asleep: true, ..h.req("feed", cmd("sleep 300"), RunInputs::default(), RunBy::Timer) });
            assert_eq!((tick.outcome, tick.record.state()), (Outcome::Noop, Some(RunState::Running)));
            assert_eq!(h.deps.records.borrow().len(), count);
            h.runs.stop_all(THREAD, true, None);
        });
    }
}

/// A road that ends each start at once with `code`, counting the starts and keeping the envs.
struct FakeRoad {
    code: RefCell<Option<i64>>,
    starts: RefCell<Vec<i64>>,
    envs: RefCell<Vec<IndexMap<String, String>>>,
    clock: Rc<dyn Clock>,
    sync_end: bool,
}

impl RunRoad for FakeRoad {
    fn slate_dir(&self) -> String {
        "/tmp/wsp-slates/t1".into()
    }
    fn start(&self, o: RoadStart, on: Rc<dyn RoadEvents>) -> RoadStarted {
        self.starts.borrow_mut().push(self.clock.now());
        self.envs.borrow_mut().push(o.env);
        let code = *self.code.borrow();
        let end = RoadEnd { code, out: "{}".into(), ..Default::default() };
        if self.sync_end {
            on.end(end);
        } else {
            tokio::task::spawn_local(async move { on.end(end) });
        }
        RoadStarted { kill: Rc::new(|| {}), pid: None }
    }
    fn reshape(&self, o: ReshapeStart) -> Reshape {
        self.envs.borrow_mut().push(o.env);
        Reshape { done: Box::pin(async { ReshapeAnswer::Json(json!({})) }), kill: Rc::new(|| {}) }
    }
}

fn fake_road(clock: Rc<dyn Clock>, code: i64, sync_end: bool) -> Rc<FakeRoad> {
    Rc::new(FakeRoad { code: RefCell::new(Some(code)), starts: RefCell::new(vec![]), envs: RefCell::new(vec![]), clock, sync_end })
}

mod timers {
    use super::*;

    // runs_table Behaviour 14: slate-runs.test.ts:437
    #[test]
    fn an_always_timer_that_fails_five_starts_in_a_row_starts_once_every_five_minutes() {
        local(async {
            let dir = support::temp_root("slate-runs-");
            let clock = ManualClock::new(1_000_000);
            let road = fake_road(clock.clock(), 1, true);
            let d = deps(dir.path(), clock.clock());
            *d.road.borrow_mut() = Some(road.clone());
            let h = Rc::new(harness_with(dir.path(), d));
            let decl = CmdRunDecl { every: Some(JsNumber::from(10i64)), always: Some(true), ..cmd("false") };
            let weak = Rc::downgrade(&h);
            let tick_decl = decl.clone();
            *h.deps.on_timer.borrow_mut() = Some(Rc::new(move |run: &str| {
                let h = weak.upgrade().unwrap();
                h.runs.start(RunStart { by: RunBy::Timer, ..h.req(run, tick_decl.clone(), RunInputs::default(), RunBy::Timer) });
            }));
            h.runs.timers(THREAD, vec![TimerDecl { run: "poll".into(), every: 10.0, key: "k-poll".into(), always: true }]);
            assert!(road.starts.borrow().is_empty());
            h.runs.approve(THREAD, "poll", ApproveScope::Always);
            clock.advance(40_000);
            assert_eq!(road.starts.borrow().len(), 5);
            let last = h.of("poll").last().cloned().unwrap();
            assert_eq!((last.state(), s(&last, "why").unwrap()), (Some(RunState::Failed), format!("exited with 1; {BACKED_OFF}")));
            clock.advance(290_000);
            assert_eq!(road.starts.borrow().len(), 5);
            clock.advance(10_000);
            assert_eq!(road.starts.borrow().len(), 6);
            *road.code.borrow_mut() = Some(0);
            clock.advance(300_000);
            assert_eq!(road.starts.borrow().len(), 7);
            assert_eq!(h.of("poll").last().unwrap().state(), Some(RunState::Done));
            clock.advance(30_000);
            assert_eq!(road.starts.borrow().len(), 10);
        });
    }

    // runs_table Behaviour 14: slate-runs.test.ts:474
    #[test]
    fn tick_only_while_shown_by_default_always_with_always_never_under_10_s() {
        local(async {
            let dir = support::temp_root("slate-runs-");
            let clock = ManualClock::new(1_000_000);
            let h = harness_with(dir.path(), deps(dir.path(), clock.clock()));
            let count = |name: &str| h.deps.timers.borrow().iter().filter(|t| *t == name).count();
            h.runs.timers(
                THREAD,
                vec![
                    TimerDecl { run: "shown".into(), every: 1.0, key: "k-shown".into(), always: false },
                    TimerDecl { run: "always".into(), every: 60.0, key: "k-always".into(), always: true },
                ],
            );
            assert_eq!(*h.deps.timers.borrow(), ["always"]);
            clock.advance(30_000);
            assert_eq!(*h.deps.timers.borrow(), ["always"]);
            h.runs.shown(THREAD, true);
            assert_eq!(count("shown"), 1);
            clock.advance(30_000);
            assert_eq!((count("shown"), count("always")), (4, 2));
            h.runs.shown(THREAD, false);
            clock.advance(120_000);
            assert_eq!((count("shown"), count("always")), (4, 4));
        });
    }

    // runs_table Behaviour 14: an unchanged timer keeps its period; a changed one fires at once
    #[test]
    fn an_unchanged_timer_keeps_its_period_and_a_changed_one_fires_at_once() {
        local(async {
            let dir = support::temp_root("slate-runs-");
            let clock = ManualClock::new(1_000_000);
            let h = harness_with(dir.path(), deps(dir.path(), clock.clock()));
            let t = |key: &str, every: f64| TimerDecl { run: "a".into(), every, key: key.into(), always: true };
            h.runs.timers(THREAD, vec![t("k1", 60.0)]);
            clock.advance(30_000);
            h.runs.timers(THREAD, vec![t("k1", 60.0)]);
            assert_eq!(h.deps.timers.borrow().len(), 1);
            clock.advance(30_000);
            assert_eq!(h.deps.timers.borrow().len(), 2);
            h.runs.timers(THREAD, vec![t("k2", 60.0)]);
            assert_eq!(h.deps.timers.borrow().len(), 3);
            h.runs.timers(THREAD, vec![]);
            clock.advance(600_000);
            assert_eq!(h.deps.timers.borrow().len(), 3);
            assert_eq!(clock.pending(), 0);
        });
    }

    // runs_table Edge case, rule 8.18, pilot finding: a huge every never panics; the clock is asked the delay as is
    #[test]
    fn a_huge_every_never_panics_and_asks_the_clock_for_its_whole_delay() {
        local(async {
            let dir = support::temp_root("slate-runs-");
            let clock = ManualClock::new(1_000_000);
            let h = harness_with(dir.path(), deps(dir.path(), clock.clock()));
            h.runs.timers(THREAD, vec![TimerDecl { run: "big".into(), every: 3_000_000.0, key: "k".into(), always: true }]);
            assert_eq!(*h.deps.timers.borrow(), ["big"]);
            assert_eq!(*clock.asked.borrow(), [3_000_000_000]);
            h.runs.timers(THREAD, vec![TimerDecl { run: "big".into(), every: 1e300, key: "k2".into(), always: true }]);
            assert!(clock.asked.borrow().last().copied().unwrap() > 2_147_483_647);
        });
    }

    // runs_table Behaviour 14: only a run with a timer counts a streak (slate-runs.ts:694-696)
    #[test]
    fn failures_of_a_run_with_no_timer_never_back_off() {
        local(async {
            let dir = support::temp_root("slate-runs-");
            let clock = ManualClock::new(1_000_000);
            let road = fake_road(clock.clock(), 1, true);
            let d = deps(dir.path(), clock.clock());
            *d.road.borrow_mut() = Some(road.clone());
            let h = harness_with(dir.path(), d);
            let decl = cmd("false");
            h.start("free", decl.clone(), RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "free", ApproveScope::Always);
            for _ in 0..6 {
                h.start("free", decl.clone(), RunInputs::default(), RunBy::Person);
            }
            assert!(h.of("free").iter().all(|r| s(r, "why").map_or(true, |w| !w.contains("five starts"))));
        });
    }
}

mod a_command_s_process {
    use super::*;

    // runs_table Behaviour 9: slate-runs.test.ts:497
    #[test]
    fn is_named_as_the_thread_s_slate_run_while_it_runs_the_pid_its_own_and_released() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let d = Deps { own_on: true, ..deps(dir.path(), test_clock()) };
            let h = harness_with(dir.path(), d);
            let done = h.run_allowed("pid", cmd("echo $$"), RunInputs::default()).await;
            assert!(until(2_000, || h.deps.owned.borrow().first().is_some_and(|o| *o.3.borrow())).await);
            let owned = h.deps.owned.borrow();
            assert_eq!(
                owned.iter().map(|o| (o.0.clone(), o.2.clone(), *o.3.borrow())).collect::<Vec<_>>(),
                [(THREAD.to_string(), "slate".to_string(), true)]
            );
            assert_eq!(s(&done, "out").unwrap().trim(), owned[0].1.to_string());
        });
    }

    // runs_table Behaviour 6, 8: slate-runs.test.ts:511
    #[test]
    fn on_the_thread_s_own_machine_carries_the_thread_s_mark_for_the_command_and_its_then() {
        local(async {
            let dir = support::temp_root("slate-runs-");
            let road = fake_road(test_clock(), 0, false);
            let d = deps(dir.path(), test_clock());
            *d.road.borrow_mut() = Some(road.clone());
            let h = harness_with(dir.path(), d);
            h.run_allowed("there", CmdRunDecl { then: Some("cat".into()), ..cmd("true") }, RunInputs::default()).await;
            let mark = IndexMap::from([
                ("SLATE_DIR".to_string(), "/tmp/wsp-slates/t1".to_string()),
                ("WSP_THREAD".to_string(), THREAD.to_string()),
            ]);
            assert_eq!(*road.envs.borrow(), [mark.clone(), mark]);
        });
    }

    // runs_table Behaviour 6: SLATE_DIR comes from dir, and its failure fails the run with why (slate-runs.ts:645-650, 747-749)
    #[test]
    fn slate_dir_is_the_slate_s_folder_and_files_not_written_fail_the_run() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            *h.deps.dir.borrow_mut() = Some(Ok("/state/slates/t1".into()));
            let r = h.run_allowed("dir", cmd("printf %s \"$SLATE_DIR\""), RunInputs::default()).await;
            assert_eq!(s(&r, "out").unwrap(), "/state/slates/t1");
            *h.deps.dir.borrow_mut() = Some(Err("EACCES: permission denied".into()));
            let f = h.run_allowed("nodir", cmd("true"), RunInputs::default()).await;
            assert_eq!(s(&f, "why").unwrap(), "the slate's files were not written: EACCES: permission denied");
        });
    }
}

mod pilot_review {
    use super::*;

    // runs_table Behaviour 2, pilot BLOCKER: a hold never wipes the kept result
    #[test]
    fn a_hold_keeps_the_last_result_for_the_next_start() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let decl = CmdRunDecl { confirm: Some("Sure?".into()), ..cmd("echo one") };
            let first = h.run_allowed("r", decl.clone(), RunInputs::default()).await;
            assert_eq!(s(&first, "out").unwrap(), "one\n");
            let held = h.start("r", decl.clone(), RunInputs::default(), RunBy::Person);
            assert_eq!(serde_json::to_string(&held.record).unwrap(), r#"{"state":"held","why":"needs your approval","runs":1}"#);
            let running = h.runs.approve(THREAD, "r", ApproveScope::Once).unwrap();
            assert_eq!(keys(&running.record), ["exit", "out", "err", "endedAt", "ms", "state", "runs", "startedAt", "refreshing"]);
            assert_eq!(
                (s(&running.record, "out").unwrap().as_str(), running.record.0["refreshing"].clone(), running.record.runs()),
                ("one\n", json!(true), 2)
            );
        });
    }

    // runs_table Behaviour 1: a start with `last` seeds the count and the kept result once, from a record
    #[test]
    fn a_first_start_seeds_its_count_and_kept_result_from_the_stored_record() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let stored = RunRecord(
                serde_json::from_str(r#"{"state":"done","exit":0,"out":"old\n","err":"","startedAt":1,"endedAt":2,"ms":1,"runs":4}"#)
                    .unwrap(),
            );
            h.start("r", cmd("sleep 300"), RunInputs::default(), RunBy::Person);
            let other =
                h.runs.start(RunStart { last: Some(stored.clone()), ..h.req("s", cmd("sleep 300"), RunInputs::default(), RunBy::Person) });
            assert_eq!(other.record.runs(), 4);
            let running = h.runs.approve(THREAD, "s", ApproveScope::Always).unwrap();
            assert_eq!(
                serde_json::to_string(&running.record).unwrap().split("\"startedAt\"").next().unwrap(),
                r#"{"exit":0,"out":"old\n","err":"","endedAt":2,"ms":1,"state":"running","runs":5,"#
            );
            let ignored = h.runs.start(RunStart {
                last: Some(RunRecord(serde_json::from_str(r#"{"state":"done","runs":40}"#).unwrap())),
                ..h.req("s", cmd("sleep 300"), RunInputs::default(), RunBy::Person)
            });
            assert_eq!(ignored.record.runs(), 6);
            h.runs.stop_all(THREAD, true, None);
        });
    }

    // runs_table Behaviour 17, pilot BLOCKER: drop with keep still revokes approvals and forgets secrets
    #[test]
    fn drop_with_keep_leaves_runs_running_but_revokes_approvals_and_forgets_secrets() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.runs.secrets().set(THREAD, "token", "tok_abcdef123456", false);
            h.start("long", CmdRunDecl { stream: Some(true), ..cmd("echo $$; sleep 300") }, RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "long", ApproveScope::Always);
            assert!(until(5_000, || !h.deps.lines.borrow().is_empty()).await);
            let pgid: u32 = h.deps.lines.borrow()[0].parse().unwrap();
            h.runs.drop_thread(THREAD, true);
            assert!(h.runs.allowed(THREAD).is_empty());
            assert!(!h.runs.secrets().handle(THREAD, "token").set);
            sleep_ms(300).await;
            assert!(!in_group(pgid).is_empty(), "keep killed the run");
            unsafe { libc::kill(-(pgid as i32), libc::SIGKILL) };
        });
    }

    // runs_table Behaviour 17: drop without keep kills every run and writes nothing
    #[test]
    fn drop_without_keep_kills_every_run_and_writes_nothing() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.start("long", CmdRunDecl { stream: Some(true), ..cmd("echo $$; sleep 300") }, RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "long", ApproveScope::Always);
            assert!(until(5_000, || !h.deps.lines.borrow().is_empty()).await);
            let pgid: u32 = h.deps.lines.borrow()[0].parse().unwrap();
            let count = h.deps.records.borrow().len();
            h.runs.drop_thread(THREAD, false);
            assert!(until(3_000, || in_group(pgid).is_empty()).await);
            sleep_ms(1_200).await;
            assert_eq!(h.deps.records.borrow().len(), count);
        });
    }

    // runs_table Behaviour 18, rule 6.40: close kills every run of every thread and writes no record
    #[test]
    fn close_kills_every_run_and_writes_no_record() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            h.start("long", CmdRunDecl { stream: Some(true), ..cmd("echo $$; sleep 300") }, RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "long", ApproveScope::Always);
            assert!(until(5_000, || !h.deps.lines.borrow().is_empty()).await);
            let pgid: u32 = h.deps.lines.borrow()[0].parse().unwrap();
            let count = h.deps.records.borrow().len();
            h.runs.close();
            assert!(until(3_000, || in_group(pgid).is_empty()).await);
            sleep_ms(1_200).await;
            assert_eq!(h.deps.records.borrow().len(), count);
            assert_eq!(h.last().state(), Some(RunState::Running));
        });
    }

    // rule R1, pilot BLOCKER: on_record may re-enter the table, which holds no borrow while it calls out
    #[test]
    fn on_record_may_call_back_into_the_table() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = Rc::new(harness(dir.path()));
            let weak = Rc::downgrade(&h);
            *h.deps.on_record.borrow_mut() = Some(Rc::new(move |run: &str, rec: &RunRecord| {
                let h = weak.upgrade().unwrap();
                let _ = h.runs.held(THREAD);
                if run == "a" && rec.state() == Some(RunState::Done) {
                    h.start("b", cmd("echo b"), RunInputs::default(), RunBy::Reaction);
                }
            }));
            h.start("b", cmd("echo b"), RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "b", ApproveScope::Always);
            h.until_from(0, "b", &[RunState::Done], 10_000).await;
            let from = h.deps.records.borrow().len();
            let r = h.run_allowed("a", cmd("echo a"), RunInputs::default()).await;
            assert_eq!(s(&r, "out").unwrap(), "a\n");
            let b = h.until_from(from, "b", &[RunState::Done], 10_000).await;
            assert_eq!(s(&b, "out").unwrap(), "b\n");
        });
    }

    // rule R1, runs_road Decisions 3: a road that ends inside start leaves no kill behind and finishes the run
    #[test]
    fn a_road_that_ends_inside_its_start_finishes_the_run() {
        local(async {
            let dir = support::temp_root("slate-runs-");
            let road = fake_road(test_clock(), 0, true);
            let d = deps(dir.path(), test_clock());
            *d.road.borrow_mut() = Some(road.clone());
            let h = harness_with(dir.path(), d);
            h.start("x", cmd("true"), RunInputs::default(), RunBy::Person);
            let answer = h.runs.approve(THREAD, "x", ApproveScope::Always).unwrap();
            assert_eq!(answer.outcome, Outcome::Running);
            let last = h.last();
            assert_eq!((last.state(), s(&last, "out").unwrap().as_str()), (Some(RunState::Done), "{}"));
            assert_eq!(last.0["json"], json!({}));
            h.runs.cancel(THREAD, "x");
            assert_eq!(h.last().state(), Some(RunState::Done));
        });
    }

    // runs_table Behaviour 11, slates.test.ts:1210: deny beside a running instance drops only the new start
    #[test]
    fn deny_beside_a_running_instance_drops_only_the_new_start() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let decl = CmdRunDecl { confirm: Some("Deploy now?".into()), ..cmd("sleep 1; echo deployed") };
            h.start("deploy", decl.clone(), RunInputs::default(), RunBy::Person);
            let from = h.deps.records.borrow().len();
            h.runs.approve(THREAD, "deploy", ApproveScope::Once);
            let again = h.start("deploy", decl, RunInputs::default(), RunBy::Person);
            assert_eq!((again.outcome, again.record.state(), again.record.runs()), (Outcome::Held, Some(RunState::Running), 1));
            assert!(again.ask.is_some());
            let count = h.deps.records.borrow().len();
            h.runs.deny(THREAD, "deploy");
            assert_eq!(h.deps.records.borrow().len(), count);
            let done = h.until_from(from, "deploy", &[RunState::Done], 10_000).await;
            assert_eq!((s(&done, "out").unwrap().as_str(), done.runs()), ("deployed\n", 1));
            assert!(h.runs.held(THREAD).is_empty());
        });
    }

    // invariant 2: one end per instance; a restarted run's first instance never writes its end
    #[test]
    fn invariant_2_a_replaced_instance_never_writes_its_end() {
        own_group();
        let dir = support::temp_root("slate-runs-");
        local(async {
            let h = harness(dir.path());
            let decl = cmd("sleep 0.5; echo $$");
            h.start("x", decl.clone(), RunInputs::default(), RunBy::Person);
            h.runs.approve(THREAD, "x", ApproveScope::Always);
            h.start("x", decl, RunInputs::default(), RunBy::Person);
            sleep_ms(2_000).await;
            let ends: Vec<RunRecord> = h.of("x").into_iter().filter(|r| r.state() == Some(RunState::Done)).collect();
            assert_eq!(ends.len(), 1);
            assert_eq!(ends[0].runs(), 2);
        });
    }

    // runs_table Behaviour 2: the approvals deps backs Always when given (slate-runs.ts:615, 910)
    #[test]
    fn always_is_kept_in_the_approvals_the_host_gives() {
        own_group();
        #[derive(Default)]
        struct Kept(RefCell<Vec<String>>);
        impl RunApprovals for Kept {
            fn has(&self, _t: &str, key: &str) -> bool {
                self.0.borrow().iter().any(|k| k == key)
            }
            fn allow(&self, _t: &str, key: &str) {
                self.0.borrow_mut().push(key.into());
            }
            fn revoke(&self, _t: &str, key: &str) {
                self.0.borrow_mut().retain(|k| k != key);
            }
            fn list(&self, _t: &str) -> Vec<String> {
                self.0.borrow().clone()
            }
        }
        let dir = support::temp_root("slate-runs-");
        local(async {
            let kept = Rc::new(Kept::default());
            let h = harness_with(dir.path(), Deps { approvals: Some(kept.clone()), ..deps(dir.path(), test_clock()) });
            h.run_allowed("a", cmd("true"), RunInputs::default()).await;
            assert_eq!(*kept.0.borrow(), [run_key(&cmd("true"))]);
            h.runs.drop_thread(THREAD, false);
            assert!(kept.0.borrow().is_empty());
        });
    }
}
