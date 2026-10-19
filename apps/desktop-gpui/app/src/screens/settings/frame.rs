// SPDX-License-Identifier: AGPL-3.0-only
//! The frame every page stands in: the settings sidebar (`SettingsSidebar.tsx`), the top bar's crumbs
//! (`SettingsCrumbs.tsx`) and act (`SettingsHeaderActions.tsx`), the page's column (`SettingsPage.tsx`) and the
//! results page while the field holds text.

use gpui_kit::base::{MotionReveal, Presence, Transition};
use gpui_kit::component::input::{Input, InputState, MoveDown};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, relative, svg, AnyElement, App, Entity, FocusHandle, Focusable as _, FontWeight, Hsla, KeyDownEvent, Pixels,
    ScrollHandle, SharedString, Window,
};

use super::groups::{self, group};
use super::model::{At, GroupId};
use super::page::{CardData, Ctx};
use super::pages;
use crate::ui::focus::ring;
use crate::ui::{motion, narrow, scroll_view, size, text, ActivePalette, Button, Glyph, Palette, LOCKUP as LOCKUP_PATH};

/// The sidebar's width (`SIDEBAR_DEFAULT_WIDTH`).
pub const SIDEBAR_WIDTH: Pixels = px(256.);
/// `--workspace-topbar-height`.
pub const TOP_BAR: Pixels = px(52.);
/// `--sidebar-content-inset`.
const INSET: Pixels = px(8.);
/// `ONE_LINE_ROW_CLASS`'s `h-9`.
const ROW_HEIGHT: Pixels = px(36.);
/// A group's list under it: `ml-[14px]`.
const SUB_INDENT: Pixels = px(14.);
/// The page column's `max-w-[760px]` and `px-8`; Usage and Activity keep the sizes Usage was locked at.
const COLUMN: Pixels = px(760.);
const LOCKED_COLUMN: Pixels = px(1080.);
const COLUMN_PAD: Pixels = px(32.);
/// `max-sm:px-4 max-sm:pt-6`: the page's sides and top under 640 px, Usage's and Activity's too.
const COLUMN_PAD_NARROW: Pixels = px(16.);
const COLUMN_TOP_NARROW: Pixels = px(24.);
/// The wordmark's box: `h-3.5` on its 51 by 14 view box.
const LOCKUP: (Pixels, Pixels) = (px(51.), px(14.));

/// What the centre draws this frame, and the section heads the sidebar's trail lists.
pub struct Body {
    pub cards: Vec<CardData>,
    /// A sentence standing alone in the centre: the host unreachable, nothing matching.
    pub lone: Option<SharedString>,
    /// Each head with the index of its card in the page's column.
    pub trail: Vec<(SharedString, usize)>,
    /// While the field holds text, how many items of each group match, for the sidebar's dimming.
    pub hits: Option<Vec<(GroupId, usize)>>,
    /// While the field holds text, each matching item's id and title by group, the rows a narrow window's sidebar lists.
    pub results: Vec<Results>,
}

/// A group's matches as the narrow sidebar lists them: each item's id and its title.
pub type Results = (GroupId, Vec<(SharedString, SharedString)>);

impl Body {
    pub fn build(ctx: &Ctx, search: &str, unreachable: Option<&str>, window: &mut Window, cx: &mut App) -> Body {
        if let Some(said) = unreachable {
            return Body {
                cards: Vec::new(),
                lone: Some(SharedString::from(said.to_owned())),
                trail: Vec::new(),
                hits: None,
                results: Vec::new(),
            };
        }
        if !search.is_empty() {
            let mut hits = Vec::new();
            let mut cards = Vec::new();
            let mut results = Vec::new();
            for id in GroupId::ALL {
                let found: Vec<_> = pages::group_cards(id, ctx, window, cx).into_iter().flat_map(|card| card.found(search)).collect();
                hits.push((id, found.len()));
                if !found.is_empty() {
                    let titles = found.iter().map(|item| (item.id.clone(), item.words.first().cloned().unwrap_or_default())).collect();
                    results.push((id, titles));
                    let acts = ctx.acts.clone();
                    let card = CardData::new(SharedString::from(format!("search-{}", id.id())))
                        .head(group(id).name)
                        .on_head_click(move |window, cx| acts.go(At::Group(id), window, cx))
                        .items(found);
                    cards.push(card);
                }
            }
            let lone = cards.is_empty().then(|| SharedString::from("Nothing matches."));
            return Body { cards, lone, trail: Vec::new(), hits: Some(hits), results };
        }
        let cards = pages::cards(ctx.at, ctx, window, cx);
        let open = ctx.at.group();
        let trail = if matches!(ctx.at, At::Group(_)) && groups::sub_pages(open, ctx).is_empty() {
            cards.iter().enumerate().filter_map(|(ix, c)| c.head.clone().map(|h| (h, ix))).collect()
        } else {
            Vec::new()
        };
        Body { cards, lone: None, trail, hits: None, results: Vec::new() }
    }
}

/// One 36 px row of the sidebar: the glyph, the words, and a mono word at its edge. A tab stop wearing the ring under
/// the keyboard, which the arrows walk with the others.
#[allow(clippy::too_many_arguments)]
fn sidebar_row(
    id: SharedString,
    focus: &FocusHandle,
    glyph: Option<AnyElement>,
    name: SharedString,
    meta: Option<SharedString>,
    lifted: bool,
    ink: Hsla,
    dimmed: bool,
    p: &Palette,
    mono: SharedString,
    on_click: impl Fn(&mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let rest = p.sidebar_row_hover;
    let ring_ink = p.ring;
    let selector = id.to_string();
    h_flex()
        .id(id)
        .debug_selector(move || selector.clone())
        .track_focus(focus)
        .focus_visible(move |s| s.shadow(vec![ring(ring_ink)]))
        .h(ROW_HEIGHT)
        .w_full()
        .flex_none()
        .gap(px(10.))
        .px_2()
        .rounded(size::BUTTON_RADIUS)
        .text_size(text::SETTING_TITLE.0)
        .line_height(text::SETTING_TITLE.1)
        .text_color(if lifted { p.sidebar_foreground } else { ink })
        .cursor_pointer()
        .when(dimmed, |el| el.opacity(0.5))
        .when(lifted, |el| {
            el.bg(p.sidebar_row_selected).font_weight(FontWeight::MEDIUM).border_1().border_color(p.sidebar_row_edge).px(px(7.))
        })
        .when(!lifted, |el| el.hover(|s| s.bg(rest).text_color(p.sidebar_foreground)))
        .on_click(move |_, window, cx| on_click(window, cx))
        .children(glyph)
        .child(div().flex_1().min_w_0().truncate().child(name))
        .children(meta.map(|m| div().flex_none().font_family(mono).text_size(px(11.)).text_color(p.top_row_meta).child(m)))
}

/// Whether a group's row reads in the foreground, and its glyph: the open group only while a page under it stands,
/// whether or not the field holds text, and the glyph only while the row is lifted (`SettingsSidebar`'s group row).
pub fn group_inks(open: bool, on_its_page: bool, lifted: bool) -> (bool, bool) {
    (lifted || (open && !on_its_page), lifted)
}

/// A row's focus, kept across frames by the row's id.
fn row_focus(id: &SharedString, window: &mut Window, cx: &mut App) -> FocusHandle {
    window.use_keyed_state(SharedString::from(format!("{id}-focus")), cx, |_, cx| cx.focus_handle().tab_stop(true)).read(cx).clone()
}

/// The row a key moves to from `current` among `count` rows in reading order (`SettingsSidebar`'s `onKeyDown`): Down
/// enters the list from outside it, Up, Home and End only move within it, and neither arrow goes past an end.
pub fn walk(key: &str, current: Option<usize>, count: usize) -> Option<usize> {
    if count == 0 {
        return None;
    }
    match (key, current) {
        ("down", None) => Some(0),
        ("down", Some(ix)) => Some((ix + 1).min(count - 1)),
        ("up", Some(ix)) => Some(ix.saturating_sub(1)),
        ("home", Some(_)) => Some(0),
        ("end", Some(_)) => Some(count - 1),
        _ => None,
    }
}

/// A group's list under it, opening as one block whose height grows from nothing (`animate-sub-open`), so the groups
/// under it slide rather than jump; it stands open at once under reduced motion.
fn opening(id: SharedString, list: gpui_kit::Div, window: &mut Window, cx: &mut App) -> AnyElement {
    let selector = id.to_string();
    let shell = div().w_full().debug_selector(move || selector.clone());
    if motion::reduced(cx) {
        return shell.child(list).into_any_element();
    }
    let policy = Transition::new(motion::SUB_OPEN).easing(motion::sub_open_curve());
    let progress = Presence::new(id.clone(), true).transition(policy).sample(window, cx).progress;
    // Opened, the rows stand on their own: a press on a row under `MotionReveal` never reaches it.
    if progress >= 1. {
        return shell.child(list).into_any_element();
    }
    shell.child(MotionReveal::new(id, progress, list.opacity(progress).into_any_element())).into_any_element()
}

/// The settings sidebar in the app sidebar's place. `results` stands in place of the groups in a narrow window while the
/// field holds text: each matching title under its group's name, a press opening the group on that row.
#[allow(clippy::too_many_arguments)]
pub fn sidebar(
    ctx: &Ctx,
    field: &Entity<InputState>,
    hits: Option<&[(GroupId, usize)]>,
    results: Option<&[Results]>,
    trail: &[(SharedString, usize, bool)],
    frame_row: bool,
    window_height: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    let searching = hits.is_some();
    let p = cx.palette().clone();
    let mono = cx.theme().mono_font_family.clone();
    let open = ctx.at.group();
    let mut walked: Vec<FocusHandle> = Vec::new();
    let mut list = v_flex().w_full().gap_1().px(INSET).pt_1();
    if let Some(results) = results {
        for (group_id, items) in results {
            let mut rows = v_flex().ml(SUB_INDENT).min_w_0();
            for (item, title) in items {
                let id = SharedString::from(format!("settings-result-{}-{item}", group_id.id()));
                let focus = row_focus(&id, window, cx);
                walked.push(focus.clone());
                let (acts, group_id, item) = (ctx.acts.clone(), *group_id, item.clone());
                rows = rows.child(sidebar_row(
                    id,
                    &focus,
                    None,
                    title.clone(),
                    None,
                    false,
                    p.sidebar_muted_foreground,
                    false,
                    &p,
                    mono.clone(),
                    move |window, cx| {
                        acts.go(At::Group(group_id), window, cx);
                        acts.reveal_item(item.clone(), cx);
                    },
                ));
            }
            let head = h_flex().h(px(28.)).px_2().text_size(px(13.)).text_color(p.sidebar_muted_foreground).child(group(*group_id).name);
            let rows = opening(SharedString::from(format!("settings-results-{}", group_id.id())), rows, window, cx);
            list = list.child(v_flex().min_w_0().child(head).child(rows));
        }
    }
    for id in GroupId::ALL.into_iter().filter(|_| results.is_none()) {
        let g = group(id);
        let group_at = At::Group(id);
        let is_open = open == id;
        let dimmed = hits.is_some_and(|h| h.iter().any(|(gid, n)| *gid == id && *n == 0));
        let lifted = !searching && *ctx.at == group_at;
        let (text_lit, glyph_lit) = group_inks(is_open, *ctx.at == group_at, lifted);
        let ink = if text_lit { p.sidebar_foreground } else { p.sidebar_muted_foreground };
        let acts = ctx.acts.clone();
        let glyph = Icon::new(g.glyph).with_size(px(16.)).text_color(if glyph_lit { p.sidebar_foreground } else { p.sidebar_icon_color });
        let row_id = SharedString::from(format!("settings-{}", id.id()));
        let focus = row_focus(&row_id, window, cx);
        walked.push(focus.clone());
        let mut item = v_flex().min_w_0().child(sidebar_row(
            row_id,
            &focus,
            Some(glyph.into_any_element()),
            g.name.into(),
            groups::meta(id, ctx),
            lifted,
            ink,
            dimmed,
            &p,
            mono.clone(),
            move |window, cx| acts.go(At::Group(id), window, cx),
        ));
        if !is_open {
            list = list.child(item);
            continue;
        }
        let under = groups::sub_pages(id, ctx);
        if !under.is_empty() {
            let mut subs = v_flex().ml(SUB_INDENT).min_w_0();
            for (at, name) in under {
                let lifted = !searching && *ctx.at == at;
                let acts = ctx.acts.clone();
                let target = at.clone();
                let glyph = pages::sub_glyph(&at, ctx, cx);
                let row_id = SharedString::from(format!("settings-row-{}", at.id()));
                let focus = row_focus(&row_id, window, cx);
                walked.push(focus.clone());
                subs = subs.child(sidebar_row(
                    row_id,
                    &focus,
                    glyph,
                    name,
                    None,
                    lifted,
                    p.sidebar_muted_foreground,
                    dimmed,
                    &p,
                    mono.clone(),
                    move |window, cx| acts.go(target.clone(), window, cx),
                ));
            }
            item = item.child(opening(SharedString::from(format!("settings-sub-{}", id.id())), subs, window, cx));
        } else if !trail.is_empty() && !searching {
            let mut subs = v_flex().ml(SUB_INDENT).min_w_0();
            for (head, ix, in_view) in trail.iter().cloned() {
                let acts = ctx.acts.clone();
                let row_id = SharedString::from(format!("settings-section-{ix}"));
                let focus = row_focus(&row_id, window, cx);
                walked.push(focus.clone());
                let lit = div().when(in_view, |el| el.debug_selector(move || format!("settings-section-lit-{ix}")));
                subs = subs.child(lit.child(sidebar_row(
                    row_id,
                    &focus,
                    None,
                    head,
                    None,
                    false,
                    if in_view { p.sidebar_foreground } else { p.sidebar_muted_foreground },
                    false,
                    &p,
                    mono.clone(),
                    move |_, cx| acts.reveal_section(ix, cx),
                )));
            }
            item = item.child(opening(SharedString::from(format!("settings-trail-{}", id.id())), subs, window, cx));
        }
        list = list.child(item);
    }
    let first = walked.first().cloned();
    let field_focus = field.read(cx).focus_handle(cx);
    let search = v_flex()
        .px(INSET)
        .pt_3()
        .pb_1()
        // The field's own binding takes ArrowDown before any key listener hears it, so the list is entered from here.
        .capture_action(move |_: &MoveDown, window, cx| {
            if let Some(first) = first.as_ref().filter(|_| field_focus.is_focused(window)) {
                window.focus(first, cx);
                cx.stop_propagation();
            }
        })
        .child(
            h_flex()
                .h(px(28.))
                .gap_2()
                .px_2()
                .border_b_1()
                .border_color(p.sidebar_border)
                .child(Icon::new(Glyph::Search).with_size(px(14.)).text_color(p.sidebar_muted_foreground))
                .child(div().flex_1().min_w_0().text_size(px(13.)).text_color(p.sidebar_foreground).child(
                    div().flex_1().debug_selector(|| "settings-search".into()).child(Input::new(field).appearance(false).px_0().h(px(28.))),
                )),
        );
    let acts = ctx.acts.clone();
    let back = div().p(INSET).child(
        div().px_1().child(
            h_flex()
                .id("settings-back")
                .debug_selector(|| "settings-back".into())
                .h(ROW_HEIGHT)
                .w_full()
                .gap(px(10.))
                .px_2()
                .rounded(size::BUTTON_RADIUS)
                .text_size(text::SETTING_TITLE.0)
                .text_color(p.sidebar_muted_foreground)
                .cursor_pointer()
                .hover(|s| s.text_color(p.sidebar_foreground))
                .on_click(move |_, window, cx| acts.close(window, cx))
                .child(Icon::new(Glyph::ArrowLeft).with_size(px(14.)))
                .child(div().flex_1().min_w_0().child("Back"))
                .child(div().flex_none().font_family(mono.clone()).text_size(px(11.)).text_color(p.top_row_meta).child("esc")),
        ),
    );
    let groups_list = div()
        .id("settings-groups")
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .on_key_down(move |event: &KeyDownEvent, window, cx| {
            if event.keystroke.modifiers.modified() {
                return;
            }
            let current = walked.iter().position(|f| f.is_focused(window));
            if let Some(next) = walk(&event.keystroke.key, current, walked.len()) {
                window.focus(&walked[next], cx);
                cx.stop_propagation();
            }
        })
        .child(list);
    v_flex()
        .debug_selector(|| "settings-sidebar".into())
        .h_full()
        .text_color(p.sidebar_foreground)
        .relative()
        // In the app's frame the column is the frame's, which the person may drag wider or narrower: its width, its
        // ground, its grain and its edge are drawn there.
        .when(frame_row, |el| {
            el.w(SIDEBAR_WIDTH)
                .flex_none()
                .bg(p.sidebar)
                .border_r_1()
                .border_color(p.sidebar_border)
                .child(crate::ui::grain(gpui_kit::size(SIDEBAR_WIDTH, window_height)))
                .child(header_row(&p))
        })
        .when(!frame_row, |el| el.w_full())
        .child(search)
        .child(groups_list)
        .child(back)
}

/// The sidebar's frame row: the toggle, then the wordmark.
fn header_row(p: &Palette) -> impl IntoElement {
    h_flex()
        .h(TOP_BAR)
        .flex_none()
        .pl(INSET)
        .gap(px(16.))
        .child(Button::icon("settings-sidebar-toggle", Glyph::PanelLeftClose))
        .child(svg().path(LOCKUP_PATH).w(LOCKUP.0).h(LOCKUP.1).mt(px(4.)).text_color(p.muted_foreground))
}

/// The top bar over the page in a window with no frame: the crumbs, and Restore defaults or a group's tabs at its right.
pub fn top_bar(ctx: &Ctx, searching: bool, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let act = actions(ctx, window, cx);
    h_flex()
        .debug_selector(|| "settings-top-bar".into())
        .h(TOP_BAR)
        .flex_none()
        .pl(px(20.))
        .pr(px(20.))
        .gap(px(16.))
        .child(crumbs(ctx, searching, cx))
        .child(div().flex_1())
        .children(act.map(|a| div().mr(px(1.)).child(a)))
}

/// The crumbs: the back chevron under a page, then Settings, its group and the page, or Search while searching.
pub fn crumbs(ctx: &Ctx, searching: bool, cx: &mut App) -> impl IntoElement {
    let p = cx.palette().clone();
    let group_id = ctx.at.group();
    let g = group(group_id);
    let page_name = page_name(ctx).or_else(|| ctx.level.map(|l| l.name.clone()));
    let project = match ctx.at {
        At::Project(id) if ctx.data.projects.iter().any(|p| p.id == *id) => Some(id.clone()),
        _ => None,
    };
    let slash = || div().flex_none().text_color(p.muted_foreground.opacity(0.5)).child("/");
    let mut crumbs = h_flex().min_w_0().gap_2().text_size(text::SETTING_TITLE.0).line_height(text::SETTING_TITLE.1);
    let sub = !searching && page_name.is_some();
    if sub {
        let acts = ctx.acts.clone();
        let up = ctx.level.and_then(|l| l.up.as_deref().cloned());
        let back = format!("Back to {}", up.as_ref().map_or(g.name, |up| up.name.as_ref()));
        crumbs = crumbs.child(div().debug_selector(|| "settings-up".into()).ml(px(-4.)).child(
            Button::icon_xs("settings-up", Glyph::ChevronLeft).tooltip(back).on_click(move |_, window, cx| match up.clone() {
                Some(up) => acts.open_level(Some(up), cx),
                None => acts.go(At::Group(group_id), window, cx),
            }),
        ));
    }
    crumbs = crumbs.child(div().min_w_0().truncate().text_color(p.muted_foreground).child("Settings")).child(slash());
    crumbs = if searching {
        crumbs.child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(p.foreground).child("Search"))
    } else if let Some(name) = page_name {
        let acts = ctx.acts.clone();
        crumbs
            .child(
                div()
                    .id("settings-crumb-group")
                    .min_w_0()
                    .truncate()
                    .cursor_pointer()
                    .text_color(p.muted_foreground)
                    .hover(|s| s.text_color(p.foreground))
                    .on_click(move |_, window, cx| acts.go(At::Group(group_id), window, cx))
                    .child(g.name),
            )
            .child(slash())
            .child(
                h_flex()
                    .flex_shrink_0()
                    .min_w_0()
                    .max_w(relative(0.7))
                    .gap_2()
                    .font_weight(FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .debug_selector(|| "settings-crumb-page".into())
                    .children(project.map(|id| {
                        let mark = pages::projects::look::project_mark(ctx, &id, Some(p.foreground), cx);
                        div().flex_none().debug_selector(|| "settings-crumb-glyph".into()).child(mark)
                    }))
                    .child(div().min_w_0().truncate().child(name)),
            )
    } else {
        crumbs.child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(p.foreground).child(g.name))
    };
    crumbs.debug_selector(|| "settings-crumbs".into())
}

/// The header's act for the open group: its tabs (Activity), else Restore defaults while a row is off its default.
pub fn actions(ctx: &Ctx, window: &mut Window, cx: &mut App) -> Option<AnyElement> {
    match ctx.at {
        At::Group(id) => pages::head(*id, ctx, window, cx).or_else(|| restore(*id, ctx, narrow::width(window).sm)),
        _ => None,
    }
}

/// Restore defaults, drawn only while a row of the open group is off its default; under 640 px its glyph alone, its
/// words on its hover.
fn restore(id: GroupId, ctx: &Ctx, narrow: bool) -> Option<AnyElement> {
    let restore = group(id).restore?;
    let off = ctx.data.preferences().is_some_and(|p| (restore.off)(p));
    if !off {
        return None;
    }
    let acts = ctx.acts.clone();
    let patch = restore.patch;
    let button = Button::ghost("restore-defaults", RESTORE)
        .glyph(Glyph::RotateCcw)
        .xs()
        .tooltip(RESTORE)
        .tooltip_side(gpui_kit::TooltipSide::Bottom);
    let button = button.glyph_only(narrow).on_click(move |_, _, cx| acts.set_preferences(patch(), cx));
    Some(div().debug_selector(|| "restore-defaults".into()).child(button).into_any_element())
}

/// `SETTINGS_WORDS.restore`.
const RESTORE: &str = "Restore defaults";

/// The page's own name where it is under a group: a computer's, a project's or an agent's.
fn page_name(ctx: &Ctx) -> Option<SharedString> {
    match ctx.at {
        At::Group(_) => None,
        At::Computer(id) => ctx.data.places.iter().find(|p| p.id == *id).map(|p| pages::computers::place_name(p).into()),
        At::Project(id) => ctx.data.projects.iter().find(|p| p.id == *id).map(|p| p.name.clone().into()),
        At::Agent(id) => {
            Some(ctx.data.harnesses.iter().find(|h| h.harness == *id).map(|h| h.label.clone()).unwrap_or_else(|| id.clone()).into())
        }
    }
}

/// The page column's sides and top: the locked pages' top, else the settings design's, both 24 under 640 px.
fn column_pad(locked: bool, narrow: bool) -> (Pixels, Pixels) {
    match (narrow, locked) {
        (true, _) => (COLUMN_PAD_NARROW, COLUMN_TOP_NARROW),
        (false, true) => (COLUMN_PAD, px(56.)),
        (false, false) => (COLUMN_PAD, px(28.)),
    }
}

/// The centre: the page's cards in one column, scrolled as a whole.
pub fn page(body: Body, scroll: &ScrollHandle, locked: bool, narrow: bool, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.palette().clone();
    let column_width = if locked { LOCKED_COLUMN } else { COLUMN };
    let (sides, top) = column_pad(locked, narrow);
    // `mx-auto max-w-[760px] px-8` under border-box sizing: each child centred at the column's width less its padding,
    // in whatever width the frame leaves the page. One wrapper per card keeps a card the column's child at its index.
    let centred = move |el: AnyElement| div().w_full().max_w(column_width - sides * 2.).mx_auto().child(el);
    let mut column =
        v_flex().debug_selector(|| "settings-page".into()).px(sides).pt(top).gap(if locked { px(48.) } else { size::PAGE_GAP });
    if let Some(lone) = body.lone {
        column = column.child(centred(
            div()
                .w_full()
                .py_3()
                .text_center()
                .text_size(px(13.))
                .line_height(px(20.))
                .text_color(p.muted_foreground)
                .child(lone)
                .into_any_element(),
        ));
    }
    let gap = if locked { px(48.) } else { size::PAGE_GAP };
    // `pb-12` as a last child: a scroll box here does not count its own bottom padding in how far it scrolls.
    let column = column
        .children(body.cards.into_iter().map(|card| centred(card.render().locked(locked).into_any_element())))
        .child(div().flex_none().h(px(48.) - gap));
    scroll_view("settings-page", scroll, column, window, cx).flex_1().min_h_0()
}

/// A page that draws itself, Usage's, in the locked column on the page's scroll.
pub fn page_of(view: AnyElement, scroll: &ScrollHandle, narrow: bool, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let (sides, top) = column_pad(true, narrow);
    let column = v_flex()
        .debug_selector(|| "settings-page".into())
        .px(sides)
        .pt(top)
        .child(div().w_full().max_w(LOCKED_COLUMN - sides * 2.).mx_auto().child(view))
        .child(div().flex_none().h(px(48.)));
    scroll_view("settings-page", scroll, column, window, cx).flex_1().min_h_0()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_column_takes_16_px_sides_and_a_24_px_top_under_640_locked_or_not() {
        assert_eq!(column_pad(false, false), (px(32.), px(28.)));
        assert_eq!(column_pad(true, false), (px(32.), px(56.)));
        assert_eq!(column_pad(false, true), (px(16.), px(24.)));
        assert_eq!(column_pad(true, true), (px(16.), px(24.)));
    }
}
