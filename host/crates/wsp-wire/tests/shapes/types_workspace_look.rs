// SPDX-License-Identifier: AGPL-3.0-only
//! The shapes of workspace-look.ts against what zod answered for the same inputs. Generated.

use super::common::cases;
use wsp_wire::types::workspace_look::*;

const FIXTURES: &str = include_str!("../fixtures/shapes/types_workspace_look.json");

/// 7 cases: what zod's parse of ThemeDot wrote, or the first issue it raised.
#[test]
fn theme_dot_reads_and_writes_as_zod_does() {
    cases::<ThemeDot>(FIXTURES, "ThemeDot");
}

/// 7 cases: what zod's parse of ThemeHarmony wrote, or the first issue it raised.
#[test]
fn theme_harmony_reads_and_writes_as_zod_does() {
    cases::<ThemeHarmony>(FIXTURES, "ThemeHarmony");
}

/// 4 cases: what zod's parse of ThemeMode wrote, or the first issue it raised.
#[test]
fn theme_mode_reads_and_writes_as_zod_does() {
    cases::<ThemeMode>(FIXTURES, "ThemeMode");
}

/// 25 cases: what zod's parse of WorkspaceGlyph wrote, or the first issue it raised.
#[test]
fn workspace_glyph_reads_and_writes_as_zod_does() {
    cases::<WorkspaceGlyph>(FIXTURES, "WorkspaceGlyph");
}

/// 4 cases: what zod's parse of WorkspaceLook wrote, or the first issue it raised.
#[test]
fn workspace_look_reads_and_writes_as_zod_does() {
    cases::<WorkspaceLook>(FIXTURES, "WorkspaceLook");
}

/// 11 cases: what zod's parse of WorkspaceTheme wrote, or the first issue it raised.
#[test]
fn workspace_theme_reads_and_writes_as_zod_does() {
    cases::<WorkspaceTheme>(FIXTURES, "WorkspaceTheme");
}
