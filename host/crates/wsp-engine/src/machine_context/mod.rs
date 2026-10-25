// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/machine-context.ts, in two units: `probe` (lines 1-302: the facts kept between
//! builds, the probe command and its parse; spec/context_probe.md) and `render` (lines 304-547: the document, the
//! short text, the skill, each agent's hooks and the run; spec/context_render.md).
pub mod probe;
pub mod render;

pub use probe::*;
pub use render::*;
pub use wsp_catalog::context::{ContextOutcomeKind, GuestFile, GuestRoots, CONTEXT_MARKER, SKILL_NAME};
