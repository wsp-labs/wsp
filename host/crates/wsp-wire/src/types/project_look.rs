// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/project-look.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

use serde::{Deserialize, Serialize};

use crate::types::check::{Check, SchemaError};

/// TS: packages/protocol/src/project-look.ts:16 at a3b870a6e.
pub const PROJECT_ICON_PX: u64 = 128;

/// TS: packages/protocol/src/project-look.ts:18 at a3b870a6e.
pub const PROJECT_ICON_MAX_BYTES: u64 = 81_920;

/// TS: packages/protocol/src/project-look.ts:21 at a3b870a6e.
pub const PROJECT_ICON_SOURCE_PX: u64 = 4_096;

/// TS: packages/protocol/src/project-look.ts:23 at a3b870a6e.
pub const PROJECT_ICON_TYPES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif", "image/svg+xml"];

/// TS: packages/protocol/src/project-look.ts:26 at a3b870a6e.
pub const PROJECT_ICON_AGAIN: &str = "Pick it again; if it is refused again, pick another file.";

/// TS: packages/protocol/src/project-look.ts:7 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectIcon {
    #[serde(rename = "folder")]
    Folder,
    #[serde(rename = "code")]
    Code,
    #[serde(rename = "terminal")]
    Terminal,
    #[serde(rename = "globe")]
    Globe,
    #[serde(rename = "rocket")]
    Rocket,
    #[serde(rename = "box")]
    Box,
    #[serde(rename = "database")]
    Database,
    #[serde(rename = "server")]
    Server,
    #[serde(rename = "cpu")]
    Cpu,
    #[serde(rename = "zap")]
    Zap,
    #[serde(rename = "flame")]
    Flame,
    #[serde(rename = "leaf")]
    Leaf,
    #[serde(rename = "star")]
    Star,
    #[serde(rename = "heart")]
    Heart,
    #[serde(rename = "book")]
    Book,
    #[serde(rename = "music")]
    Music,
    #[serde(rename = "camera")]
    Camera,
    #[serde(rename = "gamepad")]
    Gamepad,
    #[serde(rename = "shield")]
    Shield,
    #[serde(rename = "wrench")]
    Wrench,
}

impl Check for ProjectIcon {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&[
                "folder", "code", "terminal", "globe", "rocket", "box", "database", "server", "cpu", "zap", "flame", "leaf", "star",
                "heart", "book", "music", "camera", "gamepad", "shield", "wrench",
            ]),
        )
    }
}

/// TS: packages/protocol/src/project-look.ts:10 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProjectHue {
    #[serde(rename = "neutral")]
    Neutral,
    #[serde(rename = "red")]
    Red,
    #[serde(rename = "orange")]
    Orange,
    #[serde(rename = "amber")]
    Amber,
    #[serde(rename = "green")]
    Green,
    #[serde(rename = "teal")]
    Teal,
    #[serde(rename = "blue")]
    Blue,
    #[serde(rename = "violet")]
    Violet,
    #[serde(rename = "pink")]
    Pink,
}

impl Check for ProjectHue {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(
            path,
            crate::types::check::Zod::Options(&["neutral", "red", "orange", "amber", "green", "teal", "blue", "violet", "pink"]),
        )
    }
}

/// TS: packages/protocol/src/project-look.ts:14 at a3b870a6e.
/// TS: project-look.ts:14 at a3b870a6e..
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProjectIconHash(pub String);

impl Check for ProjectIconHash {
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::regex_check(crate::types::check::hex_64(&self.0))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Word("string"))
    }
}
