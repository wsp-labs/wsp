// SPDX-License-Identifier: AGPL-3.0-only
//! The Commits tab (`pull-request/Commits.tsx`): day heads, then one plain rail with a 6 px dot per commit, the merge
//! glyph in a merge's place with the branch it brought in as a chip and the row dimmed. The author's face stands in its
//! own column, then the subject, the line counts and the SHA; the time is on the row's hover, the body on its press.

use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, Context, Pixels, SharedString, Window};

use super::model;
use super::parts;
use super::words::PR_WORDS;
use super::PullRequestScreen;
use crate::ui::{tip, ActivePalette, Glyph};

/// The row's fixed columns: the rail, the face, the counts and the SHA, with their gaps and sides.
const RAIL: Pixels = px(28.);
const FACE: Pixels = px(18.);
const LINES: Pixels = px(84.);
const SHA: Pixels = px(56.);

impl PullRequestScreen {
    pub(super) fn commits_tab(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let Some(page) = &self.page else { return div().into_any_element() };
        if page.commits.is_empty() {
            return div().text_size(px(13.)).text_color(p.muted_foreground).child(PR_WORDS.no_commits).into_any_element();
        }
        let mono = cx.theme().mono_font_family.clone();
        let days = model::commit_days(&page.commits, model::now());
        let cut = page.cut.as_ref().is_some_and(|c| c.commits);
        let width = self.width;
        let subject_w = (width - RAIL - FACE - LINES - SHA - px(40.) - px(12.)).max(px(40.));
        let mut out = v_flex().debug_selector(|| "pr-commits".into()).gap_4();
        for (day, rows) in days {
            let mut list =
                v_flex().relative().gap(px(2.)).child(div().absolute().top(px(8.)).bottom(px(8.)).left(px(19.5)).w(px(1.)).bg(p.border));
            for row in rows {
                let open = self.commit_open.as_deref() == Some(row.oid.as_str());
                let me = cx.weak_entity();
                let (oid, has_body) = (row.oid.clone(), !row.body.is_empty());
                let dot = if row.merge {
                    div()
                        .rounded(px(3.))
                        .bg(p.background)
                        .child(Icon::new(Glyph::GitMerge).with_size(px(13.)).text_color(p.muted_foreground))
                        .into_any_element()
                } else {
                    div().size(px(6.)).rounded_full().bg(p.muted_foreground).into_any_element()
                };
                let subject = h_flex()
                    .w(subject_w)
                    .flex_none()
                    .min_w_0()
                    .gap_2()
                    .text_size(px(13.5))
                    .text_color(if row.merge { p.muted_foreground } else { p.foreground })
                    .children(row.from.clone().map(|from| {
                        div()
                            .debug_selector(|| "pr-commit-from".into())
                            .flex_none()
                            .px(px(5.))
                            .rounded(px(4.))
                            .border_1()
                            .border_color(p.border)
                            .font_family(mono.clone())
                            .text_size(px(11.))
                            .text_color(p.muted_foreground)
                            .child(from)
                    }))
                    .child(div().min_w_0().truncate().child(row.subject.clone()));
                let lines = h_flex()
                    .w(LINES)
                    .flex_none()
                    .justify_end()
                    .opacity(0.85)
                    .children(row.lines.map(|(a, d)| parts::stat_whole(a, d, px(12.), px(8.), cx)));
                let line = h_flex()
                    .id(SharedString::from(format!("pr-commit-{}", row.oid)))
                    .debug_selector(|| "pr-commit".into())
                    .min_h(px(36.))
                    .px(px(6.))
                    .gap(px(10.))
                    .rounded(px(7.))
                    .hover(|s| s.bg(p.foreground.opacity(0.04)))
                    .when(has_body, |el| el.cursor_pointer())
                    .tooltip(tip(model::ago(&row.at, model::now())))
                    .on_click(move |_, _, cx| {
                        let oid = oid.clone();
                        _ = me.update(cx, |this, cx| {
                            this.commit_open =
                                if !has_body || this.commit_open.as_deref() == Some(oid.as_str()) { None } else { Some(oid) };
                            cx.notify();
                        });
                    })
                    .child(h_flex().w(RAIL).h(px(36.)).flex_none().justify_center().child(dot))
                    .child(parts::face(&row.author, FACE, false, None, cx))
                    .child(subject)
                    .child(lines)
                    .child(
                        div()
                            .w(SHA)
                            .flex_none()
                            .text_right()
                            .font_family(mono.clone())
                            .text_size(px(12.))
                            .text_color(p.muted_foreground)
                            .child(row.sha.clone()),
                    );
                list = list.child(v_flex().child(line).when(open, |el| {
                    el.child(
                        div()
                            .debug_selector(|| "pr-commit-body".into())
                            .pt_1()
                            .pr(px(6.))
                            .pb_2()
                            .pl(px(74.))
                            .text_size(px(13.))
                            .line_height(px(20.))
                            .text_color(p.muted_foreground)
                            .child(row.body.clone()),
                    )
                }));
            }
            out = out.child(
                v_flex()
                    .debug_selector(|| "pr-day".into())
                    .gap_4()
                    .child(div().text_size(px(13.)).text_color(p.muted_foreground).child(day))
                    .child(list),
            );
        }
        if cut {
            out = out.child(parts::cut_note(PR_WORDS.cut.commits, cx));
        }
        out.into_any_element()
    }
}
