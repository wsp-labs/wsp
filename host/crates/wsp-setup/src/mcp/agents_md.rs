// SPDX-License-Identifier: AGPL-3.0-only
//! `agents-md.ts`: the section wsp keeps in a project's own instruction files, its markers, its text and the rewrite around it.
use wsp_catalog::catalog::AgentEntry;

/// `SECTION_BEGIN` (`agents-md.ts:14`).
pub const SECTION_BEGIN: &str = "<!-- wsp:begin -->";

/// `SECTION_END` (`agents-md.ts:15`).
pub const SECTION_END: &str = "<!-- wsp:end -->";

/// `sectionText` (`agents-md.ts:23`).
pub fn section_text() -> String {
    todo!()
}

/// `withSection` (`agents-md.ts:59`).
pub fn with_section(_text: Option<&str>, _section: &str) -> String {
    todo!()
}

/// `withoutSection` (`agents-md.ts:67`).
pub fn without_section(_text: &str) -> Option<String> {
    todo!()
}

/// `projectDocPaths` (`agents-md.ts:73`).
pub fn project_doc_paths(_entry: &AgentEntry, _project: &str) -> Vec<String> {
    todo!()
}

/// `placeSections` (`agents-md.ts:79`).
pub fn place_sections(_entry: &AgentEntry, _project: &str) -> Vec<String> {
    todo!()
}

/// `removeSections` (`agents-md.ts:89`).
pub fn remove_sections(_entry: &AgentEntry, _project: &str) -> Vec<String> {
    todo!()
}
