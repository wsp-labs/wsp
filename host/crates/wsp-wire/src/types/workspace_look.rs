// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/workspace-look.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};
use crate::types::js_number::JsNumber;

/// TS: packages/protocol/src/workspace-look.ts:15 at a3b870a6e.
pub const WORKSPACE_GLYPHS: &[&str] = &[
    "terminal", "code", "bug", "wrench", "hammer", "rocket", "flask", "database", "cloud", "globe", "compass", "map", "book", "feather",
    "palette", "leaf", "star", "bolt", "key", "shield", "box", "folder", "chip", "lamp",
];

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

impl Check for WorkspaceGlyph {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "terminal", "code", "bug", "wrench", "hammer", "rocket", "flask", "database", "cloud", "globe", "compass", "map", "book",
                "feather", "palette", "leaf", "star", "bolt", "key", "shield", "box", "folder", "chip", "lamp",
            ]),
        )
    }
}

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
        crate::types::check::num_at_least(self.angle.0, 0.0).map_err(|e| e.under("angle"))?;
        crate::types::check::num_at_most(self.angle.0, 360.0).map_err(|e| e.under("angle"))?;
        crate::types::check::num_at_least(self.radius.0, 0.0).map_err(|e| e.under("radius"))?;
        crate::types::check::num_at_most(self.radius.0, 1.0).map_err(|e| e.under("radius"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsNumber>(obj, "angle", &mut out, Self::rule_angle);
        crate::types::keys::req_key_with::<JsNumber>(obj, "radius", &mut out, Self::rule_radius);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "angle" => <JsNumber as Check>::describe(rest),
                "radius" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ThemeDot {
    fn rule_angle(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0, 0.0)?;
        crate::types::check::num_at_most(x.0, 360.0)?;
        Ok(())
    }
    fn rule_radius(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0, 0.0)?;
        crate::types::check::num_at_most(x.0, 1.0)?;
        Ok(())
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

impl Check for ThemeHarmony {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&["single", "complementary", "analogous", "splitComplementary", "triadic", "analogousThree"]),
        )
    }
}

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

impl Check for ThemeMode {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["auto", "light", "dark"]))
    }
}

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
        crate::types::check::items_at_least(self.dots.len(), 1).map_err(|e| e.under("dots"))?;
        crate::types::check::items_at_most(self.dots.len(), 3).map_err(|e| e.under("dots"))?;
        self.dots.check().map_err(|e| e.under("dots"))?;
        self.harmony.check().map_err(|e| e.under("harmony"))?;
        crate::types::check::num_at_least(self.grain.0, 0.0).map_err(|e| e.under("grain"))?;
        crate::types::check::num_at_most(self.grain.0, 1.0).map_err(|e| e.under("grain"))?;
        crate::types::check::num_at_least(self.opacity.0, 0.1).map_err(|e| e.under("opacity"))?;
        crate::types::check::num_at_most(self.opacity.0, 1.0).map_err(|e| e.under("opacity"))?;
        self.mode.check().map_err(|e| e.under("mode"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<Vec<ThemeDot>>(obj, "dots", &mut out, Self::rule_dots);
        crate::types::keys::req_key::<ThemeHarmony>(obj, "harmony", &mut out);
        crate::types::keys::req_key_with::<JsNumber>(obj, "grain", &mut out, Self::rule_grain);
        crate::types::keys::req_key_with::<JsNumber>(obj, "opacity", &mut out, Self::rule_opacity);
        crate::types::keys::req_key::<ThemeMode>(obj, "mode", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "dots" => <Vec<ThemeDot> as Check>::describe(rest),
                "harmony" => <ThemeHarmony as Check>::describe(rest),
                "grain" => <JsNumber as Check>::describe(rest),
                "opacity" => <JsNumber as Check>::describe(rest),
                "mode" => <ThemeMode as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl WorkspaceTheme {
    fn rule_dots(x: &Vec<ThemeDot>) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::items_at_least((*x).len(), 1)?;
        crate::types::check::items_at_most((*x).len(), 3)?;
        Ok(())
    }
    fn rule_grain(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0, 0.0)?;
        crate::types::check::num_at_most(x.0, 1.0)?;
        Ok(())
    }
    fn rule_opacity(x: &JsNumber) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::num_at_least(x.0, 0.1)?;
        crate::types::check::num_at_most(x.0, 1.0)?;
        Ok(())
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
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::dbl_key::<WorkspaceTheme>(obj, "theme", &mut out);
        crate::types::keys::dbl_key::<WorkspaceGlyph>(obj, "glyph", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "theme" => <Option<Option<WorkspaceTheme>> as Check>::describe(rest),
                "glyph" => <Option<Option<WorkspaceGlyph>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
