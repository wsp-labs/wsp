// SPDX-License-Identifier: AGPL-3.0-only
//! How an agent's settings name the hook scripts it runs, so a copy brings each script or takes the hook out. Port of
//! packages/catalog/src/hooks.ts and codex-hooks.ts at a3b870a6e; the spec is spec/hooks.md.
use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

use wsp_words::exit::HostError;

use crate::json_edit::{edit_json, JsonEdit};
use crate::jsonc::read_jsonc;
use crate::jsonc_tree::PathSeg;
use crate::mcp_toml::{rewrite_toml_line, toml_open_array, toml_strings, uncomment_toml};
use crate::plugins::{js_entries, js_space, js_trim, js_trim_start};

/// Where a script lands on the guest, absolute; nothing when it cannot travel (hooks.ts:11).
pub type HookPlacer<'a> = &'a dyn Fn(&str) -> Option<String>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Carried {
    pub from: String,
    pub to: String,
}

/// hooks.ts:13.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CarriedHooks {
    pub text: String,
    pub carried: Vec<Carried>,
    pub left: Vec<String>,
}

pub const CLAUDE_SETTINGS_FILE: &str = "~/.claude/settings.json";
pub const CODEX_CONFIG_FILE: &str = "~/.codex/config.toml";

/// One module per agent (hooks.ts:132, codex-hooks.ts:221).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum HookCarry {
    Claude,
    Codex,
}

/// A word starting with one of these is a path under the home directory as the shell reads it (hooks.ts:33).
const HOME_PREFIXES: [&str; 3] = ["~/", "$HOME/", "${HOME}/"];

/// The directories every machine image has binaries in (hooks.ts:36).
const MACHINE_BINS: [&str; 4] = ["/bin/", "/usr/bin/", "/usr/local/bin/", "/usr/sbin/"];

/// The words that end one simple command and start the next (hooks.ts:76).
const OPERATORS: [&str; 4] = ["&&", "||", "|", ";"];

/// Commands whose first path argument is the script they run (hooks.ts:78).
const INTERPRETERS: [&str; 12] = ["env", "bash", "sh", "zsh", "dash", "node", "bun", "deno", "python", "python3", "ruby", "perl"];

/// Which word of a command the next one is: the command itself, the script an interpreter runs, or its arguments.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect {
    Command,
    Script,
    Args,
}

/// hooks.ts:102.
enum Rewritten {
    Done { command: String, carried: Vec<Carried> },
    Left(String),
}

impl HookCarry {
    pub fn file(self) -> &'static str {
        match self {
            HookCarry::Claude => CLAUDE_SETTINGS_FILE,
            HookCarry::Codex => CODEX_CONFIG_FILE,
        }
    }

    /// The settings as the machine gets them: the input byte for byte where nothing moved (hooks.ts:134,
    /// codex-hooks.ts:223). Claude Code's edit may refuse as `edit_json` refuses.
    pub fn carry(self, text: &str, home: &str, place: HookPlacer<'_>) -> Result<CarriedHooks, HostError> {
        match self {
            HookCarry::Claude => carry_claude(text, home, place),
            HookCarry::Codex => Ok(carry_codex(text, home, place)),
        }
    }
}

/// An absolute path under /bin/, /usr/bin/, /usr/local/bin/ or /usr/sbin/ with a name after it (hooks.ts:39).
pub fn on_machine(abs: &str) -> bool {
    MACHINE_BINS.iter().any(|d| abs.starts_with(d) && abs.len() > d.len())
}

/// A word's path, `~/`, `$HOME/` and `${HOME}/` expanded; nothing for a word that is not a path (hooks.ts:42).
pub fn path_of(word: &str, home: &str) -> Option<String> {
    if word.starts_with('/') {
        return Some(word.to_string());
    }
    HOME_PREFIXES.iter().find_map(|p| word.strip_prefix(p).map(|rest| format!("{home}/{rest}")))
}

/// hooks.ts:82.
pub fn runs_a_file(word: &str, home: &str) -> bool {
    let name = if path_of(word, home).is_none() { word } else { word.rsplit('/').next().unwrap_or(word) };
    INTERPRETERS.contains(&name)
}

/// hooks.ts:88.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlacedWord {
    Word { word: String, carried: Option<Carried> },
    Left(String),
}

/// hooks.ts:90.
pub fn place_word(word: &str, home: &str, place: HookPlacer<'_>) -> PlacedWord {
    let Some(path) = path_of(word, home) else {
        return PlacedWord::Word { word: word.to_string(), carried: None };
    };
    if on_machine(&path) {
        return PlacedWord::Word { word: word.to_string(), carried: None };
    }
    match place(&path) {
        None => PlacedWord::Left(word.to_string()),
        Some(to) => PlacedWord::Word { word: to.clone(), carried: Some(Carried { from: path, to }) },
    }
}

/// Every path under home a settings file's hooks run, once each (hooks.ts:98).
pub fn hook_files(hooks: HookCarry, text: &str, home: &str) -> Result<Vec<String>, HostError> {
    let carried = hooks.carry(text, home, &|abs: &str| Some(abs.to_string()))?;
    let prefix = format!("{home}/");
    Ok(carried.carried.into_iter().filter(|c| c.from.starts_with(&prefix)).map(|c| c.from).collect())
}

/// The command line cut into whitespace runs and words: a quoted span keeps its spaces, and a `;` glued to a word is its
/// own token, so the tokens join back to the line (hooks.ts:50).
fn tokens(line: &str) -> Vec<&str> {
    let cs: Vec<(usize, char)> = line.char_indices().collect();
    let at = |k: usize| cs.get(k).map_or(line.len(), |c| c.0);
    let mut out = Vec::new();
    let mut i = 0;
    while i < cs.len() {
        let mut j = i;
        if js_space(cs[i].1) {
            while j < cs.len() && js_space(cs[j].1) {
                j += 1;
            }
            out.push(&line[at(i)..at(j)]);
        } else {
            let mut quote: Option<char> = None;
            while j < cs.len() && (quote.is_some() || !js_space(cs[j].1)) {
                let c = cs[j].1;
                if quote.is_none() && (c == '"' || c == '\'') {
                    quote = Some(c);
                } else if Some(c) == quote {
                    quote = None;
                }
                j += 1;
            }
            let word = &line[at(i)..at(j)];
            match word.strip_suffix(';') {
                Some(head) if !head.is_empty() && !head.ends_with(['"', '\'']) => {
                    out.push(head);
                    out.push(";");
                }
                _ => out.push(word),
            }
        }
        i = j;
    }
    out
}

/// `NAME=` at the start of a word, an environment assignment (hooks.ts:79).
fn is_env_assign(word: &str) -> bool {
    let bytes = word.as_bytes();
    let name_len = bytes.iter().take_while(|b| b.is_ascii_alphanumeric() || **b == b'_').count();
    name_len > 0 && !bytes[0].is_ascii_digit() && bytes.get(name_len) == Some(&b'=')
}

/// The quote a token is wrapped in, when it is wrapped in one.
fn quote_of(token: &str) -> Option<char> {
    let first = token.chars().next()?;
    (token.len() >= 2 && (first == '"' || first == '\'') && token.ends_with(first)).then_some(first)
}

/// The command with the first word of each simple command, and the script an interpreter runs, rewritten to its guest
/// path or left as written where the machine has it; the first word the placer refuses. Arguments, redirect targets and
/// everything else pass through (hooks.ts:107).
fn rewrite_command(command: &str, home: &str, place: HookPlacer<'_>) -> Rewritten {
    let mut carried = Vec::new();
    let mut expect = Expect::Command;
    let mut refused: Option<String> = None;
    let mut joined = String::new();
    for token in tokens(command) {
        if token.starts_with(js_space) {
            joined.push_str(token);
            continue;
        }
        if OPERATORS.contains(&token) {
            expect = Expect::Command;
            joined.push_str(token);
            continue;
        }
        if expect == Expect::Args {
            joined.push_str(token);
            continue;
        }
        let quote = quote_of(token);
        let word = match quote {
            Some(_) => &token[1..token.len() - 1],
            None => token,
        };
        if is_env_assign(word) || (expect == Expect::Script && word.starts_with('-')) {
            joined.push_str(token);
            continue;
        }
        expect = if runs_a_file(word, home) { Expect::Script } else { Expect::Args };
        match place_word(word, home, place) {
            PlacedWord::Left(left) => {
                if refused.is_none() {
                    refused = Some(left);
                }
                joined.push_str(token);
            }
            PlacedWord::Word { word: placed, carried: moved } => {
                carried.extend(moved);
                if placed == word {
                    joined.push_str(token);
                } else {
                    let q = quote.map(String::from).unwrap_or_default();
                    joined.push_str(&format!("{q}{placed}{q}"));
                }
            }
        }
    }
    match refused {
        Some(left) => Rewritten::Left(left),
        None => Rewritten::Done { command: joined, carried },
    }
}

/// The path of a hook, or of its command field, under `hooks`.
fn hook_path(event: &str, group: usize, hook: usize, field: Option<&str>) -> Vec<PathSeg> {
    let mut path = vec![
        PathSeg::Key("hooks".to_string()),
        PathSeg::Key(event.to_string()),
        PathSeg::Index(group as i64),
        PathSeg::Key("hooks".to_string()),
        PathSeg::Index(hook as i64),
    ];
    if let Some(field) = field {
        path.push(PathSeg::Key(field.to_string()));
    }
    path
}

/// Claude Code's hooks: `hooks` maps an event to groups, each with a matcher and its `hooks`, each a shell command or a
/// prompt (hooks.ts:132). Rewrites first, then each removal from the last index back, so every edit's path still names
/// what it did.
fn carry_claude(text: &str, home: &str, place: HookPlacer<'_>) -> Result<CarriedHooks, HostError> {
    let mut out = CarriedHooks { text: text.to_string(), carried: vec![], left: vec![] };
    let Ok(parsed) = read_jsonc(text) else {
        return Ok(out);
    };
    let Some(hooks) = parsed.as_object().and_then(|o| o.get("hooks")).and_then(Value::as_object) else {
        return Ok(out);
    };
    let mut seen: HashSet<String> = HashSet::new();
    let mut rewrites: Vec<JsonEdit> = Vec::new();
    let mut removals: Vec<JsonEdit> = Vec::new();
    let mut events_left = 0;
    for (event, groups) in js_entries(hooks) {
        let group_list: &[Value] = groups.as_array().map_or(&[], Vec::as_slice);
        let mut gone: Vec<JsonEdit> = Vec::new();
        let mut groups_left = 0;
        for (g, group) in group_list.iter().enumerate() {
            let mut cut: Vec<JsonEdit> = Vec::new();
            let mut hooks_left = 0;
            let hook_list: &[Value] = group.as_object().and_then(|o| o.get("hooks")).and_then(Value::as_array).map_or(&[], Vec::as_slice);
            for (h, hook) in hook_list.iter().enumerate() {
                let Some(command) = hook.as_object().and_then(|o| o.get("command")).and_then(Value::as_str) else {
                    hooks_left += 1;
                    continue;
                };
                match rewrite_command(command, home, place) {
                    Rewritten::Left(word) => {
                        out.left.push(word);
                        cut.insert(0, (hook_path(event, g, h, None), None));
                    }
                    Rewritten::Done { command: rewritten, carried } => {
                        hooks_left += 1;
                        if rewritten != command {
                            rewrites.push((hook_path(event, g, h, Some("command")), Some(Value::String(rewritten))));
                        }
                        for c in carried {
                            if seen.insert(c.from.clone()) {
                                out.carried.push(c);
                            }
                        }
                    }
                }
            }
            let emptied = hooks_left == 0 && !cut.is_empty();
            if !emptied {
                groups_left += 1;
            }
            let add: Vec<JsonEdit> = if emptied { vec![(hook_path_of_group(event, g), None)] } else { cut };
            gone.splice(0..0, add);
        }
        let emptied = groups.is_array() && groups_left == 0 && !gone.is_empty();
        if !emptied {
            events_left += 1;
        }
        if emptied {
            removals.push((vec![PathSeg::Key("hooks".to_string()), PathSeg::Key(event.to_string())], None));
        } else {
            removals.extend(gone);
        }
    }
    let edits: Vec<JsonEdit> = if events_left == 0 && !removals.is_empty() {
        rewrites.into_iter().chain(std::iter::once((vec![PathSeg::Key("hooks".to_string())], None))).collect()
    } else {
        rewrites.into_iter().chain(removals).collect()
    };
    if !edits.is_empty() {
        out.text = edit_json(text, &edits)?;
    }
    Ok(out)
}

/// The path of one group under `hooks`.
fn hook_path_of_group(event: &str, group: usize) -> Vec<PathSeg> {
    vec![PathSeg::Key("hooks".to_string()), PathSeg::Key(event.to_string()), PathSeg::Index(group as i64)]
}

/// Whether a code line sets the top-level `notify` key (codex-hooks.ts:15).
fn is_notify(code: &str) -> bool {
    js_trim_start(code).strip_prefix("notify").is_some_and(|rest| js_trim_start(rest).starts_with('='))
}

/// Whether a line is a comment line (codex-hooks.ts:16).
fn is_comment(line: &str) -> bool {
    js_trim_start(line).starts_with('#')
}

/// What carrying a `notify` program does: the line leaves, or its one word moves (codex-hooks.ts:24).
enum Notify {
    Left(String),
    Moved { words: Vec<String>, carried: Carried },
}

/// The argv Codex runs after a turn: the program, then its arguments. A program that names a path has to be one the image
/// has; when the program is an interpreter the machine has, the file it runs travels if the placer takes it (codex-hooks.ts:24).
fn carry_notify(words: &[String], home: &str, place: HookPlacer<'_>) -> Option<Notify> {
    let program = words.first()?;
    if let Some(path) = path_of(program, home) {
        if !on_machine(&path) {
            return Some(Notify::Left(program.clone()));
        }
    }
    if !runs_a_file(program, home) {
        return None;
    }
    let at = words.iter().enumerate().position(|(i, w)| i > 0 && !w.starts_with('-'))?;
    match place_word(&words[at], home, place) {
        PlacedWord::Left(word) => Some(Notify::Left(word)),
        PlacedWord::Word { carried: None, .. } => None,
        PlacedWord::Word { word, carried: Some(carried) } => {
            let mut moved = words.to_vec();
            moved[at] = word;
            Some(Notify::Moved { words: moved, carried })
        }
    }
}

/// Codex's global config: its one hook is the top-level `notify`, read line by line so every other key, comment and
/// spacing survives (codex-hooks.ts:40).
fn carry_codex(text: &str, home: &str, place: HookPlacer<'_>) -> CarriedHooks {
    let lines: Vec<&str> = text.split('\n').collect();
    let mut kept: Vec<String> = Vec::new();
    let mut carried: Vec<Carried> = Vec::new();
    let mut left: Vec<String> = Vec::new();
    let mut table = false;
    let mut changed = false;
    let mut i = 0;
    while i < lines.len() {
        let code = uncomment_toml(lines[i]);
        // Only the `notify` written before the first table header is the program; one under a table belongs to that table.
        if js_trim(&code).starts_with('[') {
            table = true;
        }
        if table || !is_notify(&code) {
            kept.push(lines[i].to_string());
            i += 1;
            continue;
        }
        let mut span = vec![lines[i].to_string()];
        let mut value = js_trim(code.find('=').map_or("", |p| &code[p + 1..])).to_string();
        while toml_open_array(&value) && i + 1 < lines.len() {
            i += 1;
            span.push(lines[i].to_string());
            let piece = uncomment_toml(lines[i]);
            value.push_str(js_trim(&piece));
        }
        // An array that never closed is not TOML this reads, so it stays as written.
        let words = if toml_open_array(&value) { Vec::new() } else { toml_strings(&value) };
        match carry_notify(&words, home, place) {
            None => kept.extend(span),
            Some(Notify::Left(program)) => {
                changed = true;
                left.push(program);
                // The comment lines above a key document that key, so they go with it.
                while kept.last().is_some_and(|l| is_comment(l)) {
                    kept.pop();
                }
            }
            Some(Notify::Moved { words: moved, carried: c }) => {
                changed = true;
                carried.push(c);
                let swap: HashMap<String, String> =
                    words.iter().zip(&moved).filter(|(w, m)| w != m).map(|(w, m)| (w.clone(), m.clone())).collect();
                kept.extend(
                    span.iter().map(|line| rewrite_toml_line(line, &|v: &str| swap.get(v).cloned().unwrap_or_else(|| v.to_string()))),
                );
            }
        }
        i += 1;
    }
    CarriedHooks { text: if changed { kept.join("\n") } else { text.to_string() }, carried, left }
}
