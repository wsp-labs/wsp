// SPDX-License-Identifier: AGPL-3.0-only
//! The queries code is colored with, so it takes the colors today's app gives it (shiki with the pierre themes, through
//! the `highlight` blocks `scripts/pierre-themes.mjs` writes). Each replaces gpui-kit's query for its language in the
//! process's registry and keeps gpui-kit's grammar and injections. gpui-kit keeps the first capture of a node, so a
//! query lists its specific patterns before the catch-alls; none runs locals, whose captures would hide those patterns.

use gpui_kit::component::highlighter::LanguageRegistry;
use gpui_kit::SharedString;

const JSX: &str = include_str!("syntax/jsx.scm");

fn queries() -> [(&'static str, String); 7] {
    [
        ("javascript", [JSX, include_str!("syntax/javascript.scm")].concat()),
        ("typescript", include_str!("syntax/typescript.scm").into()),
        ("tsx", [JSX, include_str!("syntax/typescript.scm")].concat()),
        ("rust", include_str!("syntax/rust.scm").into()),
        ("python", include_str!("syntax/python.scm").into()),
        ("markdown", include_str!("syntax/markdown.scm").into()),
        ("markdown_inline", include_str!("syntax/markdown_inline.scm").into()),
    ]
}

pub fn init() {
    let registry = LanguageRegistry::singleton();
    for (name, highlights) in queries() {
        let mut config = registry.language(name).unwrap_or_else(|| panic!("gpui-kit has no {name} grammar"));
        config.highlights = SharedString::from(highlights);
        config.locals = SharedString::default();
        registry.register(name, &config);
    }
}
