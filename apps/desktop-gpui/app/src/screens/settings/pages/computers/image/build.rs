// SPDX-License-Identifier: AGPL-3.0-only
//! The build's own view under the IMAGE row (`ImageBuild.tsx`): its stages, their lines, the sign-ins and a provider's key.

use std::rc::Rc;

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::prelude::*;
use gpui_kit::{div, px, AnyElement, App, Entity, ScrollHandle, SharedString, Window};
use serde_json::json;
use wsp_host_client::protocol::{
    InitCancelRequest, InitJob, InitJobRowsItem, InitKeysRequest, InitRetryRequest, InitSignInCodeRequest, PlaceView,
};

use super::{job_building, job_over, row_failed, row_over, words, Kept, OPEN, OVER, RETRYING, RUNNING, SLOT, UNRUN, WAITING};
use crate::screens::settings::model::{req, Reread};
use crate::screens::settings::page::Ctx;
use crate::screens::settings::pages::agents::keyed::{self, typed};
use crate::screens::settings::pages::computers::remove::call_whole;
use crate::screens::settings::pages::computers::words::{now_ms, provider_words};
use crate::ui::computer_glyph::stroked_svg;
use crate::ui::{catalog, scroll_view, spinner, text, ActivePalette, Button, Glyph, Refusal};

/// The machine's sign-ins and stages under the IMAGE row, after `ImageBuild.tsx`: the build's title and sentence,
/// its stages or, while the sign-in stage runs, the sign-ins alone, Cancel asking once, and Close once it ended. While a
/// press is out every link but Close is held.
pub(super) fn image_build(
    ctx: &Ctx,
    place: &PlaceView,
    job: &InitJob,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let phase = job.phase.to_string();
    let building = job_building(&phase);
    let signs: Vec<&InitJobRowsItem> = job.rows.iter().filter(|r| r.kind.to_string() == "sign-in").collect();
    let stage_state = sign_in_stage_state(&signs);
    let slide = building && !signs.is_empty() && (stage_state == OPEN || stage_state == RUNNING);
    let headline = if slide {
        words::SLIDE_HEADLINE
    } else if phase == "done" {
        words::DONE
    } else if phase == "cancelled" {
        words::STOPPED
    } else if job_over(&phase) {
        words::FAILED
    } else {
        words::BUILD_HEADLINE
    };
    let top = if building { Some((if slide { words::SLIDE_TOP } else { words::BUILD_TOP }).to_owned()) } else { job.error.clone() };
    let (asking, changing, busy, step_refusal) = {
        let k = kept.read(cx);
        (k.asking, k.changing, k.step_busy, k.step_refusal.clone())
    };
    let key_words = (job.key_refused == Some(true)).then(|| provider_words(&place.id)).flatten();
    let marked = signs.iter().any(|s| s.tool.as_deref().is_some_and(has_row_mark));
    let mut rows: Vec<AnyElement> = Vec::new();
    if slide {
        for s in &signs {
            let retry = row_failed(s).then(|| s.tool.clone().unwrap_or_else(|| s.id.clone()));
            rows.push(slide_row(ctx, s, marked, retry, kept, window, cx));
        }
    } else {
        let mut folded = false;
        for row in &job.rows {
            let kind = row.kind.to_string();
            if kind == "agent" {
                continue;
            }
            if kind == "sign-in" {
                if folded {
                    continue;
                }
                folded = true;
                let stage = Stage {
                    id: SIGN_IN_STAGE,
                    label: words::SIGNING_IN,
                    state: &stage_state,
                    login: None,
                    lines: &[],
                    since: None,
                    detail: None,
                };
                rows.push(stage_row(ctx, &stage, Some((&signs, marked)), building, kept, window, cx));
                continue;
            }
            let login = row.login.as_ref().map(|l| l.to_string());
            let stage = Stage {
                id: &row.id,
                label: &row.label,
                state: &row.state,
                login: login.as_deref(),
                lines: &row.lines,
                since: row.since,
                detail: row.detail.as_deref(),
            };
            rows.push(stage_row(ctx, &stage, None, building, kept, window, cx));
        }
    }
    let total = job.rows.iter().filter(|r| r.kind.to_string() == "stage").count();
    let done = job
        .rows
        .iter()
        .filter(|r| r.kind.to_string() == "stage" && row_over(r) && !row_failed(r) && !UNRUN.contains(&r.state.as_str()))
        .count();
    let fraction = if total > 0 { done as f32 / total as f32 } else { 0. };
    let card = v_flex()
        .w_full()
        .rounded(crate::ui::size::CARD_RADIUS)
        .border_1()
        .border_color(p.border.opacity(0.6))
        .bg(p.card.opacity(0.4))
        .when(!slide, |el| {
            el.child(
                div()
                    .h(px(2.))
                    .w_full()
                    .bg(p.muted_foreground.opacity(0.2))
                    .child(div().h_full().w(gpui_kit::relative(fraction)).bg(p.foreground.opacity(0.7))),
            )
        })
        .children(rows.into_iter().enumerate().map(|(ix, el)| div().when(ix > 0, |d| d.border_t_1().border_color(p.border)).child(el)));
    let mut links: Vec<AnyElement> = Vec::new();
    if !building {
        let kept = kept.clone();
        let id = job.id.clone();
        links.push(
            Button::ghost("build-close", words::CLOSE)
                .xs()
                .on_click(move |_, _, cx| kept.update(cx, |k, _| k.put_away = Some(id.clone())))
                .into_any_element(),
        );
    } else if asking {
        let (acts, kept2) = (ctx.acts.clone(), kept.clone());
        links.push(
            div()
                .debug_selector(|| "build-sure".into())
                .child(Button::destructive("build-sure", words::CANCEL_SURE).xs().held(busy).on_click(move |_, _, cx| {
                    kept2.update(cx, |k, _| k.asking = false);
                    attempt(&acts, &kept2, req::<InitCancelRequest>(json!({})), cx);
                }))
                .into_any_element(),
        );
        let kept3 = kept.clone();
        links.push(
            div()
                .debug_selector(|| "build-keep".into())
                .child(
                    Button::ghost("build-keep", words::CANCEL_KEEP)
                        .xs()
                        .held(busy)
                        .on_click(move |_, _, cx| kept3.update(cx, |k, _| k.asking = false)),
                )
                .into_any_element(),
        );
    } else {
        let kept2 = kept.clone();
        links.push(
            Button::ghost_danger("build-cancel", words::CANCEL)
                .xs()
                .held(!job.stoppable || busy)
                .on_click(move |_, _, cx| kept2.update(cx, |k, _| k.asking = true))
                .into_any_element(),
        );
    }
    let primary = (!building && !changing && key_words.is_some()).then(|| {
        let kept = kept.clone();
        Button::outline("build-primary", words::CHANGE_KEY)
            .xs()
            .held(busy)
            .on_click(move |_, _, cx| kept.update(cx, |k, _| k.changing = true))
    });
    let waiting = (building && !job.stoppable).then_some(words::CANNOT_STOP);
    let mut slot = Refusal::new();
    if let Some(said) = step_refusal {
        slot = slot.said(said);
    }
    if let Some(waiting) = waiting {
        slot = slot.quiet(waiting);
    }
    let mut step = v_flex()
        .gap_4()
        .child(
            v_flex()
                .gap(px(6.))
                .child(
                    div()
                        .text_size(text::SETTING_TITLE.0)
                        .line_height(text::SETTING_TITLE.1)
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .child(headline),
                )
                .children(
                    top.filter(|t| !t.is_empty())
                        .map(|t| div().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(t)),
                ),
        )
        .child(card);
    if changing {
        if let Some(words) = key_words {
            step = step.child(div().pt_5().child(provider_key(ctx, place, words, kept, window, cx)));
        }
    }
    if asking && building {
        step = step.child(div().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(words::CANCEL_WHY));
    }
    step.child(h_flex().gap_4().child(h_flex().flex_1().min_w_0().gap_4().children(links)).children(primary)).child(slot).into_any_element()
}

/// One press of the build sent to the host, its refusal said in the step's slot.
fn attempt<O>(acts: &crate::screens::settings::Acts, kept: &Entity<Kept>, op: O, cx: &mut App)
where
    O: wsp_host_client::Op + Send + 'static,
    O::Reply: Send + 'static,
{
    kept.update(cx, |k, _| {
        k.step_busy = true;
        k.step_refusal = None;
    });
    let (acts2, kept) = (acts.clone(), kept.clone());
    acts.call(op, cx, move |answer, cx| {
        kept.update(cx, |k, _| {
            k.step_busy = false;
            if let Err(said) = answer {
                k.step_refusal = Some(said);
            }
        });
        acts2.notify(cx);
    });
}

/// `signInStageState`: the folded sign-in stage reads as its sign-ins do.
fn sign_in_stage_state(signs: &[&InitJobRowsItem]) -> String {
    if signs.iter().any(|r| r.state == OPEN) {
        return OPEN.into();
    }
    if signs.iter().any(|r| r.state == RUNNING) {
        return RUNNING.into();
    }
    if !signs.iter().all(|r| row_over(r)) {
        return if signs.iter().all(|r| r.state == WAITING) { WAITING.into() } else { RUNNING.into() };
    }
    if let Some(out) = signs.iter().find(|r| row_failed(r)) {
        return out.state.clone();
    }
    if signs.iter().all(|r| r.state == "skipped") {
        return "skipped".into();
    }
    "done".into()
}

/// `StageGlyph`: a ring while it waits, a dot while it runs, a check once it ended well, a cross once it failed.
fn stage_glyph(state: &str, login: Option<&str>, cx: &App) -> AnyElement {
    let p = cx.palette();
    let ring = || div().size_2().rounded_full().border_1().border_color(p.muted_foreground.opacity(0.6)).into_any_element();
    let dot = || div().size_2().rounded_full().bg(p.foreground).into_any_element();
    let failed = state == "failed" || login == Some("not-signed-in");
    if UNRUN.contains(&state) {
        return ring();
    }
    if state == RETRYING {
        return dot();
    }
    if failed {
        return stroked_svg("0 0 16 16", r#"<path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/>"#, Some(1.75), p.error_foreground, px(14.));
    }
    if login.is_some() || OVER.contains(&state) {
        return stroked_svg("0 0 16 16", r#"<path d="M3.5 8.5 6.5 11.5 12.5 5"/>"#, Some(1.75), p.foreground, px(14.));
    }
    if state == RUNNING || state == SLOT || state == OPEN {
        return dot();
    }
    ring()
}

/// `RowState`: the spinner while it runs, a muted mono word otherwise, its whole word on the hover.
fn row_state(id: &str, state: &str, cx: &App) -> AnyElement {
    if state == RUNNING {
        return spinner(cx).into_any_element();
    }
    let p = cx.palette();
    div()
        .id(SharedString::from(format!("build-state-{id}")))
        .flex_none()
        .max_w(px(224.))
        .truncate()
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(text::XS.0)
        .line_height(text::XS.1)
        .text_color(if state == "failed" { p.error_foreground } else { p.muted_foreground })
        .tooltip(crate::ui::tip(state.to_owned()))
        .child(state.to_owned())
        .into_any_element()
}

/// The folded sign-in stage's row id.
const SIGN_IN_STAGE: &str = "stage/sign-ins";

/// `GOLDEN_STAGE_TIMED`: the stages the provider says nothing during, which count their own seconds.
fn timed(id: &str) -> bool {
    matches!(id, "stage/snapshotting" | "stage/promoting")
}

/// `STAGE_BLOCK_LINES`, and `STAGE_BLOCK_LINES_SHORT` under a 700 px high window.
fn block_lines(window_height: gpui_kit::Pixels) -> usize {
    if window_height < px(700.) {
        6
    } else {
        8
    }
}

/// `initElapsedLine`: whole seconds under a minute, then minutes and seconds.
pub fn elapsed_line(ms: f64) -> String {
    if ms < 60_000. {
        format!("{}s", (ms / 1_000.).floor().max(0.))
    } else {
        crate::screens::settings::words::fmt_duration(ms)
    }
}

/// `Elapsed`: a timed stage's seconds, its own entity reading the clock once a second and drawn cached in a box its
/// line's width, so only it draws again for the clock, in place; a line of another length lays the row out again.
pub struct Elapsed {
    since: f64,
    shown: String,
    ticks: Option<gpui_kit::Task<()>>,
    /// How often it drew, which a test reads.
    pub drawn: usize,
}

impl Elapsed {
    pub fn new(since: f64) -> Self {
        Elapsed { since, shown: elapsed_line(now_ms() as f64 - since), ticks: None, drawn: 0 }
    }

    /// The line it shows, which its row sizes its box by: its digits are the mono face's, all one width.
    pub fn line(&self) -> &str {
        &self.shown
    }

    fn tick(&mut self, cx: &mut gpui_kit::Context<Self>) {
        let now = cx.try_global::<crate::ui::status::FrozenNow>().map_or(now_ms() as f64, |frozen| frozen.0);
        let line = elapsed_line(now - self.since);
        if line == self.shown {
            return;
        }
        let relaid = line.chars().count() != self.shown.chars().count();
        self.shown = line;
        if relaid {
            cx.notify();
        } else {
            cx.notify_in_place();
        }
    }
}

impl Render for Elapsed {
    fn render(&mut self, _: &mut Window, cx: &mut gpui_kit::Context<Self>) -> impl IntoElement {
        crate::frames::count("Elapsed");
        self.drawn += 1;
        if self.ticks.is_none() {
            self.ticks = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(std::time::Duration::from_secs(1)).await;
                if this.update(cx, |this, cx| this.tick(cx)).is_err() {
                    return;
                }
            }));
        }
        let p = cx.palette().clone();
        div()
            .debug_selector(|| "elapsed".into())
            .flex_none()
            .whitespace_nowrap()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(text::XS.0)
            .line_height(text::XS.1)
            .text_color(p.muted_foreground)
            .child(self.shown.clone())
    }
}

/// A stage as its row draws it.
struct Stage<'a> {
    id: &'a str,
    label: &'a str,
    state: &'a str,
    login: Option<&'a str>,
    lines: &'a [String],
    since: Option<f64>,
    detail: Option<&'a str>,
}

/// `BuildRow`: a stage's glyph, its name, its seconds where it counts them, its state and, where it folds, the
/// chevron; under it the machine's lines, or the sign-ins for the sign-in stage. A running stage and the sign-ins
/// waited on stand open; a failed one, or one with a sign-in to retry, opens on its own and folds on a click; a done
/// one opens on a click.
fn stage_row(
    ctx: &Ctx,
    stage: &Stage,
    signs: Option<(&[&InitJobRowsItem], bool)>,
    building: bool,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let running = stage.state == RUNNING || stage.state == SLOT;
    let failed = stage.state == "failed";
    let live = running || stage.state == OPEN;
    let has = match signs {
        Some((signs, _)) => !signs.is_empty(),
        None => !stage.lines.is_empty(),
    };
    let attention = building && signs.is_some_and(|(signs, _)| signs.iter().any(|s| row_failed(s)));
    let can_open = has && !live;
    let opened = kept.read(cx).opened.get(stage.id).copied();
    let open = has && (live || opened.unwrap_or(failed || attention));
    let elapsed = stage.since.filter(|_| running && timed(stage.id)).map(|since| {
        let elapsed =
            window.use_keyed_state(SharedString::from(format!("build-elapsed-{}", stage.id)), cx, move |_, _| Elapsed::new(since));
        let width = text::width(elapsed.read(cx).line(), f32::from(text::XS.0), cx.theme().mono_font_family.clone(), window).ceil();
        elapsed.cached(gpui_kit::StyleRefinement::default().flex_none().w(width).h(text::XS.1))
    });
    let chevron = can_open.then(|| {
        let turn = crate::ui::motion::ease_f32(
            SharedString::from(format!("build-{}-chevron", stage.id)),
            "turn",
            if open { 0. } else { -90. },
            crate::ui::motion::TURN,
            window,
            cx,
        );
        Icon::new(Glyph::ChevronDown)
            .with_size(px(14.))
            .flex_none()
            .text_color(p.muted_foreground)
            .rotate(gpui_kit::radians(turn.to_radians()))
    });
    let (id, kept2) = (stage.id.to_owned(), kept.clone());
    let selector = format!("build-row-{}", stage.id);
    let row = h_flex()
        .id(SharedString::from(format!("build-row-{}", stage.id)))
        .debug_selector(move || selector)
        .h(px(48.))
        .gap_3()
        .pl_4()
        .pr(px(10.))
        .when(can_open, |el| {
            let hover = p.accent.opacity(0.3);
            el.cursor_pointer().hover(move |s| s.bg(hover)).on_click(move |_, _, cx| {
                kept2.update(cx, |k, cx| {
                    k.opened.insert(id.clone(), !open);
                    cx.notify();
                })
            })
        })
        .when_some(stage.detail.map(str::to_owned), |el, detail| el.tooltip(crate::ui::tip(detail)))
        .child(div().flex().flex_none().items_center().justify_center().size(px(18.)).child(stage_glyph(stage.state, stage.login, cx)))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(15.))
                .text_color(if stage.state == WAITING { p.muted_foreground } else { p.foreground })
                .child(stage.label.to_owned()),
        )
        .child(
            h_flex()
                .ml_auto()
                .min_w(px(96.))
                .flex_none()
                .justify_end()
                .gap_2()
                .children(elapsed)
                .child(row_state(stage.id, stage.state, cx))
                .children(chevron),
        );
    let under: Option<AnyElement> = match signs {
        _ if !open => None,
        Some((signs, marked)) => Some(
            v_flex()
                .border_t_1()
                .border_color(p.border)
                .children(signs.iter().enumerate().map(|(ix, s)| {
                    let retry = (building && row_failed(s)).then(|| s.tool.clone().unwrap_or_else(|| s.id.clone()));
                    div()
                        .when(ix > 0, |d| d.border_t_1().border_color(p.border))
                        .child(sign_in_row(ctx, s, marked, retry, kept, window, cx))
                }))
                .into_any_element(),
        ),
        None => Some(stage_lines(stage.id, stage.lines, running, failed, kept, window, cx)),
    };
    v_flex().child(row).children(under).into_any_element()
}

/// `StageLines`: the open stage's block, as tall as its lines up to eight (six under a 700 px high window), scrolling
/// inside itself and pinned to the newest line while the stage runs; the padding stands outside the scroller, so a
/// pinned block shows whole lines.
fn stage_lines(
    id: &str,
    lines: &[String],
    running: bool,
    failed: bool,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let shown = lines.len().clamp(1, block_lines(window.viewport_size().height));
    let scroll = kept.update(cx, |k, _| {
        let (handle, seen) = k.lines.entry(id.to_owned()).or_insert_with(|| (ScrollHandle::new(), 0));
        if running && *seen != lines.len() {
            handle.scroll_to_bottom();
        }
        *seen = lines.len();
        handle.clone()
    });
    let dark = cx.theme().is_dark();
    let last = lines.len().saturating_sub(1);
    let lines = div()
        .pl(px(46.))
        .pr(px(10.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(text::XS.0)
        .line_height(px(20.))
        .text_color(p.muted_foreground)
        .children(lines.iter().enumerate().map(|(ix, line)| {
            let line = terminal_line(line, failed && ix == last, dark, &p);
            if ix == last {
                div().debug_selector(|| "stage-line-newest".into()).child(line).into_any_element()
            } else {
                line
            }
        }));
    div()
        .debug_selector(|| "stage-lines".into())
        .border_t_1()
        .border_color(p.border)
        .bg(p.background.opacity(0.4))
        .pt(px(7.))
        .pb_2()
        .h(px(shown as f32 * 20. + 16.))
        .child(scroll_view(SharedString::from(format!("build-lines-{id}")), &scroll, lines, window, cx).h_full())
        .into_any_element()
}

/// `TerminalLine`: a tool's prefix at 60%, the machine's colours kept, a failed stage's last line in the danger ink.
fn terminal_line(line: &str, danger: bool, dark: bool, p: &crate::ui::Palette) -> AnyElement {
    use gpui_kit::{HighlightStyle, StyledText};
    let base = if danger { p.error_foreground } else { p.muted_foreground };
    let (prefix, rest) = super::super::ansi::tool_prefix(line).map_or(("", line), |(prefix, rest)| (prefix, rest));
    let mut text = prefix.to_owned();
    let mut highlights = Vec::new();
    if !prefix.is_empty() {
        highlights.push((0..prefix.len(), HighlightStyle { color: Some(base.opacity(0.6)), ..Default::default() }));
    }
    for span in super::super::ansi::spans(rest) {
        let start = text.len();
        text.push_str(&span.text);
        let ink = span.slot.map_or(base, |slot| super::super::ansi::slot_ink(slot, dark));
        let ink = if span.dim { ink.opacity(0.6) } else { ink };
        let weight = span.bold.then_some(gpui_kit::FontWeight::MEDIUM);
        if ink != base || weight.is_some() {
            highlights.push((start..text.len(), HighlightStyle { color: Some(ink), font_weight: weight, ..Default::default() }));
        }
    }
    let selector = if danger { "stage-line-danger" } else { "stage-line" };
    div()
        .debug_selector(move || selector.into())
        .text_color(base)
        .child(StyledText::new(text).with_highlights(highlights))
        .into_any_element()
}

/// `hasRowMark`: the tool has a mark to lead with, its agent's or the company's whose page it opens.
fn has_row_mark(tool: &str) -> bool {
    catalog::image(tool, false, gpui_kit::black()).is_some() || sign_in_mark(tool).is_some()
}

/// `SIGN_IN_MARKS` for the tools no agent's mark covers.
fn sign_in_mark(tool: &str) -> Option<&'static str> {
    const GITHUB: &str = include_str!("../../../../../../../../../packages/catalog/src/marks/github.ts");
    const GOOGLE: &str = include_str!("../../../../../../../../../apps/web/src/assets/signins/google.svg");
    match tool {
        "gh" => GITHUB.find("svg: `").map(|at| &GITHUB[at + 6..]).and_then(|rest| rest.find('`').map(|end| &rest[..end])),
        "gemini" | "gcloud" => Some(GOOGLE.trim()),
        _ => None,
    }
}

/// `MarkColumn`: the tool's mark in its 18 px, or the same 18 px of nothing where it has none.
fn mark_column(tool: Option<&str>, cx: &App) -> AnyElement {
    let ink = cx.palette().foreground;
    let mark = tool.and_then(|tool| match catalog::image(tool, cx.theme().is_dark(), ink) {
        Some(image) => Some(gpui_kit::img(image).size(px(18.)).flex_none().into_any_element()),
        None => sign_in_mark(tool).map(|svg| crate::ui::computer_glyph::mark_svg(svg, ink, px(18.))),
    });
    div()
        .debug_selector(|| "mark-column".into())
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(px(18.))
        .children(mark)
        .into_any_element()
}

/// The field a page that hands a code back takes, with Submit, Enter doing what Submit does.
fn code_line(ctx: &Ctx, row: &InitJobRowsItem, kept: &Entity<Kept>, window: &mut Window, cx: &mut App) -> gpui_kit::Div {
    let p = cx.palette().clone();
    let tool = row.tool.clone().unwrap_or_else(|| row.id.clone());
    let (acts, kept) = (ctx.acts.clone(), kept.clone());
    let send = Rc::new(move |field: &Entity<InputState>, window: &mut Window, cx: &mut App| {
        let code = field.read(cx).value().trim().to_owned();
        if !code.is_empty() {
            field.update(cx, |f, cx| f.set_value("", window, cx));
            attempt(&acts, &kept, req::<InitSignInCodeRequest>(json!({ "tool": tool, "code": code })), cx);
        }
    });
    let on_enter = send.clone();
    let input = typed(
        format!("build-code-{}", row.id),
        words::CODE_ASK,
        "",
        false,
        move |field, window, cx| on_enter(field, window, cx),
        window,
        cx,
    );
    let field = input.clone();
    let held = keyed::words(&input, cx).trim().is_empty();
    let mut submit = Button::outline(SharedString::from(format!("code-{}", row.id)), words::CODE_SUBMIT).xs().held(held);
    if !held {
        submit = submit.on_click(move |_, window, cx| send(&field, window, cx));
    }
    h_flex()
        .min_w_0()
        .flex_1()
        .gap_3()
        .child(
            div()
                .debug_selector(|| "build-code-field".into())
                .flex_1()
                .min_w(px(180.))
                .h(px(32.))
                .rounded(crate::ui::size::BUTTON_RADIUS)
                .border_1()
                .border_color(p.input)
                .bg(p.background)
                .px_2()
                .flex()
                .items_center()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(text::XS.0)
                .child(Input::new(&input).appearance(false).w_full()),
        )
        .child(div().debug_selector(move || if held { "build-code-submit-held" } else { "build-code-submit" }.into()).child(submit))
}

/// The keycap that opens a sign-in's page.
fn open_button(row: &InitJobRowsItem, page: String) -> Button {
    Button::outline(SharedString::from(format!("open-{}", row.id)), format!("{} \u{2197}", words::OPEN))
        .xs()
        .on_click(move |_, _, cx| crate::ui::open_link(&page, cx))
}

fn retry_button(ctx: &Ctx, row: &InitJobRowsItem, tool: String, kept: &Entity<Kept>) -> Button {
    let (acts, kept) = (ctx.acts.clone(), kept.clone());
    Button::outline(SharedString::from(format!("retry-{}", row.id)), words::RETRY)
        .xs()
        .on_click(move |_, _, cx| attempt(&acts, &kept, req::<InitRetryRequest>(json!({ "tool": tool })), cx))
}

/// `SignInRow`: one sign-in under its stage, a step in: the mark's column, the name, the code its page asks for, and
/// in the slot the keycap that opens the page or retries, then the state word; the code field on a line of its own.
fn sign_in_row(
    ctx: &Ctx,
    row: &InitJobRowsItem,
    marked: bool,
    retry: Option<String>,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let waiting = row.state == OPEN;
    let takes_code = row.finish.as_ref().is_some_and(|f| f.to_string() == "code");
    let head = h_flex()
        .debug_selector(|| "sign-in-row".into())
        .h(px(48.))
        .gap_3()
        .pl_10()
        .pr(px(10.))
        .when(marked, |el| el.child(mark_column(row.tool.as_deref(), cx)))
        .child({
            let selector = format!("sign-in-name-{}", row.id);
            div()
                .debug_selector(move || selector)
                .flex_1()
                .min_w_0()
                .truncate()
                .text_size(px(15.))
                .text_color(p.foreground)
                .child(row.label.clone())
        })
        .children(row.code.clone().filter(|_| waiting).map(|code| {
            div().flex_none().font_family(cx.theme().mono_font_family.clone()).text_size(text::XS.0).text_color(p.foreground).child(code)
        }))
        .child(
            h_flex()
                .ml_auto()
                .min_w(px(96.))
                .flex_none()
                .justify_end()
                .gap_2()
                .children(row.page.clone().filter(|_| waiting).map(|page| open_button(row, page)))
                .children(retry.map(|tool| retry_button(ctx, row, tool, kept)))
                .child(row_state(&row.id, &row.state, cx)),
        );
    let code = (waiting && takes_code)
        .then(|| code_line(ctx, row, kept, window, cx).h(px(48.)).pl_10().pr(px(10.)).border_t_1().border_color(p.border));
    v_flex().child(head).children(code).into_any_element()
}

/// `SignInSlideRow`: a sign-in's mark, name and state, and under it, where the person has something to do, the code,
/// the keycap that opens its page or retries, and the field a page that hands a code back takes.
fn slide_row(
    ctx: &Ctx,
    row: &InitJobRowsItem,
    marked: bool,
    retry: Option<String>,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let waiting = row.state == OPEN;
    let takes_code = row.finish.as_ref().is_some_and(|f| f.to_string() == "code");
    let acts_line = (waiting && (row.code.is_some() || row.page.is_some() || takes_code)) || retry.is_some();
    let head = h_flex()
        .h(px(48.))
        .gap_3()
        .pl_4()
        .pr(px(10.))
        .when(marked, |el| el.child(mark_column(row.tool.as_deref(), cx)))
        .child(div().flex_1().min_w_0().truncate().text_size(px(15.)).text_color(p.foreground).child(row.label.clone()))
        .child(h_flex().ml_auto().min_w(px(96.)).flex_none().justify_end().gap_2().child(row_state(&row.id, &row.state, cx)));
    let mut line =
        h_flex()
            .min_h(px(40.))
            .flex_wrap()
            .gap_x_3()
            .gap_y_2()
            .pb_2()
            .pr(px(10.))
            .map(|el| if marked { el.pl(px(46.)) } else { el.pl_4() });
    if waiting {
        if let Some(code) = &row.code {
            line = line.child(
                div()
                    .flex_none()
                    .font_family(cx.theme().mono_font_family.clone())
                    .text_size(px(20.))
                    .line_height(px(28.))
                    .text_color(p.foreground)
                    .child(code.clone()),
            );
        }
        if let Some(page) = row.page.clone() {
            line = line.child(open_button(row, page));
        }
    }
    if let Some(tool) = retry {
        line = line.child(retry_button(ctx, row, tool, kept));
    }
    if waiting && takes_code {
        line = line.child(code_line(ctx, row, kept, window, cx));
    } else if waiting {
        let said = if row.code.is_some() || takes_code { "waiting for the code" } else { "the page is open on this computer" };
        line = line.child(
            div()
                .min_w_0()
                .truncate()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(text::XS.0)
                .text_color(p.muted_foreground)
                .child(said),
        );
    }
    v_flex().child(head).when(acts_line, |el| el.child(line)).into_any_element()
}

/// `ProviderKey` where a build found the saved key refused: the field, Replace or Enter, and the provider's refusal.
/// The field is emptied only once the key is saved, so a refused key stays to be corrected.
pub(super) fn provider_key(
    ctx: &Ctx,
    place: &PlaceView,
    words: &'static super::super::words::ProviderWords,
    kept: &Entity<Kept>,
    window: &mut Window,
    cx: &mut App,
) -> AnyElement {
    let p = cx.palette().clone();
    let save: SaveKey = {
        let (acts, kept, id) = (ctx.acts.clone(), kept.clone(), place.id.clone());
        Rc::new(move |field, cx| {
            let value = field.read(cx).value().trim().to_owned();
            if value.is_empty() || kept.read(cx).step_busy {
                return;
            }
            kept.update(cx, |k, _| {
                k.step_busy = true;
                k.step_refusal = None;
            });
            let (acts2, kept3, id2) = (acts.clone(), kept.clone(), id.clone());
            call_whole(&acts, req::<InitKeysRequest>(json!({ "provider": id, "key": value })), cx, move |answer, cx| {
                kept3.update(cx, |k, _| {
                    k.step_busy = false;
                    match answer {
                        Ok(setup) if setup.setup.keys.get(&id2) == Some(&true) => {
                            k.changing = false;
                            k.clear_key = true;
                            k.put_away = setup.setup.job.as_ref().map(|j| j.id.clone()).or(k.put_away.take());
                        }
                        Ok(_) => {
                            let words = provider_words(&id2);
                            let name = words.map(|w| w.name).unwrap_or_default();
                            let fix = words
                                .and_then(|w| w.key_console)
                                .map(|c| format!("Check it at {c} and paste it again."))
                                .unwrap_or_else(|| "Check it and paste it again.".into());
                            k.step_refusal = Some(format!("{name} refused that key. {fix}"));
                        }
                        Err(failure) => k.step_refusal = Some(failure.message),
                    }
                });
                acts2.reread(Reread::Places, cx);
            });
        })
    };
    let on_enter = save.clone();
    let key = typed(format!("computer-{}-key", place.id), KEY_ASK, "", true, move |field, _, cx| on_enter(field, cx), window, cx);
    if kept.update(cx, |k, _| std::mem::take(&mut k.clear_key)) {
        keyed::clear(&key, window, cx);
    }
    let typed_key = key.read(cx).value().trim().to_owned();
    let busy = kept.read(cx).step_busy;
    let field = key.clone();
    let save = Button::outline("cloud-save", if busy { "Checking" } else { "Replace" })
        .held(busy || typed_key.is_empty())
        .on_click(move |_, _, cx| save(&field, cx));
    let console = words.key_console.map(|c| {
        let url = format!("https://{c}");
        h_flex()
            .id("get-key")
            .ml_auto()
            .gap_1()
            .text_size(px(12.))
            .text_color(p.muted_foreground)
            .cursor_pointer()
            .hover(|s| s.text_color(p.foreground))
            .on_click(move |_, _, cx| cx.open_url(&url))
            .child("Get a key")
            .child(Icon::new(Glyph::ExternalLink).with_size(px(12.)))
    });
    v_flex()
        .gap_3()
        .child(
            h_flex()
                .gap(px(10.))
                .child(Icon::new(Glyph::Cloud).with_size(px(16.)).text_color(p.muted_foreground))
                .child(div().text_size(px(14.)).child(words.name))
                .child(
                    h_flex()
                        .gap_1()
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(11.))
                        .text_color(p.foreground.opacity(0.7))
                        .child(Icon::new(Glyph::Check).with_size(px(12.)))
                        .child("key saved"),
                )
                .children(console),
        )
        .child(
            h_flex()
                .gap_2()
                .child(
                    h_flex()
                        .debug_selector(|| "cloud-key".into())
                        .flex_1()
                        .min_w_0()
                        .h(px(40.))
                        .gap_2()
                        .px_3()
                        .rounded(crate::ui::size::BUTTON_RADIUS)
                        .border_1()
                        .border_color(p.input)
                        .bg(p.input_fill)
                        .font_family(cx.theme().mono_font_family.clone())
                        .text_size(px(13.))
                        .child(Icon::new(Glyph::KeyRound).with_size(px(16.)).text_color(p.muted_foreground))
                        .child(div().flex_1().min_w_0().child(Input::new(&key).appearance(false).w_full())),
                )
                .child(save),
        )
        .into_any_element()
}

/// The key field's save, shared by Replace and Enter.
type SaveKey = Rc<dyn Fn(&Entity<InputState>, &mut App)>;

/// What the key field says while empty.
const KEY_ASK: &str = "Paste a new key to replace it";
