// SPDX-License-Identifier: AGPL-3.0-only
//! `HarnessMark`: an agent's mark beside its name, the catalog's own svg in its inks for the side the theme is on, or
//! in the ink of whatever it sits in where it has none; the initials for an agent with no mark.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::ActiveTheme as _;
use gpui_kit::{div, img, prelude::*, AnyElement, App, FontWeight, Hsla, Image, ImageFormat, Pixels, Rgba};

use super::catalog::AGENTS;
use crate::ui::ActivePalette;

/// The svg is drawn at four times the size it stands at, so it stays sharp on a dense screen.
const OVER: f32 = 4.;

thread_local! {
    static DRAWN: RefCell<HashMap<String, Arc<Image>>> = RefCell::new(HashMap::new());
}

fn hex(color: Hsla) -> String {
    let c: Rgba = color.into();
    let byte = |v: f32| (v.clamp(0., 1.) * 255.).round() as u8;
    format!("#{:02x}{:02x}{:02x}", byte(c.r), byte(c.g), byte(c.b))
}

/// `harnessInitials`: two letters off one word, else the first letter of two.
pub fn initials(label: &str) -> String {
    let words: Vec<&str> = label.split_whitespace().collect();
    match words.as_slice() {
        [] => String::new(),
        [one] => one.chars().take(2).collect::<String>().to_uppercase(),
        [a, b, ..] => [a, b].iter().filter_map(|w| w.chars().next()).collect::<String>().to_uppercase(),
    }
}

/// The agent's mark at `size`, or its initials where the catalog has no mark for it.
pub fn harness_mark(id: &str, label: &str, size: Pixels, cx: &App) -> AnyElement {
    let ink = cx.palette().foreground;
    let mark = AGENTS.iter().find(|a| a.id == id).and_then(|a| a.mark.as_ref());
    let Some(mark) = mark else {
        return div()
            .flex()
            .flex_none()
            .items_center()
            .justify_center()
            .size(size)
            .font_family(cx.theme().mono_font_family.clone())
            .font_weight(FontWeight::EXTRA_BOLD)
            .text_size(size * 0.7)
            .line_height(size)
            .text_color(ink)
            .child(initials(label))
            .into_any_element();
    };
    let dark = cx.theme().is_dark();
    let inks: Vec<String> = mark.inks.iter().map(|(light, dark_ink)| if dark { dark_ink } else { light }.to_string()).collect();
    let first = inks.first().cloned().unwrap_or_else(|| hex(ink));
    let key = format!("{id}:{first}:{dark}:{}", f32::from(size));
    let image = DRAWN.with(|drawn| {
        drawn
            .borrow_mut()
            .entry(key)
            .or_insert_with(|| {
                let mut body = mark.body.replace("currentColor", &first);
                for (at, color) in inks.iter().enumerate() {
                    body = body.replace(&format!("var(--ink-{at})"), color);
                }
                let side = f32::from(size) * OVER;
                let svg = format!(
                    r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{}" width="{side}" height="{side}" preserveAspectRatio="xMidYMid" fill="{first}">{body}</svg>"#,
                    mark.view_box
                );
                Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
            })
            .clone()
    });
    img(image).flex_none().size(size).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initials_read_as_the_web_reads_them() {
        assert_eq!(initials("Crush"), "CR");
        assert_eq!(initials("Hermes Agent"), "HA");
        assert_eq!(initials(""), "");
    }
}
