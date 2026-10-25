// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/protocol/test/activity.test.ts at a3b870a6e: Activity's reading of one snapshot, the stretch
//! from a root to the agent's binary, and what a thread leaves running. The End's plan and the words of the processes
//! left are left to the words writer.

use std::collections::{BTreeMap, HashMap};

use serde_json::Value;
use wsp_frames::{ProcEntry, ProcKind, ProcOwnedBy, ProcOwner};
use wsp_wire::types::JsNumber;
use wsp_wire::views::activity::{
    activity_of, end_heads, place_procs, processes_left, ActivityProc, ActivityRead, ActivityScope, ActivityThread, ActivityView,
    PlaceInput,
};
use wsp_wire::views::activity_fold::{fold_tree, Folded, PidRow};

const START: i64 = 1_760_000_000_000;

fn row(pid: u32, ppid: u32, comm: &str, cmdline: &str) -> ProcEntry {
    ProcEntry {
        pid,
        ppid,
        user: "ada".into(),
        state: "S".into(),
        comm: comm.into(),
        cmdline: cmdline.into(),
        cpu: 1.0,
        rss: 1_000,
        started_at: START + pid as i64,
        pty: None,
        cgroup: None,
        energy_mw: None,
        pgid: None,
        owner: None,
        kind: None,
        id: None,
    }
}

fn owned(by: ProcOwnedBy, thread: &str) -> Option<ProcOwner> {
    Some(ProcOwner { by, thread: Some(thread.into()), workspace: None, warm: false })
}

fn scope(thread_id: &str, title: &str, roots: &[u32], kept: Option<u32>) -> ActivityScope {
    ActivityScope {
        thread_id: thread_id.into(),
        title: title.into(),
        workspace_id: "ws-1".into(),
        roots: roots.to_vec(),
        kept,
        ..Default::default()
    }
}

fn lab() -> ActivityScope {
    scope("lab-thread-1", "dev server", &[100], None)
}

fn kept() -> ActivityScope {
    scope("kept-thread-2", "kept", &[300], Some(300))
}

const LAB_THREAD: &str = "lab-thread-1";
const KEPT_THREAD: &str = "kept-thread-2";

/// A turn's run script and its shell down to a native install of Claude Code, which runs as its version, with vite
/// and esbuild under it; a kept agent; a server the daemon owns by the ladder with no live parent; the daemon, the host
/// and a shell of the person's own.
fn procs() -> Vec<ProcEntry> {
    vec![
        row(1, 0, "launchd", "/sbin/launchd"),
        row(100, 50, "bash", "bash runs/53af26c40316.sh"),
        row(101, 100, "bash", "bash"),
        row(102, 101, "2.1.296", "claude -p --input-format stream-json"),
        ProcEntry { cpu: 9.0, ..row(200, 102, "node", "node /lab/node_modules/.bin/vite") },
        row(201, 200, "esbuild", "esbuild --service"),
        row(300, 50, "node", "/usr/local/bin/cursor-agent --print"),
        ProcEntry { owner: owned(ProcOwnedBy::Fork, LAB_THREAD), ..row(400, 1, "node", "node detached.js") },
        row(50, 1, "node", "node wsp host"),
        row(60, 1, "wsp-daemon", "wsp-daemon serve"),
        row(70, 1, "zsh", "-zsh"),
    ]
}

fn read(procs: Vec<ProcEntry>) -> ActivityRead {
    ActivityRead {
        computer: "lab-mac".into(),
        at: JsNumber(START as f64),
        procs,
        daemon: 60,
        host: 50,
        scopes: vec![lab(), kept()],
        agent_bins: vec!["claude".into(), "cursor-agent".into()],
        ports: None,
        cgroups: None,
    }
}

fn shape_of(t: &ActivityThread) -> (String, Vec<(i64, i64, ProcKind)>) {
    (t.thread_id.clone(), t.processes.iter().map(|p| (p.pid, p.depth, p.kind)).collect())
}

fn kinds_of(rows: &[ActivityProc]) -> Vec<(i64, ProcKind)> {
    rows.iter().map(|p| (p.pid, p.kind)).collect()
}

fn pids_of(rows: &[ActivityProc]) -> Vec<i64> {
    rows.iter().map(|p| p.pid).collect()
}

mod a_snapshot_read_by_thread {
    use super::*;

    // activity.test.ts:34 at a3b870a6e
    #[test]
    fn puts_everything_under_a_thread_s_root_in_its_tree_the_stretch_down_to_the_agent_s_b() {
        let view = activity_of(&read(procs()));
        let threads: Vec<_> = view.threads.iter().map(shape_of).collect();
        assert_eq!(
            threads,
            vec![
                (
                    LAB_THREAD.to_string(),
                    vec![
                        (100, 0, ProcKind::Agent),
                        (101, 1, ProcKind::Agent),
                        (102, 2, ProcKind::Agent),
                        (200, 3, ProcKind::Dev),
                        (201, 4, ProcKind::Dev),
                        (400, 0, ProcKind::Other),
                    ]
                ),
                (KEPT_THREAD.to_string(), vec![(300, 0, ProcKind::Agent)]),
            ]
        );
        assert_eq!(view.threads[0].cpu, JsNumber(14.0));
        assert_eq!(pids_of(&view.wsp), vec![50, 60]);
        assert_eq!(view.unowned, vec![]);
        assert!(!serde_json::to_string(&view).unwrap().contains("-zsh"));
    }

    // activity.test.ts:46 at a3b870a6e
    #[test]
    fn leaves_a_settle_what_outlives_the_agent_what_runs_under_the_agent_ends_with_it() {
        let left = processes_left(&activity_of(&read(procs())), &[LAB_THREAD.to_string(), KEPT_THREAD.to_string()]);
        let got: Vec<(String, Vec<i64>)> = left.iter().map(|t| (t.thread_id.clone(), pids_of(&t.processes))).collect();
        assert_eq!(got, vec![(LAB_THREAD.to_string(), vec![400])]);
        // left to the words writer: processesLeftLine("settle", left) is "a thread to settle still runs a process: 400 node (other) of lab-thre:"
    }

    // activity.test.ts:52 at a3b870a6e
    #[test]
    fn counts_a_kept_agent_s_input_pump_and_the_script_s_shell_it_runs_in_left_under_pid_1() {
        let kept = scope("kept-thread-3", "kept on linux", &[500], Some(500));
        let group = |p: ProcEntry| ProcEntry { pgid: Some(500), owner: owned(ProcOwnedBy::Group, "kept-thread-3"), ..p };
        let procs = vec![
            ProcEntry { pgid: Some(500), ..row(500, 50, "bash", "bash /tmp/lab/runs/3d518f162d41.sh") },
            ProcEntry { pgid: Some(500), ..row(501, 500, "claude", "claude -p --input-format stream-json") },
            group(row(502, 1, "tail", "tail -n +1 -f /tmp/lab/runs/3d518f162d41.in")),
            group(row(503, 1, "bash", "bash /tmp/lab/runs/3d518f162d41.sh")),
            group(row(504, 1, "node", "node node_modules/.bin/vite")),
            group(row(505, 1, "tail", "tail -n +1 -f /tmp/notes.in")),
        ];
        let view = activity_of(&ActivityRead { scopes: vec![kept], ..read(procs) });
        let got = kinds_of(&view.threads[0].processes);
        for want in [(502, ProcKind::Agent), (503, ProcKind::Agent), (504, ProcKind::Dev), (505, ProcKind::Other)] {
            assert!(got.contains(&want), "{want:?} not in {got:?}");
        }
        assert_eq!(pids_of(&processes_left(&view, &["kept-thread-3".to_string()])[0].processes), vec![504, 505]);
    }
}

mod what_a_thread_leaves_running {
    use super::*;

    fn proc(pid: i64, ppid: i64, kind: ProcKind, command: &str) -> ActivityProc {
        ActivityProc {
            pid,
            ppid,
            depth: 0,
            started_at: JsNumber((START + pid) as f64),
            kind,
            name: command.split(' ').next().unwrap().to_string(),
            command: command.to_string(),
            memory: JsNumber(1_000.0),
            cpu: JsNumber(0.0),
            ports: vec![],
        }
    }

    fn view(processes: Vec<ActivityProc>) -> ActivityView {
        ActivityView {
            computer: "lab-mac".into(),
            at: JsNumber(START as f64),
            threads: vec![ActivityThread {
                thread_id: LAB_THREAD.into(),
                title: "lab".into(),
                workspace_id: "ws-1".into(),
                memory: JsNumber(0.0),
                cpu: JsNumber(0.0),
                processes,
            }],
            wsp: vec![],
            unowned: vec![],
        }
    }

    // activity.test.ts:156 at a3b870a6e
    #[test]
    fn keeps_the_turn_s_input_pump_and_run_script_with_the_agent_however_a_reader_kinds_th() {
        let left = processes_left(
            &view(vec![
                proc(100, 1, ProcKind::Agent, "claude -p --input-format stream-json"),
                proc(110, 100, ProcKind::Tool, "node wsp mcp --scoped"),
                proc(120, 1, ProcKind::Other, "tail -n +1 -f /tmp/lab/runs/3d518f162d41.in"),
                proc(121, 1, ProcKind::Other, "bash /tmp/lab/runs/3d518f162d41.sh"),
                proc(200, 1, ProcKind::Dev, "node node_modules/.bin/vite"),
                proc(201, 200, ProcKind::Other, "esbuild --service"),
                proc(300, 1, ProcKind::Test, "node node_modules/.bin/vitest"),
            ]),
            &[LAB_THREAD.to_string()],
        );
        assert_eq!(pids_of(&left[0].processes), vec![200, 201, 300]);
        assert_eq!(pids_of(&end_heads(&left[0].processes)), vec![200, 300]);
    }
}

mod what_a_thread_s_pane_and_its_slate_started {
    use super::*;

    fn pane_procs() -> Vec<ProcEntry> {
        let root = || owned(ProcOwnedBy::Root, LAB_THREAD);
        let mut all = procs();
        all.extend([
            ProcEntry { pty: Some("pty_1".into()), kind: Some(ProcKind::Terminal), owner: root(), ..row(600, 60, "zsh", "-zsh") },
            ProcEntry {
                kind: Some(ProcKind::Terminal),
                owner: root(),
                ..row(601, 600, "node", "node /lab/node_modules/.bin/vite --port 5199")
            },
            ProcEntry { pty: Some("pty_2".into()), ..row(602, 60, "zsh", "-zsh") },
            ProcEntry {
                kind: Some(ProcKind::Slate),
                owner: root(),
                ..row(700, 50, "python3", "python3 /state/slates/lab-thread-1/feed.py")
            },
            ProcEntry { kind: Some(ProcKind::Tool), owner: root(), ..row(701, 50, "node", "node /state/servers/github/index.js") },
        ]);
        all
    }

    // activity.test.ts:185 at a3b870a6e
    #[test]
    fn puts_a_shell_its_pane_opened_under_the_thread_as_a_terminal_what_runs_in_it_by_its() {
        let view = activity_of(&read(pane_procs()));
        let lab = view.threads.iter().find(|t| t.thread_id == LAB_THREAD).unwrap();
        let from_600: Vec<(i64, ProcKind)> = lab.processes.iter().filter(|p| p.pid >= 600).map(|p| (p.pid, p.kind)).collect();
        assert_eq!(from_600, vec![(600, ProcKind::Terminal), (601, ProcKind::Dev), (700, ProcKind::Slate), (701, ProcKind::Tool)]);
        assert_eq!(kinds_of(&view.unowned), vec![(602, ProcKind::Terminal)]);
    }

    // activity.test.ts:208 at a3b870a6e
    #[test]
    fn leaves_a_settle_or_a_delete_what_its_slate_runs_and_its_pane_s_shells_with_what_run() {
        let left = processes_left(&activity_of(&read(pane_procs())), &[LAB_THREAD.to_string()]);
        assert_eq!(pids_of(&left[0].processes), vec![400, 600, 601, 700, 701]);
        assert_eq!(pids_of(&end_heads(&left[0].processes)), vec![400, 600, 700, 701]);
    }
}

mod one_fold_walk_for_the_page_and_the_command_line {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct Pair {
        pid: i64,
        ppid: i64,
    }

    impl PidRow for Pair {
        fn pid(&self) -> i64 {
            self.pid
        }
        fn ppid(&self) -> i64 {
            self.ppid
        }
    }

    fn pair(pid: i64, ppid: i64) -> Pair {
        Pair { pid, ppid }
    }

    #[derive(Debug, PartialEq)]
    struct Shape {
        pid: i64,
        held: Vec<i64>,
        kids: Vec<Shape>,
    }

    fn shape(f: &Folded<Pair>) -> Shape {
        Shape { pid: f.p.pid, held: f.held.iter().map(|h| h.pid).collect(), kids: f.kids.iter().map(shape).collect() }
    }

    // activity.test.ts:217 at a3b870a6e
    #[test]
    fn hangs_each_process_under_its_parent_a_folded_row_held_by_the_row_it_folds_into_and() {
        // 10 run script, 11 its fork, 12 the agent, 13 a tool server, 14 the pump under pid 1, 20 a dev server under the script
        let list = vec![pair(10, 1), pair(11, 10), pair(12, 11), pair(13, 12), pair(14, 1), pair(20, 10)];
        let into: HashMap<i64, i64> = HashMap::from([(10, 12), (11, 12), (14, 12)]);
        let got: Vec<Shape> = fold_tree(&list, |pid| into.get(&pid).copied()).iter().map(shape).collect();
        assert_eq!(
            got,
            vec![Shape {
                pid: 12,
                held: vec![10, 11, 14],
                kids: vec![Shape { pid: 13, held: vec![], kids: vec![] }, Shape { pid: 20, held: vec![], kids: vec![] }],
            }]
        );
    }

    // activity.test.ts:224 at a3b870a6e
    #[test]
    fn leaves_a_row_whose_target_is_outside_the_list_standing_on_its_own() {
        let got: Vec<i64> = fold_tree(&[pair(10, 1)], |_| Some(99)).iter().map(|f| f.p.pid).collect();
        assert_eq!(got, vec![10]);
    }
}

mod an_agent_outside_this_host_s_threads {
    use super::*;

    const OUTSIDE: &str = include_str!("fixtures/activity_outside.json");

    struct Outside {
        start: i64,
        procs: Vec<ProcEntry>,
        ports: HashMap<u32, Vec<i64>>,
    }

    fn outside_fixture() -> Outside {
        let v: Value = serde_json::from_str(OUTSIDE).unwrap();
        let ports: HashMap<String, Vec<i64>> = serde_json::from_value(v["ports"].clone()).unwrap();
        Outside {
            start: v["start"].as_i64().unwrap(),
            procs: serde_json::from_value(v["procs"].clone()).unwrap(),
            ports: ports.into_iter().map(|(k, v)| (k.parse().unwrap(), v)).collect(),
        }
    }

    fn outside_read(o: &Outside) -> ActivityRead {
        ActivityRead {
            computer: "lab-mac".into(),
            at: JsNumber(o.start as f64),
            procs: o.procs.clone(),
            daemon: 20,
            host: 10,
            scopes: vec![],
            agent_bins: vec!["claude".into(), "codex".into()],
            ports: Some(o.ports.clone()),
            cgroups: None,
        }
    }

    // activity.test.ts:234 at a3b870a6e
    #[test]
    fn reads_a_claude_code_another_wsp_s_thread_runs_as_an_agent_though_a_listening_proces() {
        let view = activity_of(&outside_read(&outside_fixture()));
        let got: BTreeMap<i64, ProcKind> = view.unowned.iter().map(|p| (p.pid, p.kind)).collect();
        let want = BTreeMap::from([
            (450, ProcKind::Dev),
            (500, ProcKind::Agent),
            (510, ProcKind::Tool),
            (511, ProcKind::Other),
            (520, ProcKind::Tool),
            (530, ProcKind::Other),
            (600, ProcKind::Agent),
            (601, ProcKind::Agent),
        ]);
        assert_eq!(got, want);
    }

    // activity.test.ts:238 at a3b870a6e
    #[test]
    fn leaves_an_agent_binary_a_thread_of_this_host_left_running_to_its_settle_as_no_agen() {
        let mut all = procs();
        all.push(ProcEntry { owner: owned(ProcOwnedBy::Env, "th_1"), ..row(900, 1, "2.1.296", "claude -p summarise the diff") });
        let orphan = ActivityRead { scopes: vec![lab(), kept(), scope("th_1", "summary", &[], None)], ..read(all) };
        let view = activity_of(&orphan);
        let th_1 = view.threads.iter().find(|t| t.thread_id == "th_1").unwrap();
        assert_eq!(kinds_of(&th_1.processes), vec![(900, ProcKind::Other)]);
        let left: Vec<i64> = processes_left(&view, &["th_1".to_string()]).iter().flat_map(|t| pids_of(&t.processes)).collect();
        assert_eq!(left, vec![900]);
    }

    // activity.test.ts:245 at a3b870a6e
    #[test]
    fn reads_the_person_s_own_claude_code_as_an_agent_too_and_offers_no_end_on_it() {
        let o = outside_fixture();
        let bins = vec!["claude".to_string(), "codex".to_string()];
        let listens = |pid: u32| o.ports.contains_key(&pid);
        let input =
            PlaceInput { procs: &o.procs, daemon: 20, host: Some(10), threads: &[], agent_bins: &bins, listens: &listens, app: None };
        let placed = place_procs(&input);
        assert_eq!(placed.whose.get(&700), None);
        assert_eq!(placed.kind_of(700), ProcKind::Agent);
        assert!(!placed.endable(700));
    }
}
