// SPDX-License-Identifier: AGPL-3.0-only
//! The Files tab (`pull-request/Files.tsx`): the count with the line counts in the diff inks, then a 30 px row per
//! file; a row opens that one file's diff under itself in the shared code view, the threads on its lines standing on
//! them, and its head hands the branch to the Changes pane. One file is open at a time.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Entity, Pixels, SharedString, Window};
use serde_json::json;

use super::parts;
use super::words::{self, PR_WORDS};
use super::{DiffRead, PullRequestScreen};
use crate::screens::pr_pane::changes::{file_title, head_counts};
use crate::screens::pr_pane::comments::{said_comment, ThreadComment};
use crate::screens::pr_pane::diff::{self, CodeView, FilePatch, LineNote, Side};
use crate::ui::{ActivePalette, Button, Glyph};

/// A file row's height (`look="pane"`).
const ROW: Pixels = px(30.);
/// The counts' column at a row's end.
const COUNTS: Pixels = px(72.);
/// The most an open file's diff stands before it scrolls inside its box.
const DIFF_MAX: Pixels = px(560.);

/// The section of a whole pull request's diff that is one file's, by its new path; only that section is copied.
pub fn file_section(patch: &str, path: &str) -> Option<String> {
    let starts: Vec<usize> =
        patch.match_indices("diff --git ").map(|(i, _)| i).filter(|&i| i == 0 || patch.as_bytes()[i - 1] == b'\n').collect();
    for (n, &start) in starts.iter().enumerate() {
        let end = starts.get(n + 1).copied().unwrap_or(patch.len());
        let section = &patch[start..end];
        let head = section.lines().next().unwrap_or_default();
        let new = head.trim_start_matches("diff --git ").rsplit_once(" b/").map(|(_, b)| b).unwrap_or_default();
        if new == path {
            return Some(if section.ends_with('\n') { section.to_owned() } else { format!("{section}\n") });
        }
    }
    None
}

/// One file of the pull request's diff as the code view takes it, none where the diff holds no section for it.
pub fn file_patch(patch: &str, path: &str) -> Option<FilePatch> {
    let section = file_section(patch, path)?;
    let reply = json!({ "files": [{ "path": path, "kind": "modified", "patch": section }], "truncated": false });
    diff::read_reply(reply.as_object()?).ok()?.files.into_iter().find(|f| f.drawn())
}

impl PullRequestScreen {
    /// The code view the open file draws in, made once and told each open file's patch and threads.
    fn code_view(&mut self, cx: &mut Context<Self>) -> Entity<CodeView> {
        if let Some(view) = &self.code {
            return view.clone();
        }
        let me = cx.weak_entity();
        let heads = me.clone();
        let view = cx.new(|cx| {
            let mut view = CodeView::new(cx);
            view.on_head(move |head, window, cx| {
                let p = cx.palette().clone();
                let me = heads.clone();
                let title =
                    div().min_w_0().truncate().font_family(cx.theme().font_family.clone()).text_size(px(13.)).child(head.path.clone());
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(Icon::new(Glyph::File).with_size(px(13.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
                    .child(file_title(&head.path, title, window, cx))
                    .child(div().flex_1())
                    .child(head_counts(head.additions, head.deletions, cx))
                    .child(div().debug_selector(|| "pr-open-changes".into()).child(
                        Button::ghost("pr-open-changes", PR_WORDS.open_in_changes).xs().on_click(move |_, _, cx| {
                            _ = me.update(cx, |this, cx| this.open_in_changes(cx));
                        }),
                    ))
                    .into_any_element()
            });
            view.on_fold(|_, _, _, _| {});
            // A thread under its line is its comments alone, as `LineCommentRow` draws them; its path and code are the line's.
            view.on_note(move |note, _, cx| {
                let Some(screen) = me.upgrade() else { return div().into_any_element() };
                let comments: Vec<ThreadComment> =
                    screen.read(cx).thread_box(note.id.as_ref()).map(|b| b.read(cx).thread().comments.clone()).unwrap_or_default();
                v_flex()
                    .debug_selector(|| "pr-line-thread".into())
                    .font_family(cx.theme().font_family.clone())
                    .children(comments.iter().map(|c| {
                        let parts = super::conversation::comment_parts(&me, c.id, cx);
                        said_comment(c, parts, cx)
                    }))
                    .into_any_element()
            });
            view
        });
        self.code = Some(view.clone());
        view
    }

    /// The code view the open file draws in, once a file opened.
    pub fn code_view_now(&self) -> Option<&Entity<CodeView>> {
        self.code.as_ref()
    }

    /// The branch in the Changes pane, as the open file's head offers it.
    pub fn open_in_changes(&mut self, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.opened.workspace_id.clone() else { return };
        let thread = self.opened.thread_id.clone();
        crate::screens::pr_pane::changes::open_on(
            &workspace_id,
            thread.as_deref(),
            crate::screens::pr_pane::changes::model::Scope::Branch,
            cx,
        );
    }

    /// Tells the code view the open file and the threads on its lines: when a file opens, the diff lands or the page
    /// moves, never on a frame.
    pub(super) fn sync_open_file(&mut self, cx: &mut Context<Self>) {
        self.open_view = None;
        let Some(path) = self.open_file.clone() else { return };
        let Some(DiffRead::Read { patch, .. }) = &self.diff else { return };
        let Some(file) = file_patch(patch, &path) else { return };
        let lines = file.patch.as_deref().map_or(0, |p| p.lines().count());
        let notes: Vec<LineNote> = self
            .derived
            .threads
            .iter()
            .filter(|t| t.path == path && t.line.is_some())
            .map(|t| LineNote {
                id: t.key.clone().into(),
                path: path.clone().into(),
                side: if t.comments[0].side.as_deref() == Some("LEFT") { Side::Original } else { Side::Modified },
                line: t.line.unwrap_or_default() as usize,
            })
            .collect();
        let count = notes.len();
        let view = self.code_view(cx);
        let fresh = self.shown_file.as_deref() != Some(path.as_str());
        view.update(cx, |v, cx| {
            if fresh {
                v.set_files(std::slice::from_ref(&file), cx);
            }
            v.set_notes(notes, cx);
        });
        self.shown_file = Some(path.clone());
        self.open_view = Some((path, lines + count * 7));
    }

    pub(super) fn files_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let Some(page) = &self.page else { return div().into_any_element() };
        if page.files.is_empty() {
            return div().text_size(px(13.)).text_color(p.muted_foreground).child(PR_WORDS.no_files).into_any_element();
        }
        let files = page.files.clone();
        let (adds, dels) = files.iter().fold((0, 0), |(a, d), f| (a + f.additions, d + f.deletions));
        let nested = files.iter().any(|f| f.path.contains('/') || f.path.contains('\\'));
        let me = cx.weak_entity();
        let fold = Button::icon_xs("pr-files-fold", if self.all_expanded { Glyph::ChevronsDownUp } else { Glyph::ChevronsUpDown })
            .tooltip(if self.all_expanded { PR_WORDS.collapse_folders } else { PR_WORDS.expand_folders })
            .on_click(move |_, _, cx| {
                _ = me.update(cx, |this, cx| {
                    this.all_expanded = !this.all_expanded;
                    this.dirs_open.clear();
                    cx.notify();
                });
            });
        let counts = |a: u64, d: u64, cx: &App| parts::stat_whole(a, d, px(12.), cx);
        let head = h_flex()
            .debug_selector(|| "pr-files-head".into())
            .min_h(px(24.))
            .justify_between()
            .gap_2()
            .text_size(px(13.))
            .child(
                h_flex()
                    .min_w_0()
                    .gap(px(6.))
                    .child(words::files(files.len()))
                    .when(adds + dels > 0, |el| el.child(counts(adds, dels, cx))),
            )
            .when(nested, |el| el.child(fold));
        let open = self.open_file.clone();
        let shown = self
            .open_view
            .clone()
            .filter(|(path, _)| open.as_deref() == Some(path.as_str()))
            .and_then(|(_, lines)| self.code.clone().map(|v| (v, lines)));
        let read = self.diff.clone();
        let width = self.width;
        // Each name is given its width, worked out from the row's fixed parts, rather than shaped on every probe.
        let dir_name = (width - px(12.) - px(14.) - px(14.) - COUNTS - px(24.)).max(px(40.));
        let file_name = |indent: Pixels| (width - px(12.) - indent - px(14.) - COUNTS - px(16.)).max(px(40.));
        let mut rows = v_flex().debug_selector(|| "pr-files".into());
        let mut dirs: Vec<String> = files.iter().filter_map(|f| f.path.rsplit_once('/').map(|(d, _)| d.to_owned())).collect();
        dirs.sort();
        dirs.dedup();
        let mut order: Vec<(Option<String>, Vec<usize>)> = dirs
            .into_iter()
            .map(|d| {
                let held = files
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.path.rsplit_once('/').map(|(x, _)| x) == Some(d.as_str()))
                    .map(|(i, _)| i)
                    .collect();
                (Some(d), held)
            })
            .collect();
        order.push((None, files.iter().enumerate().filter(|(_, f)| !f.path.contains('/')).map(|(i, _)| i).collect()));
        for (dir, held) in order {
            let indent = if dir.is_some() { px(16.) } else { px(0.) };
            if let Some(dir) = &dir {
                let open_dir = self.dirs_open.get(dir).copied().unwrap_or(self.all_expanded);
                let (a, d) = held.iter().fold((0, 0), |(a, d), i| (a + files[*i].additions, d + files[*i].deletions));
                let me = cx.weak_entity();
                let toggled = dir.clone();
                rows = rows.child(
                    h_flex()
                        .id(SharedString::from(format!("pr-dir-{dir}")))
                        .debug_selector(|| "pr-dir-row".into())
                        .h(ROW)
                        .w(width)
                        .flex_none()
                        .gap_2()
                        .px(px(6.))
                        .rounded(px(6.))
                        .cursor_pointer()
                        .hover(|s| s.bg(p.accent.opacity(0.6)))
                        .on_click(move |_, _, cx| {
                            let dir = toggled.clone();
                            _ = me.update(cx, |this, cx| {
                                let now = this.dirs_open.get(&dir).copied().unwrap_or(this.all_expanded);
                                this.dirs_open.insert(dir, !now);
                                cx.notify();
                            });
                        })
                        .child(
                            Icon::new(if open_dir { Glyph::ChevronDown } else { Glyph::ChevronRight })
                                .with_size(px(14.))
                                .flex_none()
                                .text_color(p.muted_foreground),
                        )
                        .child(Icon::new(Glyph::Folder).with_size(px(14.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
                        .child(div().w(dir_name).flex_none().truncate().text_size(px(13.)).text_color(p.foreground).child(dir.clone()))
                        .child(h_flex().w(COUNTS).flex_none().justify_end().child(counts(a, d, cx))),
                );
                if !open_dir {
                    continue;
                }
            }
            for ix in held {
                let f = &files[ix];
                let is_open = open.as_deref() == Some(f.path.as_str());
                let me = cx.weak_entity();
                let path = f.path.clone();
                let name = f.path.rsplit_once('/').map_or(f.path.clone(), |(_, n)| n.to_owned());
                let row = h_flex()
                    .id(SharedString::from(format!("pr-file-{ix}")))
                    .debug_selector(|| "pr-file-row".into())
                    .h(ROW)
                    .w(width)
                    .flex_none()
                    .gap_2()
                    .pl(px(6.) + indent)
                    .pr(px(6.))
                    .rounded(px(6.))
                    .cursor_pointer()
                    .when(is_open, |el| el.bg(p.accent.opacity(0.6)))
                    .hover(|s| s.bg(p.accent.opacity(0.6)))
                    .on_click(move |_, window, cx| {
                        let path = path.clone();
                        _ = me.update(cx, |this, cx| this.press_file(path, window, cx));
                    })
                    .child(Icon::new(Glyph::File).with_size(px(14.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
                    .child(div().w(file_name(indent)).flex_none().truncate().text_size(px(13.)).text_color(p.foreground).child(name))
                    .child(h_flex().w(COUNTS).flex_none().justify_end().child(counts(f.additions, f.deletions, cx)));
                rows = rows.child(row);
                if is_open {
                    let body: AnyElement = match (&read, &shown) {
                        (None | Some(DiffRead::Reading), _) => parts::diff_skeleton(width),
                        (Some(DiffRead::Refused(said)), _) => quiet(said.clone(), &p),
                        (Some(DiffRead::Read { .. }), Some((view, lines))) => {
                            let height = (px(20.) * (*lines as f32) + px(40.)).clamp(px(120.), DIFF_MAX);
                            div().h(height).child(view.clone()).into_any_element()
                        }
                        (Some(DiffRead::Read { left, .. }), None) => {
                            quiet(if left.contains(&f.path) { PR_WORDS.diff_cut.to_owned() } else { PR_WORDS.no_diff.to_owned() }, &p)
                        }
                    };
                    rows = rows.child(
                        div()
                            .debug_selector(|| "pr-file-diff".into())
                            .mt(px(6.))
                            .mb_1()
                            .rounded(px(8.))
                            .border_1()
                            .border_color(p.border)
                            .bg(p.card)
                            .overflow_hidden()
                            .child(body),
                    );
                }
            }
        }
        v_flex().debug_selector(|| "pr-files-tab".into()).gap_4().child(head).child(rows).into_any_element()
    }
}

fn quiet(words: String, p: &crate::ui::Palette) -> AnyElement {
    div().px_3().py(px(10.)).text_size(px(13.)).text_color(p.muted_foreground).child(words).into_any_element()
}

#[cfg(test)]
mod tests {
    use super::file_section;

    #[test]
    fn a_file_is_found_by_its_new_path_in_the_whole_diff() {
        let whole = "diff --git a/a.rs b/a.rs\n--- a/a.rs\n+++ b/a.rs\n@@ -1 +1 @@\n-a\n+b\ndiff --git a/src/b.rs b/src/b.rs\n--- a/src/b.rs\n+++ b/src/b.rs\n@@ -1 +1 @@\n-c\n+d\n";
        let b = file_section(whole, "src/b.rs").unwrap();
        assert!(b.starts_with("diff --git a/src/b.rs b/src/b.rs\n") && b.ends_with("+d\n"));
        assert!(file_section(whole, "a.rs").unwrap().ends_with("+b\n"));
        assert!(file_section(whole, "c.rs").is_none());
    }
}
