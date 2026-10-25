// SPDX-License-Identifier: AGPL-3.0-only
//! The words of a project's icon. Port of the functions of packages/protocol/src/project-look.ts at
//! the cut (383708c78); its shapes and flat constants are wsp_wire::types::project_look. See spec/project_look.md.

use serde::Serialize;

use wsp_wire::types::project_look::PROJECT_ICON_AGAIN;

// The glyphs a project can wear in the sidebar and the switcher; the app maps each word to its drawing.
// project-look.ts:7
// in wsp_wire::types::project_look: ProjectIcon

// project-look.ts:8
// in wsp_wire::types::project_look: ProjectIcon

// The hues a project's glyph can take; the app maps each word to a colour of its own theme.
// project-look.ts:10
// in wsp_wire::types::project_look: ProjectHue

// project-look.ts:11
// in wsp_wire::types::project_look: ProjectHue

// An image a project wears, named by the SHA-256 of the PNG the host keeps for it.
// project-look.ts:14
// in wsp_wire::types::project_look: ProjectIconHash

// project-look.ts:16
// in wsp_wire::types::project_look: PROJECT_ICON_PX

// What the PNG the host keeps may weigh: a 128 px RGBA PNG stored with no compression comes to under 66 KB.
// project-look.ts:18
// in wsp_wire::types::project_look: PROJECT_ICON_MAX_BYTES

// project-look.ts:21
// in wsp_wire::types::project_look: PROJECT_ICON_SOURCE_PX

// The types the window fits into a project's image, by the type a browser reads off the file.
// project-look.ts:23
// in wsp_wire::types::project_look: PROJECT_ICON_TYPES

// project-look.ts:26
// in wsp_wire::types::project_look: PROJECT_ICON_AGAIN

/// The host's refusal of a project's image, naming the rule the PNG broke.
// project-look.ts:28
// export const projectIconRefusal = (why: string): { said: string; fix: string } => ({ said: `The host did not take this image: ${why}.`, fix: PROJECT_ICON_AGAIN });
/// The TS `{ said, fix }`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SaidFix {
    pub said: String,
    pub fix: String,
}

pub fn project_icon_refusal(why: &str) -> SaidFix {
    SaidFix { said: format!("The host did not take this image: {why}."), fix: PROJECT_ICON_AGAIN.to_string() }
}
