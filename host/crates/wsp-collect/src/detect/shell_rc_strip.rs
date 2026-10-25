// SPDX-License-Identifier: AGPL-3.0-only
//! Cutting secrets out of a carried rc file: every line that assigns a secret-named variable goes, with its
//! continuation and any open quote, substitution, array or heredoc; in a script each cut command stands as `true` so
//! the file still parses. Port of packages/collect/src/detect/shell-rc.ts:205-523 at a3b870a6e; the spec is
//! spec/shell_rc_strip.md. The regex sources it reads (QUOTED, HEREDOC, ASSIGNS, GUARD, KEYWORDS) are in shell_rc.

use std::sync::LazyLock;

use fancy_regex::Regex;
use wsp_words::js::{is_js_whitespace, js_trim, js_trim_start};

use crate::detect::shell_rc::{
    assigned_in, captures, command_words, comment_at, group, is_match, is_secret_name, js_re, simple_commands, split_lines, ASSIGNS_RE,
    GUARD_RE, HEREDOC_RE, KEYWORDS_RE, QUOTED_RE,
};

/// What `strip_exports` hands back (TS `{ names, carried }`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Stripped {
    /// Each variable a cut line assigns a secret to, once, in first-cut order.
    pub names: Vec<String>,
    /// The file as it travels, its line endings the file's own.
    pub carried: String,
}

/// What is still open at the end of a line.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Open {
    /// `""`, `'`, `"`, a backtick or `$'`.
    pub quote: &'static str,
    pub groups: Vec<OpenGroup>,
    pub arith: u32,
    pub heredoc: Option<String>,
    pub continues: bool,
}

/// One open `$(`, `${`, array `=(` or bare `(` group: its closer and the quote it gives back there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenGroup {
    /// `)` or `}`.
    pub close: char,
    /// `""` or `"`.
    pub quote: &'static str,
}

static ESCAPED_SINGLE_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^'[^']*\\'$"));
static LEAD_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^(?:\s|\\\n)*"));
static TRAIL_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"(?:\s|\\\n)*$"));
static BRACE_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^\s*\{"));
static AMP_PIPE_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^\s*[&|]"));
static HEAD_RE: LazyLock<Regex> = LazyLock::new(|| js_re(r"^(?:for|select|case)\s"));

/// A `Passing` carries what a kept line leaves open: a heredoc, the arithmetic depth, and the value a cut heredoc body
/// left open.
#[derive(Debug, Clone, Default)]
struct Passing {
    heredoc: Option<String>,
    arith: u32,
    value: Option<Open>,
}

/// A cut that is still open: its lines so far and whether it is one whole command.
struct Cutting {
    open: Open,
    lines: Vec<String>,
    whole: bool,
}

/// A join that is still open: its lines so far.
struct Joining {
    open: Open,
    lines: Vec<String>,
}

/// A rule of the caller's that names the secret a line holds.
pub type SecretIn<'a> = &'a dyn Fn(&str) -> Option<String>;

/// The state `strip_exports` carries from line to line.
struct Strip<'a> {
    secret_in: Option<SecretIn<'a>>,
    held: &'a mut Vec<String>,
    script: bool,
    names: Vec<String>,
    kept: Vec<String>,
    cutting: Option<Cutting>,
    passing: Passing,
    joining: Option<Joining>,
}

/// Carries the shell's quoting state across one line.
pub fn scan_line(line: &str, start: &Open) -> Open {
    let mut o = Open { continues: false, ..start.clone() };
    if let Some(word) = o.heredoc.clone() {
        if js_trim(line) == word {
            o.heredoc = None;
        }
        return o;
    }
    let chars: Vec<char> = line.chars().collect();
    let starts: Vec<usize> = line.char_indices().map(|(at, _)| at).collect();
    let n = chars.len();
    let mut skip = 0;
    for i in 0..n {
        if skip > 0 {
            skip -= 1;
            continue;
        }
        let c = chars[i];
        let next = chars.get(i + 1).copied();
        let top = o.groups.last().copied();
        o.continues = false;
        if c == '\\' && o.quote != "'" {
            if i == n - 1 {
                o.continues = true;
            }
            skip = 1;
            continue;
        }
        if o.quote == "$'" {
            if c == '\'' {
                o.quote = "";
            }
            continue;
        }
        if o.quote == "'" || o.quote == "`" {
            if (c == '\'' && o.quote == "'") || (c == '`' && o.quote == "`") {
                o.quote = "";
            }
            continue;
        }
        if o.quote.is_empty() && comment_at(&chars, i) {
            break;
        }
        if o.quote.is_empty() && c == '$' && next == Some('\'') {
            o.quote = "$'";
            skip = 1;
            continue;
        }
        if o.quote.is_empty() && (c == '\'' || c == '`') {
            o.quote = if c == '\'' { "'" } else { "`" };
            continue;
        }
        if c == '"' {
            o.quote = if o.quote == "\"" { "" } else { "\"" };
            continue;
        }
        if o.quote.is_empty() && c == '$' && next == Some('(') && chars.get(i + 2) == Some(&'(') {
            o.arith += 1;
            skip = 2;
            continue;
        }
        if c == '$' && next == Some('(') {
            o.groups.push(OpenGroup { close: ')', quote: if o.quote == "\"" { "\"" } else { "" } });
            o.quote = "";
            skip = 1;
            continue;
        }
        if c == '$' && next == Some('{') {
            o.groups.push(OpenGroup { close: '}', quote: "" });
            skip = 1;
            continue;
        }
        if o.quote.is_empty() && c == '(' && next == Some('(') {
            o.arith += 1;
            skip = 1;
            continue;
        }
        if o.quote.is_empty() && c == ')' && next == Some(')') && o.arith > 0 {
            o.arith -= 1;
            skip = 1;
            continue;
        }
        if o.quote.is_empty() && c == '(' && (top.is_some_and(|g| g.close == ')') || (i > 0 && chars[i - 1] == '=')) {
            o.groups.push(OpenGroup { close: ')', quote: "" });
            continue;
        }
        if o.quote.is_empty() && c == ')' {
            if let Some(g) = top.filter(|g| g.close == ')') {
                o.groups.pop();
                o.quote = g.quote;
                continue;
            }
        }
        if c == '}' && top.is_some_and(|g| g.close == '}') {
            o.groups.pop();
            continue;
        }
        if c == '<' && next == Some('<') && chars.get(i + 2) == Some(&'<') {
            skip = 2;
            continue;
        }
        if o.quote.is_empty() && o.arith == 0 && c == '<' {
            if let Some(caps) = captures(&HEREDOC_RE, &line[starts[i]..]) {
                if let Some(word) = (1..=4).find_map(|k| caps.get(k)) {
                    o.heredoc = Some(word.as_str().to_string());
                    break;
                }
            }
        }
    }
    o
}

/// What is still open at the end of a line: a quote, a group, a heredoc or a continuation.
fn is_open(o: &Open) -> bool {
    !o.quote.is_empty() || !o.groups.is_empty() || o.heredoc.is_some() || o.continues
}

/// Nothing open, at the given arithmetic depth.
fn closed(arith: u32) -> Open {
    Open { arith, ..Open::default() }
}

/// What a kept line may carry forward: a heredoc it opens and the depth of an arithmetic block it left open.
fn carried(line: &str, arith: u32) -> Passing {
    let o = scan_line(line, &closed(arith));
    Passing { heredoc: o.heredoc, arith: o.arith, value: None }
}

/// The characters `text[start..end]`, as a string.
fn sub(text: &[char], start: usize, end: usize) -> String {
    text[start..end].iter().collect()
}

/// The chars of a command's leading blanks and keywords, counted from the start of `segment`.
fn lead_of(segment: &[char]) -> usize {
    let text: String = segment.iter().collect();
    let mut at = LEAD_RE.find(&text).ok().flatten().map_or(0, |m| m.end());
    while let Some(m) = KEYWORDS_RE.find(&text[at..]).ok().flatten() {
        at += m.end();
    }
    text[..at].chars().count()
}

/// Pushes the span `[start + leading blanks, end without trailing blanks)` of `text[start..i]`, when it is not empty.
fn push_span(text: &[char], spans: &mut Vec<(usize, usize)>, start: usize, i: usize) {
    let segment: &[char] = if start <= i { &text[start..i] } else { &[] };
    let s = start + lead_of(segment);
    let raw: String = segment.iter().collect();
    let trimmed = match TRAIL_RE.find(&raw).ok().flatten() {
        Some(m) => raw[..m.start()].chars().count(),
        None => raw.chars().count(),
    };
    let e = start + trimmed;
    if e > s {
        spans.push((s, e));
    }
}

/// The top-level simple commands and comments of closed shell text, keywords skipped, as [start, end) offsets. They split
/// on `;`, `|`, a newline, an `&` that is no redirect's, a bare `{` or `}`, and a `(` or `)` opening a subshell or closing a
/// case pattern; quotes, substitutions, `[[ ]]` and a `(` inside a command (an array, a fish substitution, `f()`) hold together.
/// With `lists`, `&&`, `||` and `|` do not split, so each span is a whole and-or list.
fn command_spans(text: &[char], lists: bool) -> Vec<(usize, usize)> {
    let mut spans = Vec::new();
    let mut stack: Vec<String> = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        let prev = i.checked_sub(1).map(|p| text[p]);
        let next = text.get(i + 1).copied();
        let top_owned = stack.last().cloned();
        let top = top_owned.as_deref();
        if c == '\\' && top != Some("'") {
            i += 1;
        } else if matches!(top, Some("'") | Some("$'") | Some("`")) {
            let closer = if top == Some("`") { '`' } else { '\'' };
            if c == closer {
                stack.pop();
            }
        } else if c == '$' && matches!(next, Some('(') | Some('{')) {
            stack.push(next.unwrap_or('(').to_string());
            i += 1;
        } else if c == '`' || (top != Some("\"") && (c == '\'' || c == '"')) {
            stack.push(c.to_string());
        } else if top == Some("\"") {
            if c == '"' {
                stack.pop();
            }
        } else if c == '$' && next == Some('\'') {
            stack.push("$'".to_string());
            i += 1;
        } else if let Some(top) = top {
            if top == "(" && comment_at(text, i) {
                while i + 1 < text.len() && text[i + 1] != '\n' {
                    i += 1;
                }
            } else if c == '(' {
                stack.push("(".to_string());
            } else if (c == ')' && top == "(") || (c == '}' && top == "{") {
                stack.pop();
            } else if top == "[[" && text.get(i) == Some(&']') && text.get(i + 1) == Some(&']') {
                stack.pop();
                i += 1;
            }
        } else if comment_at(text, i) {
            push_span(text, &mut spans, start, i);
            let from = i;
            while i + 1 < text.len() && text[i + 1] != '\n' {
                i += 1;
            }
            spans.push((from, i + 1));
            start = i + 1;
        } else if c == '['
            && next == Some('[')
            && text.get(i + 2).is_some_and(|&x| is_js_whitespace(x))
            && lead_of(&text[start..i]) == i - start
        {
            stack.push("[[".to_string());
            i += 1;
        } else if c == '(' && lead_of(&text[start..i]) < i - start {
            stack.push("(".to_string());
        } else if ";\n()".contains(c)
            || (c == '|' && prev != Some('>') && !lists)
            || (c == '&'
                && !matches!(prev, Some('<') | Some('>'))
                && next != Some('>')
                && !(lists && (matches!(prev, Some('&') | Some('|')) || matches!(next, Some('&') | Some('|')))))
            || ((c == '{' || c == '}')
                && next.is_none_or(is_js_whitespace)
                && prev.is_none_or(|p| is_js_whitespace(p) || p == ';' || p == '&'))
        {
            push_span(text, &mut spans, start, i);
            start = i + 1;
        }
        i += 1;
    }
    push_span(text, &mut spans, start, text.len());
    spans
}

/// Whether a line leaves open the quoted argument of an `eval`, `alias` or `abbr`, whose text is shell.
fn opens_shell(line: &str, arith: u32) -> bool {
    let escaped = QUOTED_RE.find_iter(line).any(|m| is_match(&ESCAPED_SINGLE_RE, m.as_str()));
    if escaped || !matches!(scan_line(line, &closed(arith)).quote, "'" | "\"" | "$'") {
        return false;
    }
    let commands = simple_commands(line);
    let (_, run) = command_words(commands.last().map_or("", String::as_str));
    matches!(run.first().map(String::as_str), Some("eval" | "alias" | "abbr"))
}

/// Whether a line ends in an odd run of backslashes, so the next line joins it.
fn ends_with_odd_backslashes(line: &str) -> bool {
    line.chars().rev().take_while(|&c| c == '\\').count() % 2 == 1
}

/// The span to stand in for a cut command, as `(start, end, word)`: `""` is nothing, a comment or a case arm's `;;`.
fn stands(cut: &[char], lists: &[(usize, usize)], s: usize, e: usize) -> (usize, usize, String) {
    if cut.get(s) == Some(&'#') {
        return (s, e, String::new());
    }
    if is_match(&ASSIGNS_RE, &sub(cut, s, e)) {
        return (s, e, "true".to_string());
    }
    let (ls, le) = lists.iter().copied().find(|&(a, b)| a <= s && e <= b).unwrap_or((s, e));
    let prefix = sub(cut, 0, ls);
    if let Some(guard) = captures(&GUARD_RE, &prefix) {
        let until = group(&guard, 1) == "until";
        let word = if is_match(&BRACE_RE, &sub(cut, le, cut.len())) {
            format!("[[ {} '' ]]", if until { "-z" } else { "-n" })
        } else if until {
            "true".to_string()
        } else {
            "false".to_string()
        };
        let keyword_end = guard.get(0).map_or(0, |m| prefix[..m.end()].chars().count());
        return (keyword_end, le, word);
    }
    if is_match(&AMP_PIPE_RE, &sub(cut, e, le)) {
        (ls, le, "true".to_string())
    } else {
        (s, e, "true".to_string())
    }
}

impl Strip<'_> {
    /// What `secret_in` names in a line, if it names anything.
    fn found(&self, line: &str) -> Option<String> {
        self.secret_in.and_then(|f| f(line))
    }

    /// Whether a command holds a secret: one of its assigned names is secret-named, or `secret_in` names it.
    fn holds(&self, command: &str) -> bool {
        assigned_in(command).iter().any(|n| is_secret_name(n)) || self.found(command).is_some()
    }

    /// Cuts a command's text in place of its spans, as a script; see `leave` in the TS.
    fn leave(&mut self, lines: Vec<String>, whole: bool) {
        if !self.script {
            return;
        }
        let cut_text = lines.join("\n");
        let cut: Vec<char> = cut_text.chars().collect();
        let all: Vec<(usize, usize)> = if whole { command_spans(&cut, false) } else { Vec::new() };
        let spans: Vec<(usize, usize)> = all.into_iter().filter(|&(s, e)| self.holds(&sub(&cut, s, e))).collect();
        let lists = command_spans(&cut, true);
        let mut ranges: Vec<(usize, usize, String)> = if spans.iter().any(|&(s, _)| is_match(&HEAD_RE, &sub(&cut, s, cut.len()))) {
            Vec::new()
        } else {
            spans.iter().map(|&(s, e)| stands(&cut, &lists, s, e)).collect()
        };
        ranges.sort_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));
        let mut rest = String::new();
        let mut at = 0;
        for (s, e, word) in ranges {
            if s < at {
                continue;
            }
            if word.is_empty() {
                rest += sub(&cut, at, s).trim_end_matches(&[' ', '\t'][..]);
            } else {
                rest += &sub(&cut, at, s);
                rest += &word;
            }
            at = e;
        }
        rest += &sub(&cut, at, cut.len());
        let first = &lines[0];
        if rest != cut_text
            && !js_trim(&rest).is_empty()
            && !rest.split('\n').any(|l| self.found(l).is_some() || simple_commands(l).iter().any(|c| self.holds(c)))
        {
            self.kept.extend(rest.split('\n').map(String::from));
        } else if !js_trim_start(first).starts_with('#') && !first.contains(";;") && !first.contains(";&") && !first.contains(";|") {
            let indent = &first[..first.len() - first.trim_start_matches([' ', '\t']).len()];
            self.kept.push(format!("{indent}true"));
        }
    }

    /// Looks at one line that is not inside an open cut, join or heredoc (`take` in the TS).
    fn take(&mut self, line: &str) {
        let found = self.found(line);
        let mut hits = Vec::new();
        for command in simple_commands(line) {
            for name in assigned_in(&command) {
                if is_secret_name(&name) || (found.is_some() && self.found(&command).is_some()) {
                    hits.push(name);
                }
            }
        }
        if hits.is_empty() && found.is_none() {
            self.kept.extend(line.split('\n').map(String::from));
            self.passing = carried(line, self.passing.arith);
            return;
        }
        let no_hits = hits.is_empty();
        for h in hits {
            if !self.names.contains(&h) {
                self.names.push(h);
            }
        }
        if let Some(f) = found {
            if no_hits && !self.held.contains(&f) {
                self.held.push(f);
            }
        }
        let o = scan_line(line, &closed(self.passing.arith));
        if is_open(&o) {
            self.cutting = Some(Cutting { whole: o.heredoc.is_none(), open: o, lines: line.split('\n').map(String::from).collect() });
        } else {
            self.leave(line.split('\n').map(String::from).collect(), true);
            self.passing = Passing { heredoc: None, arith: o.arith, value: None };
        }
    }

    /// Takes one line of the file, in the order the states of the TS main loop are tried.
    fn line(&mut self, line: &str) {
        if let Some(mut cutting) = self.cutting.take() {
            let o = scan_line(line, &cutting.open);
            cutting.lines.push(line.to_string());
            if is_open(&o) {
                cutting.whole = cutting.whole && o.heredoc.is_none();
                cutting.open = o;
                self.cutting = Some(cutting);
            } else {
                self.leave(cutting.lines, cutting.whole);
                self.passing = Passing { heredoc: None, arith: o.arith, value: None };
            }
            return;
        }
        if let Some(mut joining) = self.joining.take() {
            let o = scan_line(line, &joining.open);
            joining.lines.push(line.to_string());
            if !o.quote.is_empty() || !o.groups.is_empty() || o.continues {
                joining.open = o;
                self.joining = Some(joining);
            } else {
                self.take(&joining.lines.join("\n"));
            }
            return;
        }
        if let Some(word) = self.passing.heredoc.clone() {
            if js_trim(line) == word {
                self.kept.push(line.to_string());
                self.passing = Passing { heredoc: None, arith: self.passing.arith, value: None };
            } else if let Some(value) = self.passing.value.take() {
                let o = scan_line(line, &value);
                self.passing.value = if is_open(&o) { Some(o) } else { None };
            } else {
                let found = self.found(line);
                let named: Vec<String> = simple_commands(line).iter().flat_map(|c| assigned_in(c)).filter(|n| is_secret_name(n)).collect();
                for n in &named {
                    if !self.names.contains(n) {
                        self.names.push(n.clone());
                    }
                }
                if named.is_empty() {
                    if let Some(f) = &found {
                        if !self.held.contains(f) {
                            self.held.push(f.clone());
                        }
                    }
                }
                if named.is_empty() && found.is_none() {
                    self.kept.push(line.to_string());
                } else if self.kept.last().is_some_and(|k| ends_with_odd_backslashes(k)) {
                    self.kept.push(String::new());
                }
                let o = scan_line(line, &Open::default());
                if !named.is_empty() && is_open(&o) && o.heredoc.is_none() {
                    self.passing.value = Some(o);
                }
            }
            return;
        }
        if opens_shell(line, self.passing.arith) {
            self.joining = Some(Joining { open: scan_line(line, &closed(self.passing.arith)), lines: vec![line.to_string()] });
        } else {
            self.take(line);
        }
    }
}

/// `secret_in` names a secret a line holds by a rule of the caller's; `held` collects what it names for a cut line
/// that assigns none (TS defaults: none, a fresh list, `script` false).
pub fn strip_exports(text: &str, secret_in: Option<SecretIn<'_>>, held: &mut Vec<String>, script: bool) -> Stripped {
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let mut strip =
        Strip { secret_in, held, script, names: Vec::new(), kept: Vec::new(), cutting: None, passing: Passing::default(), joining: None };
    for line in split_lines(text) {
        strip.line(line);
    }
    if let Some(joining) = strip.joining.take() {
        strip.take(&joining.lines.join("\n"));
    }
    if let Some(cutting) = strip.cutting.take() {
        strip.leave(cutting.lines, false);
    }
    Stripped { names: strip.names, carried: strip.kept.join(eol) }
}
