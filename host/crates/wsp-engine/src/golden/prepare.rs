// SPDX-License-Identifier: AGPL-3.0-only
//! golden.ts:360-425, 974-1060 and 1428-1453: the first half of a golden (a fresh machine with the harness on it), the
//! npm folder read, and the scripted pipeline that runs both halves.
use indexmap::IndexMap;
use wsp_catalog::roads::GUEST_HOME;
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::golden_image::{GoldenStage, MachineKind, AGENTS_BIN};
use wsp_wire::wire::capabilities::WorkspaceSize;
use wsp_wire::wire::machine_link::{MachineSpec, MachineSpecOnIdle};
use wsp_words::js::{is_js_whitespace, js_trim};
use wsp_words::words::init::machine_left_line;

use super::seal::seal_golden;
use super::smoke::setup_sha_of;
use super::types::{
    plain, ApplyImportOptions, BuildGoldenOptions, Builder, BuiltGolden, GoldenError, MachineSize, PrepareBuilderOptions,
    SealGoldenOptions, Staged,
};
use crate::errors::{EngineError, RetryClock};
use crate::exec_detached::{machine_answer, INLINE_EXEC_MS};
use crate::golden::apply_golden_import;
use crate::golden_seam::{closing, free_note, install_base_with, path_line, InstallBaseOptions, BUILDER_IDLE_MS, TOOLS_PATH};
use crate::gone::{kill_until_gone, KillConfirm};
use crate::labels::{BUILDER_LABEL, CREATED_AT_LABEL, SMOKE_LABEL};
use crate::machine::{ExecOptions, Machine, MachineBackend};

/// golden.ts:983. What a builder runs first, so the builder and every machine forked from its image keep command lines
/// off disk: a provider's exec drops stdin, so a launch's variables ride its text, and on a box that text went whole
/// into the logs.
pub const QUIET_LOGS: &str = "if [ -d /etc/sudoers.d ] && command -v visudo >/dev/null 2>&1; then t=$(mktemp) && printf '%s\\n' 'Defaults !log_allowed' 'Defaults !pam_session' > \"$t\" && visudo -cqf \"$t\" && install -m 0440 \"$t\" /etc/sudoers.d/wsp-quiet-logs; s=$?; rm -f \"$t\"; [ $s -eq 0 ] || exit $s; fi\ninstall -d -m 0755 /etc/systemd/user.conf.d && printf '[Manager]\\nLogLevel=notice\\n' > /etc/systemd/user.conf.d/wsp-quiet-logs.conf && chmod 0644 /etc/systemd/user.conf.d/wsp-quiet-logs.conf\nfor u in $(loginctl list-users --no-legend 2>/dev/null | awk '{ print $2 }'); do systemctl --user -M \"$u@\" daemon-reexec || exit 1; done";

/// golden.ts:1065. Makes the pinned agents' folder afresh with a link to the native claude the agents stage installed.
pub fn claude_link_line() -> String {
    let links = wsp_wire::types::daemon_contract::TOOL_LINKS_DIR;
    format!("rm -rf {AGENTS_BIN} && mkdir -p {AGENTS_BIN} && {{ [ ! -x {links}/claude ] || ln -s {links}/claude {AGENTS_BIN}/claude; }}")
}

/// golden.ts:362 `DEFAULT_TEMPLATE`: the provider built-in templates a backend that names none of its own boots from.
fn default_template(kind: MachineKind) -> &'static str {
    match kind {
        MachineKind::Sandbox => "base",
        MachineKind::Desktop => "default",
    }
}

/// golden.ts:365 `baseTemplateOf`: the caller's word, else the image the backend names, else the built-in.
pub(crate) fn base_template_of(backend: &dyn MachineBackend, kind: MachineKind, named: Option<&str>) -> String {
    if let Some(n) = named {
        return n.to_string();
    }
    match backend.base_templates() {
        Some(t) => match kind {
            MachineKind::Sandbox => t.sandbox,
            MachineKind::Desktop => t.desktop,
        },
        None => default_template(kind).to_string(),
    }
}

/// golden.ts:941 `sizeAsked`: always explicit, since a create that names no size gets the provider's own default.
pub(crate) fn size_asked(backend: &dyn MachineBackend, o: &MachineSize, inherit: Option<&WorkspaceSize>) -> (f64, f64) {
    let default = backend.pricing().default_size();
    (
        o.cpu.map(|n| n.0).or(inherit.map(|s| s.cpu.0)).unwrap_or(default.cpu),
        o.mem_mb.map(|n| n.0).or(inherit.map(|s| s.mem_mb.0)).unwrap_or(default.mem_mb),
    )
}

/// golden.ts:955 `sizeBuilt`: the provider's word on what it built, falling back to the request where it has none.
pub(crate) async fn size_built(machine: &dyn Machine, asked: (f64, f64)) -> WorkspaceSize {
    let shape = if machine.offers().describe { machine.describe().await.ok() } else { None };
    WorkspaceSize {
        cpu: shape.as_ref().and_then(|s| s.cpu).unwrap_or(JsNumber(asked.0)),
        mem_mb: shape.as_ref().and_then(|s| s.mem_mb).unwrap_or(JsNumber(asked.1)),
    }
}

/// golden.ts:948 `envSpec` and 369 `diskAsked` and the size, as the fields of a create.
pub(crate) fn spec_of(backend: &dyn MachineBackend, kind: MachineKind, asked: (f64, f64), o: &MachineSize) -> MachineSpec {
    let mut spec: MachineSpec = serde_json::from_value(serde_json::json!({ "kind": kind })).expect("a bare machine spec");
    spec.cpu = Some(JsNumber(asked.0));
    spec.mem_mb = Some(JsNumber(asked.1));
    spec.disk_gb = backend.pricing().builder_disk_gb().map(JsNumber);
    spec.envs = o.envs.clone();
    spec.labels = o.labels.clone();
    spec
}

/// The ISO time of the clock's now, as `new Date().toISOString()` writes it.
pub(crate) fn iso_now(clock: &dyn RetryClock) -> String {
    wsp_words::js_date::iso_string(clock.now() as f64).unwrap_or_default()
}

/// golden.ts:989. Boots a fresh machine, installs the harness and, with an import, the person's files, agents and
/// tools: a builder to seal.
pub async fn prepare_builder(opts: PrepareBuilderOptions, clock: &dyn RetryClock) -> Result<Builder, GoldenError> {
    let staged = Staged::new(opts.on_stage.clone());
    let stage = staged.stage.clone();
    let kind = opts.kind.unwrap_or(MachineKind::Sandbox);
    let base_template = base_template_of(&*opts.backend, kind, opts.base_template.as_deref());
    stage.say(GoldenStage::Creating, &format!("{} from {base_template}", kind_word(kind)));
    // A builder that idle-pauses resumes not first-life, so its seal would 502 and consume it anyway; killing on idle
    // loses the same work but fails loud and free.
    let asked = size_asked(&*opts.backend, &opts.size, None);
    // Taken before the create, so the age a person reads matches the createdAt label the sweep ages by.
    let created_at = iso_now(clock);
    let mut spec = spec_of(&*opts.backend, kind, asked, &opts.size);
    spec.template = Some(base_template.clone());
    spec.on_idle = Some(MachineSpecOnIdle::Kill);
    spec.idle_timeout_ms = Some(JsNumber(BUILDER_IDLE_MS as f64));
    let machine = opts.backend.create(spec, None, None).await?;
    let attempt = async {
        let quiet = machine.exec(QUIET_LOGS, ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() }).await?;
        if quiet.exit_code.get() != 0 {
            return Err(GoldenError::from(plain(format!(
                "the builder would keep every launch's command line in its logs; {}",
                machine_answer(&quiet)
            ))));
        }
        let size = size_built(&*machine, asked).await;
        stage.bare(GoldenStage::DeployingDaemon);
        // The floor goes on before the daemon: the deploy's own steps type the tools it puts there.
        let base = install_base_with(&*machine, stage.listener(), InstallBaseOptions::default()).await?;
        let deployed = match &opts.deploy_daemon {
            Some(deploy) => deploy(machine.clone()).await?,
            None => None,
        };
        let free = free_note(&*machine, None).await?;
        let detail = closing(&[Some(base.line.as_str()), deployed.as_deref(), free.as_deref()]);
        if !detail.is_empty() {
            stage.say(GoldenStage::DeployingDaemon, &detail);
        }
        let applied = apply_golden_import(
            &machine,
            ApplyImportOptions {
                import: opts.import.clone(),
                setup: opts.setup.clone(),
                base: Some(base.tools.clone()),
                ledger: None,
                retired: None,
                fetch: opts.fetch.clone(),
                on_stage: Some(stage.as_fn()),
            },
            clock,
        )
        .await?;
        stage.bare(GoldenStage::Ready);
        Ok(Builder {
            machine: machine.clone(),
            kind,
            base_template: base_template.clone(),
            setup_sha: setup_sha_of(&opts.setup, opts.import.as_ref()),
            created_at: created_at.clone(),
            first_life: true,
            size,
            import: opts.import.as_ref().map(|_| applied.ledger.clone()),
            parent_snapshot_id: None,
            base: Some(base.versions.clone()),
            retired: None,
        })
    };
    match attempt.await {
        Ok(builder) => Ok(builder),
        Err(e) => {
            // Nothing records this machine yet, so one that survives here is said on the stage's own block and named to
            // whoever can kill it again; the failure's line stays the failure's own.
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

pub(crate) fn kind_word(kind: MachineKind) -> &'static str {
    match kind {
        MachineKind::Sandbox => "sandbox",
        MachineKind::Desktop => "desktop",
    }
}

/// golden.ts:1055. The folder npm's global installs put their commands in on a builder, where the tools PATH names no
/// such folder: a provider's Node under nvm keeps its prefix there, so nothing the recipe installed by npm would answer.
/// Read with no user npmrc and never taken from under the home.
pub async fn npm_bin_of(machine: &dyn Machine) -> Result<Option<String>, EngineError> {
    let line = path_line(TOOLS_PATH, None);
    let res = machine
        .exec(
            &format!("{line}\ncd / && b=\"$(npm_config_userconfig=/dev/null npm prefix -g 2>/dev/null)/bin\" && [ -d \"$b\" ] && readlink -f \"$b\""),
            ExecOptions { timeout_ms: Some(INLINE_EXEC_MS), ..ExecOptions::default() },
        )
        .await?;
    let bin = if res.exit_code.get() == 0 { js_trim(&res.stdout).to_string() } else { String::new() };
    if !bin.starts_with('/')
        || bin.chars().any(|c| is_js_whitespace(c) || c == ':')
        || TOOLS_PATH.split(':').any(|p| p == bin)
        || bin == GUEST_HOME
        || bin.starts_with(&format!("{GUEST_HOME}/"))
    {
        return Ok(None);
    }
    Ok(Some(bin))
}

/// golden.ts:1429. The scripted pipeline: prepare then seal, no one in between.
pub async fn build_golden(opts: BuildGoldenOptions, clock: &dyn RetryClock) -> Result<BuiltGolden, GoldenError> {
    let BuildGoldenOptions { backend, size, setup, smoke, kind, base_template, manifest, smoke_timeout_ms, on_stage, name, host_id } = opts;
    let labelled = |extra: &[(&str, String)]| -> MachineSize {
        let mut labels: IndexMap<String, String> = size.labels.clone().unwrap_or_default();
        for (k, v) in extra {
            labels.insert((*k).to_string(), v.clone());
        }
        MachineSize { labels: Some(labels), ..size.clone() }
    };
    let builder = prepare_builder(
        PrepareBuilderOptions {
            backend: backend.clone(),
            size: labelled(&[(BUILDER_LABEL, "1".to_string())]),
            kind,
            base_template,
            deploy_daemon: None,
            setup,
            import: None,
            fetch: None,
            on_stage: on_stage.clone(),
        },
        clock,
    )
    .await?;
    let sealed = seal_golden(
        builder,
        SealGoldenOptions {
            backend,
            size: labelled(&[(SMOKE_LABEL, "1".to_string()), (CREATED_AT_LABEL, iso_now(clock))]),
            smoke,
            smoke_timeout_ms,
            manifest,
            on_stage,
            kill_confirm: None,
            logins: None,
            keep_builder: false,
            snapshot_retry_ms: None,
            name,
            host_id,
            template_ready_ms: None,
            template_poll_ms: None,
            refuse: None,
            vault_paths: None,
            fetch: None,
        },
        clock,
    )
    .await?;
    Ok(BuiltGolden { manifest: sealed.manifest, version: sealed.version })
}
