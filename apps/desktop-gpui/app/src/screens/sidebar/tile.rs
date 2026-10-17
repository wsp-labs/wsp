// SPDX-License-Identifier: AGPL-3.0-only
//! One row of the list as `ThreadTile.tsx`, `SectionRow.tsx`, `SubagentRow.tsx` and `LeadTree.tsx` draw it, with the
//! tree's rail (`rowGrammar.ts`), its hover act, its context menu and its drag; and the project switcher.

use gpui_kit::component::menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    anchored, deferred, div, prelude::*, px, svg, AnyElement, App, Context, FontWeight, Hsla, Pixels, Rgba, SharedString, WeakEntity,
    Window,
};

use super::chrome::{self, ONE_LINE, ROW_RADIUS};
use super::model::{self, Item, Kind, Line, Section, Shape, Tile, Under};
use super::{DropTarget, Sidebar, ARMED};
use crate::ui::icons::agent_mark;
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

fn status_of(kind: Kind, text: Option<String>) -> Status {
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
fn agent(harness: &str, cx: &App) -> AnyElement {
    match agent_mark(harness) {
        Some(mark) => {
            let hex = if cx.theme().is_dark() { mark.dark } else { mark.light };
            let ink: Hsla = Rgba::try_from(hex).map(Into::into).unwrap_or_else(|_| cx.palette().sidebar_muted_foreground);
            svg().path(mark.path).flex_none().size(px(12.)).text_color(ink).into_any_element()
        }
        None if harness.is_empty() => div().into_any_element(),
        None => div()
            .flex_none()
            .w(px(12.))
            .text_size(px(7.))
            .font_weight(FontWeight::SEMIBOLD)
            .child(SharedString::from(harness.chars().take(2).collect::<String>().to_uppercase()))
            .into_any_element(),
    }
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

/// The one-line row's grammar: 36 px, 10 px between, 8 px in, the rest ink, the row hover.
fn one_line(id: SharedString, p: &Palette) -> gpui_kit::Stateful<gpui_kit::Div> {
    let (hover, active, ink) = (p.sidebar_row_hover, p.sidebar_row_active, p.sidebar_foreground);
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
                Section::Threads => "Threads",
                Section::Settled => "Settled",
            };
            let section = *section;
            let me2 = me.clone();
            frame(ix, line, 28., &p)
                .child(
                    chrome::section_head(SharedString::from(format!("head-{}", section.id())), word, *count, *folded, cx)
                        .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.toggle_section(section, cx)))
                        .drag_over::<DraggedTile>(move |st, _, _, _| st.bg(p.sidebar_row_hover))
                        .on_drop(move |d: &DraggedTile, _, cx| {
                            _ = me2.update(cx, |this, cx| this.drop_on(&d.key, &DropTarget::Head(section), cx))
                        }),
                )
                .into_any_element()
        }
        Item::Tile(tile) => tile_row(s, ix, line, tile, window, cx),
        Item::Group { attempt, title_of, copies, open, .. } => {
            let title = s.threads.get(*title_of).map(|t| t.title.clone()).unwrap_or_default();
            let attempt = attempt.clone();
            let icon = p.sidebar_icon_color;
            frame(ix, line, 36., &p)
                .child(
                    one_line(SharedString::from(format!("group-{attempt}")), &p)
                        .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.toggle_group(&attempt, cx)))
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
            let text = match kind {
                Kind::Working => Some(model::running_time(s.now - sub.started_at)),
                Kind::Resting => Some(model::resting_age(sub.ended_at.unwrap_or(sub.started_at), s.now)),
                _ => None,
            };
            let key = format!("sub-{}-{}", s.threads[*lead].key, sub.id);
            frame(ix, line, 36., &p)
                .child(
                    one_line(SharedString::from(key.clone()), &p)
                        .gap(px(6.))
                        .child(Icon::new(Glyph::Bot).with_size(px(12.)).text_color(p.sidebar_icon_color))
                        .child(div().flex_1().min_w_0().truncate().text_color(p.sidebar_muted_foreground).child(sub.title.clone()))
                        .child(
                            StatusMark::new(status_of(kind, text))
                                .rest_ink(p.muted_foreground)
                                .key(SharedString::from(format!("crab-{key}"))),
                        ),
                )
                .into_any_element()
        }
        Item::Finished { lead, count, open } => {
            let lead = s.threads[*lead].key.clone();
            frame(ix, line, 36., &p)
                .child(
                    one_line(SharedString::from(format!("finished-{lead}")), &p)
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
        Item::More { n, lead } => {
            let lead = lead.map(|l| s.threads[l].key.clone());
            frame(ix, line, 36., &p)
                .child(
                    one_line(SharedString::from(format!("more-{ix}")), &p)
                        .gap(px(6.))
                        .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.more(lead.as_deref(), cx)))
                        .child(div().flex_1().min_w_0().truncate().text_color(p.sidebar_muted_foreground).child(format!("{n} more"))),
                )
                .into_any_element()
        }
    }
}

/// The two rows of a thread's tile, or its slim row in Settled or a Finished fold, with its act and its menu.
fn tile_row(s: &mut Sidebar, ix: usize, line: &Line, tile: &Tile, _: &mut Window, cx: &mut Context<Sidebar>) -> AnyElement {
    let p = cx.palette().clone();
    let me = cx.weak_entity();
    let t = &s.threads[tile.thread];
    let key = t.key.clone();
    let active = s.selected.as_deref() == Some(key.as_str());
    let kind = model::tile_kind(t, tile, s.now);
    let slot = model::tile_slot(t, tile, s.now);
    let slim = tile.shape != Shape::Full;
    let settled = tile.shape == Shape::Settled;
    let renaming = s.renaming.as_ref().filter(|r| r.key == key).map(|r| r.input.clone());
    let group = SharedString::from(format!("tile-{key}"));
    let works = s.tree_works(&key);
    let stops = works && !settled && renaming.is_none();
    let quiet_root = tile.moves && !works;
    let settles = !stops && quiet_root && renaming.is_none();
    let armed = s.armed.as_ref().is_some_and(|(k, at)| *k == key && at.elapsed() < ARMED);
    let height = if slim { 36. } else { TILE.as_f32() };
    let (hover, active_fill, ink) = (p.sidebar_row_hover, p.sidebar_row_active, p.sidebar_foreground);

    let title_ink = if slim || (!active && model::recedes(kind)) { p.sidebar_muted_foreground } else { p.sidebar_foreground };
    let title: AnyElement = match renaming {
        Some(input) => div()
            .flex_1()
            .min_w_0()
            .h(px(18.))
            .child(gpui_kit::component::input::Input::new(&input).appearance(false).text_size(px(14.)))
            .into_any_element(),
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
            .child(Icon::new(Glyph::AlarmClock).with_size(px(12.)))
            .child(format!("{n} working"))
            .into_any_element()
    } else if slot.glyph.is_none() && !slot.crab && slot.tone.is_none() && kind != Kind::Resting {
        div().flex_none().child(slot.text.clone().unwrap_or_default()).into_any_element()
    } else {
        StatusMark::new(status_of(kind, slot.text.clone()))
            .crab(false)
            .rest_ink(p.sidebar_muted_foreground)
            .text(px(if slim { 12. } else { 11. }), px(if slim { 16. } else { 14. }))
            .key(SharedString::from(format!("slot-crab-{key}")))
            .into_any_element()
    };
    let status = div().flex_none().flex().when(stops || settles, |el| el.group_hover(group.clone(), |st| st.invisible())).child(status);

    let place = [t.project_name.clone().unwrap_or_default(), t.computer.clone().unwrap_or_default()]
        .into_iter()
        .filter(|w| !w.is_empty())
        .collect::<Vec<_>>()
        .join(" @ ");
    let glyph =
        Icon::new(Glyph::Folder).with_size(px(12.)).text_color(if slim { p.sidebar_icon_color } else { p.sidebar_muted_foreground });
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
            .child(glyph)
            .child(div().flex_1().min_w_0().truncate().child(place))
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
    let menu = menu_for(s, tile, &key, me.clone());
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
        .when(active, |el| el.bg(p.sidebar_row_selected))
        .hover(move |st| st.bg(hover).text_color(ink))
        .active(move |st| st.bg(active_fill))
        .on_click(move |_, _, cx| _ = me_click.update(cx, |this, cx| this.select(&key_click, cx)))
        .child(body)
        .when(active, |el| el.child(div().absolute().inset_0().rounded(ROW_RADIUS).border_1().border_color(p.sidebar_row_edge)));
    if tile.moves {
        let (title, dragged) = (SharedString::from(t.title.clone()), key.clone());
        let (me_drop, target) = (me.clone(), DropTarget::Root(key.clone()));
        button = button
            .on_drag(DraggedTile { key: dragged, title }, |d, _, _, cx| cx.new(|_| DragPreview { title: d.title.clone() }))
            .drag_over::<DraggedTile>(move |st, _, _, _| st.bg(hover))
            .on_drop(move |d: &DraggedTile, _, cx| _ = me_drop.update(cx, |this, cx| this.drop_on(&d.key, &target, cx)));
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
            .when(!armed, |el| el.opacity(0.).group_hover(group.clone(), |st| st.opacity(1.)))
            .hover(move |st| st.bg(p.accent))
            .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.press_stop(&key, cx)));
        Some(if armed {
            el.px(px(6.)).text_size(px(12.)).font_weight(FontWeight::MEDIUM).text_color(p.error_foreground).child("Stop").into_any_element()
        } else {
            el.w(px(20.)).child(Icon::new(Glyph::Square).with_size(px(14.)).text_color(p.sidebar_muted_foreground)).into_any_element()
        })
    } else if settles {
        let (me, key) = (me.clone(), key.clone());
        Some(
            div()
                .id(SharedString::from(format!("settle-{key}")))
                .absolute()
                .right(px(4.))
                .top(px(if slim { 8. } else { 5. }))
                .size(px(20.))
                .flex()
                .items_center()
                .justify_center()
                .rounded(px(10.))
                .cursor_pointer()
                .opacity(0.)
                .group_hover(group.clone(), |st| st.opacity(1.))
                .hover(move |st| st.bg(p.accent))
                .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.settle(&key, cx)))
                .child(Icon::new(Glyph::Archive).with_size(px(14.)).text_color(p.sidebar_muted_foreground))
                .into_any_element(),
        )
    } else {
        None
    };
    frame(ix, line, height, &p).child(div().group(group).relative().child(button.context_menu(menu)).children(act)).into_any_element()
}

type MenuBuilder = Box<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;
type Act = Box<dyn Fn(&mut Sidebar, &mut Window, &mut Context<Sidebar>)>;

/// The tile's menu, in `threadActions.ts`'s order, each item only where it applies.
fn menu_for(
    s: &Sidebar,
    tile: &Tile,
    key: &str,
    me: WeakEntity<Sidebar>,
) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu {
    let t = &s.threads[tile.thread];
    let settled = tile.shape == Shape::Settled;
    let root = tile.moves || settled && t.parent.is_none();
    let works = s.tree_works(key);
    let pinned = t.pinned_at.is_some();
    let in_folder = s.in_folder(t);
    let moves = tile.moves;
    let key = key.to_owned();
    let builder: MenuBuilder = Box::new(move |menu, _, cx| {
        let place = me.upgrade().filter(|_| moves).and_then(|s| s.read(cx).root_place(&key));
        let act = |label: &'static str, f: Act| {
            let me = me.clone();
            PopupMenuItem::new(label).on_click(move |_, window, cx| _ = me.update(cx, |this, cx| f(this, window, cx)))
        };
        let k = || key.clone();
        let mut menu = menu.item(
            act(
                "Stop thread",
                Box::new({
                    let k = k();
                    move |this, _, cx| this.stop(&k, cx)
                }),
            )
            .disabled(!works),
        );
        if root && !settled {
            menu = menu.item(
                act(
                    "Settle thread",
                    Box::new({
                        let k = k();
                        move |this, _, cx| this.settle(&k, cx)
                    }),
                )
                .disabled(works),
            );
        }
        if root && settled {
            menu = menu.item(act(
                "Restore thread",
                Box::new({
                    let k = k();
                    move |this, _, cx| this.restore(&k, cx)
                }),
            ));
        }
        menu = menu.item(act(
            "Rename thread",
            Box::new({
                let k = k();
                move |this, window, cx| this.open_rename(&k, window, cx)
            }),
        ));
        if root && !settled {
            menu = menu.separator();
            let (word, to) = if pinned { ("Unpin thread", false) } else { ("Pin thread", true) };
            menu = menu.item(act(
                word,
                Box::new({
                    let k = k();
                    move |this, _, cx| this.mark_threads(vec![k.clone()], serde_json::json!({ "pinned": to }), cx)
                }),
            ));
        }
        if let Some(place) = place {
            menu = menu
                .separator()
                .item(
                    act(
                        "Move up",
                        Box::new({
                            let k = k();
                            move |this, _, cx| this.move_root(&k, model::Move::Up, cx)
                        }),
                    )
                    .disabled(place.index == 0),
                )
                .item(
                    act(
                        "Move down",
                        Box::new({
                            let k = k();
                            move |this, _, cx| this.move_root(&k, model::Move::Down, cx)
                        }),
                    )
                    .disabled(place.index + 1 >= place.count),
                )
                .item(
                    act(
                        "Move to top",
                        Box::new({
                            let k = k();
                            move |this, _, cx| this.move_root(&k, model::Move::Top, cx)
                        }),
                    )
                    .disabled(place.index == 0),
                );
        }
        menu = menu.separator();
        if in_folder {
            menu.item(act(
                "Delete thread",
                Box::new({
                    let k = k();
                    move |this, window, cx| this.confirm_delete(&k, window, cx)
                }),
            ))
        } else {
            menu.item(act(
                "Forget thread",
                Box::new({
                    let k = k();
                    move |this, _, cx| this.delete(&k, cx)
                }),
            ))
        }
    });
    builder
}

/// The project switcher's row and, while open, its list of projects under it (`NounSwitcher.tsx`).
pub fn project_switcher(
    picked: Option<(String, String)>,
    projects: Vec<(String, String)>,
    open: bool,
    me: WeakEntity<Sidebar>,
    cx: &mut Context<Sidebar>,
) -> AnyElement {
    let p = cx.palette().clone();
    let name = picked.as_ref().map_or_else(|| "All projects".to_owned(), |(_, name)| name.clone());
    let me2 = me.clone();
    let row = chrome::top_row("projects", Glyph::Folder, name, true, cx).on_click(move |_, _, cx| {
        _ = me2.update(cx, |this, cx| {
            this.switcher_open = !this.switcher_open;
            cx.notify();
        })
    });
    let list = open.then(|| {
        let pick = |id: Option<String>, word: String, on: bool| {
            let me = me.clone();
            h_flex()
                .id(SharedString::from(format!("pick-{}", id.clone().unwrap_or_default())))
                .h(ONE_LINE)
                .gap(px(10.))
                .px(px(8.))
                .rounded(ROW_RADIUS)
                .text_size(px(14.))
                .cursor_pointer()
                .hover(|st| st.bg(p.accent))
                .on_click(move |_, _, cx| {
                    _ = me.update(cx, |this, cx| {
                        this.prefs.project = id.clone();
                        this.switcher_open = false;
                        this.rebuild(cx);
                    })
                })
                .child(Icon::new(Glyph::Folder).with_size(px(16.)).text_color(p.sidebar_icon_color))
                .child(div().flex_1().min_w_0().truncate().child(word))
                .when(on, |el| el.bg(p.accent).child(Icon::new(Glyph::Check).with_size(px(16.))))
        };
        let (me3, me4) = (me.clone(), me.clone());
        deferred(
            anchored().child(
                v_flex()
                    .id("project-switcher")
                    .occlude()
                    .w(px(239.))
                    .mt(px(4.))
                    .p(px(4.))
                    .rounded(px(10.))
                    .bg(p.popover)
                    .border_1()
                    .border_color(p.popover_edge)
                    .shadow(p.popover_shadow.clone())
                    .text_color(p.foreground)
                    .on_mouse_down_out(move |_, _, cx| {
                        _ = me3.update(cx, |this, cx| {
                            this.switcher_open = false;
                            cx.notify();
                        })
                    })
                    .child(pick(None, "All projects".to_owned(), picked.is_none()))
                    .children(
                        projects
                            .iter()
                            .map(|(id, name)| pick(Some(id.clone()), name.clone(), picked.as_ref().is_some_and(|(p, _)| p == id))),
                    )
                    .child(
                        h_flex()
                            .id("add-project")
                            .mt(px(4.))
                            .pt(px(4.))
                            .border_t_1()
                            .border_color(p.border)
                            .child(
                                h_flex()
                                    .h(ONE_LINE)
                                    .w_full()
                                    .gap(px(10.))
                                    .px(px(8.))
                                    .rounded(ROW_RADIUS)
                                    .text_size(px(14.))
                                    .cursor_pointer()
                                    .hover(|st| st.bg(p.accent))
                                    .child(Icon::new(Glyph::Plus).with_size(px(16.)).text_color(p.muted_foreground))
                                    .child("Add a project"),
                            )
                            .on_click(move |_, _, cx| {
                                _ = me4.update(cx, |this, cx| {
                                    this.switcher_open = false;
                                    cx.emit(super::Asked::AddProject);
                                })
                            }),
                    ),
            ),
        )
    });
    div().relative().child(row).children(list).into_any_element()
}
