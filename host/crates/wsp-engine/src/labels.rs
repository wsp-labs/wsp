// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/labels.ts. Spec: spec/sweeps.md.
//!
//! The metadata keys wsp stamps on every machine it creates, in one place, so the builder, the sweep and the fork read
//! the same names.
use indexmap::IndexMap;

/// labels.ts:6. Every machine wsp makes wears this; the sweep never touches one without it.
pub const WSP_LABEL: &str = "wsp";
pub const BUILDER_LABEL: &str = "wsp-builder";
pub const SMOKE_LABEL: &str = "wsp-smoke";
/// labels.ts:10.
pub const HOST_LABEL: &str = "wsp-host";
pub const DOCTOR_LABEL: &str = "wsp-doctor";
/// labels.ts:14. Which state file made a machine; survives a resume, unlike the provider's createdAt.
pub const OWNER_LABEL: &str = "wsp-owner";
/// labels.ts:16.
pub const WORKSPACE_LABEL: &str = "wsp-workspace";
pub const NAME_LABEL: &str = "wsp-name";
pub const GOLDEN_LABEL: &str = "wsp-golden";
/// labels.ts:21. Stamped by our clock: the provider's own createdAt moves on a running machine.
pub const CREATED_AT_LABEL: &str = "createdAt";
/// labels.ts:23. Sleeping experiments wear this; the reaper never touches one, even when it also wears ours.
pub const RESERVED_LABEL: &str = "poc";

/// labels.ts:26. A machine no sweep, doctor or cleanup may touch: the key is present, whatever its value (empty too).
pub fn is_reserved(labels: &IndexMap<String, String>) -> bool {
    todo!()
}
