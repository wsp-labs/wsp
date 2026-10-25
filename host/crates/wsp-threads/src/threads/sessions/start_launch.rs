// SPDX-License-Identifier: AGPL-3.0-only
//! sessions.ts:476-895: from a checked start to a running turn. Spec: spec/sessions_launch.md.
use std::rc::Rc;

use wsp_core::types::wiring::SessionHandle;
use wsp_words::exit::HostError;

use super::start::Prepared;
use super::SessionsArea;

impl SessionsArea {
    /// sessions.ts:476-895. Rules L1-L21, Z1-Z6.
    pub async fn launch_prepared(&self, prepared: Prepared) -> Result<Rc<dyn SessionHandle>, HostError> {
        todo!()
    }
}
