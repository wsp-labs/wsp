// SPDX-License-Identifier: AGPL-3.0-only
//! The pane as drawn: its head, its notes, the changed-files fold and the body under them.

use std::rc::Rc;

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{canvas, div, prelude::*, px, AnyElement, App, BoxShadow, Context, Pixels, SharedString, Window};

use super::model::{self, base_name, Scope};
use super::tree::{self, ChangedFilesTree};
use super::{commit, hex, not_running, words, ChangesScreen, Load};
use crate::screens::computer::daemon_down::DaemonDown;
use crate::screens::pr_pane::diff::{self, Layout};
use crate::screens::pr_pane::files::crumbs::{CrumbRow, Pick};
use crate::ui::controls::checkbox;
use crate::ui::menu::menu_on;
use crate::ui::{spinner, tagged, tip, ActivePalette as _, Button, Glyph, MenuAct};

/// Tailwind's `sm`: under it the head keeps the scope and its acts and lets the folder, branch and view go.
const NARROW: Pixels = px(640.);
/// `RIGHT_PANEL_SHEET_MAX_VIEWPORT`: past it the panel stands inline and the head is 28 px.
const SHEET_MAX: Pixels = px(980.);

impl ChangesScreen {
    fn head(&mut self, inline: bool, narrow: bool, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let turn = self.turn().cloned();
        let scope = self.scope();
        let scope_label: SharedString = if turn.is_some() { words::TURN_SCOPE.into() } else { scope.label().into() };
        let me = cx.weak_entity();
        let acts: Vec<MenuAct> = Scope::ALL
            .iter()
            .map(|&s| {
                let me = me.clone();
                MenuAct {
                    group: 0,
                    glyph: None,
                    title: s.label().into(),
                    destructive: false,
                    held: false,
                    why: None,
                    keys: None,
                    run: Rc::new(move |_, cx| _ = me.update(cx, |this, cx| this.set_scope(s, cx))),
                }
            })
            .collect();
        let trigger = h_flex()
            .id("changes-scope")
            .debug_selector(|| "changes-scope".into())
            .h(px(24.))
            .flex_none()
            .gap(px(4.))
            .px(px(8.))
            .rounded(px(8.))
            .bg(p.accent)
            .hover(|s| s.bg(p.accent.opacity(0.8)))
            .cursor_pointer()
            .text_size(px(12.))
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(p.foreground)
            .child(scope_label)
            .child(Icon::new(Glyph::ChevronDown).with_size(px(14.)).text_color(p.foreground.opacity(0.7)));
        let scope_menu = menu_on("changes-scope", trigger, gpui_kit::Anchor::TopLeft, Rc::new(acts));
        let mono = cx.theme().mono_font_family.clone();
        let commenting = !self.comments.read(cx).kept().is_empty();
        let crumbs = (!narrow && !commenting).then(|| self.folder.clone()).flatten().map(|folder| {
            let me = cx.weak_entity();
            let pick: Pick = Rc::new(move |folder, _, cx| _ = me.update(cx, |this, cx| this.go(folder, cx)));
            let row = CrumbRow::new("changes-folder-crumbs", self.roots(cx), folder, pick);
            // `size-full flex-initial shrink-[999]`: a basis of the whole row, so the path takes what is left and the
            // branch gives the sliver that cuts it, as Chromium cuts "main" to "ma…".
            div().debug_selector(|| "changes-crumbs".into()).flex().w_full().h(px(24.)).min_w_0().flex_grow(0.).flex_shrink(999.).child(row)
        });
        let branch = (!narrow).then(|| self.repo.clone()).flatten().filter(|r| !r.branch.is_empty()).map(|repo| {
            h_flex()
                .id("changes-branch")
                .debug_selector(|| "changes-branch".into())
                .h(px(24.))
                .min_w_0()
                .gap(px(4.))
                .px(px(4.))
                .font_family(mono.clone())
                .text_size(px(11.))
                .text_color(p.muted_foreground)
                .tooltip(tip(format!("git: {}", repo.root)))
                .child(Icon::new(Glyph::FolderGit).with_size(px(14.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
                .child(div().min_w_0().max_w(px(160.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(repo.branch))
        });
        let pinned = self.pinned(cx);
        let pin = (!narrow && self.thread_id().is_some()).then(|| {
            div().debug_selector(|| "changes-pin".into()).flex_none().child(
                Button::icon_xs("changes-pin", if pinned { Glyph::PinOff } else { Glyph::Pin })
                    .side(px(20.))
                    .tooltip(if pinned { words::UNPIN } else { words::PIN })
                    .on_click(cx.listener(|this, _, _, cx| this.toggle_pin(cx))),
            )
        });
        let narrowed = turn.as_ref().filter(|t| t.only.is_some()).zip(self.diff.as_ref()).map(|(_, diff)| {
            h_flex()
                .flex_none()
                .gap(px(8.))
                .text_size(px(12.))
                .text_color(p.muted_foreground)
                .child(words::narrowed(diff.files.len(), self.whole))
                .child(Button::text("changes-show-all", words::SHOW_ALL).on_click(cx.listener(|this, _, _, cx| this.show_all(cx))))
        });
        let base =
            (turn.is_none() && scope == Scope::Branch).then(|| self.diff.as_ref().and_then(|d| d.base.clone())).flatten().map(|base| {
                h_flex()
                    .debug_selector(|| "changes-base".into())
                    .min_w_0()
                    .gap(px(8.))
                    .overflow_hidden()
                    .text_size(px(12.))
                    .text_color(p.muted_foreground)
                    .child(div().flex_none().child("HEAD"))
                    .child(Icon::new(Glyph::ArrowRight).with_size(px(14.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
                    .child(div().min_w_0().max_w(px(192.)).overflow_hidden().text_ellipsis().whitespace_nowrap().child(base))
            });
        let files = self.diff.as_ref().map_or(0, |d| d.files.len());
        let send = commenting.then(|| {
            div().debug_selector(|| "changes-send".into()).child(
                Button::ghost("changes-send", words::SEND_TO_THREAD).xs().on_click(cx.listener(|this, _, _, cx| this.send_comments(cx))),
            )
        });
        let stat = self.diff.as_ref().filter(|d| !d.files.is_empty()).map(|d| {
            div().debug_selector(|| "changes-stat".into()).mr(px(4.)).child(tree::stat_inline(d.additions(), d.deletions(), px(11.), cx))
        });
        let commit = (model::committable(scope, turn.is_some(), files) && !self.elsewhere()).then(|| {
            div().debug_selector(|| "changes-commit".into()).child(
                Button::outline("changes-commit", words::COMMIT)
                    .xs()
                    .held(self.commit.is_some())
                    .on_click(cx.listener(|this, _, window, cx| this.open_commit(window, cx))),
            )
        });
        let pending = self.load == Load::Pending;
        let refresh = Button::icon("changes-refresh", Glyph::RefreshCw)
            .busy(pending)
            .tooltip(if pending { words::REFRESHING } else { words::REFRESH })
            .on_click(cx.listener(|this, _, window, cx| this.fetch(window, cx)));
        let all_collapsed = self.diff.as_ref().is_some_and(|d| self.folds.all_collapsed(d.files.iter().map(|f| f.path.as_ref())));
        let fold_all = (!narrow && files > 0).then(|| {
            Button::icon("changes-fold-all", if all_collapsed { Glyph::ChevronsUpDown } else { Glyph::ChevronsDownUp })
                .tooltip(if all_collapsed { words::EXPAND_ALL } else { words::COLLAPSE_ALL })
                .on_click(cx.listener(|this, _, _, cx| this.toggle_all(cx)))
        });
        let layout = self.kept.layout;
        let view = (!narrow).then(|| {
            let toggle = |id: &'static str, glyph: Glyph, picked: Layout| {
                let fill = (layout == picked).then_some(p.accent);
                let drawn = fill.map_or_else(|| "none".to_owned(), hex);
                div()
                    .id(id)
                    .debug_selector(move || format!("{id}:{drawn}"))
                    .flex()
                    .flex_none()
                    .items_center()
                    .justify_center()
                    .size(px(28.))
                    .rounded(px(8.))
                    .border_1()
                    .border_color(gpui_kit::transparent_black())
                    .when_some(fill, |el, fill| el.bg(fill))
                    .hover(|s| s.bg(p.accent))
                    .cursor_pointer()
                    .tab_index(0)
                    .focus_visible(|s| s.shadow(vec![crate::ui::focus::ring(p.ring)]))
                    .on_click(cx.listener(move |this, _, _, cx| this.set_layout(picked, cx)))
                    .child(Icon::new(glyph).with_size(px(14.)).text_color(p.foreground.opacity(0.8)))
            };
            h_flex().flex_none().gap(px(4.)).child(toggle("changes-stacked", Glyph::Rows3, Layout::Stacked)).child(toggle(
                "changes-split",
                Glyph::Columns2,
                Layout::Split,
            ))
        });
        let _ = window;
        h_flex()
            .debug_selector(|| "changes-head".into())
            .flex_none()
            .h(if inline { px(28.) } else { px(40.) })
            .when(inline, |el| el.mb(px(12.)))
            .when(!inline, |el| el.border_b_1().border_color(p.border.opacity(0.6)))
            .justify_between()
            .gap(px(8.))
            .px(px(12.))
            .bg(p.background)
            .child(
                h_flex()
                    .min_w_0()
                    .flex_1()
                    .gap(px(8.))
                    .child(scope_menu)
                    .children(crumbs)
                    .children(branch)
                    .children(pin)
                    .children(narrowed)
                    .children(base),
            )
            .child(
                h_flex()
                    .flex_none()
                    .gap(px(4.))
                    .children(send)
                    .children(stat)
                    .children(commit)
                    .child(refresh)
                    .children(fold_all)
                    .children(view),
            )
    }

    fn note(
        text: impl Into<SharedString>,
        tag: &'static str,
        ink: gpui_kit::Hsla,
        fill: Option<gpui_kit::Hsla>,
        mono: bool,
        cx: &App,
    ) -> impl IntoElement {
        let p = cx.palette();
        div()
            .debug_selector(move || tag.into())
            .flex_none()
            .px(px(12.))
            .py(px(6.))
            .border_b_1()
            .border_color(p.border.opacity(0.7))
            .when_some(fill, |el, fill| el.bg(fill))
            .when(mono, |el| el.font_family(cx.theme().mono_font_family.clone()))
            .text_size(px(11.))
            // `text-[11px]` sets no leading, so the page's 1.5 holds.
            .line_height(gpui_kit::relative(1.5))
            .text_color(ink)
            .child(text.into())
    }

    fn pane_line(text: impl Into<SharedString>, cx: &App) -> impl IntoElement {
        div()
            .debug_selector(|| "changes-line".into())
            .flex()
            .flex_1()
            .items_center()
            .justify_center()
            .px(px(20.))
            .text_center()
            .text_size(px(13.))
            .text_color(cx.palette().muted_foreground)
            .child(text.into())
    }

    fn changed_files(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let files = self.tree.files.clone();
        let count = files.len();
        let any_blob = self.tree.any_blob;
        let open = self.tree_open;
        let me = cx.weak_entity();
        let toggle = h_flex()
            .id("changed-files-fold")
            .debug_selector(|| "changed-files-fold".into())
            .w_full()
            .gap(px(6.))
            .px(px(12.))
            .py(px(6.))
            .cursor_pointer()
            .hover(|s| s.bg(p.accent.opacity(0.6)))
            .text_size(px(12.))
            .line_height(px(16.))
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(p.foreground)
            .on_click(cx.listener(|this, _, _, cx| this.toggle_tree(cx)))
            .child(diff::fold_chevron("changed-files-chevron", open, window, cx))
            .child(div().child(words::changed_files(count)))
            .when(any_blob, |el| {
                el.child(
                    div()
                        .debug_selector(|| "viewed-count".into())
                        .ml(px(12.))
                        .font_weight(gpui_kit::FontWeight::NORMAL)
                        .text_color(p.muted_foreground)
                        .child(words::viewed_count(self.viewed_count(), count)),
                )
            });
        let tree = open.then(|| {
            let rows = self.tree.rows.clone();
            let scope = self.scope();
            let can_discard = model::discardable(scope, self.is_turn()) && !self.elsewhere();
            let committing = self.commit.as_ref().map(|c| c.ticked.clone());
            let viewed = self.tree.viewed.clone();
            let any_tick = any_blob;
            let controls_width = px(if can_discard { 24. } else { 0. } + if any_tick { 20. } else { 0. } + 8.);
            let (on_folder, on_file, lead_me, control_me) = (me.clone(), me.clone(), me.clone(), me.clone());
            let width = self.width - px(8.);
            ChangedFilesTree::new("changes-tree", rows, self.tree.shut.clone(), &files, width)
                .scroll(self.tree_scroll.clone())
                .on_folder(move |path, _, cx| _ = on_folder.update(cx, |this, cx| this.toggle_folder(path, cx)))
                .on_file(move |path, _, cx| {
                    _ = on_file.update(cx, |this, cx| {
                        this.reveal(path, cx);
                    })
                })
                .when_some(committing, |tree, ticked| {
                    tree.lead(move |path, window, cx| {
                        let me = lead_me.clone();
                        let path_owned = path.to_owned();
                        Some(
                            div()
                                .id(SharedString::from(format!("commit-tick:{path}")))
                                .debug_selector({
                                    let tag = format!("commit-tick:{path}");
                                    move || tag.clone()
                                })
                                .flex_none()
                                .cursor_pointer()
                                .tooltip(tip(words::commit_tick(base_name(path))))
                                .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.tick(&path_owned, cx)))
                                .child(checkbox(SharedString::from(format!("commit-box:{path}")), ticked.contains(path), false, window, cx))
                                .into_any_element(),
                        )
                    })
                })
                .controls(controls_width, move |path, window, cx| {
                    let name = base_name(path).to_owned();
                    let tick = viewed.get(path).copied().flatten();
                    let mut row = h_flex().gap(px(4.));
                    if can_discard {
                        let me = control_me.clone();
                        let path = path.to_owned();
                        row = row.child(tagged(
                            format!("tree-discard:{path}"),
                            Button::icon_xs(SharedString::from(format!("tree-discard:{path}")), Glyph::Trash)
                                .side(px(20.))
                                .tooltip(words::discard_control(&name))
                                .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.ask_discard(&path, window, cx))),
                        ));
                    }
                    if let Some(viewed) = tick {
                        let me = control_me.clone();
                        let path = path.to_owned();
                        let mark = checkbox(SharedString::from(format!("tree-viewed-box:{path}")), viewed, false, window, cx);
                        row = row.child(
                            div()
                                .id(SharedString::from(format!("tree-viewed:{path}")))
                                .ml(px(4.))
                                .cursor_pointer()
                                .tooltip(tip(words::viewed_tick(&name)))
                                .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.toggle_viewed(&path, cx)))
                                .child(mark),
                        );
                    }
                    Some(row.into_any_element())
                })
        });
        v_flex()
            .debug_selector(|| "changed-files".into())
            .flex_none()
            .border_b_1()
            .border_color(p.border.opacity(0.6))
            .child(toggle)
            .children(tree)
    }
}

impl Render for ChangesScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let viewport = window.viewport_size().width;
        let (narrow, inline) = (viewport < NARROW, viewport > SHEET_MAX);
        let me = cx.weak_entity();
        let measure = canvas(
            move |bounds, _, cx| {
                _ = me.update(cx, |this, cx| {
                    if this.width != bounds.size.width {
                        this.width = bounds.size.width;
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .size_full();
        let frame = v_flex()
            .id("changes")
            .debug_selector(|| "changes".into())
            .key_context("ChangesPane")
            .on_key_down(cx.listener(Self::up_key))
            .track_focus(&self.focus)
            .relative()
            .size_full()
            .min_w_0()
            .bg(p.background)
            .focus_visible(|s| {
                s.shadow(vec![BoxShadow {
                    color: p.border,
                    offset: gpui_kit::point(px(0.), px(0.)),
                    blur_radius: px(0.),
                    spread_radius: px(1.),
                    inset: true,
                }])
            })
            .child(measure);
        if self.opened.workspace_id.is_none() {
            return frame.child(ChangesScreen::pane_line(words::PICK_A_PROJECT, cx)).into_any_element();
        }
        if let Some(form) = self.not_running_form() {
            let (host, workspace, restart) =
                (self.host.clone(), self.opened.workspace_id.clone().unwrap_or_default(), self.restart.clone());
            let down = move |said: &str, start: &str| {
                DaemonDown::new(host, workspace, said.to_owned(), Some(start.to_owned().into())).state(restart).into_any_element()
            };
            return frame.child(not_running::view(&form, down, window, cx)).into_any_element();
        }
        let head = self.head(inline, narrow, window, cx);
        let me = cx.weak_entity();
        let commit = self.commit.as_ref().map(|state| {
            let (go, stop) = (me.clone(), me.clone());
            commit::commit_box(
                state,
                self.facts.as_ref().is_some_and(|f| f.working),
                move |window, cx| _ = go.update(cx, |this, cx| this.make_commit(window, cx)),
                move |_, cx| _ = stop.update(cx, |this, cx| this.cancel_commit(cx)),
                cx,
            )
        });
        let mut notes: Vec<AnyElement> = Vec::new();
        if self.diff.as_ref().is_some_and(|d| d.cut) {
            notes.push(
                ChangesScreen::note(diff::CUT_NOTE, "changes-cut", p.muted_foreground, Some(p.muted.opacity(0.4)), false, cx)
                    .into_any_element(),
            );
        }
        if let Some(note) = &self.write_note {
            notes.push(ChangesScreen::note(note.clone(), "changes-write-note", p.muted_foreground, None, false, cx).into_any_element());
        }
        if let Load::Error { message, line: None } = &self.load {
            notes.push(ChangesScreen::note(message.clone(), "changes-error", p.error_foreground, None, false, cx).into_any_element());
        }
        if let Some(note) = &self.reveal_note {
            notes.push(ChangesScreen::note(note.clone(), "changes-reveal-note", p.muted_foreground, None, true, cx).into_any_element());
        }
        let body: AnyElement = match (&self.diff, &self.load) {
            (None, Load::Pending | Load::Idle) => div()
                .debug_selector(|| "changes-loading".into())
                .flex()
                .flex_1()
                .items_center()
                .justify_center()
                .child(spinner(cx))
                .into_any_element(),
            (None, Load::Error { line: Some(line), .. }) => ChangesScreen::pane_line(*line, cx).into_any_element(),
            (None, _) => div().into_any_element(),
            (Some(diff), _) if diff.raw.is_some() => diff::raw_patch(diff.raw.as_ref().expect("raw"), cx).into_any_element(),
            (Some(diff), _) if diff.files.is_empty() => {
                let folder = self.repo.as_ref().map(|r| r.root.clone()).or_else(|| self.folder.clone()).unwrap_or_default();
                ChangesScreen::pane_line(words::empty(self.scope().noun(), &folder), cx).into_any_element()
            }
            (Some(diff), _) => {
                let all_cut = diff.all_cut();
                v_flex()
                    .flex_1()
                    .min_h_0()
                    .child(self.changed_files(window, cx))
                    .child(if all_cut {
                        ChangesScreen::pane_line(diff::ALL_CUT, cx).into_any_element()
                    } else {
                        div().flex_1().min_h_0().child(self.code.clone()).into_any_element()
                    })
                    .into_any_element()
            }
        };
        frame
            .child(head)
            .children(commit)
            .child(v_flex().flex_1().min_h_0().min_w_0().overflow_hidden().bg(p.background).children(notes).child(body))
            .into_any_element()
    }
}
