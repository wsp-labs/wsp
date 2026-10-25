// SPDX-License-Identifier: AGPL-3.0-only
//! Code in a diff is colored as today's app colors it. `pr_highlight/pierre.tsv` holds the color shiki 4.4.3 with
//! @pierre/theme 1.1.0 gives each token of each `pr_highlight/sample.*` (light and dark), written by
//! `scripts/shiki-tokens.mjs` from the installed packages. Each sample is read as a patch that adds the file, and gpui-kit's
//! diff colors it as it draws it (the language from the path, wsp's queries, the captures merged), with the Paper and
//! Graphite blocks; every token whose bytes take another color is listed. The list must be
//! `pr_highlight/differences.tsv`: the tokens that cannot match without a grammar of their own.

use std::collections::BTreeSet;
use std::fmt::Write as _;
use std::path::Path;

use desktop_gpui::ui::{self, Side as ThemeSide};
use gpui_kit::component::diff::{self, DiffFile, DiffSide};
use gpui_kit::component::highlighter::HighlightTheme;
use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{Hsla, Rgba, TestAppContext};

const SAMPLES: [&str; 9] =
    ["sample.ts", "sample.tsx", "sample.js", "sample.rs", "sample.json", "sample.css", "sample.md", "sample.sh", "sample.py"];

fn hex(color: Hsla) -> String {
    let c = Rgba::from(color);
    let byte = |v: f32| (v * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

/// `#rrggbb` as a theme file's color reads back, so both sides round alike.
fn normal(color: &str) -> String {
    hex(Rgba::try_from(color).unwrap().into())
}

/// The color gpui-kit's diff draws each byte of `text` in when a patch adds it as `file`, or None where it draws the
/// plain text color.
fn ours(file: &str, text: &str, theme: &HighlightTheme) -> Vec<Option<String>> {
    let added: String = text.lines().map(|line| format!("+{line}\n")).collect();
    let patch = format!(
        "diff --git a/{file} b/{file}\nnew file mode 100644\n--- /dev/null\n+++ b/{file}\n@@ -0,0 +1,{} @@\n{added}",
        text.lines().count()
    );
    let parsed = DiffFile::parse(&patch).unwrap().remove(0);
    let plain = theme.style.editor_foreground.map(hex);
    let mut colors = vec![None; text.len()];
    let mut start = 0;
    for (line, runs) in text.lines().zip(diff::syntax_colors(&parsed, DiffSide::Modified, theme)) {
        for (range, style) in runs {
            if let Some(color) = style.color {
                colors[start + range.start..start + range.end].fill(Some(hex(color)).filter(|c| Some(c) != plain.as_ref()));
            }
        }
        start += line.len() + 1;
    }
    colors
}

/// Each token of today's whose bytes are not all drawn in today's color, as `file side line:col text today ours`.
fn differences(dir: &Path, side: &str, theme: &HighlightTheme) -> Vec<String> {
    let table = std::fs::read_to_string(dir.join("pierre.tsv")).unwrap();
    let mut out = Vec::new();
    for file in SAMPLES {
        let text = std::fs::read_to_string(dir.join(file)).unwrap();
        let starts: Vec<usize> = std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1)).collect();
        let colors = ours(file, &text, theme);
        let rows = table.lines().map(|l| l.split('\t').collect::<Vec<_>>()).filter(|r| r[0] == file && r[1] == side);
        let mut seen = 0;
        for row in rows {
            seen += 1;
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
        assert!(seen > 0, "pierre.tsv has no {side} rows for {file}");
    }
    out
}

#[gpui_kit::test]
fn code_takes_the_colors_todays_app_gives_it(cx: &mut TestAppContext) {
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
    let known = std::fs::read_to_string(dir.join("differences.tsv")).unwrap();
    let known: Vec<&str> = known.lines().filter(|l| !l.is_empty()).collect();
    let mut report = String::new();
    for line in &found {
        if !known.contains(&line.as_str()) {
            writeln!(report, "new: {line}").unwrap();
        }
    }
    for line in &known {
        if !found.iter().any(|f| f == line) {
            writeln!(report, "gone: {line}").unwrap();
        }
    }
    assert!(report.is_empty(), "{} tokens differ from today's app, {} listed\n{report}", found.len(), known.len());
}
