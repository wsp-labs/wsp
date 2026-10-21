// SPDX-License-Identifier: AGPL-3.0-only
//! One row of the list as `ThreadTile.tsx`, `SectionRow.tsx`, `SubagentRow.tsx` and `LeadTree.tsx` draw it, with the
//! tree's rail (`rowGrammar.ts`), its hover act, its context menu and its drag; and the project switcher.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, FontWeight, Hsla, MouseButton, Pixels, SharedString, WeakEntity, Window};

use super::age::{self, Count, Room};
use super::chrome::{self, ONE_LINE, ROW_RADIUS};
use super::model::{self, Item, Kind, Line, Section, Shape, Tile, Under};
use super::{looks, Sidebar, ARMED};
use crate::ui::catalog;
use crate::ui::{ActivePalette, Crab, Glyph, Palette, Status, StatusMark};

/// `TILE_CLASS`'s `h-[52px]`.
pub const TILE: Pixels = px(52.);
/// A child list stands 13 px in from its parent's content and each item 8 px past its line (`CHILD_LIST_CLASS`,
/// `RAIL_ITEM_CLASS`): each level is 21 px further in.
const LEVEL: f32 = 21.;
/// Where the elbow meets a row: a tile's title row, a one-line row's middle (`--mark-y`).
const MARK_TILE: f32 = 35.;
const MARK_SLIM: f32 = 18.;
/// `rounded-bl-sm` on the elbow.
const ELBOW_RADIUS: Pixels = px(6.);

/// A root tile carried by the pointer.
#[derive(Clone)]
pub struct DraggedTile {
    pub key: String,
    title: SharedString,
}

struct DragPreview {
    title: SharedString,
}

impl Render for DragPreview {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette();
        div()
            .px(px(8.))
            .py(px(4.))
            .rounded(ROW_RADIUS)
            .bg(p.popover)
            .border_1()
            .border_color(p.popover_edge)
            .text_size(px(14.))
            .text_color(p.foreground)
            .child(self.title.clone())
    }
}

/// A root that moves, carried by the pointer (`draggable` on the tile): the sidebar folds its tree once the drag starts
/// and stands as it was once the drag ends, dropped or not.
fn carried<E: StatefulInteractiveElement>(el: E, me: WeakEntity<Sidebar>, key: String, title: SharedString) -> E {
    el.on_drag(DraggedTile { key, title }, move |d, _, _, cx| {
        _ = me.update(cx, |this, cx| this.start_drag(&d.key, cx));
        let me = me.clone();
        cx.new(|cx| {
            cx.on_release(move |_, cx| _ = me.update(cx, |this, cx| this.end_drag(cx))).detach();
            DragPreview { title: d.title.clone() }
        })
    })
}

pub(crate) fn status_of(kind: Kind, text: Option<String>) -> Status {
    let text = || SharedString::from(text.clone().unwrap_or_default());
    match kind {
        Kind::NeedsYou => Status::NeedsYou,
        Kind::Waiting => Status::Waiting,
        Kind::Starting => Status::Starting,
        Kind::Working => Status::Working(text()),
        Kind::Limited => Status::Limited,
        Kind::Resuming => Status::Resuming,
        Kind::Failed => Status::Failed,
        Kind::Released => Status::Released,
        Kind::Expired => Status::Expired,
        Kind::Stopped => Status::Stopped,
        Kind::NotPushed => Status::NotPushed,
        Kind::Done => Status::Done,
        Kind::Resting => Status::Read(text()),
        Kind::Finished => Status::Finished,
    }
}

/// The agent's mark at 12 px in its own ink, or its initials where the catalog has no mark.
pub(crate) fn agent(harness: &str, cx: &App) -> AnyElement {
    if harness.is_empty() {
        return div().into_any_element();
    }
    catalog::mark(harness, &catalog::name(harness), px(12.), cx.palette().sidebar_muted_foreground, cx)
}

/// An inbox tile's glyph as `ThreadTile.tsx` draws it for a thread another started: the project's glyph with its
/// top right 6 by 4 px left out, and the corner-down-right mark over that corner.
fn started_by_glyph(glyph: impl Fn() -> AnyElement, ink: Hsla) -> AnyElement {
    let part = |left: f32, top: f32, w: f32, h: f32| {
        div()
            .absolute()
            .left(px(left))
            .top(px(top))
            .w(px(w))
            .h(px(h))
            .overflow_hidden()
            .child(div().absolute().left(px(-left)).top(px(-top)).size(px(12.)).child(glyph()))
    };
    div()
        .relative()
        .flex_none()
        .size(px(12.))
        .child(part(0., 4., 12., 8.))
        .child(part(0., 0., 6., 4.))
        .child(div().absolute().top(px(-5.)).right(px(-2.)).child(Icon::new(Glyph::CornerDownRight).with_size(px(8.)).text_color(ink)))
        .into_any_element()
}

/// The rails a row draws: the lines of the lists it stands inside that run on past it, and its own elbow and line.
fn rails(line: &Line, height: f32, p: &Palette) -> Vec<AnyElement> {
    let depth = line.depth as usize;
    if depth == 0 {
        return Vec::new();
    }
    let rail = p.sidebar_rail;
    let mark = if height > ONE_LINE.as_f32() { MARK_TILE } else { MARK_SLIM };
    let mut out: Vec<AnyElement> = (1..depth)
        .filter(|k| line.rails & (1 << k) != 0)
        .map(|k| div().absolute().left(px(LEVEL * k as f32)).top_0().bottom_0().w(px(1.)).bg(rail).into_any_element())
        .collect();
    let x = px(LEVEL * depth as f32);
    out.push(
        div()
            .absolute()
            .left(x)
            .top_0()
            .w(px(8.))
            .h(px(mark + 1.))
            .border_l_1()
            .border_b_1()
            .border_color(rail)
            .rounded_bl(ELBOW_RADIUS)
            .into_any_element(),
    );
    if !line.last {
        out.push(div().absolute().left(x).top(px(mark - 5.)).bottom_0().w(px(1.)).bg(rail).into_any_element());
    }
    out
}

/// A row's box in the list: 8 px in, each level 21 px further, the sections' gap over it where it opens one.
fn frame(ix: usize, line: &Line, height: f32, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let depth = line.depth as f32;
    div()
        .id(("line", ix))
        .relative()
        .w_full()
        .pl(px(8. + LEVEL * depth))
        .pr(px(8.))
        .when(line.gap, |el| el.pt(px(12.)))
        .children(rails(line, height, p))
}

/// The one-line row's grammar: 36 px, 10 px between, 8 px in, the rest ink, the row hover; a lit row keeps its fill under it.
fn one_line(id: SharedString, lit: bool, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (hover, active, ink) = (p.sidebar_row_hover, p.sidebar_row_active, p.sidebar_foreground);
    let (hover, active) = if lit { (p.sidebar_row_selected, p.sidebar_row_selected) } else { (hover, active) };
    h_flex()
        .id(id)
        .h(ONE_LINE)
        .w_full()
        .gap(px(10.))
        .px(px(8.))
        .rounded(ROW_RADIUS)
        .text_size(px(14.))
        .line_height(px(20.))
        .text_color(p.sidebar_row_rest)
        .cursor_pointer()
        .hover(move |s| s.bg(hover).text_color(ink))
        .active(move |s| s.bg(active))
}

/// The row-meta grammar: the mono at 11 px in the counts' whisper.
fn meta(text: String, p: &Palette, cx: &App) -> impl IntoElement {
    div().flex_none().font_family(cx.theme().mono_font_family.clone()).text_size(px(11.)).text_color(p.top_row_meta).child(text)
}

pub fn line(s: &mut Sidebar, ix: usize, line: &Line, window: &mut Window, cx: &mut Context<Sidebar>) -> AnyElement {
    let p = cx.palette().clone();
    let me = cx.weak_entity();
    match &line.item {
        Item::Head { section, count, folded } => {
            let word = match section {
                Section::Pinned => "Pinned",
                Section::NeedsYou => "Needs you",
                Section::Working => "Working",
                Section::Threads => "Threads",
                Section::Settled => "Settled",
            };
            let section = *section;
            frame(ix, line, 28., &p)
                .child(
                    chrome::section_head(SharedString::from(format!("head-{}", section.id())), word, *count, *folded, cx)
                        .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.toggle_section(section, cx))),
                )
                .into_any_element()
        }
        Item::Tile(tile) => tile_row(s, ix, line, tile, window, cx),
        Item::Group { attempt, title_of, copies, open, moves } => {
            let title = s.threads.get(*title_of).map(|t| t.title.clone()).unwrap_or_default();
            let attempt = attempt.clone();
            let icon = p.sidebar_icon_color;
            let head = one_line(SharedString::from(format!("group-{attempt}")), false, &p).debug_selector({
                let attempt = attempt.clone();
                move || format!("group-{attempt}")
            });
            let head =
                if *moves { carried(head, me.clone(), format!("attempt:{attempt}"), SharedString::from(title.clone())) } else { head };
            frame(ix, line, 36., &p)
                .child(
                    head.on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.toggle_group(&attempt, cx)))
                        .child(Icon::new(Glyph::Copy).with_size(px(16.)).text_color(icon))
                        .child(div().flex_1().min_w_0().truncate().child(title))
                        .child(meta(copies.to_string(), &p, cx))
                        .child(Icon::new(if *open { Glyph::ChevronDown } else { Glyph::ChevronRight }).with_size(px(16.)).text_color(icon)),
                )
                .into_any_element()
        }
        Item::Subagent { lead, index, part } => {
            let Some(sub) = s.threads.get(*lead).and_then(|t| t.subagents.get(*index)) else { return div().into_any_element() };
            let kind = model::subagent_kind(sub, *part);
            let since = match kind {
                Kind::Working => Some((Count::Running, sub.started_at)),
                Kind::Resting => Some((Count::Age, sub.ended_at.unwrap_or(sub.started_at))),
                _ => None,
            };
            let key = format!("sub-{}-{}", s.threads[*lead].key, sub.id);
            let mark = match since {
                Some((count, since)) => {
                    let time = age::age(format!("time-{key}:{count:?}:{since}"), count, super::now_ms() - since, Room::Fitted, window, cx);
                    let working = kind == Kind::Working;
                    let ink = if working { p.status_working } else { p.muted_foreground };
                    let crab = working.then(|| SharedString::from(format!("crab-{key}")));
                    age::timed_mark(&time, working, ink, crab, crate::ui::text::XS, window, cx)
                }
                None => StatusMark::new(status_of(kind, None))
                    .rest_ink(p.muted_foreground)
                    .key(SharedString::from(format!("crab-{key}")))
                    .into_any_element(),
            };
            // `SubagentRow`: lit while its page is open, and a press opens that page.
            let lit = sub_lit(s, *lead, sub);
            let page = sub.call.as_ref().map(|call| {
                let at =
                    crate::transcript::ThreadAt { workspace: s.threads[*lead].workspace_id.clone(), thread: s.threads[*lead].key.clone() };
                crate::screens::Route::Screen(crate::screens::thread_view::cards::subagent_route(&at, call).into())
            });
            frame(ix, line, 36., &p)
                .child(
                    one_line(SharedString::from(key.clone()), lit, &p)
                        .debug_selector({
                            let key = key.clone();
                            move || key.clone()
                        })
                        .gap(px(6.))
                        .when(lit, |el| el.bg(p.sidebar_row_selected).font_weight(gpui_kit::FontWeight::MEDIUM))
                        .when_some(page, |el, page| el.on_click(move |_, _, cx| crate::screens::navigate(page.clone(), cx)))
                        .child(Icon::new(Glyph::Bot).with_size(px(12.)).text_color(if lit {
                            p.sidebar_foreground
                        } else {
                            p.sidebar_icon_color
                        }))
                        .child(div().flex_1().min_w_0().truncate().text_color(p.sidebar_muted_foreground).child(sub.title.clone()))
                        .child(mark),
                )
                .into_any_element()
        }
        Item::Finished { lead, count, open } => {
            let lead = s.threads[*lead].key.clone();
            frame(ix, line, 36., &p)
                .child(
                    one_line(SharedString::from(format!("finished-{lead}")), false, &p)
                        .gap(px(6.))
                        .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.toggle_finished(&lead, cx)))
                        .child(Icon::new(Glyph::CircleCheck).with_size(px(12.)).text_color(p.sidebar_icon_color))
                        .child(div().flex_1().min_w_0().truncate().text_color(p.sidebar_muted_foreground).child("Finished"))
                        .child(meta(count.to_string(), &p, cx))
                        .child(
                            Icon::new(if *open { Glyph::ChevronDown } else { Glyph::ChevronRight })
                                .with_size(px(14.))
                                .text_color(p.sidebar_icon_color),
                        ),
                )
                .into_any_element()
        }
        Item::Machine { workspace } => {
            let card = s.workspace_card(workspace, cx);
            frame(ix, line, TILE.as_f32(), &p).child(card).into_any_element()
        }
        Item::More { n, lead } => {
            let lead = lead.map(|l| s.threads[l].key.clone());
            frame(ix, line, 36., &p)
                .child(
                    one_line(SharedString::from(format!("more-{ix}")), false, &p)
                        .gap(px(6.))
                        .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.more(lead.as_deref(), cx)))
                        .child(div().flex_1().min_w_0().truncate().text_color(p.sidebar_muted_foreground).child(format!("{n} more"))),
                )
                .into_any_element()
        }
    }
}

/// What a tile's slot counts where its text is a time, and from when: Working's run (from `now`, the list's build,
/// while its start has not landed), a resting row's age.
pub(super) fn slot_time(t: &model::Thread, tile: &Tile, kind: Kind, now: f64) -> Option<(Count, f64)> {
    if tile.snoozed_working.is_some() || (tile.shape == Shape::Full && t.setup_refusal.is_some() && kind == Kind::Resting) {
        return None;
    }
    match kind {
        Kind::Working => Some((Count::Running, t.started_at.unwrap_or(now))),
        Kind::Resting => Some((Count::Age, tile.age_at?)),
        _ => None,
    }
}

/// Whether the subagent `sub` of the thread at `lead` has its page open.
fn sub_lit(s: &Sidebar, lead: usize, sub: &model::Subagent) -> bool {
    s.selected.as_deref() == Some(s.threads[lead].key.as_str()) && sub.call.is_some() && s.opened == sub.call
}

/// Whether the thread at `thread` draws as the open one: a subagent's page marks no tile of its lead, its row being
/// the one lit wherever it is drawn, unless the person folded the lead (`useTileActive`).
pub(super) fn tile_lit(s: &Sidebar, thread: usize) -> bool {
    let t = &s.threads[thread];
    s.selected.as_deref() == Some(t.key.as_str()) && (s.opened.is_none() || t.folded_at.is_some())
}

/// The row drawn as the open one: `tile-<thread>` or `sub-<lead>-<subagent>`, as their selectors name them.
pub(super) fn lit_row(s: &Sidebar) -> Option<String> {
    s.lines.iter().find_map(|l| match &l.item {
        Item::Tile(t) if tile_lit(s, t.thread) => Some(format!("tile-{}", s.threads[t.thread].key)),
        Item::Subagent { lead, index, .. } => {
            let sub = s.threads[*lead].subagents.get(*index)?;
            sub_lit(s, *lead, sub).then(|| format!("sub-{}-{}", s.threads[*lead].key, sub.id))
        }
        _ => None,
    })
}

/// The two rows of a thread's tile, or its slim row in Settled or a Finished fold, with its act and its menu.
fn tile_row(s: &mut Sidebar, ix: usize, line: &Line, tile: &Tile, window: &mut Window, cx: &mut Context<Sidebar>) -> AnyElement {
    let p = cx.palette().clone();
    let me = cx.weak_entity();
    let t = &s.threads[tile.thread];
    let key = t.key.clone();
    let active = tile_lit(s, tile.thread);
    let kind = model::tile_kind(t, tile, s.now);
    let slot = model::tile_slot(t, tile, s.now);
    let slim = tile.shape != Shape::Full;
    let settled = tile.shape == Shape::Settled;
    let renaming = s.renaming.as_ref().filter(|r| r.key == key).map(|r| r.input.clone());
    let group = SharedString::from(format!("tile-{key}"));
    let works = s.tree_works(&key);
    let stops = works && !settled && renaming.is_none();
    // `settlesOnHover`: a live tile, a root's or a child's (`childActs`), whose tree nothing works or asks in.
    let quiet = !settled && !works && !s.tree_keys(&key).iter().any(|k| s.thread(k).is_some_and(|t| t.asks));
    // `group-focus-within`: the row the keys are on shows its act as a hovered one does.
    let focused = s.key_row == Some(ix) && s.focus.is_focused(window);
    let settles = !stops && quiet && renaming.is_none();
    let armed = s.armed.as_ref().is_some_and(|(k, at)| *k == key && at.elapsed() < ARMED);
    let height = if slim { 36. } else { TILE.as_f32() };
    let (hover, active_fill, ink) = (p.sidebar_row_hover, p.sidebar_row_active, p.sidebar_foreground);

    let title_ink = if slim || (!active && model::recedes(kind)) { p.sidebar_muted_foreground } else { p.sidebar_foreground };
    let title: AnyElement = match renaming {
        Some(input) => s.rename_field(&input, cx).flex_1().min_w_0().h(px(18.)).into_any_element(),
        None => {
            let me2 = me.clone();
            let key2 = key.clone();
            div()
                .id(SharedString::from(format!("title-{key}")))
                .flex_1()
                .min_w_0()
                .h(px(if slim { 20. } else { 18. }))
                .truncate()
                .text_size(px(14.))
                .line_height(px(if slim { 20. } else { 18. }))
                .text_color(title_ink)
                .when(active, |el| el.font_weight(FontWeight::MEDIUM))
                .on_click(move |e, window, cx| {
                    if e.click_count() == 2 {
                        // The tile's own press would take the keys back from the box that just opened.
                        cx.stop_propagation();
                        _ = me2.update(cx, |this, cx| this.open_rename(&key2, window, cx));
                    }
                })
                .child(t.title.clone())
                .into_any_element()
        }
    };
    let status: AnyElement = if let Some(n) = tile.snoozed_working {
        h_flex()
            .flex_none()
            .gap(px(4.))
            .font_features(crate::ui::text::tabular())
            .child(Icon::new(Glyph::AlarmClock).with_size(px(12.)))
            .child(format!("{n} working"))
            .into_any_element()
    } else if slot.glyph.is_none() && !slot.crab && slot.tone.is_none() && kind != Kind::Resting {
        div().flex_none().child(slot.text.clone().unwrap_or_default()).into_any_element()
    } else if let Some((count, since)) = slot_time(t, tile, kind, s.now) {
        let time =
            age::age(format!("time-{key}:{}:{count:?}:{since}", tile.inbox), count, super::now_ms() - since, Room::Fitted, window, cx);
        let working = kind == Kind::Working;
        let ink = if working { p.status_working } else { p.sidebar_muted_foreground };
        age::timed_mark(&time, working, ink, None, (px(if slim { 12. } else { 11. }), px(if slim { 16. } else { 14. })), window, cx)
    } else {
        StatusMark::new(status_of(kind, slot.text.clone()))
            .crab(false)
            .rest_ink(p.sidebar_muted_foreground)
            .text(px(if slim { 12. } else { 11. }), px(if slim { 16. } else { 14. }))
            .key(SharedString::from(format!("slot-crab-{key}")))
            .into_any_element()
    };
    let status = div()
        .flex_none()
        .flex()
        .when(stops || settles, |el| el.group_hover(group.clone(), |st| st.invisible()).when(focused, |el| el.invisible()))
        .child(status);

    let place = [t.project_name.clone().unwrap_or_default(), s.computer_name(&t.workspace_id).unwrap_or_default()]
        .into_iter()
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" @ ");
    let prefs = s.around.preferences.as_deref().unwrap_or(&super::NO_PREFERENCES);
    let glyph_ink = if slim { p.sidebar_icon_color } else { p.sidebar_muted_foreground };
    let project_glyph = || looks::project_glyph(t.project_id.as_deref(), prefs, &s.around.images, px(12.), glyph_ink, !slim || active, &p);
    let glyph = project_glyph();
    let started_by = tile.started_by.last().map(|&i| s.threads[i].title.clone());
    let body = if slim {
        h_flex()
            .h(ONE_LINE)
            .gap(px(10.))
            .px(px(8.))
            .text_size(px(14.))
            .line_height(px(20.))
            .child(div().flex_none().when(!active, |el| el.opacity(0.4)).child(glyph))
            .child(title)
            .child(div().flex_none().text_size(px(12.)).text_color(p.sidebar_muted_foreground).child(status))
    } else {
        let row_one = h_flex()
            .h(px(14.))
            .min_w_0()
            .gap(px(6.))
            .text_size(px(11.))
            .line_height(px(14.))
            .text_color(p.sidebar_muted_foreground)
            .child(match started_by {
                Some(_) => started_by_glyph(project_glyph, p.sidebar_muted_foreground),
                None => glyph,
            })
            .child({
                let said = started_by.unwrap_or(place);
                div().debug_selector(|| format!("tile-where:{said}")).flex_1().min_w_0().truncate().child(said)
            })
            .child(status);
        let fold = match tile.under {
            Under::None => None,
            Under::Open => Some(
                div()
                    .flex_none()
                    .opacity(0.)
                    .group_hover(group.clone(), |st| st.opacity(1.))
                    .child(Icon::new(Glyph::ChevronDown).with_size(px(14.)).text_color(p.sidebar_muted_foreground))
                    .into_any_element(),
            ),
            Under::Folded(n) => Some(
                h_flex()
                    .flex_none()
                    .gap(px(4.))
                    .child(meta(n.to_string(), &p, cx))
                    .child(Icon::new(Glyph::ChevronRight).with_size(px(14.)).text_color(p.sidebar_muted_foreground))
                    .into_any_element(),
            ),
        };
        let fold = fold.map(|f| {
            let (me, key) = (me.clone(), key.clone());
            div()
                .id(SharedString::from(format!("fold-{key}")))
                .flex_none()
                .on_click(move |_, _, cx| {
                    cx.stop_propagation();
                    _ = me.update(cx, |this, cx| this.fold_tree(&key, cx));
                })
                .child(f)
        });
        let row_two = h_flex()
            .h(px(18.))
            .min_w_0()
            .gap(px(6.))
            .child(agent(&t.harness, cx))
            .child(title)
            .when(kind == Kind::Working, |el| el.child(Crab::new(p.status_working).key(SharedString::from(format!("crab-{key}")))))
            .children(fold);
        v_flex().h(TILE).gap(px(4.)).p(px(8.)).child(row_one).child(row_two)
    };

    let me_click = me.clone();
    let key_click = key.clone();
    let mut button = div()
        .id(SharedString::from(format!("tile-{key}")))
        .debug_selector({
            let key = key.clone();
            move || format!("tile-{key}")
        })
        .relative()
        .w_full()
        .rounded(ROW_RADIUS)
        .cursor_pointer()
        // `data-[active=true]:bg-sidebar-row-selected` comes after `hover:` and `active:`: the open row keeps its fill.
        .when(active, |el| el.bg(p.sidebar_row_selected))
        .when(!active, |el| el.hover(move |st| st.bg(hover).text_color(ink)).active(move |st| st.bg(active_fill)))
        .on_click(move |_, window, cx| {
            _ = me_click.update(cx, |this, cx| {
                this.point_at(&key_click, window, cx);
                this.select(&key_click, cx);
            })
        })
        .on_mouse_down(MouseButton::Right, {
            let (me, key, inbox) = (me.clone(), key.clone(), tile.inbox);
            move |e, window, cx| _ = me.update(cx, |this, cx| this.open_menu(&key, inbox, e.position, window, cx))
        })
        .on_hover({
            let (me, hover_id) = (me.clone(), format!("t:{key}:{}", tile.inbox));
            move |hovered, _, cx| _ = me.update(cx, |this, cx| this.hover_tile(&hover_id, *hovered, cx))
        })
        .child(body)
        .when(active, |el| el.child(div().absolute().inset_0().rounded(ROW_RADIUS).border_1().border_color(p.sidebar_row_edge)));
    if tile.moves {
        button = carried(button, me.clone(), key.clone(), SharedString::from(t.title.clone()));
    }
    let act: Option<AnyElement> = if stops {
        let (me, key) = (me.clone(), key.clone());
        let el = h_flex()
            .id(SharedString::from(format!("stop-{key}")))
            .absolute()
            .right(px(4.))
            .top(px(if slim { 8. } else { 5. }))
            .h(px(20.))
            .min_w(px(20.))
            .justify_center()
            .rounded(px(10.))
            .cursor_pointer()
            .when(!armed && !focused, |el| el.opacity(0.).group_hover(group.clone(), |st| st.opacity(1.)))
            .hover(move |st| st.bg(p.accent))
            .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.press_stop(&key, cx)));
        Some(if armed {
            el.px(px(6.))
                .text_size(px(12.))
                .font_weight(FontWeight::MEDIUM)
                .text_color(p.error_foreground)
                .child(super::menu::act_words::STOP_ARMED)
                .into_any_element()
        } else {
            el.w(px(20.)).child(Icon::new(Glyph::Square).with_size(px(14.)).text_color(p.sidebar_muted_foreground)).into_any_element()
        })
    } else if settles {
        let (me, key) = (me.clone(), key.clone());
        Some(
            div()
                .id(SharedString::from(format!("settle-{key}")))
                .debug_selector(|| format!("settle-{}{key}", if tile.inbox { "inbox-" } else { "" }))
                .absolute()
                .right(px(4.))
                .top(px(if slim { 8. } else { 5. }))
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(10.))
                .cursor_pointer()
                .when(!focused, |el| el.opacity(0.).group_hover(group.clone(), |st| st.opacity(1.)))
                .hover(move |st| st.bg(p.accent))
                .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.settle(&key, cx)))
                .child(Icon::new(Glyph::Archive).with_size(px(14.)).text_color(p.sidebar_muted_foreground))
                .into_any_element(),
        )
    } else {
        None
    };
    let workspace = t.workspace_id.clone();
    s.want_checkout(&workspace, cx);
    let card_key = format!("t:{key}:{}", tile.inbox);
    let card = (crate::ui::hover_card::shows(cx.entity_id(), &card_key, cx) && s.renaming.is_none()).then(|| {
        // The card stands beside the tile's right edge.
        div()
            .absolute()
            .inset_0()
            .child(crate::ui::hover_card::opener(cx.entity_id(), card_key))
            .child(div().absolute().top_0().right_0().child(s.tile_card(tile, px(0.), window, cx)))
    });
    frame(ix, line, height, &p).child(div().group(group).relative().child(button).children(act).children(card)).into_any_element()
}
