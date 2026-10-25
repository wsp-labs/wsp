// SPDX-License-Identifier: AGPL-3.0-only
//! The files that pin a toolchain by name and version: asdf's .tool-versions and mise's [tools] table. Port of
//! packages/collect/src/project/readers/toolchain-pins.ts at a3b870a6e; the spec is spec/project.md.
use std::sync::LazyLock;

use regex::Regex;
use wsp_words::js::{is_js_whitespace, js_trim};

use crate::project::reader::{ProjectFile, ProjectFinding, ProjectReader, Reads};

pub const FILES: [&str; 3] = [".tool-versions", "mise.toml", ".mise.toml"];

/// One pin: a tool's name and the first version named for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pin {
    pub name: String,
    pub version: Option<String>,
}

/// The `#` comment to the end of a line. JavaScript's `.` stops before a carriage return, so a CRLF line keeps its comment.
static COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"#[^\n\r\x{2028}\x{2029}]*$").expect("COMMENT compiles"));
static VERSION: LazyLock<Regex> = LazyLock::new(|| Regex::new(r#"["']([^"']+)["']"#).expect("VERSION compiles"));

/// `golang 1.24.0 1.23.5` to golang at 1.24.0; a comment or a blank line to nothing.
fn tool_versions_pins(text: &str) -> Vec<Pin> {
    text.split('\n')
        .filter_map(|line| {
            let cut = COMMENT.replace(line, "");
            let mut words = js_trim(&cut).split(is_js_whitespace).filter(|w| !w.is_empty());
            let name = words.next()?;
            Some(Pin { name: name.to_string(), version: words.next().map(str::to_string) })
        })
        .collect()
}

/// The key with one leading and one trailing quote removed, each on its own.
fn unquote_key(key: &str) -> &str {
    let start = usize::from(key.starts_with(['"', '\'']));
    let end = if key.len() > start && key.ends_with(['"', '\'']) { key.len() - 1 } else { key.len() };
    &key[start..end]
}

/// The `[tools]` table's keys with the first version each names; a value that is a table or a list gives the first string in it.
fn mise_toml_pins(text: &str) -> Vec<Pin> {
    let mut out = Vec::new();
    let mut in_tools = false;
    for raw in text.split('\n') {
        let cut = COMMENT.replace(raw, "");
        let line = js_trim(&cut);
        if line.starts_with('[') {
            in_tools = line == "[tools]";
            continue;
        }
        if !in_tools {
            continue;
        }
        let Some(eq) = line.find('=') else {
            continue;
        };
        let name = unquote_key(js_trim(&line[..eq]));
        if name.is_empty() {
            continue;
        }
        let version = VERSION.captures(&line[eq + 1..]).map(|c| c[1].to_string());
        out.push(Pin { name: name.to_string(), version });
    }
    out
}

/// One finding per pin: `<path> names <name>` with its version when the file gave one.
pub fn read(file: &ProjectFile) -> Vec<ProjectFinding> {
    let pins = if file.path == ".tool-versions" { tool_versions_pins(&file.text) } else { mise_toml_pins(&file.text) };
    pins.into_iter()
        .map(|p| ProjectFinding {
            label: p.name.clone(),
            why: format!("{} names {}{}", file.path, p.name, p.version.map_or(String::new(), |v| format!(" {v}"))),
            name: p.name,
            catalog_only: false,
            installs: None,
        })
        .collect()
}

pub const TOOLCHAIN_PINS_READER: ProjectReader = ProjectReader { id: "toolchain-pins", files: &FILES, reads: Reads::Text, read };
