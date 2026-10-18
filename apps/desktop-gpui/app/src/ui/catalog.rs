// SPDX-License-Identifier: AGPL-3.0-only
//! What the app draws of each agent, read from the catalog the web app draws it from (packages/catalog/src/agents, with
//! each mark's source and license beside it): its name, and its mark filled with its inks for the side the window
//! draws, else the ink of the text around it, as HarnessMark.tsx fills them.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme;
use gpui_kit::{div, img, prelude::*, AnyElement, App, FontWeight, Hsla, Image, ImageFormat, Pixels, Rgba};

const AGENTS: [(&str, &str); 11] = [
    ("amp", include_str!("../../../../../packages/catalog/src/agents/amp.ts")),
    ("claude", include_str!("../../../../../packages/catalog/src/agents/claude.ts")),
    ("codex", include_str!("../../../../../packages/catalog/src/agents/codex.ts")),
    ("crush", include_str!("../../../../../packages/catalog/src/agents/crush.ts")),
    ("cursor", include_str!("../../../../../packages/catalog/src/agents/cursor.ts")),
    ("gemini", include_str!("../../../../../packages/catalog/src/agents/gemini.ts")),
    ("goose", include_str!("../../../../../packages/catalog/src/agents/goose.ts")),
    ("hermes", include_str!("../../../../../packages/catalog/src/agents/hermes.ts")),
    ("opencode", include_str!("../../../../../packages/catalog/src/agents/opencode.ts")),
    ("pi", include_str!("../../../../../packages/catalog/src/agents/pi.ts")),
    ("qwen", include_str!("../../../../../packages/catalog/src/agents/qwen.ts")),
];

fn source(agent: &str) -> Option<&'static str> {
    AGENTS.iter().find(|(id, _)| *id == agent).map(|(_, src)| *src)
}

/// `agentName`: the catalog's name for the agent, or the id itself for one the catalog does not list.
pub fn name(agent: &str) -> String {
    let named = source(agent).and_then(|src| {
        let at = src.find("\n  name: \"")? + "\n  name: \"".len();
        Some(&src[at..at + src[at..].find('"')?])
    });
    named.map_or_else(|| agent.to_owned(), str::to_owned)
}

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

/// The mark's first ink, light and dark, where it has one; the Usage chart draws the agent's line in it.
pub fn ink(agent: &str) -> Option<(Rgba, Rgba)> {
    let (light, dark) = *read(source(agent)?)?.inks.first()?;
    Some((Rgba::try_from(light).ok()?, Rgba::try_from(dark).ok()?))
}

/// Each drawn mark by agent, side and text ink.
type Drawn = HashMap<(&'static str, bool, u32), Option<Arc<Image>>>;

thread_local! {
    /// One image per agent, side and text ink: the catalog's agents in each theme at most.
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

/// `HarnessMark`: the agent's mark at `size`, or its initials where it has none; a mark with no inks of its own, and
/// the initials, take `ink`, the text's around them.
pub fn mark(harness: &str, label: &str, size: Pixels, ink: Hsla, cx: &App) -> AnyElement {
    match image(harness, cx.theme().is_dark(), ink) {
        Some(image) => img(image).size(size).flex_none().into_any_element(),
        None => div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .text_size(size * 0.7)
            .font_family(cx.theme().mono_font_family.clone())
            .font_weight(FontWeight::EXTRA_BOLD)
            .text_color(ink)
            .child(initials(label))
            .into_any_element(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_catalogs_names_marks_and_inks_read() {
        let claude = read(AGENTS[1].1).expect("claude's mark");
        assert!(claude.svg.starts_with("<svg viewBox=\"0 0 256 257\">"));
        assert_eq!(claude.inks, [("#c4643f", "#d97757")]);
        assert_eq!(read(source("pi").unwrap()).unwrap().inks.len(), 3);
        for (id, src) in AGENTS {
            assert_eq!(read(src).is_some(), id != "crush", "{id}: the catalog draws a mark for every agent but Crush");
        }
        assert!(image("claude", true, gpui_kit::white()).is_some());
        assert!(image("codex", true, gpui_kit::white()).is_some(), "a mark with no inks takes the text's");
        assert!(image("nobody", true, gpui_kit::white()).is_none());
        assert_eq!(initials("Claude Code"), "CC");
        assert_eq!(initials("codex"), "CO");
        let names: Vec<String> = ["claude", "codex", "opencode", "hermes", "gemini", "crush", "someone"].map(name).into();
        assert_eq!(names, ["Claude Code", "Codex", "OpenCode", "Hermes Agent", "Gemini CLI", "Crush", "someone"]);
        assert_eq!(ink("claude"), Some((Rgba::try_from("#c4643f").unwrap(), Rgba::try_from("#d97757").unwrap())));
        assert_eq!(ink("codex"), None);
    }
}
