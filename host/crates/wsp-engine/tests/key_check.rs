// SPDX-License-Identifier: AGPL-3.0-only
//! Ports packages/engine/test/key-check.test.ts.
#[path = "rest_common/mod.rs"]
mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde_json::{json, Value};

use common::{reply, FakeFetch, NoWaitClock, Play};
use wsp_engine::errors::{Result, RoadCause, RoadError};
use wsp_engine::fetch::Fetch;
use wsp_engine::key_check::{check_provider_key, key_check_line, KeyCheck};
use wsp_engine::machine::{BackendOffers, BackendPricing, Capabilities, Machine, MachineBackend, MachineListRow, MachineSpec, Waiting};
use wsp_engine::solari_backend::{SolariBackend, SolariBackendOptions};

fn solari(api_key: &str, f: &Arc<FakeFetch>, clock: bool) -> Arc<SolariBackend> {
    let fetch: Arc<dyn Fetch> = f.clone();
    let clock: Option<Arc<dyn wsp_engine::errors::RetryClock>> = if clock { Some(NoWaitClock::new()) } else { None };
    SolariBackend::new(SolariBackendOptions { api_key: api_key.into(), fetch: Some(fetch), clock, ..Default::default() })
}

/// A backend with no key to check: its offers have no `check_key`, and the road counts the calls it gets.
struct KeylessBackend {
    key_checks: AtomicUsize,
}

#[async_trait]
impl MachineBackend for KeylessBackend {
    fn capabilities(&self) -> Capabilities {
        panic!("a backend that holds no key was asked for its capabilities")
    }
    fn pricing(&self) -> Arc<dyn BackendPricing> {
        panic!("a backend that holds no key was asked for its pricing")
    }
    fn offers(&self) -> BackendOffers {
        BackendOffers { check_key: false, ..BackendOffers::default() }
    }
    async fn create(&self, _spec: MachineSpec, _waiting: Option<Waiting>, _sent: Option<String>) -> Result<Arc<dyn Machine>> {
        panic!("a backend that holds no key was asked to create")
    }
    async fn get(&self, _id: &str) -> Result<Arc<dyn Machine>> {
        panic!("a backend that holds no key was asked for a machine")
    }
    async fn list(&self, _labels: Option<&IndexMap<String, String>>) -> Result<Vec<MachineListRow>> {
        panic!("a backend that holds no key was asked to list")
    }
    async fn delete_snapshot(&self, _id: &str) -> Result<()> {
        panic!("a backend that holds no key was asked to delete a snapshot")
    }
    async fn check_key(&self) -> Result<Value> {
        self.key_checks.fetch_add(1, Ordering::SeqCst);
        Ok(Value::Null)
    }
}

// TS: packages/engine/test/key-check.test.ts:23
#[tokio::test]
async fn asks_the_template_table_the_cheapest_read_the_key_opens_and_nothing_else() {
    let f = FakeFetch::fixed(vec![(
        "GET /templates",
        reply(200, json!({ "templates": [{ "templateId": "base", "name": "base", "status": "ready" }] })),
    )]);
    let b = solari("slr_live_fake", &f, false);
    let check = check_provider_key(b.as_ref()).await;
    assert_eq!(check, KeyCheck::Taken { answer: None });
    assert_eq!(key_check_line(&check, "solari", false), None);
    let calls = f.calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].url, "https://api.getsolari.com/templates");
}

// TS: packages/engine/test/key-check.test.ts:33
#[tokio::test]
async fn reads_a_refusal_as_the_providers_own_status_and_word_under_our_sentence() {
    let f = FakeFetch::fixed(vec![("GET /templates", reply(401, json!({ "error": "Unauthorized" })))]);
    let b = solari("slr_live_wrong", &f, false);
    let check = check_provider_key(b.as_ref()).await;
    assert_eq!(check, KeyCheck::Refused { said: "401 Unauthorized".into() });
    assert_eq!(key_check_line(&check, "solari", false).as_deref(), Some("Solari refused this key: 401 Unauthorized"));
    assert_eq!(key_check_line(&check, "box", false).as_deref(), Some("Boat refused this key: 401 Unauthorized"));
    assert_eq!(key_check_line(&check, "solari", true).as_deref(), Some("Solari refused the saved key: 401 Unauthorized"));
    assert_eq!(key_check_line(&check, "box", true).as_deref(), Some("Boat refused the saved key: 401 Unauthorized"));
}

// TS: packages/engine/test/key-check.test.ts:43
#[tokio::test]
async fn a_plan_that_may_not_read_the_account_is_the_same_refusal_the_key_alone_will_not_do() {
    let f = FakeFetch::fixed(vec![("GET /templates", reply(403, json!({ "error": "NotEntitled" })))]);
    let b = solari("slr_live_free", &f, false);
    assert_eq!(check_provider_key(b.as_ref()).await, KeyCheck::Refused { said: "403 NotEntitled".into() });
}

// TS: packages/engine/test/key-check.test.ts:48
#[tokio::test]
async fn a_road_that_never_left_this_computer_says_so_instead_of_blaming_the_key() {
    let enotfound = RoadError {
        name: "TypeError".into(),
        message: "fetch failed".into(),
        code: None,
        cause: Some(RoadCause { message: String::new(), code: Some("ENOTFOUND".into()), errors: None }),
    };
    let f = FakeFetch::fixed(vec![("GET /templates", Play::Fail(enotfound))]);
    let b = solari("slr_live_fake", &f, true);
    let check = check_provider_key(b.as_ref()).await;
    assert_eq!(check, KeyCheck::Unchecked { said: "fetch failed".into() });
    assert_eq!(key_check_line(&check, "solari", false).as_deref(), Some("Solari could not be reached to check the key: fetch failed"));
    assert_eq!(key_check_line(&check, "box", false).as_deref(), Some("Boat could not be reached to check the key: fetch failed"));
    assert_eq!(key_check_line(&check, "solari", true).as_deref(), Some("Solari could not be reached to check the key: fetch failed"));
}

// TS: packages/engine/test/key-check.test.ts:58
#[tokio::test]
async fn a_gateway_status_answered_about_the_gateway_not_the_key() {
    let f = FakeFetch::fixed(vec![("GET /templates", reply(503, json!({ "error": "Service Unavailable" })))]);
    let b = solari("slr_live_fake", &f, true);
    assert_eq!(check_provider_key(b.as_ref()).await, KeyCheck::Unchecked { said: "Service Unavailable".into() });
}

// TS: packages/engine/test/key-check.test.ts:62
#[tokio::test]
async fn a_backend_that_holds_no_key_has_none_to_check() {
    let keyless = KeylessBackend { key_checks: AtomicUsize::new(0) };
    assert_eq!(check_provider_key(&keyless).await, KeyCheck::Taken { answer: None });
    assert_eq!(keyless.key_checks.load(Ordering::SeqCst), 0);
}
