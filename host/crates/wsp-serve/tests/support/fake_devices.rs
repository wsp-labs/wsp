// SPDX-License-Identifier: AGPL-3.0-only
//! A device door over the script: every method records `devices.<method>` with its arguments and answers what the
//! test scripted. Unscripted, `match` answers nobody, `seen` and `refuses` answer none and false, and the rest refuse.
use std::rc::Rc;

use async_trait::async_trait;
use serde_json::json;
use wsp_core::devices::{DeviceDoor, HeldDevice, IssuedCode, MintMore, PairedDevice};
use wsp_wire::views::workspace_more::ThreadScope;
use wsp_wire::wire::devices::{DeviceVia, DeviceView};
use wsp_words::exit::HostError;

use super::script::Script;

pub struct FakeDevices {
    pub script: Rc<Script>,
}

impl FakeDevices {
    pub fn new(script: Rc<Script>) -> Rc<FakeDevices> {
        Rc::new(FakeDevices { script })
    }
}

#[async_trait(?Send)]
impl DeviceDoor for FakeDevices {
    async fn issue(&self, now: i64, ttl_ms: u64, here: bool) -> Result<IssuedCode, HostError> {
        self.script.call("devices.issue", json!({ "now": now, "ttlMs": ttl_ms, "here": here }))
    }
    async fn redeem(&self, code: &str, name: &str, now: i64) -> Result<Option<PairedDevice>, HostError> {
        self.script.call("devices.redeem", json!({ "code": code, "name": name, "now": now }))
    }
    async fn spend(&self, code: &str, now: i64) -> Result<bool, HostError> {
        self.script.call("devices.spend", json!({ "code": code, "now": now }))
    }
    async fn admit(&self, name: &str, now: i64) -> Result<PairedDevice, HostError> {
        self.script.call("devices.admit", json!({ "name": name, "now": now }))
    }
    async fn admit_account(&self, name: &str, via: DeviceVia, now: i64) -> Result<PairedDevice, HostError> {
        self.script.call("devices.admitAccount", json!({ "name": name, "via": serde_json::to_value(&via).unwrap(), "now": now }))
    }
    async fn refuses(&self, fingerprint: &str) -> Result<bool, HostError> {
        let call = self.script.record("devices.refuses", json!({ "fingerprint": fingerprint }));
        self.script.answer(&call).unwrap_or(Ok(false))
    }
    async fn mint(&self, name: &str, scope: ThreadScope, now: i64, more: MintMore) -> Result<PairedDevice, HostError> {
        self.script.call(
            "devices.mint",
            json!({ "name": name, "scope": serde_json::to_value(&scope).unwrap(), "now": now, "road": more.road, "aside": more.aside }),
        )
    }
    async fn revoke_asides(&self) -> Result<(), HostError> {
        self.script.call("devices.revokeAsides", json!({}))
    }
    async fn r#match(&self, token: &str) -> Result<Option<HeldDevice>, HostError> {
        let call = self.script.record("devices.match", json!({ "token": token }));
        self.script.answer(&call).unwrap_or(Ok(None))
    }
    async fn seen(&self, id: &str, now: i64) -> Result<Option<HeldDevice>, HostError> {
        let call = self.script.record("devices.seen", json!({ "id": id, "now": now }));
        self.script.answer(&call).unwrap_or(Ok(None))
    }
    async fn list(&self) -> Result<Vec<DeviceView>, HostError> {
        self.script.call("devices.list", json!({}))
    }
    async fn revoke(&self, id: &str) -> Result<bool, HostError> {
        self.script.call("devices.revoke", json!({ "id": id }))
    }
}
