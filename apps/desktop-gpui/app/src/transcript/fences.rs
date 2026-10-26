// SPDX-License-Identifier: AGPL-3.0-only
//! The fenced blocks a reply opens, by runBlockKey's rules.

/// The reply's fenced block opening on the line holding `offset`, in UTF-16 units (runBlockKey in protocol/src/run-block.ts).
pub(crate) fn block_at(text: &str, offset: usize) -> Option<usize> {
    fence_openings(text).iter().position(|opening| (opening.at..=opening.at + opening.width).contains(&offset))
}

/// A fenced block's opening line: its byte offset, and its offset and width in UTF-16 units.
pub(crate) struct FenceOpening {
    pub byte: usize,
    pub at: usize,
    pub width: usize,
}

/// Each fenced block's opening line in a reply, in order, by runBlockKey's rules.
pub(crate) fn fence_openings(text: &str) -> Vec<FenceOpening> {
    let (mut byte, mut at) = (0, 0);
    let mut open: Option<(char, usize)> = None;
    let mut openings = Vec::new();
    for line in text.split('\n') {
        let width = line.encode_utf16().count();
        let bare = line.trim_start_matches([' ', '>', '\t']);
        let fence = bare.chars().next().filter(|c| *c == '`' || *c == '~').map(|c| (c, bare.chars().take_while(|d| *d == c).count()));
        match (open, fence) {
            (Some((c, n)), Some((d, m))) if c == d && m >= n && bare.trim_end().chars().all(|e| e == c) => open = None,
            (Some(_), _) => {}
            (None, Some((c, n))) if n >= 3 && !(c == '`' && bare[n..].contains('`')) => {
                openings.push(FenceOpening { byte, at, width });
                open = Some((c, n));
            }
            (None, _) => {}
        }
        byte += line.len() + 1;
        at += width + 1;
    }
    openings
}
