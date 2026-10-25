// SPDX-License-Identifier: AGPL-3.0-only
//! The marks on text that only means something on a cloud, and the two readings of them. Port of
//! `packages/host/src/cloud-text.ts` at 65f6b89c7; the spec is `spec/skill.md`.

pub const CLOUD_MARK: &str = "<!-- cloud -->";
pub const CLOUD_SPAN_END: &str = "<!-- /cloud -->";
pub const BOAT_MARK: &str = "<!-- boat -->";
pub const BOAT_SPAN_END: &str = "<!-- /boat -->";
pub const NO_CLOUD_MARK: &str = "<!-- no cloud -->";
pub const NO_CLOUD_SPAN_END: &str = "<!-- /no cloud -->";

/// cloud-text.ts:22-27.
pub fn cloud_text(text: &str, cloud: bool) -> String {
    todo!()
}

/// cloud-text.ts:31-51.
pub fn skill_for(skill: &str, cloud: bool) -> String {
    todo!()
}
