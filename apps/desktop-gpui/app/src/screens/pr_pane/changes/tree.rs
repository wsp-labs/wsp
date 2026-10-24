// SPDX-License-Identifier: AGPL-3.0-only
//! The changed-files tree (`components/chat/ChangedFilesTree.tsx`, its card look): folders first with their chevron,
//! lone folders joined, each row's counts in green and red, and a file row's lead (the commit tick) and controls
//! beside its button. Rows are virtual, so 300 files draw only the rows in sight. The Changes pane draws it under its
//! fold; a turn card under a reply can draw the same with its own rows and no lead or controls.

use std::collections::HashSet;
use std::rc::Rc;

use gpui_kit::component::{h_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, uniform_list, AnyElement, App, ElementId, Pixels, SharedString, UniformListScrollHandle, Window};

use super::model::{compact, TreeFile, TreeRow};
use crate::screens::pr_pane::diff::fold_chevron;
use crate::ui::{motion, scrollbar, ActivePalette as _, Glyph};

/// A row's own height, and the `space-y-0.5` under it. Today's file rows measure 24.78 px and its folder rows about 2 px
/// less; one height for both, on the device grid, keeps every row within 3 device px of today's down a full tree.
pub const ROW: Pixels = px(24.5);
pub const ROW_GAP: Pixels = px(2.);
/// `max-h-56`: past it the rows scroll.
pub const MAX_HEIGHT: Pixels = px(224.);
/// `pb-1.5`, under the last row.
const FOOT: Pixels = px(6.);
/// `DiffStatLabel`'s aligned layout: two `4ch` columns `gap-2` apart, in 11 px mono.
const STAT_WIDTH: Pixels = px(2. * 4. * 6.6 + 8.);

pub type RowPiece = Rc<dyn Fn(&str, &mut Window, &mut App) -> Option<AnyElement>>;
pub type RowPress = Rc<dyn Fn(&str, &mut Window, &mut App)>;

/// The tree as drawn: its rows (see [`super::model::tree_rows`]) and what a press on each does.
#[derive(IntoElement)]
pub struct ChangedFilesTree {
    id: SharedString,
    rows: Rc<Vec<TreeRow>>,
    shut: Rc<HashSet<String>>,
    /// Whether any file sits in a folder: every file row then keeps the chevron's room before its glyph.
    nested: bool,
    width: Pixels,
    scroll: Option<UniformListScrollHandle>,
    on_folder: Option<RowPress>,
    on_file: Option<RowPress>,
    lead: Option<RowPiece>,
    controls: Option<RowPiece>,
    /// The controls' width, so each name gets a computed width rather than one shaped on every probe.
    controls_width: Pixels,
    /// The pane's tree stops at `max-h-56` over a foot; a turn's card draws every row.
    capped: bool,
}

impl ChangedFilesTree {
    /// `width` is the tree's own, inside its `px-1`.
    pub fn new(id: impl Into<SharedString>, rows: Rc<Vec<TreeRow>>, shut: Rc<HashSet<String>>, files: &[TreeFile], width: Pixels) -> Self {
        ChangedFilesTree {
            id: id.into(),
            rows,
            shut,
            nested: files.iter().any(|f| f.path.contains(['/', '\\'])),
            width,
            scroll: None,
            on_folder: None,
            on_file: None,
            lead: None,
            controls: None,
            controls_width: px(0.),
            capped: true,
        }
    }

    /// Every row at its height, with no cap and no foot under the last, as a turn's card draws its tree.
    pub fn uncapped(mut self) -> Self {
        self.capped = false;
        self
    }

    pub fn scroll(mut self, handle: UniformListScrollHandle) -> Self {
        self.scroll = Some(handle);
        self
    }

    pub fn on_folder(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_folder = Some(Rc::new(f));
        self
    }

    pub fn on_file(mut self, f: impl Fn(&str, &mut Window, &mut App) + 'static) -> Self {
        self.on_file = Some(Rc::new(f));
        self
    }

    pub fn lead(mut self, f: impl Fn(&str, &mut Window, &mut App) -> Option<AnyElement> + 'static) -> Self {
        self.lead = Some(Rc::new(f));
        self
    }

    pub fn controls(mut self, width: Pixels, f: impl Fn(&str, &mut Window, &mut App) -> Option<AnyElement> + 'static) -> Self {
        self.controls = Some(Rc::new(f));
        self.controls_width = width;
        self
    }

    /// The tree's height: every row, capped at `max-h-56`.
    pub fn height(rows: usize) -> Pixels {
        Self::content(rows).min(MAX_HEIGHT)
    }

    fn content(rows: usize) -> Pixels {
        (ROW + ROW_GAP) * rows as f32 - ROW_GAP + FOOT
    }
}

/// `DiffStatLabel` in its aligned layout and the diff tone: `+N` in green, `-N` in red, right-aligned in two columns.
pub fn stat(additions: usize, deletions: usize, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    let mono = cx.theme().mono_font_family.clone();
    let col = |text: String, ink| div().w(px(4. * 6.6)).flex_none().flex().justify_end().text_color(ink).child(text);
    h_flex()
        .debug_selector(|| "stat-aligned".into())
        .flex_none()
        .gap(px(8.))
        .font_family(mono)
        .text_size(px(11.))
        .child(col(format!("+{}", compact(additions)), p.success))
        .child(col(format!("-{}", compact(deletions)), p.error_foreground))
}

/// `DiffStatLabel` inline: `+N -N` with 4 px between, as the head and a folder's count draw it.
pub fn stat_inline(additions: usize, deletions: usize, size: Pixels, cx: &App) -> impl IntoElement {
    let p = cx.palette();
    h_flex()
        .flex_none()
        .gap(px(4.))
        .font_family(cx.theme().mono_font_family.clone())
        .text_size(size)
        .child(div().text_color(p.success).child(format!("+{}", compact(additions))))
        .child(div().text_color(p.error_foreground).child(format!("-{}", compact(deletions))))
}

impl RenderOnce for ChangedFilesTree {
    fn render(mut self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let count = self.rows.len();
        let height = ChangedFilesTree::height(count);
        // Past `max-h-56` the native bar takes its lane at the scroller's edge, outside the `px-1`.
        let lane = self.capped && ChangedFilesTree::content(count) > MAX_HEIGHT;
        if lane {
            self.width -= scrollbar::WIDTH;
        }
        let me = Rc::new(self);
        let list_me = me.clone();
        // Each row keeps its gap under it, the last one too, so the foot under the list is that much shorter.
        let rows = (ROW + ROW_GAP) * count as f32;
        let list = uniform_list(SharedString::from(format!("{}-rows", me.id)), count, move |range, window, cx| {
            range.map(|ix| list_me.row(ix, window, cx)).collect::<Vec<_>>()
        })
        .h(if me.capped { height } else { rows })
        .w_full()
        .pl(px(4.))
        .pr(px(4.) + if lane { scrollbar::WIDTH } else { px(0.) })
        .when(me.capped, |list| list.pb(FOOT - ROW_GAP));
        let (list, bar) = match me.scroll.clone() {
            Some(handle) => {
                let bar = lane.then(|| scrollbar::list_bar(format!("{}-bar", me.id), &handle, window, cx)).flatten();
                (list.track_scroll(&handle), bar)
            }
            None => (list, None),
        };
        if !me.capped {
            // The card draws every row with no gap under the last: the list keeps it and the frame cuts it.
            let frame = div().debug_selector(|| "changed-files-tree".into()).h(rows - ROW_GAP).overflow_hidden();
            return frame.child(list).into_any_element();
        }
        div().debug_selector(|| "changed-files-tree".into()).relative().child(list).children(bar).into_any_element()
    }
}

impl ChangedFilesTree {
    fn row(&self, ix: usize, window: &mut Window, cx: &mut App) -> AnyElement {
        let row = &self.rows[ix];
        let p = cx.palette().clone();
        let pad = px(8. + row.depth as f32 * 14.);
        let id = ElementId::from(SharedString::from(format!("{}-{}{}", self.id, if row.folder { "dir:" } else { "file:" }, row.path)));
        let hovered = motion::hovered(id.clone(), window, cx);
        let over = *hovered.read(cx);
        let fill = motion::ease_ink(
            id.clone(),
            "row-fill",
            if over { p.accent.opacity(0.6) } else { p.accent.opacity(0.) },
            motion::COLORS,
            window,
            cx,
        );
        let ink = if over { p.foreground.opacity(0.9) } else { p.muted_foreground };
        let mono = cx.theme().mono_font_family.clone();
        let path = row.path.clone();
        let name = |width: Pixels| {
            div()
                .w(width.max(px(0.)))
                .flex_none()
                .overflow_hidden()
                .text_ellipsis()
                .whitespace_nowrap()
                .font_family(mono.clone())
                .text_size(px(11.))
                .text_color(ink)
                .child(row.name.clone())
        };
        let frame = h_flex()
            .id(id.clone())
            .debug_selector({
                let tag = format!("tree-row:{}", row.path);
                move || tag.clone()
            })
            .h(ROW + ROW_GAP)
            .pb(ROW_GAP)
            .w_full()
            .on_hover(motion::track_hover(hovered));
        let inner = h_flex().h(ROW).w_full().rounded(px(14.)).bg(fill).pl(pad);
        if row.folder {
            let open = !self.shut.contains(&row.path);
            let counted = row.additions + row.deletions > 0;
            let fixed = pad + px(14. + 6. + 14. + 6. + 12.) + if counted { STAT_WIDTH + px(6.) } else { px(0.) };
            let on_folder = self.on_folder.clone();
            return frame
                .cursor_pointer()
                .on_click(move |_, window, cx| {
                    if let Some(f) = &on_folder {
                        f(&path, window, cx);
                    }
                })
                .child(
                    inner
                        .gap(px(6.))
                        .pr(px(12.))
                        .child(fold_chevron(id.clone(), open, window, cx))
                        .child(
                            Icon::new(if open { Glyph::Folder } else { Glyph::FolderClosed })
                                .with_size(px(14.))
                                .flex_none()
                                .text_color(p.muted_foreground.opacity(0.75)),
                        )
                        .child(name(self.width - fixed))
                        .when(counted, |el| el.child(div().ml_auto().child(stat(row.additions, row.deletions, cx)))),
                )
                .into_any_element();
        }
        let lead = self.lead.as_ref().and_then(|f| f(&row.path, window, cx));
        let controls = self.controls.as_ref().and_then(|f| f(&row.path, window, cx));
        let lead_width = if lead.is_some() { px(16. + 4.) } else { px(0.) };
        let spacer = if self.nested || row.depth > 0 { px(14. + 6.) } else { px(0.) };
        let controls_width = if controls.is_some() { px(4.) + self.controls_width } else { px(0.) };
        let fixed = pad + lead_width + spacer + px(14. + 6. + 6. + 12.) + STAT_WIDTH + controls_width;
        let on_file = self.on_file.clone();
        frame
            .child(
                inner
                    .gap(px(4.))
                    .children(lead)
                    .child(
                        h_flex()
                            .id(SharedString::from(format!("{}-open:{}", self.id, row.path)))
                            .flex_1()
                            .min_w_0()
                            .h_full()
                            .gap(px(6.))
                            .pr(px(12.))
                            .cursor_pointer()
                            .on_click(move |_, window, cx| {
                                if let Some(f) = &on_file {
                                    f(&path, window, cx);
                                }
                            })
                            .when(spacer > px(0.), |el| el.child(div().size(px(14.)).flex_none()))
                            .child(Icon::new(Glyph::File).with_size(px(14.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
                            .child(name(self.width - fixed))
                            .child(div().ml_auto().child(stat(row.additions, row.deletions, cx))),
                    )
                    .children(controls.map(|c| div().flex_none().pr(px(8.)).child(c))),
            )
            .into_any_element()
    }
}
