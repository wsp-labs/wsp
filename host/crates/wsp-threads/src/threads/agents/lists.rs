// SPDX-License-Identifier: AGPL-3.0-only
//! agents_lists: what each agent offers on a machine, and thread titles. agents.ts:26-324, 682-703. Spec:
//! spec/agents_lists.md.
use std::sync::Arc;

use futures::future::LocalBoxFuture;
use wsp_core::types::harness::HarnessAdapter;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::session::{SessionView, TitleSource};
use wsp_words::exit::HostError;

use super::super::threads::Entry;
use super::AgentsArea;


pub use wsp_core::types::events::{CATALOG_TTL_MS, CATALOG_PROBE_TIMEOUT_MS, FIRST_RUN_READ_MS, AGENT_VERSION_READ_MS, SESSION_TITLE_TTL_MS, SESSION_TITLE_TIMEOUT_MS, SESSION_TITLE_REFRESH_MAX, TITLE_MAKE_TIMEOUT_MS};
pub use wsp_core::types::internal::{AGENT_LISTS};
/// `{ harnessTitle, titleSource }` (agents.ts:246).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CarriedTitle {
    pub harness_title: Option<String>,
    pub title_source: Option<TitleSource>,
}

/// agents.ts:60-76 `known`: L1.
pub fn known_catalog(table: &HarnessCatalog, adapter: &dyn HarnessAdapter) -> HarnessCatalog {
    todo!()
}

/// agents.ts:228 `sourceOf`: L6.
pub fn source_of(view: &SessionView) -> TitleSource {
    todo!()
}

/// agents.ts:317 `titleRows`: L9.
pub fn title_rows(rows: &[SessionView]) -> Vec<SessionView> {
    todo!()
}

impl AgentsArea {
    /// agents.ts:58: L2, L3.
    pub fn catalog_on(&self, table: &HarnessCatalog, entry: &Entry, adapter: Arc<dyn HarnessAdapter>) -> LocalBoxFuture<'static, HarnessCatalog> {
        todo!()
    }
    /// agents.ts:52: L4.
    pub fn first_run_here(&self, entry: &Entry, harness: &str) -> LocalBoxFuture<'static, bool> {
        todo!()
    }
    /// agents.ts:167: L5. Never fails.
    pub fn refresh_title(&self, view: &SessionView, force: bool) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// agents.ts:230: L6.
    pub fn rows_on(&self, thread_id: &str) -> Vec<SessionView> {
        todo!()
    }
    /// agents.ts:233: L6.
    pub fn opening_of(&self, thread_id: &str) -> Option<String> {
        todo!()
    }
    /// agents.ts:236: L6.
    pub fn thread_source(&self, thread_id: &str) -> TitleSource {
        todo!()
    }
    /// agents.ts:246: L6.
    pub fn carried_title(&self, thread_id: &str) -> CarriedTitle {
        todo!()
    }
    /// agents.ts:257: L7. Never fails.
    pub fn name_in_harness(&self, view: &SessionView, title: &str) -> LocalBoxFuture<'static, ()> {
        todo!()
    }
    /// agents.ts:284: L8.
    pub async fn make_title(&self, view: &SessionView) -> Result<(), HostError> {
        todo!()
    }
    /// agents.ts:689: L10.
    pub async fn agent_lists(&self, entry: Option<&Entry>) -> Vec<HarnessCatalog> {
        todo!()
    }
}
