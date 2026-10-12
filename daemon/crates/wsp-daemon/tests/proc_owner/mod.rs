// SPDX-License-Identifier: AGPL-3.0-only
use std::collections::HashMap;
use std::path::PathBuf;

use wsp_frames::{ProcEntry, ProcKind, ProcOwnedBy, ProcOwner, ProcScope, ProcScopePid};

use super::{mark_of, Mark, Owned as Named, OwnerFacts, Ownership};

fn row(pid: u32, ppid: u32, pgid: u32) -> ProcEntry {
    ProcEntry {
        pid,
        ppid,
        user: "tester".into(),
        state: "S".into(),
        comm: "x".into(),
        cmdline: String::new(),
        cpu: 0.0,
        rss: 0,
        started_at: 1_000 + i64::from(pid),
        pty: None,
        cgroup: None,
        energy_mw: None,
        pgid: Some(pgid),
        owner: None,
        kind: None,
    }
}

fn scope(thread: &str) -> ProcScope {
    ProcScope {
        thread: thread.into(),
        workspace: Some("ws_1".into()),
        agents: vec![],
        groups: vec![],
        folder: None,
        warm: false,
        cgroup: None,
        pids: vec![],
    }
}

#[derive(Default)]
struct Facts {
    marks: HashMap<u32, String>,
    slates: std::collections::HashSet<u32>,
    folders: HashMap<u32, PathBuf>,
    coalitions: HashMap<u32, u64>,
    uniques: HashMap<u32, u64>,
    forks: HashMap<u32, Named>,
    reads: std::cell::Cell<usize>,
}

impl OwnerFacts for Facts {
    fn thread_mark(&self, pid: u32) -> Option<Mark> {
        self.reads.set(self.reads.get() + 1);
        self.marks.get(&pid).map(|thread| Mark { thread: thread.clone(), slate: self.slates.contains(&pid) })
    }
    fn folder(&self, pid: u32) -> Option<PathBuf> {
        self.folders.get(&pid).cloned()
    }
    fn coalition(&self, pid: u32) -> Option<u64> {
        self.coalitions.get(&pid).copied()
    }
    fn unique_id(&self, pid: u32) -> Option<u64> {
        self.uniques.get(&pid).copied()
    }
    fn forked(&self, pid: u32) -> Option<Named> {
        self.forks.get(&pid).cloned()
    }
}

/// A row's pid, the rung and thread that own it, and its kind.
type Owned = (u32, Option<(ProcOwnedBy, Option<String>)>, Option<ProcKind>);

fn owners(procs: &[ProcEntry]) -> Vec<Owned> {
    procs.iter().map(|p| (p.pid, p.owner.as_ref().map(|o| (o.by, o.thread.clone())), p.kind)).collect()
}

fn t(by: ProcOwnedBy, thread: &str) -> Option<(ProcOwnedBy, Option<String>)> {
    Some((by, Some(thread.to_owned())))
}

#[test]
fn the_agent_its_turn_group_and_what_the_host_started_are_the_threads_own_and_their_children_follow() {
    // 100 the agent leading group 100; 101 a tool server in its group; 110 a Bash call's shell in a group of its
    // own under the agent, 111 its child; 200 a slate run the host started, 201 the python under it; 300 nobody's.
    let mut procs = vec![
        row(100, 1, 100),
        row(101, 100, 100),
        row(110, 100, 110),
        row(111, 110, 110),
        row(200, 50, 200),
        row(201, 200, 200),
        row(300, 1, 300),
    ];
    let mut s = scope("t1");
    s.agents = vec![100];
    s.groups = vec![100];
    s.pids = vec![ProcScopePid { pid: 200, kind: ProcKind::Slate }];
    Ownership::default().stamp(&mut procs, &[s], &Facts::default(), 4242);
    assert_eq!(
        owners(&procs),
        vec![
            (100, t(ProcOwnedBy::Root, "t1"), Some(ProcKind::Agent)),
            (101, t(ProcOwnedBy::Root, "t1"), None),
            (110, t(ProcOwnedBy::Root, "t1"), None),
            (111, t(ProcOwnedBy::Root, "t1"), None),
            (200, t(ProcOwnedBy::Root, "t1"), Some(ProcKind::Slate)),
            (201, t(ProcOwnedBy::Root, "t1"), Some(ProcKind::Slate)),
            (300, None, None),
        ]
    );
    assert_eq!(
        procs[0].owner,
        Some(ProcOwner { by: ProcOwnedBy::Root, thread: Some("t1".into()), workspace: Some("ws_1".into()), warm: false })
    );
}

#[test]
fn a_group_seen_under_its_thread_keeps_its_members_once_the_shell_that_led_it_is_gone_and_is_forgotten_when_empty() {
    let mut s = scope("t1");
    s.agents = vec![100];
    let mut ownership = Ownership::default();
    // The Bash call's shell 110 leads group 110 and started 111 with & and 112 with nohup.
    let mut seen = vec![row(100, 1, 100), row(110, 100, 110), row(111, 110, 110), row(112, 110, 110)];
    ownership.stamp(&mut seen, std::slice::from_ref(&s), &Facts::default(), 4242);
    // The shell exited: its children went to init, still in its group.
    let mut after = vec![row(100, 1, 100), row(111, 1, 110), row(112, 1, 110), row(113, 112, 113)];
    ownership.stamp(&mut after, std::slice::from_ref(&s), &Facts::default(), 4242);
    assert_eq!(
        owners(&after)[1..],
        [(111, t(ProcOwnedBy::Group, "t1"), None), (112, t(ProcOwnedBy::Group, "t1"), None), (113, t(ProcOwnedBy::Group, "t1"), None)]
    );
    // The group emptied, and a stranger's new group took the number.
    let mut empty = vec![row(100, 1, 100)];
    ownership.stamp(&mut empty, std::slice::from_ref(&s), &Facts::default(), 4242);
    let mut reused = vec![row(100, 1, 100), row(110, 1, 110)];
    ownership.stamp(&mut reused, std::slice::from_ref(&s), &Facts::default(), 4242);
    assert_eq!(reused[1].owner, None);
}

#[test]
fn a_slate_run_from_one_threads_slate_is_that_threads_never_wsps_or_another_threads_in_the_same_folder() {
    // Threads A and B work in one folder. The host 50 started A's slate run 200 (python3 "$SLATE_DIR/x.py") and A's
    // slate MCP client 210; the run's child 201. B's agent 300 runs in the same folder. wsp's coalition holds them all.
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().canonicalize().unwrap();
    let mut a = scope("t_a");
    a.folder = Some(folder.to_string_lossy().into_owned());
    a.pids = vec![ProcScopePid { pid: 200, kind: ProcKind::Slate }, ProcScopePid { pid: 210, kind: ProcKind::Tool }];
    let mut b = scope("t_b");
    b.agents = vec![300];
    b.folder = Some(folder.to_string_lossy().into_owned());
    let mut facts = Facts::default();
    for pid in [50, 200, 201, 210, 300] {
        facts.folders.insert(pid, folder.clone());
        facts.coalitions.insert(pid, 9);
    }
    facts.coalitions.insert(4242, 9);
    let mut procs = vec![row(50, 1, 50), row(200, 50, 50), row(201, 200, 50), row(210, 50, 50), row(300, 1, 300)];
    Ownership::default().stamp(&mut procs, &[a, b], &facts, 4242);
    assert_eq!(
        owners(&procs),
        vec![
            (50, Some((ProcOwnedBy::Folder, None)), None),
            (200, t(ProcOwnedBy::Root, "t_a"), Some(ProcKind::Slate)),
            (201, t(ProcOwnedBy::Root, "t_a"), Some(ProcKind::Slate)),
            (210, t(ProcOwnedBy::Root, "t_a"), Some(ProcKind::Tool)),
            (300, t(ProcOwnedBy::Root, "t_b"), Some(ProcKind::Agent)),
        ]
    );
}

#[test]
fn a_group_whose_leader_is_not_the_threads_is_never_remembered_for_it() {
    // The host 50 leads its own group; a tool server 60 it started without a group of its own is the thread's, and
    // the host's group must not become the thread's with it.
    let mut s = scope("t1");
    s.pids = vec![ProcScopePid { pid: 60, kind: ProcKind::Tool }];
    let mut ownership = Ownership::default();
    let mut procs = vec![row(50, 1, 50), row(60, 50, 50), row(70, 50, 50)];
    ownership.stamp(&mut procs, std::slice::from_ref(&s), &Facts::default(), 4242);
    assert_eq!(owners(&procs), vec![(50, None, None), (60, t(ProcOwnedBy::Root, "t1"), Some(ProcKind::Tool)), (70, None, None)]);
    let mut later = vec![row(50, 1, 50), row(70, 50, 50)];
    ownership.stamp(&mut later, &[], &Facts::default(), 4242);
    assert_eq!(later[1].owner, None);
}

#[test]
fn the_mark_cgroup_folder_and_coalition_rungs_in_order_each_read_only_for_what_the_rungs_above_left() {
    let dir = tempfile::tempdir().unwrap();
    let folder = dir.path().canonicalize().unwrap();
    let mut a = scope("t_a");
    a.agents = vec![100];
    a.folder = Some(folder.to_string_lossy().into_owned());
    let mut b = scope("t_b");
    b.workspace = Some("ws_2".into());
    b.cgroup = Some("/wsp-threads/t_b".into());
    let mut facts = Facts::default();
    // 100's own mark says another thread: the first rung wins.
    facts.marks.insert(100, "t_b".into());
    // 400 a setsid dev server marked t_a, 401 its child, unmarked.
    facts.marks.insert(400, "t_a".into());
    // 500 stands in t_b's cgroup; 510 in t_b10's, which is not under t_b however its name begins.
    let mut in_b = row(500, 1, 500);
    in_b.cgroup = Some("/wsp-threads/t_b/run".into());
    let mut in_b10 = row(510, 1, 510);
    in_b10.cgroup = Some("/wsp-threads/t_b10".into());
    // 600 runs in a's folder with nothing else to go on; 700 in wsp's coalition; 800 nobody's.
    facts.folders.insert(600, folder.join("src"));
    facts.folders.insert(800, PathBuf::from("/elsewhere"));
    facts.coalitions.extend([(4242, 9), (700, 9), (800, 7)]);
    let mut procs =
        vec![row(100, 1, 100), row(400, 1, 400), row(401, 400, 400), in_b, in_b10, row(600, 1, 600), row(700, 1, 700), row(800, 1, 800)];
    Ownership::default().stamp(&mut procs, &[a, b], &facts, 4242);
    let got: Vec<_> = procs.iter().map(|p| (p.pid, p.owner.clone())).collect();
    let o = |by, thread: Option<&str>, workspace: Option<&str>| {
        Some(ProcOwner { by, thread: thread.map(Into::into), workspace: workspace.map(Into::into), warm: false })
    };
    assert_eq!(
        got,
        vec![
            (100, o(ProcOwnedBy::Root, Some("t_a"), Some("ws_1"))),
            (400, o(ProcOwnedBy::Env, Some("t_a"), Some("ws_1"))),
            (401, o(ProcOwnedBy::Env, Some("t_a"), Some("ws_1"))),
            (500, o(ProcOwnedBy::Cgroup, Some("t_b"), Some("ws_2"))),
            (510, None),
            (600, o(ProcOwnedBy::Folder, None, Some("ws_1"))),
            (700, o(ProcOwnedBy::Coalition, None, None)),
            (800, None),
        ]
    );
    // 100 was named on the first rung, so its mark is never read.
    assert_eq!(facts.reads.get(), 7);
}

#[test]
fn a_workspace_folder_swapped_for_a_link_names_nobody_and_a_real_one_names_what_runs_under_it() {
    let dir = tempfile::tempdir().unwrap();
    let top = dir.path().canonicalize().unwrap();
    let real = top.join("ws");
    std::fs::create_dir_all(real.join("src")).unwrap();
    // The agent replaced its worktree with a link to the root of the computer.
    let swapped = top.join("swapped");
    std::os::unix::fs::symlink("/", &swapped).unwrap();
    let mut a = scope("t_a");
    a.folder = Some(real.to_string_lossy().into_owned());
    let mut b = scope("t_b");
    b.workspace = Some("ws_2".into());
    b.folder = Some(swapped.to_string_lossy().into_owned());
    let mut facts = Facts::default();
    // 600 in a's folder, 601 in a folder under it, 602 anywhere else on the computer.
    facts.folders.insert(600, real.clone());
    facts.folders.insert(601, real.join("src"));
    facts.folders.insert(602, PathBuf::from("/usr"));
    let mut procs = vec![row(600, 1, 600), row(601, 1, 601), row(602, 1, 602)];
    Ownership::default().stamp(&mut procs, &[a, b], &facts, 4242);
    let got: Vec<_> = procs.iter().map(|p| (p.pid, p.owner.as_ref().map(|o| (o.by, o.workspace.clone())))).collect();
    let ws1 = Some((ProcOwnedBy::Folder, Some("ws_1".to_owned())));
    assert_eq!(got, vec![(600, ws1.clone()), (601, ws1), (602, None)]);
}

#[test]
fn every_row_of_a_warm_agent_reads_warm_until_a_send_claims_it() {
    // 100 a warm agent kept for a thread no send has claimed, 101 its child, 400 a setsid server carrying its mark;
    // 200 a claimed thread's agent.
    let mut warm = scope("t_warm");
    warm.agents = vec![100];
    warm.warm = true;
    let mut claimed = scope("t1");
    claimed.agents = vec![200];
    let mut facts = Facts::default();
    facts.marks.insert(400, "t_warm".into());
    let mut procs = vec![row(100, 1, 100), row(101, 100, 100), row(200, 1, 200), row(400, 1, 400)];
    Ownership::default().stamp(&mut procs, &[warm.clone(), claimed], &facts, 4242);
    let got: Vec<_> = procs.iter().map(|p| (p.pid, p.owner.as_ref().map(|o| (o.by, o.warm)))).collect();
    assert_eq!(
        got,
        vec![
            (100, Some((ProcOwnedBy::Root, true))),
            (101, Some((ProcOwnedBy::Root, true))),
            (200, Some((ProcOwnedBy::Root, false))),
            (400, Some((ProcOwnedBy::Env, true)))
        ]
    );
    // Claimed, the same scope without the flag: the rows are the thread's like any other.
    warm.warm = false;
    let mut procs = vec![row(100, 1, 100), row(400, 1, 400)];
    Ownership::default().stamp(&mut procs, &[warm], &facts, 4242);
    assert!(procs.iter().all(|p| p.owner.as_ref().is_some_and(|o| !o.warm && o.thread.as_deref() == Some("t_warm"))));
    assert_eq!(serde_json::to_value(&procs[0].owner).unwrap(), serde_json::json!({"by": "root", "thread": "t_warm", "workspace": "ws_1"}));
}

#[test]
fn a_slate_run_on_a_box_reads_its_thread_and_kind_slate_off_its_environment_and_its_children_follow() {
    // On a box the host starts no slate run itself and names no pid: 500 is the launcher, in the thread's cgroup with
    // the mark and SLATE_DIR, 501 the command under it; 600 another process of the thread's cgroup, a turn's.
    let mut b = scope("t_b");
    b.cgroup = Some("/wsp-threads/t_b".into());
    let mut facts = Facts::default();
    facts.marks.insert(500, "t_b".into());
    facts.slates.insert(500);
    let in_b = |pid, ppid| ProcEntry { cgroup: Some("/wsp-threads/t_b".into()), ..row(pid, ppid, pid) };
    let mut procs = vec![in_b(500, 1), in_b(501, 500), in_b(600, 1)];
    Ownership::default().stamp(&mut procs, &[b], &facts, 4242);
    assert_eq!(
        owners(&procs),
        vec![
            (500, t(ProcOwnedBy::Env, "t_b"), Some(ProcKind::Slate)),
            (501, t(ProcOwnedBy::Env, "t_b"), Some(ProcKind::Slate)),
            (600, t(ProcOwnedBy::Cgroup, "t_b"), None),
        ]
    );
}

#[test]
fn a_mark_is_read_once_per_process_and_again_for_a_new_process_under_the_same_pid() {
    let mut facts = Facts::default();
    facts.marks.insert(400, "t_a".into());
    let mut ownership = Ownership::default();
    for _ in 0..3 {
        let mut procs = vec![row(400, 1, 400)];
        ownership.stamp(&mut procs, &[], &facts, 4242);
        assert_eq!(procs[0].owner.as_ref().and_then(|o| o.thread.as_deref()), Some("t_a"));
    }
    assert_eq!(facts.reads.get(), 1);
    let mut reborn = vec![ProcEntry { started_at: 9_999, ..row(400, 1, 400) }];
    ownership.stamp(&mut reborn, &[], &facts, 4242);
    assert_eq!(facts.reads.get(), 2);
}

#[test]
fn the_fork_tracker_names_a_thread_after_the_parent_chain_and_before_the_group_and_its_children_follow_with_its_kind() {
    let mut s = scope("t1");
    s.agents = vec![100];
    let mut facts = Facts::default();
    let followed = |thread: &str, kind| Named { thread: thread.into(), workspace: Some("ws_1".into()), kind };
    // 101 under the agent is the agent's by its parent, whatever the tracker says; 400, an orphan the tracker followed
    // from a slate run, and 401 under it; 500 the tracker followed while its group is remembered for another thread.
    facts.forks.insert(101, followed("t9", None));
    facts.forks.insert(400, followed("t1", Some(ProcKind::Slate)));
    facts.forks.insert(500, followed("t1", None));
    facts.marks.insert(500, "t2".into());
    let mut procs = vec![row(100, 1, 100), row(101, 100, 100), row(400, 1, 400), row(401, 400, 400), row(500, 1, 500)];
    Ownership::default().stamp(&mut procs, &[s], &facts, 4242);
    assert_eq!(
        owners(&procs),
        vec![
            (100, t(ProcOwnedBy::Root, "t1"), Some(ProcKind::Agent)),
            (101, t(ProcOwnedBy::Root, "t1"), None),
            (400, t(ProcOwnedBy::Fork, "t1"), Some(ProcKind::Slate)),
            (401, t(ProcOwnedBy::Fork, "t1"), Some(ProcKind::Slate)),
            (500, t(ProcOwnedBy::Fork, "t1"), None),
        ]
    );
}

#[test]
fn a_mark_is_read_again_for_a_new_unique_id_under_the_same_pid_and_start() {
    let mut facts = Facts::default();
    facts.marks.insert(400, "t_a".into());
    facts.uniques.insert(400, 7);
    let mut ownership = Ownership::default();
    ownership.stamp(&mut [row(400, 1, 400)], &[], &facts, 4242);
    ownership.stamp(&mut [row(400, 1, 400)], &[], &facts, 4242);
    assert_eq!(facts.reads.get(), 1);
    facts.uniques.insert(400, 8);
    ownership.stamp(&mut [row(400, 1, 400)], &[], &facts, 4242);
    assert_eq!(facts.reads.get(), 2);
}

#[test]
fn a_remembered_group_whose_number_now_leads_another_process_is_forgotten() {
    let mut s = scope("t1");
    s.agents = vec![100];
    let mut facts = Facts::default();
    facts.uniques.extend([(100, 1), (110, 2), (111, 3)]);
    let mut ownership = Ownership::default();
    ownership.stamp(&mut [row(100, 1, 100), row(110, 100, 110), row(111, 110, 110)], std::slice::from_ref(&s), &facts, 4242);
    // The shell 110 left; 111 keeps the group. Then the group's number leads a stranger, 111 still in it.
    let mut kept = vec![row(100, 1, 100), row(111, 1, 110)];
    ownership.stamp(&mut kept, std::slice::from_ref(&s), &facts, 4242);
    assert_eq!(kept[1].owner.as_ref().map(|o| o.by), Some(ProcOwnedBy::Group));
    facts.uniques.insert(110, 9);
    let mut moved = vec![row(100, 1, 100), row(110, 1, 110), row(111, 1, 110)];
    ownership.stamp(&mut moved, std::slice::from_ref(&s), &facts, 4242);
    assert_eq!((moved[1].owner.clone(), moved[2].owner.clone()), (None, None));
}

#[test]
fn a_mark_is_a_word_of_wsps_and_anything_else_is_no_mark() {
    let thread = |environ: &[u8]| mark_of(environ).map(|m| m.thread);
    assert_eq!(thread(b"PATH=/bin\0WSP_THREAD=1f0c-ab_9\0HOME=/root\0").as_deref(), Some("1f0c-ab_9"));
    assert_eq!(thread(b"XWSP_THREAD=t1\0"), None);
    assert_eq!(thread(b"WSP_THREAD=\0"), None);
    assert_eq!(thread(b"WSP_THREAD=../../etc\0"), None);
    assert_eq!(thread(format!("WSP_THREAD={}\0", "a".repeat(65)).as_bytes()), None);
    // A slate's run carries SLATE_DIR beside the mark, before it or after; a turn carries the mark alone.
    let slate = |environ: &[u8]| mark_of(environ).map(|m| m.slate);
    assert_eq!(slate(b"WSP_THREAD=t1\0SLATE_DIR=/tmp/wsp-slates/t1\0"), Some(true));
    assert_eq!(slate(b"SLATE_DIR=/tmp/wsp-slates/t1\0WSP_THREAD=t1\0"), Some(true));
    assert_eq!(slate(b"WSP_THREAD=t1\0XSLATE_DIR=/x\0"), Some(false));
    assert_eq!(slate(b"SLATE_DIR=/tmp/wsp-slates/t1\0"), None);
}

/// Real processes in every shape the ticket names, read off this computer's /proc by both roads a Linux daemon has:
/// a fake agent leads a group as a turn does, and a Bash call's shell, a group and a session of its own as Claude Code
/// makes it, starts each shape and exits.
#[cfg(target_os = "linux")]
mod shapes {
    use super::*;
    use crate::proc::{ProcFsSource, ProcScanInput, ProcSource};
    use crate::proc_local::{road_for, LocalProcSource};
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    use wsp_frames::numbers;

    /// Every process the case started, by the pid files the shapes wrote, ended however the case ends.
    struct Started(Vec<PathBuf>, Vec<u32>);

    impl Drop for Started {
        fn drop(&mut self) {
            let mut pids = self.1.clone();
            for file in &self.0 {
                if let Some(pid) = std::fs::read_to_string(file).ok().and_then(|s| s.trim().parse::<u32>().ok()) {
                    pids.push(pid);
                }
            }
            for pid in pids {
                let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGKILL);
            }
        }
    }

    fn wait_for(what: &str, ready: impl Fn() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while !ready() {
            assert!(Instant::now() < deadline, "{what} never happened in 20 s");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    fn pid_in(file: &Path) -> u32 {
        wait_for(&format!("{} written", file.display()), || std::fs::read_to_string(file).is_ok_and(|s| s.trim().parse::<u32>().is_ok()));
        std::fs::read_to_string(file).unwrap().trim().parse().unwrap()
    }

    fn alive(pid: u32) -> bool {
        std::fs::read_to_string(format!("/proc/{pid}/stat")).is_ok_and(|s| !s.contains(") Z "))
    }

    const SHAPES: [&str; 6] = ["amp", "nohup", "setsid_node", "double_fork", "env_i", "setsid_env_i"];

    /// The Bash call's script: each shape writes its own pid, the shell then waits for the gate and exits.
    fn tool_shell(dir: &Path) -> String {
        let d = dir.display();
        [
            format!("sleep 300 & echo $! > {d}/amp"),
            format!("nohup sleep 300 > /dev/null 2>&1 & echo $! > {d}/nohup"),
            format!("setsid node -e 'require(\"fs\").writeFileSync(\"{d}/setsid_node\", String(process.pid)); setTimeout(() => {{}}, 300000)' > /dev/null 2>&1 &"),
            format!("( sleep 300 & echo $! > {d}/double_fork ) &"),
            format!("env -i /bin/sleep 300 & echo $! > {d}/env_i"),
            format!("setsid env -i /bin/sh -c 'echo $$ > {d}/setsid_env_i; exec /bin/sleep 300' > /dev/null 2>&1 &"),
            format!("echo $$ > {d}/shell"),
            format!("while [ ! -f {d}/gate ]; do sleep 0.05; [ -d {d} ] || exit; done"),
        ]
        .join("\n")
    }

    fn stamped(source: &dyn ProcSource, ownership: &mut Ownership, scopes: &[ProcScope]) -> Vec<ProcEntry> {
        let mut scan = source
            .scan(&ProcScanInput { at: crate::sys::now_ms(), elapsed_ms: 0, pty: HashMap::new(), only: None, daemon: std::process::id() })
            .unwrap();
        ownership.stamp(&mut scan.procs, scopes, source, std::process::id());
        scan.procs
    }

    fn owner_of(procs: &[ProcEntry], pid: u32) -> Option<(ProcOwnedBy, Option<String>)> {
        let p = procs.iter().find(|p| p.pid == pid).unwrap_or_else(|| panic!("pid {pid} is listed"));
        p.owner.as_ref().map(|o| (o.by, o.thread.clone()))
    }

    /// Runs the shapes under a fake agent; `watched` stamps once while the Bash call's shell still lives.
    fn run(source: &dyn ProcSource, watched: bool) -> HashMap<&'static str, Option<(ProcOwnedBy, Option<String>)>> {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().canonicalize().unwrap();
        let files: Vec<PathBuf> = SHAPES.iter().chain(["shell"].iter()).map(|n| folder.join(n)).collect();
        let script = tool_shell(&folder);
        let mut agent = Command::new("bash")
            .args(["-c", "setsid bash -c \"$0\" & wait", &script])
            .current_dir(&folder)
            .env("WSP_THREAD", "t_shapes")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let agent_pid = agent.id();
        let started = Started(files.clone(), vec![agent_pid]);
        let pids: HashMap<&str, u32> = SHAPES.iter().map(|n| (*n, pid_in(&folder.join(n)))).collect();
        let shell = pid_in(&folder.join("shell"));
        let mut s = scope("t_shapes");
        s.agents = vec![agent_pid];
        s.groups = vec![agent_pid];
        s.folder = Some(folder.to_string_lossy().into_owned());
        let scopes = [s];
        let mut ownership = Ownership::default();
        if watched {
            wait_for("every shape listed", || {
                let procs = stamped(source, &mut Ownership::default(), &scopes);
                pids.values().all(|pid| procs.iter().any(|p| p.pid == *pid))
            });
            let procs = stamped(source, &mut ownership, &scopes);
            assert_eq!(
                owner_of(&procs, shell),
                Some((ProcOwnedBy::Root, Some("t_shapes".into()))),
                "the Bash call's shell is under the agent"
            );
        }
        std::fs::write(folder.join("gate"), "").unwrap();
        wait_for("the Bash call's shell exits", || !alive(shell));
        agent.wait().unwrap();
        assert!(pids.values().all(|pid| alive(*pid)), "every shape outlives the shell and the agent");
        let procs = stamped(source, &mut ownership, &scopes);
        let got = pids.iter().map(|(name, pid)| (*name, owner_of(&procs, *pid))).collect();
        drop(started);
        got
    }

    fn sources() -> Vec<(&'static str, Box<dyn ProcSource>)> {
        let (_dir, passwd) = crate::proc::fake_proc::fake_passwd();
        vec![
            ("/proc", Box::new(ProcFsSource::new(PathBuf::from("/proc"), passwd, usize::MAX)) as Box<dyn ProcSource>),
            ("ps", Box::new(LocalProcSource::new(None, road_for("linux"), numbers::PROC_CAP * 10))),
        ]
    }

    #[test]
    fn every_shape_a_bash_call_starts_stays_its_threads_once_the_shell_and_the_agent_are_gone() {
        for (road, source) in sources() {
            let got = run(source.as_ref(), true);
            let t = |by| Some((by, Some("t_shapes".to_owned())));
            assert_eq!(got["amp"], t(ProcOwnedBy::Group), "{road}");
            assert_eq!(got["nohup"], t(ProcOwnedBy::Group), "{road}");
            assert_eq!(got["double_fork"], t(ProcOwnedBy::Group), "{road}");
            assert_eq!(got["env_i"], t(ProcOwnedBy::Group), "{road}");
            // A setsid child leads a group of its own, seen under the shell while the shell lived.
            assert_eq!(got["setsid_node"], t(ProcOwnedBy::Group), "{road}");
            assert_eq!(got["setsid_env_i"], t(ProcOwnedBy::Group), "{road}");
        }
    }

    #[test]
    fn with_nobody_watching_while_the_shell_lived_the_mark_still_names_the_thread_and_the_folder_the_rest() {
        for (road, source) in sources() {
            let got = run(source.as_ref(), false);
            let marked = Some((ProcOwnedBy::Env, Some("t_shapes".to_owned())));
            for shape in ["amp", "nohup", "double_fork", "setsid_node"] {
                assert_eq!(got[shape], marked, "{road} {shape}");
            }
            assert_eq!(got["env_i"], Some((ProcOwnedBy::Folder, None)), "{road}");
            assert_eq!(got["setsid_env_i"], Some((ProcOwnedBy::Folder, None)), "{road}");
        }
    }
}

/// Real processes in twelve shapes a thread starts on a Mac, read off this computer by its own road: a fake agent
/// leads a group with the thread's mark, waits for the tracker to follow it as the host would name it, then runs each
/// shape from a zsh in a session of its own that exits at once, as Claude Code runs a Bash call, and exits itself.
/// Nobody watches while they start, so the rows are read once everything that will exit has.
#[cfg(target_os = "macos")]
mod mac_shapes {
    use super::*;
    use crate::libproc;
    use crate::proc::{ProcScanInput, ProcSource};
    use crate::proc_local::{road_for, LocalProcSource};
    use std::os::unix::process::CommandExt;
    use std::path::Path;
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};
    use wsp_frames::numbers;

    const AGENT: &str = r#"
import os, subprocess, sys, time
folder, calls = sys.argv[1], sys.argv[2:]
while not os.path.exists(os.path.join(folder, "gate")):
    time.sleep(0.02)
for call in calls:
    subprocess.Popen(["/bin/zsh", "-c", call], start_new_session=True, stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL).wait()
"#;

    fn on_path(name: &str) -> String {
        let path = std::env::var_os("PATH").unwrap_or_default();
        let found = std::env::split_paths(&path).map(|d| d.join(name)).find(|p| p.is_file());
        found.unwrap_or_else(|| panic!("{name} is on PATH")).to_string_lossy().into_owned()
    }

    /// Every pid a shape wrote, the launchd job and the screen session by their labels, and the agent: ended however
    /// the case ends, and nothing else.
    struct Started {
        folder: PathBuf,
        label: String,
        agent: u32,
    }

    impl Drop for Started {
        fn drop(&mut self) {
            let _ = Command::new("launchctl").args(["remove", &self.label]).status();
            let _ = Command::new("screen").args(["-S", &self.label, "-X", "quit"]).status();
            let mut pids = vec![self.agent];
            for entry in std::fs::read_dir(&self.folder).into_iter().flatten().flatten() {
                if entry.path().extension().is_some_and(|e| e == "pid") {
                    pids.extend(pids_in(&entry.path()));
                }
            }
            for pid in pids {
                let _ = nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), nix::sys::signal::Signal::SIGKILL);
            }
        }
    }

    /// The pids a shape wrote to a file, one a line.
    fn pids_in(file: &Path) -> Vec<u32> {
        std::fs::read_to_string(file).unwrap_or_default().lines().filter_map(|l| l.trim().parse().ok()).collect()
    }

    /// The pid launchd runs a submitted job under, once it runs it.
    fn launchd_job(label: &str) -> Option<u32> {
        let out = Command::new("launchctl").args(["list", label]).output().ok()?;
        String::from_utf8_lossy(&out.stdout).lines().find_map(|l| l.trim().strip_prefix("\"PID\" = ")?.trim_end_matches(';').parse().ok())
    }

    /// Each shape with the files its processes write their pids to, so the case reads and ends those and nothing else.
    fn shapes(dir: &Path, label: &str, python: &str) -> Vec<(&'static str, String, Vec<PathBuf>)> {
        let f = |name: &str| dir.join(format!("{name}.pid"));
        let q = |name: &str| f(name).to_string_lossy().into_owned();
        vec![
            ("sleep &", format!("/bin/sleep 600 & echo $! > {}", q("amp")), vec![f("amp")]),
            ("nohup sleep &", format!("nohup /bin/sleep 600 > /dev/null 2>&1 & echo $! > {}", q("nohup")), vec![f("nohup")]),
            (
                "setsid python with a child",
                format!(
                    "python3 -c 'import os, subprocess, sys, time; os.setsid(); c = subprocess.Popen([\"/bin/sleep\", \"600\"]); open(sys.argv[1], \"w\").write(f\"{{os.getpid()}}\\n{{c.pid}}\\n\"); time.sleep(600)' {} > /dev/null 2>&1 &",
                    q("py")
                ),
                vec![f("py")],
            ),
            ("( sleep & ), a double fork", format!("( /bin/sleep 600 & echo $! > {} )", q("double")), vec![f("double")]),
            ("env -i /bin/sleep &", format!("env -i /bin/sleep 600 & echo $! > {}", q("envi")), vec![f("envi")]),
            (
                "node detached server",
                format!(
                    "node -e 'const c = require(\"child_process\").spawn(\"node\", [\"-e\", \"setInterval(() => {{}}, 1e8)\"], {{ detached: true, stdio: \"ignore\" }}); require(\"fs\").writeFileSync(process.argv[1], String(c.pid)); c.unref()' {}",
                    q("node")
                ),
                vec![f("node")],
            ),
            (
                "perl setsid double fork, then exec sleep",
                format!(
                    "perl -MPOSIX -e 'exit if fork; POSIX::setsid(); exit if fork; open(my $f, \">\", $ARGV[0]); print $f \"$$\\n\"; close $f; exec \"/bin/sleep\", \"600\"' {}",
                    q("perl")
                ),
                vec![f("perl")],
            ),
            ("launchctl submit", format!("launchctl submit -l {label} -- /bin/sleep 600"), vec![]),
            ("screen -dmS", format!("screen -dmS {label} /bin/sh -c 'echo $$ > {}; exec /bin/sleep 600'", q("screen")), vec![f("screen")]),
            (
                "script with a pty child",
                format!(
                    "script -q /dev/null /bin/sh -c 'echo $$ > {}; exec /bin/sleep 600' < /dev/null > /dev/null 2>&1 & echo $! > {}",
                    q("script-child"),
                    q("script")
                ),
                vec![f("script"), f("script-child")],
            ),
            (
                "a child that set its own group",
                format!(
                    "perl -e 'setpgrp(0, 0); open(my $f, \">\", $ARGV[0]); print $f \"$$\\n\"; close $f; exec \"/bin/sleep\", \"600\"' {} &",
                    q("pgrp")
                ),
                vec![f("pgrp")],
            ),
            (
                "setsid python with its environment cleared",
                format!("env -i {python} -c 'import os, sys, time; os.setsid(); open(sys.argv[1], \"w\").write(str(os.getpid())); time.sleep(600)' {} &", q("cleared")),
                vec![f("cleared")],
            ),
        ]
    }

    #[test]
    fn every_shape_a_detached_zsh_starts_is_the_threads_once_the_shell_and_the_agent_exit_and_a_launchd_job_is_not_wsps() {
        let dir = tempfile::tempdir().unwrap();
        let folder = dir.path().canonicalize().unwrap();
        let label = format!("com.wsp.test.shapes.{}", std::process::id());
        let shapes = shapes(&folder, &label, &on_path("python3"));
        let mut agent = Command::new(on_path("python3"))
            .args(["-c", AGENT, &folder.to_string_lossy()])
            .args(shapes.iter().map(|(_, call, _)| call))
            .current_dir(&folder)
            .env("WSP_THREAD", "t_shapes")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0)
            .spawn()
            .unwrap();
        let _started = Started { folder: folder.clone(), label: label.clone(), agent: agent.id() };
        let source = LocalProcSource::new(None, road_for("macos"), numbers::PROC_CAP * 10);
        let mut s = scope("t_shapes");
        s.agents = vec![agent.id()];
        s.groups = vec![agent.id()];
        s.folder = Some(folder.to_string_lossy().into_owned());
        let scopes = [s];
        source.scopes_moved(&scopes);
        std::fs::write(folder.join("gate"), "").unwrap();
        agent.wait().unwrap();
        // Every shape wrote its pids, and launchd runs the job.
        let deadline = Instant::now() + Duration::from_secs(30);
        let pids_of = |files: &[PathBuf]| files.iter().flat_map(|f| pids_in(f)).collect::<Vec<u32>>();
        loop {
            let down: Vec<&str> = shapes
                .iter()
                .filter(
                    |(_, _, files)| {
                        if files.is_empty() {
                            launchd_job(&label).is_none()
                        } else {
                            files.iter().any(|f| pids_in(f).is_empty())
                        }
                    },
                )
                .map(|(name, _, _)| *name)
                .collect();
            if down.is_empty() {
                break;
            }
            assert!(Instant::now() < deadline, "never started in 30 s: {down:?}");
            std::thread::sleep(Duration::from_millis(50));
        }
        std::thread::sleep(Duration::from_millis(500));
        let mut scan = source
            .scan(&ProcScanInput { at: crate::sys::now_ms(), elapsed_ms: 0, pty: HashMap::new(), only: None, daemon: std::process::id() })
            .unwrap();
        Ownership::default().stamp(&mut scan.procs, &scopes, &source, std::process::id());
        let mut table = Vec::new();
        for (name, _, files) in &shapes {
            let mut pids = if files.is_empty() { launchd_job(&label).into_iter().collect() } else { pids_of(files) };
            // screen's server, above the setuid login that is root's and so never one of the person's rows.
            if *name == "screen -dmS" {
                let login = libproc::lineage(pids[0]).map(|(ppid, _)| ppid);
                pids.extend(login.and_then(libproc::lineage).map(|(ppid, _)| ppid));
            }
            for pid in pids {
                let row = scan.procs.iter().find(|p| p.pid == pid);
                table.push((*name, pid, row.map(|p| p.ppid), row.map(|p| p.owner.as_ref().map(|o| (o.by, o.thread.clone())))));
            }
        }
        let printed: Vec<String> = table.iter().map(|r| format!("{r:?}")).collect();
        let printed = printed.join("\n");
        eprintln!("{printed}");
        for (name, pid, ppid, owner) in &table {
            assert!(ppid.is_some(), "{name} pid {pid} has a row\n{printed}");
            let owner = owner.clone().flatten();
            if *name == "launchctl submit" {
                assert_eq!(owner, None, "a launchd job is not wsp's\n{printed}");
            } else {
                assert_eq!(owner.and_then(|(_, t)| t), Some("t_shapes".to_owned()), "{name}\n{printed}");
            }
        }
    }
}
