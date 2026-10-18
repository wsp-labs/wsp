// SPDX-License-Identifier: AGPL-3.0-only
//! The Conversation tab (`Conversation.tsx`, `ThreadActs.tsx`): Status, the description under its clamp, then
//! Activity on a hairline rail with a 24 px face on each entry: comments, a bot's notice folded to one line, reviews
//! with their verdict and the threads they left, a thread no review holds, and one quiet line where commits were
//! pushed, which opens the Commits tab. Each comment and review takes a quiet send to the agent, a quote reply and
//! the reactions, on its head's hover.

use gpui_kit::component::popover::Popover;
use gpui_kit::component::text::TextView;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, linear_color_stop, linear_gradient, prelude::*, px, AnyElement, App, Context, Div, Entity, FontWeight, SharedString,
    WeakEntity, Window,
};

use super::model::{self, Entry, Item, ItemKind, Reaction, Seen};
use super::parts;
use super::words::{self, PR_WORDS};
use super::PullRequestScreen;
use crate::screens::pr_pane::comments::CommentParts;
use crate::screens::pr_pane::files::markdown::on_link;
use crate::ui::{self, motion, section_head, tip, ActivePalette, Button, Glyph};

/// Taller than this a body folds: twelve lines of the pane's 14 px text and the band its fade takes (`CLAMP_PX`).
pub const CLAMP: gpui_kit::Pixels = px(300.);

impl PullRequestScreen {
    pub(super) fn conversation_tab(&mut self, seen: &Seen, window: &mut Window, cx: &mut Context<Self>) -> Vec<AnyElement> {
        let p = cx.palette().clone();
        let mut out = vec![self.status_card(seen, cx)];
        if let Some(page) = &self.page {
            if page.body.trim().is_empty() {
                out.push(
                    div()
                        .debug_selector(|| "pr-no-body".into())
                        .text_size(px(13.))
                        .text_color(p.muted_foreground)
                        .child(PR_WORDS.no_description)
                        .into_any_element(),
                );
            } else if let Some(body) = self.body("description").cloned() {
                out.push(
                    div()
                        .debug_selector(|| "pr-body".into())
                        .child(self.clamped("description", markdown("description", &body, cx).into_any_element(), cx))
                        .into_any_element(),
                );
            }
        }
        if let Some(draft) = &self.review_draft {
            let find = self.scroll_mark(super::ScrollTo::Draft, cx);
            out.push(div().relative().child(find).child(draft.clone()).into_any_element());
        }
        let mut activity = v_flex().debug_selector(|| "pr-activity".into()).gap(px(10.)).child(section_head(PR_WORDS.activity, cx));
        if let Some(cut) = self.page.as_ref().and_then(|p| p.cut.clone()) {
            for (on, words) in
                [(cut.reviews, PR_WORDS.cut.reviews), (cut.threads, PR_WORDS.cut.threads), (cut.comments, PR_WORDS.cut.comments)]
            {
                if on {
                    activity = activity.child(parts::cut_note(words, cx));
                }
            }
        }
        activity = activity.child(div().mt(px(2.)).child(match &self.page {
            None => parts::timeline_skeleton(self.width),
            Some(_) => self.timeline(window, cx),
        }));
        if self.page.is_some() {
            if let Some(foot) = &self.foot {
                let find = self.scroll_mark(super::ScrollTo::Foot, cx);
                activity = activity.child(div().debug_selector(|| "pr-comment-box".into()).relative().child(find).child(foot.clone()));
            }
        }
        out.push(activity.into_any_element());
        out
    }

    /// Covers a section and, drawn while the pane is asked to scroll to it, scrolls there: the draft to the top, the
    /// comment box to the middle.
    fn scroll_mark(&self, target: super::ScrollTo, cx: &mut Context<Self>) -> AnyElement {
        let me = cx.weak_entity();
        canvas(
            move |bounds, _, cx| {
                _ = me.update(cx, |this, _| {
                    if this.scroll_to != Some(target) {
                        return;
                    }
                    this.scroll_to = None;
                    let down = bounds.origin.y - this.top;
                    let y = match target {
                        super::ScrollTo::Draft => -down,
                        super::ScrollTo::Foot => (this.scroll.bounds().size.height - bounds.size.height) / 2. - down,
                    };
                    this.scroll.set_offset(gpui_kit::point(px(0.), y.min(px(0.))));
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .inset_0()
        .into_any_element()
    }

    /// A body under the clamp: past 300 px its last lines fade and Show more stands in the clear bottom (`Clamped`).
    pub(super) fn clamped(&self, key: &str, body: AnyElement, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let tall = self.tall.contains(key);
        let open = self.unclamped.contains(key);
        let folded = tall && !open;
        let me = cx.weak_entity();
        let measured = key.to_owned();
        let inner = div().on_children_prepainted(move |bounds, _, cx| {
            let height = bounds.iter().map(|b| b.size.height).fold(px(0.), |a, b| a + b);
            let is_tall = height > CLAMP;
            _ = me.update(cx, |this, cx| {
                if this.tall.contains(&measured) != is_tall {
                    if is_tall {
                        this.tall.insert(measured.clone());
                    } else {
                        this.tall.remove(&measured);
                    }
                    cx.notify();
                }
            });
        });
        let me = cx.weak_entity();
        let toggled = key.to_owned();
        let button =
            Button::ghost(SharedString::from(format!("pr-show-more-{key}")), if open { PR_WORDS.show_less } else { PR_WORDS.show_more })
                .xs()
                .on_click(move |_, _, cx| {
                    let key = toggled.clone();
                    _ = me.update(cx, |this, cx| {
                        if !this.unclamped.remove(&key) {
                            this.unclamped.insert(key);
                        }
                        cx.notify();
                    });
                });
        let fade = linear_gradient(180., linear_color_stop(p.background.opacity(0.), 0.), linear_color_stop(p.background, 0.7));
        div()
            .debug_selector(|| "pr-clamp".into())
            .relative()
            .child(
                div()
                    .when(folded, |el| el.max_h(CLAMP).overflow_hidden())
                    .child(inner.child(body))
                    .when(folded, |el| el.child(div().absolute().left_0().right_0().bottom_0().h(px(80.)).bg(fade))),
            )
            .when(tall, |el| {
                el.child(
                    h_flex().justify_end().map(|el| if folded { el.absolute().right_0().bottom_0() } else { el.mt(px(6.)) }).child(button),
                )
            })
            .into_any_element()
    }

    fn timeline(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        if self.page.is_none() {
            return div().into_any_element();
        }
        let derived = self.derived.clone();
        let entries = &derived.entries;
        if entries.is_empty() {
            return div().into_any_element();
        }
        let p = cx.palette().clone();
        let mut list = v_flex()
            .debug_selector(|| "pr-timeline".into())
            .relative()
            .gap(px(22.))
            .pt(px(2.))
            .child(div().absolute().top(px(14.)).bottom(px(14.)).left(px(11.5)).w(px(1.)).bg(p.border));
        for entry in entries {
            list = list.child(self.entry(entry.clone(), window, cx));
        }
        list.into_any_element()
    }

    fn entry(&mut self, entry: Entry, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let key = SharedString::from(entry.key().to_owned());
        let over = motion::hovered(key.clone(), window, cx);
        let hovered = *over.read(cx) || self.picking.as_deref() == Some(key.as_ref());
        let fade = f32::from(motion::ease_px(key.clone(), "acts", px(if hovered { 1. } else { 0. }), motion::COLORS, window, cx));
        let (face, content): (AnyElement, Vec<AnyElement>) = match entry {
            Entry::Comment { at, comment, .. } if comment.bot => {
                let open = self.notices_open.contains(key.as_ref());
                let me = cx.weak_entity();
                let toggled = key.to_string();
                let word = Button::ghost(SharedString::from(format!("{key}-notice")), if open { PR_WORDS.hide } else { PR_WORDS.show })
                    .xs()
                    .on_click(move |_, _, cx| {
                        let key = toggled.clone();
                        _ = me.update(cx, |this, cx| {
                            if !this.notices_open.remove(&key) {
                                this.notices_open.insert(key);
                            }
                            cx.notify();
                        });
                    });
                let item = Item { kind: ItemKind::Comment, id: comment.id };
                let mut content = vec![self.head_line(
                    &key,
                    &comment.author,
                    Some(PR_WORDS.left_notice),
                    Some(word.into_any_element()),
                    &at,
                    Some(item),
                    None,
                    false,
                    hovered,
                    fade,
                    cx,
                )];
                if open {
                    let body_key = format!("comment:{}", comment.id);
                    if let Some(body) = self.body(&body_key).cloned() {
                        content.push(div().mt_1().child(markdown(&body_key, &body, cx)).into_any_element());
                    }
                }
                (parts::face(&comment.author, px(24.), false, cx), content)
            }
            Entry::Comment { at, comment, .. } => {
                let item = Item { kind: ItemKind::Comment, id: comment.id };
                let tools = self.item_tools(
                    &key,
                    comment.node_id.clone(),
                    comment.reactions.as_deref(),
                    Some((comment.author.clone(), comment.body.clone())),
                    window,
                    cx,
                );
                let mut content = vec![self.head_line(
                    &key,
                    &comment.author,
                    Some(PR_WORDS.commented),
                    None,
                    &at,
                    Some(item),
                    tools,
                    false,
                    hovered,
                    fade,
                    cx,
                )];
                let body_key = format!("comment:{}", comment.id);
                if let Some(body) = self.body(&body_key).cloned() {
                    content.push(
                        div()
                            .mt_1()
                            .child(self.clamped(&body_key, markdown(&body_key, &body, cx).into_any_element(), cx))
                            .into_any_element(),
                    );
                }
                content.extend(self.chips(&key, comment.node_id.clone(), comment.reactions.as_deref(), cx));
                (parts::face(&comment.author, px(24.), false, cx), content)
            }
            Entry::Review { at, review, threads, .. } => {
                let (verdict, tone) = model::review_verdict(&review.state);
                let said = !review.body.is_empty();
                let item = review.id.filter(|_| said).map(|id| Item { kind: ItemKind::Review, id });
                let word = parts::state_word(&format!("{key}-verdict"), &verdict, tone, false, cx);
                let tools = if said {
                    self.item_tools(
                        &key,
                        review.node_id.clone(),
                        review.reactions.as_deref(),
                        Some((review.author.clone(), review.body.clone())),
                        window,
                        cx,
                    )
                } else {
                    None
                };
                let mut content = vec![self.head_line(
                    &key,
                    &review.author,
                    Some(PR_WORDS.reviewed),
                    Some(word),
                    &at,
                    item,
                    tools,
                    false,
                    hovered,
                    fade,
                    cx,
                )];
                let body_key = key.to_string();
                if said {
                    if let Some(body) = self.body(&body_key).cloned() {
                        content.push(
                            div()
                                .mt_1()
                                .child(self.clamped(&body_key, markdown(&body_key, &body, cx).into_any_element(), cx))
                                .into_any_element(),
                        );
                    }
                    content.extend(self.chips(&key, review.node_id.clone(), review.reactions.as_deref(), cx));
                }
                for thread in threads {
                    if let Some(b) = self.threads.get(&thread.key) {
                        content.push(div().mt(px(10.)).child(b.clone()).into_any_element());
                    }
                }
                (parts::face(&review.author, px(24.), false, cx), content)
            }
            Entry::Thread { at, thread, .. } => {
                let first = &thread.comments[0];
                let mut content = vec![self.head_line(
                    &key,
                    &first.author,
                    Some(PR_WORDS.commented_on_line),
                    None,
                    &at,
                    None,
                    None,
                    false,
                    hovered,
                    fade,
                    cx,
                )];
                if let Some(b) = self.threads.get(&thread.key) {
                    content.push(div().mt(px(10.)).child(b.clone()).into_any_element());
                }
                (parts::face(&first.author, px(24.), false, cx), content)
            }
            Entry::Push { at, commits, authors, .. } => {
                let who = authors.iter().map(|a| model::author_name(a)).collect::<Vec<_>>().join(", ");
                let verb = words::pushed(commits.len());
                let head = self.head_line(&key, &who, Some(&verb), None, &at, None, None, true, hovered, fade, cx);
                let me = cx.weak_entity();
                let line = div()
                    .id(SharedString::from(format!("{key}-open")))
                    .debug_selector(|| "pr-push".into())
                    .cursor_pointer()
                    .on_click(move |_, _, cx| {
                        _ = me.update(cx, |this, cx| this.pick_tab(super::Tab::Commits, cx));
                    })
                    .child(head)
                    .into_any_element();
                (parts::face(authors.first().map_or("", String::as_str), px(24.), false, cx), vec![line])
            }
        };
        h_flex()
            .id(key.clone())
            .debug_selector(|| "pr-entry".into())
            .relative()
            .items_start()
            .gap_3()
            .on_hover(motion::track_hover(over))
            .child(div().flex_none().child(face))
            .child(v_flex().flex_1().min_w_0().children(content))
            .into_any_element()
    }

    /// An entry's head: who, what they did, a word, when it was sent to the agent, the acts on the hover, when.
    #[allow(clippy::too_many_arguments)]
    fn head_line(
        &self,
        key: &str,
        author: &str,
        verb: Option<&str>,
        word: Option<AnyElement>,
        at: &str,
        item: Option<Item>,
        tools: Option<AnyElement>,
        go: bool,
        hovered: bool,
        fade: f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let p = cx.palette().clone();
        let mono = cx.theme().mono_font_family.clone();
        let page = self.page.as_ref();
        let road = item.and_then(|item| page.map(|page| (item, model::sent_at(page, item))));
        let sent = road.and_then(|(_, at)| at).map(|at| words::sent(&model::spaced(&model::ago(&model::ms_to_iso(at), model::now()))));
        let send = road
            .filter(|(_, at)| at.is_none())
            .filter(|(item, _)| !self.is_sending(*item))
            .map(|(item, _)| self.send_button(key, item, cx));
        let author = if verb.is_some() || go { model::author_name(author) } else { author.to_owned() };
        // Drawn at no width at rest, not left out, so a picker opened from them outlives the pointer leaving.
        let acts = (tools.is_some() || send.is_some()).then(|| {
            h_flex()
                .flex_none()
                .overflow_hidden()
                .map(|el| if hovered { el.mr(px(6.)).opacity(fade) } else { el.w(px(0.)).opacity(0.) })
                .children(tools)
                .children(send)
        });
        h_flex()
            .debug_selector(|| "pr-entry-head".into())
            .min_h(px(24.))
            .min_w_0()
            .gap_2()
            .text_size(px(13.))
            .line_height(px(24.))
            .child(div().flex_none().font_weight(FontWeight::MEDIUM).text_color(p.foreground).child(author))
            .children(verb.map(|v| div().min_w_0().truncate().text_color(p.muted_foreground).child(v.to_owned())))
            .when(go, |el| {
                el.child(Icon::new(Glyph::ChevronRight).with_size(px(13.)).flex_none().text_color(p.muted_foreground.opacity(0.7)))
            })
            .children(word.map(|w| div().flex_none().ml(px(2.)).child(w)))
            .child(div().flex_1())
            .children(
                sent.map(|s| {
                    div().debug_selector(|| "pr-sent".into()).flex_none().text_size(px(12.)).text_color(p.muted_foreground).child(s)
                }),
            )
            .child(
                h_flex()
                    .flex_none()
                    .font_family(mono)
                    .text_size(px(12.))
                    .text_color(p.muted_foreground)
                    .children(acts)
                    .child(model::ago(at, model::now())),
            )
            .into_any_element()
    }

    fn send_button(&self, key: &str, item: Item, cx: &mut Context<Self>) -> AnyElement {
        send_to_agent(cx.weak_entity(), key, item, &self.agent, cx)
    }

    /// The head's tools for an item: Quote reply where the person can reply, and the reaction add button.
    fn item_tools(
        &self,
        key: &str,
        node_id: Option<String>,
        reactions: Option<&[Reaction]>,
        quote: Option<(String, String)>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let posts = self.page.as_ref().is_some_and(|p| p.posts_as_you);
        if !posts {
            return None;
        }
        let me = cx.weak_entity();
        let quote = quote.filter(|_| self.foot.is_some()).map(|(author, body)| {
            Button::icon_xs(SharedString::from(format!("{key}-quote")), Glyph::Quote).tooltip(PR_WORDS.quote_reply).on_click(
                move |_, window, cx| {
                    let (author, body) = (author.clone(), body.clone());
                    _ = me.update(cx, |this, cx| this.quote(&author, &body, window, cx));
                },
            )
        });
        let add = node_id.map(|subject| react_add(cx.weak_entity(), key, "pr-react-add", subject, reactions.unwrap_or_default().to_vec()));
        if quote.is_none() && add.is_none() {
            return None;
        }
        Some(h_flex().gap(px(2.)).children(quote).children(add).into_any_element())
    }

    /// The used reactions as chips under an item's body, the person's own marked; a press toggles the person's own.
    fn chips(&self, key: &str, node_id: Option<String>, reactions: Option<&[Reaction]>, cx: &mut Context<Self>) -> Option<AnyElement> {
        reaction_chips(
            cx.weak_entity(),
            key,
            node_id.filter(|_| self.page.as_ref().is_some_and(|p| p.posts_as_you)),
            reactions.unwrap_or_default(),
            cx,
        )
    }
}

/// A body in the pane's markdown, the item's `key` naming it: 14 px at 1.55. Someone else wrote it, so a link opens
/// only as a web page.
pub fn markdown(key: &str, state: &Entity<gpui_kit::component::text::TextViewState>, cx: &App) -> Div {
    let p = cx.palette();
    let key = key.to_owned();
    div()
        .debug_selector(move || format!("pr-markdown:{key}"))
        .text_size(px(14.))
        .line_height(px(21.7))
        .text_color(p.foreground)
        .child(TextView::new(state).on_link_click(on_link))
}

/// The used reactions as 22 px chips; with a subject each is a press that toggles the person's own.
pub fn reaction_chips(
    me: WeakEntity<PullRequestScreen>,
    key: &str,
    subject: Option<String>,
    reactions: &[Reaction],
    cx: &App,
) -> Option<AnyElement> {
    let used: Vec<&Reaction> = reactions.iter().filter(|r| r.count > 0).collect();
    if used.is_empty() {
        return None;
    }
    let p = cx.palette().clone();
    let mono = cx.theme().mono_font_family.clone();
    Some(
        h_flex()
            .debug_selector(|| "pr-reactions".into())
            .mt_2()
            .flex_wrap()
            .gap(px(6.))
            .children(used.into_iter().map(|r| {
                let mine = r.mine;
                let chip = h_flex()
                    .id(SharedString::from(format!("{key}-reaction-{}", r.content)))
                    .debug_selector(|| if mine { "pr-reaction-mine".into() } else { "pr-reaction".into() })
                    .h(px(22.))
                    .gap(px(5.))
                    .px_2()
                    .rounded(px(11.))
                    .border_1()
                    .text_size(px(12.))
                    .map(|el| {
                        if mine {
                            el.border_color(p.primary.opacity(0.45)).bg(p.primary.opacity(0.08)).text_color(p.foreground)
                        } else {
                            el.border_color(p.border).text_color(p.muted_foreground)
                        }
                    })
                    .tooltip(tip(format!("{}, {}", model::reaction_label(&r.content), r.count)))
                    .child(model::emoji_of(&r.content).to_owned())
                    .child(div().font_family(mono.clone()).text_size(px(11.)).font_weight(FontWeight::MEDIUM).child(r.count.to_string()));
                match subject.clone() {
                    Some(subject) => {
                        let me = me.clone();
                        let content = r.content.clone();
                        chip.cursor_pointer()
                            .when(!mine, |el| el.hover(|s| s.bg(p.accent)))
                            .on_click(move |_, window, cx| {
                                let (subject, content) = (subject.clone(), content.clone());
                                _ = me.update(cx, |this, cx| this.react(subject, content, !mine, window, cx));
                            })
                            .into_any_element()
                    }
                    None => chip.into_any_element(),
                }
            }))
            .into_any_element(),
    )
}

/// The add button and GitHub's eight behind it, the person's own marked (`ReactAdd`).
pub fn react_add(
    me: WeakEntity<PullRequestScreen>,
    key: &str,
    selector: &'static str,
    subject: String,
    reactions: Vec<Reaction>,
) -> AnyElement {
    let trigger = div().id(SharedString::from(format!("{key}-react"))).debug_selector(move || selector.into()).child(
        Button::icon_xs_path(SharedString::from(format!("{key}-react-add")), crate::ui::icons::SMILE_PLUS).tooltip(PR_WORDS.add_reaction),
    );
    let picked = me.clone();
    let held = key.to_owned();
    Popover::new(SharedString::from(format!("{key}-react-picker")))
        .anchor(gpui_kit::Anchor::BottomLeft)
        .on_open_change(move |open, _, cx| {
            let key = held.clone();
            let open = *open;
            _ = picked.update(cx, |this, cx| {
                this.picking = if open { Some(key) } else { this.picking.take().filter(|k| *k != key) };
                cx.notify();
            });
        })
        .trigger(TriggerButton(trigger))
        .content(move |_, _, cx| {
            let p = cx.palette().clone();
            let popover = cx.entity();
            let me = me.clone();
            let subject = subject.clone();
            let reactions = reactions.clone();
            h_flex()
                .debug_selector(|| "pr-react-picker".into())
                .occlude()
                .gap(px(2.))
                .children(model::REACTIONS.iter().map(move |(content, emoji, label)| {
                    let mine = reactions.iter().any(|r| r.content == *content && r.mine);
                    let me = me.clone();
                    let subject = subject.clone();
                    let popover = popover.clone();
                    div()
                        .id(SharedString::from(format!("pick-{content}")))
                        .debug_selector(move || format!("pr-react-pick-{content}"))
                        .flex()
                        .items_center()
                        .justify_center()
                        .size(px(32.))
                        .rounded(px(6.))
                        .text_size(px(16.))
                        .cursor_pointer()
                        .when(mine, |el| el.bg(p.primary.opacity(0.08)))
                        .hover(|s| s.bg(p.accent))
                        .tooltip(tip(*label))
                        .on_click(move |_, window, cx| {
                            let subject = subject.clone();
                            _ = me.update(cx, |this, cx| this.react(subject, (*content).to_owned(), !mine, window, cx));
                            popover.update(cx, |s, cx| s.dismiss(window, cx));
                        })
                        .child(*emoji)
                }))
                .into_any_element()
        })
        .into_any_element()
}

/// The element a popover opens from.
#[derive(IntoElement)]
pub struct TriggerButton(pub gpui_kit::Stateful<gpui_kit::Div>);

impl RenderOnce for TriggerButton {
    fn render(self, _: &mut Window, _: &mut App) -> impl IntoElement {
        self.0
    }
}

impl gpui_kit::component::Selectable for TriggerButton {
    fn selected(self, _: bool) -> Self {
        self
    }

    fn is_selected(&self) -> bool {
        false
    }
}

/// What a thread box draws for one of its comments: the face, the body, the acts on its hover and the chips.
pub fn comment_parts(me: &WeakEntity<PullRequestScreen>, id: u64, cx: &mut App) -> CommentParts {
    let empty = || CommentParts { face: div().into_any_element(), body: div().into_any_element(), tools: None, chips: None, note: None };
    let Some(screen) = me.upgrade() else { return empty() };
    let this = screen.read(cx);
    let Some(page) = &this.page else { return empty() };
    let Some(comment) = page.review_comments.iter().find(|c| c.id == id).cloned() else { return empty() };
    let key = format!("reviewComment:{id}");
    let item = Item { kind: ItemKind::ReviewComment, id };
    let sent = model::sent_at(page, item)
        .map(|at| SharedString::from(words::sent(&model::spaced(&model::ago(&model::ms_to_iso(at), model::now())))));
    let posts = page.posts_as_you;
    let sending = this.is_sending(item);
    let agent = this.agent.clone();
    let body = this.body(&key).cloned();
    let face = parts::face(&comment.author, px(20.), false, cx);
    let body = match body {
        Some(b) => markdown(&key, &b, cx).text_size(px(13.5)).line_height(px(20.9)).into_any_element(),
        None => div().into_any_element(),
    };
    let chips =
        reaction_chips(me.clone(), &key, comment.node_id.clone().filter(|_| posts), comment.reactions.as_deref().unwrap_or_default(), cx);
    let add = comment
        .node_id
        .clone()
        .filter(|_| posts)
        .map(|subject| react_add(me.clone(), &key, "pr-line-react-add", subject, comment.reactions.clone().unwrap_or_default()));
    let send = (sent.is_none() && !sending).then(|| send_to_agent(me.clone(), &key, item, &agent, cx));
    let tools = (add.is_some() || send.is_some()).then(|| h_flex().gap(px(2.)).children(add).children(send).into_any_element());
    CommentParts { face, body, tools, chips, note: sent }
}

/// The quiet send on an item's head: the agent's mark, the agent named on its hover (`SendToAgent`).
fn send_to_agent(me: WeakEntity<PullRequestScreen>, key: &str, item: Item, agent: &str, cx: &App) -> AnyElement {
    let p = cx.palette();
    div()
        .id(SharedString::from(format!("{key}-send")))
        .debug_selector(|| "pr-send".into())
        .flex()
        .items_center()
        .justify_center()
        .size(px(24.))
        .rounded(px(6.))
        .cursor_pointer()
        .hover(|s| s.bg(p.accent))
        .tooltip(tip(words::send_to_agent(&ui::catalog::name(agent))))
        .on_click(move |_, window, cx| {
            _ = me.update(cx, |this, cx| this.send(vec![item], window, cx));
        })
        .child(ui::catalog::mark(agent, &ui::catalog::name(agent), px(14.), p.muted_foreground, cx))
        .into_any_element()
}
