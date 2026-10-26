// SPDX-License-Identifier: AGPL-3.0-only
//! gpui-kit's own coloring is what it was before wsp's highlight patch: its built-in queries still capture what they
//! did (the input of gpui-kit's `test_html_script_injects_javascript_highlights`, which this checkout cannot build), and
//! the four names the patch adds sit after the 41 it had, falling back to their first part in a theme that leaves
//! them out. No `ui::init` here: wsp's queries replace gpui-kit's in the registry once it runs.

use std::sync::Mutex;

use gpui_kit::base::input::HighlightStyleResolver;
use gpui_kit::component::highlighter::{SyntaxColors, SyntaxHighlighter};
use gpui_kit::component::Rope;
use gpui_kit::{hsla, HighlightStyle};

/// Hands each capture name its own color, so the color a range is drawn in names its capture.
#[derive(Default)]
struct Names(Mutex<Vec<String>>);

impl HighlightStyleResolver for Names {
    fn style(&self, name: &str) -> Option<HighlightStyle> {
        let mut names = self.0.lock().unwrap();
        let ix = names.iter().position(|n| n == name).unwrap_or_else(|| {
            names.push(name.to_string());
            names.len() - 1
        });
        Some(HighlightStyle { color: Some(hsla(ix as f32 / 1000.0, 1.0, 0.5, 1.0)), ..Default::default() })
    }
}

/// A theme's colors as gpui-kit's own theme resolves them.
struct Colors(SyntaxColors);

impl HighlightStyleResolver for Colors {
    fn style(&self, name: &str) -> Option<HighlightStyle> {
        self.0.style(name)
    }
}

/// The style the start of each of `words` (the first place each is found in `text`) is drawn in, by gpui-kit's
/// default registry.
fn styled(language: &str, text: &str, words: &[&str], theme: &dyn HighlightStyleResolver) -> Vec<Option<HighlightStyle>> {
    let mut highlighter = SyntaxHighlighter::new(language);
    highlighter.update(None, &Rope::from_str(text), None);
    let styles = highlighter.styles(&(0..text.len()), theme);
    words
        .iter()
        .map(|word| {
            let at = text.find(word).unwrap();
            styles.iter().find(|(r, _)| r.start <= at && at < r.end).map(|(_, style)| *style)
        })
        .collect()
}

/// The capture each of `words` is drawn as.
fn drawn(language: &str, text: &str, words: &[&str]) -> Vec<String> {
    let names = Names::default();
    let styles = styled(language, text, words, &names);
    let names = names.0.lock().unwrap();
    styles
        .iter()
        .map(|style| style.and_then(|s| s.color).map_or("none".into(), |c| names[(c.h * 1000.0).round() as usize].clone()))
        .collect()
}

#[test]
fn gpui_kits_queries_capture_what_they_did() {
    let html = "<script>\nconst answer = 42;\nconsole.log(answer);\n</script>\n";
    assert_eq!(drawn("html", html, &["const", "answer"]), ["keyword", "variable"]);
    let ts = "const MAX = 1;\nfunction f(list: Row) { return this; }\n";
    assert_eq!(drawn("typescript", ts, &["const", "MAX", "function", "this"]), ["keyword", "variable", "keyword", "variable.builtin"]);
    let rs = "struct A;\nfn f(value: u32) { let low = value; }\n";
    assert_eq!(drawn("rust", rs, &["struct", "let", "value"]), ["keyword", "keyword", "variable.parameter"]);
}

/// gpui-kit runs each language's locals query, whose captures come first: a parameter, and a name used in a body, are
/// drawn as `local.*` there, not as the highlight query's capture.
#[test]
fn gpui_kit_still_runs_locals() {
    let ts = "function f(list: Row) { return list; }\n";
    assert_eq!(drawn("typescript", ts, &["list:"]), ["local.definition"]);
    let js = "function g(param) { return param + 1; }\n";
    assert_eq!(drawn("javascript", js, &["param)", "param +"]), ["local.definition", "local.reference"]);
}

const OLD_NAMES: [&str; 41] = [
    "attribute",
    "boolean",
    "comment",
    "comment.doc",
    "constant",
    "constructor",
    "embedded",
    "emphasis",
    "emphasis.strong",
    "enum",
    "function",
    "hint",
    "keyword",
    "label",
    "link_text",
    "link_uri",
    "number",
    "operator",
    "predictive",
    "preproc",
    "primary",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "punctuation.list_marker",
    "punctuation.special",
    "string",
    "string.escape",
    "string.regex",
    "string.special",
    "string.special.symbol",
    "tag",
    "tag.doctype",
    "text.code.span",
    "text.literal",
    "title",
    "type",
    "variable",
    "variable.special",
    "variant",
];

#[test]
fn the_new_names_come_after_the_old_and_fall_back_when_a_theme_leaves_them_out() {
    let theme: serde_json::Map<String, serde_json::Value> = OLD_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_string(), serde_json::json!({ "color": format!("#0000{i:02x}ff") })))
        .collect();
    let colors: SyntaxColors = serde_json::from_value(theme.into()).unwrap();
    for (i, name) in OLD_NAMES.iter().enumerate() {
        assert_eq!(colors.style_for_index(i), colors.style(name), "index {i}");
        assert!(colors.style(name).is_some(), "{name}");
    }
    for (new, old) in [("constant.builtin", "constant"), ("keyword.storage", "keyword"), ("variable.parameter", "variable")] {
        assert_eq!(colors.style(new), colors.style(old), "{new}");
    }
    assert_eq!(colors.style("namespace"), None);
    let theme = Colors(colors.clone());
    let builtin = |language, text, word| styled(language, text, &[word], &theme)[0];
    assert_eq!(builtin("typescript", "f(this);\n", "this"), colors.style("variable"), "`this` in TypeScript");
    assert_eq!(builtin("rust", "fn f(&self) { self.g(); }\n", "self."), colors.style("variable"), "`self` in Rust");
    assert_eq!(colors.style("variable.builtin"), colors.style("variable"), "variable.builtin falls back to variable");
    assert_eq!(colors.style_for_index(41), colors.style("constant.builtin"));
}

/// The names wsp's highlight patch appends, in order, each read from a theme as its own color.
const NEW_NAMES: [&str; 16] = [
    "constant.builtin",
    "keyword.storage",
    "namespace",
    "variable.parameter",
    "constant.other",
    "property.json",
    "constant.builtin.json",
    "property.css",
    "property.name",
    "attribute.pseudo",
    "keyword.unit",
    "punctuation.link",
    "text.quote",
    "variable.parameter.python",
    "type.builtin.python",
    "attribute.id",
];

#[test]
fn each_new_name_reads_its_own_color() {
    let theme: serde_json::Map<String, serde_json::Value> = NEW_NAMES
        .iter()
        .enumerate()
        .map(|(i, name)| (name.to_string(), serde_json::json!({ "color": format!("#00{i:02x}00ff") })))
        .collect();
    let colors: SyntaxColors = serde_json::from_value(theme.into()).unwrap();
    for (i, name) in NEW_NAMES.iter().enumerate() {
        assert!(colors.style(name).is_some(), "{name}");
        assert_eq!(colors.style_for_index(OLD_NAMES.len() + i), colors.style(name), "{name}");
    }
    assert_eq!(colors.style_for_index(OLD_NAMES.len() + NEW_NAMES.len()), None);
}
