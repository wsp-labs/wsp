// SPDX-License-Identifier: AGPL-3.0-only
//! local_exec_no_ps: a turn's stream where the group read cannot run, ported from
//! packages/runtime/test/local-exec-no-ps.test.ts. Spec: spec/local_exec_reap.md (decision 1).
//!
//! The TS mocks `execFile` to throw for every `ps`. Here each case runs in its own process with a `ps` first on PATH
//! that appends a line to a counter file when its arguments hold `-eo` (the tree's reading, the one the idle clock
//! pays for) and exits 1, which a run reads as no reading. `spawns.trees` is the counter file's line count.
#![allow(dead_code)]
mod support;

use std::io::Write as _;
use std::os::unix::fs::OpenOptionsExt;
use std::sync::Arc;

use wsp_adapters::port::{ExecOptions as Launch, ExecStream, ExecStreamFactory};
use wsp_core::local_exec::{local_exec_stream, LocalExecFactory, LocalExecOptions};

use support::strays::{gone, grandchild};

struct Case {
    _dir: tempfile::TempDir,
    root: String,
    run_dir: String,
    counter: String,
}

impl Case {
    /// The root, and a `ps` ahead of every other on PATH. This is called first in each case's body, which runs in the
    /// child process the macro started: the environment is changed before any work of the runtime under test starts,
    /// since the runtime's threads read it and set_var is not safe beside a reader.
    fn new() -> Self {
        let dir = tempfile::Builder::new().prefix("wsp-localexec-nops-").tempdir().expect("a temp root");
        let root = dir.path().to_str().expect("a utf-8 path").to_string();
        let run_dir = format!("{root}/runs");
        let bin = format!("{root}/bin");
        let counter = format!("{root}/trees");
        std::fs::create_dir(&bin).unwrap();
        // Every ps fails, so both readings this module makes meet the failure; only the tree's is counted, since that is
        // the one the idle clock pays for every second. The reap's one read of a leader's age at the end of a run is not
        // a reading of the tree and says nothing about the clock.
        let script =
            format!("#!/bin/sh\nfor a in \"$@\"; do\n  if [ \"$a\" = \"-eo\" ]; then echo tree >> '{counter}'; fi\ndone\nexit 1\n");
        let mut f = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o755).open(format!("{bin}/ps")).expect("a stub ps");
        f.write_all(script.as_bytes()).expect("a stub ps's text");
        drop(f);
        std::env::set_var("PATH", format!("{bin}:{}", std::env::var("PATH").unwrap_or_default()));
        Case { _dir: dir, root, run_dir, counter }
    }

    fn factory(&self, idle_ms: f64, deadline_ms: f64) -> Arc<LocalExecFactory> {
        local_exec_stream(
            LocalExecOptions {
                idle_ms: Some(idle_ms),
                deadline_ms: Some(deadline_ms),
                poll_ms: Some(20),
                ..LocalExecOptions::new(self.root.clone(), self.run_dir.clone())
            },
            None,
        )
    }

    /// What `spawns.trees` counted.
    fn trees(&self) -> usize {
        support::read(&self.counter).lines().count()
    }
}

impl Drop for Case {
    fn drop(&mut self) {
        support::strays::sweep();
    }
}

fn is_idle_cut(message: &str) -> bool {
    regex::Regex::new(r"^stopped after \d+m \d\ds with no output for 0m$").unwrap().is_match(message)
}

mod local_exec_stream_where_the_group_read_cannot_spawn {
    use super::*;

    own_group_test!(a_read_that_throws_where_node_never_calls_back_leaves_this_process_up_the_idle_c, 20_000, {
        let c = Case::new();
        let marker = format!("{}/busy.pid", c.root);
        let factory = c.factory(300.0, 30_000.0);
        let stream: Arc<dyn ExecStream> =
            factory.launch(&format!("( while :; do :; done ) & echo $! > {marker}; sleep 20"), Launch::default());
        let busy = grandchild(&marker).await;
        let (lines, failure) = support::collect(&*stream).await;
        assert!(is_idle_cut(&failure.expect("a cut").message));
        // The TS also reads that no uncaught exception escaped the timer; a failed spawn in this crate has no such
        // road, so the process still being here to read this is that assertion.
        assert_eq!(lines, Vec::<String>::new());
        assert_eq!(stream.exited().await, None);
        assert!(c.trees() > 0, "the idle clock never read the tree");
        gone(busy).await;
    });

    own_group_test!(a_stream_with_no_idle_limit_as_the_exec_verb_runs_one_never_reads_its_tree_at_al, 20_000, {
        let c = Case::new();
        let stream = c.factory(f64::INFINITY, f64::INFINITY).launch("sleep 0.5; echo built", Launch::default());
        let lines = support::lines_ok(&*stream).await;
        assert_eq!(lines, vec!["built".to_string()]);
        assert_eq!(stream.exited().await, Some(0));
        assert_eq!(c.trees(), 0);
    });

    own_group_test!(a_stream_that_never_goes_quiet_for_half_its_idle_limit_never_reads_its_tree_eith, 20_000, {
        let c = Case::new();
        let stream = c.factory(10_000.0, 30_000.0).launch("for i in 1 2 3 4 5; do echo tick; sleep 0.1; done", Launch::default());
        let lines = support::lines_ok(&*stream).await;
        assert_eq!(lines.len(), 5);
        assert_eq!(stream.exited().await, Some(0));
        assert_eq!(c.trees(), 0);
    });
}
