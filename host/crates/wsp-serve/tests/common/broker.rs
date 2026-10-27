// SPDX-License-Identifier: AGPL-3.0-only
//! The `broker` option of the TS whole-runtime tests (`broker: { ready, codexClaims, status }`): a host that keeps the person's
//! credentials, ready or not as the test says, with the Codex login it holds.
use std::rc::Rc;

use async_trait::async_trait;
use futures::FutureExt as _;
use wsp_adapters::pending::CodexClaims;
use wsp_core::types::wiring::{Broker, Local};
use wsp_words::exit::HostError;

pub struct TestBroker {
    pub ready: Rc<dyn Fn() -> bool>,
    pub claims: Option<CodexClaims>,
    /// What `status` answers, where the test names one.
    pub status: Option<wsp_wire::wire::runtime::BrokerStatus>,
}

impl TestBroker {
    /// `{ ready: () => true, codexClaims: async () => undefined }`.
    pub fn ready() -> TestBroker {
        TestBroker { ready: Rc::new(|| true), claims: None, status: None }
    }
    pub fn gated(ready: impl Fn() -> bool + 'static) -> TestBroker {
        TestBroker { ready: Rc::new(ready), claims: None, status: None }
    }
    /// A Codex login of `kind` ("chatgpt" or "key") whose account is `account`.
    pub fn codex(mut self, kind: &str, account: &str) -> TestBroker {
        self.status = Some(serde_json::from_value(serde_json::json!({ "codex": kind })).expect("a status"));
        self.claims = Some(CodexClaims { account_id: account.into(), plan_type: None, user_id: None, fedramp: None });
        self
    }
    pub fn rc(self) -> Rc<dyn Broker> {
        Rc::new(self)
    }
}

#[async_trait(?Send)]
impl Broker for TestBroker {
    fn ready(&self) -> bool {
        (self.ready)()
    }
    async fn codex_claims(&self) -> Result<Option<CodexClaims>, HostError> {
        Ok(self.claims.clone())
    }
    fn status(&self) -> Option<Local<'static, wsp_wire::wire::runtime::BrokerStatus>> {
        let status = self.status.clone()?;
        Some(async move { Ok(status) }.boxed_local())
    }
}
