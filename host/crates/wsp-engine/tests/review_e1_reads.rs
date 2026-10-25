// SPDX-License-Identifier: AGPL-3.0-only
//! Review of packet e1: disk_sync.rs and target_line.rs against the TS.
#[path = "rest_common/mod.rs"]
mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{ok, FakeMachine};
use wsp_engine::daemon_targets::Platform;
use wsp_engine::disk_sync::sync_disk;
use wsp_engine::target_line::{stage_as_login, TargetLogin};

/// disk-sync.ts:34-35: `^Dirty: (\d+)$` with the m flag is the first line that matches whole, so a `Dirty: abc` line is
/// skipped for a later `Dirty: 5`. node answers (5, 7) for each of these; the Rust takes the first line that starts
/// `Dirty: ` and gives up.
#[tokio::test]
async fn a_counter_is_the_first_line_that_matches_whole_not_the_first_line_that_starts_with_its_name() {
    for stdout in ["Dirty: abc\nDirty: 5\nWriteback: 7\n", "Dirty: 5\nWriteback: x\nWriteback: 7\n", "Dirty: \nDirty: 5\nWriteback: 7\n"] {
        let m = FakeMachine::exec("m1", move |_, _| Ok(ok(stdout)));
        let got = sync_disk(&m).await.unwrap().map(|r| (r.dirty_kb, r.writeback_kb));
        assert_eq!(got, Some((5.0, 7.0)), "{stdout:?}");
    }
}

/// target-line.ts:107-113 removes the staging folder in a `finally`, which runs even when the caller stopped waiting
/// (a TS caller cannot cancel the call). node, `use` taking 50 ms and the caller giving up at 10 ms, ends with
/// `rm -rf -- '/tmp/wsp-land-X'`. A Rust caller that drops the future leaves the folder, and the bytes in it, behind.
#[tokio::test]
async fn the_staging_folder_is_removed_even_when_the_caller_stops_waiting() {
    let mut m = FakeMachine::new("m1");
    m.put_bytes_fn = Some(Arc::new(|_, _, _| Ok(None)));
    let t = TargetLogin { platform: Platform::Linux, home: "/home/ada".into(), path: None, user: "ada".into(), run_as: None };
    let use_it = |_, _| async {
        tokio::time::sleep(Duration::from_millis(50)).await;
        Ok(7)
    };
    let staged = stage_as_login(&m, &m, &t, "the daemon", &[1], use_it, None);
    assert!(tokio::time::timeout(Duration::from_millis(10), staged).await.is_err(), "the caller gave up");
    tokio::time::sleep(Duration::from_millis(120)).await;
    let cmds = m.cmds();
    assert!(cmds.last().is_some_and(|c| c.starts_with("rm -rf -- ")), "the folder was left behind: {cmds:?}");
}
