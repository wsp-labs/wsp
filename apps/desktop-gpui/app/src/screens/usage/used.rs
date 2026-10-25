// SPDX-License-Identifier: AGPL-3.0-only
//! The Usage tab: the range and the split, the totals strip, the chart, the split table and where the tokens went.
//! The last range's figures stand while the next range is read.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, Context, FontWeight, Hsla, SharedString, Window};
use wsp_host_client::protocol::UsedAnswer;

use super::model::{self, Chart, InkTheme, MixPart, Range, Split};
use super::parts::{self, identity, quiet, rule, share_bar, sized, split_glyph, surface, Looks, BLOCK_GAP};
use super::{words, UsagePage};
use crate::ui::{chart, ActivePalette, Card, DigitRoll, Segmented, Skeleton, CHART_HEIGHT};

/// The split table's columns past the first: tokens, cache hit, turns and the estimate.
const COLUMNS: [f32; 4] = [104., 96., 80., 120.];
const COLUMN_GAP: f32 = 24.;
/// Under 640 px: tokens and the estimate, `max-sm:grid-cols-[minmax(0,1fr)_72px_88px] max-sm:gap-x-4`.
const COLUMNS_NARROW: [f32; 2] = [72., 88.];
const COLUMN_GAP_NARROW: f32 = 16.;

pub fn render(page: &UsagePage, _now: f64, window: &mut Window, cx: &mut Context<UsagePage>) -> AnyElement {
    let narrow = crate::ui::narrow::width(window).sm;
    let controls = controls(page, narrow, cx);
    let mut section = v_flex().debug_selector(|| "usage-used".into()).gap(BLOCK_GAP).child(controls);
    section = match (&page.used.value, &page.used.refused) {
        (None, Some(refused)) => section.child(quiet(words::used_refused(refused), cx).py(px(20.))),
        (None, None) => section.child(skeleton(page.range, narrow, cx)),
        (Some(used), _) if used.rows.is_empty() => {
            section.child(quiet(words::NO_USE, cx).py(px(40.)).w_full().flex().justify_center().debug_selector(|| "no-use".into()))
        }
        (Some(used), _) => {
            let models = if used.split == wsp_host_client::protocol::UsedAnswerSplit::Agent && page.models.refused.is_none() {
                page.models.value.as_ref().filter(|m| m.range == used.range).map_or(&[][..], |m| &m.rows[..])
            } else {
                &[][..]
            };
            let looks = Looks { projects: &page.projects, places: &page.places, preferences: page.preferences.as_ref() };
            let chart = Chart::of(used, &page.projects, |key| Some(looks.project(key).1));
            let p = cx.palette();
            let ink = InkTheme {
                foreground: p.foreground,
                background: p.background,
                muted_foreground: p.muted_foreground,
                dark: cx.theme().is_dark(),
                hues: [p.hue_red, p.hue_orange, p.hue_amber, p.hue_green, p.hue_teal, p.hue_blue, p.hue_violet, p.hue_pink],
            };
            section
                .child(totals(used, narrow, cx))
                .child(chart_card(used, &chart, &ink, window, cx))
                .child(table(used, models, &chart, &ink, looks, narrow, cx))
                .children(mix(used, cx))
                .children(used.logs.as_ref().map(|logs| quiet(words::logs_line(&logs.agents, &logs.computers), cx)))
        }
    };
    section.into_any_element()
}

/// The range and the split; a segment's sides are `px-3.5`, `max-sm:px-2`.
fn controls(page: &UsagePage, narrow: bool, cx: &mut Context<UsagePage>) -> impl IntoElement {
    let (ranges, splits) = (Range::ALL, Split::ALL);
    let entity = cx.entity();
    let pad = if narrow { px(8.) } else { px(14.) };
    let range = ranges.iter().fold(
        Segmented::new("usage-range", ranges.iter().position(|r| *r == page.range).unwrap_or(1))
            .height(px(36.))
            .segment_size((px(14.), px(20.)), pad),
        |s, r| s.segment(r.word()),
    );
    let split = splits.iter().fold(
        Segmented::new("usage-split", splits.iter().position(|s| *s == page.split).unwrap_or(0))
            .height(px(36.))
            .segment_size((px(14.), px(20.)), pad),
        |s, split| s.segment(split.word()),
    );
    let on_split = entity.clone();
    h_flex()
        .flex_wrap()
        .justify_between()
        .gap_x_4()
        .gap_y_2()
        .child(
            div()
                .debug_selector(|| "usage-range".into())
                .child(range.on_pick(move |ix, _, cx| entity.update(cx, |page, cx| page.pick_range(ranges[ix], cx)))),
        )
        .child(
            div()
                .debug_selector(|| "usage-split".into())
                .child(split.on_pick(move |ix, _, cx| on_split.update(cx, |page, cx| page.pick_split(splits[ix], cx)))),
        )
}

fn mono(cx: &App) -> SharedString {
    cx.theme().mono_font_family.clone()
}

/// Cells on a strip's grid: one row with a rule between each, or two columns with none under 640 px
/// (`max-sm:grid-cols-2 max-sm:divide-x-0`), a short last row keeping its columns' widths.
fn strip(cells: Vec<gpui_kit::Div>, narrow: bool, cx: &App) -> gpui_kit::Div {
    let count = cells.len();
    let per = if narrow { 2 } else { count.max(1) };
    let mut cells = cells.into_iter().enumerate();
    let mut grid = surface(cx);
    loop {
        let line: Vec<AnyElement> = cells
            .by_ref()
            .take(per)
            .map(|(ix, cell)| cell.when(!narrow && ix + 1 < count, |el| el.border_r_1().border_color(rule(cx))).into_any_element())
            .collect();
        if line.is_empty() {
            break;
        }
        let blanks = per - line.len();
        grid = grid.child(h_flex().items_stretch().children(line).children((0..blanks).map(|_| div().flex_1().min_w_0())));
    }
    grid
}

fn totals(used: &UsedAnswer, narrow: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let stats = model::Totals::of(used).stats();
    let cells = stats.into_iter().map(|stat| {
        v_flex()
            .debug_selector(move || format!("stat-{}", stat.key))
            .flex_1()
            .min_w_0()
            .gap(px(6.))
            .p(px(20.))
            .child(sized(div(), 13.).text_color(p.muted_foreground).child(stat.label))
            .child(
                div()
                    .debug_selector(move || format!("{}-figure", stat.key))
                    .whitespace_nowrap()
                    .font_family(mono(cx))
                    .text_size(px(26.))
                    .line_height(px(32.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(DigitRoll::new(SharedString::from(format!("stat-{}-figure", stat.key)), stat.value, px(32.)).roll_in()),
            )
            .children(stat.note.map(|note| div().truncate().text_xs().line_height(px(16.)).text_color(p.muted_foreground).child(note)))
    });
    strip(cells.collect(), narrow, cx).debug_selector(|| "usage-totals".into())
}

/// `LEGEND`: each line's 14 by 2 swatch before its name, an agent's 12 px mark between them.
fn legend(items: Vec<(Hsla, Option<String>, String)>, window: &Window, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let family = cx.theme().font_family.clone();
    h_flex().debug_selector(|| "usage-legend".into()).flex_wrap().gap_x_6().gap_y_2().children(items.into_iter().map(
        |(ink, agent, label)| {
            sized(h_flex().gap_2(), 13.5)
                .text_color(p.muted_foreground)
                .child(div().w(px(14.)).h(px(2.)).rounded_full().bg(ink))
                .children(agent.and_then(|agent| parts::agent_glyph(&agent, px(12.), p.muted_foreground, cx)))
                .child(div().flex_none().whitespace_nowrap().w(crate::ui::text::width(&label, 13.5, family.clone(), window)).child(label))
        },
    ))
}

fn chart_card(used: &UsedAnswer, chart: &Chart, ink: &InkTheme, window: &Window, cx: &App) -> impl IntoElement {
    let range = Range::from(used.range);
    let items = if chart.per_split {
        chart.lines.iter().map(|line| (line.ink.resolve(ink), line.agent.clone(), line.label.clone())).collect()
    } else {
        vec![(ink.foreground, None, words::all_of(Split::from(used.split)).to_string())]
    };
    let lines = chart
        .lines
        .iter()
        .map(|line| chart::ChartLine {
            key: line.key.clone().into(),
            label: line.label.clone().into(),
            points: line.points.clone(),
            ink: line.ink.resolve(ink),
        })
        .collect();
    let tz = chrono::Local;
    let ticks = model::ticks(used, &tz).into_iter().map(|t| chart::Tick { at: t.at as f32, word: t.word.into() }).collect();
    let steps = used.series.iter().map(|s| s.t).collect();
    let body = v_flex().debug_selector(|| "usage-chart".into()).gap_4().child(legend(items, window, cx)).child(
        chart::Chart::new("usage-chart-plot", steps, lines, ticks)
            .tween()
            .roll()
            .figure(words::fmt_tokens)
            .step_word(move |t| words::step_word(range, t, &chrono::Local)),
    );
    Card::new("usage-chart").locked(true).head(words::chart_head(range)).body(body)
}

/// The split table; under 640 px its cache hit and turns go (`max-sm:hidden`).
fn table(
    used: &UsedAnswer,
    models: &[model::Row],
    chart: &Chart,
    ink: &InkTheme,
    looks: Looks,
    narrow: bool,
    cx: &App,
) -> impl IntoElement {
    let p = cx.palette();
    let split = Split::from(used.split);
    let t = model::split_table(used, models);
    let line = rule(cx);
    let head_cell =
        |text: &'static str, k: &'static str| div().debug_selector(move || format!("used-head-{k}")).child(text).into_any_element();
    let mut heads = vec![head_cell(words::TOKENS, "tokens")];
    if !narrow {
        heads.push(head_cell(words::CACHE_HIT, "cache-hit"));
        if t.turns {
            heads.push(head_cell(words::TURNS, "turns"));
        }
    }
    heads.push(head_cell(words::ESTIMATE, "estimate"));
    let mut widths = COLUMNS.to_vec();
    if !t.turns {
        widths.remove(2);
    }
    if narrow {
        widths = COLUMNS_NARROW.to_vec();
    }
    let gap = if narrow { COLUMN_GAP_NARROW } else { COLUMN_GAP };
    let row = move |first: AnyElement, cells: Vec<AnyElement>| {
        h_flex()
            .gap(px(gap))
            .child(div().flex_1().min_w_0().child(first))
            .children(cells.into_iter().zip(widths.clone()).map(|(cell, w)| div().flex_none().w(px(w)).flex().justify_end().child(cell)))
    };
    let number = |text: String, color: Hsla| {
        div().font_family(mono(cx)).text_sm().line_height(px(20.)).text_color(color).child(text).into_any_element()
    };
    let cells_of = |r: &model::Row| {
        let c = model::row_cells(r);
        let mut cells = vec![number(c.tokens, p.foreground)];
        if !narrow {
            cells.push(number(c.cache_hit, p.muted_foreground));
            if t.turns {
                cells.push(number(c.turns, p.muted_foreground));
            }
        }
        cells.push(number(c.price, if c.estimated { p.foreground } else { p.muted_foreground }));
        cells
    };
    let head = row(div().child(t.head).into_any_element(), heads)
        .debug_selector(|| "used-head".into())
        .py_3()
        .text_size(px(13.))
        .line_height(px(16.))
        .text_color(p.muted_foreground)
        .border_t_1()
        .border_color(line);
    let groups = t.groups.iter().map(|group| {
        let r = group.row;
        let bar = chart.ink_of(&r.key).map_or(p.foreground.opacity(0.45), |i| i.resolve(ink));
        let glyph = split_glyph(split, &r.key, r.agent.as_deref(), looks, cx);
        let first = identity(glyph, r.label.clone(), None, Some(share_bar(group.share, bar, cx).into_any_element()), cx);
        let key = r.key.clone();
        v_flex()
            .debug_selector(move || format!("used-row-{key}"))
            .pb_1()
            .border_t_1()
            .border_color(line)
            .child(row(first.into_any_element(), cells_of(r)).py_4())
            .children(group.models.iter().map(|sub| {
                let label = div()
                    .pl(px(44.))
                    .truncate()
                    .text_sm()
                    .line_height(px(20.))
                    .text_color(p.foreground.opacity(0.8))
                    .child(sub.row.label.clone())
                    .into_any_element();
                {
                    let key = sub.row.key.clone();
                    row(label, cells_of(sub.row)).debug_selector(move || format!("used-sub-{key}")).py(px(10.)).mt(px(-2.))
                }
            }))
    });
    let body = v_flex().border_b_1().border_color(line).child(head).children(groups);
    Card::new("usage-used").locked(true).head(words::by(split.word())).body(body)
}

fn mix(used: &UsedAnswer, cx: &App) -> Option<impl IntoElement> {
    let p = cx.palette();
    let slices = model::mix(used)?;
    let fill = |part: MixPart| match part {
        MixPart::Fresh => p.primary,
        MixPart::CacheWrite => p.primary.opacity(0.5),
        MixPart::Cached => p.foreground.opacity(0.2),
        MixPart::Out => p.foreground.opacity(0.8),
    };
    let bar =
        h_flex().h(px(12.)).gap(px(2.)).rounded(px(4.)).overflow_hidden().children(
            slices.iter().filter(|s| s.n > 0.).map(|s| div().h_full().min_w(px(2.)).w(relative(s.share as f32)).bg(fill(s.part))),
        );
    let legend = h_flex().flex_wrap().gap_x_6().gap_y_2().children(slices.iter().map(|s| {
        let key = s.part.key();
        sized(h_flex().gap_2(), 13.5)
            .debug_selector(move || format!("mix-{key}"))
            .text_color(p.muted_foreground)
            .child(div().size(px(8.)).rounded(px(2.)).bg(fill(s.part)))
            .child(s.part.word())
            .child(div().font_family(mono(cx)).text_color(p.foreground).child(words::fmt_tokens(s.n)))
    }));
    let body = v_flex().debug_selector(|| "usage-mix".into()).gap_3().child(bar).child(legend);
    Some(Card::new("usage-mix-card").locked(true).head(words::MIX).body(body))
}

/// The tab while its answer is on the way, in the loaded page's shape and heights so nothing moves when it lands.
fn skeleton(range: Range, narrow: bool, cx: &App) -> impl IntoElement {
    let bar = |w: f32, h: f32| Skeleton::new(px(w), px(h));
    let cells = (0..4).map(|_| {
        v_flex()
            .flex_1()
            .gap(px(6.))
            .p(px(20.))
            .child(div().my_1().child(bar(64., 12.)))
            .child(div().my(px(2.)).child(bar(112., 28.)))
            .child(div().my(px(2.)).child(bar(96., 12.)))
    });
    let strip = strip(cells.collect(), narrow, cx);
    let axis = v_flex().w(px(36.)).h(CHART_HEIGHT).py_1().justify_between().children((0..5).map(|_| bar(32., 10.)));
    let chart = v_flex()
        .gap_4()
        .child(h_flex().gap_6().child(bar(112., 14.)).child(bar(80., 14.)))
        .child(
            h_flex()
                .gap_3()
                .items_start()
                .child(axis)
                .child(div().flex_1().opacity(0.6).child(Skeleton::new(px(0.), CHART_HEIGHT).full().radius(px(8.)))),
        )
        .child(div().h_4());
    let line = rule(cx);
    let rows = (0..2).map(|_| {
        h_flex()
            .gap_3()
            .py_4()
            .border_t_1()
            .border_color(line)
            .child(bar(32., 32.).radius(px(8.)))
            .child(v_flex().flex_1().gap_2().child(bar(128., 14.)).child(bar(192., 4.)))
            .child(bar(64., 14.))
            .child(div().ml_6().child(bar(80., 14.)))
    });
    let table = v_flex().gap_4().child(bar(80., 14.)).child(
        v_flex().border_b_1().border_color(line).child(div().py_3().border_t_1().border_color(line).child(bar(96., 12.))).children(rows),
    );
    v_flex()
        .debug_selector(|| "used-loading".into())
        .gap(BLOCK_GAP)
        .child(strip)
        .child(Card::new("usage-chart-loading").locked(true).head(words::chart_head(range)).body(chart))
        .child(table)
}
