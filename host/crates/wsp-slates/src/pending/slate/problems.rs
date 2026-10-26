// SPDX-License-Identifier: AGPL-3.0-only
//! protocol/src/slate/problems.ts at 65f6b89c7.
use super::*;

pub fn slate_server_not_set_up(server: &str, computer: &str, agent_id: &str, agent_name: &str, thread: &str) -> String {
    format!(
        "{server} is not set up on {computer}: add it to {agent_name}'s MCP config there with wsp servers add {server} {thread} --agent {agent_id} --command \"<its command line>\" (or --url <its address>), then write the run again"
    )
}

pub fn slate_host_away(run: &str, computer: &str, host: &str) -> String {
    format!("{run} runs on=\"host\", which is {host} with the sign-ins there, and this thread runs on {computer}: drop on=\"host\" and it runs on {computer}")
}

pub fn slate_description_missing(run: &str, server: &str, tool: &str) -> String {
    format!(
        "{run} calls {server}.{tool}, which its server does not mark read-only, so it needs description=\"...\" saying in one plain line what it does; the person reads it before allowing the run"
    )
}

pub const SLATE_CODES: &[(&str, &str)] = &[
    ("P100", "bad-syntax"),
    ("P102", "bare-not-boolean"),
    ("P103", "bad-id"),
    ("P104", "duplicate-id"),
    ("P105", "bad-patch-op"),
    ("P107", "reaction-without-id"),
    ("D200", "schema-unknown"),
    ("D201", "kit-unknown"),
    ("D202", "root-ambiguous"),
    ("D203", "piece-missing"),
    ("D205", "cycle"),
    ("D206", "too-deep"),
    ("D207", "too-many-pieces"),
    ("D208", "too-big"),
    ("T300", "type-unknown"),
    ("T302", "prop-unknown"),
    ("T303", "prop-type"),
    ("T304", "prop-required"),
    ("T305", "prop-not-bindable"),
    ("T306", "prop-enum"),
    ("T307", "label-missing"),
    ("T308", "children-not-allowed"),
    ("T309", "children-required"),
    ("T311", "two-loud"),
    ("T312", "prop-reserved"),
    ("T314", "item-unknown"),
    ("X400", "expr-syntax"),
    ("X401", "path-unknown"),
    ("X402", "path-scope"),
    ("X403", "path-consent"),
    ("X404", "fn-unknown"),
    ("X405", "fn-arity"),
    ("X406", "fn-arg-type"),
    ("X407", "expr-limit"),
    ("X408", "expr-type"),
    ("X409", "item-outside-scope"),
    ("X410", "two-way-not-value"),
    ("X420", "javascript"),
    ("Q420", "pipe-syntax"),
    ("Q422", "step-unknown"),
    ("Q423", "step-arity"),
    ("Q424", "step-type"),
    ("Q425", "step-literal"),
    ("Q426", "pipe-limit"),
    ("S500", "values-too-big"),
    ("S501", "name-undeclared"),
    ("S502", "editable-not-value"),
    ("S503", "start-not-literal"),
    ("S510", "derived-cycle"),
    ("S512", "too-many-derived"),
    ("S513", "too-many-values"),
    ("S520", "secret-exposed"),
    ("A600", "step-unknown"),
    ("A601", "step-arg"),
    ("A602", "event-unknown"),
    ("A603", "text-not-literal"),
    ("A606", "too-many-steps"),
    ("A607", "set-not-value"),
    ("A608", "window-step-in-reaction"),
    ("A610", "reaction-cycle"),
    ("A611", "reaction-shape"),
    ("K700", "cmd-not-literal"),
    ("K701", "run-kind"),
    ("K702", "run-name"),
    ("K703", "timer"),
    ("K704", "run-arg"),
    ("K705", "tool-name"),
    ("K706", "tool-unknown"),
    ("K707", "too-many-runs"),
    ("K708", "file-name"),
    ("K709", "files-too-big"),
    ("K710", "server-not-set-up"),
    ("K711", "host-away"),
    ("K712", "machine-on-box"),
    ("K713", "description-missing"),
    ("K714", "server-off"),
    ("V750", "version-behind"),
    ("V751", "write-rate"),
    ("V752", "nothing-to-undo"),
    ("V753", "send-rate"),
    ("V754", "event-rate"),
    ("Z800", "thread-not-yours"),
    ("Z801", "thread-not-in-tree"),
    ("Z802", "no-slate"),
    ("Z803", "sub-thread"),
    ("Z804", "rewound-before"),
    ("R900", "data-missing"),
    ("R901", "data-late"),
    ("R902", "eval-failed"),
    ("R903", "source-unavailable"),
    ("R904", "consent-refused"),
    ("R905", "draw-failed"),
    ("R906", "tool-error"),
    ("R907", "over-budget"),
    ("R908", "piece-fallback"),
    ("R910", "reaction-runaway"),
    ("R911", "run-runaway"),
    ("R912", "reaction-failed"),
    ("R913", "run-held"),
    ("R914", "not-a-file"),
    ("R915", "image-too-big"),
    ("R916", "not-an-image"),
    ("R917", "address-refused"),
    ("W001", "content-looks-like-path"),
    ("W002", "tone-alone"),
    ("W003", "series-as-bars"),
    ("W004", "copy-style"),
    ("W006", "deprecated"),
    ("W010", "short-secret"),
    ("W011", "secret-in-argv"),
    ("W012", "joined-by-mark"),
    ("W013", "chart-x-index"),
    ("W014", "mono-on-sentence"),
    ("W015", "file-undeclared"),
    ("W017", "emoji"),
    ("W018", "title-case"),
    ("W019", "run-never-started"),
    ("W020", "value-beside-series"),
    ("W021", "time-in-seconds"),
];

pub const SLATE_WARNINGS: &[&str] = &[
    "T311", "X403", "W001", "W002", "W003", "W004", "W006", "W010", "W011", "W012", "W013", "W014", "W015", "W017", "W018", "W019", "W020",
    "W021",
];

fn slate_code_name(code: &str) -> Option<&'static str> {
    SLATE_CODES.iter().find(|(known, _)| *known == code).map(|(_, name)| *name)
}

pub fn slate_problem(code: &str, message: &str, fix: Option<&str>) -> SlateProblem {
    SlateProblem {
        code: code.to_string(),
        name: slate_code_name(code).unwrap_or_default().to_string(),
        message: message.to_string(),
        fix: fix.map(str::to_string),
        ..Default::default()
    }
}

fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Levenshtein distance over UTF-16 units, capped: past cap the answer is cap + 1.
fn edit_distance(a: &str, b: &str, cap: usize) -> usize {
    let a: Vec<u16> = a.encode_utf16().collect();
    let b: Vec<u16> = b.encode_utf16().collect();
    if a.len().abs_diff(b.len()) > cap {
        return cap + 1;
    }
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for i in 1..=a.len() {
        let mut cur = vec![i];
        let mut best = i;
        for j in 1..=b.len() {
            let v = (prev[j] + 1).min(cur[j - 1] + 1).min(prev[j - 1] + usize::from(a[i - 1] != b[j - 1]));
            cur.push(v);
            best = best.min(v);
        }
        if best > cap {
            return cap + 1;
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The option closest to word within max edits, ties to the first listed; None when none is close.
pub fn slate_nearest(word: &str, options: &[String]) -> Option<String> {
    let max = if utf16_len(word) >= 7 { 3 } else { 2 };
    let lower = word.to_lowercase();
    let mut found: Option<&String> = None;
    let mut best = max + 1;
    for option in options {
        let d = edit_distance(&lower, &option.to_lowercase(), max);
        if d < best {
            found = Some(option);
            best = d;
        }
    }
    if found.is_none() && utf16_len(word) >= 3 {
        return options.iter().find(|option| option.to_lowercase().starts_with(&lower)).cloned();
    }
    found.cloned()
}

/// "a, b or c".
pub fn or_list<S: AsRef<str>>(words: &[S]) -> String {
    match words {
        [] => String::new(),
        [only] => only.as_ref().to_string(),
        [head @ .., last] => format!("{} or {}", head.iter().map(AsRef::as_ref).collect::<Vec<_>>().join(", "), last.as_ref()),
    }
}

/// What an HTML habit means in the kit: the piece a small model meant when it writes a tag the kit does not have.
fn html_habit(tag: &str) -> Option<&'static str> {
    Some(match tag {
        "p" | "span" | "label" | "strong" | "em" | "b" | "i" | "small" | "pre" | "code" => "words go in <text>...</text>",
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "header" | "title" => "a title is a <heading>, or a <section title=\"...\">",
        "div" | "main" | "body" | "nav" | "footer" | "article" | "aside" | "card" | "container" | "stack" | "box" => {
            "a group is a <column>, a <row> or a <section>"
        }
        "ul" | "ol" | "li" | "list" => "a list is a <checklist> or a <table items={...}>",
        "data" | "fetch" | "source" | "poll" => "data comes from a <run name=\"x\" cmd='...' every={60} />, read as $x.json",
        "piece" | "component" => "the tag is the piece's own type, like <number ...> or <text>",
        "a" => "a link is a <button> whose onPress opens it",
        "img" => "a slate draws no images",
        _ => return None,
    })
}

/// The words for a tag the kit has no piece for: the piece an HTML habit meant, else the nearest name.
pub fn slate_unknown_piece(tag: &str, pieces: &[String]) -> UnknownPiece {
    if let Some(meant) = html_habit(&tag.to_lowercase()) {
        return UnknownPiece { message: format!("\"{tag}\" is not a piece; {meant}"), fix: None };
    }
    let fix = slate_nearest(tag, pieces);
    let hint = fix.as_ref().map_or(String::new(), |fix| format!("; did you mean {fix}?"));
    UnknownPiece { message: format!("\"{tag}\" is not a piece{hint} slate_catalog lists them"), fix }
}

/// A tag the kit has no piece for, with the sentence that says what to write instead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownPiece {
    pub message: String,
    pub fix: Option<String>,
}
