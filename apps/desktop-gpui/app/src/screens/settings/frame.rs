// SPDX-License-Identifier: AGPL-3.0-only
//! The frame every page stands in: the settings sidebar (`SettingsSidebar.tsx`), the top bar's crumbs
//! (`SettingsCrumbs.tsx`) and act (`SettingsHeaderActions.tsx`), the page's column (`SettingsPage.tsx`) and the
//! results page while the field holds text.

use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, svg, AnyElement, App, Entity, FontWeight, Hsla, Pixels, ScrollHandle, SharedString, WeakEntity, Window,
};

use super::groups::{self, group};
use super::model::{At, GroupId};
use super::page::{CardData, Ctx};
use super::{pages, SettingsScreen};
use crate::ui::{size, text, ActivePalette, Button, Glyph, Palette, LOCKUP as LOCKUP_PATH};

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
}

impl Body {
    pub fn build(ctx: &Ctx, search: &str, unreachable: Option<&str>, window: &mut Window, cx: &mut App) -> Body {
        if let Some(said) = unreachable {
            return Body { cards: Vec::new(), lone: Some(SharedString::from(said.to_owned())), trail: Vec::new(), hits: None };
        }
        if !search.is_empty() {
            let mut hits = Vec::new();
            let mut cards = Vec::new();
            for id in GroupId::ALL {
                let found: Vec<_> = pages::group_cards(id, ctx, window, cx).into_iter().flat_map(|card| card.found(search)).collect();
                hits.push((id, found.len()));
                if !found.is_empty() {
                    let acts = ctx.acts.clone();
                    let card = CardData::new(SharedString::from(format!("search-{}", id.id())))
                        .head(group(id).name)
                        .on_head_click(move |window, cx| acts.go(At::Group(id), window, cx))
                        .items(found);
                    cards.push(card);
                }
            }
            let lone = cards.is_empty().then(|| SharedString::from("Nothing matches."));
            return Body { cards, lone, trail: Vec::new(), hits: Some(hits) };
        }
        let cards = pages::cards(ctx.at, ctx, window, cx);
        let open = ctx.at.group();
        let trail = if matches!(ctx.at, At::Group(_)) && groups::sub_pages(open, ctx).is_empty() {
            cards.iter().enumerate().filter_map(|(ix, c)| c.head.clone().map(|h| (h, ix))).collect()
        } else {
            Vec::new()
        };
        Body { cards, lone: None, trail, hits: None }
    }
}

/// One 36 px row of the sidebar: the glyph, the words, and a mono word at its edge.
#[allow(clippy::too_many_arguments)]
fn sidebar_row(
    id: SharedString,
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
    let selector = id.to_string();
    h_flex()
        .id(id)
        .debug_selector(move || selector.clone())
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

/// The settings sidebar in the app sidebar's place.
pub fn sidebar(
    ctx: &Ctx,
    field: &Entity<InputState>,
    hits: Option<&[(GroupId, usize)]>,
    trail: &[(SharedString, usize, bool)],
    frame_row: bool,
    window_height: Pixels,
    cx: &mut App,
) -> impl IntoElement {
    let searching = hits.is_some();
    let p = cx.palette().clone();
    let mono = cx.theme().mono_font_family.clone();
    let open = ctx.at.group();
    let mut list = v_flex().w_full().gap_1().px(INSET).pt_1();
    for id in GroupId::ALL {
        let g = group(id);
        let group_at = At::Group(id);
        let is_open = open == id;
        let dimmed = hits.is_some_and(|h| h.iter().any(|(gid, n)| *gid == id && *n == 0));
        let lifted = !searching && *ctx.at == group_at;
        let ink = if is_open && !lifted { p.sidebar_foreground } else { p.sidebar_muted_foreground };
        let acts = ctx.acts.clone();
        let glyph =
            Icon::new(g.glyph).with_size(px(16.)).text_color(if lifted || is_open { p.sidebar_foreground } else { p.sidebar_icon_color });
        let mut item = v_flex().min_w_0().child(sidebar_row(
            SharedString::from(format!("settings-{}", id.id())),
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
                subs = subs.child(sidebar_row(
                    SharedString::from(format!("settings-row-{}", at.id())),
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
            item = item.child(subs);
        } else if !trail.is_empty() && !searching {
            let mut subs = v_flex().ml(SUB_INDENT).min_w_0();
            for (head, ix, in_view) in trail.iter().cloned() {
                let acts = ctx.acts.clone();
                subs = subs.child(sidebar_row(
                    SharedString::from(format!("settings-section-{ix}")),
                    None,
                    head,
                    None,
                    false,
                    if in_view { p.sidebar_foreground } else { p.sidebar_muted_foreground },
                    false,
                    &p,
                    mono.clone(),
                    move |_, cx| acts.reveal_section(ix, cx),
                ));
            }
            item = item.child(subs);
        }
        list = list.child(item);
    }
    let search = v_flex().px(INSET).pt_3().pb_1().child(
        h_flex()
            .h(px(28.))
            .gap_2()
            .px_2()
            .border_b_1()
            .border_color(p.sidebar_border)
            .child(Icon::new(Glyph::Search).with_size(px(14.)).text_color(p.sidebar_muted_foreground))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_size(px(13.))
                    .text_color(p.sidebar_foreground)
                    .child(Input::new(field).appearance(false).px_0().h(px(28.))),
            ),
    );
    let acts = ctx.acts.clone();
    let back = div().p(INSET).child(
        div().px_1().child(
            h_flex()
                .id("settings-back")
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
    v_flex()
        .debug_selector(|| "settings-sidebar".into())
        .w(SIDEBAR_WIDTH)
        .h_full()
        .flex_none()
        .bg(p.sidebar)
        .border_r_1()
        .border_color(p.sidebar_border)
        .text_color(p.sidebar_foreground)
        .relative()
        .child(crate::ui::grain(gpui_kit::size(SIDEBAR_WIDTH, window_height)))
        .when(frame_row, |el| el.child(header_row(&p)))
        .child(search)
        .child(div().id("settings-groups").flex_1().min_h_0().overflow_y_scroll().child(list))
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

/// The top bar over the page: the crumbs, and Restore defaults or a group's tabs at its right.
/// `bare` leaves out the row's own height and insets, for a frame whose header row holds it.
pub fn top_bar(ctx: &Ctx, searching: bool, bare: bool, window: &mut Window, cx: &mut App) -> impl IntoElement {
    let p = cx.palette().clone();
    let group_id = ctx.at.group();
    let g = group(group_id);
    let page_name = page_name(ctx).or_else(|| ctx.level.map(|l| l.name.clone()));
    let slash = || div().flex_none().text_color(p.muted_foreground.opacity(0.5)).child("/");
    let mut crumbs = h_flex().min_w_0().gap_2().text_size(text::SETTING_TITLE.0).line_height(text::SETTING_TITLE.1);
    let sub = !searching && page_name.is_some();
    if sub {
        let acts = ctx.acts.clone();
        let up = ctx.level.and_then(|l| l.up.as_deref().cloned());
        crumbs =
            crumbs.child(div().ml(px(-4.)).child(Button::icon_xs("settings-up", Glyph::ChevronLeft).on_click(move |_, window, cx| {
                match up.clone() {
                    Some(up) => acts.open_level(Some(up), cx),
                    None => acts.go(At::Group(group_id), window, cx),
                }
            })));
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
            .child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(p.foreground).truncate().child(name))
    } else {
        crumbs.child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(p.foreground).child(g.name))
    };
    let act: Option<AnyElement> = match ctx.at {
        At::Group(id) => pages::head(*id, ctx, window, cx).or_else(|| restore(*id, ctx)),
        _ => None,
    };
    h_flex()
        .debug_selector(|| "settings-top-bar".into())
        .when(!bare, |el| el.h(TOP_BAR).flex_none().pl(px(20.)).pr(px(20.)))
        .when(bare, |el| el.flex_1().min_w_0())
        .gap(px(16.))
        .child(crumbs)
        .child(div().flex_1())
        .children(act.map(|a| div().mr(px(1.)).child(a)))
}

/// Restore defaults, drawn only while a row of the open group is off its default.
fn restore(id: GroupId, ctx: &Ctx) -> Option<AnyElement> {
    let restore = group(id).restore?;
    let off = ctx.data.preferences().is_some_and(|p| (restore.off)(p));
    if !off {
        return None;
    }
    let acts = ctx.acts.clone();
    let patch = restore.patch;
    Some(
        Button::ghost("restore-defaults", "Restore defaults")
            .glyph(Glyph::RotateCcw)
            .xs()
            .on_click(move |_, _, cx| acts.set_preferences(patch(), cx))
            .into_any_element(),
    )
}

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

/// The centre: the page's cards in one column, scrolled as a whole.
pub fn page(body: Body, scroll: &ScrollHandle, width: Pixels, locked: bool, cx: &App) -> impl IntoElement {
    let p = cx.palette().clone();
    let center = width - SIDEBAR_WIDTH;
    let column_width = if locked { LOCKED_COLUMN } else { COLUMN };
    let side = (center - center.min(column_width)) / 2. + COLUMN_PAD;
    let mut column = v_flex()
        .id("settings-page")
        .debug_selector(|| "settings-page".into())
        .flex_1()
        .min_h_0()
        .overflow_y_scroll()
        .track_scroll(scroll)
        .px(side)
        .pt(if locked { px(56.) } else { px(28.) })
        .gap(if locked { px(48.) } else { size::PAGE_GAP });
    if let Some(lone) = body.lone {
        column = column
            .child(div().w_full().py_3().text_center().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(lone));
    }
    let gap = if locked { px(48.) } else { size::PAGE_GAP };
    // `pb-12` as a last child: a scroll box here does not count its own bottom padding in how far it scrolls.
    column.children(body.cards.into_iter().map(|card| card.render().locked(locked))).child(div().flex_none().h(px(48.) - gap))
}

/// A dialog over the screen, on the scrim, closed by a press outside it.
pub fn dialog_layer(dialog: AnyElement, screen: WeakEntity<SettingsScreen>, cx: &App) -> impl IntoElement {
    let p = cx.palette().clone();
    div()
        .id("settings-dialog-layer")
        .absolute()
        .inset_0()
        .flex()
        .items_center()
        .justify_center()
        // `dialog-backdrop`: the page at 60% (64% dark) under a 4 px blur.
        .bg(p.background.opacity(if cx.theme().is_dark() { 0.64 } else { 0.6 }))
        .backdrop_blur(px(4.))
        .on_click(move |_, _, cx| {
            if let Some(screen) = screen.upgrade() {
                screen.update(cx, |s, cx| s.close_dialog(cx));
            }
        })
        .child(div().id("settings-dialog").occlude().on_click(|_, _, cx| cx.stop_propagation()).child(dialog))
}
