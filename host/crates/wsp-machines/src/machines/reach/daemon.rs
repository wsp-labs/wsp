// SPDX-License-Identifier: AGPL-3.0-only
//! reach.ts:199-372, 381-474: the dial preamble, the wait on a woken daemon, the hello's version, the checkout read
//! and a lead's tree. Spec: spec/reach_daemon.md.
use std::rc::Rc;

use tokio_util::sync::CancellationToken;

use wsp_wire::types::changes::Checkout;
use wsp_wire::types::tree::{TreeChild, TreeFact};

use super::{DaemonLink, ReachArea};
use crate::machines::core_types::{AreaResult, Live};

/// reach.ts:212-213, 278, 319.
pub const ASK_DAEMON_MS: f64 = 5_000.0;
pub const ASK_AGAIN_MS: f64 = 500.0;
pub const START_DAEMON_AFTER_MS: f64 = 60_000.0;
pub const LOOK_EXITS_EVERY_MS: f64 = 10_000.0;

impl ReachArea {
    /// reach.ts:203 (rule 2). `None` where the guest holds no daemon token.
    pub(crate) async fn dial_daemon(&self, entry: &Live, deadline: f64, on_event: Option<Rc<dyn Fn(&serde_json::Value)>>, heartbeat_ms: Option<f64>) -> AreaResult<Option<Rc<dyn DaemonLink>>> {
        todo!()
    }

    /// reach.ts:222 (rule 3). `None` is the daemon answering; `Some` the row's words for why not.
    pub(crate) async fn daemon_answer(&self, entry: &Live, quit: Option<CancellationToken>) -> Option<String> {
        todo!()
    }

    /// reach.ts:280 (rule 4).
    pub(crate) async fn starting_daemon(&self, entry: &Live, quit: CancellationToken) -> Option<String> {
        todo!()
    }

    /// reach.ts:321 (rule 5).
    pub async fn ping_daemon(&self, entry: &Live) -> Option<String> {
        todo!()
    }

    /// reach.ts:357 (rule 6).
    pub async fn hello_version(&self, entry: &Live) -> Option<f64> {
        todo!()
    }

    /// reach.ts:386 (rule 7).
    pub async fn read_checkout(&self, entry: &Live, force: bool) -> Option<Checkout> {
        todo!()
    }

    /// reach.ts:428 (rule 8).
    pub async fn read_tree(&self, lead: &Live) -> Option<TreeFact> {
        todo!()
    }

    /// reach.ts:456 (rule 9).
    pub(crate) async fn tree_child_of(&self, child: &Live, lead_branch: &str) -> TreeChild {
        todo!()
    }

    /// reach.ts:471 (rule 10).
    pub fn read_lead_of(&self, child: &Live) {
        todo!()
    }
}
