// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:292-459 at 65f6b89c7: upgrade, builderReach, builders and kill. Spec: `spec/golden_upgrade.md`.
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use wsp_core::types::internal::{BuilderLife, BUILDERS};
use wsp_core::types::wiring::{GoldenRecipe, GoldenUpgradeResult, UpgradeRoad};
use wsp_engine::errors::is_missing;
use wsp_engine::labels::{BUILDER_LABEL, OWNER_LABEL, SMOKE_LABEL};
use wsp_engine::machine::{ExecOptions, Machine, MachineBackend};
use wsp_engine::preview::DAEMON_PORT;
use wsp_wire::views::golden_image::{
    GoldenBuilderView, GoldenLogin, GoldenManifest, GoldenStage, GoldenVersion,
};
use wsp_wire::views::workspace::DaemonReachView;
use wsp_words::exit::HostError;

use super::super::builders::Claim;
use super::super::engine::{ApplyDeltaAsk, GoldenDelta, MachineSize, StageFn, UpgradeBuilderAsk};
use super::super::{BuilderRef, ImageError, ImageResult};
use super::GoldenArea;

/// api.ts:616 `golden.upgrade`'s ask. wsp-core's `api::UpgradeAsk` carries the empty `pending::GoldenDelta`; this one
/// carries the engine's delta, whose `import` and `retiredOnImage` the update reads.
#[derive(Clone, Default)]
pub struct UpgradeRequest {
    pub name: Option<String>,
    pub delta: GoldenDelta,
    pub keep_previous: Option<bool>,
    pub logins: Option<Vec<GoldenLogin>>,
    pub recipe: Option<GoldenRecipe>,
}

impl GoldenArea {
    /// golden.ts:292-415 (U1 to U8).
    pub async fn upgrade(self: &Rc<Self>, o: UpgradeRequest) -> ImageResult<GoldenUpgradeResult> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        let recipe = ctx.recipe_or_throw(o.recipe.clone())?;
        let name = o.name.clone().unwrap_or_else(|| "default".to_string());
        let place = ctx.image_place(&name).await?;
        let prior = ctx.copy_of(&place, &name).await?;
        let Some(head) = wsp_wire::views::golden_image::golden_head(prior.as_ref()).cloned() else {
            return Err(HostError::new(format!("no golden named \"{name}\" to update")).into());
        };
        let at = ctx.backend_at(&place)?;
        let stage = ctx.stage_of(&name, Some(&place), false);

        let swept = ctx.expire_grace().await?;
        for f in &swept.failed {
            let id = f.id.as_deref().unwrap_or("undefined");
            stage(
                GoldenStage::Creating,
                Some(format!("an earlier kept builder {id}: {}", f.message)),
                None,
                None,
            );
        }
        ctx.refresh_builders().await?;

        let kept = {
            let list: Vec<BuilderRef> = ctx.builders().borrow().values().cloned().collect();
            list.into_iter().find(|x| {
                let b = x.borrow();
                let sealed_here = b.record.sealed.as_ref().is_some_and(|s| {
                    f64::from(s.version) == head.version.0 && ctx.in_window(&s.at)
                });
                matches!(b.life, BuilderLife::Own | BuilderLife::Reusable)
                    && b.record.name == name
                    && b.record.place.as_deref().unwrap_or(place.as_str()) == place.as_str()
                    && sealed_here
            })
        };

        let mut entry: Option<BuilderRef> = None;
        if let Some(kept) = kept {
            let timer = ctx
                .grace_timers()
                .borrow_mut()
                .shift_remove(&kept.borrow().record.id);
            if let Some(cancel) = timer {
                cancel();
            }
            let machine = kept.borrow().builder.machine.clone();
            {
                let mut b = kept.borrow_mut();
                b.record.sealed = None;
                b.record.building = Some(true);
            }
            put_record(&*ctx, &kept).await?;
            let alive = match machine.exec("true", ExecOptions::default()).await {
                Ok(r) => r.exit_code.get() == 0,
                Err(_) => false,
            };
            if !alive {
                ctx.roads()
                    .kill_until_gone(at.clone(), machine, ctx.kill_confirm())
                    .await
                    .map_err(ImageError::Engine)?;
                let id = kept.borrow().record.id.clone();
                ctx.forget_builder(&id).await?;
            } else {
                stage(
                    GoldenStage::Creating,
                    Some(format!(
                        "your builder from v{}, kept since the save",
                        head.version.0
                    )),
                    None,
                    None,
                );
                match apply_kept(
                    &*ctx,
                    &kept,
                    &machine,
                    &o.delta,
                    &head,
                    &recipe.setup,
                    &stage,
                )
                .await
                {
                    Ok(()) => {
                        stage(GoldenStage::Ready, None, None, None);
                        entry = Some(kept);
                    }
                    Err(e) => {
                        let mut detail = e.message();
                        if let Err(k) = ctx
                            .roads()
                            .kill_until_gone(at.clone(), machine, ctx.kill_confirm())
                            .await
                        {
                            detail.push_str(&format!("; {k}"));
                        }
                        ctx.forget_if_gone(&kept, at.clone()).await?;
                        stage(GoldenStage::Failed, Some(detail), None, None);
                        return Err(e);
                    }
                }
            }
        }

        let road = if entry.is_some() {
            UpgradeRoad::Builder
        } else {
            UpgradeRoad::Fork
        };
        let forked = matches!(road, UpgradeRoad::Fork);
        let entry = match entry {
            Some(entry) => entry,
            None => fork(&ctx, &name, &place, &at, &o.delta, &recipe, &head, &stage).await?,
        };
        entry.borrow_mut().recipe = Some(recipe.clone());

        let sealed = ctx.seal_entry(&entry, true, o.logins.clone(), None).await?;
        let mut manifest = sealed.manifest;
        let mut previous_dropped = false;
        let mut builder_kept = sealed.builder_kept;
        let standing = ctx.forked_from(&head.snapshot_id);
        if o.keep_previous == Some(false) {
            if !standing.is_empty() {
                ctx.warn(&format!(
                    "golden {name} v{} kept: {} still on it",
                    head.version.0,
                    standing.join(", ")
                ));
            } else {
                if forked && builder_kept {
                    let id = entry.borrow().record.id.clone();
                    let timer = ctx.grace_timers().borrow_mut().shift_remove(&id);
                    if let Some(cancel) = timer {
                        cancel();
                    }
                    let machine = entry.borrow().builder.machine.clone();
                    ctx.roads()
                        .kill_until_gone(at.clone(), machine, ctx.kill_confirm())
                        .await
                        .map_err(ImageError::Engine)?;
                    ctx.forget_builder(&id).await?;
                    builder_kept = false;
                }
                match drop_head(&*ctx, &name, &place, &at, &head, &manifest).await {
                    Ok(next) => {
                        manifest = next;
                        previous_dropped = true;
                    }
                    Err(e) => ctx.warn(&format!(
                        "golden {name} v{} kept: its snapshot was not deleted ({})",
                        head.version.0,
                        e.message()
                    )),
                }
            }
        }
        Ok(GoldenUpgradeResult {
            manifest,
            version: sealed.version,
            road,
            previous_dropped,
            builder_kept,
        })
    }

    /// golden.ts:418-428 (U9).
    pub async fn builder_reach(&self, builder_id: &str) -> ImageResult<DaemonReachView> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        ctx.refresh_builders().await?;
        let entry = find_builder(&*ctx, builder_id)?;
        ctx.refuse_untouchable(&entry)?;
        let machine = entry.borrow().builder.machine.clone();
        let held = entry.borrow().reach.clone();
        let reach = ctx
            .roads()
            .refresh_preview_token(machine.clone(), DAEMON_PORT, held)
            .await
            .map_err(ImageError::Engine)?;
        entry.borrow_mut().reach = Some(reach.clone());
        let daemon_token = ctx.daemon_token_of(machine).await?;
        Ok(DaemonReachView {
            url: reach.url,
            expires_at: reach.expires_at,
            daemon_token,
        })
    }

    /// golden.ts:430-434 (U10).
    pub async fn builders(&self) -> Result<Vec<GoldenBuilderView>, HostError> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        ctx.refresh_builders().await?;
        let held: Vec<BuilderRef> = ctx.builders().borrow().values().cloned().collect();
        Ok(held.iter().map(|b| ctx.builder_view(b)).collect())
    }

    /// golden.ts:436-459 (U11, U12).
    pub async fn kill(&self, builder_id: &str) -> ImageResult<()> {
        let ctx = self.ctx.clone();
        ctx.ready().await?;
        ctx.refresh_builders().await?;
        let found = ctx.builders().borrow().get(builder_id).cloned();
        let Some(entry) = found else {
            return kill_unrecorded(&*ctx, builder_id).await;
        };
        if matches!(
            entry.borrow().life,
            BuilderLife::Foreign | BuilderLife::Held
        ) {
            ctx.refuse_untouchable(&entry)?;
        }
        let place = entry
            .borrow()
            .record
            .place
            .clone()
            .unwrap_or_else(|| ctx.places().wired());
        let at = ctx.backend_at(&place)?;
        let machine = entry.borrow().builder.machine.clone();
        ctx.roads()
            .kill_until_gone(at, machine, ctx.kill_confirm())
            .await
            .map_err(ImageError::Engine)?;
        ctx.forget_builder(builder_id).await?;
        Ok(())
    }
}

fn find_builder(ctx: &dyn super::GoldenCtx, builder_id: &str) -> Result<BuilderRef, HostError> {
    let found = ctx.builders().borrow().get(builder_id).cloned();
    found.ok_or_else(|| HostError::new(format!("no such builder: {builder_id}")))
}

/// golden.ts:444-454: a machine no builder record claims is killed only where it wears this state's builder or smoke label.
async fn kill_unrecorded(ctx: &dyn super::GoldenCtx, builder_id: &str) -> ImageResult<()> {
    let backend: Arc<dyn MachineBackend> = ctx.backend();
    let machine = match backend.get(builder_id).await {
        Ok(machine) => Some(machine),
        Err(e) if is_missing(&e) => None,
        Err(e) => return Err(ImageError::Engine(e)),
    };
    let owner = ctx.owner();
    let Some(machine) = machine else {
        return Err(HostError::new(format!("no such builder: {builder_id}")).into());
    };
    let mine = machine.labels().is_some_and(|l| {
        l.get(OWNER_LABEL) == Some(&owner)
            && (l.get(BUILDER_LABEL).map(String::as_str) == Some("1")
                || l.get(SMOKE_LABEL).map(String::as_str) == Some("1"))
    });
    if !mine {
        return Err(HostError::new(format!("no such builder: {builder_id}")).into());
    }
    ctx.roads()
        .kill_until_gone(backend, machine, ctx.kill_confirm())
        .await
        .map_err(ImageError::Engine)
}

async fn put_record(ctx: &dyn super::GoldenCtx, entry: &BuilderRef) -> Result<(), HostError> {
    let record = entry.borrow().record.clone();
    let value = serde_json::to_value(&record).map_err(|e| HostError::new(e.to_string()))?;
    ctx.store().put(BUILDERS, &record.id, value).await
}

/// golden.ts:327-351: the kept builder takes the delta and is held again as the head's successor.
async fn apply_kept(
    ctx: &dyn super::GoldenCtx,
    kept: &BuilderRef,
    machine: &Arc<dyn Machine>,
    delta: &GoldenDelta,
    head: &GoldenVersion,
    setup: &str,
    stage: &StageFn,
) -> ImageResult<()> {
    let applied = ctx
        .roads()
        .apply_delta(
            machine.clone(),
            delta.clone(),
            ApplyDeltaAsk {
                setup: setup.to_string(),
                previous_smoke: head.smoke.cmd.clone(),
                previous_base: head.base.clone(),
                previous_missing: head.missing_tools.clone(),
                previous_left_behind: head.left_behind.clone(),
                on_stage: stage.clone(),
            },
        )
        .await?;
    let setup_sha = ctx
        .roads()
        .next_setup_sha(&head.setup_sha, setup, &delta.import);
    {
        let mut b = kept.borrow_mut();
        b.record.import = Some(applied.ledger.clone());
        b.record.setup_sha = setup_sha.clone();
        b.record.parent_snapshot_id = Some(head.snapshot_id.clone());
        b.record.retired = Some(delta.retired_on_image.clone());
        b.record.building = None;
        b.builder.import = Some(applied.ledger);
        b.builder.setup_sha = setup_sha;
        b.builder.parent_snapshot_id = Some(head.snapshot_id.clone());
        b.builder.retired = Some(delta.retired_on_image.clone());
        b.life = BuilderLife::Own;
    }
    ctx.hold(kept).await?;
    Ok(())
}

/// golden.ts:352-379 (U5): a fork of the head on the bare builder key, at the place the image stands.
#[allow(clippy::too_many_arguments)]
async fn fork(
    ctx: &Rc<dyn super::GoldenCtx>,
    name: &str,
    place: &str,
    at: &Arc<dyn MachineBackend>,
    delta: &GoldenDelta,
    recipe: &GoldenRecipe,
    head: &GoldenVersion,
    stage: &StageFn,
) -> ImageResult<BuilderRef> {
    let claim: Rc<dyn Claim> =
        ctx.claiming(&format!("builder/{name}"), Some(at.clone()), Some(place));
    let backend = claim.backend();
    let placeholder: Rc<RefCell<Option<BuilderRef>>> = Rc::new(RefCell::new(None));
    let slot = placeholder.clone();
    let made: Rc<dyn Fn(BuilderRef)> = Rc::new(move |p: BuilderRef| *slot.borrow_mut() = Some(p));
    let recording = ctx.recording_creates(
        backend,
        name,
        Some(ctx.import_mark(&delta.import)),
        made,
        None,
        place,
    );
    let size = MachineSize {
        cpu: recipe.cpu,
        mem_mb: recipe.mem_mb,
        envs: recipe.envs.clone(),
        labels: Some(ctx.builder_labels(recipe.labels.as_ref())),
    };
    let built = ctx
        .roads()
        .upgrade_builder(UpgradeBuilderAsk {
            backend: recording,
            head: head.clone(),
            delta: delta.clone(),
            setup: recipe.setup.clone(),
            size,
            on_stage: stage.clone(),
        })
        .await;
    let builder = match built {
        Ok(builder) => builder,
        Err(e) => {
            let gone = placeholder.borrow().clone();
            if let Some(gone) = gone {
                ctx.forget_if_gone(&gone, at.clone()).await?;
            }
            return Err(e);
        }
    };
    let placed = placeholder.borrow().clone();
    let entry = ctx.settle_builder(name, builder, placed, place).await?;
    drop(claim);
    Ok(entry)
}

/// golden.ts:389-413 (U7): drops the head's version and its recipe, and says so when the snapshot will not go.
async fn drop_head(
    ctx: &dyn super::GoldenCtx,
    name: &str,
    place: &str,
    at: &Arc<dyn MachineBackend>,
    head: &GoldenVersion,
    manifest: &GoldenManifest,
) -> ImageResult<GoldenManifest> {
    ctx.drop_image(head, Some(at.clone())).await?;
    let next = GoldenManifest {
        versions: manifest
            .versions
            .iter()
            .filter(|v| v.version.0 != head.version.0)
            .cloned()
            .collect(),
        ..manifest.clone()
    };
    ctx.put_copy(place, name, &next).await?;
    ctx.drop_copy_recipe(place, name, head.version.0 as u32)
        .await?;
    Ok(next)
}
