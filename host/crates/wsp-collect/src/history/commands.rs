// SPDX-License-Identifier: AGPL-3.0-only
//! What a shell line says the agent ran, as names: the first word of each command in it and the packages its install
//! commands asked for. Heredoc bodies are cut before anything is read, quoted text is never split. Port of
//! packages/collect/src/history/commands.ts at a3b870a6e; the spec is spec/history_commands.md.
use std::sync::LazyLock;

use fancy_regex::Regex;
use wsp_words::js::{is_js_whitespace, js_trim, JS_WS};

pub const SEPARATORS: [&str; 6] = ["|", "||", "&&", ";", "&", "\n"];
const PREFIXES: [&str; 19] = [
    "sudo",
    "doas",
    "env",
    "time",
    "nohup",
    "exec",
    "command",
    "builtin",
    "nice",
    "xargs",
    "caffeinate",
    "then",
    "do",
    "else",
    "if",
    "elif",
    "while",
    "until",
    "!",
];
const NOT_COMMANDS: [&str; 34] = [
    "for", "case", "function", "fi", "done", "esac", "export", "local", "declare", "typeset", "readonly", "unset", "return", "exit",
    "break", "continue", "shift", "set", "cd", "echo", "printf", "true", "false", "[", "[[", "test", "wait", "trap", "source", ".",
    "alias", "eval", "read", ":",
];
const VERSION_FLAGS: [&str; 4] = ["--version", "-version", "-v", "-V"];
const COMMAND_LOOKUP_FLAGS: [&str; 2] = ["-v", "-V"];
const NPM_INSTALL: [&str; 3] = ["install", "i", "add"];

static ASSIGNMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^[A-Za-z_][A-Za-z0-9_]*=").unwrap());
static REDIRECT_ALONE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:[0-9]*|&)[<>]{1,2}&?$").unwrap());
static REDIRECT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(?:[0-9]*|&)[<>]").unwrap());
static HEREDOC: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!("<<-?[{JS_WS}]*(?:'([^']+)'|\"([^\"]+)\"|([A-Za-z0-9_]+))")).unwrap());
static VERSION_SEGMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^v[0-9]+$").unwrap());

fn matches(re: &Regex, text: &str) -> bool {
    re.is_match(text).unwrap_or(false)
}

fn is_assignment(word: &str) -> bool {
    matches(&ASSIGNMENT, word)
}

fn prefix_option_args(word: &str) -> &'static [&'static str] {
    match word {
        "sudo" => &["-u", "-g", "-h", "-p"],
        "doas" => &["-u"],
        "nice" => &["-n"],
        "xargs" => &["-I", "-n", "-P", "-d", "-L", "-s"],
        _ => &[],
    }
}

/// The line with every heredoc body removed: from the line after `<<TAG` to the line that is the tag.
pub fn without_heredocs(line: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let mut tag: Option<String> = None;
    for l in line.split('\n') {
        if let Some(t) = tag.take() {
            if js_trim(l) != t {
                tag = Some(t);
            }
            continue;
        }
        out.push(l);
        if let Ok(Some(m)) = HEREDOC.captures(l) {
            tag = m.get(1).or(m.get(2)).or(m.get(3)).map(|g| g.as_str().to_string());
        }
    }
    out.join("\n")
}

/// The index of the brace closing the one at `open`, nested braces skipped; the last index when none closes it.
fn closing_brace(c: &[char], open: usize) -> usize {
    let mut depth = 0i64;
    for (i, ch) in c.iter().enumerate().skip(open) {
        if *ch == '{' {
            depth += 1;
        } else if *ch == '}' {
            depth -= 1;
            if depth == 0 {
                return i;
            }
        }
    }
    c.len() - 1
}

fn flush(cur: &mut String, out: &mut Vec<String>) {
    let text = js_trim(cur);
    if !text.is_empty() {
        out.push(text.to_string());
    }
    cur.clear();
}

/// The line cut into commands at unquoted separators, subshells and substitutions opened; each command trimmed, empty
/// ones dropped.
pub fn split_commands(line: &str) -> Vec<String> {
    let c: Vec<char> = line.chars().collect();
    let n = c.len();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    let mut quote: Option<char> = None;
    let mut opened: Vec<Option<char>> = Vec::new();
    let mut i = 0;
    while i < n {
        let ch = c[i];
        let two: String = c[i..(i + 2).min(n)].iter().collect();
        if let Some(q) = quote {
            if ch == '\\' && q == '"' {
                cur.push(ch);
                if let Some(next) = c.get(i + 1) {
                    cur.push(*next);
                }
                i += 2;
            } else if q == '"' && two == "$(" {
                flush(&mut cur, &mut out);
                opened.push(quote);
                quote = None;
                i += 2;
            } else {
                if ch == q {
                    quote = None;
                } else {
                    cur.push(ch);
                }
                i += 1;
            }
            continue;
        }
        if ch == '\'' || ch == '"' {
            quote = Some(ch);
            i += 1;
            continue;
        }
        if ch == '\\' {
            if let Some(next) = c.get(i + 1) {
                cur.push(*next);
            }
            i += 2;
            continue;
        }
        if ch == '#' && (i == 0 || is_js_whitespace(c[i - 1]) || ";&|(){}".contains(c[i - 1])) {
            flush(&mut cur, &mut out);
            i = c[i..].iter().position(|x| *x == '\n').map_or(n, |p| i + p);
            continue;
        }
        if SEPARATORS.contains(&two.as_str()) || two == "$(" {
            flush(&mut cur, &mut out);
            if two == "$(" {
                opened.push(None);
            }
            i += 2;
            continue;
        }
        if two == "${" {
            let end = closing_brace(&c, i + 1);
            cur.extend(&c[i..=end]);
            i = end + 1;
            continue;
        }
        if ch == '(' || ch == ')' {
            flush(&mut cur, &mut out);
            if ch == '(' {
                opened.push(None);
            } else if let Some(q) = opened.pop() {
                quote = q;
            }
            i += 1;
            continue;
        }
        let prev = i.checked_sub(1).map(|p| c[p]);
        let redirect = ch == '&' && (prev == Some('>') || prev == Some('<') || c.get(i + 1) == Some(&'>'));
        if !redirect && matches!(ch, '|' | '&' | ';' | '\n' | '`' | '{' | '}') {
            flush(&mut cur, &mut out);
            i += 1;
            continue;
        }
        cur.push(ch);
        i += 1;
    }
    flush(&mut cur, &mut out);
    out
}

/// The words of one command, prefixes and their options skipped, assignments and redirections skipped, the head's
/// directory dropped; empty for shell syntax alone.
pub fn command_words(command: &str) -> Vec<String> {
    let words: Vec<&str> = command.split(is_js_whitespace).filter(|w| !w.is_empty()).collect();
    let mut i = 0;
    while i < words.len() {
        let w = words[i];
        if is_assignment(w) || matches(&REDIRECT, w) {
            i += if matches(&REDIRECT_ALONE, w) { 2 } else { 1 };
            continue;
        }
        if w == "command" && words.get(i + 1).is_some_and(|a| COMMAND_LOOKUP_FLAGS.contains(a)) {
            break;
        }
        if PREFIXES.contains(&w) {
            let with_arg = prefix_option_args(w);
            i += 1;
            while i < words.len() && (words[i].starts_with('-') || (w == "env" && is_assignment(words[i]))) {
                i += if with_arg.contains(&words[i]) { 2 } else { 1 };
            }
            continue;
        }
        break;
    }
    let rest = words.get(i..).unwrap_or(&[]);
    let Some(head) = rest.first() else { return Vec::new() };
    if NOT_COMMANDS.contains(head) {
        return Vec::new();
    }
    let name = &head[head.rfind('/').map_or(0, |p| p + 1)..];
    std::iter::once(name.to_string()).chain(rest[1..].iter().map(|w| w.to_string())).collect()
}

/// Whether one command's words only ask whether a program is there or which version it is; nothing was worked with.
pub fn is_version_check(words: &[String]) -> bool {
    let Some(head) = words.first() else { return false };
    let arg = words.get(1).map(String::as_str);
    if head == "which" {
        return true;
    }
    if head == "command" {
        return arg.is_some_and(|a| COMMAND_LOOKUP_FLAGS.contains(&a));
    }
    words.len() == 2 && arg.is_some_and(|a| VERSION_FLAGS.contains(&a))
}

/// The names of the programs a shell line runs, one per command in it, in order and with repeats; a version check
/// or a lookup is not a run.
pub fn command_names(line: &str) -> Vec<String> {
    split_commands(&without_heredocs(line))
        .iter()
        .flat_map(|c| {
            let words = command_words(c);
            if is_version_check(&words) {
                Vec::new()
            } else {
                words.into_iter().take(1).collect()
            }
        })
        .collect()
}

/// commands.ts:169.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Install {
    /// brew, npm, pnpm, bun, uv, pipx, cargo, go, apt; or a one-shot runner (npx, bunx, uvx, pipx-run).
    pub via: String,
    pub name: String,
}

/// `@scope/name@1.2.3` to `@scope/name`, `name@latest` to `name`, `pkg==1.0` to `pkg`, `mod/path@v1` to `path`.
fn bare_name(spec: &str, via: &str) -> String {
    if via == "go" {
        let s = match spec.find('@') {
            Some(at) => &spec[..at],
            None => spec,
        };
        let segment = &s[s.rfind('/').map_or(0, |p| p + 1)..];
        let segment = if matches(&VERSION_SEGMENT, segment) { "" } else { segment };
        return if segment.is_empty() { s.to_string() } else { segment.to_string() };
    }
    if matches!(via, "uv" | "pipx" | "uvx" | "pipx-run") {
        return spec.split(|c: char| "=<>!~[".contains(c)).next().unwrap_or_default().to_string();
    }
    let skip = spec.chars().next().map_or(0, char::len_utf8);
    match spec[skip..].find('@') {
        Some(at) => spec[..skip + at].to_string(),
        None => spec.to_string(),
    }
}

/// The words after `from` that are neither options nor redirections nor the target of a bare one.
fn positional(words: &[String], from: usize) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = from;
    while i < words.len() {
        let w = &words[i];
        if matches(&REDIRECT_ALONE, w) {
            i += 1;
        } else if !w.starts_with('-') && !matches(&REDIRECT, w) {
            out.push(w.clone());
        }
        i += 1;
    }
    out
}

/// The packages one command asks an installer for, with the installer; one-shot runs are listed under their runner.
pub fn installs_in(command: &str) -> Vec<Install> {
    let w = command_words(command);
    let Some(head) = w.first() else { return Vec::new() };
    let at = |i: usize, names: &[&str]| w.get(i).is_some_and(|x| names.contains(&x.as_str()));
    let global = w.iter().skip(2).any(|x| x == "-g" || x == "--global");
    let list = |via: &str, from: usize| -> Vec<Install> {
        positional(&w, from)
            .into_iter()
            .map(|name| Install { via: via.to_string(), name: bare_name(&name, via) })
            .filter(|i| !i.name.is_empty())
            .collect()
    };
    match head.as_str() {
        "brew" => {
            if at(1, &["install", "reinstall"]) {
                list("brew", 2)
            } else {
                Vec::new()
            }
        }
        "npm" => {
            if at(1, &NPM_INSTALL[..]) && global {
                list("npm", 2)
            } else {
                Vec::new()
            }
        }
        "pnpm" | "bun" => {
            if at(1, &["add", "install", "i"]) && global {
                list(head.as_str(), 2)
            } else {
                Vec::new()
            }
        }
        "uv" => {
            if at(1, &["tool"]) && at(2, &["install"]) {
                list("uv", 3)
            } else {
                Vec::new()
            }
        }
        "pipx" => {
            if at(1, &["install"]) {
                list("pipx", 2)
            } else if at(1, &["run"]) {
                list("pipx-run", 2).into_iter().take(1).collect()
            } else {
                Vec::new()
            }
        }
        "cargo" | "go" => {
            if at(1, &["install"]) {
                list(head.as_str(), 2)
            } else {
                Vec::new()
            }
        }
        "apt" | "apt-get" => {
            if at(1, &["install"]) {
                list("apt", 2)
            } else {
                Vec::new()
            }
        }
        "npx" | "bunx" | "uvx" => list(head.as_str(), 1).into_iter().take(1).collect(),
        _ => Vec::new(),
    }
}

/// Every install a shell line asks for, across its commands.
pub fn install_names(line: &str) -> Vec<Install> {
    split_commands(&without_heredocs(line)).iter().flat_map(|c| installs_in(c)).collect()
}
