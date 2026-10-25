// SPDX-License-Identifier: AGPL-3.0-only
//! The brand, provider and editor marks the catalog draws. Port of packages/catalog/src/marks/*.ts at a3b870a6e; the
//! spec is spec/marks.md. The svgs are data, copied by script from tests/fixtures/marks.json.
use serde::Serialize;

use crate::catalog::AgentMark;
use wsp_wire::views::preferences::EditorId;
use wsp_wire::types::agents_report::McpRowTransport;

/// marks/mark.ts:5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BrandMark {
    pub id: String,
    #[serde(flatten)]
    pub mark: AgentMark,
    pub tools: Vec<String>,
    pub hosts: Vec<String>,
    pub packages: Vec<String>,
}

/// marks/provider.ts:5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProviderMark {
    pub id: String,
    #[serde(flatten)]
    pub mark: AgentMark,
    pub provider: String,
}

/// marks/editor.ts:5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EditorMark {
    pub id: String,
    #[serde(flatten)]
    pub mark: AgentMark,
    pub editors: Vec<EditorId>,
}

/// node, github, cloudflare, linear (marks/index.ts:33).
pub fn brand_marks() -> &'static [BrandMark] {
    todo!()
}

/// solari then boat; boat alone with the `public-build` feature (marks/index.ts:39).
pub fn provider_marks() -> &'static [ProviderMark] {
    todo!()
}

/// vscode, cursor, vscode-insiders, zed, idea, webstorm, pycharm, goland, clion, phpstorm, rubymine, rider, jetbrains,
/// finder (marks/index.ts:41).
pub fn editor_marks() -> &'static [EditorMark] {
    todo!()
}

/// marks/index.ts:44.
pub fn editor_mark(editor: EditorId) -> Option<&'static EditorMark> {
    todo!()
}

/// marks/index.ts:49.
pub fn provider_mark(provider: &str) -> Option<&'static ProviderMark> {
    todo!()
}

/// marks/index.ts:56.
pub fn tool_mark(id: &str) -> Option<&'static BrandMark> {
    todo!()
}

/// A word with its version cut: `@scope/name@1.2` is `@scope/name`, `image:tag` is `image` (marks/index.ts:69).
pub fn package_of_word(word: &str) -> String {
    todo!()
}

/// A server's mark off its address's host, or the first word of its line naming a package or an address a mark
/// claims (marks/index.ts:72).
pub fn server_mark(transport: &McpRowTransport) -> Option<&'static BrandMark> {
    todo!()
}
