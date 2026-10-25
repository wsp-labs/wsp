// SPDX-License-Identifier: AGPL-3.0-only
//! threads_kept: agent processes kept between turns and launched ahead. threads.ts:98-301. Spec:
//! spec/threads_kept.md.
use std::cell::RefCell;
use std::rc::Rc;

use futures::future::LocalBoxFuture;
use wsp_adapters::port::{SessionFile, SessionRenameWrite};
use wsp_core::types::internal::{KeptLaunch, KeptProcess};
use wsp_words::exit::HostError;

use super::{KeptHand, ThreadsArea};

/// The protocol's kept-agent numbers (wire/limits.ts:45-70).
pub const AGENT_KEEP_MS: i64 = 30 * 60_000;
pub const AGENTS_KEPT: usize = 6;
pub const KEPT_AGENT_MB: f64 = 400.0;
pub const KEPT_MEMORY_SHARE: f64 = 0.25;
pub const AGENT_WARM_MS: i64 = 3 * 60_000;
pub const AGENTS_WARM: usize = 2;

/// limits.ts:59 `agentsKeptOn`: K1.
pub fn agents_kept_on(mem_mb: f64) -> usize {
    todo!()
}

/// `{ place, cap }` (threads.ts:132).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptBox {
    pub place: String,
    pub cap: usize,
}

/// The shell line `boxFileSize` sends (K4).
pub fn box_file_line(at: &SessionFile) -> String {
    todo!()
}

impl ThreadsArea {
    /// threads.ts:132: K1.
    pub fn kept_box(&self, workspace_id: &str) -> Option<KeptBox> {
        todo!()
    }
    /// threads.ts:105: K2. `now`: end its tree at once.
    pub fn reap_kept(&self, thread_id: &str, now: bool) {
        todo!()
    }
    /// threads.ts:111: K3.
    pub fn end_kept(&self, thread_id: &str, kept: Rc<RefCell<KeptProcess>>, now: bool) {
        todo!()
    }
    /// threads.ts:140: K4. -1 where the agent wrote no file there.
    pub fn box_file_size(&self, workspace_id: &str, at: &SessionFile) -> LocalBoxFuture<'static, Option<i64>> {
        todo!()
    }
    /// threads.ts:160: K6.
    pub async fn kept_check(&self, thread_id: &str) {
        todo!()
    }
    /// threads.ts:177: K5.
    pub fn write_session(
        &self,
        harness_session_id: &str,
        write: LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>>,
    ) -> LocalBoxFuture<'static, Result<SessionRenameWrite, HostError>> {
        todo!()
    }
    /// threads.ts:206: K7.
    pub fn take_kept(&self, thread_id: &str, launch: Option<&KeptLaunch>, session: Option<&str>, whole: Option<&KeptLaunch>) -> Option<Rc<RefCell<KeptProcess>>> {
        todo!()
    }
    /// threads.ts:224: K8. Whether it was kept (the turn's token and device stay with it only then).
    pub fn hold_kept(&self, thread_id: &str, o: KeptHand) -> bool {
        todo!()
    }
    /// threads.ts:240: K9.
    pub fn hold_warm(&self, thread_id: &str, o: KeptHand) {
        todo!()
    }
    /// threads.ts:254: K10. The thread id and the process.
    pub fn warm_on(&self, workspace_id: &str, harness: &str) -> Option<(String, Rc<RefCell<KeptProcess>>)> {
        todo!()
    }
    /// threads.ts:257: K11.
    pub fn rewarm(&self, thread_id: &str) {
        todo!()
    }
    /// threads.ts:266: K12.
    pub fn claim_warm(&self, workspace_id: &str, harness: &str) -> Option<String> {
        todo!()
    }
    /// threads.ts:274: K13.
    pub(crate) fn arm(&self, thread_id: &str, kept: Rc<RefCell<KeptProcess>>, ms: i64) {
        todo!()
    }
    /// threads.ts:281: K14.
    pub(crate) fn keep(&self, thread_id: &str, kept: KeptProcess, ms: i64) {
        todo!()
    }
}
