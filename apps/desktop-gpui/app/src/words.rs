// SPDX-License-Identifier: AGPL-3.0-only
//! The protocol's words both the transcript and the sidebar's markdown read: `fmtTokens` and `fmtDuration` in
//! packages/protocol/src/words/units.ts and TOOL_ROWS in tools.ts, in one place so the two screens cannot drift.

/// How a tool's call reads as one line (the `line` of a TOOL_ROWS row).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolLine {
    /// `$ ` and the command's first line.
    Shell,
    /// The verb and the path in `field`.
    Path,
    /// The verb and the first line of `field`.
    About,
    /// The verb and the one path changed, or the count of files.
    Change,
    /// The first question asked.
    Question,
}

/// `itemType`: the kind a client groups a call by.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolItem {
    Command,
    FileChange,
    WebSearch,
    Agent,
    Mcp,
}

/// `requestKind`: what a prompt over the call asks leave for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolRequest {
    Command,
    FileRead,
    FileChange,
}

/// One row of TOOL_ROWS.
#[derive(Clone, Copy, Debug)]
pub struct ToolRow {
    pub line: ToolLine,
    /// The line's verb while the call runs and once it is done; `did` None reads `doing` at both.
    pub doing: &'static str,
    pub did: Option<&'static str>,
    /// The input field a Path or About line names.
    pub field: &'static str,
    /// The fields the call's detail shows, before the shared ones.
    pub shows: &'static [&'static str],
    pub title: Option<&'static str>,
    pub item: Option<ToolItem>,
    pub request: Option<ToolRequest>,
    /// The input field the changed path is in.
    pub path: Option<&'static str>,
    pub code_search: bool,
}

const BLANK: ToolRow = ToolRow {
    line: ToolLine::About,
    doing: "",
    did: None,
    field: "",
    shows: &[],
    title: None,
    item: None,
    request: None,
    path: None,
    code_search: false,
};

pub fn tool_row(name: &str) -> Option<ToolRow> {
    let path = |doing, did, field, request| ToolRow {
        line: ToolLine::Path,
        doing,
        did: Some(did),
        field,
        shows: &["file_path"],
        request: Some(request),
        ..BLANK
    };
    let edit = |field| ToolRow {
        shows: &["file_path"],
        item: Some(ToolItem::FileChange),
        path: Some(field),
        ..path("editing", "edited", field, ToolRequest::FileChange)
    };
    let about = |doing, did, field, item| ToolRow { line: ToolLine::About, doing, did, field, shows: &[], item, ..BLANK };
    let searched =
        ToolRow { code_search: true, shows: &["pattern"], ..about("searching code for", Some("searched code for"), "pattern", None) };
    let web = |doing, did, field| ToolRow { shows: &[], ..about(doing, Some(did), field, Some(ToolItem::WebSearch)) };
    Some(match name {
        "Bash" | "command_execution" => ToolRow {
            line: ToolLine::Shell,
            field: "command",
            shows: &["description"],
            item: Some(ToolItem::Command),
            request: Some(ToolRequest::Command),
            ..BLANK
        },
        "Read" => path("reading", "read", "file_path", ToolRequest::FileRead),
        "Write" => ToolRow { doing: "writing", did: Some("wrote"), ..edit("file_path") },
        "Edit" | "MultiEdit" => edit("file_path"),
        "NotebookEdit" => ToolRow { shows: &["notebook_path"], ..edit("notebook_path") },
        "file_change" => ToolRow {
            line: ToolLine::Change,
            doing: "editing",
            did: Some("edited"),
            item: Some(ToolItem::FileChange),
            request: Some(ToolRequest::FileChange),
            path: Some("changes"),
            ..BLANK
        },
        "Grep" | "Glob" => searched,
        "WebSearch" | "web_search" => ToolRow { shows: &["query"], ..web("searching the web for", "searched the web for", "query") },
        "WebFetch" => ToolRow { shows: &["url"], ..web("fetching", "fetched", "url") },
        "Task" | "Agent" => ToolRow { shows: &["description"], ..about("agent:", None, "description", Some(ToolItem::Agent)) },
        "spawn_agent" => ToolRow { shows: &["prompt"], ..about("agent:", None, "prompt", Some(ToolItem::Agent)) },
        "AskUserQuestion" => ToolRow { line: ToolLine::Question, title: Some("Asked you"), ..BLANK },
        _ => return None,
    })
}

impl ToolRow {
    /// The verb the line opens with, asked or done.
    pub fn verb(&self, done: bool) -> &'static str {
        if done {
            self.did.unwrap_or(self.doing)
        } else {
            self.doing
        }
    }
}

/// A number as JavaScript's String() writes the whole ones these lines carry.
pub fn js_number(n: f64) -> String {
    if n.fract() == 0.0 && n.abs() < 1e21 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// A magnitude's decimal digits rounded half up at `places`, off its integer and fraction digits.
pub fn round_half_up(int: &str, frac: &str, places: usize) -> String {
    let mut digits: Vec<u8> = int.bytes().chain(frac.bytes().chain(std::iter::repeat(b'0')).take(places)).collect();
    if frac.as_bytes().get(places).is_some_and(|d| *d >= b'5') {
        let mut i = digits.len();
        loop {
            if i == 0 {
                digits.insert(0, b'1');
                break;
            }
            i -= 1;
            if digits[i] == b'9' {
                digits[i] = b'0';
            } else {
                digits[i] += 1;
                break;
            }
        }
    }
    let split = digits.len() - places;
    let (whole, fraction) = digits.split_at(split);
    let whole = String::from_utf8_lossy(whole);
    if places == 0 {
        whole.into_owned()
    } else {
        format!("{whole}.{}", String::from_utf8_lossy(fraction))
    }
}

/// `Number.prototype.toFixed` for finite values under 1e21: the double's exact value rounded, a tie up.
pub fn to_fixed(x: f64, places: usize) -> String {
    let magnitude = x.abs();
    let bits = magnitude.to_bits();
    let exponent = ((bits >> 52) & 0x7ff) as i64;
    // A double is m * 2^q, and its exact decimal expansion has -q digits after the point.
    let q = if exponent == 0 { -1074 } else { exponent - 1075 };
    let exact = format!("{magnitude:.*}", (-q).max(0) as usize);
    let (int, frac) = exact.split_once('.').unwrap_or((&exact, ""));
    let fixed = round_half_up(int, frac, places);
    if x < 0.0 {
        format!("-{fixed}")
    } else {
        fixed
    }
}

/// `fmtTokens`: whole under a thousand, then three figures in k, M or B with no trailing zero (4.27k, 22.6M, 687M).
pub fn fmt_tokens(n: f64) -> String {
    let count = if n.is_finite() && n > 0.0 { n.round() } else { 0.0 };
    if count < 1_000.0 {
        return js_number(count);
    }
    let (value, unit) = if count < 999_500.0 {
        (count / 1_000.0, "k")
    } else if count < 999_500_000.0 {
        (count / 1_000_000.0, "M")
    } else {
        (count / 1_000_000_000.0, "B")
    };
    let places = match value {
        v if v < 10.0 => 2,
        v if v < 100.0 => 1,
        _ => 0,
    };
    let fixed = to_fixed(value, places);
    let trimmed = if fixed.contains('.') { fixed.trim_end_matches('0').trim_end_matches('.') } else { &fixed };
    format!("{trimmed}{unit}")
}

/// `fmtDuration`'s short style: ms under a second, tenths under ten, whole seconds under a minute, then minutes and
/// seconds.
pub fn fmt_duration(ms: f64) -> String {
    if ms < 60_000.0 || !ms.is_finite() {
        if !ms.is_finite() || ms < 0.0 {
            return "0ms".to_string();
        }
        if ms < 1_000.0 {
            return format!("{}ms", js_number(ms.round().max(1.0)));
        }
        if ms < 10_000.0 {
            let tenths = (ms / 100.0).round() as i64;
            return if tenths >= 100 { "10s".to_string() } else { format!("{}.{}s", tenths / 10, tenths % 10) };
        }
        return format!("{}s", js_number((ms / 1_000.0).round()));
    }
    let total = if ms > 0.0 { (ms / 1_000.0).round() as i64 } else { 0 };
    let (minutes, seconds) = (total / 60, total % 60);
    if seconds == 0 {
        format!("{minutes}m")
    } else {
        format!("{minutes}m {seconds}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_and_durations_read_as_the_protocol_writes_them() {
        assert_eq!(fmt_tokens(999.0), "999");
        assert_eq!(fmt_tokens(4_270.0), "4.27k");
        assert_eq!(fmt_tokens(4_265.0), "4.26k");
        assert_eq!(fmt_tokens(22_550_000.0), "22.6M");
        assert_eq!(fmt_tokens(1_005.0), "1k");
        assert_eq!(fmt_duration(0.4), "1ms");
        assert_eq!(fmt_duration(9_960.0), "10s");
        assert_eq!(fmt_duration(4_250.0), "4.3s");
        assert_eq!(fmt_duration(125_000.0), "2m 5s");
        assert_eq!(fmt_duration(f64::NAN), "0ms");
    }

    #[test]
    fn every_tool_row_carries_its_line_and_its_kinds() {
        let bash = tool_row("Bash").unwrap();
        assert_eq!((bash.line, bash.item, bash.request), (ToolLine::Shell, Some(ToolItem::Command), Some(ToolRequest::Command)));
        let write = tool_row("Write").unwrap();
        assert_eq!((write.verb(false), write.verb(true), write.path), ("writing", "wrote", Some("file_path")));
        assert!(tool_row("Grep").unwrap().code_search);
        assert_eq!(tool_row("Task").unwrap().verb(true), "agent:");
        assert_eq!(tool_row("AskUserQuestion").unwrap().title, Some("Asked you"));
        assert!(tool_row("mcp__x__y").is_none());
    }
}
