// SPDX-License-Identifier: AGPL-3.0-only
//! The one shape of packages/protocol/src/project-import.ts at a3b870a6e that the host reads; its functions are
//! wsp-words'.

use serde::{Deserialize, Serialize};

// project-import.ts:51 at a3b870a6e
// export interface ProjectImportRequest {
//   source: string;
//   dest: string;
//   carry: string[];
//   rewrite: string[];
//   agents?: string[];
//   replace?: true;
// }
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectImportRequest {
    pub source: String,
    pub dest: String,
    pub carry: Vec<String>,
    pub rewrite: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replace: Option<wsp_frames::True>,
}
