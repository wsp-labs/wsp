// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/activity.test.ts at a3b870a6e, lines 46-50 (the settle's line), 68-150 (an End's
//! plan), 197-206 (a pane's shell ended) and 250-253 (an agent outside this host's threads ended from No thread): the
//! cases that call `end_plan`, `agent_end_line`, `agent_under_line` and `processes_left_line`. The rest of that file is
//! wsp-wire's.

mod support;

use std::collections::HashMap;

use serde_json::{json, Value};
use support::over;
use wsp_wire::types::JsNumber;
use wsp_wire::views::activity::{
    activity_of, end_gone_line, end_reused_line, not_started_line, not_threads_line, processes_left, wsp_own_line, ActivityRead,
    ActivityScope, EndPlan, LeftThread,
};
use wsp_words::exit::HostError;
use wsp_words::views::activity::{agent_end_line, agent_under_line, end_plan, processes_left_line, LeftVerb};

const START: i64 = 1_760_000_000_000;

fn row(pid: i64, ppid: i64, comm: &str, cmdline: &str) -> Value {
    json!({ "pid": pid, "ppid": ppid, "user": "ada", "state": "S", "comm": comm, "cmdline": cmdline, "cpu": 1, "rss": 1_000, "startedAt": START + pid })
}

fn row_with(pid: i64, ppid: i64, comm: &str, cmdline: &str, extra: Value) -> Value {
    over(row(pid, ppid, comm, cmdline), extra)
}

const LAB_THREAD: &str = "lab-thread-1";
const KEPT_THREAD: &str = "kept-thread-2";

fn lab() -> ActivityScope {
    ActivityScope {
        thread_id: LAB_THREAD.into(),
        title: "dev server".into(),
        workspace_id: "ws-1".into(),
        roots: vec![100],
        ..Default::default()
    }
}

fn kept() -> ActivityScope {
    ActivityScope {
        thread_id: KEPT_THREAD.into(),
        title: "kept".into(),
        workspace_id: "ws-1".into(),
        roots: vec![300],
        kept: Some(300),
        ..Default::default()
    }
}

/// A turn's run script and its shell down to a native install of Claude Code, which runs as its version, with vite
/// and esbuild under it; a kept agent; a server the daemon owns by the ladder with no live parent; the daemon, the host
/// and a shell of the person's own.
fn procs() -> Vec<Value> {
    vec![
        row(1, 0, "launchd", "/sbin/launchd"),
        row(100, 50, "bash", "bash runs/53af26c40316.sh"),
        row(101, 100, "bash", "bash"),
        row(102, 101, "2.1.296", "claude -p --input-format stream-json"),
        row_with(200, 102, "node", "node /lab/node_modules/.bin/vite", json!({ "cpu": 9 })),
        row(201, 200, "esbuild", "esbuild --service"),
        row(300, 50, "node", "/usr/local/bin/cursor-agent --print"),
        row_with(400, 1, "node", "node detached.js", json!({ "owner": { "by": "fork", "thread": LAB_THREAD } })),
        row(50, 1, "node", "node wsp host"),
        row(60, 1, "wsp-daemon", "wsp-daemon serve"),
        row(70, 1, "zsh", "-zsh"),
    ]
}

fn read(procs: Vec<Value>) -> ActivityRead {
    ActivityRead {
        computer: "lab-mac".into(),
        at: JsNumber(START as f64),
        procs: serde_json::from_value(Value::Array(procs)).unwrap(),
        daemon: 60,
        host: 50,
        scopes: vec![lab(), kept()],
        agent_bins: vec!["claude".into(), "cursor-agent".into()],
        ports: None,
        cgroups: None,
    }
}

fn at(pid: i64) -> f64 {
    (START + pid) as f64
}

/// The TS test's `refusal`: the message End's plan refused with. The stub returns the plan itself, so a refusal is
/// read off the panic that carries it, a HostError or its sentence.
fn refusal(f: impl FnOnce() -> Result<EndPlan, HostError>) -> String {
    f().expect_err("expected a refusal").message
}

fn pids_of(plan: EndPlan) -> Vec<u32> {
    match plan {
        EndPlan::Targets(t) => t.iter().map(|p| p.pid).collect(),
        EndPlan::Kept(_) => panic!("expected targets"),
    }
}

mod a_snapshot_read_by_thread {
    use super::*;

    // activity.test.ts:46
    #[test]
    fn leaves_a_settle_what_outlives_the_agent_what_runs_under_the_agent_ends_with_it() {
        let left = processes_left(&activity_of(&read(procs())), &[LAB_THREAD.to_string(), KEPT_THREAD.to_string()]);
        let shape: Vec<(String, Vec<i64>)> = left.iter().map(|t| (t.thread_id.clone(), t.processes.iter().map(|p| p.pid).collect())).collect();
        assert_eq!(shape, vec![(LAB_THREAD.to_string(), vec![400])]);
        let left: Vec<LeftThread> = left
            .into_iter()
            .map(|t| LeftThread {
                thread_id: t.thread_id,
                title: t.title,
                workspace_id: t.workspace_id,
                memory: t.memory,
                cpu: t.cpu,
                processes: t.processes,
                on: None,
            })
            .collect();
        assert_eq!(
            processes_left_line(LeftVerb::Settle, &left, None),
            "a thread to settle still runs a process: 400 node (other) of lab-thre:"
        );
    }
}

mod an_end_s_plan {
    use super::*;

    fn anyone(_: Option<&str>) -> bool {
        true
    }

    // activity.test.ts:79
    #[test]
    fn takes_the_process_and_everything_under_it_each_by_the_start_it_was_read_with() {
        let r = read(procs());
        let want = EndPlan::Targets(vec![r.procs[4].clone(), r.procs[5].clone()]);
        assert_eq!(end_plan(&r, 200.0, at(200), &anyone).unwrap(), want);
    }

    // activity.test.ts:83
    #[test]
    fn refuses_each_process_of_the_stretch_to_the_agent_and_one_with_the_agent_under_it() {
        for pid in [100_i64, 101, 102] {
            let said = refusal(|| end_plan(&read(procs()), pid as f64, at(pid), &anyone));
            let line = agent_end_line(pid as f64, &lab());
            assert!(said.contains(&line), "{said}");
        }
        let mut all: Vec<Value> = procs()
            .into_iter()
            .map(|p| if p["pid"] == 100 { over(p, json!({ "ppid": 90 })) } else { p })
            .collect();
        all.push(row_with(90, 1, "tmux", "tmux", json!({ "owner": { "by": "env", "thread": LAB_THREAD, "workspace": "ws-1" } })));
        let above = read(all);
        let said = refusal(|| end_plan(&above, 90.0, at(90), &anyone));
        let line = agent_under_line(90.0, &lab());
        assert!(said.contains(&line), "{said}");
    }

    // activity.test.ts:89
    #[test]
    fn refuses_the_native_codex_that_codex_s_npm_launcher_runs_as_its_one_child_which_i() {
        let codex = ActivityScope {
            thread_id: "codex-thread-3".into(),
            title: "codex".into(),
            workspace_id: "ws-1".into(),
            roots: vec![600],
            ..Default::default()
        };
        let native = "/usr/local/lib/node_modules/@openai/codex/vendor/x86_64-unknown-linux-musl/bin/codex app-server";
        let mut all = procs();
        all.push(row(600, 50, "bash", "bash runs/r3.sh"));
        all.push(row(601, 600, "node", "node /usr/local/bin/codex app-server"));
        all.push(row(602, 601, "codex", native));
        let r = ActivityRead { scopes: vec![codex.clone()], agent_bins: vec!["codex".into()], ..read(all) };
        let said = refusal(|| end_plan(&r, 602.0, at(602), &anyone));
        let line = agent_end_line(602.0, &codex);
        assert!(said.contains(&line), "{said}");
    }

    // activity.test.ts:96
    #[test]
    fn frees_a_kept_agent_through_its_thread_instead_of_a_signal() {
        assert_eq!(end_plan(&read(procs()), 300.0, at(300), &anyone).unwrap(), EndPlan::Kept(kept()));
    }

    // activity.test.ts:100
    #[test]
    fn refuses_a_pid_that_is_gone_and_one_that_names_another_process_now() {
        let said = refusal(|| end_plan(&read(procs()), 999.0, START as f64, &anyone));
        let line = end_gone_line(999);
        assert!(said.contains(&line), "{said}");
        let r = read(procs());
        let said = refusal(|| end_plan(&r, 200.0, (START - 60_000) as f64, &anyone));
        let line = end_reused_line(200, &r.procs[4], START - 60_000);
        assert!(said.contains(&line), "{said}");
        // A start read a second off by another sampler is the same process.
        let want = EndPlan::Targets(vec![r.procs[5].clone()]);
        assert_eq!(end_plan(&r, 201.0, at(201) + 1_000.0, &anyone).unwrap(), want);
    }

    // activity.test.ts:107
    #[test]
    fn refuses_wsp_s_own_and_for_a_thread_anything_outside_its_tree() {
        let r = read(procs());
        let said = refusal(|| end_plan(&r, 60.0, at(60), &anyone));
        let line = wsp_own_line(60);
        assert!(said.contains(&line), "{said}");
        let lab_only = |t: Option<&str>| t == Some(LAB_THREAD);
        let said = refusal(|| end_plan(&r, 70.0, at(70), &lab_only));
        let line = not_threads_line(70);
        assert!(said.contains(&line), "{said}");
        assert_eq!(end_plan(&r, 400.0, at(400), &lab_only).unwrap(), EndPlan::Targets(vec![r.procs[7].clone()]));
    }

    // activity.test.ts:113
    #[test]
    fn tells_a_thread_nothing_of_a_process_outside_its_tree_whatever_start_it_names() {
        let lab_only = |t: Option<&str>| t == Some(LAB_THREAD);
        for started in [at(70), (START - 60_000) as f64] {
            let said = refusal(|| end_plan(&read(procs()), 70.0, started, &lab_only));
            let line = not_threads_line(70);
            assert!(said.contains(&line), "{said}");
            assert!(!said.contains("zsh"), "{said}");
        }
    }

    // activity.test.ts:122
    #[test]
    fn refuses_for_the_person_a_process_wsp_did_not_start_one_its_folder_or_coalition_a() {
        let said = refusal(|| end_plan(&read(procs()), 70.0, at(70), &anyone));
        let line = not_started_line(70);
        assert!(said.contains(&line), "{said}");
        let said = refusal(|| end_plan(&read(procs()), 70.0, 1.0, &anyone));
        assert!(said.contains(&line), "{said}");
        // The person's own editor in a workspace's folder, and one in wsp's coalition on a Mac: where it runs, not who started it.
        for (by, owner) in [("folder", json!({ "by": "folder", "workspace": "ws-1" })), ("coalition", json!({ "by": "coalition" }))] {
            let mut all = procs();
            all.push(row_with(500, 1, "vim", "vim notes.md", json!({ "owner": owner })));
            let placed = read(all);
            let said = refusal(|| end_plan(&placed, 500.0, at(500), &anyone));
            let line = not_started_line(500);
            assert!(said.contains(&line), "{said}");
            let unowned: Vec<i64> = activity_of(&placed).unowned.iter().map(|p| p.pid).collect();
            assert_eq!(unowned, if by == "coalition" { vec![500] } else { vec![] });
        }
        // A thread's process the daemon names by its mark, the thread no longer among the read's scopes, is still wsp's.
        let mut all = procs();
        all.push(row_with(500, 1, "node", "node vite.js", json!({ "owner": { "by": "env", "thread": "t-gone" } })));
        let marked = read(all);
        assert_eq!(end_plan(&marked, 500.0, at(500), &anyone).unwrap(), EndPlan::Targets(vec![marked.procs.last().unwrap().clone()]));
        let unowned: Vec<i64> = activity_of(&marked).unowned.iter().map(|p| p.pid).collect();
        assert_eq!(unowned, vec![500]);
    }

    // activity.test.ts:137
    #[test]
    fn lets_no_thread_end_a_process_by_the_wsp_thread_mark_alone_which_any_process_can_() {
        let lab_only = |t: Option<&str>| t == Some(LAB_THREAD);
        let mut all = procs();
        all.push(row_with(500, 1, "node", "node lookalike.js", json!({ "owner": { "by": "env", "thread": LAB_THREAD } })));
        let marked = read(all);
        // Drawn under the thread it names, and ended by nobody but the person.
        let drawn: Vec<i64> = activity_of(&marked).threads[0].processes.iter().map(|p| p.pid).collect();
        assert!(drawn.contains(&500), "{drawn:?}");
        let said = refusal(|| end_plan(&marked, 500.0, at(500), &lab_only));
        let line = not_threads_line(500);
        assert!(said.contains(&line), "{said}");
        assert_eq!(end_plan(&marked, 500.0, at(500), &anyone).unwrap(), EndPlan::Targets(vec![marked.procs.last().unwrap().clone()]));
        // The same process placed by the fork tracker, or standing in the thread's cgroup on a box, is the thread's to end.
        let mut all = procs();
        all.push(row_with(500, 1, "node", "node vite.js", json!({ "owner": { "by": "fork", "thread": LAB_THREAD } })));
        let forked = read(all);
        assert_eq!(end_plan(&forked, 500.0, at(500), &lab_only).unwrap(), EndPlan::Targets(vec![forked.procs.last().unwrap().clone()]));
        let boxed = ActivityRead {
            scopes: vec![ActivityScope { roots: vec![], cgroup: Some("/wsp-threads/lab-thread-1".into()), ..lab() }],
            ..read(vec![row_with(
                500,
                1,
                "node",
                "node vite.js",
                json!({ "owner": { "by": "env", "thread": LAB_THREAD }, "cgroup": "/wsp-threads/lab-thread-1" }),
            )])
        };
        assert_eq!(end_plan(&boxed, 500.0, at(500), &lab_only).unwrap(), EndPlan::Targets(vec![boxed.procs[0].clone()]));
    }
}

mod what_a_thread_s_pane_and_its_slate_started {
    use super::*;

    fn pane_procs() -> Vec<Value> {
        let root = json!({ "by": "root", "thread": LAB_THREAD });
        let mut all = procs();
        all.push(row_with(600, 60, "zsh", "-zsh", json!({ "pty": "pty_1", "kind": "terminal", "owner": root })));
        all.push(row_with(601, 600, "node", "node /lab/node_modules/.bin/vite --port 5199", json!({ "kind": "terminal", "owner": root })));
        all.push(row_with(602, 60, "zsh", "-zsh", json!({ "pty": "pty_2" })));
        all.push(row_with(700, 50, "python3", "python3 /state/slates/lab-thread-1/feed.py", json!({ "kind": "slate", "owner": root })));
        all.push(row_with(701, 50, "node", "node /state/servers/github/index.js", json!({ "kind": "tool", "owner": root })));
        all
    }

    // activity.test.ts:197
    #[test]
    fn lets_the_thread_end_a_shell_its_pane_opened_and_what_runs_in_it_as_the_person_ca() {
        let person = |_: Option<&str>| true;
        let thread = |t: Option<&str>| t == Some(LAB_THREAD);
        let other = |t: Option<&str>| t == Some(KEPT_THREAD);
        let may: [&dyn Fn(Option<&str>) -> bool; 2] = [&person, &thread];
        for may in may {
            assert_eq!(pids_of(end_plan(&read(pane_procs()), 601.0, at(601), may).unwrap()), vec![601]);
            assert_eq!(pids_of(end_plan(&read(pane_procs()), 600.0, at(600), may).unwrap()), vec![600, 601]);
        }
        for pid in [600_i64, 601] {
            let said = refusal(|| end_plan(&read(pane_procs()), pid as f64, at(pid), &other));
            let line = not_threads_line(pid);
            assert!(said.contains(&line), "{said}");
        }
    }
}

mod an_agent_outside_this_host_s_threads {
    use super::*;

    const OUTSIDE_START: i64 = 1_760_000_000_000;
    const MB: i64 = 1024 * 1024;

    fn outside_row(pid: i64, ppid: i64, comm: &str, cmdline: &str, mb: i64, extra: Value) -> Value {
        over(
            json!({ "pid": pid, "ppid": ppid, "user": "dev", "state": "S", "comm": comm, "cmdline": cmdline, "cpu": 0, "rss": mb * MB, "startedAt": OUTSIDE_START + pid }),
            extra,
        )
    }

    /// A Claude Code turn this host does not hold, as a Mac's daemon reads it: the native binary named by its version,
    /// under its run script, under another wsp's app and daemon, which listen; with two tool servers and the python
    /// each runs, the script down stamped by the WSP_THREAD mark of that wsp's thread. Beside it a Codex npm launcher
    /// and the native codex it runs, of a second such thread, and the person's own Claude Code, which no wsp started.
    fn outside_procs() -> Vec<Value> {
        let other = json!({ "owner": { "by": "env", "thread": "th_other" } });
        let third = json!({ "owner": { "by": "env", "thread": "th_third" } });
        vec![
            outside_row(1, 0, "launchd", "/sbin/launchd", 10, json!({})),
            outside_row(10, 1, "node", "node wsp host", 200, json!({})),
            outside_row(20, 10, "wsp-daemon", "wsp-daemon serve", 4, json!({})),
            outside_row(400, 1, "Electron", "/Applications/wsp.app/Contents/MacOS/wsp", 300, json!({})),
            outside_row(410, 400, "wsp-daemon", "/Applications/wsp.app/Contents/Resources/daemon/wsp-daemon serve", 6, json!({})),
            outside_row(450, 410, "bash", "bash /home/dev/wsp-work/state/runs/0a1b2c3d4e5f.sh", 2, other.clone()),
            outside_row(
                500,
                450,
                "2.1.296",
                "claude -p --input-format stream-json --output-format stream-json --verbose --model claude-opus-5-5",
                640,
                over(other.clone(), json!({ "cpu": 3 })),
            ),
            outside_row(510, 500, "uv", "/opt/homebrew/bin/uv tool uvx mcp-search-console", 30, other.clone()),
            outside_row(
                511,
                510,
                "python3.13",
                "/home/dev/.cache/uv/archive-v0/djbB/bin/python /home/dev/.cache/uv/archive-v0/djbB/bin/mcp-search-console",
                64,
                other.clone(),
            ),
            outside_row(520, 500, "node", "node /home/dev/.npm/_npx/9f1/node_modules/.bin/mcp-server-github", 80, other.clone()),
            outside_row(530, 500, "zsh", "/bin/zsh -c ls", 3, other),
            outside_row(600, 1, "node", "node /opt/homebrew/bin/codex exec --json -", 40, third.clone()),
            outside_row(
                601,
                600,
                "codex",
                "/opt/homebrew/lib/node_modules/@openai/codex/vendor/aarch64-apple-darwin/codex/codex exec --json -",
                210,
                third,
            ),
            outside_row(700, 1, "2.1.296", "claude --resume", 420, json!({})),
        ]
    }

    // activity.test.ts:250
    #[test]
    fn still_ends_it_from_no_thread_since_no_thread_s_stop_will() {
        let anyone = |_: Option<&str>| true;
        let outside = ActivityRead {
            computer: "lab-mac".into(),
            at: JsNumber(OUTSIDE_START as f64),
            procs: serde_json::from_value(Value::Array(outside_procs())).unwrap(),
            daemon: 20,
            host: 10,
            scopes: vec![],
            agent_bins: vec!["claude".into(), "codex".into()],
            ports: Some(HashMap::from([(400, vec![4400, 4420]), (410, vec![58815])])),
            cgroups: None,
        };
        let plan = end_plan(&outside, 500.0, (OUTSIDE_START + 500) as f64, &anyone).unwrap();
        let pids = match plan {
            EndPlan::Targets(t) => Some(t.iter().map(|p| p.pid).collect::<Vec<u32>>()),
            EndPlan::Kept(_) => None,
        };
        assert_eq!(pids, Some(vec![500, 510, 511, 520, 530]));
    }
}
