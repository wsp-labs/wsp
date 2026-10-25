// SPDX-License-Identifier: AGPL-3.0-only
//! The words a number's stat cell carries in its note, after `apps/web/src/slate/pieces/riders.ts`: a status and the
//! text that follows it, written right after the number, or beside it in the same row or cell (alone, or stacked in a
//! column of their own). The number draws them; the pieces themselves draw nothing. Two numbers or more alone in a row,
//! column or grid are a strip of stat cells.

use super::super::Slate;
use super::layout::is_note_text;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Riders {
    /// The status whose word leads the note.
    pub state: Option<String>,
    /// The texts after it, in order.
    pub texts: Vec<String>,
    /// Every piece that rides, the columns holding them included.
    pub all: Vec<String>,
}

fn kind<'a>(slate: &'a Slate, id: &str) -> Option<&'a str> {
    slate.doc()?.piece(id).map(|p| p.kind.as_str())
}

fn children<'a>(slate: &'a Slate, id: &str) -> &'a [String] {
    slate.doc().and_then(|d| d.piece(id)).map_or(&[], |p| p.children.as_slice())
}

fn parent<'a>(slate: &'a Slate, id: &str) -> Option<&'a str> {
    slate.doc()?.parent_id(id)
}

fn is_words(kind: Option<&str>) -> bool {
    matches!(kind, Some("status" | "text"))
}

/// The parent lays its children side by side or stacks them in one cell, rather than gathering them into cards.
fn holds_one_cell(slate: &Slate, id: &str) -> bool {
    match kind(slate, id) {
        Some("row") => true,
        Some("column") => parent(slate, id).is_some_and(|p| !matches!(kind(slate, p), Some("section" | "column"))),
        _ => false,
    }
}

/// The numbers of a strip: a row, column or grid holding two numbers or more and nothing else, but for a status
/// written right after a number and a text right after that status.
pub fn strip_cells(slate: &Slate, id: &str) -> Option<Vec<String>> {
    if !matches!(kind(slate, id), Some("row" | "column" | "grid")) {
        return None;
    }
    let all = children(slate, id);
    let kinds: Vec<Option<&str>> = all.iter().map(|c| kind(slate, c)).collect();
    let numbers: Vec<String> = all.iter().zip(&kinds).filter(|(_, k)| **k == Some("number")).map(|(c, _)| c.clone()).collect();
    if numbers.len() < 2 {
        return None;
    }
    let before = |at: usize, back: usize| at.checked_sub(back).and_then(|i| kinds[i]);
    let fits = kinds.iter().enumerate().all(|(at, k)| match k {
        Some("number") => true,
        Some("status") => before(at, 1) == Some("number"),
        Some("text") => before(at, 1) == Some("status") && before(at, 2) == Some("number"),
        _ => false,
    });
    fits.then_some(numbers)
}

pub fn is_strip(slate: &Slate, id: &str) -> bool {
    strip_cells(slate, id).is_some()
}

/// `ridersOf`: the status and texts riding a number's note.
pub fn riders_of(slate: &Slate, number: &str) -> Riders {
    let Some(parent) = parent(slate, number) else { return Riders::default() };
    let siblings = children(slate, parent);
    if holds_one_cell(slate, parent) && !is_strip(slate, parent) {
        let (mut leaves, mut all) = (Vec::new(), Vec::new());
        for sibling in siblings.iter().filter(|s| *s != number) {
            let k = kind(slate, sibling);
            let inner = children(slate, sibling);
            if is_words(k) {
                leaves.push(sibling.clone());
                all.push(sibling.clone());
            } else if k == Some("column") && !inner.is_empty() && inner.iter().all(|c| is_words(kind(slate, c))) {
                leaves.extend(inner.iter().cloned());
                all.push(sibling.clone());
                all.extend(inner.iter().cloned());
            } else {
                return Riders::default();
            }
        }
        let Some(state) = leaves.iter().find(|l| kind(slate, l) == Some("status")).cloned() else { return Riders::default() };
        let texts = leaves.into_iter().filter(|l| kind(slate, l) == Some("text")).collect();
        return Riders { state: Some(state), texts, all };
    }
    let Some(at) = siblings.iter().position(|s| s == number) else { return Riders::default() };
    let Some(state) = siblings.get(at + 1).filter(|s| kind(slate, s) == Some("status")).cloned() else { return Riders::default() };
    let strip = is_strip(slate, parent);
    let texts: Vec<String> = siblings
        .get(at + 2)
        .filter(|f| if strip { kind(slate, f) == Some("text") } else { is_note_text(slate, f) })
        .cloned()
        .into_iter()
        .collect();
    let mut all = vec![state.clone()];
    all.extend(texts.iter().cloned());
    Riders { state: Some(state), texts, all }
}

/// The number whose note a piece rides, if any.
pub fn ridden_by(slate: &Slate, id: &str) -> Option<String> {
    let parent = parent(slate, id)?;
    let grand = self::parent(slate, parent);
    let near = children(slate, parent).iter().chain(grand.map_or(&[][..], |g| children(slate, g)));
    near.filter(|c| kind(slate, c) == Some("number")).find(|c| riders_of(slate, c).all.iter().any(|r| r == id)).cloned()
}

/// A row or cell holding one number and only the words that ride it: the number's stat cell stands in its place.
pub fn is_stat_cell(slate: &Slate, id: &str) -> bool {
    let numbers: Vec<&String> = children(slate, id).iter().filter(|c| kind(slate, c) == Some("number")).collect();
    numbers.len() == 1 && holds_one_cell(slate, id) && !riders_of(slate, numbers[0]).all.is_empty()
}
