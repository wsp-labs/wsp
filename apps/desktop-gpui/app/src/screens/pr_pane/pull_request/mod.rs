// SPDX-License-Identifier: AGPL-3.0-only
//! The Pull request pane as today's app draws it (`apps/web/src/pull-request/`): a head with the state glyph in its
//! frame, the title, who opened it, the number as a link out, the one word and the branches; then three tabs.
//! Conversation is Status first, then the description, then Activity; Commits is one rail by day; Files is the list,
//! a file opening its diff in place. The page is read when the pane opens, again when the status the host pushes
//! says it moved, and on the refresh, which asks the host past the minute it holds a read.

pub mod commits;
pub mod conversation;
pub mod files;
pub mod git_split;
pub mod link;
pub mod model;
pub mod parts;
pub mod review;
pub mod status;
pub mod words;

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::component::input::RopeExt as _;
use gpui_kit::component::text::TextViewState;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, prelude::*, px, AnyElement, AnyWindowHandle, App, AsyncWindowContext, Context, Entity, FontWeight, Pixels, ScrollHandle,
    SharedString, Task, WeakEntity, Window,
};
use serde_json::{json, Value};
use wsp_host_client::protocol;

use self::model::{Item, Page, Seen, State, Status};
use self::words::PR_WORDS;
use super::comments::{CommentField, ReviewThread, Sent, ThreadBox, ThreadComment};
use super::files::markdown::open_web;
use crate::screens::{Host, Opened, Pane, Place, Screen};
use crate::ui::{self, framed, tip, ActivePalette, Button, Glyph, Notice, NoticeAction, Segmented};

/// A section the pane scrolls to once it is drawn: the review draft to the top, the comment box to the middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollTo {
    Draft,
    Foot,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    #[default]
    Conversation,
    Commits,
    Files,
}

/// The diff a pull request's files open into, read once, the first time a file opens.
#[derive(Clone, Debug, PartialEq)]
pub enum DiffRead {
    Reading,
    Read { patch: Rc<str>, left: Vec<String> },
    Refused(String),
}

pub struct PullRequestScreen {
    host: Host,
    opened: Opened,
    /// The workspace's status as the host last pushed it, none until the first lands.
    status: Option<Status>,
    page: Option<Page>,
    /// What of the status the page was read against, so a push that moved it reads it again.
    page_mark: Option<String>,
    refusal: Option<String>,
    /// Each read asked; an answer to an older one is dropped.
    asked: u64,
    tab: Tab,
    /// The checks fold, None while it follows where the pull request stands.
    checks_open: Option<bool>,
    diff: Option<DiffRead>,
    diff_asked: u64,
    open_file: Option<String>,
    all_expanded: bool,
    /// Folders the person opened or shut against the fold all.
    dirs_open: HashMap<String, bool>,
    /// The code view the open file draws in, and the file it was last given.
    code: Option<Entity<super::diff::CodeView>>,
    shown_file: Option<String>,
    /// The open file the code view shows and the lines its box makes room for; none while it has no diff.
    open_view: Option<(String, usize)>,
    /// What the page reads as, worked out when it changes.
    derived: Rc<model::Derived>,
    scroll: ScrollHandle,
    /// Where the column's top stands in the window, so a section can be scrolled to.
    top: Pixels,
    /// The capture's state is opened once, on the first page; `review` then scrolls to the draft once it is drawn.
    shot_done: bool,
    scroll_to: Option<ScrollTo>,
    sending: Vec<Item>,
    /// A merge, an update or a fix on its way: another press waits for its answer, as the git button's does.
    acting: bool,
    /// The latest press on each item's reactions, so only it may settle them.
    presses: HashMap<String, u64>,
    /// Bodies past the clamp that the person opened, and those measured taller than it.
    unclamped: HashSet<String>,
    tall: HashSet<String>,
    notices_open: HashSet<String>,
    said: Option<String>,
    /// The entry whose reaction picker is open, whose acts stay out while it is.
    picking: Option<String>,
    commit_open: Option<String>,
    agent: String,
    /// Each body's markdown, by the item it belongs to; only the page's items are kept.
    bodies: HashMap<String, (String, Entity<TextViewState>)>,
    threads: HashMap<String, Entity<ThreadBox>>,
    foot: Option<Entity<CommentField>>,
    review_draft: Option<Entity<review::ReviewDraftView>>,
    width: Pixels,
    _follow: Option<Task<()>>,
    _read: Option<Task<()>>,
}

impl Screen for PullRequestScreen {
    const NAME: &'static str = "pull-request";
    const PLACE: Place = Place::Panel(Pane::PullRequest);

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // A route may open the pane on a tab, as `pull-request:commits` does for a capture.
        let tab = match opened.tab.as_deref() {
            Some("commits") => Tab::Commits,
            Some("files") => Tab::Files,
            _ => Tab::Conversation,
        };
        let mut screen = PullRequestScreen {
            host,
            opened,
            status: None,
            page: None,
            page_mark: None,
            refusal: None,
            asked: 0,
            tab,
            checks_open: None,
            diff: None,
            diff_asked: 0,
            open_file: None,
            all_expanded: false,
            dirs_open: HashMap::new(),
            code: None,
            shown_file: None,
            open_view: None,
            derived: Rc::default(),
            scroll: ScrollHandle::new(),
            top: px(0.),
            shot_done: false,
            scroll_to: None,
            sending: Vec::new(),
            acting: false,
            presses: HashMap::new(),
            unclamped: HashSet::new(),
            tall: HashSet::new(),
            notices_open: HashSet::new(),
            picking: None,
            said: None,
            commit_open: None,
            agent: "claude".into(),
            bodies: HashMap::new(),
            threads: HashMap::new(),
            foot: None,
            review_draft: None,
            width: px(420.),
            _follow: None,
            _read: None,
        };
        screen.follow(window, cx);
        screen
    }
}

impl PullRequestScreen {
    pub fn workspace_id(&self) -> Option<&str> {
        self.opened.workspace_id.as_deref()
    }

    pub fn status(&self) -> Option<&Status> {
        self.status.as_ref()
    }

    pub fn page(&self) -> Option<&Page> {
        self.page.as_ref()
    }

    pub fn tab(&self) -> Tab {
        self.tab
    }

    pub fn refusal(&self) -> Option<&str> {
        self.refusal.as_deref()
    }

    pub fn diff(&self) -> Option<&DiffRead> {
        self.diff.as_ref()
    }

    /// How far the pane is scrolled down.
    pub fn scrolled(&self) -> Pixels {
        -self.scroll.offset().y
    }

    pub fn foot(&self) -> Option<&Entity<CommentField>> {
        self.foot.as_ref()
    }

    pub fn thread_box(&self, key: &str) -> Option<&Entity<ThreadBox>> {
        self.threads.get(key)
    }

    fn name(&self) -> String {
        self.status
            .as_ref()
            .map(|s| s.name.clone())
            .filter(|n| !n.is_empty())
            .or_else(|| self.opened.workspace_id.clone())
            .unwrap_or_default()
    }

    fn seen(&self) -> Option<&Seen> {
        self.status.as_ref()?.pr.as_ref()
    }

    /// Follows the workspace's status, and the agent its sends reach.
    fn follow(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.opened.workspace_id.clone() else { return };
        let (tx, mut rx) = mpsc::unbounded();
        let road = link::follow(&self.host, workspace_id.clone(), tx);
        let agent = link::agent_of(&self.host, workspace_id);
        cx.spawn(async move |this, cx| {
            if let Ok(agent) = agent.await {
                _ = this.update(cx, |this, cx| {
                    this.agent = agent;
                    cx.notify();
                });
            }
        })
        .detach();
        self._follow = Some(cx.spawn_in(window, async move |this, cx| {
            let pushes = async {
                while let Some(status) = rx.next().await {
                    if this.update_in(cx, |this, window, cx| this.take_status(status, window, cx)).is_err() {
                        break;
                    }
                }
            };
            let road = async {
                if let Err(failure) = road.await {
                    eprintln!("desktop-gpui: the pull request pane stopped following its status: {}", failure.message);
                }
            };
            futures::join!(pushes, road);
        }));
    }

    /// A status the host sent: the head redraws from it, and a pull request that moved is read again.
    pub fn take_status(&mut self, value: Value, window: &mut Window, cx: &mut Context<Self>) {
        let status = Status::read(&value);
        let reviewing = matches!(status.from, Some(model::From::Review));
        if reviewing && self.review_draft.is_none() {
            if let Some(workspace_id) = self.opened.workspace_id.clone() {
                let host = self.host.clone();
                self.review_draft = Some(cx.new(|cx| review::ReviewDraftView::new(host, workspace_id, window, cx)));
            }
        }
        if let (Some(draft), Some(Seen::Fact(fact))) = (&self.review_draft, &status.pr) {
            let head = fact.head_oid.clone();
            draft.update(cx, |d, cx| d.set_head_oid(Some(head), cx));
        }
        let settled_before = self.seen().map(|s| s.state() != State::Open);
        let mark = status.pr.as_ref().filter(|s| s.number().is_some()).map(Seen::mark);
        let number_before = self.seen().and_then(Seen::number);
        let number_now = status.pr.as_ref().and_then(Seen::number);
        self.status = Some(status);
        if let Some(draft) = &self.review_draft {
            let name = self.name();
            draft.update(cx, |d, _| d.set_name(name));
        }
        if number_before.is_some() && number_before != number_now {
            self.forget_page();
        }
        let settled_now = self.seen().map(|s| s.state() != State::Open);
        if settled_before.is_some() && settled_before != settled_now {
            self.checks_open = None;
        }
        if mark.is_some() && mark != self.page_mark {
            self.page_mark = mark;
            self.read_page(false, window, cx);
        }
        cx.notify();
    }

    /// Reads the page; `fresh` asks past what the host holds.
    fn read_page(&mut self, fresh: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.opened.workspace_id.clone() else { return };
        self.asked += 1;
        let asked = self.asked;
        let params = if fresh { json!({ "workspaceId": workspace_id, "fresh": true }) } else { json!({ "workspaceId": workspace_id }) };
        let read = link::ask::<protocol::WorkspacesPullRequestViewRequest>(&self.host, params);
        self._read = Some(cx.spawn_in(window, async move |this, cx| {
            let answer = read.await;
            _ = this.update_in(cx, |this, window, cx| {
                if asked != this.asked {
                    return;
                }
                match answer.and_then(|v| serde_json::from_value::<Page>(v).map_err(|e| wsp_host_client::Failure::new(e.to_string()))) {
                    Ok(page) => {
                        this.refusal = None;
                        this.set_page(page, window, cx);
                    }
                    Err(failure) => this.refusal = Some(failure.message),
                }
                cx.notify();
            });
        }));
    }

    /// Opens straight into the state a capture names: `tab:<conversation|commits|files>`, `file:<path>` (its diff open
    /// under its row) or `review` (scrolled to the review draft).
    fn open_shot(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(shot) = super::shot().filter(|_| !self.shot_done) else { return };
        self.shot_done = true;
        match shot.split_once(':') {
            Some(("tab", "conversation")) => self.tab = Tab::Conversation,
            Some(("tab", "commits")) => self.tab = Tab::Commits,
            Some(("tab", "files")) => self.tab = Tab::Files,
            Some(("file", path)) => {
                self.tab = Tab::Files;
                let mut dir = path;
                while let Some((up, _)) = dir.rsplit_once('/') {
                    self.dirs_open.insert(up.to_owned(), true);
                    dir = up;
                }
                self.press_file(path.to_owned(), window, cx);
            }
            _ if shot == "review" => {
                self.tab = Tab::Conversation;
                self.scroll_to = Some(ScrollTo::Draft);
            }
            _ => {}
        }
        cx.notify();
    }

    /// Drops everything read for one pull request, as another takes its place: an answer still on its way for the
    /// old one is dropped with it.
    fn forget_page(&mut self) {
        self.page = None;
        self.page_mark = None;
        self.derived = Rc::default();
        self.asked += 1;
        self.diff = None;
        self.diff_asked += 1;
        self.open_file = None;
        self.shown_file = None;
        self.open_view = None;
        self.tall.clear();
        self.unclamped.clear();
        self.notices_open.clear();
        self.presses.clear();
        self.commit_open = None;
    }

    /// Whether anything of the last pull request is still held for the one on screen now.
    pub fn holds_another(&self) -> bool {
        self.open_file.is_some() || self.diff.is_some() || !self.presses.is_empty()
    }

    fn set_page(&mut self, page: Page, window: &mut Window, cx: &mut Context<Self>) {
        self.page = Some(page);
        self.page_changed(cx);
        self.open_shot(window, cx);
        if self.foot.is_none() && self.page.as_ref().is_some_and(|p| p.posts_as_you) {
            let me = cx.weak_entity();
            self.foot = Some(cx.new(|cx| {
                let mut field = CommentField::new(PR_WORDS.leave_comment, window, cx).clear_on_sent();
                field.on_send(move |body, _, cx| reply_on(&me, body, None, cx));
                field
            }));
        }
    }

    /// The page moved: what it reads as, its bodies, its threads and the open file's notes follow it.
    fn page_changed(&mut self, cx: &mut Context<Self>) {
        self.derived = Rc::new(self.page.as_ref().map(model::derive).unwrap_or_default());
        self.keep_bodies(cx);
        self.keep_threads(cx);
        self.sync_open_file(cx);
        cx.notify();
    }

    pub fn derived(&self) -> &model::Derived {
        &self.derived
    }

    /// The markdown each item of the page draws, made once per body and dropped with the item.
    fn keep_bodies(&mut self, cx: &mut Context<Self>) {
        let Some(page) = &self.page else { return };
        let wanted = model::bodies_of(page);
        let keys: HashSet<&String> = wanted.iter().map(|(k, _)| k).collect();
        self.bodies.retain(|k, _| keys.contains(k));
        for (key, text) in wanted {
            if self.bodies.get(&key).is_some_and(|(held, _)| *held == text) {
                continue;
            }
            let shown = parts::restricted(&text);
            let state = cx.new(|cx| TextViewState::markdown(&shown, cx));
            self.bodies.insert(key, (text, state));
        }
    }

    pub fn body(&self, key: &str) -> Option<&Entity<TextViewState>> {
        self.bodies.get(key).map(|(_, s)| s)
    }

    /// One thread box per review thread on the page, told its thread again on each read so its fold and field stay.
    fn keep_threads(&mut self, cx: &mut Context<Self>) {
        let Some(page) = &self.page else { return };
        let threads = self.derived.threads.clone();
        let keys: HashSet<&str> = threads.iter().map(|t| t.key.as_str()).collect();
        self.threads.retain(|k, _| keys.contains(k.as_str()));
        let posts = page.posts_as_you;
        let now = model::now();
        for thread in threads {
            let shown = ReviewThread {
                key: thread.key.clone().into(),
                path: thread.path.clone().into(),
                line: thread.line.map(|l| l as usize),
                resolved: thread.resolved,
                hunk: thread.hunk.clone(),
                comments: thread
                    .comments
                    .iter()
                    .map(|c| ThreadComment { id: c.id, author: model::author_name(&c.author).into(), at: model::ago(&c.at, now).into() })
                    .collect(),
            };
            if let Some(held) = self.threads.get(&thread.key) {
                held.update(cx, |b, cx| b.set_thread(shown, cx));
                continue;
            }
            let me = cx.weak_entity();
            let parts: super::comments::PartsOf = Rc::new(move |c, _, cx| conversation::comment_parts(&me, c.id, cx));
            let root = thread.comments[0].id;
            let me = cx.weak_entity();
            let thread_id = thread.thread_id.clone();
            let entity = cx.new(|_| {
                let mut b = ThreadBox::new(shown, parts);
                if posts {
                    if let Some(id) = thread_id.clone() {
                        let me = me.clone();
                        b.on_resolve(move |resolved, window, cx| resolve_on(&me, id.clone(), resolved, window.window_handle(), cx));
                    }
                    let me = me.clone();
                    let thread_id = thread_id.clone();
                    b.on_reply(move |body, _, cx| reply_on(&me, body, Some((root, thread_id.clone())), cx));
                }
                b
            });
            self.threads.insert(thread.key.clone(), entity);
        }
    }

    pub fn pick_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.tab = tab;
        cx.notify();
    }

    /// The refresh: the page asked past what the host holds, and the diff read again when a file next opens.
    pub fn refresh(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.diff_asked += 1;
        self.diff = None;
        self.shown_file = None;
        self.open_view = None;
        if self.seen().and_then(Seen::number).is_some() {
            self.read_page(true, window, cx);
            if self.open_file.is_some() {
                self.read_diff(window, cx);
            }
        }
        cx.notify();
    }

    pub fn toggle_checks(&mut self, cx: &mut Context<Self>) {
        let settled = self.seen().is_some_and(|s| s.state() != State::Open);
        self.checks_open = Some(!self.checks_open.unwrap_or(!settled));
        cx.notify();
    }

    pub fn checks_open(&self) -> bool {
        let settled = self.seen().is_some_and(|s| s.state() != State::Open);
        self.checks_open.unwrap_or(!settled)
    }

    /// Says what an act did in the app's toasts, or the host's refusal in its words, about the workspace `name`,
    /// and keeps the words as the last the pane said.
    fn toast(this: &WeakEntity<Self>, cx: &mut AsyncWindowContext, notice: Notice, name: &str) {
        let notice = notice.place(name.to_owned());
        _ = this.update(cx, |this, _| this.said = Some(notice.text.to_string()));
        _ = cx.update(|_, cx| ui::notify(cx, notice));
    }

    /// What the pane's last toast said.
    pub fn said(&self) -> Option<&str> {
        self.said.as_deref()
    }

    /// Sends a failed check, or with none the conflicts an update from the base found, to the agent (`askToFix`).
    pub fn fix(&mut self, check: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.opened.workspace_id.clone().filter(|_| !self.acting) else { return };
        let mut params = json!({ "workspaceId": workspace_id });
        if let Some(check) = &check {
            params["check"] = json!(check);
        }
        if let Some(thread) = &self.opened.thread_id {
            params["threadId"] = json!(thread);
        }
        let asking = link::ask::<protocol::WorkspacesFixRequest>(&self.host, params);
        let name = self.name();
        self.acting = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| this.done_acting(cx));
            let notice = match answer {
                Ok(done) => {
                    let base = done["base"].as_str().unwrap_or_default();
                    let agent = ui::catalog::name(done["agent"].as_str().unwrap_or_default());
                    Notice::done(if done["outcome"] == "updated" {
                        words::fix_nothing(&name, base)
                    } else if let Some(check) = done["check"].as_str() {
                        words::fix_asked(&name, &agent, check)
                    } else {
                        words::fix_conflicts(&name, &agent, base)
                    })
                }
                Err(failure) => Notice::error(failure.message),
            };
            Self::toast(&this, cx, notice, &name);
        })
        .detach();
    }

    /// Merges the base's latest commits into the branch; a conflict says which files and offers the fix.
    pub fn update(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.opened.workspace_id.clone().filter(|_| !self.acting) else { return };
        let mut params = json!({ "workspaceId": workspace_id });
        if let Some(thread) = &self.opened.thread_id {
            params["threadId"] = json!(thread);
        }
        let asking = link::ask::<protocol::WorkspacesUpdateRequest>(&self.host, params);
        let name = self.name();
        self.acting = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| this.done_acting(cx));
            let notice = match answer {
                Ok(done) => {
                    let base = done["base"].as_str().unwrap_or_default().to_owned();
                    if done["merged"] == true {
                        Notice::done(words::updated_line(&name, &base, done["commits"].as_u64().unwrap_or_default()))
                    } else {
                        let files: Vec<String> =
                            done["conflicts"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_owned)).collect();
                        let me = this.clone();
                        Notice::note(words::update_conflicts(&name, &base, &files)).action(NoticeAction::new(
                            PR_WORDS.fix,
                            move |window, cx| {
                                _ = me.update(cx, |this, cx| this.fix(None, window, cx));
                            },
                        ))
                    }
                }
                Err(failure) => Notice::error(failure.message),
            };
            Self::toast(&this, cx, notice, &name);
        })
        .detach();
    }

    /// Merges by the method given, or the repository's default, or once the checks pass, naming the head drawn.
    pub fn merge(&mut self, method: Option<model::MergeMethod>, when_checks_pass: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(workspace_id), Some(Seen::Fact(fact))) = (self.opened.workspace_id.clone().filter(|_| !self.acting), self.seen()) else {
            return;
        };
        let mut params = json!({ "workspaceId": workspace_id, "head": fact.head_oid });
        if let Some(method) = method {
            params["method"] = json!(method.wire());
        }
        if when_checks_pass {
            params["whenChecksPass"] = json!(true);
        }
        let asking = link::ask::<protocol::WorkspacesMergeRequest>(&self.host, params);
        let name = self.name();
        self.acting = true;
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let answer = asking.await;
            _ = this.update(cx, |this, cx| this.done_acting(cx));
            let notice = match answer {
                Ok(done) => Notice::done(words::merged_line(
                    &name,
                    done["number"].as_u64().unwrap_or_default(),
                    done["method"].as_str().unwrap_or_default(),
                    done["merged"] == true,
                )),
                Err(failure) => Notice::error(failure.message),
            };
            Self::toast(&this, cx, notice, &name);
        })
        .detach();
    }

    fn done_acting(&mut self, cx: &mut Context<Self>) {
        self.acting = false;
        cx.notify();
    }

    /// Whether a merge, an update or a fix is on its way.
    pub fn is_acting(&self) -> bool {
        self.acting
    }

    /// Sends items of the page to the agent; each is held while on its way and marked sent once the host says so.
    pub fn send(&mut self, items: Vec<Item>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.opened.workspace_id.clone() else { return };
        if items.is_empty() {
            return;
        }
        self.sending.extend(items.iter().copied());
        let mut params = json!({ "workspaceId": workspace_id, "items": items });
        if let Some(thread) = &self.opened.thread_id {
            params["threadId"] = json!(thread);
        }
        let asking = link::ask::<protocol::WorkspacesPullRequestSendRequest>(&self.host, params);
        let name = self.name();
        cx.notify();
        cx.spawn_in(window, async move |this, cx| {
            let answer = asking.await;
            let failed = answer.as_ref().err().map(|f| f.message.clone());
            _ = this.update(cx, |this, cx| {
                this.sending.retain(|s| !items.contains(s));
                if let (Ok(done), Some(page)) = (&answer, &mut this.page) {
                    if let Ok(sent) = serde_json::from_value(done["sent"].clone()) {
                        page.sent = sent;
                    }
                }
                this.page_changed(cx);
            });
            if let Some(said) = failed {
                Self::toast(&this, cx, Notice::error(said), &name);
            }
        })
        .detach();
    }

    pub fn is_sending(&self, item: Item) -> bool {
        self.sending.contains(&item)
    }

    /// Puts the person's reaction on or off at once, and back where the host refuses it (`usePageActs`' react).
    pub fn react(&mut self, subject: String, content: String, on: bool, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(workspace_id), Some(page)) = (self.opened.workspace_id.clone(), self.page.as_mut()) else { return };
        let press = self.presses.get(&subject).copied().unwrap_or(0) + 1;
        self.presses.insert(subject.clone(), press);
        let before = model::reactions_of(page, &subject).unwrap_or_default();
        model::set_reactions(page, &subject, model::toggled(&before, &content, on));
        self.page_changed(cx);
        let params = json!({ "workspaceId": workspace_id, "subject": subject, "content": content, "on": on });
        let asking = link::ask::<protocol::WorkspacesPullRequestReactRequest>(&self.host, params);
        let name = self.name();
        cx.spawn_in(window, async move |this, cx| {
            let answer = asking.await;
            let failed = answer.as_ref().err().map(|f| f.message.clone());
            _ = this.update(cx, |this, cx| {
                if this.presses.get(&subject) != Some(&press) {
                    return;
                }
                let Some(page) = this.page.as_mut() else { return };
                match &answer {
                    Ok(done) => {
                        if let Ok(now) = serde_json::from_value(done["reactions"].clone()) {
                            model::set_reactions(page, done["subject"].as_str().unwrap_or(&subject), now);
                        }
                    }
                    Err(_) => model::set_reactions(page, &subject, before.clone()),
                }
                this.page_changed(cx);
            });
            if let Some(said) = failed {
                Self::toast(&this, cx, Notice::error(said), &name);
            }
        })
        .detach();
    }

    /// Opens a file's diff under its row, reading the whole diff the first time one opens.
    pub fn press_file(&mut self, path: String, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file = if self.open_file.as_deref() == Some(path.as_str()) { None } else { Some(path) };
        self.sync_open_file(cx);
        if self.diff.is_none() && self.open_file.is_some() {
            self.read_diff(window, cx);
        }
        cx.notify();
    }

    fn read_diff(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(workspace_id) = self.opened.workspace_id.clone() else { return };
        self.diff_asked += 1;
        let asked = self.diff_asked;
        self.diff = Some(DiffRead::Reading);
        let read = link::ask::<protocol::WorkspacesPullRequestDiffRequest>(&self.host, json!({ "workspaceId": workspace_id }));
        cx.spawn_in(window, async move |this, cx| {
            let answer = read.await;
            _ = this.update(cx, |this, cx| {
                if asked != this.diff_asked {
                    return;
                }
                this.diff = Some(match answer {
                    Ok(done) => DiffRead::Read {
                        patch: done["diff"].as_str().unwrap_or_default().into(),
                        left: done["left"].as_array().into_iter().flatten().filter_map(|f| f.as_str().map(str::to_owned)).collect(),
                    },
                    Err(failure) => DiffRead::Refused(failure.message),
                });
                this.shown_file = None;
                this.sync_open_file(cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// A quote of an item put in the comment box at Activity's foot, after what the person already typed.
    pub fn quote(&mut self, author: &str, body: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(foot) = &self.foot else { return };
        let field = foot.read(cx).field().clone();
        let typed = field.read(cx).value().to_string();
        let quote = model::quote_of(author, body);
        let text = if typed.trim().is_empty() { quote } else { format!("{}\n\n{quote}", typed.trim_end()) };
        field.update(cx, |f, cx| {
            f.set_value(text, window, cx);
            let end = f.text().offset_to_position(f.text().len());
            f.set_cursor_position(end, window, cx);
        });
        self.scroll_to = Some(ScrollTo::Foot);
        cx.notify();
    }

    fn head(&mut self, seen: &Seen, cx: &mut Context<Self>) -> AnyElement {
        let p = cx.palette().clone();
        let page = self.page.as_ref();
        let fact = seen.fact();
        let (word, tone, hollow) = model::tone_of(seen);
        let number = seen.number().unwrap_or_default();
        let glyph = match seen.state() {
            State::Merged => Glyph::GitMerge,
            State::Closed => Glyph::GitPullRequestClosed,
            State::Open if fact.is_some_and(|f| f.draft) => Glyph::GitPullRequestDraft,
            State::Open => Glyph::GitPullRequest,
        };
        let title = page
            .map(|p| p.title.clone())
            .or_else(|| fact.map(|f| f.head_subject.clone()).filter(|s| !s.is_empty()))
            .unwrap_or_else(|| words::row(number));
        let author = page.map(|p| p.author.clone()).filter(|a| !a.is_empty()).or_else(|| fact.and_then(|f| f.author.clone()));
        let opened = page.filter(|p| !p.created_at.is_empty()).map(|p| model::spaced_ago(&p.created_at, model::now())).unwrap_or_default();
        let base = seen.base().to_owned();
        let branch =
            fact.map(|f| f.branch.clone()).or_else(|| self.status.as_ref().and_then(|s| s.checkout.as_ref().map(|c| c.branch.clone())));
        let url = seen.url().to_owned();
        let mono = cx.theme().mono_font_family.clone();
        let rest = v_flex().pl(px(44.)).gap(px(10.));
        let me = cx.weak_entity();
        let number_link = {
            let url = url.clone();
            h_flex()
                .id("pr-number")
                .debug_selector(|| "pr-number".into())
                .flex_none()
                .h(px(22.))
                .gap(px(5.))
                .font_family(mono.clone())
                .text_size(px(13.))
                .text_color(p.muted_foreground)
                .cursor_pointer()
                .hover(|s| s.text_color(p.foreground))
                .on_click(move |_, _, cx| open_web(&url, cx))
                .child(words::number(number))
                .child(Icon::new(Glyph::ExternalLink).with_size(px(12.)))
        };
        let refresh = Button::icon_xs("pr-refresh", Glyph::RefreshCw).tooltip(PR_WORDS.refresh).on_click(move |_, window, cx| {
            _ = me.update(cx, |this, cx| this.refresh(window, cx));
        });
        v_flex()
            .debug_selector(|| "pr-head".into())
            .gap(px(10.))
            .child(
                h_flex()
                    .items_start()
                    .gap_3()
                    .child(framed(Icon::new(glyph).with_size(px(16.)).text_color(parts::tone_ink(tone, &p)), cx))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .child(
                                div()
                                    .debug_selector(|| "pr-title".into())
                                    .text_size(px(15.))
                                    .line_height(px(22.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .text_color(p.foreground)
                                    .child(title),
                            )
                            .children(author.map(|author| {
                                h_flex()
                                    .debug_selector(|| "pr-by".into())
                                    .mt_1()
                                    .gap(px(4.))
                                    .text_size(px(13.))
                                    .line_height(px(20.))
                                    .text_color(p.muted_foreground)
                                    .child(parts::who(&author, px(16.), cx))
                                    .when(!opened.is_empty(), |el| {
                                        el.child(div().debug_selector(|| "pr-when".into()).child(words::opened(&opened)))
                                    })
                            })),
                    )
                    .child(h_flex().flex_none().gap_1().child(number_link).child(refresh)),
            )
            .child(
                h_flex()
                    .pl(px(44.))
                    .h(px(20.))
                    .min_w_0()
                    .justify_between()
                    .gap(px(14.))
                    .child(parts::state_word("pr-word", &word, tone, hollow, cx))
                    .children(branch.map(|branch| {
                        h_flex()
                            .debug_selector(|| "pr-branches".into())
                            .min_w_0()
                            .gap(px(6.))
                            .font_family(mono.clone())
                            .text_size(px(12.))
                            .line_height(px(20.))
                            .text_color(p.muted_foreground)
                            .child(Icon::new(Glyph::GitBranch).with_size(px(13.)).flex_none())
                            .child(div().min_w_0().truncate().child(format!("{base} \u{2190} {branch}")))
                    })),
            )
            .child(
                rest.children(page.filter(|p| !p.labels.is_empty()).map(|page| {
                    h_flex().debug_selector(|| "pr-labels".into()).flex_wrap().gap(px(6.)).children(page.labels.iter().enumerate().map(
                        |(ix, l)| {
                            let swatch = gpui_kit::Rgba::try_from(format!("#{}", l.color).as_str())
                                .map(gpui_kit::Hsla::from)
                                .unwrap_or(p.muted_foreground);
                            h_flex()
                                .id(SharedString::from(format!("pr-label-{ix}")))
                                .debug_selector(|| "pr-label".into())
                                .h(px(22.))
                                .gap(px(6.))
                                .px_2()
                                .rounded(px(6.))
                                .border_1()
                                .border_color(p.border)
                                .text_size(px(12.))
                                .text_color(p.foreground)
                                .child(div().size(px(7.)).rounded(px(2.)).bg(swatch))
                                .child(l.name.clone())
                                .when_some(l.description.clone().filter(|d| !d.is_empty()), |el, d| el.tooltip(tip(d)))
                        },
                    ))
                }))
                .children(page.filter(|p| !p.review_requests.is_empty()).map(|page| {
                    h_flex()
                        .debug_selector(|| "pr-asks".into())
                        .flex_wrap()
                        .gap(px(4.))
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(p.muted_foreground)
                        .child(format!("{} ", PR_WORDS.review_asked_of))
                        .children(page.review_requests.iter().enumerate().map(|(i, r)| {
                            let who =
                                if r.team { div().child(r.name.clone()).into_any_element() } else { parts::who(&r.name, px(16.), cx) };
                            h_flex().child(who).when(i + 1 < page.review_requests.len(), |el| el.child(","))
                        }))
                }))
                .children(self.status.as_ref().and_then(|s| s.from.clone()).and_then(|from| {
                    let (text, url) = match from {
                        model::From::Issue { number, title, url } => (words::from_issue(number, &title), url),
                        model::From::PullRequest { number, url } => (words::from_pull_request(number), url),
                        model::From::Review => return None,
                    };
                    Some(
                        div()
                            .id("pr-from")
                            .debug_selector(|| "pr-from".into())
                            .min_w_0()
                            .truncate()
                            .text_size(px(13.))
                            .text_color(p.muted_foreground)
                            .cursor_pointer()
                            .hover(|s| s.text_color(p.foreground))
                            .on_click(move |_, _, cx| open_web(&url, cx))
                            .child(text),
                    )
                }))
                .children(self.refusal.clone().map(|said| {
                    div()
                        .debug_selector(|| "pr-refusal".into())
                        .text_size(px(13.))
                        .line_height(px(20.))
                        .text_color(p.error_foreground)
                        .child(said)
                })),
            )
            .into_any_element()
    }

    fn tabs(&self, seen: &Seen, cx: &mut Context<Self>) -> AnyElement {
        let page = self.page.as_ref();
        let fact = seen.fact();
        let count = |n: Option<usize>| n.map(|n| format!(" {n}")).unwrap_or_default();
        let conversation = count(page.map(|_| self.derived.count));
        let commits = count(page.map(|p| p.commits.len()).or(fact.map(|f| f.commits as usize)));
        let files = count(page.map(|p| p.files.len()).or(fact.map(|f| f.changed_files as usize)));
        let picked = match self.tab {
            Tab::Conversation => 0,
            Tab::Commits => 1,
            Tab::Files => 2,
        };
        let me = cx.weak_entity();
        div()
            .debug_selector(|| "pr-tabs".into())
            .child(
                Segmented::new("pr-tabs", picked)
                    .fill()
                    .segment_size((px(13.), px(13.)), px(8.))
                    .segment(format!("{}{conversation}", PR_WORDS.conversation))
                    .segment(format!("{}{commits}", PR_WORDS.commits))
                    .segment(format!("{}{files}", PR_WORDS.files))
                    .on_pick(move |ix, _, cx| {
                        let tab = [Tab::Conversation, Tab::Commits, Tab::Files][ix];
                        _ = me.update(cx, |this, cx| this.pick_tab(tab, cx));
                    }),
            )
            .into_any_element()
    }
}

/// A reply as the person, into a thread under its first comment or with none a new comment at the conversation's
/// end; the page takes the comment GitHub answered with.
fn reply_on(me: &WeakEntity<PullRequestScreen>, body: String, into: Option<(u64, Option<String>)>, cx: &mut App) -> Sent {
    let Some(screen) = me.upgrade() else { return Task::ready(Err("The pane is closed.".into())) };
    let (host, workspace_id) = {
        let s = screen.read(cx);
        (s.host.clone(), s.opened.workspace_id.clone().unwrap_or_default())
    };
    let mut params = json!({ "workspaceId": workspace_id, "body": body });
    if let Some((root, thread)) = into {
        params["replyTo"] = json!(root);
        if let Some(thread) = thread {
            params["threadId"] = json!(thread);
        }
    }
    let asking = link::ask::<protocol::WorkspacesPullRequestReplyRequest>(&host, params);
    let me = me.clone();
    cx.spawn(async move |cx| {
        let done = asking.await.map_err(|f| f.message)?;
        _ = me.update(cx, |this, cx| {
            let Some(page) = this.page.as_mut() else { return };
            if let Ok(comment) = serde_json::from_value(done["comment"].clone()) {
                page.comments.push(comment);
            }
            if let Ok(line) = serde_json::from_value(done["reviewComment"].clone()) {
                page.review_comments.push(line);
            }
            this.page_changed(cx);
        });
        Ok(())
    })
}

/// A thread resolved or opened again as the person; it folds or opens once GitHub said so.
fn resolve_on(me: &WeakEntity<PullRequestScreen>, thread_id: String, resolved: bool, window: AnyWindowHandle, cx: &mut App) -> Sent {
    let Some(screen) = me.upgrade() else { return Task::ready(Err("The pane is closed.".into())) };
    let (host, workspace_id, name) = {
        let s = screen.read(cx);
        (s.host.clone(), s.opened.workspace_id.clone().unwrap_or_default(), s.name())
    };
    let params = json!({ "workspaceId": workspace_id, "threadId": thread_id, "resolved": resolved });
    let asking = link::ask::<protocol::WorkspacesPullRequestResolveRequest>(&host, params);
    let me = me.clone();
    cx.spawn(async move |cx| match asking.await {
        Ok(done) => {
            _ = me.update(cx, |this, cx| {
                let Some(page) = this.page.as_mut() else { return };
                let id = done["threadId"].as_str().unwrap_or_default();
                for c in page.review_comments.iter_mut().filter(|c| c.thread_id.as_deref() == Some(id)) {
                    c.resolved = Some(done["resolved"] == true);
                }
                this.page_changed(cx);
            });
            Ok(())
        }
        Err(failure) => {
            let said = failure.message.clone();
            _ = me.update(cx, |this, _| this.said = Some(said.clone()));
            _ = window.update(cx, |_, _, cx| ui::notify(cx, Notice::error(said).place(name)));
            Err(failure.message)
        }
    })
}

impl Render for PullRequestScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.weak_entity();
        let measure = canvas(
            move |bounds, _, cx| {
                _ = me.update(cx, |this, cx| {
                    this.top = bounds.origin.y;
                    if (this.width - bounds.size.width).abs() > px(0.5) {
                        this.width = bounds.size.width;
                        cx.notify();
                    }
                });
            },
            |_, _, _, _| {},
        )
        .absolute()
        .top_0()
        .left_0()
        .right_0()
        .h(px(1.));
        let width = self.width;
        let seen = self.seen().cloned();
        let body: Vec<AnyElement> = match seen {
            Some(Seen::Unread(why)) => {
                vec![div()
                    .px_0()
                    .py(px(8.))
                    .text_size(px(13.))
                    .line_height(px(20.))
                    .text_color(cx.palette().muted_foreground)
                    .child(why)
                    .into_any_element()]
            }
            Some(seen) if seen.number().is_some() => {
                let mut out = vec![self.head(&seen, cx), self.tabs(&seen, cx)];
                match self.tab {
                    Tab::Conversation => out.extend(self.conversation_tab(&seen, window, cx)),
                    Tab::Commits => out.push(match &self.page {
                        None => parts::commits_skeleton(width, cx),
                        Some(_) => self.commits_tab(window, cx),
                    }),
                    Tab::Files => out.push(match &self.page {
                        None => parts::files_skeleton(width),
                        Some(_) => self.files_tab(window, cx),
                    }),
                }
                out
            }
            _ => vec![parts::head_skeleton(width)],
        };
        let column = v_flex().relative().w_full().gap(px(16.)).child(measure).children(body);
        let content = div().px(px(16.)).pt(px(16.)).pb(px(20.)).child(column);
        ui::scroll_area("pr-pane", &self.scroll, content, window, cx).debug_selector(|| "pr-pane".into()).size_full()
    }
}
