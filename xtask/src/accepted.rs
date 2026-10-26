// SPDX-License-Identifier: AGPL-3.0-only
//! `parity/accepted.tsv`: the differences the owner said yes to, one a line as `tape<TAB>field<TAB>why`. A `*` in the
//! tape or the field matches any run of characters. A difference no line names fails the run, and a line that matched
//! nothing is named, since the list only shrinks.

use std::fs;
use std::path::Path;

pub const ACCEPTED_FILE: &str = "parity/accepted.tsv";

#[derive(Clone, Debug)]
pub struct Line {
    pub tape: String,
    pub field: String,
    pub why: String,
    pub at: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Accepted {
    pub lines: Vec<Line>,
}

impl Accepted {
    pub fn read(root: &Path) -> Result<Accepted, String> {
        let path = root.join(ACCEPTED_FILE);
        let Ok(text) = fs::read_to_string(&path) else { return Ok(Accepted::default()) };
        Accepted::parse(&text)
    }

    pub fn parse(text: &str) -> Result<Accepted, String> {
        let mut lines = Vec::new();
        for (n, raw) in text.lines().enumerate() {
            if raw.trim().is_empty() || raw.starts_with('#') {
                continue;
            }
            let parts: Vec<&str> = raw.split('\t').collect();
            let [tape, field, why] = parts.as_slice() else {
                return Err(format!("{ACCEPTED_FILE}:{}: a line is tape, field and why, split by tabs", n + 1));
            };
            if why.trim().is_empty() {
                return Err(format!("{ACCEPTED_FILE}:{}: an accepted difference says why", n + 1));
            }
            lines.push(Line { tape: tape.to_string(), field: field.to_string(), why: why.to_string(), at: n + 1 });
        }
        Ok(Accepted { lines })
    }

    /// The index of the first line that accepts this field of this tape.
    pub fn accepts(&self, tape: &str, field: &str) -> Option<usize> {
        self.lines.iter().position(|l| glob(&l.tape, tape) && glob(&l.field, field))
    }
}

/// Whether `text` matches `pattern`, where `*` stands for any run of characters and nothing else is special.
pub fn glob(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }
    let (first, last) = (parts[0], parts[parts.len() - 1]);
    if !text.starts_with(first) || text.len() < first.len() + last.len() || !text.ends_with(last) {
        return false;
    }
    let mut rest = &text[first.len()..text.len() - last.len()];
    for part in &parts[1..parts.len() - 1] {
        match rest.find(part) {
            Some(i) => rest = &rest[i + part.len()..],
            None => return false,
        }
    }
    true
}
