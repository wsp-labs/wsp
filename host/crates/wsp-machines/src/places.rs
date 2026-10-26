// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places.ts` at 65f6b89c7 (the door's assembly, 50 lines) and `places/`. Spec:
//! `spec/places_door.md`.

pub mod context;
pub mod helpers;
pub mod link;
pub mod manage;
pub mod pane_ports;
pub mod provider_remove;
pub mod readings;
pub mod records;
pub mod setup;
pub mod types;
pub mod views;

use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;

use wsp_core::daemon_channel::{ChannelEnd, ChannelError, Frame};
use wsp_core::types::pending::{DaemonChannel, DaemonReach};
use wsp_engine::link_backend::MachineLink;
use wsp_engine::machine::{ExecResult, MachineBackend};
use wsp_engine::provision::ProvisionStage;
use wsp_store::keys::Seal;
use wsp_wire::types::agents_report::AgentSignInState;
use wsp_wire::types::recipe_file::RecipeFile;
use wsp_wire::views::place::{
    PendingComputer, PlaceAddJob, PlaceProvisionRow, PlaceSetAlso, PlaceSetupEvent, PlaceSetupStep,
    PlaceSync, PlaceSyncEvent,
};
use wsp_wire::views::place_more::{
    PlaceDial, PlaceHolds, PlaceRemoved, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk,
    PlaceUpdateReply, PlaceView,
};
use wsp_wire::views::place_setup_words::PlaceEstimate;
use wsp_wire::wire::machine_link::SysSample;
use wsp_wire::wire::place_link::{
    PlaceAuthRequest, PlaceJoinRequest, PlaceProveRequest, PlaceReport,
};

use context::{place_door_context, PlaceDoorContext};
use helpers::LinkSocket;
use link::{LinkDoor, LinkRecords, LinkViews, PlaceChannel};
use manage::{ManageDoor, ManageSetup, SetupPicks, SetupStarted, SyncRead as ManageSyncRead};
use pane_ports::PanePorts;
use records::{LocalFut, PlaceRecordsArea};
use setup::{GivenPicks, PlaceSetupArea, SetupRecords, Started};
use types::{
    AddAsk, AuthAnswer, FolderComputer, ForwardAsk, JoinAnswer, PlaceAdded, PlaceChallenge,
    PlaceDoor, PlaceDoorOptions, PlaceEvent, PlaceExecOpts, PlaceFor, PlaceRecord, PlaceResult,
    PlaceSetUp, Proved, RemoveAsk, Resolved, SetUpAsk,
};
use views::{LinkReads, PlaceViewsArea, ViewsSetup};

pub use helpers::{
    place_home_refusal, sign_ins_of, taken_report, vault_sign_in, ADD_NOT_TAKEN_BACK_LINE,
    ADD_STOPPED_FIX, ADD_STOPPED_LINE, PLACE_BAD_KEY_REFUSAL,
};
pub use types::{
    is_place_add_taken_back, is_place_forks_nowhere, is_place_host_key_changed,
    is_place_login_refused, is_place_provisioning, is_place_record, place_add_taken_back,
    place_elsewhere_swept_over_link_line, place_forks_nowhere, place_host_key_changed,
    place_login_refused, place_login_road_line, place_provisioning, place_swept_over_link_line,
    place_swept_over_ssh_line, place_update_slow_line, HerePlace, PlaceBackHolder, PlaceDialler,
    PlaceInstallRequest, PlaceInstalled, PlaceInstaller, PlaceLeaveRequest, PlaceLeaver,
    PlaceLogReader, PlaceLogin, PlaceProvisioner, PlaceRecordRoad, PlaceRecording, PlaceStaging,
    PlaceUndo, PlaceUpdateLanded, PlaceUpdateRequest, PlaceUpdater, PlaceWiring, RecipeResolver,
    NO_PLACE_DOOR, NO_PLACE_UPDATER, PLACES,
};
pub use wsp_store::keys::{new_place_key_pair, PlaceKeyPair};

/// The methods of the link half (places.ts:46). The TS spreads the manage half over it, so a name in both would be the
/// manage half's; none is.
pub const LINK_HALF: [&str; 33] = [
    "answer_challenge",
    "join",
    "auth",
    "host_key",
    "prove",
    "attach",
    "link",
    "channel",
    "watch_sys",
    "load",
    "backend_of",
    "folder_computer",
    "joined",
    "forking_backend",
    "name_of",
    "settings_at",
    "turn_limit_at",
    "threads_at",
    "machines_at",
    "sign_ins_at",
    "github_from_vault",
    "vault_at",
    "login_landed",
    "logins_again",
    "key_landed",
    "gh_there",
    "offer_of",
    "forward",
    "pane_forwards",
    "place_for",
    "default_place",
    "mark_used",
    "mark_default_if_none",
];

/// The methods of the manage half (places.ts:46).
pub const MANAGE_HALF: [&str; 28] = [
    "add",
    "set_up",
    "pending",
    "choose",
    "dial",
    "set",
    "road",
    "exec",
    "report_of",
    "home_of",
    "list",
    "rows",
    "adds",
    "update",
    "holds",
    "remove",
    "find",
    "follow",
    "recipe_changed",
    "followers",
    "skip",
    "setup_log",
    "estimate",
    "unfollow",
    "picks_of",
    "on",
    "off",
    "close",
];

/// What views and manage read off `ctx.door` (views.ts:126,178,236-238,410): a forwarder over the door the context
/// holds, so the areas built before the door still reach its link half.
pub struct DoorLinkReads {
    pub ctx: Rc<PlaceDoorContext>,
}

#[async_trait(?Send)]
impl LinkReads for DoorLinkReads {
    fn backend_of(&self, place_id: &str) -> Option<Arc<dyn MachineBackend>> {
        self.ctx.door().backend_of(place_id)
    }
    async fn forking_backend(&self, place_id: &str) -> PlaceResult<Arc<dyn MachineBackend>> {
        self.ctx.door().forking_backend(place_id).await
    }
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        self.ctx.door().folder_computer(place_id)
    }
}

/// The door `make_place_door` answers (places.ts:46): the link half and the manage half behind one `PlaceDoor`, each
/// method on the half that owns it.
pub struct AssembledDoor {
    pub ctx: Rc<PlaceDoorContext>,
    pub link: Rc<LinkDoor>,
    pub manage: Rc<ManageDoor>,
}

#[async_trait(?Send)]
impl PlaceDoor for AssembledDoor {
    /// The link half (places.ts:46).
    fn answer_challenge(
        &self,
        subject: &str,
        nonce: &str,
        ephemeral: Option<&str>,
    ) -> Option<PlaceChallenge> {
        self.link.answer_challenge(subject, nonce, ephemeral)
    }
    /// The link half (places.ts:46).
    async fn join(
        &self,
        req: PlaceJoinRequest,
        from: &str,
        now: u64,
    ) -> PlaceResult<Option<JoinAnswer>> {
        self.link.join(req, from, now).await.map(Some)
    }
    /// The link half (places.ts:46).
    async fn auth(&self, req: PlaceAuthRequest, _now: u64) -> PlaceResult<AuthAnswer> {
        self.link.auth(req).await
    }
    /// The link half (places.ts:46).
    fn host_key(&self) -> String {
        self.link.host_key()
    }
    /// The link half (places.ts:46).
    async fn prove(
        &self,
        place_id: &str,
        req: PlaceProveRequest,
        expect: &[u8],
        from: &str,
        now: u64,
    ) -> PlaceResult<Proved> {
        self.link.prove(place_id, req, expect, from, now).await
    }
    /// The link half (places.ts:46).
    async fn attach(
        &self,
        place_id: &str,
        socket: Rc<dyn LinkSocket>,
        report: PlaceReport,
        from: &str,
        now: u64,
        seal: Option<Seal>,
    ) -> PlaceResult<()> {
        self.link
            .attach(place_id, socket, report, from, now, seal)
            .await
    }
    /// The link half (places.ts:46).
    fn link(&self, place_id: &str) -> Option<Rc<dyn DaemonReach>> {
        self.link.link(place_id)
    }
    /// The link half (places.ts:46).
    fn channel(
        &self,
        place_id: &str,
        on_event: Rc<dyn Fn(serde_json::Map<String, serde_json::Value>)>,
    ) -> Option<Rc<dyn DaemonChannel>> {
        let on_event = Rc::new(move |frame: &serde_json::Map<String, serde_json::Value>| {
            on_event(frame.clone())
        });
        self.link
            .channel(place_id, on_event)
            .map(|channel| Rc::new(ChannelFace(channel)) as Rc<dyn DaemonChannel>)
    }
    /// The link half (places.ts:46).
    async fn watch_sys(
        &self,
        place_id: &str,
        f: Rc<dyn Fn(SysSample)>,
    ) -> PlaceResult<Box<dyn FnOnce()>> {
        let release = self
            .link
            .watch_sys(
                place_id,
                Rc::new(move |sample: &SysSample| f(sample.clone())),
            )
            .await?;
        Ok(Box::new(move || release()))
    }
    /// The link half (places.ts:46).
    async fn load(&self) -> PlaceResult<()> {
        self.link.load().await
    }
    /// The link half (places.ts:46).
    fn backend_of(&self, place_id: &str) -> Option<std::sync::Arc<dyn MachineBackend>> {
        self.link.backend_of(place_id)
    }
    /// The link half (places.ts:46).
    fn folder_computer(&self, place_id: &str) -> Option<FolderComputer> {
        self.link.folder_computer(place_id)
    }
    /// The link half (places.ts:46).
    fn joined(&self, place_id: &str) -> bool {
        self.link.joined(place_id)
    }
    /// The link half (places.ts:46).
    async fn forking_backend(
        &self,
        place_id: &str,
    ) -> PlaceResult<std::sync::Arc<dyn MachineBackend>> {
        self.link.forking_backend(place_id).await
    }
    /// The link half (places.ts:46).
    fn name_of(&self, place_id: &str) -> String {
        self.link.name_of(place_id)
    }
    /// The link half (places.ts:46).
    fn settings_at(&self, place_id: &str) -> PlaceSettings {
        self.link.settings_at(place_id)
    }
    /// The link half (places.ts:46).
    fn turn_limit_at(&self, place_id: &str) -> Option<Option<u64>> {
        self.link.turn_limit_at(place_id)
    }
    /// The link half (places.ts:46).
    fn threads_at(&self, place_id: &str) -> Option<u64> {
        self.link.threads_at(place_id)
    }
    /// The link half (places.ts:46).
    fn machines_at(&self, place_id: &str) -> Option<u64> {
        self.link.machines_at(place_id)
    }
    /// The link half (places.ts:46).
    fn sign_ins_at(
        &self,
        place_id: &str,
        agents: Option<&[String]>,
    ) -> Option<IndexMap<String, AgentSignInState>> {
        self.link.sign_ins_at(place_id, agents)
    }
    /// The link half (places.ts:46).
    fn github_from_vault(&self, place_id: &str) -> bool {
        self.link.github_from_vault(place_id)
    }
    /// The link half (places.ts:46).
    fn vault_at(
        &self,
        place_id: &str,
        vault: &IndexMap<String, String>,
    ) -> IndexMap<String, String> {
        self.link.vault_at(place_id, vault)
    }
    /// The link half (places.ts:46).
    async fn login_landed(&self, place_id: &str, agent: &str) -> PlaceResult<()> {
        self.link.login_landed(place_id, agent).await
    }
    /// The link half (places.ts:46).
    async fn logins_again(&self, place_id: &str) -> PlaceResult<()> {
        self.link.logins_again(place_id).await
    }
    /// The link half (places.ts:46).
    async fn key_landed(&self, agent: &str) -> PlaceResult<()> {
        self.link.key_landed(agent).await
    }
    /// The link half (places.ts:46).
    async fn gh_there(
        &self,
        place_id: &str,
        stage: ProvisionStage,
    ) -> PlaceResult<Vec<PlaceProvisionRow>> {
        self.link.gh_there(place_id, stage).await
    }
    /// The link half (places.ts:46).
    fn offer_of(&self, place_id: &str) -> Option<String> {
        self.link.offer_of(place_id)
    }
    /// The link half (places.ts:46).
    async fn forward(&self, place_id: &str, place_port: u16, o: ForwardAsk) -> PlaceResult<u16> {
        self.link.forward(place_id, place_port, o).await
    }
    /// The link half (places.ts:46).
    fn pane_forwards(&self) -> Rc<PanePorts> {
        self.link.pane_forwards()
    }
    /// The link half (places.ts:46).
    async fn place_for(&self, word: &str) -> PlaceResult<PlaceFor> {
        self.link.place_for(word).await
    }
    /// The link half (places.ts:46).
    async fn default_place(&self) -> PlaceResult<PlaceFor> {
        self.link.default_place().await
    }
    /// The link half (places.ts:46).
    async fn mark_used(&self, place_id: Option<&str>) -> PlaceResult<()> {
        self.link.mark_used(place_id).await
    }
    /// The link half (places.ts:46).
    async fn mark_default_if_none(&self, place_id: &str) -> PlaceResult<()> {
        self.link.mark_default_if_none(place_id).await
    }
    /// The manage half (places.ts:46).
    async fn add(&self, req: AddAsk, now: u64) -> PlaceResult<PlaceAdded> {
        self.manage.add(req, now).await
    }
    /// The manage half (places.ts:46).
    async fn set_up(&self, reference: &str, o: SetUpAsk) -> PlaceResult<PlaceSetUp> {
        self.manage.set_up(reference, o).await
    }
    /// The manage half (places.ts:46).
    async fn pending(&self) -> PlaceResult<Vec<PendingComputer>> {
        self.manage.pending().await
    }
    /// The manage half (places.ts:46).
    async fn choose(
        &self,
        reference: &str,
        choices: RecipeFile,
        recipe: Option<String>,
    ) -> PlaceResult<PendingComputer> {
        self.manage.choose(reference, choices, recipe).await
    }
    /// The manage half (places.ts:46).
    async fn dial(&self, place_id: &str, now: u64) -> PlaceResult<PlaceDial> {
        self.manage.dial(place_id, now).await
    }
    /// The manage half (places.ts:46).
    async fn set(
        &self,
        place_id: &str,
        set: PlaceSettingsAsk,
        reset: Vec<PlaceSettingWord>,
        also: PlaceSetAlso,
    ) -> PlaceResult<PlaceView> {
        self.manage.set(place_id, set, reset, also).await
    }
    /// The manage half (places.ts:46).
    async fn road(&self, place_id: &str) -> PlaceResult<u16> {
        self.manage.road(place_id).await
    }
    /// The manage half (places.ts:46).
    async fn exec(
        &self,
        place_id: &str,
        cmd: &str,
        opts: PlaceExecOpts,
    ) -> PlaceResult<ExecResult> {
        self.manage.exec(place_id, cmd, opts).await
    }
    /// The manage half (places.ts:46).
    async fn report_of(&self, place_id: &str) -> PlaceResult<Option<PlaceReport>> {
        self.manage.report_of(place_id).await
    }
    /// The manage half (places.ts:46).
    async fn home_of(&self, place_id: &str) -> PlaceResult<Option<String>> {
        self.manage.home_of(place_id).await
    }
    /// The manage half (places.ts:46).
    async fn list(&self, now: u64) -> PlaceResult<Vec<PlaceView>> {
        self.manage.list(now).await
    }
    /// The manage half (places.ts:46).
    async fn rows(&self) -> PlaceResult<Vec<PlaceView>> {
        self.manage.rows().await
    }
    /// The manage half (places.ts:46).
    fn adds(&self) -> Vec<PlaceAddJob> {
        self.manage.adds()
    }
    /// The manage half (places.ts:46).
    async fn update(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceUpdateReply> {
        self.manage.update(place_id, ask).await
    }
    /// The manage half (places.ts:46).
    async fn holds(&self, place_id: &str) -> PlaceResult<PlaceHolds> {
        self.manage.holds(place_id).await
    }
    /// The manage half (places.ts:46).
    async fn remove(&self, place_id: &str, ask: RemoveAsk) -> PlaceResult<PlaceRemoved> {
        self.manage.remove(place_id, ask).await
    }
    /// The manage half (places.ts:46).
    async fn find(&self, reference: &str) -> PlaceResult<Vec<PlaceRecord>> {
        self.manage.find(reference).await
    }
    /// The manage half (places.ts:46).
    async fn follow(&self, place_id: &str, recipe: &str) -> PlaceResult<PlaceView> {
        self.manage.follow(place_id, recipe).await
    }
    /// The manage half (places.ts:46).
    async fn recipe_changed(&self, slug: &str, after_ms: u64) -> PlaceResult<Vec<String>> {
        self.manage.recipe_changed(slug, after_ms).await
    }
    /// The manage half (places.ts:46).
    async fn followers(&self) -> PlaceResult<IndexMap<String, Vec<String>>> {
        self.manage.followers().await
    }
    /// The manage half (places.ts:46).
    async fn skip(&self, place_id: &str, row: &str) -> PlaceResult<PlaceView> {
        self.manage.skip(place_id, row).await
    }
    /// The manage half (places.ts:46).
    async fn setup_log(
        &self,
        place_id: &str,
        step: Option<PlaceSetupStep>,
    ) -> PlaceResult<Vec<String>> {
        self.manage.setup_log(place_id, step).await
    }
    /// The manage half (places.ts:46).
    async fn estimate(&self, reference: &str, choices: RecipeFile) -> PlaceResult<PlaceEstimate> {
        self.manage.estimate(reference, choices).await
    }
    /// The manage half (places.ts:46).
    async fn unfollow(&self, slug: &str) -> PlaceResult<Vec<String>> {
        self.manage.unfollow(slug).await
    }
    /// The manage half (places.ts:46).
    async fn picks_of(&self, place_id: &str) -> PlaceResult<Option<RecipeFile>> {
        self.manage.picks_of(place_id).await
    }
    /// The manage half (places.ts:46).
    fn on(&self, f: Rc<dyn Fn(&PlaceEvent)>) -> u64 {
        self.manage.on(f)
    }
    /// The manage half (places.ts:46).
    fn off(&self, id: u64) {
        self.manage.off(id)
    }
    /// The manage half (places.ts:46).
    async fn close(&self) -> PlaceResult<()> {
        self.manage.close().await
    }
}

/// `makePlaceDoor` (places.ts:38): [`assemble_place_door`], read as the `PlaceDoor` every caller holds.
pub fn make_place_door(opts: PlaceDoorOptions) -> Rc<dyn PlaceDoor> {
    assemble_place_door(opts)
}

/// The assembly itself (places.ts:39-46): the context, then the areas in the TS order (records, setup, views, then the
/// link and manage halves), then the door set on the context so an area that reads `ctx.door` finds it. Views and
/// manage are handed [`DoorLinkReads`] for their `LinkReads` and the setup area for `ViewsSetup` and `ManageSetup`.
pub fn assemble_place_door(opts: PlaceDoorOptions) -> Rc<AssembledDoor> {
    let ctx = place_door_context(opts);
    let records = PlaceRecordsArea::new(Rc::clone(&ctx));
    let setup = PlaceSetupArea::new(Rc::clone(&ctx), Rc::clone(&records) as Rc<dyn SetupRecords>);
    let links: Rc<dyn LinkReads> = Rc::new(DoorLinkReads {
        ctx: Rc::clone(&ctx),
    });
    let views_setup: Rc<dyn ViewsSetup> = Rc::new(SetupFace(Rc::clone(&setup)));
    let manage_setup: Rc<dyn ManageSetup> = Rc::new(SetupFace(Rc::clone(&setup)));
    let views = PlaceViewsArea::new(
        Rc::clone(&ctx),
        Rc::clone(&records),
        views_setup,
        Rc::clone(&links),
    );
    let link = LinkDoor::new(
        Rc::clone(&ctx),
        Rc::clone(&records) as Rc<dyn LinkRecords>,
        Rc::clone(&setup),
        Rc::clone(&views) as Rc<dyn LinkViews>,
    );
    let manage = ManageDoor::new(
        Rc::clone(&ctx),
        Rc::clone(&records),
        manage_setup,
        Rc::clone(&views),
        links,
    );
    let door = Rc::new(AssembledDoor {
        ctx: Rc::clone(&ctx),
        link,
        manage,
    });
    let held = Rc::downgrade(&door);
    ctx.set_door(held);
    door
}

/// The setup area as the views and manage areas hold it. The area's own methods that start a task or take a road
/// need the `Rc` the area lives in, which a `&self` trait method does not have, so the handle carries it.
struct SetupFace(Rc<PlaceSetupArea>);

impl ViewsSetup for SetupFace {
    fn woken(&self, place_id: &str, back: bool) {
        self.0.woken(place_id, back);
    }
    fn forget_closed_at(&self, place_id: &str) {
        self.0.closed_at.borrow_mut().shift_remove(place_id);
    }
    fn link_to(&self, place_id: &str) -> Arc<dyn MachineLink> {
        self.0.link_to(place_id)
    }
}

fn setup_started(started: Started) -> SetupStarted {
    SetupStarted {
        setup: started.setup,
        said: started.said,
    }
}

#[async_trait(?Send)]
impl ManageSetup for SetupFace {
    fn waiting_ids(&self) -> Vec<String> {
        self.0.waiting.borrow().keys().cloned().collect()
    }
    fn is_setting(&self, place_id: &str) -> bool {
        self.0.setting.borrow().contains(place_id)
    }
    fn is_syncing(&self, place_id: &str) -> bool {
        self.0.syncing.borrow().contains(place_id)
    }
    fn skipper(&self, key: &str) -> Option<Rc<dyn Fn() -> LocalFut<'static, PlaceResult<()>>>> {
        let skip = self.0.skippers.borrow().get(key).cloned()?;
        Some(Rc::new(move || -> LocalFut<'static, PlaceResult<()>> {
            let run = skip();
            Box::pin(async move {
                run.await;
                Ok(())
            })
        }))
    }
    fn setting_now(&self, record: &PlaceRecord) -> Option<String> {
        self.0.setting_now(record)
    }
    fn recipes_moved(&self, slug: &str) {
        self.0.recipes_moved(slug);
    }
    fn sync_frame(&self, event: PlaceSyncEvent) {
        self.0.sync_frame(event);
    }
    fn setup_frame(&self, event: PlaceSetupEvent) {
        self.0.setup_frame(event);
    }
    async fn start_setup(
        &self,
        place_id: &str,
        add_id: &str,
        picks: Option<SetupPicks>,
        row: Option<String>,
    ) -> PlaceResult<SetupStarted> {
        let given = picks.map(|p| GivenPicks {
            picks: p.picks,
            recipe: p.recipe,
        });
        Ok(setup_started(
            self.0
                .start_setup(place_id, add_id, given, row.as_deref())
                .await?,
        ))
    }
    async fn started_or_said(
        &self,
        place_id: &str,
        add_id: &str,
        picks: SetupPicks,
    ) -> PlaceResult<SetupStarted> {
        let given = GivenPicks {
            picks: picks.picks,
            recipe: picks.recipe,
        };
        Ok(setup_started(
            self.0.started_or_said(place_id, add_id, Some(given)).await,
        ))
    }
    fn clear_sync_timers(&self) {
        let timers: Vec<_> = self.0.sync_timers.borrow_mut().drain(..).collect();
        for (_, cancel) in timers {
            cancel();
        }
    }
    async fn sync_of(
        &self,
        record: &PlaceRecord,
        resolved: Option<&Resolved>,
    ) -> PlaceResult<Option<ManageSyncRead>> {
        let read = self.0.sync_of(record, resolved.cloned()).await;
        Ok(read.map(|read| ManageSyncRead {
            change_keys: read
                .changes
                .iter()
                .map(|change| change.key.clone())
                .collect(),
        }))
    }
    async fn mark_sync(&self, place_id: &str, sync: PlaceSync) -> PlaceResult<()> {
        self.0.mark_sync(place_id, Some(sync)).await
    }
    fn sync_soon(&self, place_id: &str, after_ms: u64) {
        self.0.sync_soon(place_id, after_ms);
    }
}

/// One open channel as the door hands it out; its methods take the `Rc` the channel lives in, which the handle carries.
struct ChannelFace(Rc<PlaceChannel>);

#[async_trait(?Send)]
impl DaemonChannel for ChannelFace {
    async fn send(&self, frame: Frame) -> Result<Frame, ChannelError> {
        let reply = self
            .0
            .send(frame)
            .await
            .map_err(|e| ChannelError::Refused { message: e.message })?;
        match serde_json::to_value(reply).map_err(|e| ChannelError::Refused {
            message: e.to_string(),
        })? {
            serde_json::Value::Object(said) => Ok(said),
            _ => Err(ChannelError::Refused {
                message: "the daemon's reply is not an object".to_string(),
            }),
        }
    }
    fn close(&self) {
        self.0.close();
    }
    async fn closed(&self) -> ChannelEnd {
        let gone = self.0.closed().await;
        ChannelEnd {
            code: gone.code,
            reason: gone.reason,
        }
    }
}
