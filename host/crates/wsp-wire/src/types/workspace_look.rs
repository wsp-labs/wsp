// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/workspace-look.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;

/// TS: packages/protocol/src/workspace-look.ts:15 at a3b870a6e.
pub const WORKSPACE_GLYPHS: &[&str] = &["terminal", "code", "bug", "wrench", "hammer", "rocket", "flask", "database", "cloud", "globe", "compass", "map", "book", "feather", "palette", "leaf", "star", "bolt", "key", "shield", "box", "folder", "chip", "lamp"];

/// TS: packages/protocol/src/workspace-look.ts:51 at a3b870a6e.
pub const THEME_HARMONIES: &[&str] = &["single", "complementary", "analogous", "splitComplementary", "triadic", "analogousThree"];

/// TS: packages/protocol/src/workspace-look.ts:72 at a3b870a6e.
pub const THEME_MAX_DOTS: u64 = 3;

/// TS: packages/protocol/src/workspace-look.ts:79 at a3b870a6e.
pub const THEME_MIN_OPACITY: f64 = 0.1;

/// TS: packages/protocol/src/workspace-look.ts:81 at a3b870a6e.
pub const THEME_GRAIN_STEPS: u64 = 16;

/// TS: packages/protocol/src/workspace-look.ts:105 at a3b870a6e.
pub const LOOK_PARTS: &[&str] = &["glyph", "theme"];

/// TS: packages/protocol/src/workspace-look.ts:172 at a3b870a6e.
pub const WORD_FLOOR: f64 = 4.5;

/// TS: packages/protocol/src/workspace-look.ts:173 at a3b870a6e.
pub const INK_FLOOR: u64 = 3;

/// TS: packages/protocol/src/workspace-look.ts:41 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorkspaceGlyph {
    #[serde(rename = "terminal")]
    Terminal,
    #[serde(rename = "code")]
    Code,
    #[serde(rename = "bug")]
    Bug,
    #[serde(rename = "wrench")]
    Wrench,
    #[serde(rename = "hammer")]
    Hammer,
    #[serde(rename = "rocket")]
    Rocket,
    #[serde(rename = "flask")]
    Flask,
    #[serde(rename = "database")]
    Database,
    #[serde(rename = "cloud")]
    Cloud,
    #[serde(rename = "globe")]
    Globe,
    #[serde(rename = "compass")]
    Compass,
    #[serde(rename = "map")]
    Map,
    #[serde(rename = "book")]
    Book,
    #[serde(rename = "feather")]
    Feather,
    #[serde(rename = "palette")]
    Palette,
    #[serde(rename = "leaf")]
    Leaf,
    #[serde(rename = "star")]
    Star,
    #[serde(rename = "bolt")]
    Bolt,
    #[serde(rename = "key")]
    Key,
    #[serde(rename = "shield")]
    Shield,
    #[serde(rename = "box")]
    Box,
    #[serde(rename = "folder")]
    Folder,
    #[serde(rename = "chip")]
    Chip,
    #[serde(rename = "lamp")]
    Lamp,
}

impl Check for WorkspaceGlyph {}

/// TS: packages/protocol/src/workspace-look.ts:46 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThemeDot {
    pub angle: JsNumber,
    pub radius: JsNumber,
}

impl Check for ThemeDot {
    /// Rules: angle: >= 0; angle: <= 360; radius: >= 0; radius: <= 1.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/workspace-look.ts:52 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeHarmony {
    #[serde(rename = "single")]
    Single,
    #[serde(rename = "complementary")]
    Complementary,
    #[serde(rename = "analogous")]
    Analogous,
    #[serde(rename = "splitComplementary")]
    SplitComplementary,
    #[serde(rename = "triadic")]
    Triadic,
    #[serde(rename = "analogousThree")]
    AnalogousThree,
}

impl Check for ThemeHarmony {}

/// TS: packages/protocol/src/workspace-look.ts:75 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ThemeMode {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "light")]
    Light,
    #[serde(rename = "dark")]
    Dark,
}

impl Check for ThemeMode {}

/// TS: packages/protocol/src/workspace-look.ts:83 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTheme {
    pub dots: Vec<ThemeDot>,
    pub harmony: ThemeHarmony,
    pub grain: JsNumber,
    pub opacity: JsNumber,
    pub mode: ThemeMode,
}

impl Check for WorkspaceTheme {
    /// Rules: dots: at least 1 items; dots: at most 3 items; grain: >= 0; grain: <= 1; opacity: >= 0.1; opacity: <= 1.
    /// Nested checks: dots, harmony, mode.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}

/// TS: packages/protocol/src/workspace-look.ts:95 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceLook {
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub theme: Option<Option<WorkspaceTheme>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub glyph: Option<Option<WorkspaceGlyph>>,
}

impl Check for WorkspaceLook {
    /// Nested checks: theme, glyph.
    fn check(&self) -> Result<(), SchemaError> {
        todo!()
    }
}
