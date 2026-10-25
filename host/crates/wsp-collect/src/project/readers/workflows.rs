// SPDX-License-Identifier: AGPL-3.0-only
//! GitHub Actions workflows: what CI installs before it builds the repo. Port of
//! packages/collect/src/project/readers/workflows.ts at a3b870a6e; the spec is spec/project.md.
use std::sync::LazyLock;

use regex::Regex;
use wsp_words::js::{js_trim, js_trim_start, utf16, JS_WS};

use crate::history::commands::command_names;
use crate::project::reader::{ProjectFile, ProjectFinding, ProjectReader, Reads};

pub const WORKFLOWS_DIR: &str = ".github/workflows/";

/// `actions/setup-go@v5` to go, `pnpm/action-setup@v4` to pnpm; any other action to nothing. A word ending in
/// `-action` is the owner's own action for one of its features (`docker/setup-buildx-action`), so the owner is the
/// tool: buildx is not something a machine installs, Docker is.
static SETUP: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r#"^[{JS_WS}]*-?[{JS_WS}]*uses:[{JS_WS}]*["']?[A-Za-z0-9_.-]+/(?:setup-([A-Za-z0-9_.-]+)|(?:([A-Za-z0-9_.-]+)/)?action-setup)"#
    ))
    .expect("SETUP compiles")
});
static USES_OWNER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r#"^[{JS_WS}]*-?[{JS_WS}]*uses:[{JS_WS}]*["']?([A-Za-z0-9_.-]+)/"#)).expect("USES_OWNER compiles"));
static OWNS_IT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"-action$").expect("OWNS_IT compiles"));
static RUN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"^([{JS_WS}]*)-?[{JS_WS}]*run:[{JS_WS}]*([^\r\n\x{{2028}}\x{{2029}}]*)$")).expect("RUN compiles")
});

/// Every step's shell text: an inline `run:`, or the block a `run: |` opens, which runs to the first line no deeper than the key.
fn run_blocks(lines: &[&str]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let Some(m) = RUN.captures(lines[i]) else {
            i += 1;
            continue;
        };
        let rest = js_trim(&m[2]);
        if !rest.is_empty() && !matches!(rest, "|" | ">" | "|-" | ">-") {
            out.push(rest.to_string());
            i += 1;
            continue;
        }
        let indent = utf16::len(&m[1]);
        let mut block = Vec::new();
        let mut j = i + 1;
        while j < lines.len() {
            let line = lines[j];
            if !js_trim(line).is_empty() && utf16::len(line) - utf16::len(js_trim_start(line)) <= indent {
                break;
            }
            block.push(js_trim(line));
            i = j;
            j += 1;
        }
        out.push(block.join("\n"));
        i += 1;
    }
    out
}

/// One finding per setup action (`<path> sets up <name>`), then one per program a run step calls (`<path> runs <name>`).
pub fn read(file: &ProjectFile) -> Vec<ProjectFinding> {
    let lines: Vec<&str> = file.text.split('\n').collect();
    let mut out = Vec::new();
    for line in &lines {
        let Some(m) = SETUP.captures(line) else {
            continue;
        };
        let owner = USES_OWNER.captures(line).and_then(|o| o.get(1)).map(|o| o.as_str());
        let name = match m.get(1) {
            Some(captured) if !OWNS_IT.is_match(captured.as_str()) => captured.as_str(),
            _ => match m.get(2).map(|g| g.as_str()).or(owner) {
                Some(name) => name,
                None => continue,
            },
        };
        out.push(ProjectFinding {
            name: name.to_string(),
            label: name.to_string(),
            why: format!("{} sets up {name}", file.path),
            catalog_only: false,
            installs: None,
        });
    }
    for block in run_blocks(&lines) {
        for name in command_names(&block) {
            out.push(ProjectFinding {
                name: name.clone(),
                label: name.clone(),
                why: format!("{} runs {name}", file.path),
                catalog_only: true,
                installs: None,
            });
        }
    }
    out
}

pub const WORKFLOWS_READER: ProjectReader = ProjectReader { id: "workflows", files: &[WORKFLOWS_DIR], reads: Reads::Text, read };
