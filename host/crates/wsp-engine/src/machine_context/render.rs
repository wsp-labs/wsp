// SPDX-License-Identifier: AGPL-3.0-only
//! machine-context.ts:304-547 at a4033dc34. Spec: spec/context_render.md.
use std::sync::Arc;

use wsp_catalog::context::{ContextHooks, ContextOutcomeKind, GuestRoots};

use super::probe::{BuildFacts, ContextAgent, ContextProbe, ProbeShells};
use crate::fetch::Fetch;
use crate::golden_seam::ImportResult;
use crate::machine::Machine;

/// machine-context.ts:31.
pub const SKILL_DESCRIPTION: &str = r#"How this cloud Linux machine differs from the person's computer, what did not install here, where secrets live, and how to keep a server alive, hand over a URL and sign in. Read it before starting a server, opening a port, using a secret or calling a tool that may not be installed."#;

/// machine-context.ts:321.
pub const SERVERS_OFF_LINE: &str = r#"- A server that needs the person's own sign-in or key does not run on a cloud machine, which holds no sign-in or key of theirs. When a tool from one is missing, say it is not available on cloud machines, and do not suggest signing it in here (no /mcp, no mcp login command)."#;

/// A workspace by name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceName {
    pub name: String,
}

/// `Pick<GoldenVersion, "version" | "createdAt" | "setupSha" | "npmBin">`.
#[derive(Debug, Clone, PartialEq)]
pub struct GoldenPick {
    pub version: u32,
    pub created_at: String,
    pub setup_sha: String,
    pub npm_bin: Option<String>,
}

/// machine-context.ts:306.
#[derive(Debug, Clone)]
pub struct ContextInput {
    pub workspace: Option<WorkspaceName>,
    pub golden: Option<GoldenPick>,
    pub probe: ContextProbe,
    pub facts: BuildFacts,
    pub agent: Option<ContextAgent>,
    pub at_provider: bool,
}

/// machine-context.ts:341.
pub fn render_machine_context(input: &ContextInput) -> String {
    todo!()
}

/// machine-context.ts:409.
pub fn render_short_context(input: &ContextInput) -> String {
    todo!()
}

/// machine-context.ts:440.
pub fn render_skill(doc: &str) -> String {
    todo!()
}

/// machine-context.ts:449. `None` roots are the guest's.
pub fn agent_files(agent: ContextAgent, short: &str, skill: &str, probe: &ContextProbe, roots: Option<&GuestRoots>) -> ContextHooks {
    todo!()
}

/// machine-context.ts:455.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextResult {
    pub agent: String,
    pub outcome: ContextOutcomeKind,
    pub path: Option<String>,
    pub note: Option<String>,
    pub skill: String,
}

/// machine-context.ts:469.
pub fn context_line(r: &ContextResult) -> String {
    todo!()
}

/// machine-context.ts:474.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextOutcome {
    pub context: Vec<ContextResult>,
    pub summary: String,
    /// Set when the probe or the write did not run; nothing was changed on the guest then.
    pub failure: Option<String>,
}

/// machine-context.ts:482.
#[derive(Clone, Default)]
pub struct ApplyContextOptions {
    pub workspace: Option<WorkspaceName>,
    pub golden: Option<GoldenPick>,
    pub result: Option<ImportResult>,
    pub roots: Option<GuestRoots>,
    /// The upload's transport; tests hand in one.
    pub fetch: Option<Arc<dyn Fetch>>,
    /// The PATH the probe runs on; `None` is TOOLS_PATH.
    pub path: Option<String>,
    pub shells: Option<ProbeShells>,
    pub at_provider: bool,
}

/// machine-context.ts:498.
pub fn summarize(results: &[ContextResult], agents: &[String], bytes: u64) -> String {
    todo!()
}

/// machine-context.ts:514. Never fails: a guest that does not answer is the outcome's `failure`.
pub async fn apply_machine_context(machine: &dyn Machine, opts: ApplyContextOptions) -> ContextOutcome {
    todo!()
}
