// SPDX-License-Identifier: AGPL-3.0-only
//! The git button in the thread header, beside Open (`pull-request/GitSplit.tsx`, `gitAction.logic.ts`): the act the
//! checkout calls for on the main part, Commit, Push and the pull request in its menu. A commit takes every changed
//! file with the message the workspace's agent drafts, then brings the work back; where no draft comes back the
//! Changes pane opens on the uncommitted files so the person writes one. View PR opens the Pull request pane.
//!
//! The thread view mounts it in its header's actions: `cx.new(|cx| GitSplit::new(host, workspace_id, thread_id,
//! window, cx))`, and tells it each thread switch with [`GitSplit::show`].

use std::rc::Rc;

use futures::channel::mpsc;
use futures::StreamExt as _;
use gpui_kit::component::notification::Notification;
use gpui_kit::component::{h_flex, WindowExt as _};
use gpui_kit::{div, prelude::*, px, Context, Corners, SharedString, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol;

use super::link;
use super::model::{Checkout, Seen, State, Status};
use crate::screens::pr_pane::changes::{self, model::Scope};
use crate::screens::{navigate, Host, Route};
use crate::ui::{self, menu_on, tip, Button, Glyph, MenuAct};

/// The checkout's branch on a detached head (`DETACHED_HEAD`).
const DETACHED_HEAD: &str = "(detached)";

pub struct GitWords;

impl GitWords {
    pub const COMMIT: &'static str = "Commit";
    pub const COMMIT_PUSH_PR: &'static str = "Commit, push and PR";
    pub const COMMIT_PUSH: &'static str = "Commit and push";
    pub const PUSH: &'static str = "Push";
    pub const PUSH_CREATE_PR: &'static str = "Push and create PR";
    pub const CREATE_PR: &'static str = "Create PR";
    pub const VIEW_PR: &'static str = "View PR";
    pub const MENU: &'static str = "More git actions";
    pub const NO_BRANCH: &'static str = "The copy is on no branch. Check one out in the terminal to push it.";
    pub const NOTHING: &'static str = "Nothing to commit or push.";
    pub const UNREAD: &'static str = "The branch is not read yet.";
    pub const BUSY: &'static str = "A git action is running.";

    pub fn on_base(base: &str) -> String {
        format!("{base} is the branch the work goes back to; put the work on a branch of its own to push it.")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GitAct {
    CommitPushPr,
    CommitPush,
    Push,
    CreatePr,
    ViewPr,
}

/// What the button reads: the checkout, the pull request, the base, whether the copy opens a pull request of its own,
/// whether an act runs, and why the machine cannot be asked now.
#[derive(Clone, Debug, Default)]
pub struct GitState {
    pub checkout: Option<Checkout>,
    pub pr_open: bool,
    pub base: Option<String>,
    pub opens_pr: bool,
    pub busy: bool,
    pub away: Option<String>,
}

/// The main part: its label, the act it runs or none with why it is held, and the act a held part stands for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitQuick {
    pub label: String,
    pub act: Option<GitAct>,
    pub hint: Option<String>,
    pub holds: Option<GitAct>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuItemAct {
    Changes,
    Push,
    CreatePr,
    ViewPr,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitMenuItem {
    pub id: &'static str,
    pub label: &'static str,
    pub act: MenuItemAct,
    pub disabled: bool,
}

fn held(label: &str, hint: impl Into<String>) -> GitQuick {
    GitQuick { label: label.into(), act: None, hint: Some(hint.into()), holds: None }
}

fn branched(c: &Checkout) -> bool {
    !c.branch.is_empty() && c.branch != DETACHED_HEAD
}

/// The one act the checkout calls for (`gitQuickAction`).
pub fn quick_action(s: &GitState) -> GitQuick {
    let Some(checkout) = &s.checkout else { return held(GitWords::COMMIT, GitWords::UNREAD) };
    if !branched(checkout) {
        return held(GitWords::COMMIT, GitWords::NO_BRANCH);
    }
    let quick = pick(s, checkout);
    if let (Some(away), Some(act)) = (&s.away, quick.act) {
        if act != GitAct::ViewPr {
            return GitQuick { holds: Some(act), ..held(&quick.label, away.clone()) };
        }
    }
    if s.busy && quick.act.is_some() {
        return held(&quick.label, GitWords::BUSY);
    }
    quick
}

fn pick(s: &GitState, checkout: &Checkout) -> GitQuick {
    let go = |label: &str, act| GitQuick { label: label.into(), act: Some(act), hint: None, holds: None };
    if let Some(base) = s.base.as_deref().filter(|b| *b == checkout.branch) {
        return held(GitWords::COMMIT, GitWords::on_base(base));
    }
    if checkout.changed > 0 {
        return if !s.opens_pr || s.pr_open {
            go(GitWords::COMMIT_PUSH, GitAct::CommitPush)
        } else {
            go(GitWords::COMMIT_PUSH_PR, GitAct::CommitPushPr)
        };
    }
    if checkout.ahead > 0 {
        return if !s.opens_pr || s.pr_open { go(GitWords::PUSH, GitAct::Push) } else { go(GitWords::PUSH_CREATE_PR, GitAct::CreatePr) };
    }
    if !s.opens_pr {
        return held(GitWords::PUSH, GitWords::NOTHING);
    }
    if s.pr_open {
        return go(GitWords::VIEW_PR, GitAct::ViewPr);
    }
    go(GitWords::CREATE_PR, GitAct::CreatePr)
}

/// Commit, Push and the pull request, each held where it cannot run (`gitMenu`).
pub fn menu(s: &GitState) -> Vec<GitMenuItem> {
    let checkout = s.checkout.as_ref();
    let branched = checkout.is_some_and(branched);
    let changes = checkout.is_some_and(|c| c.changed > 0);
    let ahead = checkout.is_some_and(|c| c.ahead > 0);
    let on_base = s.base.is_some() && checkout.map(|c| c.branch.as_str()) == s.base.as_deref();
    let stopped = s.busy || s.away.is_some();
    let mut items = vec![
        GitMenuItem { id: "commit", label: GitWords::COMMIT, act: MenuItemAct::Changes, disabled: stopped || !changes },
        GitMenuItem {
            id: "push",
            label: GitWords::PUSH,
            act: MenuItemAct::Push,
            disabled: stopped || !branched || on_base || !ahead || (s.opens_pr && !s.pr_open),
        },
    ];
    if !s.opens_pr {
        return items;
    }
    items.push(if s.pr_open {
        GitMenuItem { id: "pr", label: GitWords::VIEW_PR, act: MenuItemAct::ViewPr, disabled: false }
    } else {
        GitMenuItem {
            id: "pr",
            label: GitWords::CREATE_PR,
            act: MenuItemAct::CreatePr,
            disabled: stopped || !branched || changes || on_base,
        }
    });
    items
}

pub struct GitSplit {
    host: Host,
    workspace_id: String,
    thread_id: Option<String>,
    status: Option<Value>,
    base: Option<String>,
    busy: bool,
    _follow: Task<()>,
    _base: Option<Task<()>>,
}

impl GitSplit {
    pub fn new(host: Host, workspace_id: String, thread_id: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let follow = Self::follow(&host, workspace_id.clone(), window, cx);
        GitSplit { host, workspace_id, thread_id, status: None, base: None, busy: false, _follow: follow, _base: None }
    }

    /// The button on another thread: its workspace's status is followed instead.
    pub fn show(&mut self, workspace_id: String, thread_id: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        self.thread_id = thread_id;
        if workspace_id != self.workspace_id {
            self.workspace_id = workspace_id.clone();
            self.status = None;
            self.base = None;
            self._base = None;
            self._follow = Self::follow(&self.host, workspace_id, window, cx);
        }
        cx.notify();
    }

    fn follow(host: &Host, workspace_id: String, window: &mut Window, cx: &mut Context<Self>) -> Task<()> {
        let (tx, mut rx) = mpsc::unbounded();
        let road = link::follow(host, workspace_id, tx);
        cx.spawn_in(window, async move |this, cx| {
            let pushes = async {
                while let Some(status) = rx.next().await {
                    if this.update(cx, |this, cx| this.take_status(status, cx)).is_err() {
                        break;
                    }
                }
            };
            futures::join!(pushes, async { _ = road.await });
        })
    }

    /// The branch the work goes back to: the project's base, else its default branch.
    fn read_base(host: &Host, project: String, cx: &mut Context<Self>) -> Task<()> {
        let reading = host.run(move |client| async move {
            Ok(client.projects().await?.into_iter().find(|p| p.id == project).and_then(|p| {
                let p = serde_json::to_value(p).unwrap_or_default();
                p["base"].as_str().or_else(|| p["defaultBranch"].as_str()).map(str::to_owned)
            }))
        });
        cx.spawn(async move |this, cx| {
            if let Ok(base) = reading.await {
                _ = this.update(cx, |this, cx| {
                    this.base = base;
                    cx.notify();
                });
            }
        })
    }

    /// A status the host pushed; the first names the project whose base the button reads.
    pub fn take_status(&mut self, status: Value, cx: &mut Context<Self>) {
        if self._base.is_none() {
            if let Some(project) = status.pointer("/project/id").and_then(Value::as_str) {
                self._base = Some(Self::read_base(&self.host, project.to_owned(), cx));
            }
        }
        self.status = Some(status);
        cx.notify();
    }

    pub fn state(&self) -> Option<GitState> {
        let value = self.status.as_ref()?;
        let status = Status::read(value);
        let machine = crate::screens::sidebar::menu::workspace_state(value, Some(value));
        let away_word = if matches!(machine, "waking" | "unreachable" | "waiting") { "git action" } else { "push" };
        let pr_open = matches!(&status.pr, Some(seen) if seen.number().is_some() && seen.state() == State::Open);
        Some(GitState {
            checkout: status.checkout.clone(),
            pr_open,
            base: self.base.clone(),
            opens_pr: status.opens_pr,
            busy: self.busy,
            away: crate::screens::sidebar::menu::action_refusal(machine, away_word),
        })
    }

    /// Whether the button stands at all: a copy that is gone keeps it only for an open pull request's View PR.
    fn shown(&self) -> bool {
        let Some(value) = &self.status else { return false };
        let machine = crate::screens::sidebar::menu::workspace_state(value, Some(value));
        let pr_open = matches!(Status::read(value).pr, Some(Seen::Fact(ref f)) if f.state == State::Open)
            || matches!(Status::read(value).pr, Some(Seen::Kept(ref k)) if k.state == State::Open);
        value.get("checkout").is_some_and(|c| !c.is_null()) && machine != "released" && (machine != "gone" || pr_open)
    }

    pub fn run(&mut self, act: GitAct, window: &mut Window, cx: &mut Context<Self>) {
        if act == GitAct::ViewPr {
            navigate(Route::Screen("pull-request".into()), cx);
            return;
        }
        if self.busy {
            return;
        }
        self.busy = true;
        cx.notify();
        let commits = matches!(act, GitAct::CommitPushPr | GitAct::CommitPush);
        let host = self.host.clone();
        let workspace_id = self.workspace_id.clone();
        let thread_id = self.thread_id.clone();
        let name = self.status.as_ref().and_then(|s| s["name"].as_str()).unwrap_or_default().to_owned();
        cx.spawn_in(window, async move |this, cx| {
            let ran: Result<(), String> = async {
                if commits {
                    let mut draft = json!({ "workspaceId": workspace_id });
                    if let Some(t) = &thread_id {
                        draft["threadId"] = json!(t);
                    }
                    let drafted = link::ask::<protocol::WorkspacesCommitDraftRequest>(&host, draft).await.map_err(|f| f.message)?;
                    let Some(message) = drafted["message"].as_str().filter(|m| !m.is_empty()).map(str::to_owned) else {
                        if let Some(note) = drafted["note"].as_str() {
                            let note = note.to_owned();
                            _ = cx.update(|window, cx| window.push_notification(Notification::info(note), cx));
                        }
                        let (w, t) = (workspace_id.clone(), thread_id.clone());
                        _ = cx.update(|_, cx| changes::open_on(&w, t.as_deref(), Scope::Head, cx));
                        return Ok(());
                    };
                    link::ask::<protocol::WorkspacesCommitRequest>(&host, json!({ "workspaceId": workspace_id, "message": message }))
                        .await
                        .map_err(|f| f.message)?;
                }
                link::ask::<protocol::WorkspacesBringBackRequest>(&host, json!({ "workspaceId": workspace_id }))
                    .await
                    .map_err(|f| f.message)?;
                Ok(())
            }
            .await;
            if let Err(said) = ran {
                let said = if name.is_empty() { said } else { format!("{name}: {said}") };
                _ = cx.update(|window, cx| window.push_notification(Notification::error(said), cx));
            }
            _ = this.update(cx, |this, cx| {
                this.busy = false;
                cx.notify();
            });
        })
        .detach();
    }

    fn pick(&mut self, item: MenuItemAct, window: &mut Window, cx: &mut Context<Self>) {
        match item {
            MenuItemAct::Changes => changes::open_on(&self.workspace_id, self.thread_id.as_deref(), Scope::Head, cx),
            MenuItemAct::Push => self.run(GitAct::Push, window, cx),
            MenuItemAct::CreatePr => self.run(GitAct::CreatePr, window, cx),
            MenuItemAct::ViewPr => self.run(GitAct::ViewPr, window, cx),
        }
    }
}

fn glyph_of(act: Option<GitAct>) -> Glyph {
    match act {
        Some(GitAct::ViewPr | GitAct::CreatePr) => Glyph::GitPullRequest,
        Some(GitAct::Push) => Glyph::Upload,
        _ => Glyph::GitCommitHorizontal,
    }
}

impl Render for GitSplit {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(state) = self.state().filter(|_| self.shown()) else { return div().into_any_element() };
        let quick = quick_action(&state);
        let shown = quick.act.or(quick.holds);
        let me = cx.weak_entity();
        let act = quick.act;
        let left =
            Corners { top_left: ui::size::BUTTON_RADIUS, top_right: px(0.), bottom_right: px(0.), bottom_left: ui::size::BUTTON_RADIUS };
        let right =
            Corners { top_left: px(0.), top_right: ui::size::BUTTON_RADIUS, bottom_right: ui::size::BUTTON_RADIUS, bottom_left: px(0.) };
        let main = Button::outline("git-quick", quick.label.clone()).glyph(glyph_of(shown)).corners(left).held(act.is_none()).on_click(
            move |_, window, cx| {
                if let Some(act) = act {
                    _ = me.update(cx, |this, cx| this.run(act, window, cx));
                }
            },
        );
        let hint: SharedString = quick.hint.clone().unwrap_or_else(|| quick.label.clone()).into();
        let acts: Vec<MenuAct> = menu(&state)
            .into_iter()
            .map(|item| {
                let me = cx.weak_entity();
                let run: ui::act::Run = Rc::new(move |window, cx| {
                    _ = me.update(cx, |this, cx| this.pick(item.act, window, cx));
                });
                let glyph = match item.id {
                    "commit" => Glyph::GitCommitHorizontal,
                    "push" => Glyph::Upload,
                    _ => Glyph::GitPullRequest,
                };
                MenuAct {
                    group: 0,
                    glyph: Some(glyph),
                    title: item.label.into(),
                    destructive: false,
                    held: item.disabled,
                    why: None,
                    keys: None,
                    run,
                }
            })
            .collect();
        let more = Button::outline("git-more", "").glyph(Glyph::ChevronDown).corners(right).tooltip(GitWords::MENU);
        h_flex()
            .debug_selector(|| "git-split".into())
            .flex_none()
            .child(div().id("git-quick-hover").tooltip(tip(hint)).child(main))
            .child(div().ml(px(-1.)).child(menu_on("git-more", more, gpui_kit::Anchor::TopRight, Rc::new(acts))))
            .into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state(branch: &str, changed: i64, ahead: i64, pr_open: bool) -> GitState {
        GitState {
            checkout: Some(Checkout { branch: branch.into(), ahead, behind: 0, changed }),
            pr_open,
            base: Some("main".into()),
            opens_pr: true,
            busy: false,
            away: None,
        }
    }

    #[test]
    fn the_main_part_names_the_act_the_checkout_calls_for() {
        assert_eq!(quick_action(&state("work", 2, 0, false)).label, "Commit, push and PR");
        assert_eq!(quick_action(&state("work", 2, 0, true)).act, Some(GitAct::CommitPush));
        assert_eq!(quick_action(&state("work", 0, 1, false)).label, "Push and create PR");
        assert_eq!(quick_action(&state("work", 0, 0, true)).act, Some(GitAct::ViewPr));
        assert_eq!(quick_action(&state("work", 0, 0, false)).act, Some(GitAct::CreatePr));
        let on_base = quick_action(&state("main", 2, 0, false));
        assert_eq!((on_base.act, on_base.hint.as_deref()), (None, Some(GitWords::on_base("main").as_str())));
        let mut away = state("work", 2, 0, false);
        away.away = Some("Workspace is paused; wake it to push".into());
        let held = quick_action(&away);
        assert_eq!((held.act, held.holds), (None, Some(GitAct::CommitPushPr)));
        let mut busy = state("work", 2, 0, false);
        busy.busy = true;
        assert_eq!(quick_action(&busy).hint.as_deref(), Some(GitWords::BUSY));
    }

    #[test]
    fn the_menu_holds_what_cannot_run() {
        let items = menu(&state("work", 0, 1, true));
        assert_eq!(
            items.iter().map(|i| (i.label, i.disabled)).collect::<Vec<_>>(),
            [("Commit", true), ("Push", false), ("View PR", false)]
        );
        let none = menu(&state("work", 2, 0, false));
        assert_eq!(
            none.iter().map(|i| (i.label, i.disabled)).collect::<Vec<_>>(),
            [("Commit", false), ("Push", true), ("Create PR", true)]
        );
    }
}
