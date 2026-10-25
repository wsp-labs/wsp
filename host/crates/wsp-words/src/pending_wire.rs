// SPDX-License-Identifier: AGPL-3.0-only
//! The wire types this crate reads that `wsp-wire` has not pushed yet, each at the path it will have there: the
//! merge replaces `crate::pending_wire::` with `wsp_wire::` and deletes this file. A type marked partial holds only
//! the fields these words read; the wire's own type has every field of the zod schema. Field names are the TS names
//! in snake case, serialized in camel case.

pub mod wire {
    pub mod helpers {
        use serde::{Deserialize, Serialize};

        /// protocol/src/wire/helpers.ts:11.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum PermissionEffect {
            Allow,
            Deny,
            Mode,
            Answer,
        }

        /// protocol/src/wire/helpers.ts:14.
        #[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub struct PermissionOption {
            pub id: String,
            pub label: String,
            pub effect: PermissionEffect,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub mode: Option<String>,
        }
    }

    pub mod runtime {
        use serde::{Deserialize, Serialize};

        /// protocol/src/wire/runtime.ts:1356.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum SessionAnswerOutcome {
            Answered,
            Gone,
            Unsupported,
            NotFound,
            NoOption,
        }
    }

    pub mod capabilities {
        use serde::{Deserialize, Serialize};

        /// protocol/src/wire/capabilities.ts:9.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct WorkspaceSize {
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub cpu: f64,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub mem_mb: f64,
        }

        impl AsRef<WorkspaceSize> for WorkspaceSize {
            fn as_ref(&self) -> &WorkspaceSize {
                self
            }
        }

        /// protocol/src/wire/capabilities.ts:44: a WorkspaceSize with its rate and an optional name.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct MachineSizeOffer {
            #[serde(flatten)]
            pub size: WorkspaceSize,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub rate_usd_per_hour: f64,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub name: Option<String>,
        }

        impl AsRef<WorkspaceSize> for MachineSizeOffer {
            fn as_ref(&self) -> &WorkspaceSize {
                &self.size
            }
        }

        /// protocol/src/wire/capabilities.ts, Capabilities. Partial.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct Capabilities {
            #[serde(default)]
            pub copies: bool,
            #[serde(default)]
            pub own_network: bool,
        }

        /// protocol/src/init-job.ts, InitAgent.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct InitAgent {
            pub id: String,
            pub name: String,
            pub configured: bool,
            #[serde(default)]
            pub takes_tools: bool,
        }

        /// The price of the machine a build boots.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct InitPricing {
            pub size: WorkspaceSize,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub rate_usd_per_hour: f64,
        }

        /// protocol/src/wire/capabilities.ts:15. Partial; `keys` is InitKeys, a provider word to whether a key is held.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct InitSetup {
            pub keys: indexmap::IndexMap<String, bool>,
            pub agents: Vec<InitAgent>,
            pub pricing: Option<InitPricing>,
            pub job: Option<crate::pending_wire::types::init_job::InitJob>,
        }

        /// protocol/src/wire/capabilities.ts:140. Partial.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct PlaceCapacity {
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub cores: f64,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub mem_mb: f64,
            #[serde(default, serialize_with = "crate::js::serialize_f64")]
            pub mem_room_mb: f64,
            #[serde(default, serialize_with = "crate::js::serialize_f64")]
            pub machine_mem_mb: f64,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub cpu_taken: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub mem_taken_mb: Option<f64>,
        }
    }
}

pub mod views {
    pub mod snapshot_lineage {
        use serde::{Deserialize, Serialize};

        /// A project on a project image's disk. Partial.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct WorkspaceProject {
            pub name: String,
        }

        /// protocol/src/views/snapshot-lineage.ts:22.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct ProjectGolden {
            pub snapshot_id: String,
            pub projects: Vec<WorkspaceProject>,
            #[serde(default)]
            pub golden: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub version: Option<i64>,
            #[serde(default)]
            pub workspace_id: String,
            pub workspace_name: String,
            pub created_at: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub place: Option<String>,
        }

        /// protocol/src/views/snapshot-lineage.ts:43.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct SealedProjectImage {
            #[serde(flatten)]
            pub golden: ProjectGolden,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub size_bytes: Option<f64>,
        }
    }

    pub mod harness {
        use serde::{Deserialize, Serialize};

        /// protocol/src/views/harness.ts:39.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum HarnessCatalogSource {
            Harness,
            Table,
        }

        /// protocol/src/views/harness.ts:55. Partial.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct HarnessCatalog {
            pub harness: String,
            pub label: String,
            pub source: HarnessCatalogSource,
            pub version: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub refusal: Option<String>,
        }
    }

    pub mod golden_image {
        use serde::{Deserialize, Serialize};

        /// protocol/src/views/golden-image.ts:13.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct GoldenLogin {
            pub name: String,
            pub state: crate::pending_wire::types::init_job::LoginState,
        }

        /// protocol/src/views/golden-image.ts:334. Partial.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct SealedVault {
            #[serde(default)]
            pub sha256: String,
            #[serde(default)]
            pub bytes: i64,
            pub paths: i64,
        }

        /// protocol/src/views/golden-image.ts:362. Partial: recipe and pins left out.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct SealedImage {
            pub name: String,
            pub version: i64,
            pub hash: String,
            #[serde(default)]
            pub recipe_hash: String,
            pub logins: Vec<GoldenLogin>,
            #[serde(default)]
            pub sealed_at: String,
            pub sealed_from: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub vault: Option<SealedVault>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub used_bytes: Option<i64>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub place: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub no_secrets: Option<bool>,
        }

        /// protocol/src/views/golden-image.ts:393.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct SealedImageCopy {
            pub place: String,
            pub version: i64,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub hash: Option<String>,
            #[serde(default)]
            pub snapshot_id: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub template_id: Option<String>,
            #[serde(default)]
            pub built_at: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub size_bytes: Option<i64>,
        }

        /// protocol/src/views/golden-image.ts:415.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct SealedImageExport {
            pub path: String,
            pub bytes: i64,
            #[serde(default)]
            pub hash: String,
        }

        /// protocol/src/views/golden-image.ts:126.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct ToolPin {
            pub tag: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub sha256: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub arch: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub latest: Option<bool>,
        }

        /// protocol/src/views/golden-image.ts:293.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum GoldenStage {
            Creating,
            DeployingDaemon,
            ApplyingSetup,
            UploadingFiles,
            InstallingHarness,
            InstallingTools,
            InstallingMcp,
            Ready,
            Snapshotting,
            Promoting,
            SmokeForking,
            Sealed,
            Failed,
        }
    }

    pub mod place {
        use crate::pending_wire::types::agents_report::AgentSignInState;
        use indexmap::IndexMap;
        use serde::{Deserialize, Serialize};

        /// protocol/src/views/place.ts:513. Partial.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct PlaceView {
            #[serde(default)]
            pub id: String,
            /// PlaceKind's wire word: "here", "box", "provider", ...
            #[serde(default)]
            pub kind: String,
            pub name: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub engine: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub agents: Option<Vec<String>>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub agent_versions: Option<IndexMap<String, String>>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub sign_ins: Option<IndexMap<String, AgentSignInState>>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub build: Option<String>,
        }

        /// protocol/src/views/place.ts:473.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct CloudPlan {
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub key: Option<String>,
            pub trial: bool,
            pub machines_at_once: i64,
            pub sizes: Vec<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub running: Option<i64>,
        }

        /// protocol/src/views/place.ts:484.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct StartsWindow {
            pub used: i64,
            pub limit: i64,
        }

        /// protocol/src/views/place.ts:488.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct CloudStarts {
            pub minute: StartsWindow,
            pub hour: StartsWindow,
            pub day: StartsWindow,
        }
    }

    pub mod desktop_bridge {
        use serde::{Deserialize, Serialize};

        /// protocol/src/views/desktop-bridge.ts:68.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct ContextMenuItem {
            pub id: String,
            pub label: String,
            pub group: String,
            pub enabled: bool,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub refusal: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub hint: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub shortcut: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub accelerator: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub destructive: Option<bool>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub checked: Option<bool>,
        }

        /// protocol/src/views/desktop-bridge.ts:89.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct HostListing {
            pub alias: String,
            pub url: String,
        }

        /// protocol/src/views/desktop-bridge.ts:96.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct HostsView {
            pub here: String,
            pub current: Option<String>,
            pub hosts: Vec<HostListing>,
        }
    }

    pub mod project_bundle {
        use serde::{Deserialize, Serialize};

        /// protocol/src/views/project-bundle.ts:21. Partial: `signals` as words, `rewrite` left out.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct ProjectSecret {
            pub path: String,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub bytes: f64,
            pub signals: Vec<String>,
        }

        /// protocol/src/views/project-bundle.ts:57. Partial.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct ProjectImportEvent {
            /// ProjectImportStage: planned, consented, packing, uploading, landing, done, failed.
            pub stage: String,
            #[serde(default)]
            pub message: String,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub bytes: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub total: Option<f64>,
        }

        /// protocol/src/views/project-bundle.ts:105. Partial.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct ProjectExportEvent {
            /// ProjectExportStage: packing, downloading, landing, done, failed.
            pub stage: String,
            #[serde(default)]
            pub message: String,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub bytes: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub total: Option<f64>,
        }

        /// protocol/src/views/project-bundle.ts:154.
        #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct TerminalRgb {
            pub r: u8,
            pub g: u8,
            pub b: u8,
        }

        #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct PaddingX {
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub left: f64,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub right: f64,
        }

        #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct PaddingY {
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub top: f64,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub bottom: f64,
        }

        /// protocol/src/views/project-bundle.ts:164.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct TerminalConfig {
            pub files: Vec<String>,
            pub font_family: Vec<String>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub font_size: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub theme: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub background: Option<TerminalRgb>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub foreground: Option<TerminalRgb>,
            pub palette: Vec<Option<TerminalRgb>>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub selection_background: Option<TerminalRgb>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub cursor_color: Option<TerminalRgb>,
            /// TerminalCursorStyle: block, bar, underline, block_hollow.
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub cursor_style: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub cursor_style_blink: Option<bool>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub window_padding_x: Option<PaddingX>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub window_padding_y: Option<PaddingY>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub background_opacity: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub background_blur: Option<i64>,
        }
    }

    pub mod session {
        use serde::{Deserialize, Serialize};

        /// protocol/src/views/session.ts:22.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum TitleSource {
            Seed,
            Auto,
            Person,
        }

        /// protocol/src/views/session.ts:34.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum PermissionOutcome {
            Allowed,
            Denied,
            Unanswered,
            Cancelled,
        }

        /// protocol/src/views/session.ts:28.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub enum TurnRefusal {
            #[serde(rename = "sign-in")]
            SignIn,
        }
    }

    pub mod session_events {
        use super::session::TurnRefusal;
        use serde::{Deserialize, Serialize};

        /// protocol/src/views/session-events.ts:16.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "snake_case")]
        pub enum DeltaKind {
            Text,
            Thinking,
            Note,
            ToolUse,
            ToolResult,
        }

        /// protocol/src/views/session-events.ts:154. Partial: the scope fields and ids are left out.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct SessionDeltaEvent {
            pub kind: DeltaKind,
            pub text: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub tool_name: Option<String>,
        }

        /// protocol/src/views/session-events.ts:486 with wire/helpers.ts:26's prompt fields. Partial.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct SessionPermissionEvent {
            #[serde(default)]
            pub ask_id: String,
            pub tool_name: String,
            pub input: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub detail: Option<String>,
            #[serde(default)]
            pub options: Vec<crate::pending_wire::wire::helpers::PermissionOption>,
        }

        /// protocol/src/views/session-events.ts, the event union. Partial: every other member is `Other`.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(tag = "type")]
        pub enum SessionEvent {
            #[serde(rename = "session.delta")]
            Delta(SessionDeltaEvent),
            #[serde(rename = "session.permission")]
            Permission(SessionPermissionEvent),
            #[serde(other)]
            Other,
        }

        /// protocol/src/views/session-events.ts:19.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum TurnStatus {
            Completed,
            Interrupted,
            Failed,
        }

        /// protocol/src/views/session-events.ts:25.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct TurnTokens {
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub input: f64,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub output: f64,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub cached: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub cache_write: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub reasoning: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub context: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub window: Option<f64>,
        }

        /// protocol/src/views/session-events.ts:36. `models` and `limit` are held as JSON here.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct TurnResult {
            pub status: TurnStatus,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub duration_ms: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub waited_ms: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub cost_usd: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub tokens: Option<TurnTokens>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub model: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub models: Option<Vec<serde_json::Value>>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub text: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub error: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub unreached: Option<bool>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub refusal: Option<TurnRefusal>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub limit: Option<serde_json::Value>,
        }
    }
}

pub mod types {
    /// protocol/src/place-word.ts. `ProviderKeyWords` and the table are the views writer's; `provider_key_name` is
    /// this crate's own, a later unit (`wsp_words::place_word`), declared here until it is written.
    pub mod place_word {
        /// place-word.ts:12.
        #[derive(Clone, Debug, PartialEq, Eq)]
        pub struct ProviderKeyWords {
            pub name: &'static str,
            pub key_name: &'static str,
            pub key_console: Option<&'static str>,
            pub sub_threads: bool,
        }

        /// place-word.ts:25, `PROVIDER_KEY_WORDS[provider]`: box and, in a cloud build, solari.
        pub fn provider_key_words(provider: &str) -> Option<ProviderKeyWords> {
            todo!()
        }

        /// place-word.ts:53, namesPlace: the word is the place's id or name, or, for a provider, its id, name or
        /// provider name in any case.
        pub fn names_place(id: &str, name: &str, kind: Option<&str>, word: &str) -> bool {
            todo!()
        }

        /// place-word.ts:44, placeSaid: a provider by `provider_key_name(name)`, any other place by its name.
        pub fn place_said(name: &str, kind: Option<&str>) -> String {
            todo!()
        }

        /// place-word.ts:38, providerKeyName: the provider's own name ("Boat"), or the word as given for one it does not know.
        pub fn provider_key_name(provider: &str) -> String {
            todo!()
        }
    }

    pub mod agents_report {
        use serde::{Deserialize, Serialize};

        /// protocol/src/agents-report.ts:21.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum AgentSignInState {
            SignedIn,
            VaultKey,
            None,
        }
    }

    /// protocol/src/workspace-look.ts. The types are the views writer's; the colour functions are this crate's own,
    /// a later unit (`wsp_words::workspace_look`), declared here until it is written.
    pub mod workspace_look {
        use serde::{Deserialize, Serialize};

        /// workspace-look.ts:134.
        pub type Rgb = [f64; 3];

        /// workspace-look.ts:46.
        #[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct ThemeDot {
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub angle: f64,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub radius: f64,
        }

        /// workspace-look.ts:83. `harmony` and `mode` as their wire words.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct WorkspaceTheme {
            pub dots: Vec<ThemeDot>,
            pub harmony: String,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub grain: f64,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub opacity: f64,
            pub mode: String,
        }

        /// workspace-look.ts:138.
        pub fn dot_colour(dot: &ThemeDot) -> Rgb {
            todo!()
        }

        /// workspace-look.ts:239.
        pub fn effective_opacity(theme: &WorkspaceTheme, app_dark: bool) -> f64 {
            todo!()
        }

        /// workspace-look.ts:248.
        pub fn theme_ink(theme: &WorkspaceTheme, app_dark: bool) -> Rgb {
            todo!()
        }
    }

    pub mod thread_read {
        use serde::{Deserialize, Serialize};

        /// protocol/src/thread-read.ts:17.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum ThreadVoice {
            Person,
            Agent,
            Tool,
            Turn,
        }

        /// protocol/src/thread-read.ts:20.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        pub struct ThreadMessage {
            pub who: ThreadVoice,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub at: Option<f64>,
            pub text: String,
        }
    }

    pub mod init_job {
        use indexmap::IndexMap;
        use serde::{Deserialize, Serialize};

        /// protocol/src/init-job.ts:13.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum InitRoad {
            Manual,
            Agent,
            Terminal,
        }

        /// protocol/src/init-job.ts:19.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum InitPhase {
            Agent,
            Reading,
            Answering,
            Building,
            SigningIn,
            Sealing,
            Finishing,
            Done,
            Failed,
            Cancelled,
        }

        /// protocol/src/init-job.ts:78.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum InitRowKind {
            Fact,
            Stage,
            SignIn,
            Secret,
            Agent,
            Workspace,
            Project,
            Machine,
        }

        /// protocol/src/init-job.ts:86.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "kebab-case")]
        pub enum LoginState {
            SignedIn,
            NotSignedIn,
            NotVerified,
            Skipped,
            Copied,
            Deferred,
        }

        /// protocol/src/init-job.ts:93-94.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum LoginChoice {
            Copy,
            Machine,
            Later,
            Key,
            Skip,
            Token,
        }

        /// protocol/src/init-job.ts:93, in its order.
        pub const LOGIN_CHOICES: [LoginChoice; 6] = [LoginChoice::Copy, LoginChoice::Machine, LoginChoice::Later, LoginChoice::Key, LoginChoice::Skip, LoginChoice::Token];

        /// protocol/src/init-job.ts:101.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum SignInFinish {
            Callback,
            Code,
            None,
        }

        /// protocol/src/init-job.ts:108. Keys in the TS order; partial past `ms`.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct InitRow {
            pub id: String,
            pub kind: InitRowKind,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub tool: Option<String>,
            pub label: String,
            pub state: String,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub login: Option<LoginState>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub detail: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub page: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub code: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub finish: Option<SignInFinish>,
            #[serde(skip_serializing_if = "Option::is_none", default, serialize_with = "crate::js::serialize_opt_f64")]
            pub ms: Option<f64>,
        }

        /// The job's progress count.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct InitProgress {
            pub done: i64,
            pub total: i64,
        }

        /// The job's first workspace.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct InitWorkspace {
            pub id: String,
            pub name: String,
        }

        /// protocol/src/init-job.ts:23.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(rename_all = "lowercase")]
        pub enum InitScreenId {
            Agents,
            Tools,
            Also,
            Logins,
            Wsp,
        }

        /// protocol/src/init-job.ts:26.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct InitChoice {
            pub value: String,
            pub label: String,
        }

        /// protocol/src/init-job.ts:33. Partial: `size` None is both absent and null.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct InitScreenItem {
            pub id: String,
            pub label: String,
            #[serde(default, serialize_with = "crate::js::serialize_opt_f64")]
            pub size: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub lock: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub choices: Option<Vec<InitChoice>>,
        }

        /// protocol/src/init-job.ts:54. Partial.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct InitScreen {
            pub id: InitScreenId,
            #[serde(default)]
            pub title: String,
            #[serde(default)]
            pub top: String,
            pub items: Vec<InitScreenItem>,
            pub ticks: Vec<String>,
            #[serde(default)]
            pub answers: IndexMap<String, String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub tally: Option<String>,
        }

        /// protocol/src/init-job.ts:152.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct InitDraft {
            pub at: String,
            pub ticks: Vec<String>,
            #[serde(default)]
            pub answers: IndexMap<String, String>,
        }

        /// protocol/src/init-job.ts:192.
        #[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
        pub struct InitDisk {
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub fixed: f64,
            #[serde(serialize_with = "crate::js::serialize_f64")]
            pub total: f64,
        }

        /// protocol/src/init-job.ts:180. Partial.
        #[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
        #[serde(rename_all = "camelCase")]
        pub struct InitJob {
            #[serde(default)]
            pub id: String,
            #[serde(default = "manual")]
            pub road: InitRoad,
            #[serde(default = "answering")]
            pub phase: InitPhase,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub disk: Option<InitDisk>,
            pub screens: Vec<InitScreen>,
            #[serde(default)]
            pub rows: Vec<InitRow>,
            #[serde(default)]
            pub progress: InitProgress,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub drafts: Option<Vec<InitDraft>>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub error: Option<String>,
            #[serde(skip_serializing_if = "Option::is_none", default)]
            pub workspace: Option<InitWorkspace>,
        }

        fn manual() -> InitRoad {
            InitRoad::Manual
        }

        fn answering() -> InitPhase {
            InitPhase::Answering
        }
    }

    pub mod env {
        /// protocol/src/env.ts:21.
        pub const CLOUD_ENV: &str = "WSP_CLOUD";
    }
}
