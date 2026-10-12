// SPDX-License-Identifier: AGPL-3.0-only
//! proc.kill against real processes this test starts, each in a process group of its own so nothing else is in reach.

use super::*;
use nix::unistd::Pid;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};

const SHORT: Duration = Duration::from_millis(300);

struct Started {
    child: Child,
    lines: Lines<BufReader<ChildStdout>>,
}

/// Whatever the test ends on, the script's group goes with it.
impl Drop for Started {
    fn drop(&mut self) {
        if let Some(pid) = self.child.id() {
            let _ = nix::sys::signal::killpg(Pid::from_raw(pid as i32), Signal::SIGKILL);
        }
    }
}

/// bash in a group of its own, its script's lines readable.
fn start(script: &str) -> Started {
    let mut cmd = std::process::Command::new("bash");
    cmd.args(["-c", script]).stdin(Stdio::null()).stdout(Stdio::piped()).process_group(0);
    let mut child = Command::from(cmd).kill_on_drop(true).spawn().unwrap();
    let lines = BufReader::new(child.stdout.take().unwrap()).lines();
    Started { child, lines }
}

impl Started {
    fn pid(&self) -> u32 {
        self.child.id().unwrap()
    }

    async fn line(&mut self) -> String {
        tokio::time::timeout(Duration::from_secs(10), self.lines.next_line())
            .await
            .unwrap()
            .unwrap()
            .expect("the script ended before its line")
    }

    async fn signal(&mut self) -> Option<i32> {
        tokio::time::timeout(Duration::from_secs(10), self.child.wait()).await.unwrap().unwrap().signal()
    }
}

fn running(pid: u32) -> bool {
    table().unwrap().one(pid).is_some_and(|s| !s.zombie)
}

fn ask(pid: u32, mode: KillMode) -> Ask {
    Ask { pid, signal: ProcSignal::Term, mode, started_at: None, identity: None, start_slack_ms: 0 }
}

fn sorted(mut v: Vec<u32>) -> Vec<u32> {
    v.sort_unstable();
    v
}

#[tokio::test]
async fn a_group_kill_takes_every_member_and_a_tree_kill_takes_what_left_the_group() {
    // set -m puts each background job in a group of its own, as a dev server that called setsid would be.
    let mut p = start("sleep 60 & echo $!; set -m; sleep 60 & echo $!; wait");
    let (same, own) = (p.line().await.parse::<u32>().unwrap(), p.line().await.parse::<u32>().unwrap());
    let t = table().unwrap();
    assert_ne!(t.one(own).unwrap().pgid, p.pid());

    let reply = kill_processes(t.as_ref(), ask(p.pid(), KillMode::Group), &Guard::default(), SHORT).await.unwrap();
    assert_eq!(sorted(reply.signalled), sorted(vec![p.pid(), same]));
    assert_eq!((reply.forced, reply.left), (vec![], vec![]));
    assert_eq!(p.signal().await, Some(15));
    assert!(!running(same));
    assert!(running(own), "a process in another group is no member of this one");

    let mut q = start("set -m; sleep 60 & echo $!; sleep 60 & echo $!; wait");
    let kids = [q.line().await.parse::<u32>().unwrap(), q.line().await.parse::<u32>().unwrap()];
    let reply = kill_processes(t.as_ref(), ask(q.pid(), KillMode::Tree), &Guard::default(), SHORT).await.unwrap();
    assert_eq!(sorted(reply.signalled), sorted(vec![q.pid(), kids[0], kids[1]]));
    assert_eq!(q.signal().await, Some(15));
    assert!(kids.iter().all(|k| !running(*k)));
    nix::sys::signal::kill(Pid::from_raw(own as i32), Signal::SIGKILL).unwrap();
}

#[tokio::test]
async fn an_identity_that_is_not_the_pids_own_signals_nothing() {
    let mut p = start("echo ready; sleep 60");
    p.line().await;
    let t = table().unwrap();
    let id = t.one(p.pid()).unwrap().id;
    let stale = Ask { identity: Some(id + 1), ..ask(p.pid(), KillMode::Pid) };
    let err = kill_processes(t.as_ref(), stale, &Guard::default(), SHORT).await.unwrap_err();
    assert_eq!(
        (err.code, err.message),
        (Some(DaemonErrorCode::NotFound), format!("pid {} is no longer the process that was listed", p.pid()))
    );
    assert!(running(p.pid()));
    // The identity pins the process: a start time a cached boot clock put off by a step does not refuse it.
    let pinned = Ask { identity: Some(id), started_at: Some(t.one(p.pid()).unwrap().started_at - 60_000), ..ask(p.pid(), KillMode::Pid) };
    kill_processes(t.as_ref(), pinned, &Guard::default(), SHORT).await.unwrap();
    assert_eq!(p.signal().await, Some(15));
}

#[test]
fn only_a_box_daemon_running_as_root_ends_nothing_outside_its_threads_and_a_forks_turns_stay_killable() {
    let on = |kind: &str, root: bool, v2: bool| boxed(crate::readings::readings_for(kind, "linux").unwrap(), root, v2);
    assert!(on("place", true, true));
    assert!(!on("place", false, true));
    // A box with no cgroup v2 launches its turns where they stand, so its daemon kills by identity and the agent rule.
    assert!(!on("place", true, false));
    // A cloud machine's daemon is root and its turns come through the provider's exec, in no thread cgroup.
    assert!(!on("cloud", true, true));
    assert!(!on("local", true, true));
}

#[tokio::test]
async fn term_ignored_for_the_grace_gets_kill_and_a_quick_exit_returns_before_it() {
    let mut p = start("trap '' TERM; echo ready; while :; do sleep 0.05; done");
    p.line().await;
    let at = std::time::Instant::now();
    let reply = kill_processes(table().unwrap().as_ref(), ask(p.pid(), KillMode::Pid), &Guard::default(), SHORT).await.unwrap();
    assert!(at.elapsed() >= SHORT);
    assert_eq!((reply.signalled, reply.forced, reply.left), (vec![p.pid()], vec![p.pid()], vec![]));
    assert_eq!(p.signal().await, Some(9));

    let mut q = start("echo ready; sleep 60");
    q.line().await;
    let at = std::time::Instant::now();
    let reply =
        kill_processes(table().unwrap().as_ref(), ask(q.pid(), KillMode::Pid), &Guard::default(), Duration::from_secs(5)).await.unwrap();
    assert!(at.elapsed() < Duration::from_secs(2), "a process that went on TERM is not waited the grace out");
    assert_eq!((reply.forced, reply.left), (vec![], vec![]));
    assert_eq!(q.signal().await, Some(15));
}

#[tokio::test]
async fn a_start_time_that_is_not_the_processes_signals_nothing() {
    let mut p = start("echo ready; sleep 60");
    p.line().await;
    let t = table().unwrap();
    let at = t.one(p.pid()).unwrap().started_at;
    let stale = Ask { started_at: Some(at - 60_000), ..ask(p.pid(), KillMode::Pid) };
    let err = kill_processes(t.as_ref(), stale, &Guard::default(), SHORT).await.unwrap_err();
    assert_eq!(
        (err.code, err.message),
        (Some(DaemonErrorCode::NotFound), format!("pid {} is no longer the process that was listed", p.pid()))
    );
    assert!(running(p.pid()));
    let slack = Ask { started_at: Some(at - 1_500), start_slack_ms: 2_000, ..ask(p.pid(), KillMode::Pid) };
    assert!(kill_processes(t.as_ref(), Ask { start_slack_ms: 0, ..slack }, &Guard::default(), SHORT).await.is_err());

    let fresh = if cfg!(target_os = "linux") { slack } else { Ask { started_at: Some(at), ..ask(p.pid(), KillMode::Pid) } };
    kill_processes(t.as_ref(), fresh, &Guard::default(), SHORT).await.unwrap();
    assert_eq!(p.signal().await, Some(15));
}

#[tokio::test]
async fn init_the_daemon_its_parent_and_the_hosts_pids_are_refused_in_every_mode() {
    let mut p = start("sleep 60 & echo $!; wait");
    let kid = p.line().await.parse::<u32>().unwrap();
    let t = table().unwrap();
    let guard = Guard { this: 5000, parent: 4000, protect: HashSet::from([kid]), ..Guard::default() };
    for (pid, mode) in [(kid, KillMode::Pid), (p.pid(), KillMode::Group), (p.pid(), KillMode::Tree)] {
        let err = kill_processes(t.as_ref(), ask(pid, mode), &guard, SHORT).await.unwrap_err();
        assert_eq!(err.code, Some(DaemonErrorCode::Forbidden), "{mode:?}");
        assert!(err.message.ends_with("it is part of wsp or the system"), "{}", err.message);
    }
    assert!(running(p.pid()) && running(kid));
    let me = Guard { this: p.pid(), ..Guard::default() };
    assert_eq!(
        kill_processes(t.as_ref(), ask(p.pid(), KillMode::Pid), &me, SHORT).await.unwrap_err().code,
        Some(DaemonErrorCode::Forbidden)
    );
    let parent = Guard { parent: p.pid(), ..Guard::default() };
    assert_eq!(
        kill_processes(t.as_ref(), ask(p.pid(), KillMode::Tree), &parent, SHORT).await.unwrap_err().code,
        Some(DaemonErrorCode::Forbidden)
    );
    if t.one(1).is_some() {
        assert_eq!(
            kill_processes(t.as_ref(), ask(1, KillMode::Pid), &Guard::default(), SHORT).await.unwrap_err().code,
            Some(DaemonErrorCode::Forbidden)
        );
    }
    assert!(running(p.pid()) && running(kid));
}

#[tokio::test]
async fn a_tree_holding_a_turns_root_is_refused_with_the_root_named_while_what_stands_outside_it_is_not() {
    // The root leads, as the host launches a turn; a dev server it started with set -m is its child all the same.
    let mut root = start("sleep 60 & echo $!; set -m; sleep 60 & echo $!; wait");
    let (tool, server) = (root.line().await.parse::<u32>().unwrap(), root.line().await.parse::<u32>().unwrap());
    let t = table().unwrap();
    let guard = Guard { agents: HashSet::from([root.pid()]), ..Guard::default() };
    for mode in [KillMode::Pid, KillMode::Tree] {
        let err = kill_processes(t.as_ref(), ask(root.pid(), mode), &guard, SHORT).await.unwrap_err();
        assert_eq!(err.code, Some(DaemonErrorCode::Forbidden));
        assert!(err.message.ends_with("it is a thread's agent; stop the thread instead"), "{}", err.message);
    }
    let err = kill_processes(t.as_ref(), ask(tool, KillMode::Group), &guard, SHORT).await.unwrap_err();
    assert!(err.message.contains(&format!("pid {}, which its process group takes with pid {tool}: it", root.pid())), "{}", err.message);
    assert!(running(root.pid()) && running(tool));

    let reply = kill_processes(t.as_ref(), ask(server, KillMode::Tree), &guard, SHORT).await.unwrap();
    assert_eq!(reply.signalled, vec![server]);
    assert!(!running(server) && running(root.pid()));
}

#[tokio::test]
async fn a_pid_that_is_gone_answers_not_found() {
    // 2^22 - 1 is above every Linux pid_max default and no macOS pid.
    let err = kill_processes(table().unwrap().as_ref(), ask(4_194_303, KillMode::Tree), &Guard::default(), SHORT).await.unwrap_err();
    assert_eq!((err.code, err.message.as_str()), (Some(DaemonErrorCode::NotFound), "no process 4194303"));
}

#[tokio::test]
async fn everything_under_the_apps_main_process_is_refused_short_of_what_stands_under_the_host() {
    // The app leads; one helper hangs off it, and a host it started runs a dev server of its own, as a dev run does.
    let mut app = start("sleep 60 & echo helper $!; (sleep 60 & echo dev $!; wait) & echo host $!; wait");
    let mut said = std::collections::HashMap::new();
    for _ in 0..3 {
        let line = app.line().await;
        let (what, pid) = line.split_once(' ').unwrap();
        said.insert(what.to_owned(), pid.parse::<u32>().unwrap());
    }
    let t = table().unwrap();
    let guard = Guard { parent: said["host"], apps: HashSet::from([app.pid()]), ..Guard::default() };
    for (pid, mode) in [(said["helper"], KillMode::Pid), (app.pid(), KillMode::Pid), (said["helper"], KillMode::Group)] {
        let err = kill_processes(t.as_ref(), ask(pid, mode), &guard, SHORT).await.unwrap_err();
        assert_eq!(err.code, Some(DaemonErrorCode::Forbidden), "{pid} {mode:?}");
        assert!(err.message.ends_with("it is part of wsp or the system"), "{}", err.message);
    }
    assert!(running(said["helper"]));
    let reply = kill_processes(t.as_ref(), ask(said["dev"], KillMode::Pid), &guard, SHORT).await.unwrap();
    assert_eq!(reply.signalled, vec![said["dev"]]);
    assert!(!running(said["dev"]) && running(app.pid()));
}

#[tokio::test]
async fn a_target_read_as_another_process_is_never_signalled_and_one_that_execd_still_is() {
    let mut p = start("echo ready; sleep 0.3; exec sleep 60");
    p.line().await;
    let t = table().unwrap();
    let read = t.one(p.pid()).unwrap();
    // What a reused pid looks like from the kill's side: the same number, another process's identity.
    let other = Seen { id: read.id + 1, version: read.version.wrapping_add(1), ..read.clone() };
    assert_eq!(t.signal(&other, Signal::SIGTERM), Err(Errno::ESRCH));
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert!(running(p.pid()));
    let now = t.one(p.pid()).unwrap();
    assert_eq!(now.comm, "sleep");
    if cfg!(target_os = "macos") {
        assert_ne!(now.version, read.version, "exec moves a Mac's pid version");
    }
    assert_eq!(t.signal(&read, Signal::SIGTERM), Ok(()), "exec keeps the process the kill read");
    assert_eq!(p.signal().await, Some(15));
}

#[tokio::test]
async fn every_process_from_a_turns_root_down_to_its_agent_is_refused_and_a_tool_under_the_agent_is_not() {
    // The turn's root runs a shell, the shell runs the agent (xargs here, a binary that waits on what it starts on
    // both systems), and the agent runs a tool server, as bash <run>.sh, its subshell, claude and an MCP server do.
    let mut root = start("( echo 60 | xargs sleep & echo agent $!; wait ) & echo shell $!; wait");
    let said = named_pids(&mut root, 2).await;
    let t = table().unwrap();
    let tool = sleep_under(t.as_ref(), said["agent"]).await;
    let guard = Guard { agents: HashSet::from([root.pid()]), agent_bins: HashSet::from(["xargs".to_owned()]), ..Guard::default() };
    for (pid, mode) in
        [(said["agent"], KillMode::Pid), (said["shell"], KillMode::Pid), (said["shell"], KillMode::Tree), (root.pid(), KillMode::Pid)]
    {
        let err = kill_processes(t.as_ref(), ask(pid, mode), &guard, SHORT).await.unwrap_err();
        assert_eq!(err.code, Some(DaemonErrorCode::Forbidden), "{pid} {mode:?}");
        assert!(err.message.ends_with("it is a thread's agent; stop the thread instead"), "{}", err.message);
    }
    assert!(running(said["agent"]) && running(said["shell"]));
    let reply = kill_processes(t.as_ref(), ask(tool, KillMode::Tree), &guard, SHORT).await.unwrap();
    // xargs goes once its child does, which is the agent's own doing and not the kill's.
    assert_eq!(reply.signalled, vec![tool]);
    assert!(!running(tool));
}

/// The names the host sends, the catalog's agents'.
fn agent_bins() -> HashSet<String> {
    ["claude", "codex", "opencode", "cursor-agent"].map(str::to_owned).into()
}

/// The script's `name pid` lines, as many as asked for.
async fn named_pids(p: &mut Started, n: usize) -> std::collections::HashMap<String, u32> {
    let mut said = std::collections::HashMap::new();
    for _ in 0..n {
        let line = p.line().await;
        let (what, pid) = line.split_once(' ').unwrap();
        said.insert(what.to_owned(), pid.parse::<u32>().unwrap());
    }
    said
}

#[tokio::test]
async fn an_agent_known_by_the_pid_its_run_recorded_is_refused_with_every_process_above_it_whatever_its_name() {
    // xargs stands in for an agent no name rule knows; the host lists it beside the root, as its run recorded it.
    let mut root = start("( echo 60 | xargs sleep & echo agent $!; wait ) & echo shell $!; wait");
    let said = named_pids(&mut root, 2).await;
    let t = table().unwrap();
    // Its tool, sleep, runs as no agent's name, so nothing under the recorded agent is the agent's binary.
    let guard = Guard { agents: HashSet::from([root.pid(), said["agent"]]), agent_bins: agent_bins(), ..Guard::default() };
    for (pid, mode) in
        [(said["agent"], KillMode::Pid), (said["agent"], KillMode::Tree), (said["shell"], KillMode::Pid), (said["shell"], KillMode::Tree)]
    {
        let err = kill_processes(t.as_ref(), ask(pid, mode), &guard, SHORT).await.unwrap_err();
        assert_eq!(err.code, Some(DaemonErrorCode::Forbidden), "{pid} {mode:?}");
        assert!(err.message.ends_with("it is a thread's agent; stop the thread instead"), "{}", err.message);
    }
    assert!(running(said["agent"]) && running(said["shell"]));
    let tool = sleep_under(t.as_ref(), said["agent"]).await;
    let reply = kill_processes(t.as_ref(), ask(tool, KillMode::Pid), &guard, SHORT).await.unwrap();
    assert_eq!(reply.signalled, vec![tool]);
}

/// The sleep a process starts, once its exec is done and it runs as sleep.
async fn sleep_under(t: &dyn Table, pid: u32) -> u32 {
    for _ in 0..100 {
        if let Some(kid) = t.all().into_iter().find(|s| s.ppid == pid && s.comm == "sleep") {
            return kid.pid;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    panic!("{pid} started nothing");
}

#[tokio::test]
async fn an_agent_run_through_a_link_or_under_another_argv0_is_known_by_argv0s_basename() {
    // A native Claude Code runs through ~/.local/bin/claude, a link to versions/<version>, so a Mac names it by the
    // version; cursor-agent's script ends in exec -a "$0" node, so it is named node on both systems. Here a link named
    // claude to sleep, which a Mac names sleep and Linux by the link, and sleep run under argv[0] cursor-agent.
    let dir = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(std::fs::canonicalize("/bin/sleep").unwrap(), dir.path().join("claude")).unwrap();
    let link = dir.path().join("claude");
    let mut root = start(&format!(
        "( '{}' 60 & echo claude $!; ( exec -a cursor-agent sleep 60 ) & echo cursor $!; wait ) & echo shell $!; wait",
        link.display()
    ));
    let said = named_pids(&mut root, 3).await;
    let t = table().unwrap();
    let named = |pid: u32| t.one(pid).map(|s| s.comm).filter(|comm| comm != "bash");
    for agent in ["claude", "cursor"] {
        for _ in 0..100 {
            if named(said[agent]).is_some() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }
    assert_eq!(named(said["cursor"]).as_deref(), Some("sleep"));
    if cfg!(target_os = "macos") {
        assert_eq!(named(said["claude"]).as_deref(), Some("sleep"), "a Mac names a process by the file its link resolves to");
    }
    let guard = Guard { agents: HashSet::from([root.pid()]), agent_bins: agent_bins(), ..Guard::default() };
    for (pid, mode) in [(said["claude"], KillMode::Pid), (said["cursor"], KillMode::Tree), (said["shell"], KillMode::Pid)] {
        let err = kill_processes(t.as_ref(), ask(pid, mode), &guard, SHORT).await.unwrap_err();
        assert_eq!(err.code, Some(DaemonErrorCode::Forbidden), "{pid} {mode:?}");
        assert!(err.message.ends_with("it is a thread's agent; stop the thread instead"), "{}", err.message);
    }
    assert!(running(said["claude"]) && running(said["cursor"]));
}

/// A process table written by hand, for the rules that refuse before any signal; a signal is only counted.
struct Fake {
    rows: Vec<Seen>,
    cgroups: std::collections::HashMap<u32, String>,
    sent: std::sync::Mutex<Vec<u32>>,
}

impl Table for Fake {
    fn one(&self, pid: u32) -> Option<Seen> {
        self.rows.iter().find(|s| s.pid == pid && !self.sent.lock().unwrap().contains(&pid)).cloned()
    }
    fn all(&self) -> Vec<Seen> {
        self.rows.iter().filter_map(|s| self.one(s.pid)).collect()
    }
    fn signal(&self, seen: &Seen, _signal: Signal) -> nix::Result<()> {
        self.sent.lock().unwrap().push(seen.pid);
        Ok(())
    }
    fn arg0(&self, _pid: u32) -> Option<String> {
        None
    }
    fn cgroup(&self, pid: u32) -> Option<String> {
        self.cgroups.get(&pid).cloned()
    }
}

fn row(pid: u32, ppid: u32) -> Seen {
    named(pid, ppid, "p")
}

fn named(pid: u32, ppid: u32, comm: &str) -> Seen {
    Seen { pid, ppid, pgid: pid, started_at: 0, zombie: false, comm: comm.to_owned(), id: u64::from(pid), version: 0 }
}

#[tokio::test]
async fn an_agent_recorded_as_its_launcher_keeps_the_binary_it_starts_and_a_recorded_agent_binary_ends_the_chain() {
    // npm's codex: the run (800, 801) execs node codex.js (802, recorded), which starts the native codex (803) as a
    // child; codex's tool (804) is killable. A native claude (902, recorded) runs as its name: a claude it ran (903) is
    // the agent's own child process and killable, as any tool is.
    let fake = Fake {
        rows: vec![
            row(1, 0),
            named(800, 1, "bash"),
            named(801, 800, "bash"),
            named(802, 801, "node"),
            named(803, 802, "codex"),
            named(804, 803, "node"),
            named(900, 1, "bash"),
            named(901, 900, "bash"),
            named(902, 901, "claude"),
            named(903, 902, "claude"),
        ],
        cgroups: Default::default(),
        sent: Default::default(),
    };
    let guard = Guard { agents: HashSet::from([800, 802, 900, 902]), agent_bins: agent_bins(), ..Guard::default() };
    for (pid, mode) in [(803, KillMode::Pid), (803, KillMode::Tree), (802, KillMode::Pid), (801, KillMode::Tree), (902, KillMode::Pid)] {
        let err = kill_processes(&fake, ask(pid, mode), &guard, SHORT).await.unwrap_err();
        assert!(err.message.ends_with("it is a thread's agent; stop the thread instead"), "{pid} {mode:?}: {}", err.message);
    }
    assert!(fake.sent.lock().unwrap().is_empty());
    for pid in [804, 903] {
        assert_eq!(kill_processes(&fake, ask(pid, KillMode::Tree), &guard, SHORT).await.unwrap().signalled, vec![pid]);
    }
}

#[test]
fn a_run_folder_names_the_root_and_agent_of_each_run_still_held_and_nothing_a_link_or_an_ended_run_holds() {
    let dir = tempfile::tempdir().unwrap();
    let at = |name: &str| dir.path().join(name);
    let write = |name: &str, text: &str| std::fs::write(at(name), text).unwrap();
    // Held: its claim folder, no exit code.
    std::fs::create_dir(at("aaaaaaaaaaaa.d")).unwrap();
    write("aaaaaaaaaaaa.pid", "4100\n");
    write("aaaaaaaaaaaa.agent", "4103\n");
    // Ended: its exit code is written.
    std::fs::create_dir(at("bbbbbbbbbbbb.d")).unwrap();
    write("bbbbbbbbbbbb.pid", "4200\n");
    write("bbbbbbbbbbbb.agent", "4203\n");
    write("bbbbbbbbbbbb.exit", "0\n");
    // Gone: no claim folder.
    write("cccccccccccc.pid", "4300\n");
    write("cccccccccccc.agent", "4303\n");
    // A link in place of the agent's file, planted by what runs as the login.
    std::fs::create_dir(at("dddddddddddd.d")).unwrap();
    write("dddddddddddd.pid", "4400\n");
    write("elsewhere", "4403\n");
    std::os::unix::fs::symlink(at("elsewhere"), at("dddddddddddd.agent")).unwrap();
    let mut pids = recorded_runs(dir.path());
    pids.sort_unstable();
    assert_eq!(pids, vec![4100, 4103, 4400]);
    assert!(recorded_runs(&at("none")).is_empty());
}

#[tokio::test]
async fn a_box_turns_run_script_and_shell_never_take_its_agent_while_what_the_agent_started_is_killable() {
    // A box turn: bash run.sh (700) and its subshell (701) in the thread's cgroup run the agent (702), whose tool
    // server (703) and dev server (704) stand there too. No host lists a box turn's pids; the host sends the names.
    let thread = "/wsp-threads/t1".to_owned();
    let fake = Fake {
        rows: vec![
            row(1, 0),
            row(500, 1),
            named(700, 1, "bash"),
            named(701, 700, "bash"),
            named(702, 701, "claude"),
            named(703, 702, "node"),
            named(704, 702, "vite"),
        ],
        cgroups: [700, 701, 702, 703, 704].map(|pid| (pid, thread.clone())).into(),
        sent: Default::default(),
    };
    let guard = Guard { this: 500, boxed: true, agent_bins: agent_bins(), ..Guard::default() };
    for (pid, mode) in [(700, KillMode::Tree), (700, KillMode::Pid), (701, KillMode::Pid), (702, KillMode::Pid), (702, KillMode::Tree)] {
        let err = kill_processes(&fake, ask(pid, mode), &guard, SHORT).await.unwrap_err();
        assert!(err.message.ends_with("it is a thread's agent; stop the thread instead"), "{pid} {mode:?}: {}", err.message);
    }
    assert!(fake.sent.lock().unwrap().is_empty());
    for pid in [703, 704] {
        assert_eq!(kill_processes(&fake, ask(pid, KillMode::Tree), &guard, SHORT).await.unwrap().signalled, vec![pid]);
    }
}

#[tokio::test]
async fn a_root_daemon_on_a_box_ends_only_what_stands_in_a_thread_or_under_it_and_never_a_kernel_thread() {
    // The daemon is 500; it ran a slate script, 501, which ran 502. A thread's dev server, 600, left its turn for init but stays in
    // the thread's cgroup. sshd, 400, is the box's own; 3 is a kernel thread.
    let fake = Fake {
        rows: vec![row(1, 0), row(2, 0), row(3, 2), row(400, 1), row(500, 1), row(501, 500), row(502, 501), row(600, 1)],
        cgroups: [(400, "/system.slice/ssh.service".to_owned()), (600, "/wsp-threads/t1".to_owned())].into(),
        sent: Default::default(),
    };
    let guard = Guard { this: 500, boxed: true, ..Guard::default() };
    let err = kill_processes(&fake, ask(400, KillMode::Pid), &guard, SHORT).await.unwrap_err();
    assert_eq!(err.code, Some(DaemonErrorCode::Forbidden));
    assert!(err.message.ends_with("it is outside every thread wsp runs on this computer"), "{}", err.message);
    if cfg!(target_os = "linux") {
        for pid in [2, 3] {
            let err = kill_processes(&fake, ask(pid, KillMode::Pid), &guard, SHORT).await.unwrap_err();
            assert!(err.message.ends_with("it is part of wsp or the system"), "{pid}: {}", err.message);
        }
    }
    assert!(fake.sent.lock().unwrap().is_empty());
    for pid in [502, 501, 600] {
        assert_eq!(kill_processes(&fake, ask(pid, KillMode::Pid), &guard, SHORT).await.unwrap().signalled, vec![pid]);
    }
    // The same daemon off a box, or not root, leaves the rule to the kernel.
    let unboxed = Guard { this: 500, ..Guard::default() };
    assert_eq!(kill_processes(&fake, ask(400, KillMode::Pid), &unboxed, SHORT).await.unwrap().signalled, vec![400]);
}

/// Forks until the kernel hands a killed process's pid to a new one, then signals by the old process's identity.
#[cfg(target_os = "macos")]
#[tokio::test]
#[ignore = "forks up to 100,000 times, about a minute; run with --ignored on a Mac"]
async fn a_pid_the_kernel_handed_to_a_new_process_is_refused_by_the_old_identity() {
    let mut p = start("echo ready; sleep 60");
    p.line().await;
    let t = table().unwrap();
    let old = t.one(p.pid()).unwrap();
    nix::sys::signal::kill(Pid::from_raw(old.pid as i32), Signal::SIGKILL).unwrap();
    assert_eq!(p.signal().await, Some(9));
    let target = old.pid as libc::pid_t;
    let at = std::time::Instant::now();
    let mut forks = 0u64;
    let holder = loop {
        forks += 1;
        assert!(forks < 400_000, "pid {target} never came back");
        match unsafe { libc::fork() } {
            0 => unsafe {
                // Only the child that got the number stays, until the test signals it.
                if libc::getpid() == target {
                    libc::pause();
                }
                libc::_exit(0)
            },
            -1 => panic!("fork: {}", Errno::last()),
            child if child == target => break child as u32,
            child => {
                unsafe { libc::waitpid(child, std::ptr::null_mut(), 0) };
            }
        }
    };
    let new = t.one(holder).unwrap();
    eprintln!(
        "pid {holder} reused after {forks} forks in {:.1} s; old unique id {} version {}, new unique id {} version {}",
        at.elapsed().as_secs_f64(),
        old.id,
        old.version,
        new.id,
        new.version
    );
    assert_ne!(new.id, old.id);
    let refused = t.signal(&old, Signal::SIGKILL);
    std::thread::sleep(Duration::from_millis(100));
    let alive = t.one(holder).is_some_and(|s| s.same(&new));
    t.signal(&new, Signal::SIGKILL).unwrap();
    unsafe { libc::waitpid(holder as libc::pid_t, std::ptr::null_mut(), 0) };
    assert_eq!(refused, Err(Errno::ESRCH));
    assert!(alive, "the new holder of the pid outlived a signal to the old identity");
}
