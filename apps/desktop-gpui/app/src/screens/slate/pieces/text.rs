// SPDX-License-Identifier: AGPL-3.0-only
//! The word pieces, after `apps/web/src/slate/pieces/`: `heading.tsx` (the quiet head at every level), `text.tsx`
//! (tone, size, mono, placeholder, and the status line beside a heading and a button), `markdown.tsx` (restricted,
//! images as links), `status.tsx` and `chip.tsx` (a word), `facts.tsx` (a wrapping line or a grid of lines) and
//! `empty.tsx` (one quiet sentence and its button).

use std::sync::Arc;

use gpui_kit::base::{TextView, TextViewStyle};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{
    div, prelude::*, px, rems, AnyElement, App, FontWeight, HighlightStyle, Hsla, ImageCacheError, ImageSource, Pixels, RenderImage,
    SharedString, Window,
};
use serde_json::Value;

use super::layout::{head, place_of, surface, tone_of, Place, INSET, ROW_FLOOR};
use super::look::{self, is_figure, is_sentence, Tone};
use super::riders::ridden_by;
use super::PieceCx;
use crate::screens::slate::model::Loud;
use crate::screens::slate::Slate;
use crate::ui::{text, ActivePalette};

/// `max-w-xl` on a quiet sentence.
const QUIET_MAX: f32 = 576.;

pub fn draw(p: &PieceCx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match p.piece.kind.as_str() {
        "heading" => Some(head(p.str("value").unwrap_or_default(), cx).into_any_element()),
        "text" => text(p, cx),
        "markdown" => markdown(p, cx),
        "status" => status(p, cx),
        "chip" => chip(p, cx),
        "facts" => facts(p, window, cx),
        "empty" => Some(empty(p, window, cx)),
        _ => None,
    }
}

/// A row with a heading and a button in it: a text beside them is a status line, small and muted.
fn in_header_row(slate: &Slate, id: &str) -> bool {
    let Some(doc) = slate.doc() else { return false };
    let Some(row) = doc.parent(id).filter(|r| r.kind == "row") else { return false };
    let kinds: Vec<&str> = row.children.iter().filter_map(|c| doc.piece(c)).map(|c| c.kind.as_str()).collect();
    kinds.contains(&"heading") && kinds.contains(&"button")
}

/// How a text draws: its size and leading, its ink, its weight and its face.
#[derive(Debug, PartialEq)]
pub struct TextLook {
    pub size: f32,
    pub line: f32,
    pub tone: Option<Tone>,
    pub muted: bool,
    pub medium: bool,
    pub mono: bool,
    pub quiet: bool,
}

/// `text.tsx`'s rule: small is the 12 px note, quiet and muted the 13 px quiet step, a tone 13 px medium, large 15 px
/// for the slate's one large text, mono 13 px, else 14 px.
pub fn text_look(p: &PieceCx) -> TextLook {
    let set = |name: &str| p.piece.prop(name).is_some();
    let status = !set("size") && !set("tone") && !set("emphasis") && in_header_row(p.slate, p.id());
    let emphasis = p.str("emphasis");
    let size = p.str("size");
    let small = status || size.as_deref() == Some("small");
    let quiet = !small && (emphasis.as_deref() == Some("quiet") || p.str("tone").as_deref() == Some("muted"));
    let large = !small && !quiet && size.as_deref() == Some("large") && p.slate.is_loud(Loud::Large, p.id());
    let mono = p.bool("mono") && !small && !is_sentence(p.prop("value"));
    let tone = tone_of(p.prop("tone"), p.slate, p.id());
    let toned = !small && !quiet && !matches!(tone, Tone::Default | Tone::Muted);
    let (size, line) = if small {
        (12., 16.)
    } else if quiet {
        (13., 18.85)
    } else if toned || mono {
        (13., 20.)
    } else if large {
        (15., 22.)
    } else {
        (14., 20.)
    };
    TextLook {
        size,
        line,
        tone: (!small && !quiet).then_some(tone),
        muted: small || quiet,
        medium: toned || emphasis.as_deref() == Some("strong"),
        mono,
        quiet,
    }
}

fn text(p: &PieceCx, cx: &App) -> Option<AnyElement> {
    // A text following a status beside a number rides that number's note.
    if ridden_by(p.slate, p.id()).is_some() {
        return None;
    }
    let palette = cx.palette();
    let value = p.str("value").filter(|v| !v.is_empty()).map(|v| pre_wrap(&v));
    let Some(value) = value else {
        return p.str("placeholder").map(|ph| super::quiet(ph, cx));
    };
    let look = text_look(p);
    let ink = match look.tone {
        Some(tone) => tone.ink(cx),
        None => palette.muted_foreground,
    };
    let el = div()
        .min_w_0()
        .text_size(px(look.size))
        .line_height(px(look.line))
        .text_color(ink)
        .when(look.quiet, |d| d.max_w(px(QUIET_MAX)))
        .when(look.medium, |d| d.font_weight(FontWeight::MEDIUM))
        .when(look.mono, |d| d.font_family(cx.theme().mono_font_family.clone()))
        .child(value);
    Some(el.into_any_element())
}

/// `whitespace-pre-wrap`: a block's one closing line break draws no line of its own, as CSS lays it out.
pub fn pre_wrap(text: &str) -> String {
    text.strip_suffix("\r\n").or_else(|| text.strip_suffix('\n')).unwrap_or(text).to_owned()
}

/// A restricted file's images as links to their addresses: `![alt](src)` reads `[alt](src)`.
pub fn images_as_links(src: &str) -> String {
    src.replace("![", "[")
}

/// `pr-md` over `chat-markdown`: the foreground, links in the info ink, 0.6 em between paragraphs, code spans on the
/// accent. Bold at 500, links with no underline, the code span's size and radius and bullets in the foreground have no
/// knob in this gpui-kit.
fn markdown_style(cx: &App) -> TextViewStyle {
    let p = cx.palette();
    TextViewStyle::from_theme(&gpui_kit::base::Theme::global(cx))
        .with_foreground(p.foreground)
        .with_muted_foreground(p.muted_foreground)
        .with_link(p.info_foreground)
        .with_border(p.border)
        .with_paragraph_gap(rems(0.525))
        .with_inline_code(HighlightStyle { background_color: Some(p.accent), ..HighlightStyle::default() })
}

fn markdown(p: &PieceCx, cx: &App) -> Option<AnyElement> {
    let value = p.str("value").filter(|v| !v.is_empty())?;
    let screen = p.screen.clone();
    // An `<img>` the file writes as HTML is never fetched nor decoded: its source stays empty, as a link's would.
    let nothing =
        |_: &gpui_kit::SharedUri| ImageSource::from(|_: &mut Window, _: &mut App| None::<Result<Arc<RenderImage>, ImageCacheError>>);
    let view = TextView::markdown(p.el("markdown"), images_as_links(&value))
        .style(markdown_style(cx))
        .selectable(true)
        .image_source(nothing)
        .on_link_click(move |href, _, _, cx| {
            let href = href.to_string();
            if href.starts_with("http://") || href.starts_with("https://") || href.starts_with("mailto:") {
                _ = screen.update(cx, |s, cx| s.consent.link(href, cx));
            }
        });
    Some(div().min_w_0().text_size(px(14.)).line_height(px(21.7)).child(view).into_any_element())
}

/// `capitalised`: the first letter upper case.
pub fn capitalised(word: &str) -> String {
    let mut c = word.chars();
    c.next().map(|f| f.to_uppercase().chain(c).collect()).unwrap_or_default()
}

/// A state is its word, capitalised, in its tone and weight 500 when toned, never a dot.
pub fn state_word(word: &str, tone: Tone, cx: &App) -> gpui_kit::Div {
    let toned = !matches!(tone, Tone::Default | Tone::Muted);
    div()
        .min_w_0()
        .text_size(px(13.))
        .line_height(px(20.))
        .text_color(if toned { tone.ink(cx) } else { cx.palette().muted_foreground })
        .when(toned, |d| d.font_weight(FontWeight::MEDIUM))
        .child(capitalised(word))
}

fn status(p: &PieceCx, cx: &App) -> Option<AnyElement> {
    // A status beside a number rides that number's note.
    if ridden_by(p.slate, p.id()).is_some() {
        return None;
    }
    let tone = tone_of(p.prop("tone"), p.slate, p.id());
    Some(state_word(&p.str("value").unwrap_or_default(), tone, cx).into_any_element())
}

fn chip(p: &PieceCx, cx: &App) -> Option<AnyElement> {
    let value = p.str("value").filter(|v| !v.is_empty())?;
    Some(div().min_w_0().text_size(px(13.)).line_height(px(20.)).text_color(cx.palette().foreground).child(value).into_any_element())
}

/// One label and value pair.
#[derive(Clone, Debug, PartialEq)]
pub struct Fact {
    pub label: String,
    pub value: String,
    pub tone: Option<Tone>,
    pub quiet: bool,
    pub strong: bool,
    pub mono: bool,
}

/// `factsOf`: the pairs of a facts list, a pair whose value is missing left out.
pub fn facts_of(list: &[Value], tone: impl Fn(&Value) -> Tone) -> Vec<Fact> {
    list.iter()
        .filter_map(|entry| {
            let o = entry.as_object()?;
            let value = look::str(o.get("value")).filter(|v| !v.is_empty())?;
            Some(Fact {
                label: look::str(o.get("label")).unwrap_or_default(),
                mono: o.get("mono").and_then(Value::as_bool) == Some(true) && !is_sentence(Some(&Value::String(value.clone()))),
                value,
                tone: o.get("tone").filter(|t| !t.is_null()).map(&tone),
                quiet: o.get("emphasis").and_then(Value::as_str) == Some("quiet"),
                strong: o.get("emphasis").and_then(Value::as_str) == Some("strong"),
            })
        })
        .collect()
}

/// A fact's value: a list of figures 12 px mono in the foreground, any other 13 px sans muted, one face for every value
/// so a list never alternates; a value that asks for mono keeps it, and a tone colours either.
fn fact_value(fact: &Fact, figures: bool, cx: &App) -> gpui_kit::Div {
    let palette = cx.palette();
    let mono = fact.mono || figures;
    let ink: Hsla = if fact.quiet {
        palette.muted_foreground
    } else if let Some(t) = fact.tone {
        t.ink(cx)
    } else if mono {
        palette.foreground
    } else {
        palette.muted_foreground
    };
    div()
        .min_w_0()
        .text_color(ink)
        .map(|d| {
            if mono {
                d.font_family(cx.theme().mono_font_family.clone()).text_size(px(12.)).line_height(px(16.))
            } else {
                d.text_size(px(13.)).line_height(px(19.5))
            }
        })
        .when(fact.strong, |d| d.font_weight(FontWeight::MEDIUM))
        .child(fact.value.clone())
}

/// Facts as a grid of settings lines: the label 14 px at the left, the value at the right, a hairline between, in a
/// card of their own on the page.
pub fn facts_grid(key: SharedString, list: &[Fact], place: Place, window: &Window, cx: &App) -> AnyElement {
    let palette = cx.palette();
    let sans = cx.theme().font_family.clone();
    // A label breaks between its words, never inside one, as CSS wraps it.
    let word = |label: &str| label.split_whitespace().map(|w| text::width(w, 14., sans.clone(), window)).fold(px(0.), Pixels::max).ceil();
    let figures = list.iter().all(|f| is_figure(Some(&Value::String(f.value.clone()))));
    let rule = palette.border.opacity(0.5);
    let rows = list.iter().enumerate().map(|(at, fact)| {
        h_flex()
            .min_h(ROW_FLOOR)
            .min_w_0()
            .items_center()
            .justify_between()
            .gap(px(16.))
            .py(px(12.))
            .when(place != Place::Inside, |d| d.px(INSET))
            .when(at > 0, |d| d.border_t_1().border_color(rule))
            .child(
                div()
                    .min_w(word(&fact.label))
                    .text_size(px(14.))
                    .line_height(px(20.))
                    .text_color(palette.foreground)
                    .child(fact.label.clone()),
            )
            .child(fact_value(fact, figures, cx).text_right())
    });
    let body = v_flex().min_w_0().debug_selector(|| "slate-facts".into()).children(rows);
    if place == Place::Page {
        surface(key, body)
    } else {
        body.into_any_element()
    }
}

/// Facts on one line: pairs 12 px apart that wrap, each label 12 px muted before its value.
pub fn facts_line(list: &[Fact], cx: &App) -> AnyElement {
    let figures = list.iter().all(|f| is_figure(Some(&Value::String(f.value.clone()))));
    let muted = cx.palette().muted_foreground;
    h_flex()
        .debug_selector(|| "slate-facts".into())
        .min_w_0()
        .flex_wrap()
        .items_end()
        .gap_x(px(12.))
        .gap_y(px(4.))
        .children(list.iter().map(|fact| {
            // GPUI lays no baselines: the 12 px label lifts to the 13 px sans value's, which sits 1.4 px higher.
            let lift = if fact.mono || figures { px(0.) } else { px(1.4) };
            h_flex()
                .flex_none()
                .items_end()
                .gap(px(6.))
                .child(div().pb(lift).text_size(px(12.)).line_height(px(16.)).text_color(muted).child(fact.label.clone()))
                .child(fact_value(fact, figures, cx).whitespace_nowrap())
        }))
        .into_any_element()
}

fn facts(p: &PieceCx, window: &Window, cx: &App) -> Option<AnyElement> {
    let resolved = p.prop("facts").and_then(Value::as_array).cloned().unwrap_or_default();
    let shown = p.slate.present(p.piece.prop("facts"), &resolved);
    let list = facts_of(&shown, |t| tone_of(Some(t), p.slate, p.id()));
    if list.is_empty() {
        return None;
    }
    if p.str("layout").as_deref() == Some("grid") {
        Some(facts_grid(p.el("facts"), &list, place_of(p.slate, p.id()), window, cx))
    } else {
        Some(facts_line(&list, cx))
    }
}

/// `empty.tsx`'s sentence: the title, a period where it has no stop of its own, then the body.
pub fn empty_sentence(title: &str, body: Option<&str>) -> String {
    match body.filter(|b| !b.is_empty()) {
        None => title.to_owned(),
        Some(body) => {
            let stop = if title.ends_with(['.', '!', '?']) { "" } else { "." };
            format!("{title}{stop} {body}")
        }
    }
}

fn empty(p: &PieceCx, window: &mut Window, cx: &mut App) -> AnyElement {
    let sentence = empty_sentence(&p.str("title").unwrap_or_default(), p.str("body").as_deref());
    let muted = cx.palette().muted_foreground;
    let acts = p.children(window, cx);
    h_flex()
        .debug_selector(|| "slate-empty-line".into())
        .min_w_0()
        .items_center()
        .justify_between()
        .gap(px(16.))
        .child(div().min_w_0().max_w(px(QUIET_MAX)).text_size(px(13.)).line_height(px(18.85)).text_color(muted).child(sentence))
        .when(!acts.is_empty(), |d| d.child(h_flex().flex_none().items_center().gap(px(8.)).children(acts)))
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use base64::Engine as _;
    use gpui_kit::{Image, ImageFormat, TestAppContext, WeakEntity};
    use serde_json::json;

    use super::*;
    use crate::screens::slate::pieces::layout::tests::slate;

    struct Drawn(Slate);

    impl Render for Drawn {
        fn render(&mut self, window: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div().size_full().children(super::super::draw(&self.0, "root", &WeakEntity::new_invalid(), window, cx))
        }
    }

    #[gpui_kit::test]
    fn a_markdown_image_tag_is_never_decoded(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        // One pixel here; the same few bytes on a bigger screen and more frames ask for gigabytes.
        let mut gif = b"GIF89a".to_vec();
        gif.extend([1, 0, 1, 0, 0x80, 0, 0, 0, 0, 0, 255, 255, 255, 0x2C, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 0x44, 0x01, 0, 0x3B]);
        let value = format!(
            "A picture <img src=\"data:image/gif;base64,{}\"> in a sentence.",
            base64::engine::general_purpose::STANDARD.encode(&gif)
        );
        let s = slate(json!({
            "root": {"type": "column", "children": ["md"]},
            "md": {"type": "markdown", "props": {"value": value}},
        }));
        let (_, cx) = cx.add_window_view(move |_, _| Drawn(s));
        cx.run_until_parked();
        cx.update(|window, cx| window.draw(cx).clear(cx));
        cx.run_until_parked();
        let image = Arc::new(Image::from_bytes(ImageFormat::Gif, gif));
        assert!(!cx.update(|_, cx| ImageSource::Image(image).is_asset_cached(cx)), "the picture an HTML tag names was decoded");
    }

    #[test]
    fn facts_with_no_value_are_left_out() {
        let list = facts_of(
            &[json!({"label": "Branch", "value": "main"}), json!({"label": "Gone", "value": null}), json!({"label": "Blank", "value": ""})],
            |_| Tone::Default,
        );
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].label, "Branch");
    }

    #[test]
    fn an_empty_line_is_one_sentence() {
        assert_eq!(empty_sentence("No pull request yet", Some("This branch has none.")), "No pull request yet. This branch has none.");
        assert_eq!(empty_sentence("Nothing?", Some("Ask.")), "Nothing? Ask.");
        assert_eq!(empty_sentence("Nothing", None), "Nothing");
        assert_eq!(pre_wrap("Clock 17\n"), "Clock 17", "a command's closing newline draws no empty line");
        assert_eq!(pre_wrap("a\n\n"), "a\n");
        assert_eq!(images_as_links("see ![plot](https://x/p.png)"), "see [plot](https://x/p.png)");
    }
}
