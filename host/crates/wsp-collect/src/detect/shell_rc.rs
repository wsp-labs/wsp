// SPDX-License-Identifier: AGPL-3.0-only
//! The rc files as the pack and the logins rung read them: which files they are, which variable names mean a secret,
//! the simple commands of a line and the variables each assigns, and the files a line sources. Values never leave
//! here. Port of packages/collect/src/detect/shell-rc.ts:1-205 and :525-563 at a3b870a6e; the spec is
//! spec/shell_rc.md. `stripExports` is the unit shell_rc_strip.
//!
//! Every regex source below is generated from the TS by evaluating its own definitions. Each is a JavaScript regex:
//! `\s` is JavaScript's set (wsp_words::js::is_js_whitespace), `[^]` is any character, and GUARD needs lookbehind.
pub use crate::detect::shell_rc_strip::{strip_exports, Stripped};
use std::sync::LazyLock;

use fancy_regex::{Captures, Regex};
use wsp_words::js::{is_js_whitespace, js_trim, JS_WS};

pub const FISH_CONF_D: &str = ".config/fish/conf.d";
pub const RC_PATHS: [&str; 11] = [
    ".zshrc",
    ".zshenv",
    ".zprofile",
    ".zlogin",
    ".bashrc",
    ".bash_profile",
    ".profile",
    ".inputrc",
    ".aliases",
    ".zsh_aliases",
    ".config/fish/config.fish",
];
pub const RC_NAMES: [&str; 11] = [
    ".zshrc",
    ".zshenv",
    ".zprofile",
    ".zlogin",
    ".bashrc",
    ".bash_profile",
    ".profile",
    ".inputrc",
    ".aliases",
    ".zsh_aliases",
    "config.fish",
];
pub const SECRET_WORDS: [&str; 8] = ["KEY", "APIKEY", "TOKEN", "SECRET", "PASSWORD", "PASSWD", "PASS", "PASSPHRASE"];
/// JavaScript regex source.
pub const SECRET_END: &str = r#"(?:APIKEY|TOKEN|SECRET|PASSWORD|PASSWD|PASSPHRASE)$"#;
/// JavaScript regex source.
pub const PLAIN_PASS: &str =
    r#"^(?:ASK|BY|ANTI|DE|OUT|OVER|SUB|SUR|TO|UN|UNDER|(?:BE|EN|GYRO|OUT|PRE|RE|UN)?COM|(?:COUNTER|NON)?TRES)PASS$"#;
pub const SECRET_LAST: [&str; 3] = ["PWD", "AUTH", "PAT"];
/// JavaScript regex source.
pub const SETTING: &str = r#"^(?:FISH_KEY_BINDINGS|FISH_SEQUENCE_KEY_DELAY_MS|DISABLE_FZF_KEY_BINDINGS|TIKTOKEN_CACHE_DIR|CLAUDE_CODE_SKIP_\w+_AUTH|PASSWORD_STORE_(?:DIR|KEY|SIGNING_KEY|GPG_OPTS|X_SELECTION|CLIP_TIME|UMASK|GENERATED_LENGTH|CHARACTER_SET|CHARACTER_SET_NO_SYMBOLS|ENABLE_EXTENSIONS|EXTENSIONS_DIR))$"#;
pub const NAME: &str = "[A-Za-z_][A-Za-z0-9_]*";
/// JavaScript regex source.
pub const EXPORTED: &str =
    r#"^\s*\\?(?:export|typeset|declare|readonly|local|integer|float|private|env|sudo|doas|nohup|nice)(?:\s+-\w+)*\s+(.*)$"#;
/// JavaScript regex source.
pub const FISH_SET: &str = r#"^\s*set((?:\s+--?[\w-]+)*)\s+([A-Za-z_][A-Za-z0-9_]*)(?:\s|$)"#;
/// JavaScript regex source.
pub const FISH_READS: &str = r#"\s(?:-[A-Za-z]*[eqnS][A-Za-z]*|--(?:erase|query|names|show))(?=\s|$)"#;
/// JavaScript regex source.
pub const ASSIGNED: &str = r#"^([A-Za-z_][A-Za-z0-9_]*)\+?="#;
pub const PRECOMMAND: &str = "\\\\?(?:!|-|builtin|noglob|nocorrect|and|or|not|coproc|repeat\\s+\\S+|time(?:\\s+-p)?|command(?:\\s+-p)?|exec(?:\\s+-[cl]*a\\s+\\S+|\\s+-[cl]+)*)";
/// JavaScript regex source.
pub const KEYWORDS: &str = r#"^\s*(?:then|do|else|elif|if|while|until|\\?(?:!|-|builtin|noglob|nocorrect|and|or|not|coproc|repeat\s+\S+|time(?:\s+-p)?|command(?:\s+-p)?|exec(?:\s+-[cl]*a\s+\S+|\s+-[cl]+)*))\s+"#;
/// JavaScript regex source.
pub const ASSIGNS: &str =
    r#"^(?:(?:export|typeset|declare|readonly|local|set)\s|[A-Za-z_][A-Za-z0-9_]*\+?=(?:"[^"$`]*"|'[^']*'|[^\s"'$`])*$)"#;
/// JavaScript regex source.
pub const GUARD: &str = r#"(?<=^|[\s;&|({])(if|elif|while|until)\s+(?=(?:\\?(?:!|-|builtin|noglob|nocorrect|and|or|not|coproc|repeat\s+\S+|time(?:\s+-p)?|command(?:\s+-p)?|exec(?:\s+-[cl]*a\s+\S+|\s+-[cl]+)*)\s+)*$)"#;
/// JavaScript regex source.
pub const ABBR_VALUE: &str = r#"^(?:-[^-prcf]*[prcf]|--(?:position|regex|command|function))$"#;
/// JavaScript regex source, flags `g`.
pub const QUOTED: &str = r#"\$'(?:[^'\\]|\\[^])*'?|\\[^]|"(?:[^"\\]|\\[^])*"?|'[^']*'?"#;
/// JavaScript regex source.
pub const HEREDOC: &str = r#"^<<-?\s*(?:"([A-Za-z_]\w*)"|'([A-Za-z_]\w*)'|\\([A-Za-z_]\w*)|([A-Za-z_]\w*))"#;
/// JavaScript regex source.
pub const SOURCE: &str = r#"^([ \t]*)(source|\\?\.)[ \t]+("[^"]*"|'[^']*'|[^\s;&|<>()"'\\`]+)(?=[\s;&|<>()]|$)(.*)$"#;

/// Compiles a JavaScript regex source for fancy-regex: `\s`, `\S`, `\w`, `\d`, `\b` and `.` keep JavaScript's meaning
/// and `[^]` is any character. A search never stops at fancy-regex's backtrack limit: node has none, so a long file
/// is answered, not refused.
pub(crate) fn js_re(source: &str) -> Regex {
    unbounded(&translate(source, false)).expect("a JavaScript pattern compiles")
}

/// A fancy-regex pattern with no backtrack limit, as a JavaScript engine has none.
pub(crate) fn unbounded(pattern: &str) -> Result<Regex, fancy_regex::Error> {
    fancy_regex::RegexBuilder::new(pattern).backtrack_limit(usize::MAX).build()
}

/// The same translation for the `regex` crate, which runs in linear time and never errors mid-search; only for a source
/// with no lookaround or backreference.
pub(crate) fn js_plain(source: &str) -> regex::Regex {
    regex::Regex::new(&translate(source, true)).expect("a JavaScript pattern compiles")
}

fn translate(source: &str, plain: bool) -> String {
    let mut out = String::new();
    let mut class = false;
    let mut chars = source.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let Some(n) = chars.next() else {
                    out.push_str("\\\\");
                    break;
                };
                match (n, class) {
                    ('s', false) => out.push_str(&format!("[{JS_WS}]")),
                    ('s', true) => out.push_str(JS_WS),
                    ('S', false) => out.push_str(&format!("[^{JS_WS}]")),
                    ('w', false) => out.push_str("[A-Za-z0-9_]"),
                    ('w', true) => out.push_str("A-Za-z0-9_"),
                    ('d', false) => out.push_str("[0-9]"),
                    ('d', true) => out.push_str("0-9"),
                    ('b', false) if plain => out.push_str("(?-u:\\b)"),
                    ('b', false) => out.push_str("(?:(?<=[A-Za-z0-9_])(?![A-Za-z0-9_])|(?<![A-Za-z0-9_])(?=[A-Za-z0-9_]))"),
                    _ => {
                        out.push('\\');
                        out.push(n);
                    }
                }
            }
            '[' if !class => {
                if chars.peek() == Some(&'^') {
                    chars.next();
                    if chars.peek() == Some(&']') {
                        chars.next();
                        out.push_str("(?s:.)");
                        continue;
                    }
                    class = true;
                    out.push_str("[^");
                } else {
                    class = true;
                    out.push('[');
                }
            }
            '[' => out.push_str("\\["),
            ']' => {
                class = false;
                out.push(']');
            }
            '.' if !class => out.push_str("[^\\n\\r\\x{2028}\\x{2029}]"),
            '&' | '~' if class => {
                out.push('\\');
                out.push(c);
            }
            _ => out.push(c),
        }
    }
    out
}

/// `re.test(text)`, false where the pattern cannot run.
pub(crate) fn is_match(re: &Regex, text: &str) -> bool {
    re.is_match(text).unwrap_or(false)
}

/// The groups of the first match of `re` at the start of `text` (`re.exec`).
pub(crate) fn captures<'t>(re: &Regex, text: &'t str) -> Option<Captures<'t>> {
    re.captures(text).ok().flatten()
}

/// Group `k` of a match, `""` where it did not take part.
pub(crate) fn group(caps: &Captures<'_>, k: usize) -> String {
    caps.get(k).map_or_else(String::new, |m| m.as_str().to_string())
}

/// The lines of `text` as `text.split(/\r?\n/)` gives them.
pub(crate) fn split_lines(text: &str) -> Vec<&str> {
    let parts: Vec<&str> = text.split('\n').collect();
    let last = parts.len() - 1;
    parts.iter().enumerate().map(|(k, l)| if k < last { l.strip_suffix('\r').unwrap_or(l) } else { l }).collect()
}

static SETTING_RE: LazyLock<Regex> = LazyLock::new(|| js_re(SETTING));
static SECRET_END_RE: LazyLock<Regex> = LazyLock::new(|| js_re(SECRET_END));
static PLAIN_PASS_RE: LazyLock<Regex> = LazyLock::new(|| js_re(PLAIN_PASS));
static EXPORTED_RE: LazyLock<Regex> = LazyLock::new(|| js_re(EXPORTED));
static FISH_SET_RE: LazyLock<Regex> = LazyLock::new(|| js_re(FISH_SET));
static FISH_READS_RE: LazyLock<Regex> = LazyLock::new(|| js_re(FISH_READS));
static ASSIGNED_RE: LazyLock<Regex> = LazyLock::new(|| js_re(ASSIGNED));
static ABBR_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| js_re(ABBR_VALUE));
static OPTION_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^[-+]."));
static SOURCE_RE: LazyLock<Regex> = LazyLock::new(|| js_re(SOURCE));
static QUOTE_WRAP_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r#"^(["']).*\1$"#));
pub(crate) static KEYWORDS_RE: LazyLock<Regex> = LazyLock::new(|| js_re(KEYWORDS));
pub(crate) static ASSIGNS_RE: LazyLock<Regex> = LazyLock::new(|| js_re(ASSIGNS));
pub(crate) static GUARD_RE: LazyLock<Regex> = LazyLock::new(|| js_re(GUARD));
pub(crate) static QUOTED_RE: LazyLock<regex::Regex> = LazyLock::new(|| js_plain(QUOTED));
pub(crate) static HEREDOC_RE: LazyLock<Regex> = LazyLock::new(|| js_re(HEREDOC));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCommand {
    pub indent: String,
    pub word: String,
    /// As written, quotes included.
    pub token: String,
    /// Everything after the token.
    pub rest: String,
}

/// Whether a `~`-relative path is an rc file: one of RC_PATHS, or a .fish file directly in conf.d.
pub fn is_rc_path(rel: &str) -> bool {
    if RC_PATHS.contains(&rel) {
        return true;
    }
    rel.rfind('/').is_some_and(|at| &rel[..at] == FISH_CONF_D) && rel.ends_with(".fish")
}

/// The rc files as `~`-relative paths: RC_PATHS and the .fish files in conf.d's listing.
pub fn rc_files(conf_d: &[String]) -> Vec<String> {
    let mut out: Vec<String> = RC_PATHS.iter().map(|p| p.to_string()).collect();
    out.extend(conf_d.iter().filter(|n| n.ends_with(".fish")).map(|n| format!("{FISH_CONF_D}/{n}")));
    out
}

/// A secret's word alone between underscores, any case; `KEYTIMEOUT` and `HISTKEY` are no key. A glued word, a PASS
/// ending, or a last `PWD`, `AUTH` or `PAT` counts only in capitals, as a program reads `PGPASSWORD`, `SSHPASS` and
/// `GITHUB_PAT`: in shell code `ctoken` is a word of the command line, `old_pwd` a folder and `_target_opt_pat` a pattern.
pub fn is_secret_name(name: &str) -> bool {
    let upper = name.to_uppercase();
    if is_match(&SETTING_RE, &upper) {
        return false;
    }
    let parts: Vec<&str> = upper.split('_').filter(|w| !w.is_empty()).collect();
    if parts.iter().any(|w| SECRET_WORDS.contains(w)) {
        return true;
    }
    if name != upper {
        return false;
    }
    let last = parts.last().copied().unwrap_or("");
    parts.iter().any(|w| is_match(&SECRET_END_RE, w))
        || (last.ends_with("PASS") && !is_match(&PLAIN_PASS_RE, last))
        || (parts.len() > 1 && SECRET_LAST.contains(&last))
}

/// The simple commands of a line: split outside quotes on `;`, `&&`, `||`, `|`, `&` and braces or parentheses, comment dropped.
pub fn simple_commands(line: &str) -> Vec<String> {
    let chars: Vec<char> = line.chars().collect();
    let mut out = Vec::new();
    let mut quote: Option<char> = None;
    let mut cur = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == '\\' && quote != Some('\'') {
            cur.push(c);
            if let Some(&next) = chars.get(i + 1) {
                cur.push(next);
            }
            i += 2;
            continue;
        }
        if let Some(q) = quote {
            cur.push(c);
            if c == q {
                quote = None;
            }
        } else if c == '\'' || c == '"' {
            quote = Some(c);
            cur.push(c);
        } else {
            if comment_at(&chars, i) {
                break;
            }
            if ";|&{}()".contains(c) {
                out.push(std::mem::take(&mut cur));
            } else {
                cur.push(c);
            }
        }
        i += 1;
    }
    out.push(cur);
    out.iter().map(|command| skip_keywords(command)).filter(|s| !js_trim(s).is_empty()).collect()
}

pub(crate) fn skip_keywords(command: &str) -> String {
    let mut t = command;
    while let Some(m) = KEYWORDS_RE.find(t).ok().flatten() {
        t = &t[m.end()..];
    }
    t.to_string()
}

/// A simple command's words as the shell hands them over: split on blanks outside quotes, each quote taken off.
pub(crate) fn shell_words(command: &str) -> Vec<String> {
    let chars: Vec<char> = command.chars().collect();
    let mut words = Vec::new();
    let mut cur = String::new();
    let mut in_word = false;
    let mut quote = "";
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        if quote == "'" || (quote == "$'" && c != '\\') {
            if c == '\'' {
                quote = "";
            } else {
                cur.push(c);
            }
        } else if c == '\\' {
            match next {
                Some(n) if quote == "\"" && !matches!(n, '"' | '\\' | '$' | '`') => {
                    cur.push(c);
                    cur.push(n);
                }
                Some(n) => cur.push(n),
                None => {}
            }
            in_word = true;
            i += 2;
            continue;
        } else if quote == "\"" {
            if c == '"' {
                quote = "";
            } else {
                cur.push(c);
            }
        } else if is_js_whitespace(c) {
            if in_word {
                words.push(std::mem::take(&mut cur));
            }
            cur.clear();
            in_word = false;
        } else {
            in_word = true;
            if c == '$' && next == Some('\'') {
                quote = "$'";
                i += 2;
                continue;
            } else if c == '\'' {
                quote = "'";
            } else if c == '"' {
                quote = "\"";
            } else {
                cur.push(c);
            }
        }
        i += 1;
    }
    if in_word {
        words.push(cur);
    }
    words
}

/// The text an `alias` (bash, zsh, fish) or fish `abbr` would run: each `name=body`'s body, and fish's `name body...`.
pub(crate) fn alias_bodies(words: &[String]) -> Vec<String> {
    let mut args: Vec<String> = Vec::new();
    let mut i = 1;
    while i < words.len() {
        let w = &words[i];
        if w == "--" {
            args.extend(words[i + 1..].iter().cloned());
            break;
        }
        if is_match(&OPTION_RE, w) {
            if words[0] == "abbr" && is_match(&ABBR_VALUE_RE, w) {
                i += 1;
            }
        } else {
            args.push(w.clone());
        }
        i += 1;
    }
    let mut bodies: Vec<String> = args.iter().filter_map(|a| a.split_once('=').map(|(_, body)| body.to_string())).collect();
    if args.len() > 1 && !args[0].contains('=') {
        bodies.push(args[1..].join(" "));
    }
    bodies
}

/// The leading assignments, then the rest (TS returns the pair as a tuple).
pub(crate) fn command_words(command: &str) -> (Vec<String>, Vec<String>) {
    let mut words = shell_words(command);
    match words.iter().position(|w| !is_match(&ASSIGNED_RE, w)) {
        None => (words, Vec::new()),
        Some(at) => {
            let rest = words.split_off(at);
            (words, rest)
        }
    }
}

/// The name a word assigns (`NAME=` or `NAME+=`), when it is an assignment.
fn assigned_name(word: &str) -> Option<String> {
    captures(&ASSIGNED_RE, word).map(|caps| group(&caps, 1))
}

/// A piece of shell text still to read for the names it assigns.
enum Work {
    Text(String),
    Command(String),
}

/// The names under `first`, in the order the TS recursion lists them: a command's own leading names, then each text it
/// runs again (an eval's arguments, an alias or abbr body) before the next command. The nesting is walked on a stack
/// of the heap, not of the thread, so a line of a thousand `eval` words is answered as node answers it at 500.
fn assigned_walk(first: Work) -> Vec<String> {
    let mut names = Vec::new();
    let mut stack = vec![vec![first].into_iter()];
    while let Some(top) = stack.last_mut() {
        let Some(work) = top.next() else {
            stack.pop();
            continue;
        };
        match work {
            Work::Text(text) => {
                let commands: Vec<Work> = text.split('\n').flat_map(simple_commands).map(Work::Command).collect();
                stack.push(commands.into_iter());
            }
            Work::Command(command) => {
                let (own, again) = assigned_head(&command);
                names.extend(own);
                if !again.is_empty() {
                    stack.push(again.into_iter().map(Work::Text).collect::<Vec<_>>().into_iter());
                }
            }
        }
    }
    names
}

/// The variables a simple command assigns, each of its leading assignments among them. A quoted builtin or name is read
/// as the bare one, and eval's arguments, joined by blanks as eval joins them, and an alias or abbr body are shell again.
pub(crate) fn assigned_in(command: &str) -> Vec<String> {
    assigned_walk(Work::Command(command.to_string()))
}

/// What one command assigns itself, and the texts it runs again.
fn assigned_head(command: &str) -> (Vec<String>, Vec<String>) {
    let (lead, run) = command_words(command);
    let leading: Vec<String> = lead.iter().filter_map(|w| assigned_name(w)).collect();
    match run.first().map(String::as_str) {
        Some("eval") => return (leading, vec![run[1..].join(" ")]),
        Some("alias" | "abbr") => return (leading, alias_bodies(&run)),
        _ => {}
    }
    let words = shell_words(&newline_as_space(command));
    let bare = skip_keywords(&words.join(" "));
    if let Some(fish) = captures(&FISH_SET_RE, &bare) {
        return (if is_match(&FISH_READS_RE, &group(&fish, 1)) { Vec::new() } else { vec![group(&fish, 2)] }, Vec::new());
    }
    if let Some(exported) = captures(&EXPORTED_RE, &bare) {
        return (group(&exported, 1).split(is_js_whitespace).filter_map(assigned_name).collect(), Vec::new());
    }
    let mut out = Vec::new();
    if let Some(mut i) = (0..words.len()).find(|&k| words[k..].join(" ") == bare) {
        while let Some(name) = words.get(i).and_then(|w| assigned_name(w)) {
            out.push(name);
            i += 1;
        }
    }
    (out, Vec::new())
}

/// A `\` before a newline, or a bare newline, as a space (`command.replace(/\\?\n/g, " ")`).
fn newline_as_space(command: &str) -> String {
    let mut out = String::new();
    let mut chars = command.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' && chars.peek() == Some(&'\n') {
            chars.next();
            out.push(' ');
        } else if c == '\n' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}

/// `i` indexes the line's characters (`line.chars()`); every character it compares is ASCII but `\s`, which is
/// JavaScript's set, so a `#` after a no-break space starts a comment.
pub(crate) fn comment_at(line: &[char], i: usize) -> bool {
    if line.get(i) != Some(&'#') {
        return false;
    }
    i == 0
        || is_js_whitespace(line[i - 1])
        || matches!(line[i - 1], ';' | '&' | '|')
        || (i >= 2 && line[i - 2] == '=' && line[i - 1] == '(')
}

/// A command that reads a file: its `source` or `.` word (nvm writes `\.` to dodge an alias on the dot), the one token after it and what follows.
pub fn source_command(command: &str) -> Option<SourceCommand> {
    let caps = captures(&SOURCE_RE, command)?;
    Some(SourceCommand { indent: group(&caps, 1), word: group(&caps, 2), token: group(&caps, 3), rest: group(&caps, 4) })
}

/// The file a source token names, absolute, when the token is literal: `~/`, `$HOME/` and `${HOME}/` fold to home; a
/// variable elsewhere, a glob, a substitution, an escape or an empty, `.` or `..` segment is left to a running shell.
pub fn sourced_path(token: &str, home: &str) -> Option<String> {
    let raw = if is_match(&QUOTE_WRAP_RE, token) { &token[1..token.len() - 1] } else { token };
    let path = fold_home(raw, home);
    if !path.starts_with('/')
        || path.chars().any(|c| "$`*?[]\\".contains(c))
        || path.split('/').skip(1).any(|seg| matches!(seg, "" | "." | ".."))
    {
        return None;
    }
    Some(path)
}

/// `raw` with a leading `~`, `$HOME` or `${HOME}` that is followed by `/` replaced by home.
fn fold_home(raw: &str, home: &str) -> String {
    for prefix in ["~", "$HOME", "${HOME}"] {
        if let Some(rest) = raw.strip_prefix(prefix) {
            if rest.starts_with('/') {
                return format!("{home}{rest}");
            }
        }
    }
    raw.to_string()
}

/// The files an rc file reads with `source` or `.` by a literal path, anywhere on the line, in order, once each.
pub fn sourced_paths(text: &str, home: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for line in split_lines(text) {
        // simpleCommands splits on braces, so `${HOME}` is folded before the split.
        for cmd in simple_commands(&line.replace("${HOME}", "$HOME")) {
            let Some(src) = source_command(&cmd) else { continue };
            if let Some(p) = sourced_path(&src.token, home) {
                if !out.contains(&p) {
                    out.push(p);
                }
            }
        }
    }
    out
}
