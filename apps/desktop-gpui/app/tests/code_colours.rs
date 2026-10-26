// SPDX-License-Identifier: AGPL-3.0-only
//! Code in a reply, a command row or an edit block is colored as code in a diff: `markdown::highlight_code` with the
//! Paper and Graphite themes gives each token of `pr_highlight/sample.*` the color shiki gives it (`pr_highlight/pierre.tsv`),
//! except the tokens `pr_highlight/differences.tsv` already lists for the diff.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use desktop_gpui::screens::thread_view::markdown::highlight_code;
use desktop_gpui::ui::{self, Side as ThemeSide};
use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{Hsla, Rgba, TestAppContext};

/// Each sample and the name a fence gives its language.
const SAMPLES: [(&str, &str); 5] =
    [("sample.rs", "rust"), ("sample.ts", "ts"), ("sample.py", "python"), ("sample.sh", "sh"), ("sample.json", "json")];

fn hex(color: Hsla) -> String {
    let c = Rgba::from(color);
    let byte = |v: f32| (v * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

fn normal(color: &str) -> String {
    hex(Rgba::try_from(color).unwrap().into())
}

/// Each token of today's whose bytes are not all drawn in today's color, as `file side line:col text today ours`.
fn differences(dir: &Path, side: &str, theme: &HighlightTheme) -> Vec<String> {
    let table = std::fs::read_to_string(dir.join("pierre.tsv")).unwrap();
    let plain = theme.style.editor_foreground.map(hex);
    let mut out = Vec::new();
    for (file, lang) in SAMPLES {
        let text = std::fs::read_to_string(dir.join(file)).unwrap();
        let mut colors = vec![None; text.len()];
        for (range, style) in highlight_code(lang, &text, theme) {
            if let Some(color) = style.color {
                colors[range].fill(Some(hex(color)).filter(|c| Some(c) != plain.as_ref()));
            }
        }
        let starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
        for row in table.lines().map(|l| l.split('\t').collect::<Vec<_>>()).filter(|r| r[0] == file && r[1] == side) {
            let [_, _, at, today, token] = row[..] else { panic!("pierre.tsv: {row:?}") };
            let (line, col) = at.split_once(':').unwrap();
            let start = starts[line.parse::<usize>().unwrap() - 1] + col.parse::<usize>().unwrap();
            assert_eq!(&text[start..start + token.len()], token, "{file} {at}: pierre.tsv is not this sample's");
            let today = if today == "plain" { None } else { Some(normal(today)) };
            let drawn: BTreeSet<_> =
                token.bytes().enumerate().filter(|(_, b)| !b.is_ascii_whitespace()).map(|(i, _)| colors[start + i].clone()).collect();
            if drawn.iter().any(|c| *c != today) {
                let name = |c: &Option<String>| c.clone().unwrap_or_else(|| "plain".into());
                let ours = drawn.iter().map(name).collect::<Vec<_>>().join("/");
                out.push(format!("{file}\t{side}\t{at}\t{token}\t{}\t{ours}", name(&today)));
            }
        }
    }
    out
}

#[gpui_kit::test]
fn code_in_a_reply_takes_the_colors_code_in_a_diff_takes(cx: &mut TestAppContext) {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/pr_highlight");
    let found = cx.update(|cx| {
        gpui_kit::init(cx);
        ui::init(ThemeSide::Light, cx);
        let mut found = Vec::new();
        for (theme, side) in [("paper", "light"), ("graphite", "dark")] {
            ui::theme::set_theme(theme, cx);
            found.extend(differences(&dir, side, &cx.theme().highlight_theme));
        }
        found
    });
    let mut counts = BTreeMap::new();
    for line in &found {
        let mut cols = line.split('\t');
        *counts.entry((cols.next().unwrap().to_owned(), cols.next().unwrap().to_owned())).or_insert(0) += 1;
    }
    eprintln!("tokens that differ from today's, per sample and side: {counts:?}");
    let known = std::fs::read_to_string(dir.join("differences.tsv")).unwrap();
    let known: Vec<&str> = known.lines().filter(|l| SAMPLES.iter().any(|(file, _)| l.starts_with(&format!("{file}\t")))).collect();
    let new: Vec<_> = found.iter().filter(|f| !known.contains(&f.as_str())).collect();
    let gone: Vec<_> = known.iter().filter(|k| !found.iter().any(|f| f == *k)).collect();
    assert!(
        new.is_empty() && gone.is_empty(),
        "{} tokens differ ({counts:?}), {} listed for the diff\nnew: {new:#?}\ngone: {gone:#?}",
        found.len(),
        known.len()
    );
}
