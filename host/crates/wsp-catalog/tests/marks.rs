// SPDX-License-Identifier: AGPL-3.0-only
// Ports packages/catalog/test/marks.test.ts.
use regex::Regex;
use wsp_catalog::catalog::{agent_mark, catalog_agents, AgentMark, UNMARKED_AGENTS};
use wsp_catalog::marks::{brand_marks, editor_marks, provider_marks};

const LICENSES: [&str; 3] = ["MIT", "CC0-1.0", "Apache-2.0"];

fn hex() -> Regex {
    Regex::new(r"(?i)^#[0-9a-f]{6}$").unwrap()
}

fn every_mark() -> Vec<(String, &'static AgentMark)> {
    let mut all: Vec<(String, &'static AgentMark)> = Vec::new();
    for a in catalog_agents() {
        if !UNMARKED_AGENTS.contains(&a.id.as_str()) {
            all.push((a.id.clone(), a.mark.as_ref().expect("a marked agent carries a mark")));
        }
    }
    for m in brand_marks() {
        all.push((m.id.clone(), &m.mark));
    }
    for m in provider_marks() {
        all.push((m.id.clone(), &m.mark));
    }
    for m in editor_marks() {
        all.push((format!("editor {}", m.id), &m.mark));
    }
    all
}

mod agent_marks {
    use super::*;

    #[test]
    fn every_catalog_agent_but_the_named_exceptions_carries_its_own_mark_with_where_it() {
        let unmarked: Vec<String> = catalog_agents().iter().filter(|a| a.mark.is_none()).map(|a| a.id.clone()).collect();
        let named: Vec<String> = UNMARKED_AGENTS.iter().map(|s| s.to_string()).collect();
        assert_eq!(unmarked, named);
        let source = Regex::new(r"^https://").unwrap();
        for a in catalog_agents().into_iter().filter(|a| !UNMARKED_AGENTS.contains(&a.id.as_str())) {
            let own = a.mark.as_ref().unwrap();
            let found = agent_mark(&a.id).unwrap_or_else(|| panic!("{}", a.id));
            assert!(std::ptr::eq(found, own), "{}", a.id);
            assert!(source.is_match(&own.source), "{}", a.id);
            assert!(LICENSES.contains(&own.license.as_str()), "{}", a.id);
        }
        assert_eq!(agent_mark("not-an-agent"), None);
    }

    #[test]
    fn a_mark_is_one_inline_svg_of_shapes_a_viewbox_no_text_nothing_it_loads_and_nothin() {
        let whole = Regex::new(r#"(?s)^<svg\b[^>]*\bviewBox="[\d.]+ [\d.]+ [\d.]+ [\d.]+"[^>]*>.*</svg>$"#).unwrap();
        let shapes = Regex::new(r"<(path|rect|polygon|circle)\b").unwrap();
        let banned = Regex::new(r"(?i)<(text|image|script|foreignObject|use|style)\b|\son\w+=|href=").unwrap();
        let ids_of = Regex::new(r#"\bid="([^"]+)""#).unwrap();
        let urls = Regex::new(r"url\(([^)]*)\)").unwrap();
        for (id, mark) in every_mark() {
            let svg = &mark.svg;
            assert!(whole.is_match(svg), "{id}");
            assert!(shapes.is_match(svg), "{id}");
            assert!(!banned.is_match(svg), "{id}");
            let ids: Vec<String> = ids_of.captures_iter(svg).map(|c| c[1].to_string()).collect();
            for reference in urls.captures_iter(svg) {
                let target = reference[1].strip_prefix('#').unwrap_or(&reference[1]).to_string();
                assert!(ids.contains(&target), "{id} {}", &reference[0]);
            }
        }
    }

    #[test]
    fn inks_are_a_light_and_a_dark_hex_each_the_first_fills_every_shape_without_a_fill() {
        let hex = hex();
        let named = Regex::new(r"var\(--ink-(\d+)\)").unwrap();
        for (id, mark) in every_mark() {
            let inks = mark.inks.clone().unwrap_or_default();
            for ink in &inks {
                assert!(hex.is_match(&ink.light), "{id}");
                assert!(hex.is_match(&ink.dark), "{id}");
            }
            for at in named.captures_iter(&mark.svg).map(|c| c[1].parse::<usize>().unwrap()) {
                assert!(at > 0, "{id}");
                assert!(at < inks.len(), "{id}");
            }
        }
    }
}
