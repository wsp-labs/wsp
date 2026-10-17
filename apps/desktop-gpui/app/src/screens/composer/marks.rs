// SPDX-License-Identifier: AGPL-3.0-only
//! Each agent's mark, read from the catalog the web app draws them from (packages/catalog/src/agents, with their
//! sources and licenses beside each mark), filled with its inks for the side the window draws, else the ink of the
//! text around it, as HarnessMark.tsx fills them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::{Hsla, Image, ImageFormat, Rgba};

const AGENTS: [(&str, &str); 10] = [
    ("amp", include_str!("../../../../../../packages/catalog/src/agents/amp.ts")),
    ("claude", include_str!("../../../../../../packages/catalog/src/agents/claude.ts")),
    ("codex", include_str!("../../../../../../packages/catalog/src/agents/codex.ts")),
    ("cursor", include_str!("../../../../../../packages/catalog/src/agents/cursor.ts")),
    ("gemini", include_str!("../../../../../../packages/catalog/src/agents/gemini.ts")),
    ("goose", include_str!("../../../../../../packages/catalog/src/agents/goose.ts")),
    ("hermes", include_str!("../../../../../../packages/catalog/src/agents/hermes.ts")),
    ("opencode", include_str!("../../../../../../packages/catalog/src/agents/opencode.ts")),
    ("pi", include_str!("../../../../../../packages/catalog/src/agents/pi.ts")),
    ("qwen", include_str!("../../../../../../packages/catalog/src/agents/qwen.ts")),
];

/// A mark's svg and its inks, light then dark, as its catalog module writes them.
struct Mark {
    svg: &'static str,
    inks: Vec<(&'static str, &'static str)>,
}

fn read(src: &'static str) -> Option<Mark> {
    let mark = &src[src.find("mark: {")?..];
    let svg_at = mark.find("svg: `")? + "svg: `".len();
    let svg = &mark[svg_at..svg_at + mark[svg_at..].find('`')?];
    let head = &mark[..svg_at];
    let mut inks = Vec::new();
    let mut rest = head.find("inks: [").map_or("", |at| &head[at..]);
    while let Some(at) = rest.find("light: \"") {
        let light_at = at + "light: \"".len();
        let light = &rest[light_at..light_at + rest[light_at..].find('"')?];
        let dark_at = light_at + rest[light_at..].find("dark: \"")? + "dark: \"".len();
        let dark = &rest[dark_at..dark_at + rest[dark_at..].find('"')?];
        inks.push((light, dark));
        rest = &rest[dark_at..];
    }
    Some(Mark { svg, inks })
}

/// Each drawn mark by agent, side and text ink.
type Drawn = HashMap<(&'static str, bool, u32), Option<Arc<Image>>>;

thread_local! {
    /// One image per agent, side and text ink: the catalog's ten agents in each theme at most.
    static DRAWN: RefCell<Drawn> = RefCell::new(HashMap::new());
}

/// The agent's mark for one side, in its inks or else `text`'s, or none where the catalog draws none and the
/// initials stand in.
pub fn image(harness: &str, dark: bool, text: Hsla) -> Option<Arc<Image>> {
    let (id, src) = AGENTS.iter().find(|(id, _)| *id == harness)?;
    let text = Rgba::from(text);
    let hex = |c: f32| (c * 255.).round() as u32;
    let text_rgb = hex(text.r) << 16 | hex(text.g) << 8 | hex(text.b);
    let text_hex = format!("#{text_rgb:06x}");
    DRAWN.with(|drawn| {
        drawn
            .borrow_mut()
            .entry((id, dark, text_rgb))
            .or_insert_with(|| {
                let mark = read(src)?;
                let ink = |n: usize| mark.inks.get(n).map(|(light, d)| if dark { *d } else { *light });
                let first = ink(0).unwrap_or(text_hex.as_str());
                let mut svg = mark.svg.replacen("<svg ", &format!("<svg xmlns=\"http://www.w3.org/2000/svg\" fill=\"{first}\" "), 1);
                for n in 0..mark.inks.len() {
                    svg = svg.replace(&format!("var(--ink-{n})"), ink(n).unwrap_or(first));
                }
                svg = svg.replace("currentColor", first);
                Some(Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes())))
            })
            .clone()
    })
}

/// HarnessMark.tsx `harnessInitials`: two letters of one word, or the first letters of two.
pub fn initials(label: &str) -> String {
    let words: Vec<&str> = label.split_whitespace().collect();
    match words.as_slice() {
        [] => String::new(),
        [one] => one.chars().take(2).collect::<String>().to_uppercase(),
        more => more.iter().take(2).filter_map(|w| w.chars().next()).collect::<String>().to_uppercase(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogs_marks_read_with_their_inks() {
        let claude = read(AGENTS[1].1).expect("claude's mark");
        assert!(claude.svg.starts_with("<svg viewBox=\"0 0 256 257\">"));
        assert_eq!(claude.inks, [("#c4643f", "#d97757")]);
        assert_eq!(read(AGENTS[8].1).unwrap().inks.len(), 3);
        for (id, src) in AGENTS {
            assert!(read(src).is_some(), "{id} has a mark the reader finds");
        }
        assert!(image("claude", true, gpui_kit::white()).is_some());
        assert!(image("codex", true, gpui_kit::white()).is_some(), "a mark with no inks takes the text's");
        assert!(image("nobody", true, gpui_kit::white()).is_none());
        assert_eq!(initials("Claude Code"), "CC");
        assert_eq!(initials("codex"), "CO");
    }
}
