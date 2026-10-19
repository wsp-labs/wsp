// SPDX-License-Identifier: AGPL-3.0-only
//! The small pieces every tab draws with (`pull-request/parts.tsx`): a face wherever a person is named, several faces
//! stacked, the state word, the quiet note a list read in part carries, and each tab's shape while the page is read.

use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, Animation, AnimationExt as _, AnyElement, App, FontWeight, Hsla, Pixels, SharedString};

use super::model::{author_name, Tone};
use crate::ui::{tip, ActivePalette, Palette, Skeleton};

/// The ink each tone is drawn in, off the theme's own status tokens (`TONE_INK`).
pub fn tone_ink(tone: Tone, p: &Palette) -> Hsla {
    match tone {
        Tone::Ok => p.status_done,
        Tone::Bad => p.status_failed,
        Tone::Warn => p.warning_foreground,
        Tone::Run => p.status_working,
        Tone::Merged => p.pr_merged,
        Tone::Quiet => p.muted_foreground,
    }
}

/// The initial's size in each face's box.
fn initial_size(size: Pixels) -> Pixels {
    if size >= px(24.) {
        px(10.)
    } else if size >= px(20.) {
        px(9.)
    } else {
        px(8.)
    }
}

/// A face: the login's initial in a ring. No avatar is fetched. `line` is the row's line height the initial inherits,
/// else the page's 1.5: a line taller than the ring grows the grid's one row past it from the top, so the initial sits
/// low, as `place-items-center` leaves it.
pub fn face(login: &str, size: Pixels, stacked: bool, line: Option<Pixels>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let initial: String = login.chars().next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    let ring = if stacked { p.card } else { p.border };
    div()
        .debug_selector(|| "pr-face".into())
        .flex_none()
        .size(size)
        .rounded_full()
        .overflow_hidden()
        .when(stacked, |el| el.border(px(1.5)))
        .when(!stacked, |el| el.border_1())
        .border_color(ring)
        .bg(p.muted)
        .text_size(initial_size(size))
        .line_height(line.unwrap_or(initial_size(size) * 1.5))
        .font_weight(FontWeight::MEDIUM)
        .text_color(p.muted_foreground)
        .child(div().debug_selector(|| "pr-face-initial".into()).flex().items_center().justify_center().min_h_full().child(initial))
        .into_any_element()
}

/// Several people in one row: at most three faces overlapped, then +N, each login on its hover (`Faces`), their
/// initials in the row's `line`.
pub fn faces(id: &str, logins: &[String], line: Option<Pixels>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let shown = &logins[..logins.len().min(3)];
    let rest: Vec<String> = logins[shown.len()..].iter().map(|l| author_name(l)).collect();
    h_flex()
        .debug_selector(|| "pr-faces".into())
        .flex_none()
        .mr_2()
        .children(shown.iter().enumerate().map(|(ix, login)| {
            div()
                .id(SharedString::from(format!("{id}-face-{ix}")))
                .when(ix > 0, |el| el.ml(px(-6.)))
                .tooltip(tip(author_name(login)))
                .child(face(login, px(18.), true, line, cx))
        }))
        .when(!rest.is_empty(), |el| {
            el.child(
                div()
                    .id(SharedString::from(format!("{id}-more")))
                    .ml(px(-6.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .h(px(18.))
                    .min_w(px(18.))
                    .px(px(5.))
                    .rounded(px(9.))
                    .border(px(1.5))
                    .border_color(p.card)
                    .bg(p.muted)
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(10.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.muted_foreground)
                    .tooltip(tip(rest.join(", ")))
                    .child(format!("+{}", rest.len())),
            )
        })
        .into_any_element()
}

/// A login in a sentence, its face before it (`Who`).
pub fn who(login: &str, size: Pixels, line: Option<Pixels>, cx: &App) -> AnyElement {
    h_flex()
        .debug_selector(|| "pr-who".into())
        .flex_none()
        .gap(px(6.))
        .child(face(login, size, false, line, cx))
        .child(author_name(login))
        .into_any_element()
}

/// A state as the pane says it: the word in its tone's ink after a 7 px dot, hollow for a draft or an unread one,
/// pulsing while checks run (`StateWord`, the `pr-word-pulse` keyframes).
pub fn state_word(id: &str, word: &str, tone: Tone, hollow: bool, cx: &App) -> AnyElement {
    let p = cx.palette();
    let ink = tone_ink(tone, p);
    let dot =
        div().size(px(7.)).flex_none().rounded_full().map(|el| if hollow { el.border(px(1.5)).border_color(ink) } else { el.bg(ink) });
    let still = cx.reduce_motion() || cx.try_global::<crate::ui::Still>().is_some_and(|s| s.0);
    let dot = if tone == Tone::Run && !still {
        dot.with_animation(
            SharedString::from(format!("{id}-pulse")),
            Animation::new(Duration::from_millis(1600)).repeat().with_easing(gpui_kit::ease_in_out),
            |el, delta| el.opacity(1. - 0.65 * (1. - (2. * delta - 1.).abs())),
        )
        .into_any_element()
    } else {
        dot.into_any_element()
    };
    h_flex()
        .debug_selector(|| "pr-state-word".into())
        .flex_none()
        .gap(px(7.))
        .whitespace_nowrap()
        .text_size(px(13.))
        .line_height(px(20.))
        .font_weight(FontWeight::MEDIUM)
        .text_color(ink)
        .child(dot)
        .child(SharedString::from(word.to_owned()))
        .into_any_element()
}

/// A count as the pane reads it whole, its thousands apart: "1,234" (`DiffStatLabel`'s `whole`).
pub fn whole(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, d) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(d);
    }
    out
}

/// The line counts the pane reads rather than glances at: whole figures and a true minus, in the diff inks.
pub fn stat_whole(additions: u64, deletions: u64, size: Pixels, gap: Pixels, cx: &App) -> AnyElement {
    let p = cx.palette();
    h_flex()
        .debug_selector(|| "pr-stat".into())
        .flex_none()
        .gap(gap)
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(size)
        .child(div().text_color(p.success).child(format!("+{}", whole(additions))))
        .child(div().text_color(p.error_foreground).child(format!("\u{2212}{}", whole(deletions))))
        .into_any_element()
}

/// What the host read only in part, said once where that list stands (`CutNote`).
pub fn cut_note(words: &'static str, cx: &App) -> AnyElement {
    div()
        .debug_selector(|| "pr-cut".into())
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(cx.palette().muted_foreground)
        .child(words)
        .into_any_element()
}

/// The pane's shape before the status names a pull request: the head over the timeline.
pub fn head_skeleton(width: Pixels) -> AnyElement {
    v_flex()
        .debug_selector(|| "pr-skeleton-head".into())
        .gap(px(20.))
        .child(
            h_flex()
                .items_start()
                .gap_3()
                .child(div().size(px(32.)).rounded(px(9.)).overflow_hidden().child(Skeleton::new(px(32.), px(32.))))
                .child(
                    v_flex()
                        .pt_1()
                        .gap_2()
                        .child(Skeleton::new((width - px(44.)) * 0.8, px(14.)))
                        .child(Skeleton::new((width - px(44.)) / 3., px(12.))),
                ),
        )
        .child(Skeleton::new(width, px(36.)))
        .child(timeline_skeleton(width))
        .into_any_element()
}

/// The timeline's shape while the page is read (`TimelineSkeleton`).
pub fn timeline_skeleton(width: Pixels) -> AnyElement {
    let text = width - px(36.);
    v_flex()
        .debug_selector(|| "pr-skeleton-timeline".into())
        .relative()
        .gap(px(22.))
        .pt(px(2.))
        .children([0.72, 0.9, 0.56].into_iter().map(move |w| {
            h_flex()
                .items_start()
                .gap_3()
                .child(div().size(px(24.)).flex_none().rounded_full().overflow_hidden().child(Skeleton::new(px(24.), px(24.))))
                .child(
                    v_flex()
                        .pt(px(6.))
                        .gap_2()
                        .child(Skeleton::new(px(160.), px(12.)))
                        .child(Skeleton::new(text * w, px(12.)))
                        .child(Skeleton::new(text / 2., px(12.))),
                )
        }))
        .into_any_element()
}

/// The commits' shape while the page is read (`CommitsSkeleton`).
pub fn commits_skeleton(width: Pixels, cx: &App) -> AnyElement {
    let subject = (width - px(28. + 18. + 84. + 56. + 40. + 12.)).max(px(40.));
    let p = cx.palette();
    v_flex()
        .debug_selector(|| "pr-skeleton-commits".into())
        .relative()
        .gap(px(2.))
        .child(div().absolute().top(px(8.)).bottom(px(8.)).left(px(19.5)).w(px(1.)).bg(p.border))
        .children([0.8, 0.62, 0.74, 0.5, 0.68].into_iter().map(move |w| {
            h_flex()
                .min_h(px(36.))
                .px(px(6.))
                .gap(px(10.))
                .child(h_flex().w(px(28.)).h(px(36.)).flex_none().justify_center().child(div().size(px(6.)).rounded_full().bg(p.border)))
                .child(div().size(px(18.)).flex_none().rounded_full().overflow_hidden().child(Skeleton::new(px(18.), px(18.))))
                .child(div().flex_1().child(Skeleton::new(subject * w, px(12.))))
                .child(div().w(px(84.)).flex_none())
                .child(h_flex().w(px(56.)).flex_none().justify_end().child(Skeleton::new(px(48.), px(12.))))
        }))
        .into_any_element()
}

/// The files' shape while the page is read (`FilesSkeleton`).
pub fn files_skeleton(width: Pixels) -> AnyElement {
    v_flex()
        .debug_selector(|| "pr-skeleton-files".into())
        .gap_3()
        .child(Skeleton::new(px(144.), px(12.)))
        .child(v_flex().children([0.5, 0.64, 0.44, 0.58, 0.38, 0.52].into_iter().map(move |w| {
            h_flex()
                .h(px(32.))
                .px(px(6.))
                .justify_between()
                .gap_3()
                .child(Skeleton::new((width - px(80.)) * w, px(12.)))
                .child(Skeleton::new(px(56.), px(12.)))
        })))
        .into_any_element()
}

/// A file's diff while it is read: a number and a line on each row.
pub fn diff_skeleton(width: Pixels) -> AnyElement {
    let text = (width - px(64.)).max(px(40.));
    v_flex()
        .debug_selector(|| "pr-skeleton-diff".into())
        .gap_2()
        .px_3()
        .py_3()
        .children([0.62, 0.8, 0.44, 0.7, 0.36].into_iter().map(move |w| {
            h_flex()
                .gap_3()
                .child(h_flex().w(px(28.)).flex_none().justify_end().child(Skeleton::new(px(20.), px(10.))))
                .child(Skeleton::new(text * w, px(10.)))
        }))
        .into_any_element()
}

/// The words of a body a person or a bot wrote on GitHub, in the restricted reading the pane gives them: an image
/// stands as its alt words, never fetched, and raw HTML image tags are dropped, `<image>` with them, which an HTML
/// parser reads as `<img>`.
pub fn restricted(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("![") {
        out.push_str(&rest[..at]);
        let after = &rest[at + 2..];
        let Some(close) = after.find("](") else {
            out.push_str(&rest[at..]);
            rest = "";
            break;
        };
        let Some(end) = after[close + 2..].find(')') else {
            out.push_str(&rest[at..]);
            rest = "";
            break;
        };
        out.push_str(&after[..close]);
        rest = &after[close + 2 + end + 1..];
    }
    out.push_str(rest);
    let mut clean = String::with_capacity(out.len());
    let mut rest = out.as_str();
    while let Some(at) = ["<img", "<image"].iter().filter_map(|tag| rest.to_ascii_lowercase().find(tag)).min() {
        clean.push_str(&rest[..at]);
        match rest[at..].find('>') {
            Some(end) => rest = &rest[at + end + 1..],
            None => rest = "",
        }
    }
    clean.push_str(rest);
    clean
}

#[cfg(test)]
mod tests {
    use super::restricted;

    #[test]
    fn a_whole_count_keeps_its_thousands_apart() {
        assert_eq!(super::whole(0), "0");
        assert_eq!(super::whole(999), "999");
        assert_eq!(super::whole(1234), "1,234");
        assert_eq!(super::whole(1234567), "1,234,567");
    }

    #[test]
    fn an_image_stands_as_its_words_and_is_never_fetched() {
        assert_eq!(restricted("see ![the shot](https://x/y.png) here"), "see the shot here");
        assert_eq!(restricted("a <img src=\"https://x\"> b"), "a  b");
        assert_eq!(restricted("a <IMAGE src=\"data:image/png;base64,AA\"> b"), "a  b");
        assert_eq!(restricted("plain [link](https://x)"), "plain [link](https://x)");
    }
}
