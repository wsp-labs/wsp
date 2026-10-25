// SPDX-License-Identifier: AGPL-3.0-only
//! wsp-machines' `PlaceDoor` over the script: every method records `places.<method in camelCase>` (the TS member's
//! name: `places.list`, `places.hostKey`, `places.answerChallenge`, `places.prove`, `places.attach`, `places.reportOf`,
//! `places.channel`, ...) with its arguments as JSON (camelCase keys, as the TS names them), then answers what the
//! test scripted under that name. A `Result` method unscripted refuses `unscripted call: <name>`; a method that
//! cannot fail answers its type's empty value unscripted (`None`, `false`, `vec![]`, `""` for `name_of` gives the
//! id back, `host_key` gives `"SHA256:fake-host-key"`). `channel` keeps the `on_event` it is handed on
//! `channel_listeners` before it answers, so a test can drive it.
use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::Serialize;
use serde_json::{json, Value};
use wsp_engine::machine::{ExecResult, MachineBackend};
use wsp_engine::provision::ProvisionStage;
use wsp_machines::places::helpers::LinkSocket;
use wsp_machines::places::pane_ports::PanePorts;
use wsp_machines::places::types::*;
use wsp_store::keys::Seal;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{PendingComputer, PlaceAddJob, PlaceProvisionRow, PlaceSetAlso, PlaceSetupStep};
use wsp_wire::views::place_more::{PlaceDial, PlaceHolds, PlaceRemoved, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk, PlaceUpdateReply, PlaceView};
use wsp_wire::views::place_setup_words::PlaceEstimate;
use wsp_core::types::pending::{DaemonChannel, DaemonReach};
use wsp_wire::wire::machine_link::SysSample;
use wsp_wire::wire::place_link::{PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, PlaceReport};

use super::script::Script;

/// An argument as the call log writes it: its JSON where it serializes.
pub fn sj<T: Serialize>(x: &T) -> Value {
    serde_json::to_value(x).unwrap_or(Value::Null)
}

/// An argument that only has Debug, as its Debug string.
pub fn dbg<T: Debug>(x: &T) -> Value {
    Value::String(format!("{x:?}"))
}

pub struct FakePlaceDoor {
    pub script: Rc<Script>,
    /// Every `on_event` `channel` was handed, oldest first.
    pub channel_listeners: RefCell<Vec<Rc<dyn Fn(serde_json::Map<String, Value>)>>>,
}

impl FakePlaceDoor {
    pub fn new(script: Rc<Script>) -> Rc<FakePlaceDoor> {
        Rc::new(FakePlaceDoor { script, channel_listeners: RefCell::new(Vec::new()) })
    }
}

#[async_trait(?Send)]
impl PlaceDoor for FakePlaceDoor {
    fn answer_challenge(&self, subject: &str, nonce: &str, ephemeral: Option<&str>) -> Option<PlaceChallenge> {
        self.script.call_or("places.answerChallenge", json!({ "subject": subject, "nonce": nonce, "ephemeral": ephemeral }), || None)
    }
    async fn join(&self, req: PlaceJoinRequest, from: &str, now: u64) -> PlaceResult<Option<JoinAnswer>> {
        self.script.call("places.join", json!({ "req": sj(&req), "from": from, "now": now }))
    }
    async fn auth(&self, req: PlaceAuthRequest, now: u64) -> PlaceResult<AuthAnswer> {
        self.script.call("places.auth", json!({ "req": sj(&req), "now": now }))
    }
    fn host_key(&self) -> String {
        self.script.call_or("places.hostKey", json!({}), || "SHA256:fake-host-key".to_string())
    }
    async fn prove(&self, place_id: &str, req: PlaceProveRequest, expect: &[u8], from: &str, now: u64) -> PlaceResult<Proved> {
        self.script.call("places.prove", json!({ "placeId": place_id, "req": sj(&req), "expect": expect, "from": from, "now": now }))
    }
    async fn attach(&self, place_id: &str, _socket: Rc<dyn LinkSocket>, report: PlaceReport, from: &str, now: u64, seal: Option<Seal>) -> PlaceResult<()> {
        self.script.call("places.attach", json!({ "placeId": place_id, "socket": "_", "report": sj(&report), "from": from, "now": now, "seal": seal.as_ref().map(|_| "_") }))
    }
    fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> {
        self.script.call_or("places.link", json!({ "placeId": place_id }), || None)
    }
    fn channel(&self, place_id: &str, on_event: Rc<dyn Fn(serde_json::Map<String, Value>)>) -> Option<Rc<dyn DaemonChannel>> {
        self.channel_listeners.borrow_mut().push(on_event);
        self.script.call_or("places.channel", json!({ "placeId": place_id, "onEvent": "_" }), || None)
    }
    async fn watch_sys(&self, place_id: &str, _f: Rc<dyn Fn(SysSample)>) -> PlaceResult<Box<dyn FnOnce()>> {
        self.script.call("places.watchSys", json!({ "placeId": place_id, "f": "_" }))
    }
    async fn load(&self) -> PlaceResult<()> {
        self.script.call("places.load", json!({}))
    }
    fn backend_of(&self, place_id: &str) -> Option<std::sync::Arc<dyn MachineBackend>> {
        self.script.call_or("places.backendOf", json!({ "placeId": place_id }), || None)
    }
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        self.script.call_or("places.folderComputer", json!({ "placeId": place_id }), || None)
    }
    fn joined(&self, place_id: &str) -> bool {
        self.script.call_or("places.joined", json!({ "placeId": place_id }), || false)
    }
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<std::sync::Arc<dyn MachineBackend>> {
        self.script.call("places.forkingBackend", json!({ "placeId": place_id }))
    }
    fn name_of(&self, place_id: &str) -> String {
        self.script.call_or("places.nameOf", json!({ "placeId": place_id }), || place_id.to_string())
    }
    fn settings_at(&self, place_id: &str) -> PlaceSettings {
        self.script.call_or("places.settingsAt", json!({ "placeId": place_id }), || serde_json::from_value(json!({})).expect("an empty PlaceSettings parses"))
    }
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>> {
        self.script.call_or("places.turnLimitAt", json!({ "placeId": place_id }), || None)
    }
    fn threads_at(&self, place_id: &str) -> Option<u64> {
        self.script.call_or("places.threadsAt", json!({ "placeId": place_id }), || None)
    }
    fn machines_at(&self, place_id: &str) -> Option<u64> {
        self.script.call_or("places.machinesAt", json!({ "placeId": place_id }), || None)
    }
    fn sign_ins_at(&self, place_id: &str, agents: Option<&[String]>) -> Option<IndexMap<String, AgentSignInState>> {
        self.script.call_or("places.signInsAt", json!({ "placeId": place_id, "agents": agents }), || None)
    }
    fn github_from_vault(&self, place_id: &str) -> bool {
        self.script.call_or("places.githubFromVault", json!({ "placeId": place_id }), || false)
    }
    fn vault_at(&self, place_id: &str, vault: &IndexMap<String, String>) -> IndexMap<String, String> {
        self.script.call_or("places.vaultAt", json!({ "placeId": place_id, "vault": vault }), IndexMap::new)
    }
    async fn login_landed(&self, place_id: &str, agent: &str) -> PlaceResult<()> {
        self.script.call("places.loginLanded", json!({ "placeId": place_id, "agent": agent }))
    }
    async fn logins_again(&self, place_id: &str) -> PlaceResult<()> {
        self.script.call("places.loginsAgain", json!({ "placeId": place_id }))
    }
    async fn key_landed(&self, agent: &str) -> PlaceResult<()> {
        self.script.call("places.keyLanded", json!({ "agent": agent }))
    }
    async fn gh_there(&self, place_id: &str, _stage: ProvisionStage) -> PlaceResult<Vec<PlaceProvisionRow>> {
        self.script.call("places.ghThere", json!({ "placeId": place_id, "stage": "_" }))
    }
    fn offer_of(&self, place_id: &str) -> Option<String> {
        self.script.call_or("places.offerOf", json!({ "placeId": place_id }), || None)
    }
    async fn forward(&self, place_id: &str, place_port: u16, o: ForwardAsk) -> PlaceResult<u16> {
        self.script.call("places.forward", json!({ "placeId": place_id, "placePort": place_port, "o": dbg(&o) }))
    }
    fn pane_forwards(&self) -> Rc<PanePorts> {
        self.script.record("places.paneForwards", json!({}));
        panic!("the fake place door has no pane forwards")
    }
    async fn place_for(&self, word: &str) -> PlaceResult<PlaceFor> {
        self.script.call("places.placeFor", json!({ "word": word }))
    }
    async fn default_place(&self) -> PlaceResult<PlaceFor> {
        self.script.call("places.defaultPlace", json!({}))
    }
    async fn mark_used(&self, place_id: Option<&str>) -> PlaceResult<()> {
        self.script.call("places.markUsed", json!({ "placeId": place_id }))
    }
    async fn mark_default_if_none(&self, place_id: &str) -> PlaceResult<()> {
        self.script.call("places.markDefaultIfNone", json!({ "placeId": place_id }))
    }
    async fn add(&self, req: AddAsk, now: u64) -> PlaceResult<PlaceAdded> {
        self.script.call("places.add", json!({ "req": dbg(&req), "now": now }))
    }
    async fn set_up(&self, reference: &str, o: SetUpAsk) -> PlaceResult<PlaceSetUp> {
        self.script.call("places.setUp", json!({ "reference": reference, "o": dbg(&o) }))
    }
    async fn pending(&self) -> PlaceResult<Vec<PendingComputer>> {
        self.script.call("places.pending", json!({}))
    }
    async fn choose(&self, reference: &str, choices: RecipeFile, recipe: Option<String>) -> PlaceResult<PendingComputer> {
        self.script.call("places.choose", json!({ "reference": reference, "choices": sj(&choices), "recipe": recipe }))
    }
    async fn dial(&self, place_id: &str, now: u64) -> PlaceResult<PlaceDial> {
        self.script.call("places.dial", json!({ "placeId": place_id, "now": now }))
    }
    async fn set(&self, place_id: &str, set: PlaceSettingsAsk, reset: Vec<PlaceSettingWord>, also: PlaceSetAlso) -> PlaceResult<PlaceView> {
        self.script.call("places.set", json!({ "placeId": place_id, "set": sj(&set), "reset": sj(&reset), "also": sj(&also) }))
    }
    async fn road(&self, place_id: &str) -> PlaceResult<u16> {
        self.script.call("places.road", json!({ "placeId": place_id }))
    }
    async fn exec(&self, place_id: &str, cmd: &str, opts: PlaceExecOpts) -> PlaceResult<ExecResult> {
        self.script.call("places.exec", json!({ "placeId": place_id, "cmd": cmd, "opts": dbg(&opts) }))
    }
    async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>> {
        self.script.call("places.reportOf", json!({ "placeId": place_id }))
    }
    async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>> {
        self.script.call("places.homeOf", json!({ "placeId": place_id }))
    }
    async fn list(&self, now: u64) -> PlaceResult<Vec<PlaceView>> {
        self.script.call("places.list", json!({ "now": now }))
    }
    async fn rows(&self) -> PlaceResult<Vec<PlaceView>> {
        self.script.call("places.rows", json!({}))
    }
    fn adds(&self) -> Vec<PlaceAddJob> {
        self.script.call_or("places.adds", json!({}), Vec::new)
    }
    async fn update(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceUpdateReply> {
        self.script.call("places.update", json!({ "placeId": place_id, "ask": dbg(&ask) }))
    }
    async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds> {
        self.script.call("places.holds", json!({ "placeId": place_id }))
    }
    async fn remove(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceRemoved> {
        self.script.call("places.remove", json!({ "placeId": place_id, "ask": dbg(&ask) }))
    }
    async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>> {
        self.script.call("places.find", json!({ "reference": reference }))
    }
    async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView> {
        self.script.call("places.follow", json!({ "placeId": place_id, "recipe": recipe }))
    }
    async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>> {
        self.script.call("places.recipeChanged", json!({ "slug": slug, "afterMs": after_ms }))
    }
    async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>> {
        self.script.call("places.followers", json!({}))
    }
    async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView> {
        self.script.call("places.skip", json!({ "placeId": place_id, "row": row }))
    }
    async fn setup_log(&self, place_id: &str, step: Option<PlaceSetupStep>) -> PlaceResult<Vec<String>> {
        self.script.call("places.setupLog", json!({ "placeId": place_id, "step": step.as_ref().map(sj) }))
    }
    async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate> {
        self.script.call("places.estimate", json!({ "reference": reference, "choices": sj(&choices) }))
    }
    async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>> {
        self.script.call("places.unfollow", json!({ "slug": slug }))
    }
    async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>> {
        self.script.call("places.picksOf", json!({ "placeId": place_id }))
    }
    fn on(&self, _f: Rc<dyn Fn(&PlaceEvent)>) -> u64 {
        self.script.call_or("places.on", json!({ "f": "_" }), || 0)
    }
    fn off(&self, id: u64) {
        self.script.call_or("places.off", json!({ "id": id }), || ())
    }
    async fn close(&self) -> PlaceResult<()> {
        self.script.call("places.close", json!({}))
    }
}
