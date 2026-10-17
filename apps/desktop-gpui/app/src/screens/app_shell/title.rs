// SPDX-License-Identifier: AGPL-3.0-only
//! A thread's title as `foldThreads` in `packages/protocol/src/views/session.ts` makes it, and as the tool server's
//! `fold` makes it in Rust: the harness's own name for it on one line, else its opening turn's first sentence cut to
//! 48 characters at a word.

/// `OPENING_TITLE_MAX` in `packages/protocol/src/words/turn.ts`.
pub const OPENING_TITLE_MAX: usize = 48;
const ELLIPSIS: char = '\u{2026}';

/// The thread's title from its harness's name for it, else its opening words.
pub fn thread_title(harness_title: Option<&str>, opening: Option<&str>) -> Option<String> {
    match (harness_title, opening) {
        (Some(named), _) => Some(title_line(named)),
        (None, Some(prompt)) => Some(opening_title(prompt)),
        (None, None) => None,
    }
}

/// `titleLine`: the first line with words on it, its white space collapsed.
pub fn title_line(text: &str) -> String {
    let first = text.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l)).find(|l| !l.trim().is_empty()).unwrap_or("");
    first.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// `openingTitle`: the first sentence of the first line, cut at a word to [`OPENING_TITLE_MAX`] characters.
pub fn opening_title(text: &str) -> String {
    let line = title_line(text);
    let chars: Vec<(usize, char)> = line.char_indices().collect();
    let sentence = chars
        .iter()
        .enumerate()
        .find(|(i, (_, c))| matches!(c, '.' | '!' | '?') && chars.get(i + 1).is_none_or(|(_, next)| next.is_whitespace()))
        .map_or(line.as_str(), |(_, (at, c))| &line[..at + c.len_utf8()]);
    cut_line(sentence, OPENING_TITLE_MAX)
}

/// The JavaScript length of `text`, in UTF-16 units.
fn length(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The longest head of `text` within `room` UTF-16 units, a pair never split.
fn head(text: &str, room: usize) -> &str {
    let mut units = 0;
    for (at, c) in text.char_indices() {
        units += c.len_utf16();
        if units > room {
            return &text[..at];
        }
    }
    text
}

fn without_separator_tail(text: &str) -> &str {
    text.trim_end_matches(|c: char| c.is_whitespace() || matches!(c, ',' | ';' | ':'))
}

/// `cutLine` in `packages/protocol/src/words/base.ts`: the whole words that fit, the ellipsis counted.
fn cut_line(text: &str, room: usize) -> String {
    if length(text) <= room {
        return text.to_owned();
    }
    let room = room - 1;
    let within = head(text, room + 1);
    let kept = match within.rfind(' ').filter(|at| *at > 0) {
        Some(boundary) => without_separator_tail(&within[..boundary]),
        None => without_separator_tail(head(text, room)),
    };
    format!("{kept}{ELLIPSIS}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cap_is_the_protocols() {
        let src = std::fs::read_to_string(format!("{}/../../../packages/protocol/src/words/turn.ts", env!("CARGO_MANIFEST_DIR"))).unwrap();
        let line = src.lines().find(|l| l.starts_with("const OPENING_TITLE_MAX = ")).expect("OPENING_TITLE_MAX");
        assert_eq!(
            line.trim_start_matches("const OPENING_TITLE_MAX = ").trim_end_matches(';').parse::<usize>().unwrap(),
            OPENING_TITLE_MAX
        );
    }

    #[test]
    fn a_title_reads_as_fold_threads_makes_it() {
        assert_eq!(thread_title(Some("  Fix the\tbuild  \nsecond line"), Some("ignored")).as_deref(), Some("Fix the build"));
        assert_eq!(
            thread_title(None, Some("Add a test that the paging stops at the first event. (short-3, turn 1)")).as_deref(),
            Some("Add a test that the paging stops at the first\u{2026}")
        );
        assert_eq!(thread_title(None, Some("\n\nWhy? Because.")).as_deref(), Some("Why?"));
        assert_eq!(thread_title(None, Some("v1.2 is out")).as_deref(), Some("v1.2 is out"));
        assert_eq!(opening_title(&"x".repeat(60)), format!("{}\u{2026}", "x".repeat(47)));
        assert_eq!(
            opening_title("one, two, three, four, five, six, seven, eight, nine"),
            "one, two, three, four, five, six, seven, eight\u{2026}"
        );
        assert_eq!(thread_title(None, None), None);
    }
}
