// SPDX-License-Identifier: AGPL-3.0-only
//! A terminal line as spans for the build's stage block, after `recipe/ansi.ts`: the machine's own ANSI colours kept
//! (the eight and their bright pair, bold, dim), a tool's own prefix (`pnpm:`) set apart for the block to dim, and any
//! other code dropped, never shown. The colours are the terminal pane's sixteen slots for each side, `index.css`'s
//! `--terminal-ansi-<n>`, which no theme overrides.

use gpui_kit::{rgb, Hsla};

/// One run of a line in one style: its palette slot, bold, dim.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Span {
    pub text: String,
    pub slot: Option<u8>,
    pub bold: bool,
    pub dim: bool,
}

/// `ansiSpans`: the line split on its SGR codes into styled runs, each code read alone, as the web reads `38;5;1`.
pub fn spans(line: &str) -> Vec<Span> {
    let mut out = Vec::new();
    let mut style = Span::default();
    let mut rest = line;
    let mut push = |text: &str, style: &Span| {
        if !text.is_empty() {
            out.push(Span { text: text.to_owned(), ..style.clone() });
        }
    };
    while let Some(at) = rest.find("\u{1b}[") {
        let after = &rest[at + 2..];
        let codes_len = after.find(|c: char| !(c.is_ascii_digit() || c == ';')).unwrap_or(after.len());
        if !after[codes_len..].starts_with('m') {
            push(&rest[..at + 2], &style);
            rest = after;
            continue;
        }
        push(&rest[..at], &style);
        let codes = if codes_len == 0 { "0" } else { &after[..codes_len] };
        for code in codes.split(';').map(|c| c.parse::<u32>().unwrap_or(0)) {
            match code {
                0 => style = Span::default(),
                1 => style.bold = true,
                2 => style.dim = true,
                22 => {
                    style.bold = false;
                    style.dim = false;
                }
                39 => style.slot = None,
                30..=37 => style.slot = Some((code - 30) as u8),
                90..=97 => style.slot = Some((code - 90 + 8) as u8),
                _ => {}
            }
        }
        rest = &after[codes_len + 1..];
    }
    push(rest, &style);
    out
}

/// `toolPrefix`: the line split into the tool's own prefix (`pnpm:`) and the rest, where it has one.
pub fn tool_prefix(line: &str) -> Option<(&str, &str)> {
    let mut chars = line.char_indices();
    let (_, first) = chars.next()?;
    if !first.is_ascii_lowercase() {
        return None;
    }
    let colon = chars.find(|(_, c)| !(c.is_ascii_alphanumeric() || *c == '_' || *c == '-')).filter(|(_, c)| *c == ':')?.0;
    line[colon + 1..].starts_with(char::is_whitespace).then(|| line.split_at(colon + 1))
}

const LIGHT: [u32; 16] = [
    0x1d1f21, 0xc63340, 0x686e25, 0x846726, 0x296ea3, 0xa634c5, 0x29766e, 0x696c6a, 0x666666, 0xa92e36, 0x586122, 0x6b5b22, 0x235d9c,
    0x8b30af, 0x26665c, 0x525252,
];
const DARK: [u32; 16] = [
    0x858789, 0xce6868, 0xb5bd68, 0xf0c674, 0x81a2be, 0xb294bb, 0x8abeb7, 0xc5c8c6, 0x949494, 0xe25a5d, 0xb9ca4a, 0xe7c547, 0x7aa6da,
    0xc397d8, 0x70c0b1, 0xeaeaea,
];

/// `--terminal-ansi-<slot>` on the side the theme draws.
pub fn slot_ink(slot: u8, dark: bool) -> Hsla {
    rgb(if dark { DARK } else { LIGHT }[usize::from(slot.min(15))]).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(text: &str, slot: Option<u8>, bold: bool, dim: bool) -> Span {
        Span { text: text.into(), slot, bold, dim }
    }

    #[test]
    fn colours_bold_and_dim_ride_their_codes_and_the_rest_is_dropped() {
        assert_eq!(spans("plain"), [span("plain", None, false, false)]);
        let line = "\u{1b}[1;32mok\u{1b}[0m then \u{1b}[91mred\u{1b}[39m \u{1b}[2mdim\u{1b}[22m\u{1b}[38;5;1mx\u{1b}[m!";
        assert_eq!(
            spans(line),
            [
                span("ok", Some(2), true, false),
                span(" then ", None, false, false),
                span("red", Some(9), false, false),
                span(" ", None, false, false),
                span("dim", None, false, true),
                span("x", None, true, false),
                span("!", None, false, false),
            ]
        );
    }

    #[test]
    fn a_tools_prefix_is_a_lowercase_word_and_a_colon_before_a_space() {
        assert_eq!(tool_prefix("pnpm: done"), Some(("pnpm:", " done")));
        assert_eq!(tool_prefix("apt-get:\tok"), Some(("apt-get:", "\tok")));
        assert_eq!(tool_prefix("Pnpm: x"), None);
        assert_eq!(tool_prefix("pnpm:x"), None);
        assert_eq!(tool_prefix("pnpm done"), None);
        assert_eq!(tool_prefix("pnpm:"), None);
    }

    #[test]
    fn the_slots_are_the_panes_on_each_side() {
        assert_eq!(slot_ink(1, false), rgb(0xc63340).into());
        assert_eq!(slot_ink(15, true), rgb(0xeaeaea).into());
    }
}
