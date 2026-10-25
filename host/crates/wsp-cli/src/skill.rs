// SPDX-License-Identifier: AGPL-3.0-only
//! The wsp skill, inlined at build time from `skills/wsp/SKILL.md`, and the MCP server's instructions. Port of
//! `packages/host/src/skill.ts` at 65f6b89c7; the spec is `spec/skill.md`.
pub use crate::cloud_text::{
    cloud_text, skill_for, BOAT_MARK, BOAT_SPAN_END, CLOUD_MARK, CLOUD_SPAN_END, NO_CLOUD_MARK, NO_CLOUD_SPAN_END,
};

/// The repo's skill file as written, marks and all (skill.ts:6).
pub const SKILL_TEXT: &str = include_str!("../../../../skills/wsp/SKILL.md");

/// skill.ts:14: `WSP_SKILL_NAME`.
pub const SKILL_NAME: &str = "wsp";

/// skill.ts:18: `skill_for(SKILL_TEXT, cloud_on())`, worked out at each ask.
pub fn wsp_skill() -> String {
    todo!()
}

/// `skill_for(SKILL_TEXT, cloud)`.
pub fn wsp_skill_for(cloud: bool) -> String {
    todo!()
}

pub const SETUP_HEADING: &str = "## Setting a person up from nothing";
pub const RULES_HEADING: &str = "## Running work well";
pub const VERBS_HEADING: &str = "## Verbs and tools";
pub const SHELL_HEADING: &str = "### For a shell script";

/// skill.ts:34-35.
pub const ESSENTIALS: &str = "Agents work on a project, a folder on one computer, as threads in the person's sidebar. Setup, every verb and the rules are in the wsp skill (`wsp mcp install --agent <id>` writes it).";

/// skill.ts:38-40 with the cloud reading given.
pub fn agents_line_for(agents: &[&str], cloud: bool) -> String {
    todo!()
}

/// skill.ts:38-40 as this process reads the cloud.
pub fn agents_line(agents: &[&str]) -> String {
    todo!()
}

/// skill.ts:44.
pub const INSTRUCTIONS_KEPT: usize = 2048;

/// skill.ts:48.
pub const SLATE_WORDS: &str = "Each thread can own a slate, a live panel the person reads, presses and fills in: when they ask for a panel, dashboard, form, checklist, tracker or walkthrough, call slate_catalog, then slate_write naming the thread";

/// skill.ts:53-54.
pub const THREAD_SLATE_WORDS: &str = "This session is a wsp thread with a slate, a live panel here the person sees while you work. When they want to see, watch, monitor or keep an eye on something (live data, traffic, metrics, a price, logs, a PR, what's unread, progress as it goes), a dashboard, something to fill in or tick off (a form, checklist or walkthrough) or a chart, donut, timeline, treemap, diagram or image, your first tool call is slate_catalog, then slate_write; a list in chat is not a slate. Show data fetched with another tool on the slate, not in chat. A one-off answer or an explanation stays in chat unless they ask to see it. Build and read it with the slate tools, never wsp from a shell. Once the slate is written, a request to see something lands there. A run with every={60} refreshes it with no turns; never poll in chat. Read the sketch a write answers before saying it works, then reply briefly: what you built and what waits on the person.";

/// skill.ts:59-62 with the cloud reading given.
pub fn instructions_of_for(agents: &[&str], scoped: bool, slate: bool, cloud: bool) -> String {
    todo!()
}

/// skill.ts:59-62 as this process reads the cloud.
pub fn instructions_of(agents: &[&str], scoped: bool, slate: bool) -> String {
    todo!()
}

/// skill.ts:66: `instructions_of(THREAD_AGENTS, scoped, slate)`.
pub fn instructions(scoped: bool, slate: bool) -> String {
    todo!()
}
