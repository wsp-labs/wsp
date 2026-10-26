// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:56-130 and 1067-1228: the second half of a golden. Snapshots the builder, proves the snapshot boots by
//! smoke-testing a fork, and appends a manifest version.
use std::sync::Arc;

use wsp_catalog::signin_rows::NEVER_IN_IMAGE;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::views::golden_image::{
    on_npm_bin, GoldenManifest, GoldenStage, GoldenVersion, GoldenVersionSmoke, AGENTS_BIN,
};
use wsp_wire::wire::machine_link::{MachineLife, MachineSpec};
use wsp_words::js::utf16;
use wsp_words::words::image::{
    credential_on_builder_line, snapshot_attempt_line, snapshot_failed_line, BuilderReading, ProviderAnswer,
};
use wsp_words::words::init::{
    disk_unsettled_line, machine_left_line, snapshot_progress_line, snapshot_stage_line, DISK_SYNC_LINE, SAVING_IMAGE_LINE,
};

use super::prepare::{iso_now, npm_bin_of, claude_link_line, size_asked, spec_of};
use super::smoke::smoke_tally;
use super::templates::{promote_version, templates_of, TemplateWait};
use super::types::{plain, Builder, GoldenError, SealGoldenOptions, SealResult, Staged};
use crate::disk_sync::{disk_unsettled, sync_disk, SyncDiskError};
use crate::errors::{is_missing, EngineError, ErrorKind, RetryClock};
use crate::exec_detached::INLINE_EXEC_MS;
use crate::golden_seam::{used_bytes, profile_path_line, PROFILE_PATH_FILE, TOOLS_PATH};
use crate::gone::{kill_until_gone, machine_state_word};
use crate::image_vault::{export_image_vault, present_paths, ImageVault};
use crate::machine::{ExecOptions, Machine, RunOptions, SnapshotOptions};
use crate::machine_context::BROWSER_SHIM_PATH;
use crate::snapshot_names::golden_name;
use crate::vault::{ReadRoad, VaultOptions};

/// golden.ts:59.
pub const SNAPSHOT_ATTEMPTS: u32 = 3;
/// golden.ts:60.
pub const SNAPSHOT_RETRY_MS: u64 = 60_000;

/// golden.ts:90 `isSnapshotRefusal`.
fn is_snapshot_refusal(e: &EngineError) -> bool {
    e.kind() == Some(ErrorKind::SnapshotUnavailable)
}

/// golden.ts:94. The provider's answer as the failure line reports it: a refusal carries its status; a failure that
/// came with none (a job the far side failed, a link that dropped) is reported as the provider's 500, since the
/// provider's side is where it stopped.
pub fn answer_of(e: &EngineError, now_ms: i64) -> ProviderAnswer {
    ProviderAnswer {
        status: e.status().unwrap_or(if is_snapshot_refusal(e) { 502 } else { 500 }) as f64,
        message: e.to_string(),
        request_id: e.request_id().map(str::to_string),
        at: wsp_words::js_date::iso_string(now_ms as f64).unwrap_or_default(),
    }
}

/// golden.ts:99 `readState`: what the provider says the machine is now, one read: a 404 reads as gone, any other failed
/// read as unread with its reason.
async fn read_state(machine: &dyn Machine) -> (BuilderReading, Option<String>) {
    match machine.state().await {
        Ok(state) => (BuilderReading::State(machine_state_word(state).to_string()), None),
        Err(e) if is_missing(&e) => (BuilderReading::State("gone".to_string()), None),
        Err(e) => (BuilderReading::Unread, Some(e.to_string())),
    }
}

/// What the seal has made so far, which its failure path takes back.
#[derive(Default)]
struct Made {
    snapshot_id: Option<String>,
    template_id: Option<String>,
    builder_alive: bool,
    fork: Option<Arc<dyn Machine>>,
}

/// golden.ts:1071. Sequenced for a two-machine cap: unless the builder is kept, it dies before the smoke fork boots, so
/// the seal itself never holds more than one machine.
pub async fn seal_golden(builder: Builder, opts: SealGoldenOptions, clock: &dyn RetryClock) -> Result<SealResult, GoldenError> {
    let staged = Staged::new(opts.on_stage.clone());
    let stage = staged.stage.clone();
    let smoke = builder.import.as_ref().map(|i| i.smoke.clone()).unwrap_or_else(|| opts.smoke.clone());
    let prior: Vec<GoldenVersion> = opts.manifest.as_ref().map(|m| m.versions.clone()).unwrap_or_default();
    let version_num = prior.last().map_or(0.0, |v| v.version.0) + 1.0;
    // One name for the version: the snapshot is taken under it and the template is promoted under it, and the host in
    // it is the only mark a snapshot can carry (the provider takes no metadata on one).
    let name = golden_name(&opts.host_id, opts.name.as_deref().unwrap_or("default"), version_num as u64)?;
    let templates = templates_of(opts.backend.clone());
    let confirm = opts.kill_confirm.unwrap_or_default();
    let kill = |m: Arc<dyn Machine>| {
        let backend = opts.backend.clone();
        async move { kill_until_gone(&*backend, &*m, confirm, clock).await }
    };
    let retry_ms = opts.snapshot_retry_ms.unwrap_or(SNAPSHOT_RETRY_MS);
    // Read before the snapshot is asked for, off the disk the snapshot takes, and before the try: a builder whose files
    // cannot be read is left as the person set it up, the way a refused snapshot leaves it. A sign-in never sits in an
    // image: a credential file left on the builder would ride into every fork of it.
    let refuse: Vec<String> = opts.refuse.clone().unwrap_or_else(|| NEVER_IN_IMAGE.clone());
    let held = present_paths(&*builder.machine, &refuse).await?;
    if !held.is_empty() {
        return Err(GoldenError::CredentialOnBuilder { message: credential_on_builder_line(&held), paths: held });
    }
    let npm_bin = npm_bin_of(&*builder.machine).await?;
    if let Some(bin) = &npm_bin {
        let wrote = builder
            .machine
            .exec(
                &format!("{} && {}", claude_link_line(), profile_path_line(&on_npm_bin(Some(bin), TOOLS_PATH))),
                ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() },
            )
            .await?;
        if wrote.exit_code.get() != 0 {
            return Err(plain(format!(
                "could not put {AGENTS_BIN} and {bin} on the PATH in {PROFILE_PATH_FILE}: {}",
                wrote.stderr.trim()
            ))
            .into());
        }
    }
    // The person's logins as the sign-in and secrets stages left them, read off the same disk the snapshot takes: a
    // builder whose vault cannot be read is left as the person set it up.
    let vault: Option<ImageVault> = match &opts.vault_paths {
        None => None,
        Some(paths) => Some(
            export_image_vault(
                &*builder.machine,
                paths,
                &VaultOptions {
                    fetch: opts.fetch.clone(),
                    // A provider that mints no signed URL hands the archive back through the one call every backend
                    // has; the image vault is credential files and two secrets files, which that road can carry.
                    read_road: if opts.backend.capabilities().signed_urls { ReadRoad::SignedUrl } else { ReadRoad::Exec },
                    ..VaultOptions::default()
                },
            )
            .await?,
        ),
    };
    // A snapshot that failed changed nothing on the builder, so every failure here is typed as one that leaves it: the
    // provider's 502 is asked again while it still reads the builder running, and any other failure is said once with
    // the builder's state beside it. Only a resumed machine's refusal is another kind of failure, and it passes as
    // itself.
    let take_snapshot = || async {
        let mut attempt = 1u32;
        loop {
            let said = stage.clone();
            let progress: Arc<dyn Fn(crate::machine::SnapshotProgress)> = Arc::new(move |p| {
                said.say(GoldenStage::Snapshotting, &snapshot_progress_line(p.bytes as f64, p.total.map(|t| t as f64)));
            });
            match builder
                .machine
                .snapshot(&name, MachineLife { first_life: builder.first_life }, SnapshotOptions { on_progress: Some(progress) })
                .await
            {
                Ok(id) => return Ok(id),
                Err(e @ EngineError::NotFirstLife { .. }) => return Err(GoldenError::Engine(e)),
                Err(e) => {
                    let answer = answer_of(&e, clock.now());
                    let (state, read_error) = read_state(&*builder.machine).await;
                    let running = state == BuilderReading::State("running".to_string());
                    if !is_snapshot_refusal(&e) || !running || attempt >= SNAPSHOT_ATTEMPTS {
                        return Err(GoldenError::SnapshotFailed {
                            message: snapshot_failed_line(attempt as f64, &answer, &state, read_error.as_deref()),
                            machine_id: builder.machine.id().to_string(),
                            attempts: attempt,
                            answer,
                            builder_state: state,
                            read_error,
                        });
                    }
                    stage.say(
                        GoldenStage::Snapshotting,
                        &snapshot_attempt_line(attempt as f64, SNAPSHOT_ATTEMPTS as f64, &answer, &state, retry_ms as f64),
                    );
                    clock.sleep(retry_ms).await;
                    attempt += 1;
                }
            }
        }
    };
    let fork_spec = |made: &Made| -> MachineSpec {
        let asked = size_asked(&*opts.backend, &opts.size, Some(&builder.size));
        let mut o = opts.size.clone();
        if let Some(bin) = &npm_bin {
            let mut envs = o.envs.clone().unwrap_or_default();
            let path = on_npm_bin(Some(bin), o.envs.as_ref().and_then(|e| e.get("PATH")).map_or(TOOLS_PATH, String::as_str));
            envs.insert("PATH".to_string(), path);
            o.envs = Some(envs);
        }
        let mut spec = spec_of(&*opts.backend, builder.kind, asked, &o);
        let snapshot = made.snapshot_id.clone().expect("a snapshot before its fork");
        match &made.template_id {
            Some(t) => spec.template = Some(t.clone()),
            None => spec.from_snapshot = Some(snapshot),
        }
        spec
    };

    let mut made = Made { builder_alive: true, ..Made::default() };
    let outcome: Result<SealResult, GoldenError> = async {
        stage.say(GoldenStage::Snapshotting, DISK_SYNC_LINE);
        let left = sync_disk(&*builder.machine).await.map_err(|e| match e {
            SyncDiskError::Sync { answer, line } => GoldenError::DiskSync { line, answer },
            SyncDiskError::Exec(e) => GoldenError::Engine(e),
        })?;
        if disk_unsettled(left.as_ref()) {
            if let Some(r) = &left {
                stage.say(GoldenStage::Snapshotting, &disk_unsettled_line(r.dirty_kb, r.writeback_kb));
            }
        }
        let used = used_bytes(&*builder.machine).await;
        stage.say(GoldenStage::Snapshotting, &snapshot_stage_line(used));
        let snapshot_id = take_snapshot().await?;
        made.snapshot_id = Some(snapshot_id.clone());
        if !opts.keep_builder {
            kill(builder.machine.clone()).await?;
            made.builder_alive = false;
        }
        // The template is what forks boot from, so the smoke proves it and not the snapshot behind it.
        if let Some(templates) = &templates {
            stage.say(GoldenStage::Promoting, SAVING_IMAGE_LINE);
            let on_status = stage.clone();
            let wait = TemplateWait {
                ready_ms: opts.template_ready_ms,
                poll_ms: opts.template_poll_ms,
                on_status: Some(Arc::new(move |line| on_status.say(GoldenStage::Promoting, line))),
            };
            made.template_id = Some(promote_version(&**templates, &snapshot_id, &name, &wait, clock).await?.template_id);
        }

        stage.say(GoldenStage::SmokeForking, &smoke);
        let fork = match opts.backend.create(fork_spec(&made), None, None).await {
            Ok(f) => f,
            Err(e) => {
                if !made.builder_alive || e.kind() != Some(ErrorKind::Concurrency) {
                    return Err(e.into());
                }
                stage.say(GoldenStage::SmokeForking, &format!("{smoke}; the account is at its machine cap, so the builder is not kept"));
                kill(builder.machine.clone()).await?;
                made.builder_alive = false;
                opts.backend.create(fork_spec(&made), None, None).await?
            }
        };
        made.fork = Some(fork.clone());
        let lines = stage.clone();
        let smoke_res = fork
            .run(
                &smoke,
                RunOptions {
                    deadline_ms: opts.smoke_timeout_ms.unwrap_or(120_000),
                    on_line: Some(Arc::new(move |line| lines.say(GoldenStage::SmokeForking, line))),
                    ..RunOptions::default()
                },
            )
            .await?;
        if smoke_res.exit_code.get() != 0 {
            return Err(plain(format!(
                "golden smoke failed (exit {}) for {}: {}",
                smoke_res.exit_code.get(),
                serde_json::to_string(&smoke).unwrap_or_default(),
                utf16::slice(&smoke_res.stderr, -500, None)
            ))
            .into());
        }
        // The stage's last detail is what the terminal keeps as its end line, so the tally follows the agents' output.
        stage.say(GoldenStage::SmokeForking, &smoke_tally(&smoke));
        // Read on the fork, which is the image: forks of this version get BROWSER only when the shim is there.
        let browser_shim = fork
            .exec(&format!("test -x {BROWSER_SHIM_PATH}"), ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() })
            .await?
            .exit_code
            .get()
            == 0;
        // The image is proven by now; a fork that outlives its kills is a leak to name, not a reason to throw the
        // person's setup away.
        let leak = kill(fork.clone()).await.err().map(|k| k.to_string());

        let imp = builder.import.as_ref();
        let version = GoldenVersion {
            version: JsNumber(version_num),
            snapshot_id: snapshot_id.clone(),
            template_id: made.template_id.clone(),
            base_template: builder.base_template.clone(),
            kind: Some(builder.kind),
            setup_sha: builder.setup_sha.clone(),
            created_at: iso_now(clock),
            smoke: GoldenVersionSmoke { cmd: smoke.clone(), exit_code: JsNumber(smoke_res.exit_code.get() as f64) },
            size: Some(builder.size.clone()),
            browser_shim: Some(browser_shim),
            npm_bin: npm_bin.clone(),
            logins: opts.logins.clone(),
            missing_tools: imp.and_then(|i| i.missing_tools.clone()),
            silenced: imp.and_then(|i| i.silenced.clone()),
            shell_noise: imp.and_then(|i| i.shell_noise.clone()),
            left_behind: imp.and_then(|i| i.left_behind.clone()),
            parent_snapshot_id: builder.parent_snapshot_id.clone(),
            base: builder.base.clone(),
            retired: builder.retired.clone().filter(|r| !r.is_empty()),
            used_bytes: used.map(|u| JsInt(JsNumber(u))),
            image_hash: None,
            no_secrets: None,
        };
        let kept = if made.builder_alive { "; builder kept for one more change" } else { "" };
        let version_text = wsp_words::js::number_string(version_num);
        match &leak {
            None => stage.say(GoldenStage::Sealed, &format!("v{version_text}{kept}")),
            Some(leak) => stage.say(GoldenStage::Sealed, &format!("v{version_text}{kept}; {leak}")),
        }
        let mut versions = prior.clone();
        versions.push(version.clone());
        Ok(SealResult {
            manifest: GoldenManifest { head: JsNumber(version_num), versions },
            version,
            builder_kept: made.builder_alive,
            vault: vault.clone(),
        })
    }
    .await;
    match outcome {
        Ok(sealed) => Ok(sealed),
        Err(e) => {
            let detail = e.message();
            // A rollback the provider would not take leaves a machine billing: the refusal goes on the stage's own block
            // and the machine is named to whoever can kill it again; the failure's line stays the failure's own.
            let mut left: Vec<String> = Vec::new();
            let mut leaked = |machine: &dyn Machine, k: EngineError| {
                if let Some(at) = staged.current() {
                    stage.say(at, &machine_left_line(&k.to_string()));
                }
                left.push(machine.id().to_string());
            };
            // A refused snapshot changed nothing on the builder, whether the provider refused it, the backend refused a
            // resumed machine or the guest's sync failed: it is left as the person set it up, and whoever holds the
            // record decides what becomes of it.
            let leaves_builder = matches!(
                e,
                GoldenError::SnapshotFailed { .. }
                    | GoldenError::DiskSync { .. }
                    | GoldenError::Engine(EngineError::MachineAlive { .. })
                    | GoldenError::Engine(EngineError::NotFirstLife { .. })
            );
            if made.builder_alive && !leaves_builder {
                if let Err(k) = kill(builder.machine.clone()).await {
                    leaked(&*builder.machine, k);
                }
            }
            if let Some(fork) = made.fork.clone() {
                if let Err(k) = kill(fork.clone()).await {
                    leaked(&*fork, k);
                }
            }
            // A template that read ready and then lost its smoke goes first: the provider refuses to delete a snapshot
            // while a template stands on it.
            if let (Some(id), Some(templates)) = (&made.template_id, &templates) {
                let _ = templates.delete(id).await;
            }
            if let Some(id) = &made.snapshot_id {
                let _ = opts.backend.delete_snapshot(id).await;
            }
            stage.failed(&detail, &left);
            Err(e)
        }
    }
}

