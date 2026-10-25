// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:1398-1480: delete and forget a thread. Spec: spec/sessions_delete.md.
use wsp_core::types::api::{DeleteThreadAsk, ThreadDeleted};
use wsp_wire::views::workspace::Caller;
use wsp_words::exit::HostError;

use super::SessionsArea;

impl SessionsArea {
    /// sessions.ts:1398. Rules D1-D8.
    pub async fn delete_thread(&self, thread_id: &str, origin: Option<Caller>, ask: DeleteThreadAsk) -> Result<ThreadDeleted, HostError> {
        todo!()
    }
    /// sessions.ts:1453. Rules D9-D11.
    pub async fn forget_thread(&self, thread_id: &str, origin: Option<Caller>, check: bool) -> Result<(), HostError> {
        todo!()
    }
}
