// SPDX-License-Identifier: AGPL-3.0-only
//! local_exec_reap: the tree's reading, the agent pid and a stranger's group, ported from
//! packages/runtime/test/local-exec.test.ts. Spec: spec/local_exec_reap.md.
#![allow(dead_code)]
mod support;

use std::io::Write as _;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::Arc;

use wsp_adapters::port::{agent_word, ExecOptions as Launch, ExecStreamFactory};
use wsp_core::local_exec::{agent_pid_of, group_exists, local_exec_stream, tree_work, LocalExecFactory, LocalExecOptions, TreeWorkReader};

use support::strays::{alive, gone, grandchild};
use support::{sq, wait_for};

/// One case's temp root, with the run folder under it; the guard ends what the case recorded and removes the root.
struct Case {
    _dir: tempfile::TempDir,
    root: String,
    run_dir: String,
}

impl Case {
    fn new(prefix: &str) -> Self {
        let dir = tempfile::Builder::new().prefix(prefix).tempdir().expect("a temp root");
        let root = dir.path().to_str().expect("a utf-8 path").to_string();
        let run_dir = format!("{root}/runs");
        Case { _dir: dir, root, run_dir }
    }

    fn opts(&self) -> LocalExecOptions {
        LocalExecOptions::new(self.root.clone(), self.run_dir.clone())
    }

    fn factory(&self) -> Arc<LocalExecFactory> {
        local_exec_stream(self.opts(), None)
    }

    fn path(&self, name: &str) -> String {
        format!("{}/{name}", self.root)
    }

    /// The TS `afterEach`'s sweep of the run folder with a fresh factory.
    async fn sweep(&self) {
        self.factory().sweep(&[]).expect("a sweep").await.expect("a sweep that answers");
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        support::strays::sweep();
    }
}

/// Starts a process as the leader of a group of its own, with nothing of this process's attached, and reaps it as
/// node does, so a killed one is gone and not a zombie.
fn spawn_group(cmd: &mut Command) -> i32 {
    let child = cmd.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).process_group(0).spawn().expect("a spawn");
    let pid = child.id() as i32;
    std::thread::spawn(move || {
        let mut child = child;
        let _ = child.wait();
    });
    pid
}

fn kill_group_hard(pid: i32) {
    assert!(pid > 1);
    // SAFETY: a negative pid names a group this test started.
    unsafe {
        libc::kill(-pid, libc::SIGKILL);
    }
}

fn write_stub(path: &str, text: &str) {
    let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o755).open(path).expect("a stub");
    f.write_all(text.as_bytes()).expect("a stub's text");
}

/// Sets a path's modification time to `secs_ago` seconds before now.
fn set_mtime_ago(path: &str, secs_ago: i64) {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).expect("the clock").as_secs() as i64;
    let at = libc::timespec { tv_sec: (now - secs_ago) as libc::time_t, tv_nsec: 0 };
    let c = std::ffi::CString::new(path).expect("a path without nul");
    // SAFETY: `c` is a valid C string and `times` holds two timespecs.
    let rc = unsafe { libc::utimensat(libc::AT_FDCWD, c.as_ptr(), [at, at].as_ptr(), 0) };
    assert_eq!(rc, 0, "utimensat {path}");
}

fn owner_dir(path: &str) {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(path).expect(path);
}

/// A perl program that burns 1.2 s of CPU, since Linux's ps counts whole seconds, writes its pid to `burned`, and
/// holds what it burned until `release` exists, so the test reads it while it runs and again once it is gone.
const BURNER: &str = r#"my ($burned, $release) = @ARGV;
my $s = (times)[0];
while (((times)[0] - $s) < 1.2) {}
open(my $f, ">", $burned) or die "open $burned: $!";
print $f $$;
close($f);
select(undef, undef, undef, 0.02) while !-e $release;
exit 0;
"#;

/// The command that starts the burner program, written under the case's root.
fn burner(c: &Case, burned: &str, release: &str) -> String {
    let file = c.path("burner.pl");
    if !std::path::Path::new(&file).exists() {
        std::fs::write(&file, BURNER).expect("the burner");
    }
    format!("perl {} {} {}", sq(&file), sq(burned), sq(release))
}

/// A tree started as the leader of a group of its own, as a turn's launch does, and the reader of its work.
struct Tree {
    pgid: i32,
    read: TreeWorkReader,
    done: bool,
}

impl Tree {
    fn start(script: &str) -> Self {
        let pgid = spawn_group(Command::new("/bin/sh").args(["-c", script]));
        Tree { pgid, read: tree_work(), done: false }
    }

    async fn reading(&self) -> Option<i64> {
        (self.read)(self.pgid).await
    }

    /// The TS `finally`: the group is killed and must be gone.
    async fn finish(mut self) {
        kill_group_hard(self.pgid);
        self.done = true;
        let pgid = self.pgid;
        wait_for("the group to go", 5_000, move || !group_exists(pgid)).await;
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        if !self.done {
            kill_group_hard(self.pgid);
        }
    }
}

/// The pid the burner wrote, once it has, recorded for the sweep.
async fn burned(file: &str) -> i32 {
    wait_for("the burner's pid", 15_000, || !support::read(file).trim().is_empty()).await;
    grandchild(file).await
}

mod local_exec_stream {
    use super::*;

    own_group_test!(
        the_agent_a_launch_starts_through_agent_word_writes_its_own_pid_the_process_it_r,
        15_000,
        {
            let c = Case::new("wsp-localexec-");
            let factory = c.factory();
            // A link named claude to sleep, as a native Claude Code's link to its version file.
            std::fs::create_dir(c.path("bin")).unwrap();
            std::os::unix::fs::symlink("/bin/sleep", c.path("bin/claude")).unwrap();
            let stream = factory.launch(&format!("echo before; PATH={}:/usr/bin:/bin {} 30", sq(&c.path("bin")), agent_word("claude", None)), Launch::default());
            let run = stream.run().expect("a run");
            wait_for("the agent's pid", 5_000, || agent_pid_of(&run).is_some()).await;
            let pid = agent_pid_of(&run).unwrap();
            let out = Command::new("ps").args(["-o", "pgid=,ucomm=,command=", "-p", &pid.to_string()]).output().expect("ps");
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let mut words = text.split_whitespace();
            let pgid = words.next().expect("a pgid").parse::<u32>().expect("a number");
            let ucomm = words.next().expect("a name").to_string();
            let command = words.collect::<Vec<_>>().join(" ");
            // A Mac names a process by the file its link resolves to, Linux by the name exec was handed.
            let named = if cfg!(target_os = "macos") { "sleep" } else { "claude" };
            assert_eq!((Some(pgid), ucomm.as_str(), command.as_str()), (stream.pid(), named, "claude 30"));
            stream.kill();
            stream.exited().await;
            assert_eq!(agent_pid_of(&run), None);
            c.sweep().await;
        }
    );

    own_group_test!(
        the_pid_agent_word_writes_is_the_agent_s_own_and_the_agent_never_sees_the_variab,
        15_000,
        {
            let c = Case::new("wsp-localexec-");
            let factory = c.factory();
            let stream = factory.launch(&format!("{} -c {}", agent_word("sh", None), sq(r#"echo "[${WSP_AGENT_PID_FILE:-}] $$"; sleep 30"#)), Launch::default());
            let run = stream.run().expect("a run");
            let log = format!("{run}.log");
            wait_for("the agent's line", 5_000, || !support::read(&log).trim().is_empty()).await;
            let line = support::read(&log).trim().to_string();
            assert_eq!(line, format!("[] {}", agent_pid_of(&run).expect("the agent's pid")));
            stream.kill();
            stream.exited().await;
            c.sweep().await;
        }
    );

    own_group_test!(the_tree_s_reading_keeps_the_work_of_a_child_its_parent_has_reaped, 30_000, {
        let c = Case::new("wsp-localexec-");
        let (done, release) = (c.path("burned"), c.path("release"));
        let tree = Tree::start(&format!("{}; sleep 30", burner(&c, &done, &release)));
        let child = burned(&done).await;
        let burning = tree.reading().await.expect("a reading");
        assert!(burning >= 100, "the burner's second of CPU reads as {burning} ticks");
        std::fs::write(&release, "").unwrap();
        gone(child).await;
        assert!(tree.reading().await.expect("a reading") >= burning);
        tree.finish().await;
        c.sweep().await;
    });

    own_group_test!(the_tree_s_reading_counts_a_child_in_a_session_of_its_own_as_an_agent_s_tool_cal, 30_000, {
        let c = Case::new("wsp-localexec-");
        let (done, release) = (c.path("burned"), c.path("release"));
        // A forked child calls setsid, which is how Claude Code starts every Bash call: the burner leaves the leader's group.
        let spawner = r#"use POSIX qw(setsid); if (fork == 0) { setsid; exec "perl", @ARGV or die } sleep 1 while 1;"#;
        let burner_file = c.path("burner.pl");
        burner(&c, &done, &release);
        let tree = Tree::start(&format!("exec perl -e {} {} {} {}", sq(spawner), sq(&burner_file), sq(&done), sq(&release)));
        let child = burned(&done).await;
        assert_eq!(support::strays::group_of(child), Some(child));
        assert!(tree.reading().await.expect("a reading") >= 100);
        tree.finish().await;
        c.sweep().await;
    });

    own_group_test!(the_tree_s_reading_counts_an_orphan_the_leader_left_in_its_group, 30_000, {
        let c = Case::new("wsp-localexec-");
        let (done, release) = (c.path("burned"), c.path("release"));
        let tree = Tree::start(&format!("{} & exit 0", burner(&c, &done, &release)));
        burned(&done).await;
        assert!(tree.reading().await.expect("a reading") >= 100);
        tree.finish().await;
        c.sweep().await;
    });

    own_group_test!(the_tree_s_reading_answers_when_ps_names_two_processes_as_each_other_s_parent, 10_000, {
        // The test body runs in the child process this macro started: PATH is changed before anything else here
        // runs, since the runtime's threads read the environment and set_var is not safe beside a reader.
        let c = Case::new("wsp-localexec-");
        let bin = c.path("bin");
        std::fs::create_dir(&bin).unwrap();
        // procps reads /proc pid by pid, so a pid reused mid-read can print a parent loop; the leader's tree is 999 and 1000.
        write_stub(&format!("{bin}/ps"), "#!/bin/sh\nprintf '999 1 999 0:00.50\\n1000 999 999 0:01.00\\n500 600 500 0:01.00\\n600 500 600 0:01.00\\n'\n");
        std::env::set_var("PATH", format!("{bin}:{}", std::env::var("PATH").unwrap_or_default()));
        let read = tree_work();
        assert_eq!(read(999).await, Some(150));
    });
}

mod what_a_run_leaves_on_this_computer {
    use super::*;

    own_group_test!(a_run_whose_recorded_pid_leads_somebody_else_s_group_is_cleaned_up_and_never_sig, 20_000, {
        let c = Case::new("wsp-localexec-files-");
        // A process group of this test's own, standing for whatever this computer handed that pid to after the run
        // that recorded it was gone. Its pid is one this test started and wrote down, never one found by name.
        let stranger = spawn_group(Command::new("bash").args(["-c", "sleep 300"]));
        assert!(stranger > 0);
        support::strays::record(stranger);
        // A claim from long before that process started, with its pid written into the run's pid file.
        owner_dir(&c.run_dir);
        let base = format!("{}/aaaaaaaaaaaa", c.run_dir);
        owner_dir(&format!("{base}.d"));
        std::fs::write(format!("{base}.pid"), format!("{stranger}\n")).unwrap();
        set_mtime_ago(&format!("{base}.d"), 600);

        let swept = c.factory().sweep(&[]).expect("a sweep").await.expect("a sweep that answers");

        assert_eq!(swept, vec![base.clone()]);
        // The files are gone, so nothing attaches to it again; the process group is not.
        assert_eq!(support::names(&c.run_dir), Vec::<String>::new());
        assert!(alive(stranger));
        kill_group_hard(stranger);
        wait_for("the stranger to go", 5_000, || !alive(stranger)).await;
    });
}

// pinned cases follow
