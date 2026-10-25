// SPDX-License-Identifier: AGPL-3.0-only
//! How each agent loads the machine context on the guest. Port of packages/catalog/src/context.ts at a3b870a6e; the
//! spec is spec/skills_context.md.
use serde::Serialize;

pub const CONTEXT_MARKER: &str = "wsp writes this file when a workspace is forked; edits are overwritten.";
pub const SKILL_NAME: &str = "wsp-machine";
/// The mode of every file the context lands (context.ts:293).
pub const MODE: u32 = 0o644;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestRoots {
    pub etc: String,
    pub home: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuestFile {
    pub path: String,
    pub mode: u32,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextOutcomeKind {
    Written,
    Fallback,
    NotLoaded,
}

/// context.ts:261.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextHookInput {
    pub short: String,
    pub skill: String,
    pub source: String,
    pub roots: GuestRoots,
    pub conflict: bool,
    pub fish: bool,
}

/// context.ts:274.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextHooks {
    pub outcome: ContextOutcomeKind,
    pub files: Vec<GuestFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub skill: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CdFacts {
    pub fact: String,
    pub howto: String,
}

/// One module per agent (context.ts:326-400).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentContext {
    Claude,
    Codex,
    Gemini,
    Opencode,
    Pi,
    Hermes,
}

impl AgentContext {
    /// Claude Code's alone.
    pub fn cd(self) -> Option<CdFacts> {
        todo!()
    }

    /// The probe condition: Gemini CLI, Pi and Hermes.
    pub fn conflict(self, roots: &GuestRoots) -> Option<String> {
        todo!()
    }

    pub fn hooks(self, input: &ContextHookInput) -> ContextHooks {
        todo!()
    }
}

/// A literal multi-line TOML string unless the text holds `'''` (context.ts:299).
pub fn toml_doc_string(doc: &str) -> String {
    todo!()
}
