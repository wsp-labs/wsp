// SPDX-License-Identifier: AGPL-3.0-only
//! The composer drawn: the box with its editor, pickers and actions, or the bar or the docked prompt in its place.

use gpui_kit::component::input::Textarea;
use gpui_kit::component::{h_flex, v_flex, Sizable as _};
use gpui_kit::{canvas, div, prelude::*, px, Bounds, Context, ExternalPaths, Focusable as _, KeyDownEvent, Pixels, Window};

use super::model::{self, Hold, Layout};
use super::{dock, menus, parts, wide, Bar, BoxWidth, Composer, COLUMN, LINE, TEXT};
use crate::ui::ActivePalette;

impl Render for Composer {
    /// The composer, and its height as drawn, which a parent that draws it cached lays it out at.
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.built += 1;
        let (drawn, laid, stale, me) = (self.drawn.clone(), self.laid.clone(), self.stale.clone(), cx.entity().downgrade());
        // Laid out cached at a height it no longer has, the composer asks to be drawn whole in the next frame.
        let measure = canvas(
            move |bounds: Bounds<Pixels>, _, cx| {
                let height = bounds.size.height;
                drawn.set(height);
                if laid.get().is_some_and(|at| at != height) {
                    stale.set(stale.get() + 1);
                    let me = me.clone();
                    cx.defer(move |cx| _ = me.update(cx, |_, cx| cx.notify()));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        div().relative().w_full().child(self.body(window, cx)).child(measure)
    }
}

impl Composer {
    /// The height the composer last drew at, none before its first draw.
    pub fn drawn_height(&self) -> Option<Pixels> {
        Some(self.drawn.get()).filter(|h| *h > px(0.))
    }

    /// Draws the composer cached at `height` from the next frame, or whole where none.
    pub fn lay_at(&self, height: Option<Pixels>) {
        self.laid.set(height);
    }

    /// How many times the composer was built; a frame that reuses its cached drawing builds nothing.
    pub fn built(&self) -> u64 {
        self.built
    }

    /// How many frames laid the composer out cached at a height it no longer had.
    pub fn stale_layouts(&self) -> u64 {
        self.stale.get()
    }

    fn body(&mut self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        if std::mem::take(&mut self.focus_search) {
            self.search.update(cx, |s, cx| s.set_value("", window, cx));
            self.search.focus_handle(cx).focus(window, cx);
        }
        if self.dragging && !cx.has_active_drag() {
            self.dragging = false;
        }
        self.narrow = crate::ui::narrow(window);
        let narrow = self.narrow;
        let p = cx.palette().clone();
        let layout = self.layout();
        let shut = self.hold.as_ref().is_some_and(Hold::shuts);
        let width = self.line_width.clone();
        let editor_width = width.get();
        // The slash hint stands while `/` opens a menu: the announced commands, `/btw` where it asks a side question
        // (`catalogFromHarness`), or the skills it lists under them.
        let slash_lists = !self.commands_offered().is_empty() || self.asides() || (self.ran.agent.is_some() && !self.skills.is_empty());
        let placeholder = model::placeholder(slash_lists, if editor_width > px(0.) { f32::from(editor_width) } else { 600. });
        if self.placeholder != placeholder {
            self.placeholder = placeholder;
            self.editor.update(cx, |e, cx| e.set_placeholder(placeholder, window, cx));
        }
        let this = cx.entity();
        let paste = this.downgrade();
        let editor = div()
            .id("composer-editor")
            .debug_selector(|| "composer-editor".into())
            .relative()
            .flex_1()
            .min_w_0()
            .text_size(TEXT)
            .line_height(LINE)
            .text_color(p.foreground)
            .when(layout == Layout::Full, |el| el.min_h(px(74.)).pb_1())
            .child(
                // gpui-kit pads the field 4 px a side and wraps its text 10 px short of its right edge for the caret;
                // the web's text runs edge to edge of its column. The field is w_full, so the margins go on a wrapper.
                div().ml(px(-4.)).mr(px(-14.)).child(
                    Textarea::new(&self.editor)
                        .appearance(false)
                        .xsmall()
                        .disabled(shut)
                        .text_size(TEXT)
                        .line_height(LINE)
                        .token(|ctx, _, cx| parts::chip(ctx, cx))
                        .on_paste(move |item, _, cx| {
                            let entries = item.entries().to_vec();
                            paste.update(cx, |this, cx| this.on_paste(entries, cx)).unwrap_or(false)
                        }),
                ),
            )
            .when(layout == Layout::Line, |el| {
                el.child(canvas(move |bounds: Bounds<Pixels>, _, _| width.set(bounds.size.width), |_, _, _, _| {}).absolute().size_full())
            });

        // A bar goes with what it says, as the drawer's row does.
        let gone = match self.bar {
            Some(Bar::Queue) => self.queue.is_empty(),
            Some(Bar::Tasks) => self.drawer.tasks.is_none(),
            Some(Bar::Usage) => self.drawer.limit.is_none(),
            Some(Bar::Capped) => self.drawer.slot.is_none(),
            None => false,
        };
        if gone {
            self.drop_bar(window, cx);
        }
        if let Some(dock) = self.dock.as_ref().filter(|d| !d.folded) {
            return dock::view(self, dock, cx);
        }
        if let Some(bar) = self.bar {
            if std::mem::take(&mut self.focus_bar) {
                self.bar_focus.focus(window, cx);
            }
            let shown = match bar {
                Bar::Queue => parts::queue_bar(self, cx),
                Bar::Tasks => parts::tasks_bar(self, cx),
                Bar::Usage => parts::usage_bar(self, cx),
                Bar::Capped => parts::cap_bar(self, cx),
            };
            return div()
                .id("composer")
                .key_context("Composer")
                .debug_selector(|| "composer".into())
                .track_focus(&self.bar_focus)
                .w_full()
                .map(|el| if narrow { el.px_3().pb_3() } else { el.px_4().pb_4() })
                .capture_action(cx.listener(Self::on_escape))
                .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                    if event.keystroke.key == "escape" && this.bar.is_some() {
                        this.fold_bar(window, cx);
                        cx.stop_propagation();
                    }
                }))
                .child(shown)
                .into_any_element();
        }
        let pickers = parts::pickers(self, layout, cx);
        let actions = parts::actions(self, shut, cx);
        let grid = match layout {
            Layout::Full => v_flex()
                .map(|el| if narrow { el.px_3().pt(px(14.)).pb_3() } else { el.px_4().pt_4().pb_4() })
                .gap_2()
                .child(editor)
                .child(h_flex().items_center().justify_between().gap_2().child(pickers).child(actions)),
            Layout::Line => h_flex()
                .items_center()
                .gap_1()
                .map(|el| if narrow { el.pl_4() } else { el.pl_5() })
                .pr_2()
                .py_2()
                .child(editor)
                .child(pickers)
                .child(actions),
            Layout::Tall => v_flex()
                .gap_1()
                .map(|el| if narrow { el.pl_4() } else { el.pl_5() })
                .pr_2()
                .pt(px(14.))
                .pb_2()
                .child(editor)
                .child(h_flex().items_center().justify_end().gap_1().child(pickers).child(actions)),
        };
        let body = v_flex().rounded(px(20.)).overflow_hidden().children(parts::files_row(self, cx)).child(grid);
        // The box's width is known only once it is laid out, so a draw that crossed into or out of `wide` asks for the next.
        let (box_width, was_wide, me) = (self.box_width.clone(), wide(self.box_width.get()), cx.entity().downgrade());
        let measure_box = canvas(
            move |bounds: Bounds<Pixels>, _, cx| {
                box_width.set(bounds.size.width);
                cx.set_global(BoxWidth(bounds.size.width));
                if wide(bounds.size.width) != was_wide {
                    cx.defer(move |cx| _ = me.update(cx, |_, cx| cx.notify()));
                }
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full();
        let shell = v_flex()
            .relative()
            .mx_auto()
            .w_full()
            .max_w(COLUMN)
            .children(self.banner.as_ref().map(|b| parts::banner(b, cx)))
            .children(parts::drawer(self, cx))
            .child(parts::glass_box(cx).child(body))
            .child(parts::tray(self, cx))
            .children(self.notice.clone().map(|n| parts::under(n, cx)))
            .children(menus::menu_layer(self, cx))
            .child(measure_box);
        div()
            .id("composer")
            .key_context("Composer")
            .debug_selector(|| "composer".into())
            .relative()
            .w_full()
            .map(|el| if narrow { el.px_3().pt(px(6.)).pb_4() } else { el.px_5().pt_2().pb_5() })
            .capture_action(cx.listener(Self::on_up))
            .capture_action(cx.listener(Self::on_down))
            .capture_action(cx.listener(Self::on_enter))
            .capture_action(cx.listener(Self::on_tab))
            .capture_action(cx.listener(Self::on_escape))
            .on_action(cx.listener(Self::on_stash))
            .capture_action(cx.listener(Self::on_pick_row))
            .on_drag_move::<ExternalPaths>(cx.listener(|this, _, _, cx| {
                if !this.dragging {
                    this.dragging = true;
                    cx.notify();
                }
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, _, cx| {
                this.dragging = false;
                if !this.hold.as_ref().is_some_and(Hold::shuts) {
                    this.attach_paths(paths.paths(), cx);
                }
            }))
            .child(shell)
            .when(self.dragging && !shut, |el| el.child(parts::drop_overlay(cx)))
            .into_any_element()
    }
}
