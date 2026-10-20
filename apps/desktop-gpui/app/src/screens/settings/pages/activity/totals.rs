// SPDX-License-Identifier: AGPL-3.0-only
//! The totals' tiles over their lines of the last minutes, and the skeletons that stand for the tiles and rows.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, FontWeight};

use super::model::{
    cpu_tint, energy_tint, energy_word, memory_share_tint, total_spent, words as w, HistoryPoint, Scope, Tint, HISTORY_POINTS,
};
use super::tables::tint_ink;
use super::{model, note, FIRST_INTERVAL_MS};
use crate::ui::fmt_bytes;
use crate::ui::{text, ActivePalette, Card, CardItem, Skeleton, Sparkline};

/// The figure's mark: warn and danger in their inks.
fn inked(tint: Option<Tint>, cx: &App) -> gpui_kit::Hsla {
    tint_ink(tint, cx)
}

/// One total: its label, the figure inked past its mark, a quiet note, and where it has a history, its line.
#[allow(clippy::too_many_arguments)]
fn tile(
    id: &'static str,
    label: &'static str,
    value: String,
    note_word: String,
    tint: Option<Tint>,
    line: Option<(Vec<(f32, f32)>, gpui_kit::Hsla)>,
    ruled: bool,
    cx: &App,
) -> AnyElement {
    let p = cx.palette();
    v_flex()
        .id(id)
        .debug_selector(move || id.into())
        .flex_1()
        .min_w_0()
        .gap(px(6.))
        .px_5()
        .py_5()
        .when(ruled, |el| el.border_l_1().border_color(p.border.opacity(0.5)))
        .child(div().text_size(text::FACT.0).line_height(px(19.5)).text_color(p.muted_foreground).child(label))
        .child(
            div()
                .font_family(cx.theme().mono_font_family.clone())
                .text_size(px(26.))
                .line_height(px(32.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(inked(tint, cx))
                .child(value),
        )
        .child(div().truncate().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(note_word))
        .children(line.map(|(points, ink)| div().mt_2().child(Sparkline::new(points, ink).height(px(40.)))))
        .into_any_element()
}

/// The tile lines' inks, CPU sky, memory violet and power teal (`LINE_INKS`, `activity/page.tsx`), which these hues hold.
fn line_inks(cx: &App) -> (gpui_kit::Hsla, gpui_kit::Hsla, gpui_kit::Hsla) {
    let p = cx.palette();
    (p.hue_blue, p.hue_violet, p.hue_teal)
}

/// The last minutes as a line, scaled to the highest point in the window and never below the floor.
fn line_points(history: &[HistoryPoint], y: impl Fn(&HistoryPoint) -> f64, floor: f64) -> Vec<(f32, f32)> {
    let to = history.last().map_or(0., |h| h.at);
    let from = to - (HISTORY_POINTS as f64 - 1.) * FIRST_INTERVAL_MS;
    let top = history.iter().map(&y).fold(floor, f64::max) * 1.15;
    history.iter().map(|h| (((h.at - from) / (to - from).max(1.)).clamp(0., 1.) as f32, (y(h) / top).clamp(0., 1.) as f32)).collect()
}

/// The tiles on their grid: one row with a rule between each, or two columns with none under 640 px
/// (`max-sm:grid-cols-2 max-sm:divide-x-0`), a short last row keeping its columns' widths.
fn tile_grid(id: &'static str, tiles: Vec<AnyElement>, narrow: bool) -> gpui_kit::Div {
    let per = if narrow { 2 } else { tiles.len().max(1) };
    let mut tiles = tiles.into_iter();
    let mut grid = v_flex().debug_selector(move || id.into());
    loop {
        let mut line: Vec<AnyElement> = tiles.by_ref().take(per).collect();
        if line.is_empty() {
            break;
        }
        line.extend((line.len()..per).map(|_| div().flex_1().min_w_0().into_any_element()));
        grid = grid.child(h_flex().items_stretch().children(line));
    }
    grid
}

/// wsp's totals on this computer, each figure over a line of the last minutes; the process count stands alone.
pub(super) fn tiles(scope: &Scope, history: &[HistoryPoint], mem: Option<(f64, f64)>, narrow: bool, cx: &App) -> AnyElement {
    let t = total_spent(&scope.groups);
    let threads = scope.groups.iter().filter(|g| g.kind == model::GroupKind::Thread).count();
    let mem_note = mem.map_or_else(|| w::PROCESSES.to_owned(), |(used, _)| w::of(&fmt_bytes(used as u64)));
    let (cpu_ink, mem_ink, energy_ink) = line_inks(cx);
    let mut row = vec![
        tile(
            "activity-mem",
            w::WSP,
            fmt_bytes(t.mem as u64),
            mem_note,
            memory_share_tint(t.mem, mem.map(|m| m.1)),
            Some((line_points(history, |h| h.mem, 1024f64.powi(3)), mem_ink)),
            false,
            cx,
        ),
        tile(
            "activity-cpu",
            w::CPU,
            format!("{}%", t.cpu.round()),
            w::ONE_CORE.into(),
            cpu_tint(t.cpu),
            Some((line_points(history, |h| h.cpu, 100.), cpu_ink)),
            !narrow,
            cx,
        ),
    ];
    if scope.energy {
        row.push(tile(
            "activity-energy",
            w::ENERGY,
            energy_word(t.energy),
            w::ENERGY_NOTE.into(),
            energy_tint(t.energy),
            Some((line_points(history, |h| h.energy.unwrap_or(0.), 1000.), energy_ink)),
            !narrow,
            cx,
        ));
    }
    row.push(tile("activity-procs", w::PROCESSES, t.count.to_string(), w::threads(threads), None, None, !narrow, cx));
    Card::new("activity-totals").locked(true).item(CardItem::any(tile_grid("activity-totals", row, narrow))).into_any_element()
}

/// A bar in the skeleton's tier at the width of whatever holds it.
fn bar(height: f32, cx: &App) -> impl IntoElement {
    div().w_full().h(px(height)).rounded(px(6.)).bg(cx.palette().border)
}

/// Whether the skeletons sweep: not while a dial waits to come again, nor once the watch was refused, since nothing
/// will arrive to end the sweep and each frame of it draws the whole window again.
pub(super) fn skeletons_move(waiting: bool, refused: bool) -> bool {
    !waiting && !refused
}

/// A skeleton bar, held still once a dial has failed: each frame of its sweep draws the whole window again.
fn skeleton(width: f32, height: f32, moving: bool, border: gpui_kit::Hsla) -> AnyElement {
    if moving {
        return Skeleton::new(px(width), px(height)).into_any_element();
    }
    div().flex_none().w(px(width)).h(px(height)).rounded(px(6.)).bg(border).into_any_element()
}

pub(super) fn tiles_skeleton(moving: bool, narrow: bool, cx: &App) -> AnyElement {
    let p = cx.palette();
    let cell = |ix: usize| {
        v_flex()
            .flex_1()
            .min_w_0()
            .gap(px(6.))
            .px_5()
            .py_5()
            .when(ix > 0 && !narrow, |el| el.border_l_1().border_color(p.border.opacity(0.5)))
            .child(div().my_1().child(skeleton(64., 12., moving, p.border)))
            .child(div().my(px(2.)).child(skeleton(112., 28., moving, p.border)))
            .child(div().my(px(2.)).child(skeleton(96., 12., moving, p.border)))
            .child(div().mt_2().child(bar(40., cx)))
            .into_any_element()
    };
    let grid = tile_grid("activity-totals-loading", (0..4).map(cell).collect(), narrow);
    Card::new("activity-totals-loading").locked(true).item(CardItem::any(grid)).into_any_element()
}

pub(super) fn rows_skeleton(moving: bool, cx: &App) -> impl IntoElement {
    let border = cx.palette().border;
    let row = |ix: usize| {
        CardItem::placed(move |corners| {
            corners
                .apply(h_flex())
                .id(("activity-loading", ix))
                .min_h(px(60.))
                .gap_3()
                .px(corners.inset)
                .py_3()
                .child(skeleton(32., 32., moving, border))
                .child(skeleton(192., 16., moving, border))
                .into_any_element()
        })
    };
    v_flex()
        .debug_selector(|| "activity-loading".into())
        .child(note(w::PENDING, cx).mb_4())
        .child(Card::new("activity-loading").locked(true).items((0..3).map(row)))
}
