// SPDX-License-Identifier: AGPL-3.0-only
//! The commands an rc file calls, and the guard the pack prepends for the ones the machine lacks: each such name a
//! function that runs the real command once there is one and returns 127 until then. Port of
//! packages/collect/src/detect/commands.ts at a3b870a6e; the spec is spec/commands.md.
use std::collections::HashSet;
use std::sync::LazyLock;

use fancy_regex::Regex;
use wsp_words::js::{is_js_whitespace, js_trim};

use crate::detect::shell_rc::{captures, group, is_match, js_plain, js_re, split_lines};
use crate::history::commands::{command_words, is_version_check, split_commands, without_heredocs};

pub const GUARD_BEGIN: &str = "# >>> wsp guard: commands this machine does not have >>>";
pub const GUARD_END: &str = "# <<< wsp guard <<<";

/// A name a function definition or a call may carry in zsh and bash, as a JavaScript regex source (commands.ts:15).
pub const WORD: &str = r"^[A-Za-z_][\w.+-]*$";

/// Builtins, reserved words and zsh's own functions: never a call to guard (commands.ts:20).
pub const RC_BUILTIN: [&str; 91] = [
    "autoload",
    "bg",
    "bindkey",
    "bye",
    "chdir",
    "compadd",
    "compdef",
    "compdescribe",
    "compinit",
    "complete",
    "compgen",
    "compopt",
    "coproc",
    "dirs",
    "disable",
    "disown",
    "echotc",
    "echoti",
    "emulate",
    "enable",
    "fc",
    "fg",
    "float",
    "functions",
    "getln",
    "getopts",
    "hash",
    "history",
    "in",
    "integer",
    "jobs",
    "let",
    "limit",
    "logout",
    "mapfile",
    "nocorrect",
    "noglob",
    "popd",
    "print",
    "private",
    "pushd",
    "pushln",
    "r",
    "readarray",
    "rehash",
    "repeat",
    "sched",
    "select",
    "setopt",
    "shopt",
    "suspend",
    "times",
    "ttyctl",
    "umask",
    "unalias",
    "unfunction",
    "unhash",
    "unlimit",
    "unsetopt",
    "vared",
    "whence",
    "where",
    "zcompile",
    "zformat",
    "zle",
    "zmodload",
    "zparseopts",
    "zprof",
    "zstyle",
    "add-zsh-hook",
    "add-zle-hook-widget",
    "bashcompinit",
    "colors",
    "compaudit",
    "is-at-least",
    "promptinit",
    "prompt",
    "run-help",
    "select-word-style",
    "vcs_info",
    "zargs",
    "zcalc",
    "zed",
    "zmv",
    "zrecompile",
    "bracketed-paste-magic",
    "url-quote-magic",
    "edit-command-line",
    "up-line-or-beginning-search",
    "down-line-or-beginning-search",
    "history-search-end",
];

/// `name() {` or `function name` at a line's start, as a JavaScript regex source with flags `gm` (commands.ts:29).
pub const FUNCTION_HEAD: &str = r"^\s*(?:function\s+)?([A-Za-z_][\w.+-]*)\s*\(\s*\)|^\s*function\s+([A-Za-z_][\w.+-]*)\b";

/// The shell a file is written for; fish is not shell and yields no calls.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Syntax {
    #[default]
    Sh,
    Fish,
}

/// What `drop_plugins` hands back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub text: String,
    pub dropped: Vec<String>,
}

static WORD_RE: LazyLock<Regex> = LazyLock::new(|| js_re(WORD));
/// FUNCTION_HEAD with its `^` the start of the text it is given: `function_heads` hands it one line start at a time.
static FUNCTION_HEAD_RE: LazyLock<regex::Regex> = LazyLock::new(|| js_plain(FUNCTION_HEAD));
static ALIAS_TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^([\w.+-]+)=(.*)$"));
static PLUGINS_OPEN_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^\s*plugins\+?=\("));
static COMMENT_AT_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"(^|\s)#"));
static WS_RUN_RE: LazyLock<regex::Regex> = LazyLock::new(|| js_plain(r"\s+"));

/// `text.split(/\s+/)`: the empty pieces at either end stay, as JavaScript keeps them.
fn split_ws(text: &str) -> Vec<String> {
    WS_RUN_RE.split(text).map(String::from).collect()
}

/// A line terminator in JavaScript's sense: where a `^` under the `m` flag can match next.
fn is_line_end(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// The first line start at or after `from`: the text's start, or the position after a line terminator.
fn line_start_from(text: &str, from: usize) -> Option<usize> {
    if from == 0 || text[..from].chars().next_back().is_some_and(is_line_end) {
        return Some(from);
    }
    let (at, c) = text[from..].char_indices().find(|&(_, c)| is_line_end(c))?;
    Some(from + at + c.len_utf8())
}

/// The names `text.matchAll(FUNCTION_HEAD)` captures, in order. The pattern is tried at one line start after another, as
/// `^` under `m` allows, and a search resumes where the last match ended. A start that fails cannot be followed by one
/// inside its own run of blanks, which would only reach the same place and fail there, so a long run of blank lines
/// costs one pass.
fn function_heads(text: &str) -> Vec<&str> {
    let mut names = Vec::new();
    let mut next = line_start_from(text, 0);
    while let Some(at) = next {
        let rest = &text[at..];
        match FUNCTION_HEAD_RE.captures(rest) {
            Some(caps) => {
                if let Some(name) = caps.get(1).or(caps.get(2)) {
                    names.push(name.as_str());
                }
                next = line_start_from(text, at + caps.get(0).map_or(0, |whole| whole.end()));
            }
            None => {
                let blanks = rest.char_indices().find(|&(_, c)| !is_js_whitespace(c)).map_or(rest.len(), |(i, _)| i);
                let past = at + blanks;
                next = match text[past..].chars().next() {
                    Some(c) => line_start_from(text, past + c.len_utf8()),
                    None => None,
                };
            }
        }
    }
    names
}

/// `text.split(/(\s+|[()])/).filter(p => p !== "")`: blank runs, single brackets and the runs between them, in order.
fn split_code(code: &str) -> Vec<String> {
    let mut parts: Vec<String> = Vec::new();
    for c in code.chars() {
        let bracket = matches!(c, '(' | ')');
        let extends = match parts.last().and_then(|last| last.chars().next()) {
            Some(l) => !bracket && !matches!(l, '(' | ')') && is_js_whitespace(l) == is_js_whitespace(c),
            None => false,
        };
        match parts.last_mut() {
            Some(last) if extends => last.push(c),
            _ => parts.push(c.to_string()),
        }
    }
    parts
}

/// A part that is only blanks (`/^\s+$/`).
fn is_blank(part: &str) -> bool {
    !part.is_empty() && part.chars().all(is_js_whitespace)
}

/// A character that may end an array's name before its `=(`: a word character, `]` or `+` (commands.ts:33-71).
fn is_array_head(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '_' | ']' | '+')
}

/// The text with every `name=( ... )` array literal emptied, across lines, quotes and comments respected: its words are values, not calls.
fn without_array_literals(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();
    let comment_at =
        |i: usize| chars[i] == '#' && (i == 0 || is_js_whitespace(chars[i - 1]) || matches!(chars[i - 1], ';' | '&' | '|' | '('));
    let mut out = String::new();
    let mut quote: Option<char> = None;
    let mut depth = 0usize;
    let mut i = 0;
    while i < n {
        let c = chars[i];
        if depth > 0 {
            if c == '\\' && quote != Some('\'') {
                i += 1;
            } else if quote.is_some() {
                if quote == Some(c) {
                    quote = None;
                }
            } else if c == '\'' || c == '"' {
                quote = Some(c);
            } else if comment_at(i) {
                i = chars[i..].iter().position(|&x| x == '\n').map_or(n, |p| i + p);
                continue;
            } else if c == '(' {
                depth += 1;
            } else if c == ')' {
                depth -= 1;
                if depth == 0 {
                    out.push(')');
                }
            }
            i += 1;
            continue;
        }
        if c == '\\' && quote != Some('\'') {
            out.push(c);
            if let Some(&next) = chars.get(i + 1) {
                out.push(next);
            }
            i += 2;
            continue;
        }
        if let Some(q) = quote {
            if c == q {
                quote = None;
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
        } else if comment_at(i) {
            let end = chars[i..].iter().position(|&x| x == '\n').map_or(n, |p| i + p);
            out.extend(&chars[i..end]);
            i = end;
            continue;
        } else if c == '(' && i >= 2 && chars[i - 1] == '=' && is_array_head(chars[i - 2]) {
            depth = 1;
            out.push(c);
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// The command a run of words calls, as the history parser reads it, when it is a plain name and no builtin.
fn call_of(command: &str) -> Option<String> {
    let words = command_words(command);
    let head = words.first()?;
    if is_version_check(&words) || !is_match(&WORD_RE, head) || RC_BUILTIN.contains(&head.as_str()) {
        return None;
    }
    if split_ws(command).iter().any(|w| w.ends_with(format!("/{head}").as_str())) {
        return None;
    }
    Some(head.clone())
}

/// The commands an rc file calls, once each in order of first call, read with the history parser: the first word of
/// every command, substitutions and pipelines included, past its environment assignments and wrappers; on top of
/// that, the first word of every alias body. Names the file itself defines as functions or aliases or marks for
/// autoload are its own and never listed; array literals are values; a fish file is not shell and yields nothing.
pub fn called_commands(text: &str, syntax: Syntax) -> Vec<String> {
    if syntax == Syntax::Fish {
        return Vec::new();
    }
    let mut called: Vec<String> = Vec::new();
    let mut own: HashSet<String> = HashSet::new();
    let mut push_call = |name: Option<String>| {
        if let Some(name) = name {
            if !called.contains(&name) {
                called.push(name);
            }
        }
    };
    for name in function_heads(text) {
        own.insert(name.to_string());
    }
    for cmd in split_commands(&without_heredocs(&without_array_literals(text))) {
        let words = split_ws(&cmd);
        if words[0] == "alias" {
            for (idx, w) in words.iter().enumerate().skip(1) {
                let Some(m) = captures(&ALIAS_TOKEN_RE, w) else { continue };
                own.insert(group(&m, 1));
                let rest = &words[idx + 1..];
                let next = rest.iter().position(|r| is_match(&ALIAS_TOKEN_RE, r)).unwrap_or(rest.len());
                let body = std::iter::once(group(&m, 2)).chain(rest[..next].iter().cloned()).collect::<Vec<_>>().join(" ");
                push_call(call_of(&body));
            }
        } else if words[0] == "autoload" {
            for w in &words[1..] {
                if !w.starts_with('-') {
                    own.insert(w.clone());
                }
            }
        } else {
            push_call(call_of(&cmd));
        }
    }
    called.into_iter().filter(|n| !own.contains(n)).collect()
}

/// The oh-my-zsh plugin list is a zsh array. A plugin whose tool is not on the image prints its own line at
/// every start and a no-op function does not satisfy its `$+commands` check, so the name leaves the list instead:
/// every `plugins=( )` and `plugins+=( )` region, one line or many, loses the names `drop` says, a line left with
/// nothing but its indent goes with them and a trailing comment stays. Every other byte is as it was.
pub fn drop_plugins(text: &str, drop: &dyn Fn(&str) -> bool) -> Dropped {
    let mut dropped: Vec<String> = Vec::new();
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut out: Vec<String> = Vec::new();
    let mut in_list = false;
    for line in split_lines(text) {
        let opens = !in_list && is_match(&PLUGINS_OPEN_RE, line);
        if !opens && !in_list {
            out.push(line.to_string());
            continue;
        }
        let (code, comment) = match captures(&COMMENT_AT_RE, line) {
            None => (line, ""),
            Some(m) => {
                let cut = m.get(0).map_or(0, |whole| whole.start()) + group(&m, 1).len();
                (&line[..cut], &line[cut..])
            }
        };
        in_list = !code.contains(')');
        let mut parts = split_code(code);
        let mut i: isize = 0;
        while i >= 0 && (i as usize) < parts.len() {
            let word = parts[i as usize].clone();
            if is_match(&WORD_RE, &word) && drop(word.as_str()) {
                if !dropped.contains(&word) {
                    dropped.push(word);
                }
                parts.remove(i as usize);
                if parts.get(i as usize).is_some_and(|p| is_blank(p)) {
                    parts.remove(i as usize);
                } else if i >= 1 && parts.get((i - 1) as usize).is_some_and(|p| is_blank(p)) {
                    parts.remove((i - 1) as usize);
                }
                i -= 1;
            }
            i += 1;
        }
        let kept: String = parts.concat();
        let indent: String = line.chars().take_while(|&c| is_js_whitespace(c)).collect();
        if js_trim(&kept).is_empty() && comment.is_empty() && !js_trim(code).is_empty() {
            continue;
        }
        out.push(if js_trim(&kept).is_empty() && !comment.is_empty() { format!("{indent}{comment}") } else { format!("{kept}{comment}") });
    }
    Dropped { text: out.join(eol), dropped }
}

/// The guard's notes: one line per plugin left out, then one naming what is guarded, so the file itself says why.
fn notes(missing: &[String], dropped: &[String]) -> Vec<String> {
    let mut out: Vec<String> = dropped.iter().map(|name| format!("# plugin {name} left out: {name} is not on the image")).collect();
    if !missing.is_empty() {
        out.push(format!(
            "# Guarded above: a call to one of these that is not on this machine is silent instead of an error: {}. Tick them in wsp init to install them.",
            missing.join(", ")
        ));
    }
    out
}

/// A command that is on the machine after all (installed by hand, by an agent, unknown to the planner, or on a directory the rc
/// itself adds to PATH below the block) always wins: the guard looks the name up at every call and hands over to the command
/// once there is one. `command -v` would find the guard itself, so the lookup runs in a subshell with the guard unset; a
/// function that unsets and redefines itself while it runs crashes zsh 5.9 (exit 139, measured), so the guard is only ever
/// unset on the way to the real command.
fn guard_line(name: &str) -> String {
    format!(
        "{name}() {{ if (unset -f {name}; command -v {name}) >/dev/null 2>&1; then unset -f {name}; {name} \"$@\"; else return 127; fi; }}"
    )
}

/// The text with the guard block prepended: a conditional silent no-op for each `missing` command and the notes for
/// those and for the plugins `dropped`. Any guard block an earlier pack wrote is taken out first, so a re-run
/// replaces it; with nothing to say the text carries no block. Line endings follow the file.
/// TS `dropped` defaults to `[]`; pass `&[]`.
pub fn guard_commands(text: &str, missing: &[String], dropped: &[String]) -> String {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let lines = split_lines(text);
    let begin = lines.iter().position(|l| *l == GUARD_BEGIN);
    let end = begin.and_then(|b| lines[b..].iter().position(|l| *l == GUARD_END).map(|p| b + p));
    let body: Vec<&str> = match (begin, end) {
        (Some(b), Some(e)) => {
            let from = if lines.get(e + 1) == Some(&"") { e + 2 } else { e + 1 };
            lines[..b].iter().chain(lines[from.min(lines.len())..].iter()).copied().collect()
        }
        _ => lines.clone(),
    };
    if missing.len() + dropped.len() == 0 {
        return body.join(eol);
    }
    let mut block: Vec<String> = vec![GUARD_BEGIN.to_string()];
    block.extend(missing.iter().map(|name| guard_line(name)));
    block.extend(notes(missing, dropped));
    block.push(GUARD_END.to_string());
    block.push(String::new());
    block.extend(body.iter().map(|l| l.to_string()));
    block.join(eol)
}
