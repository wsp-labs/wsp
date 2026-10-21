// SPDX-License-Identifier: AGPL-3.0-only
//! A root tree dragged in the list as `treeDrag.ts` drags it: the tile dims and its tree folds, every section's head
//! stands, the place under the pointer is marked (an edge before or after a root, or a section's fill), and the drop
//! writes what today's writes there.

use std::cell::RefCell;
use std::rc::Rc;

use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, DragMoveEvent, Pixels, SharedString, Window};
use serde_json::Value;

use super::chrome::ROW_RADIUS;
use super::model::{self, Item, Placement, Section, Shape, ThreadSection};
use super::rows::Drawn;
use super::tile::{DraggedTile, TILE};
use super::{menu, now_ms, Sidebar};
use crate::ui::ActivePalette;

/// Where a drop lands (`DropPlace`): on a section's head, or on the edge before or after one root of Pinned or the list.
#[derive(Clone, Debug, PartialEq)]
pub enum DropPlace {
    Head(Section),
    Edge { section: Section, root: String, after: bool },
}

/// The place under the pointer, and the one each row under it last read for the next commit.
#[derive(Default)]
pub struct Drag {
    pub at: Option<DropPlace>,
    probe: Rc<RefCell<Option<DropPlace>>>,
}

/// A row's height as the list draws it, the sections' gap over it included.
fn line_px(line: &model::Line) -> Pixels {
    let own = match &line.item {
        Item::Head { .. } => 28.,
        Item::Tile(t) if t.shape == Shape::Full => TILE.as_f32(),
        Item::Machine { .. } => TILE.as_f32(),
        _ => 36.,
    };
    px(own + if line.gap { 12. } else { 0. })
}

impl Sidebar {
    /// The root the pointer carries, by its key.
    pub fn dragging(&self) -> Option<&str> {
        self.folds.dragging.as_deref()
    }

    /// The key a root line moves by: a thread's own, or a group head's `attempt:` key; none for a row that never moves.
    fn root_key(&self, line: &model::Line) -> Option<String> {
        match &line.item {
            Item::Tile(t) if t.moves => Some(self.threads[t.thread].key.clone()),
            Item::Group { attempt, moves: true, .. } => Some(format!("attempt:{attempt}")),
            _ => None,
        }
    }

    /// The drag begins: the tree folds and every head stands.
    pub(super) fn start_drag(&mut self, key: &str, cx: &mut Context<Self>) {
        self.folds.dragging = Some(key.to_owned());
        self.drag.at = None;
        self.rebuild(cx);
    }

    /// The drag ends, dropped or not: the tree and the heads stand as they did.
    pub(super) fn end_drag(&mut self, cx: &mut Context<Self>) {
        self.drag.at = None;
        if self.folds.dragging.take().is_some() {
            self.rebuild(cx);
        }
    }

    /// `placeAt`: the place a drop on drawn row `ix` lands, the pointer `y` px down the row; none where it changes nothing.
    fn place_at(&self, ix: usize, y: Pixels) -> Option<DropPlace> {
        let dragged = self.folds.dragging.as_deref()?;
        let Drawn::Line(li) = *self.drawn_rows.get(ix)? else { return None };
        let line = self.lines.get(li)?;
        let section = line.section;
        if !matches!(section, Section::Pinned | Section::Threads) || matches!(line.item, Item::Head { .. }) {
            return Some(DropPlace::Head(section));
        }
        let top = (0..=li).rev().find(|&k| self.lines[k].depth == 0)?;
        let Some(root) = self.root_key(&self.lines[top]) else { return Some(DropPlace::Head(section)) };
        if root == dragged {
            return None;
        }
        let gap = if line.gap { 12. } else { 0. };
        let after = top != li || y >= px(gap) + (line_px(line) - px(gap)) / 2.;
        // The edges on either side of the dragged tree are where it already stands.
        let roots: Vec<String> = self
            .lines
            .iter()
            .filter(|l| l.depth == 0 && l.section == section && matches!(l.item, Item::Tile(_) | Item::Group { .. }))
            .map(|l| match &l.item {
                Item::Tile(t) => self.threads[t.thread].key.clone(),
                Item::Group { attempt, .. } => format!("attempt:{attempt}"),
                _ => String::new(),
            })
            .collect();
        let at = roots.iter().position(|r| *r == root)?;
        let beside = if after { roots.get(at + 1) } else { at.checked_sub(1).and_then(|b| roots.get(b)) };
        if beside.is_some_and(|b| b == dragged) {
            return None;
        }
        Some(DropPlace::Edge { section, root, after })
    }

    /// Takes what the rows under the pointer read on this move, once every row has read it.
    pub(super) fn commit_place(&mut self, cx: &mut Context<Self>) {
        let at = self.drag.probe.borrow_mut().take();
        if self.folds.dragging.is_some() && at != self.drag.at {
            self.drag.at = at;
            cx.notify();
        }
    }

    /// A dragged root let go over the list, where the last move marked.
    pub(super) fn drop_tree(&mut self, dragged: &str, cx: &mut Context<Self>) {
        let at = self.drag.at.take();
        self.end_drag(cx);
        let Some(at) = at else { return };
        let view = self.view();
        let members: Vec<String> = match view
            .section(Section::Pinned)
            .iter()
            .chain(view.section(Section::Threads))
            .chain(view.section(Section::NeedsYou))
            .find(|n| n.key == dragged)
        {
            Some(node) if node.thread.is_none() => node.children.iter().map(|c| c.key.clone()).collect(),
            _ => vec![dragged.to_owned()],
        };
        let marks = match &at {
            DropPlace::Head(Section::Settled) => {
                if members.iter().any(|k| self.tree_works(k)) {
                    return self.refuse(menu::settle_refusal(true).unwrap_or_default(), cx);
                }
                return if members.len() == 1 && members[0] == dragged {
                    self.settle(dragged, cx)
                } else {
                    self.settle_ids(members, super::Processes::Ask, cx)
                };
            }
            DropPlace::Head(section) => view.drop_marks(dragged, *section).map(|m| vec![m]),
            DropPlace::Edge { section, root, after } => view.root_place(root).and_then(|place| {
                let from = view.root_place(dragged).filter(|f| f.section == *section && f.index < place.index);
                let at = place.index - usize::from(from.is_some()) + usize::from(*after);
                view.move_root(dragged, model::Move::To { section: *section, at })
            }),
        };
        let marks = marks.unwrap_or_default();
        if marks.is_empty() {
            return;
        }
        for m in marks {
            self.hold_marks(&m.thread_ids, &m.marks);
            self.mark_threads(m.thread_ids, m.marks, cx);
        }
        self.refold(cx);
    }

    /// Writes a move's marks on the rows at once, as the web's store holds them, until the host's rows carry them.
    pub(super) fn hold_marks(&mut self, ids: &[String], marks: &Value) {
        for row in self.rows.iter_mut().filter(|r| ids.iter().any(|id| *id == r.thread_id.as_deref().unwrap_or(&r.id))) {
            // A held row stands apart from the host's shared copy until the host's own row with the marks replaces it.
            let row = std::sync::Arc::make_mut(row);
            match marks.get("pinned") {
                Some(Value::Bool(true)) => row.pinned_at = Some(now_ms()),
                Some(Value::Bool(false)) => row.pinned_at = None,
                Some(n) if n.is_number() => row.pinned_at = n.as_f64(),
                _ => {}
            }
            if let Some(order) = marks.get("order").and_then(Value::as_f64) {
                row.order = Some(order);
            }
            match marks.get("section") {
                Some(Value::Null) => row.section = None,
                Some(p) => {
                    row.section = p.get("name").and_then(Value::as_str).and_then(ThreadSection::parse).map(|name| Placement {
                        name,
                        while_state: p.get("whileState").and_then(Value::as_str).unwrap_or_default().to_owned(),
                    })
                }
                None => {}
            }
        }
    }

    /// Drawn row `ix` while a tree is dragged: under it the section's fill where a drop on its head lands, over it the
    /// edge line where a drop beside its root lands; and it reads the place under the pointer as the pointer moves.
    pub(super) fn drag_row(&self, ix: usize, row: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let line = match self.drawn_rows.get(ix) {
            Some(Drawn::Line(li)) => self.lines.get(*li).map(|l| (*li, l)),
            _ => None,
        };
        let gap = px(if line.is_some_and(|(_, l)| l.gap) { 12. } else { 0. });
        let fill = line.and_then(|(li, l)| {
            (self.drag.at == Some(DropPlace::Head(l.section))).then(|| {
                let id = l.section.id();
                let first = li == 0 || self.lines[li - 1].section != l.section;
                let last = self.lines.get(li + 1).is_none_or(|n| n.section != l.section);
                div()
                    .debug_selector(move || format!("drop-over-{id}"))
                    .absolute()
                    .left(px(8.))
                    .right(px(8.))
                    .top(gap)
                    .bottom_0()
                    .bg(p.sidebar_row_hover)
                    .when(first, |el| el.rounded_t(ROW_RADIUS))
                    .when(last, |el| el.rounded_b(ROW_RADIUS))
            })
        });
        let edge = line.and_then(|(li, l)| {
            let Some(DropPlace::Edge { root, after, .. }) = &self.drag.at else { return None };
            let top = (0..=li).rev().find(|&k| self.lines[k].depth == 0)?;
            if self.root_key(&self.lines[top]).as_deref() != Some(root.as_str()) {
                return None;
            }
            let last = self.lines.get(li + 1).is_none_or(|n| n.depth == 0 || n.section != l.section);
            let here = if *after { last } else { top == li };
            let name = SharedString::from(format!("drop-edge-{root}-{}", if *after { "after" } else { "before" }));
            here.then(|| {
                let el = div().debug_selector(move || name.to_string()).absolute().left(px(8.)).right(px(8.)).h(px(2.)).bg(p.ring);
                if *after {
                    el.bottom(px(-1.))
                } else {
                    el.top(gap - px(1.))
                }
            })
        });
        // `data-[drag-source]`: the carried root's row at half opacity; its tree is not drawn.
        let source = line
            .and_then(|(_, l)| self.root_key(l))
            .filter(|k| line.is_some_and(|(_, l)| l.depth == 0) && self.folds.dragging.as_deref() == Some(k.as_str()));
        let probe = self.drag.probe.clone();
        let me = cx.weak_entity();
        div()
            .relative()
            .when_some(source, |el, key| el.opacity(0.5).debug_selector(move || format!("drag-source-{key}")))
            .children(fill)
            .child(row)
            .children(edge)
            .on_drag_move(move |e: &DragMoveEvent<DraggedTile>, _: &mut Window, cx: &mut App| {
                if e.bounds.contains(&e.event.position) {
                    let y = e.event.position.y - e.bounds.top();
                    *probe.borrow_mut() = me.upgrade().and_then(|s| s.read(cx).place_at(ix, y));
                }
            })
            .into_any_element()
    }
}
