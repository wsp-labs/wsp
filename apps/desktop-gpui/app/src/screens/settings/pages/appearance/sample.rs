// SPDX-License-Identifier: AGPL-3.0-only
//! `TypeSample`: a reply with a line of code, in the faces and sizes picked, drawn as the conversation draws it
//! (`ChatMarkdown.tsx`, `codeBlocks.tsx` MarkdownCodeBlock): the sentence, then the code block with its language at
//! the left of its head, Wrap lines and Copy code at the right, and the line coloured as the web's shiki colours it
//! for the side drawn (record/type-sample.json, written by type-sample.mjs).

use std::ops::Range;
use std::sync::OnceLock;
use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, relative, App, ClipboardItem, Entity, HighlightStyle, Hsla, Rgba, SharedString, StyledText, Window};
use serde::Deserialize;
use serde_json::Value;

use crate::ui::{size, tip, ActivePalette, Glyph};

/// `MarkdownCodeBlock`'s words.
mod words {
    pub const WRAP: &str = "Wrap lines";
    pub const UNWRAP: &str = "Disable line wrap";
    pub const COPY: &str = "Copy code";
    pub const COPIED: &str = "Copied";
}

/// How long Copied stands after a copy.
const COPIED_FOR: Duration = Duration::from_millis(1200);
/// `--radius`.
const BLOCK_RADIUS: gpui_kit::Pixels = px(10.);

#[derive(Deserialize)]
struct Side {
    fg: String,
    tokens: Vec<(String, String)>,
}

#[derive(Deserialize)]
struct Record {
    language: String,
    code: String,
    light: Side,
    dark: Side,
}

/// The code the sample shows, for a test that shapes it.
pub fn code() -> &'static str {
    &record().code
}

fn record() -> &'static Record {
    static RECORD: OnceLock<Record> = OnceLock::new();
    RECORD.get_or_init(|| {
        serde_json::from_str(include_str!("../../../../../record/type-sample.json")).expect("the type sample's record reads")
    })
}

fn ink(hex: &str) -> Hsla {
    Rgba::try_from(hex).map(Hsla::from).unwrap_or_default()
}

/// The code's colour runs for one side, by byte range.
fn runs(side: &Side) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut at = 0;
    side.tokens
        .iter()
        .map(|(text, color)| {
            let range = at..at + text.len();
            at = range.end;
            (range, HighlightStyle { color: Some(ink(color)), ..HighlightStyle::default() })
        })
        .collect()
}

/// The sentence of a markdown sample, the words before its fence.
fn sentence(sample: &str) -> &str {
    sample.split("\n\n```").next().unwrap_or(sample)
}

/// Whether the block wraps its lines and whether Copied stands, held across frames.
struct Chrome {
    wrapped: bool,
    copied: bool,
    epoch: usize,
}

/// One of the head's 24 px ghost icon buttons (`size="icon-xs"`, `chat-markdown-chrome-action`): the head's ink, the
/// full ink and a faint fill while pressed, the accent under the pointer.
fn chrome_action(id: &'static str, glyph: Glyph, pressed: bool, said: &'static str, cx: &App) -> gpui_kit::Stateful<gpui_kit::Div> {
    let p = cx.palette();
    let (ink, over) = (if pressed { p.foreground } else { p.foreground.opacity(0.72) }, p.foreground);
    let glyph_ink = if pressed { p.foreground } else { p.muted_foreground };
    div()
        .id(id)
        .debug_selector(move || format!("type-sample-{id}"))
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(24.))
        .rounded(size::BUTTON_RADIUS)
        .cursor_pointer()
        .text_color(ink)
        .when(pressed, |el| el.bg(p.foreground.opacity(0.08)))
        .hover(move |s| s.text_color(over))
        .tooltip(tip(said))
        .child(Icon::new(glyph).with_size(px(12.)).text_color(glyph_ink))
}

pub fn type_sample(p: &Value, sample: &'static str, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let palette = cx.palette().clone();
    let dark = cx.theme().is_dark();
    let picked = |field: &str| p[field].as_str().filter(|f| !f.is_empty()).map(|f| SharedString::from(f.to_owned()));
    // `text-[length:var(--font-size-chat,0.875rem)]`; the code inherits it until a code size is picked.
    let text_size = p["textSize"].as_i64().unwrap_or(14) as f32;
    let code_size = p["codeSize"].as_i64().map_or(text_size, |s| s as f32);
    let code_face = picked("codeFont").unwrap_or_else(|| cx.theme().mono_font_family.clone());
    let chrome: Entity<Chrome> = window.use_keyed_state("type-sample-chrome", cx, |_, _| Chrome { wrapped: true, copied: false, epoch: 0 });
    let Chrome { wrapped, copied, .. } = *chrome.read(cx);
    let record = record();
    let side = if dark { &record.dark } else { &record.light };
    let code = StyledText::new(record.code.clone()).with_highlights(runs(side));

    let toggle = chrome.clone();
    let wrap =
        chrome_action("wrap", Glyph::TextWrap, wrapped, if wrapped { words::UNWRAP } else { words::WRAP }, cx).on_click(move |_, _, cx| {
            toggle.update(cx, |c, cx| {
                c.wrapped = !c.wrapped;
                cx.notify();
            })
        });
    let held = chrome.clone();
    let copy =
        chrome_action("copy", if copied { Glyph::Check } else { Glyph::Copy }, false, if copied { words::COPIED } else { words::COPY }, cx)
            .on_click(move |_, _, cx| {
                cx.write_to_clipboard(ClipboardItem::new_string(record.code.clone()));
                let epoch = held.update(cx, |c, cx| {
                    c.copied = true;
                    c.epoch += 1;
                    cx.notify();
                    c.epoch
                });
                let held = held.downgrade();
                cx.spawn(async move |cx| {
                    cx.background_executor().timer(COPIED_FOR).await;
                    _ = held.update(cx, |c, cx| {
                        if c.epoch == epoch {
                            c.copied = false;
                            cx.notify();
                        }
                    });
                })
                .detach();
            });
    let head = h_flex()
        .justify_between()
        .gap_2()
        .pt(px(6.))
        .pr(px(6.))
        .pl_3()
        .text_color(palette.foreground.opacity(0.72))
        .child(
            div()
                .debug_selector(|| "type-sample-language".into())
                .min_w_0()
                .truncate()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(11.))
                .line_height(relative(1.375))
                .child(record.language.clone()),
        )
        .child(h_flex().gap(px(2.)).child(wrap).child(copy));
    // `pre`: `padding: 0.8rem 0.9rem`, `white-space: pre-wrap` while wrapped, else one line that scrolls.
    let lines = div()
        .id("type-sample-code")
        .debug_selector(|| "type-sample-code".into())
        .px(px(14.4))
        .py(px(12.8))
        .font_family(code_face)
        .text_size(px(code_size))
        .line_height(relative(1.375))
        .text_color(ink(&side.fg))
        .map(|el| if wrapped { el.child(code) } else { el.overflow_x_scroll().child(div().whitespace_nowrap().child(code)) });
    let block = v_flex()
        .debug_selector(|| "type-sample-block".into())
        .mt(px(10.4))
        .overflow_hidden()
        .rounded(BLOCK_RADIUS)
        .border_1()
        // `CODE_BLOCK_SURFACE`: `border-border/70 bg-secondary dark:border-transparent dark:bg-input/32`.
        .map(|el| {
            if dark {
                el.border_color(gpui_kit::transparent_black()).bg(palette.input.opacity(0.32))
            } else {
                el.border_color(palette.border.opacity(0.7)).bg(palette.muted)
            }
        })
        .child(head)
        .child(lines);
    div()
        .debug_selector(|| "type-sample".into())
        .rounded(size::CARD_RADIUS)
        .border_1()
        .border_color(palette.border.opacity(0.6))
        .bg(palette.card.opacity(0.4))
        .px(size::CARD_INSET)
        .py_3()
        .when_some(picked("appFont"), |el, face| el.font_family(face))
        // `text-[length:var(--font-size-chat,0.875rem)] leading-relaxed text-foreground/80`.
        .text_size(px(text_size))
        .line_height(relative(1.625))
        .text_color(palette.foreground.opacity(0.8))
        .child(div().child(sentence(sample)))
        .child(block)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_record_holds_the_samples_code_coloured_whole_on_both_sides() {
        let record = record();
        let sample = super::super::words::SAMPLE;
        assert_eq!(
            sample,
            format!("{}\n\n```{}\n{}\n```", sentence(sample), record.language, record.code),
            "the record's code is the sample's"
        );
        for side in [&record.light, &record.dark] {
            assert_eq!(side.tokens.iter().map(|(t, _)| t.as_str()).collect::<String>(), record.code, "the tokens spell the code");
            assert_eq!(runs(side).last().map(|(r, _)| r.end), Some(record.code.len()));
            assert!(side.tokens.iter().all(|(_, c)| Rgba::try_from(c.as_str()).is_ok()), "every colour reads");
        }
        assert_ne!(record.light.tokens[0].1, record.dark.tokens[0].1, "each side colours its own way");
    }

    /// Needs node and the web package installed (pnpm install).
    #[test]
    fn the_record_is_what_shiki_writes_now() {
        let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("type-sample.mjs");
        let out = std::process::Command::new("node")
            .arg(script)
            .arg("--check")
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output()
            .expect("node on PATH");
        assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    }
}
