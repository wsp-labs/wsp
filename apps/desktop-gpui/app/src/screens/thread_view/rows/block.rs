// SPDX-License-Identifier: AGPL-3.0-only
//! A shell or edit call drawn as its block (`CallRow`): the command and its output, an edit's hunks and their diff.

use std::time::Duration;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Hsla, SharedString, WeakEntity, Window};

use super::calls::{label_text, layered, shifted, Marks};
use super::opens::{named_file, turn_diff};
use super::{chevron, focusable, glyph_slot, mix, FOLD_PAST, FOLD_TO, OUTPUT_MAX, SM};
use crate::screens::thread_view::{markdown, shimmer};
use crate::thread_view::ThreadView;
use crate::transcript::{Call, FilePatch, Glyph as RowGlyph, Row};
use crate::ui::{Glyph, Palette};

/// `CODE_BLOCK_SURFACE`: light, a 1 px border at 70% on `--secondary`; dark, no border on `--input` at 32%.
pub fn code_surface(p: &Palette, cx: &App) -> (Hsla, Hsla) {
    if cx.theme().is_dark() {
        (gpui_kit::transparent_black(), p.input.opacity(0.32))
    } else {
        (p.border.opacity(0.7), p.muted)
    }
}

/// `commandText`: a command as its open heading draws it, less blank lines before it and space after it.
fn command_text(command: &str) -> &str {
    let lead = command.len() - command.trim_start().len();
    let start = command[..lead].rfind('\n').map_or(0, |at| at + 1);
    command[start..].trim_end()
}

/// `DiffStatLabel`, inline: the added count in the success ink and the removed in the error ink, mono at 12 px.
fn stat_label(added: usize, removed: usize, p: &Palette, cx: &App) -> AnyElement {
    h_flex()
        .gap(px(4.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(px(12.))
        .line_height(px(16.))
        .child(div().text_color(p.success).child(format!("+{added}")))
        .child(div().text_color(p.error_foreground).child(format!("-{removed}")))
        .into_any_element()
}

/// `FileFacts`: a file written whole counts its lines, any other its added and removed lines.
fn file_facts(file: &FilePatch, p: &Palette, cx: &App) -> AnyElement {
    if file.created {
        div().child(format!("{} lines", thousands(file.added))).into_any_element()
    } else {
        stat_label(file.added, file.removed, p, cx)
    }
}

/// `OpenDiff`: the ghost `icon-xs` FileDiff, opening the file in Changes on its turn's range, or in Files at its first
/// hunk where the call has no turn.
fn open_diff(file: &FilePatch, turn: Option<&str>, view: WeakEntity<ThreadView>, cx: &App) -> impl IntoElement {
    let (path, line, turn) = (file.path.clone(), file.line, turn.map(str::to_owned));
    markdown::ghost_button("open-diff", Glyph::FileDiff, false, cx)
        .debug_selector(|| "edit-open-diff".into())
        .tooltip(crate::ui::tip("Open diff"))
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            let Some(view) = view.upgrade() else { return };
            let line = Some(line);
            let asked = match &turn {
                Some(turn) => turn_diff(view.read(cx), turn, Some(&path), line, cx),
                None => named_file(view.read(cx), &path, line, cx),
            };
            asked.run(cx);
        })
}

/// A shell or edit call (`CallRow`): a 28 px line in a 10 px round block that takes the code block surface when open;
/// a running command turns its spinner and counts its time, and opens only to show a command of more than one line.
pub(super) fn block(
    view: &mut ThreadView,
    row: &Row,
    call: &Call,
    p: &Palette,
    window: &mut Window,
    cx: &mut Context<ThreadView>,
) -> AnyElement {
    let (edge, fill) = code_surface(p, cx);
    let key = call.key.clone();
    let mono = cx.theme().mono_font_family.clone();
    let running = call.command.is_some() && call.running;
    let command = call.command.as_deref().map(command_text);
    let can_open = !running || command.is_some_and(|c| c.contains('\n'));
    let open = call.open && can_open;
    let one = (call.files.len() == 1).then(|| &call.files[0]);
    let (g, verb, target) = match command {
        Some(command) => {
            let line = if open { command.to_owned() } else { command.lines().next().unwrap_or_default().to_owned() };
            (if call.failed && call.exit_code.is_none() { RowGlyph::CircleAlert } else { RowGlyph::Terminal }, None, line)
        }
        None => (call.glyph, call.verb.clone(), call.label.clone()),
    };
    let mut facts: Vec<AnyElement> = Vec::new();
    match (one, call.patch.is_some()) {
        (Some(file), _) => facts.push(file_facts(file, p, cx)),
        (None, true) => {
            let (added, removed) = call.files.iter().fold((0, 0), |(a, r), f| (a + f.added, r + f.removed));
            facts.push(stat_label(added, removed, p, cx));
        }
        _ => {}
    }
    if running {
        if let Some(since) = call.since {
            view.clock(row, since, cx);
            let look = shimmer::ClockLook { size: px(12.), line_height: None, height: px(24.) };
            facts.push(shimmer::clock_slot(view, row, look, cx));
        }
    } else {
        if let Some(code) = call.exit_code.filter(|c| *c != 0) {
            facts.push(div().text_color(p.error_foreground).child(format!("Exited {code}")).into_any_element());
        }
        if let Some(ms) = call.duration_ms {
            facts.push(div().child(command_duration(ms)).into_any_element());
        }
    }
    // The web draws a command in Shiki's bash colors, the same pierre theme as the code blocks.
    let colors = if command.is_some() { markdown::highlight_code("bash", &target, &cx.theme().highlight_theme) } else { Vec::new() };
    let (marks, after) = match command {
        Some(_) => (super::super::find::marks(view, &row.key, &format!("{}:command", call.key), &target, cx), 0),
        None => {
            let label = label_text(verb.as_deref(), &target);
            (
                super::super::find::marks(view, &row.key, &format!("{}:label", call.key), &label, cx),
                verb.as_ref().map_or(0, |v| v.len() + 1),
            )
        }
    };
    let verb_marks = shifted(&marks, 0, verb.as_ref().map_or(0, |v| v.len()));
    let painted = !marks.is_empty();
    let colors = layered(&colors, &shifted(&marks, after, target.len()));
    let heading = h_flex()
        .flex_1()
        .min_w_0()
        .gap(px(4.))
        .text_size(SM.0)
        .line_height(px(24.))
        .text_color(p.muted_foreground)
        .children(verb.map(|v| div().flex_none().child(gpui_kit::StyledText::new(v).with_highlights(verb_marks))))
        .child(
            div()
                .min_w_0()
                .flex_1()
                .when(command.is_some() || one.is_some(), |el| el.font_family(mono.clone()))
                .when(call.patch.is_some(), |el| el.text_color(p.foreground))
                .when(painted, |el| el.debug_selector(|| "find-marked".into()))
                .when(!open, |el| el.truncate())
                .child(gpui_kit::StyledText::new(target.clone()).with_highlights(colors)),
        );
    let actions = open.then(|| match (command, one) {
        (Some(command), _) => Some(copy_command(&call.key, command, window, cx).into_any_element()),
        (None, Some(file)) => Some(open_diff(file, row.turn.as_deref(), cx.weak_entity(), cx).into_any_element()),
        _ => None,
    });
    let glyph = if running {
        div().flex_none().size(px(24.)).flex().items_center().justify_center().child(crate::ui::status::spinner(cx)).into_any_element()
    } else {
        glyph_slot(g, p.muted_foreground).into_any_element()
    };
    let line = h_flex()
        .id(SharedString::from(call.key.clone()))
        .debug_selector(|| "call-line".into())
        .items_start()
        .min_h(px(28.))
        .p(px(2.))
        .gap(px(6.))
        .child(glyph)
        .child(heading)
        .child(h_flex().h(px(24.)).flex_none().gap(px(12.)).text_size(px(12.)).text_color(p.muted_foreground).children(facts))
        .children(actions.flatten().map(|a| div().flex_none().h(px(24.)).flex().items_center().child(a)))
        .child(
            div()
                .flex_none()
                .h(px(24.))
                .w(px(16.))
                .flex()
                .items_center()
                .justify_center()
                .when(!can_open, |el| el.invisible())
                .child(chevron(open, p.muted_foreground)),
        )
        .when(can_open, |el| {
            focusable(el.cursor_pointer().hover(|s| s.bg(p.accent.opacity(0.2))), p)
                .on_click(cx.listener(move |this, _, _, cx| this.toggle(&key, cx)))
        });
    let body = (open && !running).then(|| match (&call.patch, &call.body) {
        (Some(_), _) => edit_body(call, row.turn.as_deref(), cx.weak_entity(), p, window, cx),
        (None, Some(output)) if !output.trim().is_empty() => {
            let part = format!("{}:body", call.key);
            let mark = |shown: &str, cx: &App| super::super::find::marks(view, &row.key, &part, shown, cx);
            output_band(&call.key, output, call.exit_code.unwrap_or(0) != 0, call.bytes, &mark, p, window, cx)
        }
        _ => run_footer(div().debug_selector(|| "no-output".into()).child("No output"), None, p).into_any_element(),
    });
    v_flex()
        .rounded(px(10.))
        .border_1()
        .border_color(if open { edge } else { gpui_kit::transparent_black() })
        .when(open, |el| el.bg(fill))
        .overflow_hidden()
        .child(line)
        .children(body)
        .into_any_element()
}

/// Copy command: the whole command to the clipboard, and a check in place of the copy glyph for 1.5 s.
fn copy_command(key: &str, command: &str, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let copied = window.use_keyed_state(SharedString::from(format!("copied:{key}")), cx, |_, _| false);
    let done = *copied.read(cx);
    let command = command.to_owned();
    markdown::ghost_button("copy-command", if done { Glyph::Check } else { Glyph::Copy }, false, cx).on_click(move |_, _, cx| {
        cx.stop_propagation();
        cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(command.clone()));
        copied.update(cx, |copied, cx| {
            *copied = true;
            cx.notify();
            cx.spawn(async move |copied, cx| {
                cx.background_executor().timer(Duration::from_millis(1500)).await;
                _ = copied.update(cx, |copied, cx| {
                    *copied = false;
                    cx.notify();
                });
            })
            .detach();
        });
    })
}

/// An open edit's hunks (`EditCallRow`): one file's straight under the line; several, each under a head with its path,
/// its facts and Open diff, 8 px apart; then "The transcript kept part of this change" where the host cut the patch.
fn edit_body(call: &Call, turn: Option<&str>, view: WeakEntity<ThreadView>, p: &Palette, window: &mut Window, cx: &mut App) -> AnyElement {
    let many = call.files.len() > 1;
    let files = call.files.iter().enumerate().map(|(at, file)| {
        let head = many.then(|| {
            h_flex()
                .debug_selector(|| "file-head".into())
                .min_h(px(32.))
                .py(px(4.))
                .pr(px(6.))
                .pl(px(12.))
                .gap(px(12.))
                .text_size(px(12.))
                .line_height(px(16.))
                .text_color(p.muted_foreground)
                .child(
                    div()
                        .min_w_0()
                        .flex_1()
                        .truncate()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_color(p.foreground)
                        .child(file.path.clone()),
                )
                .child(file_facts(file, p, cx))
                .child(open_diff(file, turn, view.clone(), cx))
        });
        v_flex().id(SharedString::from(format!("file:{}:{at}", call.key))).children(head).child(diff(&call.key, at, file, p, window, cx))
    });
    v_flex()
        .child(v_flex().gap(px(8.)).children(files))
        .when(call.cut, |el| {
            el.child(run_footer(div().debug_selector(|| "edit-cut".into()).child("The transcript kept part of this change"), None, p))
        })
        .into_any_element()
}

/// `commandDuration` in `timeline/callFacts.ts`: at least 1 ms, tenths floored under 10 s, whole seconds floored.
pub fn command_duration(ms: f64) -> String {
    if ms < 1_000. {
        return format!("{}ms", ms.round().max(1.) as u64);
    }
    if ms < 10_000. {
        return format!("{:.1}s", (ms / 100.).floor() / 10.);
    }
    let s = (ms / 1_000.).floor() as u64;
    match s {
        s if s < 60 => format!("{s}s"),
        s if s < 3_600 => {
            if s % 60 == 0 {
                format!("{}m", s / 60)
            } else {
                format!("{}m {}s", s / 60, s % 60)
            }
        }
        s => {
            let m = (s % 3_600) / 60;
            if m == 0 {
                format!("{}h", s / 3_600)
            } else {
                format!("{}h {m}m", s / 3_600)
            }
        }
    }
}

/// A finished command's output (`OutputBand`): past 14 lines its first 10, or after a failure its last 10, until
/// "+N lines" shows it whole and "Fewer lines" folds it again; "First 12 KB of 40 KB" where the host kept part of it.
#[allow(clippy::too_many_arguments)]
fn output_band(
    key: &str,
    output: &str,
    failed: bool,
    bytes: Option<u64>,
    mark: &dyn Fn(&str, &App) -> Marks,
    p: &Palette,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let text = output.trim_end().trim_start_matches('\n');
    let lines: Vec<&str> = text.lines().collect();
    let hidden = if lines.len() > FOLD_PAST { lines.len() - FOLD_TO } else { 0 };
    let all = window.use_keyed_state(SharedString::from(format!("output-all:{key}")), cx, |_, _| false);
    let whole = *all.read(cx);
    let shown = match (hidden > 0 && !whole, failed) {
        (false, _) => text.to_owned(),
        (true, true) => lines[lines.len() - FOLD_TO..].join("\n"),
        (true, false) => lines[..FOLD_TO].join("\n"),
    };
    let more = (hidden > 0).then(|| {
        let n = thousands(hidden);
        let word = match (whole, failed) {
            (true, _) => "Fewer lines".to_owned(),
            (false, true) => format!("+{n} earlier lines"),
            (false, false) => format!("+{n} lines"),
        };
        div().debug_selector(|| "output-more".into()).child(crate::ui::Button::text(format!("output-more:{key}"), word).xs().on_click(
            move |_, _, cx| {
                cx.stop_propagation();
                all.update(cx, |all, cx| {
                    *all = !*all;
                    cx.notify();
                });
            },
        ))
    });
    v_flex()
        .child(
            div()
                .id(SharedString::from(format!("output:{key}")))
                .debug_selector(|| "output".into())
                .max_h(OUTPUT_MAX)
                .overflow_y_scroll()
                .bg(p.background)
                .px(px(12.))
                .py(px(8.))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(12.))
                .line_height(px(18.))
                .text_color(p.foreground)
                .child({
                    let marks = mark(&shown, cx);
                    gpui_kit::StyledText::new(shown).with_highlights(marks)
                }),
        )
        .child(run_footer(
            h_flex().gap(px(12.)).children(more).children(bytes.map(|total| {
                div().debug_selector(|| "output-kept".into()).child(format!("First {}", bytes_of(output.len() as u64, total)))
            })),
            Some(copy_output(key, output, window, cx)),
            p,
        ))
        .into_any_element()
}

/// `fmtBytesOf`: two sizes against each other, the first without the unit it shares with the total.
pub fn bytes_of(used: u64, total: u64) -> String {
    let (u, t) = (crate::ui::fmt_bytes(used), crate::ui::fmt_bytes(total));
    let unit = &t[t.rfind(' ').unwrap_or(0)..];
    format!("{} of {t}", u.strip_suffix(unit).unwrap_or(&u))
}

/// Copy output: the whole output to the clipboard, and a check in place of the copy glyph for 1.5 s.
fn copy_output(key: &str, output: &str, window: &mut Window, cx: &mut App) -> AnyElement {
    let copied = window.use_keyed_state(SharedString::from(format!("copied-output:{key}")), cx, |_, _| false);
    let done = *copied.read(cx);
    let output = output.to_owned();
    let button = crate::ui::Button::text(format!("copy-output:{key}"), "Copy output")
        .xs()
        .glyph(if done { Glyph::Check } else { Glyph::Copy })
        .on_click(move |_, _, cx| {
            cx.stop_propagation();
            cx.write_to_clipboard(gpui_kit::ClipboardItem::new_string(output.clone()));
            copied.update(cx, |copied, cx| {
                *copied = true;
                cx.notify();
                cx.spawn(async move |copied, cx| {
                    cx.background_executor().timer(Duration::from_millis(1500)).await;
                    _ = copied.update(cx, |copied, cx| {
                        *copied = false;
                        cx.notify();
                    });
                })
                .detach();
            });
        });
    div().debug_selector(move || if done { "output-copied" } else { "copy-output" }.into()).child(button).into_any_element()
}

pub(super) fn thousands(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// `RunFooter`: at least 36 px, 12 px words on the left and a ghost xs act on the right.
fn run_footer(left: impl IntoElement, act: Option<AnyElement>, p: &Palette) -> impl IntoElement {
    h_flex()
        .min_h(px(36.))
        .justify_between()
        .gap(px(12.))
        .py(px(4.))
        .pr(px(6.))
        .pl(px(12.))
        .text_size(px(12.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .child(h_flex().min_w_0().gap(px(12.)).child(left))
        .children(act)
}

/// A diff folds past this many lines to its first `DIFF_FOLD_TO`.
const DIFF_FOLD_PAST: usize = 20;
const DIFF_FOLD_TO: usize = 16;

/// One line of a unified patch as the diff draws it: its number on its own side, its mark, its text.
#[derive(Debug, PartialEq)]
pub(super) struct DiffLine<'a> {
    pub(super) number: usize,
    pub(super) mark: Option<char>,
    pub(super) text: &'a str,
}

/// A unified patch's lines numbered on their own side; hunk headers set the numbers and draw nothing.
pub(super) fn diff_lines(patch: &str) -> Vec<DiffLine<'_>> {
    let (mut old, mut new) = (1, 1);
    let mut lines = Vec::new();
    for line in patch.lines() {
        if line.starts_with("+++") || line.starts_with("---") || line.starts_with("diff ") || line.starts_with("index ") {
            continue;
        }
        if let Some(hunk) = line.strip_prefix("@@ ") {
            let start = |sign: char| {
                hunk.split_whitespace().find_map(|w| w.strip_prefix(sign)).and_then(|w| w.split(',').next()?.parse().ok()).unwrap_or(1)
            };
            (old, new) = (start('-'), start('+'));
            continue;
        }
        let (mark, text) = match line.chars().next() {
            Some(c @ ('+' | '-')) => (Some(c), &line[1..]),
            Some(' ') => (None, &line[1..]),
            _ => (None, line),
        };
        let number = if mark == Some('-') { old } else { new };
        match mark {
            Some('-') => old += 1,
            Some(_) => new += 1,
            None => {
                old += 1;
                new += 1;
            }
        }
        lines.push(DiffLine { number, mark, text });
    }
    lines
}

/// A file's patch in an open edit block (`@pierre/diffs`): a bar and tint per change, syntax colors, 13/20 px mono,
/// folded past 20 lines to its first 16 until "+N lines" shows it whole and "Fewer lines" folds it again (`HunksBand`).
fn diff(key: &str, at: usize, file: &FilePatch, p: &Palette, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let ground = mix(p.card, p.background, 0.9);
    let (add, del) = (mix(p.success, ground, 0.06), mix(p.error, ground, 0.06));
    let lines = diff_lines(&file.text);
    let hidden = if lines.len() > DIFF_FOLD_PAST { lines.len() - DIFF_FOLD_TO } else { 0 };
    let all = window.use_keyed_state(SharedString::from(format!("diff-all:{key}:{at}")), cx, |_, _| false);
    let whole = *all.read(cx);
    let shown = if whole { &lines[..] } else { &lines[..lines.len() - hidden] };
    let code: String = shown.iter().map(|l| l.text).collect::<Vec<_>>().join("\n");
    let path = file.path.as_str();
    let lang = path.rsplit('.').next().filter(|ext| *ext != path).unwrap_or("");
    let styles = if lang.is_empty() { Vec::new() } else { markdown::highlight_code(lang, &code, &cx.theme().highlight_theme) };
    let digits = shown.iter().map(|l| l.number).max().unwrap_or(1).to_string().len();
    let mut start = 0;
    let rows = shown.iter().map(|line| {
        let range = start..start + line.text.len();
        start = range.end + 1;
        let own: Vec<_> = styles
            .iter()
            .filter(|(r, _)| r.start < range.end && r.end > range.start)
            .map(|(r, style)| (r.start.max(range.start) - range.start..r.end.min(range.end) - range.start, *style))
            .collect();
        let text = SharedString::from(if line.text.is_empty() { " ".to_owned() } else { line.text.to_owned() });
        let (bar, fill, number_ink) = match line.mark {
            Some('+') => (Some(p.success), Some(add), p.success),
            Some(_) => (Some(p.error), Some(del), p.error),
            None => (None, None, p.muted_foreground),
        };
        // A removed line's bar is dashed, an added line's solid.
        let bar = match (line.mark, bar) {
            (Some('-'), Some(b)) => v_flex().gap(px(1.)).children((0..10).map(move |_| div().h(px(1.)).bg(b))).into_any_element(),
            (_, Some(b)) => div().size_full().bg(b).into_any_element(),
            _ => div().into_any_element(),
        };
        h_flex()
            .items_start()
            .child(div().w(px(4.)).h(px(20.)).flex_none().child(bar))
            .child(
                div()
                    .flex_none()
                    .w(px(8. * digits as f32 + 17.))
                    .pr(px(6.))
                    .text_right()
                    .text_color(number_ink)
                    .child(line.number.to_string()),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .pl(px(12.))
                    .when_some(fill, |el, f| el.bg(f))
                    .child(gpui_kit::StyledText::new(text).with_highlights(own)),
            )
    });
    let more = (hidden > 0).then(|| {
        let word = if whole { "Fewer lines".to_owned() } else { format!("+{} lines", thousands(hidden)) };
        let id = format!("diff-more:{key}:{at}");
        let act = crate::ui::Button::text(id, word).xs().on_click(move |_, _, cx| {
            cx.stop_propagation();
            all.update(cx, |all, cx| {
                *all = !*all;
                cx.notify();
            });
        });
        run_footer(div().debug_selector(|| "diff-more".into()).child(act), None, p)
    });
    v_flex()
        .debug_selector(|| "diff".into())
        .child(
            v_flex()
                .bg(ground)
                .pt(px(7.))
                .pb(px(8.))
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(markdown::diff_size(cx))
                .line_height(px(20.))
                .text_color(p.foreground)
                .children(rows),
        )
        .children(more)
}
