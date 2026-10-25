// SPDX-License-Identifier: AGPL-3.0-only
//! A command on PATH that installs its agent the first time it runs. Port of packages/catalog/src/first-run.ts at
//! a3b870a6e; the spec is spec/seed.md.
use std::sync::LazyLock;

use regex::Regex;

use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::js::{is_js_whitespace, js_trim};

use crate::js_regex::compiled;

/// One module per installer (first-run.ts:268, 284).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirstRunInstaller {
    Mise,
    LazyRun,
}

/// mise, then lazy-run (first-run.ts:299).
pub const FIRST_RUN_INSTALLERS: [FirstRunInstaller; 2] = [FirstRunInstaller::Mise, FirstRunInstaller::LazyRun];

impl FirstRunInstaller {
    pub fn id(self) -> &'static str {
        match self {
            FirstRunInstaller::Mise => "mise",
            FirstRunInstaller::LazyRun => "lazy-run",
        }
    }

    /// The tool the script installs, off its text (first-run.ts:270, 286).
    pub fn tool(self, script: &str) -> Option<String> {
        match self {
            FirstRunInstaller::Mise => mise_tool(script),
            FirstRunInstaller::LazyRun => lazy_run_tool(script),
        }
    }

    /// A shell test that passes once the tool is installed (first-run.ts:278, 290).
    pub fn installed(self, tool: &str) -> String {
        match self {
            FirstRunInstaller::Mise => mise_installed(tool),
            FirstRunInstaller::LazyRun => lazy_run_installed(tool),
        }
    }
}

/// The characters JavaScript's multiline `^` and `$` treat as a line end.
fn is_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// The words of a command's rest, stopping at the first shell operator.
fn command_words(rest: &str) -> Vec<String> {
    let mut words = Vec::new();
    for raw in js_trim(rest).split(is_js_whitespace) {
        if ["||", "&&", ";", "|"].contains(&raw) {
            break;
        }
        let unquoted = raw.strip_prefix(['"', '\'']).unwrap_or(raw);
        words.push(unquoted.trim_end_matches(['"', '\'', ';']).to_string());
    }
    words
}

fn mise_tool(script: &str) -> Option<String> {
    let line = compiled(r"^[^#]*\bmise\s+(?:use|install)\b(.*)$", "")?;
    script.split('\n').find_map(|text| {
        let caps = line.captures(text).ok()??;
        let rest = caps.get(1)?.as_str();
        command_words(rest).into_iter().find(|word| !word.is_empty() && !word.starts_with('-'))
    })
}

/// `exec /path "$@"` on one line, answering the path.
fn exec_launcher(text: &str) -> Option<&str> {
    let path = text.strip_prefix("exec ")?.strip_suffix(" \"$@\"")?;
    (path.len() > 1 && path.starts_with('/') && !path.chars().any(is_js_whitespace)).then_some(path)
}

fn lazy_run_tool(script: &str) -> Option<String> {
    if script.split('\n').nth(1) != Some("# ascii-harness-shim") {
        return None;
    }
    script.split(is_line_end).filter_map(exec_launcher).last().map(str::to_string)
}

fn mise_installed(tool: &str) -> String {
    format!(
        r##"[ -n "$(ls -A "${{MISE_DATA_DIR:-${{XDG_DATA_HOME:-$HOME/.local/share}}/mise}}/installs/"{folder} 2>/dev/null)" ]"##,
        folder = shell_quote(&mise_folder(tool))
    )
}

fn lazy_run_installed(launcher: &str) -> String {
    let name = shell_quote(launcher.rsplit('/').next().unwrap_or_default());
    let lazy = format!(r##"case "$(sed -n 2p {path})" in "# ascii-lazy-harness "*) false ;; esac"##, path = shell_quote(launcher));
    let past = format!(
        r##"d=$(dirname "$(command -v {name})") && PATH=$(printf %s "$PATH" | tr : "\n" | grep -vxF "$d" | paste -sd: -) command -v {name} >/dev/null 2>&1"##
    );
    format!("{{ {lazy} || {{ {past}; }}; }}")
}

/// mise's folder name for a tool spec (first-run.ts:256).
pub fn mise_folder(spec: &str) -> String {
    static LOWER_UPPER: LazyLock<Regex> = LazyLock::new(|| Regex::new("([a-z])([A-Z])").unwrap());
    static UPPER_UPPER_LOWER: LazyLock<Regex> = LazyLock::new(|| Regex::new("([A-Z])([A-Z][a-z])").unwrap());
    let name = match spec.rfind('@') {
        Some(at) if at > 0 && !spec[..at].ends_with(':') => &spec[..at],
        _ => spec,
    };
    let split = LOWER_UPPER.replace_all(name, "${1}-${2}");
    let split = UPPER_UPPER_LOWER.replace_all(&split, "${1}-${2}");
    split
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>()
        .join("-")
}

/// Runs one sh script where the agents' commands are looked up, answering its stdout, or nothing when it failed.
#[allow(async_fn_in_trait)]
pub trait RunScript {
    async fn run(&self, script: &str) -> Option<String>;
}

/// The first bytes of the command each bin answers with on PATH where that command is a script, one record each.
fn heads_script(bins: &[String]) -> String {
    let names: Vec<String> = bins.iter().map(|bin| shell_quote(bin)).collect();
    format!(
        r##"for b in {}; do printf '\036'; p=$(command -v "$b" 2>/dev/null) && [ -f "$p" ] && [ "$(head -c 2 "$p" 2>/dev/null)" = "#!" ] && head -c 4096 "$p"; done; printf '\036'"##,
        names.join(" ")
    )
}

/// Whether each bin's command is a first-run installer with its run to come (first-run.ts:311): one script, and a
/// second only where a wrapper was found; false for every bin where a script could not run.
pub async fn installs_on_first_run(run: &impl RunScript, bins: &[String]) -> Vec<bool> {
    if bins.is_empty() {
        return vec![];
    }
    let heads: Vec<String> = match run.run(&heads_script(bins)).await {
        Some(text) => text.split('\u{1e}').skip(1).take(bins.len()).map(str::to_string).collect(),
        None => vec![],
    };
    let found: Vec<Option<String>> = (0..bins.len())
        .map(|i| {
            let head = heads.get(i).map_or("", String::as_str);
            FIRST_RUN_INSTALLERS.iter().find_map(|installer| {
                let tool = if head.starts_with("#!") { installer.tool(head) } else { None };
                tool.map(|tool| installer.installed(&tool))
            })
        })
        .collect();
    if found.iter().all(Option::is_none) {
        return vec![false; bins.len()];
    }
    let tests: Vec<String> = found
        .iter()
        .map(|test| match test {
            None => "echo 0".to_string(),
            Some(test) => format!("if {test}; then echo 0; else echo 1; fi"),
        })
        .collect();
    let said: Vec<String> = match run.run(&tests.join("\n")).await {
        Some(text) => text.split('\n').map(str::to_string).collect(),
        None => vec![],
    };
    found.iter().enumerate().map(|(i, test)| test.is_some() && said.get(i).map(String::as_str) == Some("1")).collect()
}
