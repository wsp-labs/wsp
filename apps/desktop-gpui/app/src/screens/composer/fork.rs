// SPDX-License-Identifier: AGPL-3.0-only
//! Fork from here off the thread view, as ForkDialog.tsx and forks.ts have it: where a new branch can be offered the
//! branch question asks first, and the draft opens on the workspace's New thread page with its own box, under the
//! fork's banner, its pickers on the source's agent, model and effort, holding the person's message and its images
//! where the fork came off one. Nothing reaches the host until the draft's send.

use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::dialog::DialogEntrance;
use gpui_kit::component::{v_flex, WindowExt as _};
use gpui_kit::{canvas, div, prelude::*, px, App, Entity, Focusable as _, Global, Pixels, SharedString, WeakEntity, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{SessionEvent, SessionsAttachmentRequest};

use super::link::{call, daemon};
use super::model::{File, Picks};
use super::{Composer, Fork};
use crate::screens::{navigate, Host, Route};
use crate::transcript::FileTile;
use crate::ui::{word, Button, Card, Choice, Dialog, Glyph, WordClass};

const DAEMON_VERSIONS: &str = include_str!("../../../../../../packages/protocol/src/wire/daemon-version.ts");

/// `FORK_BRANCH_DAEMON_VERSION`: the first daemon that cuts a worktree at a turn's checkpoint, read from its source.
fn branch_daemon_version() -> u64 {
    let at = DAEMON_VERSIONS.find("FORK_BRANCH_DAEMON_VERSION =").map(|at| &DAEMON_VERSIONS[at..]).unwrap_or_default();
    at.split(['=', ';']).nth(1).and_then(|n| n.trim().parse().ok()).unwrap_or(u64::MAX)
}

/// The words of thread.ts the fork says.
pub mod words {
    pub const TITLE: &str = "Fork from here";
    pub const FOLDER: &str = "This folder";
    pub const BRANCH: &str = "New branch";
    pub const FORK: &str = "Fork";
    pub const CANCEL: &str = "Cancel";
    pub const BRANCH_NOTE: &str = "A worktree with the files as this turn left them.";
    pub const BRANCH_HEAD_NOTE: &str = "A worktree at today's commit, with this turn's files on top.";
    /// The banner's dismiss, by name (ChatComposer.tsx).
    pub const DISMISS: &str = "Cancel the fork";

    pub fn of_line(title: &str) -> String {
        format!("Fork of {title}")
    }

    pub fn dialog_line(title: &str) -> String {
        format!("A new thread with this conversation up to here. {title} does not change.")
    }

    pub fn folder_note(title: &str) -> String {
        format!("The files as they are now, shared with {title}.")
    }

    pub fn where_line(branch: Option<&str>) -> String {
        match branch {
            None => "in this folder".to_owned(),
            Some(branch) => format!("on {branch}"),
        }
    }

    /// `forkBranchName`: the source's branch with -fork, numbered past every name taken, or the thread's short id on
    /// a detached head.
    pub fn branch_name(source: Option<&str>, taken: &[String], thread: &str) -> String {
        let base = match source.filter(|s| !s.is_empty()) {
            Some(source) => format!("{source}-fork"),
            None => format!("fork-{}", thread.chars().take(8).collect::<String>()),
        };
        if !taken.contains(&base) {
            return base;
        }
        (2..).map(|n| format!("{base}-{n}")).find(|name| !taken.contains(name)).unwrap_or(base)
    }
}

/// Fork from here pressed on a thread: its workspace and id, its title, the turn the fork carries, the source's
/// picks, and on a person's message its words and file tiles.
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub workspace: String,
    pub thread: String,
    pub title: String,
    pub turn: Option<String>,
    pub picks: Picks,
    pub message: Option<(String, Vec<FileTile>)>,
}

/// What the forked turn's checkpoint offers a new branch: whether it stands on the commit the turn ended on, and
/// whether a later turn changed files, so this turn's files are not the folder's any more (forks.ts `ForkPick`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Kept {
    pub based: bool,
    pub later_changed: bool,
}

/// The forked turn's checkpoint, where it kept one, off the thread's events: a later turn's changes come after it.
pub fn kept(events: &[SessionEvent], turn: &str) -> Option<Kept> {
    let (at, based) = events.iter().enumerate().find_map(|(at, e)| match e {
        SessionEvent::SessionCheckpoint { turn_id, ref_: Some(_), based, .. } if turn_id.as_deref() == Some(turn) => {
            Some((at, *based == Some(true)))
        }
        _ => None,
    })?;
    let later_changed = events[at..]
        .iter()
        .any(|e| matches!(e, SessionEvent::SessionChanges { turn_id, files, .. } if turn_id.as_deref() != Some(turn) && !files.is_empty()));
    Some(Kept { based, later_changed })
}

/// A fork's draft on its way to the New thread page, which takes it as it opens.
#[derive(Clone, Debug, PartialEq)]
pub struct Draft {
    pub asked: Asked,
    pub branch: Option<String>,
}

#[derive(Default)]
struct Pending(HashMap<String, Draft>);

impl Global for Pending {}

/// The workspace's fork draft, once: the New thread page opening on `workspace` takes it.
pub fn take(workspace: &str, cx: &mut App) -> Option<Draft> {
    cx.try_global::<Pending>()?;
    cx.global_mut::<Pending>().0.remove(workspace)
}

/// Fork from here, at the press: the branch question where the forked turn kept a checkpoint and the thread runs in a
/// git folder on this computer whose daemon cuts worktrees; else the draft at once. The sidebar's listing holds what that
/// takes, so nothing waits on the host while the person may move on.
pub fn ask(host: &Host, asked: Asked, kept: Option<Kept>, cwd: Option<String>, window: &mut Window, cx: &mut App) {
    let from =
        kept.and_then(|_| crate::screens::terminal::workspaces_sidebar(cx)).and_then(|s| branch_from(s.read(cx), &asked.workspace, cwd));
    match (kept, from) {
        (Some(kept), Some(from)) => {
            let (workspace, thread) = (asked.workspace.clone(), asked.thread.clone());
            let q = question(asked, kept, window, cx);
            read_branch(host, &q, workspace, thread, from, cx);
        }
        _ => open_draft(asked, None, cx),
    }
}

/// The folder a new branch is cut from (ChatView.tsx `branchFolder`): the thread's own on a workspace that is a copy
/// of a git project on this computer, whose daemon cuts worktrees at a checkpoint; none where the fork can only run in
/// its thread's folder.
pub fn branch_from(sidebar: &crate::sidebar::Sidebar, workspace: &str, cwd: Option<String>) -> Option<String> {
    let w = sidebar.workspace(workspace).0?;
    if w.get("kind").and_then(Value::as_str) != Some("local") {
        return None;
    }
    let git = sidebar.project_git(w.pointer("/project/id").and_then(Value::as_str)?);
    let cuts = sidebar.place("here").and_then(|p| p.get("daemonVersion")?.as_u64()).is_some_and(|v| v >= branch_daemon_version());
    if !git || !cuts {
        return None;
    }
    cwd.or_else(|| w.get("folder").and_then(Value::as_str).map(str::to_owned))
}

/// The pick the branch question stands on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    Folder,
    Branch,
}

/// The open branch question: the fork asked for, the pick, and the branch the worktree would take once read.
pub struct Question {
    asked: Asked,
    based: bool,
    pick: Pick,
    branch: Option<String>,
    height: Option<Pixels>,
}

struct Open(WeakEntity<Question>);

impl Global for Open {}

/// What the open branch question shows, for a test.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub line: String,
    pub pick: Pick,
    /// The branch the New branch pick names, once read.
    pub branch: Option<String>,
    pub branch_note: String,
    /// Fork waits while New branch is picked and its name is not read yet.
    pub held: bool,
}

pub fn shown(cx: &App) -> Option<Shown> {
    let q = cx.try_global::<Open>()?.0.upgrade()?;
    let q = q.read(cx);
    Some(Shown {
        line: words::dialog_line(&q.asked.title),
        pick: q.pick,
        branch: q.branch.clone(),
        branch_note: if q.based { words::BRANCH_NOTE } else { words::BRANCH_HEAD_NOTE }.to_owned(),
        held: q.held(),
    })
}

impl Question {
    fn held(&self) -> bool {
        self.pick == Pick::Branch && self.branch.is_none()
    }
}

/// Opens the branch question, New branch picked where a later turn changed files.
fn question(asked: Asked, kept: Kept, window: &mut Window, cx: &mut App) -> Entity<Question> {
    let pick = if kept.later_changed { Pick::Branch } else { Pick::Folder };
    let q = cx.new(|_| Question { asked, based: kept.based, pick, branch: None, height: None });
    cx.set_global(Open(q.downgrade()));
    let shown = q.clone();
    window.open_dialog(cx, move |dialog, window, cx| {
        let height = shown.read(cx).height.unwrap_or(px(280.));
        dialog
            .close_button(false)
            .entrance(DialogEntrance::Fade)
            .w(px(440.))
            .margin_top(((window.viewport_size().height - height) / 2.).max(px(16.)))
            .p_0()
            .gap_0()
            .min_h(px(0.))
            .border_0()
            .bg(gpui_kit::transparent_black())
            .child(body(&shown, cx))
    });
    q
}

/// Reads the branch the worktree would take off the checkout's branches (`gitBranches`), the thread's short id where
/// the read fails.
fn read_branch(host: &Host, q: &Entity<Question>, workspace: String, thread: String, from: String, cx: &mut App) {
    let (reading, named) = (host.clone(), q.downgrade());
    cx.spawn(async move |cx| {
        let read = daemon(&reading, &workspace, json!({ "op": "git.branches", "cwd": from })).await.ok();
        let current = read.as_ref().and_then(|r| r.get("current")?.as_str().map(str::to_owned));
        let taken: Vec<String> = read
            .as_ref()
            .and_then(|r| r.get("branches")?.as_array().cloned())
            .unwrap_or_default()
            .iter()
            .filter_map(|b| b.get("name")?.as_str().map(str::to_owned))
            .collect();
        let name = words::branch_name(current.as_deref(), &taken, &thread);
        _ = named.update(cx, |q, cx| {
            q.branch = Some(name);
            cx.notify();
        });
    })
    .detach();
}

fn body(q: &Entity<Question>, cx: &App) -> impl IntoElement {
    let r = q.read(cx);
    let choice = |id: &'static str, pick: Pick, name: &'static str, glyph: Glyph, note: String| {
        let q = q.clone();
        Choice::new(id, name, r.pick == pick).glyph(glyph).note(note).on_click(move |_, _, cx| {
            q.update(cx, |q, cx| {
                q.pick = pick;
                cx.notify();
            })
        })
    };
    let branch_note = if r.based { words::BRANCH_NOTE } else { words::BRANCH_HEAD_NOTE }.to_owned();
    let branch = choice("fork-branch", Pick::Branch, words::BRANCH, Glyph::GitBranch, branch_note);
    let branch = match &r.branch {
        Some(name) => branch.slot(word(SharedString::from(name.clone()), WordClass::Fact, cx)),
        None => branch,
    };
    let picks = Card::new("fork-picks")
        .item(choice("fork-folder", Pick::Folder, words::FOLDER, Glyph::Folder, words::folder_note(&r.asked.title)))
        .item(branch);
    let going = q.clone();
    let dialog = Dialog::new(words::TITLE)
        .sheet()
        .line(words::dialog_line(&r.asked.title))
        .card(picks)
        .act(Button::outline("fork-cancel", words::CANCEL).neutral_ring().on_click(|_, window, cx| window.close_dialog(cx)))
        .act(Button::primary("fork-confirm", words::FORK).held(r.held()).on_click(move |_, window, cx| {
            let q = going.read(cx);
            if q.held() {
                return;
            }
            let (asked, branch) = (q.asked.clone(), q.branch.clone().filter(|_| q.pick == Pick::Branch));
            window.close_dialog(cx);
            open_draft(asked, branch, cx);
        }));
    let measured = q.clone();
    div().debug_selector(|| "fork-dialog".into()).relative().child(v_flex().child(dialog)).child(
        canvas(
            |_, _, _| (),
            move |bounds, _, _, cx| {
                let height = bounds.size.height;
                if measured.read(cx).height != Some(height) {
                    let measured = measured.clone();
                    cx.defer(move |cx| {
                        measured.update(cx, |q, cx| {
                            q.height = Some(height);
                            cx.notify();
                        })
                    });
                }
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size_full(),
    )
}

/// The draft opens on the workspace's New thread page (`openForkDraft`), which takes it as it mounts.
pub fn open_draft(asked: Asked, branch: Option<String>, cx: &mut App) {
    let workspace = asked.workspace.clone();
    cx.default_global::<Pending>().0.insert(workspace.clone(), Draft { asked, branch });
    navigate(Route::Screen(format!("{}:{workspace}", <super::screen::ComposerScreen as crate::screens::Screen>::NAME).into()), cx);
}

impl Composer {
    /// The fork's draft takes this New thread page's box: the fork's banner over it, the source's picks, and on a
    /// person's message its words in place of the page's own, which are kept aside as the page's draft and put back
    /// on Cancel the fork; off a reply the page's words stay and go with the fork.
    pub fn fork(&mut self, draft: &Draft, window: &mut Window, cx: &mut gpui_kit::Context<Self>) {
        let Draft { asked, branch } = draft;
        if let Some((text, _)) = &asked.message {
            if self.forking.is_none() {
                self.before_fork = Some(self.prompt(cx));
            }
            self.set_text(text, window, cx);
        }
        self.bar = None;
        self.popup = None;
        self.picks = Picks {
            agent: asked.picks.agent.clone(),
            model: asked.picks.model.clone(),
            effort: asked.picks.effort.clone(),
            ..Picks::default()
        };
        if branch.is_some() {
            self.branch = branch.clone();
        }
        self.banner = Some(super::Banner {
            fork: true,
            line: words::of_line(&asked.title),
            note: words::where_line(branch.as_deref()),
            resume: None,
        });
        self.forking = Some(Fork {
            workspace: asked.workspace.clone(),
            thread: asked.thread.clone(),
            turn: asked.turn.clone(),
            branch: branch.clone(),
        });
        self.editor.focus_handle(cx).focus(window, cx);
        cx.notify();
    }

    /// Images read back for the fork's draft, which join it only while that draft still holds the box.
    fn fork_images(&mut self, fork: &Fork, images: Vec<File>, cx: &mut gpui_kit::Context<Self>) {
        if self.forking.as_ref() == Some(fork) && !images.is_empty() {
            self.files.extend(images);
            cx.notify();
        }
    }
}

/// The message's images, read off the host that keeps them, into the fork's draft: a file that was no image reached
/// the thread's folder and is not kept.
pub fn read_images(host: &Host, draft: &Draft, composer: &Entity<Composer>, cx: &mut App) {
    let Some((_, tiles)) = &draft.asked.message else { return };
    let Some(fork) = composer.read(cx).forking().cloned() else { return };
    let asks: Vec<(String, String, SessionsAttachmentRequest)> = tiles
        .iter()
        .enumerate()
        .filter(|(_, t)| t.image)
        .filter_map(|(i, t)| {
            let id = t.id.clone()?;
            let name = if t.name.is_empty() { format!("image {}", i + 1) } else { t.name.clone() };
            let ask = SessionsAttachmentRequest {
                workspace_id: draft.asked.workspace.clone(),
                thread_id: draft.asked.thread.clone(),
                request_id: id,
                index: t.index as u64,
            };
            Some((name, t.mime.clone().unwrap_or_default(), ask))
        })
        .collect();
    if asks.is_empty() {
        return;
    }
    let (host, composer) = (host.clone(), composer.downgrade());
    cx.spawn(async move |cx| {
        let mut images = Vec::new();
        for (name, mime, ask) in asks {
            let Ok(reply) = call(&host, ask).await else { continue };
            let Some(bytes) = crate::screens::thread_view::cards::unbase64(&reply.attachment.bytes) else { continue };
            let media_type = if reply.attachment.media_type.is_empty() { mime } else { reply.attachment.media_type.clone() };
            images.push(File { name, media_type, bytes: Arc::new(bytes) });
        }
        _ = composer.update(cx, |c, cx| c.fork_images(&fork, images, cx));
    })
    .detach();
}

/// Cancel the fork: the page's own words come back and the window goes back to the thread the fork came off.
pub fn cancelled(fork: &Fork, cx: &mut App) {
    navigate(Route::Thread { workspace_id: fork.workspace.clone(), thread_id: fork.thread.clone() }, cx);
}

#[cfg(test)]
mod tests {
    use gpui_kit::component::Root;
    use gpui_kit::{Modifiers, TestAppContext, VisualTestContext};

    use super::*;

    struct Page;

    impl Render for Page {
        fn render(&mut self, _: &mut Window, _: &mut gpui_kit::Context<Self>) -> impl IntoElement {
            div().size_full()
        }
    }

    fn event(v: Value) -> SessionEvent {
        serde_json::from_value(v).unwrap()
    }

    fn asked() -> Asked {
        Asked {
            workspace: "w".into(),
            thread: "t".into(),
            title: "Fix the scroll".into(),
            turn: Some("u1".into()),
            picks: Picks::default(),
            message: None,
        }
    }

    fn press(cx: &mut VisualTestContext, selector: &'static str) {
        let at = cx.debug_bounds(selector).unwrap_or_else(|| panic!("{selector} was not drawn")).center();
        cx.simulate_click(at, Modifiers::none());
        cx.run_until_parked();
    }

    #[test]
    fn a_turn_offers_a_branch_only_where_its_checkpoint_kept_a_ref() {
        let base = |kind: &str, turn: &str, more: Value| {
            let mut v = json!({ "type": kind, "threadId": "t", "workspaceId": "w", "sessionId": "s", "turnId": turn, "at": 1.0 });
            v.as_object_mut().unwrap().extend(more.as_object().unwrap().clone());
            event(v)
        };
        let events = vec![
            base("session.checkpoint", "u1", json!({ "ref": "c1", "based": true })),
            base("session.checkpoint", "u2", json!({})),
            base(
                "session.changes",
                "u2",
                json!({ "from": "a", "to": "b", "moved": [], "files": [{ "path": "a.rs", "kind": "modified", "additions": 1, "deletions": 0 }] }),
            ),
        ];
        assert_eq!(kept(&events, "u1"), Some(Kept { based: true, later_changed: true }));
        assert_eq!(kept(&events, "u2"), None, "a turn that kept no files offers no new branch");
        assert_eq!(kept(&events[..2], "u1"), Some(Kept { based: true, later_changed: false }));
    }

    #[gpui_kit::test]
    fn the_branch_question_holds_fork_until_the_branch_is_named_and_opens_the_draft_with_it(cx: &mut TestAppContext) {
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
        });
        let page = cx.new(|_| Page);
        let (_, cx) = cx.add_window_view(|window, cx| Root::new(page.clone(), window, cx));
        let q = cx.update(|window, cx| question(asked(), Kept { based: false, later_changed: true }, window, cx));
        cx.run_until_parked();
        let open = cx.update(|_, cx| shown(cx)).expect("the branch question is open");
        assert_eq!(open.line, "A new thread with this conversation up to here. Fix the scroll does not change.");
        assert_eq!((open.pick, open.held), (Pick::Branch, true), "a later turn changed files: New branch, held until named");
        assert_eq!(open.branch_note, words::BRANCH_HEAD_NOTE);
        press(cx, "dialog-act-1");
        assert!(cx.update(|_, cx| take("w", cx)).is_none(), "a held Fork opened a draft");
        q.update(cx, |q, cx| {
            q.branch = Some("main-fork".into());
            cx.notify();
        });
        cx.run_until_parked();
        press(cx, "dialog-act-1");
        let draft = cx.update(|_, cx| take("w", cx)).expect("Fork opened the draft");
        assert_eq!(draft.branch.as_deref(), Some("main-fork"));
        assert!(!cx.update(|window, cx| window.has_active_dialog(cx)), "the question stayed open after Fork");

        let q = cx.update(|window, cx| question(asked(), Kept { based: true, later_changed: false }, window, cx));
        q.update(cx, |q, _| q.branch = Some("main-fork".into()));
        cx.run_until_parked();
        assert_eq!(cx.update(|_, cx| shown(cx)).map(|s| (s.pick, s.branch_note)), Some((Pick::Folder, words::BRANCH_NOTE.to_owned())));
        press(cx, "dialog-act-1");
        assert_eq!(cx.update(|_, cx| take("w", cx)).map(|d| d.branch), Some(None), "This folder forks with no branch");

        cx.update(|window, cx| _ = question(asked(), Kept { based: true, later_changed: false }, window, cx));
        cx.run_until_parked();
        press(cx, "dialog-act-0");
        assert!(cx.update(|_, cx| take("w", cx)).is_none(), "Cancel opened a draft");
        assert!(!cx.update(|window, cx| window.has_active_dialog(cx)), "Cancel left the question open");
    }

    const THREAD_WORDS: &str = include_str!("../../../../../../packages/protocol/src/words/thread.ts");
    const COMPOSER: &str = include_str!("../../../../../web/src/components/chat/ChatComposer.tsx");

    #[test]
    fn the_fork_says_what_thread_ts_says() {
        for said in [
            "`Fork of ${title}`",
            "`A new thread with this conversation up to here. ${title} does not change.`",
            "`The files as they are now, shared with ${title}.`",
            "\"A worktree with the files as this turn left them.\"",
            "\"A worktree at today's commit, with this turn's files on top.\"",
            "\"in this folder\" : `on ${branch}`",
            "folder: \"This folder\", branch: \"New branch\", fork: \"Fork\", title: \"Fork from here\"",
        ] {
            assert!(THREAD_WORDS.contains(said), "thread.ts no longer says {said}");
        }
        assert!(COMPOSER.contains("dismiss: \"Cancel the fork\""));
        assert_eq!(words::of_line("t"), "Fork of t");
        assert_eq!(words::where_line(Some("main-fork")), "on main-fork");
        assert_eq!(words::dialog_line("t"), "A new thread with this conversation up to here. t does not change.");
    }

    #[test]
    fn a_forks_branch_is_named_as_fork_branch_name_names_it() {
        let taken = vec!["main-fork".to_owned(), "main-fork-2".to_owned()];
        assert_eq!(words::branch_name(Some("main"), &[], "abcdef123"), "main-fork");
        assert_eq!(words::branch_name(Some("main"), &taken, "abcdef123"), "main-fork-3");
        assert_eq!(words::branch_name(None, &[], "abcdef123"), "fork-abcdef12");
        assert!(branch_daemon_version() < u64::MAX, "FORK_BRANCH_DAEMON_VERSION read from its source");
    }
}
