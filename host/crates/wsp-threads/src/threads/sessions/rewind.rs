// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:1264-1396: a thread back to one of its turns. Spec: spec/sessions_rewind.md.
use wsp_core::types::api::RewindAsk;
use wsp_wire::views::workspace::Caller;
use wsp_wire::wire::runtime::SessionRewindResult;
use wsp_words::exit::HostError;

use super::SessionsArea;

impl SessionsArea {
    /// sessions.ts:1264. Rules W1-W14.
    pub async fn rewind_thread(&self, thread_id: &str, ask: RewindAsk, origin: Option<Caller>) -> Result<SessionRewindResult, HostError> {
        todo!()
    }
}
