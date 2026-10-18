// SPDX-License-Identifier: AGPL-3.0-only
//! The Usage tab: the range and the split, the totals strip, the chart, the split table and where the tokens went.
//! The last range's figures stand while the next range is read.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme};
use gpui_kit::{div, prelude::*, px, relative, AnyElement, App, Context, FontWeight, Hsla, SharedString, Window};
use wsp_host_client::protocol::UsedAnswer;

use super::chart::{self, UsageChart, CHART_HEIGHT};
use super::model::{self, Chart, InkTheme, MixPart, Range, Split};
use super::parts::{self, bone, card, identity, quiet, rule, share_bar, sized, split_glyph, surface, BLOCK_GAP};
use super::{words, UsagePage};
use crate::ui::{ActivePalette, Segmented};

/// The split table's columns past the first: tokens, cache hit, turns and the estimate.
const COLUMNS: [f32; 4] = [104., 96., 80., 120.];
const COLUMN_GAP: f32 = 24.;

pub fn render(page: &UsagePage, _now: f64, window: &mut Window, cx: &mut Context<UsagePage>) -> AnyElement {
    let controls = controls(page, cx);
    let mut section = v_flex().debug_selector(|| "usage-used".into()).gap(BLOCK_GAP).child(controls);
    section = match (&page.used.value, &page.used.refused) {
        (None, Some(refused)) => section.child(quiet(words::used_refused(refused), cx).py(px(20.))),
        (None, None) => section.child(skeleton(page.range, cx)),
        (Some(used), _) if used.rows.is_empty() => {
            section.child(quiet(words::NO_USE, cx).py(px(40.)).w_full().flex().justify_center().debug_selector(|| "no-use".into()))
        }
        (Some(used), _) => {
            let models = if used.split == wsp_host_client::protocol::UsedAnswerSplit::Agent && page.models.refused.is_none() {
                page.models.value.as_ref().filter(|m| m.range == used.range).map_or(&[][..], |m| &m.rows[..])
            } else {
                &[][..]
            };
            let chart = Chart::of(used, &page.projects);
            let ink = InkTheme {
                foreground: cx.palette().foreground,
                background: cx.palette().background,
                muted_foreground: cx.palette().muted_foreground,
                dark: cx.theme().is_dark(),
            };
            section
                .child(totals(used, cx))
                .child(chart_card(used, &chart, &ink, window, cx))
                .child(table(used, models, &chart, &ink, &page.projects, cx))
                .children(mix(used, cx))
                .children(used.logs.as_ref().map(|logs| quiet(words::logs_line(&logs.agents, &logs.computers), cx)))
        }
    };
    section.into_any_element()
}

fn controls(page: &UsagePage, cx: &mut Context<UsagePage>) -> impl IntoElement {
    let (ranges, splits) = (Range::ALL, Split::ALL);
    let entity = cx.entity();
    let range = ranges.iter().fold(
        Segmented::new("usage-range", ranges.iter().position(|r| *r == page.range).unwrap_or(1))
            .height(px(36.))
            .segment_size((px(14.), px(20.)), px(14.)),
        |s, r| s.segment(r.word()),
    );
    let split = splits.iter().fold(
        Segmented::new("usage-split", splits.iter().position(|s| *s == page.split).unwrap_or(0))
            .height(px(36.))
            .segment_size((px(14.), px(20.)), px(14.)),
        |s, split| s.segment(split.word()),
    );
    let on_split = entity.clone();
    h_flex()
        .flex_wrap()
        .justify_between()
        .gap_x_4()
        .gap_y_2()
        .child(range.on_pick(move |ix, _, cx| entity.update(cx, |page, cx| page.pick_range(ranges[ix], cx))))
        .child(split.on_pick(move |ix, _, cx| on_split.update(cx, |page, cx| page.pick_split(splits[ix], cx))))
}

fn mono(cx: &App) -> SharedString {
    cx.theme().mono_font_family.clone()
}

fn totals(used: &UsedAnswer, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let stats = model::Totals::of(used).stats();
    let count = stats.len();
    surface(cx).debug_selector(|| "usage-totals".into()).flex_row().children(stats.into_iter().enumerate().map(|(ix, stat)| {
        v_flex()
            .debug_selector(move || format!("stat-{}", stat.key))
            .flex_1()
            .min_w_0()
            .gap(px(6.))
            .p(px(20.))
            .when(ix + 1 < count, |el| el.border_r_1().border_color(rule(cx)))
            .child(sized(div(), 13.).text_color(p.muted_foreground).child(stat.label))
            .child(
                div()
                    .font_family(mono(cx))
                    .text_size(px(26.))
                    .line_height(px(32.))
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(stat.value),
            )
            .children(stat.note.map(|note| div().truncate().text_xs().line_height(px(16.)).text_color(p.muted_foreground).child(note)))
    }))
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
    let body = v_flex().gap_4().child(legend(items, window, cx)).child(
        UsageChart::new("usage-chart-plot", steps, lines, ticks)
            .figure(words::fmt_tokens)
            .step_word(Rc::new(move |t| words::step_word(range, t, &chrono::Local))),
    );
    card("usage-chart", Some(words::chart_head(range).into()), body, cx)
}

fn table(used: &UsedAnswer, models: &[model::Row], chart: &Chart, ink: &InkTheme, projects: &[String], cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let split = Split::from(used.split);
    let t = model::split_table(used, models);
    let line = rule(cx);
    let head_cell = |text: &'static str| div().child(text).into_any_element();
    let mut heads = vec![head_cell(words::TOKENS), head_cell(words::CACHE_HIT)];
    if t.turns {
        heads.push(head_cell(words::TURNS));
    }
    heads.push(head_cell(words::ESTIMATE));
    let mut widths = COLUMNS.to_vec();
    if !t.turns {
        widths.remove(2);
    }
    let row = move |first: AnyElement, cells: Vec<AnyElement>| {
        h_flex()
            .gap(px(COLUMN_GAP))
            .child(div().flex_1().min_w_0().child(first))
            .children(cells.into_iter().zip(widths.clone()).map(|(cell, w)| div().flex_none().w(px(w)).flex().justify_end().child(cell)))
    };
    let number = |text: String, color: Hsla| {
        div().font_family(mono(cx)).text_sm().line_height(px(20.)).text_color(color).child(text).into_any_element()
    };
    let cells_of = |r: &model::Row| {
        let c = model::row_cells(r);
        let mut cells = vec![number(c.tokens, p.foreground), number(c.cache_hit, p.muted_foreground)];
        if t.turns {
            cells.push(number(c.turns, p.muted_foreground));
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
        let glyph = split_glyph(split, &r.key, r.agent.as_deref(), projects, cx);
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
    card("usage-by", Some(words::by(split.word()).into()), body, cx)
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
    Some(card("usage-mix", Some(words::MIX.into()), v_flex().gap_3().child(bar).child(legend), cx))
}

/// The tab while its answer is on the way, in the loaded page's shape and heights so nothing moves when it lands.
fn skeleton(range: Range, cx: &App) -> impl IntoElement {
    let strip = surface(cx).flex_row().children((0..4).map(|ix| {
        v_flex()
            .flex_1()
            .gap(px(6.))
            .p(px(20.))
            .when(ix < 3, |el| el.border_r_1().border_color(rule(cx)))
            .child(bone(px(64.), px(12.), cx).my_1())
            .child(bone(px(112.), px(28.), cx).my(px(2.)))
            .child(bone(px(96.), px(12.), cx).my(px(2.)))
    }));
    let axis = v_flex().w(px(36.)).h(CHART_HEIGHT).py_1().justify_between().children((0..5).map(|_| bone(px(32.), px(10.), cx)));
    let chart = v_flex()
        .gap_4()
        .child(h_flex().gap_6().child(bone(px(112.), px(14.), cx)).child(bone(px(80.), px(14.), cx)))
        .child(
            h_flex()
                .gap_3()
                .items_start()
                .child(axis)
                .child(div().flex_1().h(CHART_HEIGHT).rounded(px(8.)).bg(cx.palette().border).opacity(0.6)),
        )
        .child(div().h_4());
    let line = rule(cx);
    let rows = (0..2).map(|_| {
        h_flex()
            .gap_3()
            .py_4()
            .border_t_1()
            .border_color(line)
            .child(div().size(px(32.)).rounded(px(8.)).bg(cx.palette().border))
            .child(v_flex().flex_1().gap_2().child(bone(px(128.), px(14.), cx)).child(bone(px(192.), px(4.), cx)))
            .child(bone(px(64.), px(14.), cx))
            .child(bone(px(80.), px(14.), cx).ml_6())
    });
    let table = v_flex().gap_4().child(bone(px(80.), px(14.), cx)).child(
        v_flex()
            .border_b_1()
            .border_color(line)
            .child(div().py_3().border_t_1().border_color(line).child(bone(px(96.), px(12.), cx)))
            .children(rows),
    );
    v_flex()
        .debug_selector(|| "used-loading".into())
        .gap(BLOCK_GAP)
        .child(strip)
        .child(card("usage-chart-loading", Some(words::chart_head(range).into()), chart, cx))
        .child(table)
}
