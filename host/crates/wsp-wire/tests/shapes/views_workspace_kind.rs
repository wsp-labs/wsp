// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of views/workspace-kind.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::views::workspace_kind::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/views_workspace_kind.json");

/// 4 cases: what zod's parse of WorkspaceKind wrote, or the first issue it raised.
#[test]
fn workspace_kind_reads_and_writes_as_zod_does() {
    cases::<WorkspaceKind>(FIXTURES, "WorkspaceKind");
}
