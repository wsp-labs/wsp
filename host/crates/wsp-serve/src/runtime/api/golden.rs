// SPDX-License-Identifier: AGPL-3.0-only
use std::rc::Rc;
use wsp_core::types::api::{
    AccountOrphans, BuildPlace, CatalogAt, GoldenApi, HarnessesApi, OrphansDeleted, PlacedRetention, PrepareAsk, Pruned, Sealed, UpgradeAsk,
};

use wsp_core::status::{CostPoint, StatusApi, StatusListOptions, StatusWatchOptions};
use wsp_core::types::wiring::{GoldenBuildRequest, GoldenPromotion, GoldenUpgradeResult, OriginStatusApi, ReadingsTarget, UsageDoor};
use wsp_machines::images::engine::GoldenDelta;
use wsp_machines::images::golden::upgrade::UpgradeRequest;
use wsp_machines::images::ImageError;
use wsp_wire::types::usage::{AccountsAnswer, ReadingsRange, ResetAnswer, UsageRange, UsageSplit, UsedAnswer};
use wsp_wire::views::golden_image::{GoldenBuilderView, GoldenLogin, GoldenManifest, RecipeDigest};
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::views::place_more::{PlaceSpend, PlaceView};
use wsp_wire::views::snapshot_lineage::SnapshotStorage;
use wsp_wire::views::workspace::{Caller, DaemonReachView};
use wsp_wire::views::workspace_more::WorkspaceStatus;
use wsp_wire::wire::machine_link::ReadingsAnswer;
use wsp_words::exit::HostError;

use super::{GoldenApiOf, HarnessesApiOf, OriginStatusOf, UsageDoorOf};
use crate::runtime::public::{harnesses_list, status_history, status_list, HarnessesCtx};

#[async_trait::async_trait(?Send)]
impl GoldenApi for GoldenApiOf {
    async fn build(&self, opts: GoldenBuildRequest) -> Result<Sealed, HostError> {
        self.0.golden.get().build(opts).await.map_err(ImageError::into_host)
    }
    async fn get(&self, name: Option<String>) -> Result<Option<GoldenManifest>, HostError> {
        self.0.golden.get().get(name.as_deref()).await
    }
    async fn build_place(&self, word: Option<String>) -> Result<BuildPlace, HostError> {
        self.0.golden.get().build_place(word.as_deref()).await.map_err(ImageError::into_host)
    }
    async fn prepare(&self, ask: PrepareAsk) -> Result<GoldenBuilderView, HostError> {
        self.0.golden.get().prepare(ask).await.map_err(ImageError::into_host)
    }
    async fn prepare_at(&self, name: Option<String>, place: Option<String>) -> Result<(String, String), HostError> {
        let at = self.0.golden.get().prepare_at(name.as_deref(), place.as_deref()).await?;
        Ok((at.place, at.name))
    }
    async fn seal(&self, builder_id: &str, logins: Option<Vec<GoldenLogin>>, keep_builder: Option<bool>) -> Result<Sealed, HostError> {
        self.0.golden.get().seal(builder_id, logins, keep_builder).await.map_err(ImageError::into_host)
    }
    async fn recipe(&self, name: Option<String>) -> Result<Option<RecipeDigest>, HostError> {
        self.0.golden.get().recipe(name.as_deref()).await
    }
    // wsp-core's ask carries no delta (an empty placeholder), so the engine's default delta is what the road gets.
    async fn upgrade(&self, ask: UpgradeAsk) -> Result<GoldenUpgradeResult, HostError> {
        let request = UpgradeRequest {
            name: ask.name,
            delta: GoldenDelta::default(),
            keep_previous: ask.keep_previous,
            logins: ask.logins,
            recipe: ask.recipe,
        };
        self.0.golden.get().upgrade(request).await.map_err(ImageError::into_host)
    }
    async fn builder_reach(&self, builder_id: &str) -> Result<DaemonReachView, HostError> {
        self.0.golden.get().builder_reach(builder_id).await.map_err(ImageError::into_host)
    }
    async fn builders(&self) -> Result<Vec<GoldenBuilderView>, HostError> {
        self.0.golden.get().builders().await
    }
    async fn kill(&self, builder_id: &str) -> Result<(), HostError> {
        self.0.golden.get().kill(builder_id).await.map_err(ImageError::into_host)
    }
    async fn rollback(&self, version: u32, name: Option<String>) -> Result<GoldenManifest, HostError> {
        self.0.golden.get().rollback(version, name.as_deref()).await
    }
    async fn promote(&self, name: Option<String>) -> Result<Option<Vec<GoldenPromotion>>, HostError> {
        self.0.golden.get().promote(name.as_deref()).await
    }
    async fn storage(&self) -> Result<Option<SnapshotStorage>, HostError> {
        self.0.golden.get().storage().await.map_err(ImageError::into_host)
    }
    async fn orphans(&self) -> Result<Option<AccountOrphans>, HostError> {
        self.0.golden.get().orphans().await.map_err(ImageError::into_host)
    }
    async fn delete_orphans(&self) -> Result<Option<OrphansDeleted>, HostError> {
        self.0.golden.get().delete_orphans().await.map_err(ImageError::into_host)
    }
    async fn retention_places(&self, name: Option<String>) -> Result<Vec<String>, HostError> {
        self.0.golden.get().retention_places(name.as_deref()).await
    }
    async fn retention(&self, name: Option<String>, place: Option<String>) -> Result<Option<PlacedRetention>, HostError> {
        self.0.golden.get().retention(name.as_deref(), place.as_deref()).await.map_err(ImageError::into_host)
    }
    async fn prune(&self, name: Option<String>, place: Option<String>) -> Result<Pruned, HostError> {
        self.0.golden.get().prune(name.as_deref(), place.as_deref()).await.map_err(ImageError::into_host)
    }
}

#[async_trait::async_trait(?Send)]
impl UsageDoor for UsageDoorOf {
    async fn used(&self, range: UsageRange, split: UsageSplit, outside: Option<bool>) -> Result<UsedAnswer, HostError> {
        self.0.usage.get().used(range, split, outside).await.map_err(HostError::from)
    }
    async fn accounts(&self, fresh: Option<bool>) -> Result<AccountsAnswer, HostError> {
        self.0.usage.get().accounts(fresh == Some(true)).await.map_err(HostError::from)
    }
    async fn readings(&self, target: ReadingsTarget, range: ReadingsRange, origin: Option<Caller>) -> Result<ReadingsAnswer, HostError> {
        self.0.usage.get().readings(target, range, origin.as_ref()).await.map_err(HostError::from)
    }
    async fn reset(&self, account: String, credit_id: Option<String>, on: Option<String>) -> Result<ResetAnswer, HostError> {
        self.0.usage.get().reset(&account, credit_id.as_deref(), on.as_deref()).await
    }
}

#[async_trait::async_trait(?Send)]
impl HarnessesApi for HarnessesApiOf {
    async fn list(
        &self,
        workspace_id: Option<String>,
        origin: Option<Caller>,
        at: Option<CatalogAt>,
    ) -> Result<Vec<HarnessCatalog>, HostError> {
        let rows = harnesses_list(&*self.0 as &dyn HarnessesCtx, workspace_id, origin, at).await?;
        rows.into_iter()
            .map(|row| serde_json::from_value::<HarnessCatalog>(serde_json::Value::Object(row)).map_err(|e| HostError::new(e.to_string())))
            .collect()
    }
}

#[async_trait::async_trait(?Send)]
impl StatusApi for OriginStatusOf {
    async fn list(&self, opts: StatusListOptions) -> Result<Vec<WorkspaceStatus>, String> {
        self.0.status_tracker.get().list(opts).await
    }
    fn watch(&self, opts: StatusWatchOptions) -> Rc<dyn Fn()> {
        self.0.status_tracker.get().watch(opts)
    }
    fn watched(&self) -> bool {
        self.0.status_tracker.get().watched()
    }
    async fn poll(&self, ids: &[String]) {
        self.0.status_tracker.get().poll(ids).await
    }
    async fn history(&self, workspace_id: &str) -> Vec<CostPoint> {
        self.0.status_tracker.get().history(workspace_id).await
    }
    async fn spend(&self, places: &[PlaceView], at: Option<i64>) -> Result<Vec<PlaceSpend>, String> {
        self.0.status_tracker.get().spend(places, at).await
    }
}

#[async_trait::async_trait(?Send)]
impl OriginStatusApi for OriginStatusOf {
    async fn list(&self, opts: Option<StatusListOptions>, origin: Option<Caller>) -> Result<Vec<WorkspaceStatus>, HostError> {
        status_list(&*self.0, opts, origin).await
    }
    async fn history(&self, workspace_id: &str, origin: Option<Caller>) -> Result<Vec<CostPoint>, HostError> {
        let rows = status_history(&*self.0, workspace_id, origin).await?;
        rows.into_iter().map(|row| serde_json::from_value(row).map_err(|e| HostError::new(e.to_string()))).collect()
    }
}
