// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:1322-1426: the recipe delta on a fork of the golden, or on the kept builder.
use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;

use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{golden_image, GoldenBaseTool, GoldenStage, MachineKind, MCP_ID_PREFIX};
use wsp_wire::wire::machine_link::MachineSpecOnIdle;
use wsp_words::words::base::plural;

use super::prepare::{iso_now, size_asked, size_built, spec_of};
use super::smoke::{manager_check_of, next_left_behind, next_missing, next_setup_sha, next_smoke, AGENT_INSTALLERS};
use super::types::{
    plain, Applied, ApplyDeltaOptions, ApplyImportOptions, Builder, GoldenDelta, GoldenError, Staged, UpgradeBuilderOptions,
};
use crate::errors::{EngineError, RetryClock};
use crate::golden::apply_golden_import;
use crate::golden_seam::{GoldenImport, BUILDER_IDLE_MS};
use crate::gone::{kill_until_gone, KillConfirm};
use crate::machine::Machine;
use wsp_wire::views::golden_image::ImageSpec;
use wsp_words::words::init::machine_left_line;

/// golden-diff.ts:56 `rungOf`: a row's rung is the first segment of its id (`id.slice(0, id.indexOf("/"))`, which drops
/// the last character of an id with no slash).
fn rung_of(id: &str) -> String {
    match id.find('/') {
        Some(i) => id[..i].to_string(),
        None => {
            let mut chars: Vec<char> = id.chars().collect();
            chars.pop();
            chars.into_iter().collect()
        }
    }
}

/// golden-diff.ts:57 `nameOf`: the last segment of an id.
fn name_of(id: &str) -> &str {
    &id[id.rfind('/').map_or(0, |i| i + 1)..]
}

/// golden.ts:1261 `refusePreFloor`: a version without the base tools never ran them, so a delta on it would report
/// covered rows it does not have.
fn refuse_pre_floor(base: &Option<Vec<GoldenBaseTool>>, which: &str) -> Result<(), EngineError> {
    match base {
        Some(_) => Ok(()),
        None => Err(plain(format!("{which} was sealed before the base tools existed and cannot take an update; seal it again"))),
    }
}

/// golden.ts:1326. Runs the delta through the import stages. Nothing is taken off the machine: a row the recipe
/// dropped keeps its bytes and rides the version's retired list, so the update only ever adds. The files and upload fail
/// the update as they fail a build; each tool and agent fails alone.
pub async fn apply_delta(
    machine: &Arc<dyn Machine>,
    delta: GoldenDelta,
    opts: ApplyDeltaOptions,
    clock: &dyn RetryClock,
) -> Result<Applied, EngineError> {
    refuse_pre_floor(&opts.previous_base, "this golden")?;
    let stage = super::types::Stage::new(opts.on_stage.clone());
    let retired = delta.retired.clone();
    if !retired.is_empty() {
        let names: Vec<&str> = retired.iter().map(|r| r.name.as_str()).collect();
        stage.say(
            GoldenStage::ApplyingSetup,
            &format!("{} left on the image, retired: {}", plural(retired.len() as f64, "row"), names.join(", ")),
        );
    }
    // The stages report only when one of them ran; a delta that only retires rows still has them to report.
    let report = delta.import.on_result.clone();
    let reported = Rc::new(Cell::new(false));
    let mut imp: GoldenImport = delta.import.clone();
    if let Some(report) = &report {
        let (flag, inner) = (reported.clone(), report.clone());
        imp.on_result = Some(Arc::new(move |r| {
            flag.set(true);
            inner(r);
        }));
    }
    let mut applied = apply_golden_import(
        machine,
        ApplyImportOptions {
            import: Some(imp),
            setup: opts.setup.clone(),
            base: None,
            ledger: None,
            retired: (!retired.is_empty()).then(|| retired.clone()),
            fetch: opts.fetch.clone(),
            on_stage: opts.on_stage.clone(),
        },
        clock,
    )
    .await?;
    if !reported.get() && !retired.is_empty() {
        if let Some(report) = &report {
            report(&applied.result);
        }
    }
    // A retired agent or tool is still on the image, but the recipe stopped asking for it, so its check leaves the
    // smoke: the next version is only ever held to what its own recipe claims. The rung decides, never the id's last
    // segment: a tools row may end in an agent's name and must not take that agent's check with it.
    let gone: Vec<String> = retired
        .iter()
        .flat_map(|r| {
            if rung_of(&r.id) == "tools" {
                return manager_check_of(&r.id).into_iter().collect::<Vec<_>>();
            }
            let installer =
                if rung_of(&r.id) == "agents" && !r.id.starts_with(MCP_ID_PREFIX) { AGENT_INSTALLERS.get(name_of(&r.id)) } else { None };
            installer.map(|i| i.smoke.clone()).into_iter().collect()
        })
        .collect();
    applied.ledger.smoke = next_smoke(&opts.previous_smoke, &gone, &applied.ledger.smoke);
    let missing =
        next_missing(opts.previous_missing.as_deref().unwrap_or(&[]), &delta, applied.ledger.missing_tools.as_deref().unwrap_or(&[]));
    applied.ledger.missing_tools = (!missing.is_empty()).then_some(missing);
    let left =
        next_left_behind(opts.previous_left_behind.as_deref().unwrap_or(&[]), &delta, applied.ledger.left_behind.as_deref().unwrap_or(&[]));
    applied.ledger.left_behind = (!left.is_empty()).then_some(left);
    Ok(applied)
}

/// golden.ts:1375. Forks the golden into a fresh first-life machine and applies the delta on it: a builder to seal as
/// the next version. The fork carries the daemon and everything the recipe already put there, so nothing but the delta
/// runs.
pub async fn upgrade_builder(opts: UpgradeBuilderOptions, clock: &dyn RetryClock) -> Result<Builder, GoldenError> {
    let version_text = wsp_words::js::number_string(opts.head.version.0);
    refuse_pre_floor(&opts.head.base, &format!("golden v{version_text}"))?;
    let staged = Staged::new(opts.on_stage.clone());
    let stage = staged.stage.clone();
    let kind = opts.head.kind.unwrap_or(MachineKind::Sandbox);
    stage.say(GoldenStage::Creating, &format!("fork of golden v{version_text}"));
    let asked = size_asked(&*opts.backend, &opts.size, opts.head.size.as_ref());
    let created_at = iso_now(clock);
    let mut spec = spec_of(&*opts.backend, kind, asked, &opts.size);
    match golden_image(&opts.head).spec {
        ImageSpec::Template(t) => spec.template = Some(t),
        ImageSpec::FromSnapshot(s) => spec.from_snapshot = Some(s),
    }
    spec.on_idle = Some(MachineSpecOnIdle::Kill);
    spec.idle_timeout_ms = Some(JsNumber(BUILDER_IDLE_MS as f64));
    let machine = opts.backend.create(spec, None, None).await?;
    let attempt = async {
        let size = size_built(&*machine, asked).await;
        let applied = apply_delta(
            &machine,
            opts.delta.clone(),
            ApplyDeltaOptions {
                setup: opts.setup.clone(),
                previous_smoke: opts.head.smoke.cmd.clone(),
                previous_base: opts.head.base.clone(),
                previous_missing: opts.head.missing_tools.clone(),
                previous_left_behind: opts.head.left_behind.clone(),
                fetch: opts.fetch.clone(),
                on_stage: Some(stage.as_fn()),
            },
            clock,
        )
        .await?;
        stage.bare(GoldenStage::Ready);
        Ok::<Builder, GoldenError>(Builder {
            machine: machine.clone(),
            kind,
            base_template: opts.head.base_template.clone(),
            setup_sha: next_setup_sha(&opts.head.setup_sha, &opts.setup, &opts.delta.import),
            created_at: created_at.clone(),
            first_life: true,
            size,
            import: Some(applied.ledger),
            parent_snapshot_id: Some(opts.head.snapshot_id.clone()),
            base: opts.head.base.clone(),
            retired: Some(opts.delta.retired_on_image.clone()),
        })
    };
    match attempt.await {
        Ok(builder) => Ok(builder),
        Err(e) => {
            let mut left: Vec<String> = Vec::new();
            if let Err(k) = kill_until_gone(&*opts.backend, &*machine, KillConfirm::default(), clock).await {
                if let Some(at) = staged.current() {
                    stage.say(at, &machine_left_line(&k.to_string()));
                }
                left.push(machine.id().to_string());
            }
            stage.failed(&e.message(), &left);
            Err(e)
        }
    }
}
