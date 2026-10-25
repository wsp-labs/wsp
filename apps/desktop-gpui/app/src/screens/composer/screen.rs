// SPDX-License-Identifier: AGPL-3.0-only
//! The composer alone in the main column, as `desktop-gpui --screen composer` opens it for captures: at the foot of a
//! thread column on the page's ground, on the thread the route opens, else the host's newest. `composer:<workspace>`
//! opens it on a new thread there, as New thread does, and holds a fork's draft there when Fork from here opened it.

use std::cell::Cell;
use std::rc::Rc;

use gpui_kit::component::v_flex;
use gpui_kit::{
    div, prelude::*, px, App, Context, Entity, FocusHandle, KeyDownEvent, SharedString, StyleRefinement, Subscription, Task, Window,
};
use serde_json::Value;
use wsp_host_client::protocol::ProjectsBranchRequest;
use wsp_host_client::Failure;

use super::hero::{self, HeroField};
use super::link::{Link, Target};
use super::model::Hold;
use super::Composer;
use crate::screens::{Host, Opened, Parts, Place, Screen};
use crate::ui::ActivePalette;

pub struct ComposerScreen {
    composer: Entity<Composer>,
    /// The column's own focus, which a key typed outside any field lands on.
    focus: FocusHandle,
    host: Host,
    link: Option<Entity<Link>>,
    note: Option<String>,
    /// The workspace a new thread is written on, which names the headline's project.
    workspace: Option<String>,
    /// A project with no workspace yet, whose page this is (`ProjectHome`): its first send makes the workspace.
    project: Option<String>,
    /// The page is a project's home (`ProjectHome.tsx`), not a workspace's New thread here (ChatView's fresh thread).
    home: bool,
    hero: Entity<HeroField>,
    _branch: Task<()>,
    _connect: Task<()>,
    /// What the headline last read off the sidebar.
    names_read: Names,
    /// The headline draws the project's name and mark from the sidebar, which the frame's cache of this page does not see.
    _names: Option<Subscription>,
    /// Set by any change of the composer, which may change its height: its next frame draws it whole, not cached at the
    /// height it had.
    composer_moved: Rc<Cell<bool>>,
    _moved: Subscription,
}

/// What the headline reads off the sidebar: the project's id and name, its look and its image.
#[derive(Default, PartialEq)]
struct Names(Option<String>, Option<String>, Option<Value>, Option<usize>);

/// `BRANCH_REREAD_MS`: how often a project home reads its branch again, so a checkout made in a terminal reaches the
/// tray without a send.
const BRANCH_REREAD: std::time::Duration = std::time::Duration::from_secs(5);

impl Screen for ComposerScreen {
    const NAME: &'static str = "composer";
    const PLACE: Place = Place::Main;

    /// The field behind the page is the frame's to draw, so its frames do not build the page and its composer again.
    fn parts(this: &Entity<Self>, _: &mut Window, cx: &mut App) -> Parts {
        Parts { backdrop: Some(this.read(cx).hero.clone().into()), ..Parts::default() }
    }

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| Composer::new(window, cx));
        let hero = cx.new(|_| HeroField::new());
        let names = crate::screens::terminal::workspaces_sidebar(cx).map(|sidebar| {
            cx.observe(&sidebar, |this: &mut Self, _, cx| {
                let names = this.names(cx);
                if names != this.names_read && this.composer.read(cx).fresh() {
                    cx.notify();
                }
                this.names_read = names;
            })
        });
        let composer_moved = Rc::new(Cell::new(true));
        let moved = moves(&composer, &composer_moved, cx);
        let mut screen = ComposerScreen {
            composer,
            focus: cx.focus_handle(),
            host,
            link: None,
            note: None,
            workspace: None,
            project: None,
            home: false,
            hero,
            _branch: Task::ready(()),
            _connect: Task::ready(()),
            names_read: Names::default(),
            _names: names,
            composer_moved,
            _moved: moved,
        };
        screen.connect(opened, window, cx);
        screen
    }

    /// Another `composer:<workspace>` while this one is open is another new thread, on a composer of its own.
    fn show(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        self.composer = cx.new(|cx| Composer::new(window, cx));
        self._moved = moves(&self.composer, &self.composer_moved, cx);
        self.link = None;
        self.note = None;
        self.connect(opened, window, cx);
    }
}

/// What a page's route names a project with no workspace by, as `composer:project:<id>`.
pub const PROJECT_ARG: &str = "project:";

/// The page of a machine deleted with its sub-thread, reached by a link or a kept selection: nothing to start there.
const SUB_THREAD_DELETED: &str = "This sub-thread was deleted, and its machine with it.";

impl ComposerScreen {
    /// Whether the page's workspace is a machine deleted once its sub-thread was done, whose sub-thread was deleted.
    fn deleted_sub_thread(&self, cx: &App) -> bool {
        let Some(workspace) = self.workspace.as_deref() else { return false };
        crate::screens::terminal::workspaces_sidebar(cx).is_some_and(|s| s.read(cx).deleted_sub_thread(workspace))
    }

    /// Ties the composer to the thread `opened` names, else the host's newest, or to a new thread on its workspace.
    fn connect(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        self.composer.update(cx, |c, cx| c.set_hold(Some(Hold::Connecting), cx));
        let host = self.host.clone();
        let wanted = opened.thread_id;
        let arg = opened.arg.map(String::from);
        let project = arg.as_deref().and_then(|a| a.strip_prefix(PROJECT_ARG)).map(str::to_owned);
        let new_on = arg.filter(|_| project.is_none());
        self.workspace = new_on.clone();
        self.project = project.clone();
        let (hero, composer) = (self.hero.clone(), self.composer.clone());
        let fresh = new_on.is_some() || project.is_some();
        hero.update(cx, |h, cx| h.follow(fresh.then_some(composer), cx));
        if let Some(workspace_id) = &new_on {
            crate::screens::terminal::page_starts_over(workspace_id, cx);
        }
        if fresh {
            // ChatComposer's `if (thread.fresh) editorRef.current?.focus()`: a new thread's box takes the keys.
            give_keys(&self.composer, window, cx);
        }
        // A fork's draft waits for this page's box, which takes it once the page's link stands.
        let forked = new_on.as_deref().and_then(|w| super::fork::take(w, cx));
        let reading = host.clone();
        self._connect = cx.spawn_in(window, async move |this, cx| {
            let read = async {
                if let Some(project) = project {
                    let target = Target { project: Some(project), ..Target::default() };
                    let opened = Link::read(&reading, &target).await?;
                    return Ok::<_, Failure>((target, opened));
                }
                if let Some(workspace_id) = new_on {
                    let target = Target { workspace_id, thread_id: None, project: None };
                    let opened = Link::read(&reading, &target).await?;
                    return Ok::<_, Failure>((target, opened));
                }
                // Only a page with no workspace of its own reads the host's whole thread list.
                let threads = reading.run(|client| async move { client.threads().await }).await?;
                let row = match &wanted {
                    Some(id) => threads.into_iter().find(|t| t.thread_id.as_deref() == Some(id.as_str())),
                    None => threads.into_iter().max_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.))),
                };
                let row = row.ok_or_else(|| Failure::new("the host lists no such thread"))?;
                let target = Target { workspace_id: row.workspace_id.clone(), thread_id: row.thread_id.clone(), project: None };
                let opened = Link::read(&reading, &target).await?;
                Ok::<_, Failure>((target, opened))
            }
            .await;
            _ = this.update_in(cx, |this, window, cx| match read {
                Ok((target, opened)) => {
                    let composer = this.composer.clone();
                    this.link = Some(cx.new(|cx| Link::new(host.clone(), target, opened, composer.clone(), window, cx)));
                    if let Some(draft) = &forked {
                        composer.update(cx, |c, cx| c.fork(draft, window, cx));
                        super::fork::read_images(&host, draft, &composer, cx);
                    }
                    cx.notify();
                }
                Err(failure) => {
                    eprintln!("desktop-gpui: {}", failure.message);
                    this.composer.update(cx, |c, cx| c.set_hold(Some(Hold::HostDown), cx));
                    this.note = Some(failure.message);
                    cx.notify();
                }
            });
        });
    }

    pub fn composer(&self) -> &Entity<Composer> {
        &self.composer
    }

    /// Whether the page stands as a project's home, which the shell says as it opens the page.
    pub fn set_home(&mut self, home: bool, cx: &mut Context<Self>) {
        self.home = home;
        self.read_branch(cx);
        cx.notify();
    }

    /// `HomeCheckoutRow`'s branch: the project's folder's, read now and every 5 s while the page stands as its home.
    fn read_branch(&mut self, cx: &mut Context<Self>) {
        let sidebar = crate::screens::terminal::workspaces_sidebar(cx);
        let project = self
            .workspace
            .as_deref()
            .and_then(|w| sidebar.as_ref()?.read(cx).workspace(w).0?.pointer("/project/id")?.as_str().map(str::to_owned))
            .filter(|_| self.home);
        let Some(project) = project else {
            self._branch = Task::ready(());
            return;
        };
        let host = self.host.clone();
        self._branch = cx.spawn(async move |this, cx| loop {
            let id = project.clone();
            let read = host
                .run(move |client| async move {
                    let ask: ProjectsBranchRequest =
                        serde_json::from_value(serde_json::json!({ "projectId": id })).map_err(|e| Failure::new(e.to_string()))?;
                    client.call(ask).await
                })
                .await;
            // A read that failed knows no branch, so the chip goes empty rather than keep one the folder may have left.
            let branch = read.ok().and_then(|b| b.branch).filter(|b| !b.is_empty() && b != "(detached)");
            let told = this.update(cx, |this, cx| this.composer.update(cx, |c, cx| c.set_project_branch(branch, cx)));
            if told.is_err() {
                return;
            }
            cx.background_executor().timer(BRANCH_REREAD).await;
        });
    }

    pub fn hero(&self) -> &Entity<HeroField> {
        &self.hero
    }

    fn names(&self, cx: &App) -> Names {
        let Some((sidebar, workspace)) = crate::screens::terminal::workspaces_sidebar(cx).zip(self.workspace.as_deref()) else {
            return Names::default();
        };
        let s = sidebar.read(cx);
        let record = s.workspace(workspace).0;
        let read = |at: &str| record.and_then(|w| w.pointer(at)).and_then(Value::as_str).map(str::to_owned);
        let (project, name) = (read("/project/id"), read("/project/name"));
        let prefs = s.around().preferences.as_deref();
        let look = project.as_deref().zip(prefs).and_then(|(id, p)| p.get("projectLook")?.get(id).cloned());
        let image =
            project.as_deref().zip(prefs).and_then(|(id, p)| s.around().images.of(p, id)).map(|i| std::sync::Arc::as_ptr(&i) as usize);
        Names(project, name, look, image)
    }

    /// The headline over a new thread: the workspace's project by its mark and name, the name the workspace's id where
    /// its record has not come (`workspace?.project.name ?? workspaceId`).
    fn heading(&self, window: &Window, cx: &App) -> Option<gpui_kit::AnyElement> {
        let p = cx.palette();
        let sidebar = crate::screens::terminal::workspaces_sidebar(cx);
        let sidebar = sidebar.as_ref().map(|s| s.read(cx));
        let (project, name) = match (self.workspace.as_deref(), self.project.as_deref()) {
            (Some(workspace), _) => {
                let record = sidebar.and_then(|s| s.workspace(workspace).0);
                let project = record.and_then(|w| w.pointer("/project/id")).and_then(Value::as_str);
                (project, record.and_then(|w| w.pointer("/project/name")).and_then(Value::as_str).unwrap_or(workspace))
            }
            (None, Some(project)) => {
                let named = sidebar.and_then(|s| s.around().projects.iter().find(|(id, _)| id == project).map(|(_, n)| n.as_str()));
                (Some(project), named.unwrap_or(project))
            }
            (None, None) => return None,
        };
        let ink = p.foreground.opacity(0.7);
        let mark = match sidebar {
            Some(s) => {
                let around = s.around();
                let prefs = around.preferences.clone().unwrap_or_default();
                crate::sidebar::looks::project_glyph(project, &prefs, &around.images, px(40.), ink, true, p)
            }
            None => gpui_kit::component::Icon::new(crate::ui::Glyph::Folder).size(px(40.)).text_color(ink).into_any_element(),
        };
        Some(hero::heading(SharedString::from(name.to_owned()), mark, self.home, window, cx).into_any_element())
    }
}

/// Puts the keys in `composer`'s box, unless the palette or a dialog stands over the window and holds them.
pub fn give_keys(composer: &Entity<Composer>, window: &mut Window, cx: &mut App) {
    if crate::screens::palette::palette::is_open(window, cx) || crate::screens::dialogs::layer::top(cx).is_some() {
        return;
    }
    let editor = composer.read(cx).editor().clone();
    editor.update(cx, |e, cx| e.focus(window, cx));
}

/// The thread column the composer sits in, as ChatView.tsx docks it: at the foot, or on a thread with no message
/// yet centred in the view under the thread's 52 px header, over the 88 px its heading takes.
pub fn column(fresh: bool, composer: impl IntoElement) -> gpui_kit::Div {
    let column = v_flex().size_full();
    if fresh {
        column
            .child(gpui_kit::div().h(gpui_kit::px(52.)))
            .child(v_flex().flex_1().justify_center().child(gpui_kit::div().h(gpui_kit::px(88.))).child(composer))
    } else {
        column.justify_end().child(composer)
    }
}

/// Marks `moved` now and at every change of `composer`.
fn moves(composer: &Entity<Composer>, moved: &Rc<Cell<bool>>, cx: &mut Context<ComposerScreen>) -> Subscription {
    moved.set(true);
    let moved = moved.clone();
    cx.observe(composer, move |_, _, _| moved.set(true))
}

/// A key that types into the composer from outside any field (composerTypeToFocus.ts `typesIntoComposer`): one
/// printable character with no command, control or option key held.
pub fn types_into_composer(event: &KeyDownEvent) -> Option<&str> {
    let k = &event.keystroke;
    if k.modifiers.platform || k.modifiers.control || k.modifiers.alt || k.modifiers.function {
        return None;
    }
    k.key_char.as_deref().filter(|c| c.chars().count() == 1 && !c.trim().is_empty())
}

impl Render for ComposerScreen {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("ComposerScreen");
        if window.focused(cx).is_none() {
            self.focus.focus(window, cx);
        }
        let p = cx.palette().clone();
        if self.deleted_sub_thread(cx) {
            if self.hero.read(cx).follows() {
                self.hero.update(cx, |h, cx| h.follow(None, cx));
            }
            return div()
                .debug_selector(|| "sub-thread-deleted".into())
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .text_sm()
                .text_color(p.muted_foreground)
                .child(SUB_THREAD_DELETED)
                .into_any_element();
        }
        let fresh = self.composer.read(cx).fresh();
        let focus = self.focus.clone();
        // Cached at its height as drawn, so its caret's blink draws the composer alone, as the thread's dock does.
        let height = self.composer.read(cx).drawn_height().filter(|_| !self.composer_moved.replace(false));
        self.composer.read(cx).lay_at(height);
        let composer = match height {
            Some(height) => self.composer.clone().cached(StyleRefinement::default().w_full().h(height)).into_any_element(),
            None => self.composer.clone().into_any_element(),
        };
        let heading = fresh.then(|| self.heading(window, cx)).flatten();
        let page = match heading {
            // `ProjectHome`: the headline and the composer one stack, 40 px apart, centred over 8vh of room below.
            Some(heading) if self.home => v_flex()
                .debug_selector(|| "project-home".into())
                .size_full()
                .justify_center()
                .gap_10()
                .pb(window.viewport_size().height * 0.08)
                .child(heading)
                .child(composer),
            // ChatView's fresh thread: the composer centred over the 88 px its headline takes, the headline's foot 40 px
            // over the composer.
            Some(heading) => v_flex()
                .size_full()
                .justify_center()
                .child(div().relative().h(px(88.)).child(div().absolute().left_0().right_0().bottom(px(40.)).child(heading)))
                .child(composer),
            None => column(fresh, composer),
        };
        let chat = div()
            .relative()
            .size_full()
            .child(page)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(move |this, event: &KeyDownEvent, window, cx| {
                if !focus.is_focused(window) {
                    return;
                }
                if let Some(key) = types_into_composer(event) {
                    let key = key.to_owned();
                    this.composer.update(cx, |c, cx| c.type_key(&key, window, cx));
                    cx.stop_propagation();
                }
            }))
            .text_color(p.foreground)
            .children(
                self.note.clone().map(|note| div().absolute().top_0().left_0().p_4().text_sm().text_color(p.muted_foreground).child(note)),
            );
        super::drop::chat(chat, &self.composer, window, cx).into_any_element()
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use gpui_kit::component::Root;
    use gpui_kit::{TestAppContext, VisualTestContext};
    use serde_json::json;

    use super::*;
    use crate::screens::palette::items::Project;
    use crate::screens::palette::palette::home_of_pick;
    use crate::screens::settings::door::{self, Door};
    use crate::screens::sidebar::host::{Change, Snapshot};
    use crate::screens::sidebar::Sidebar;
    use crate::screens::{Host, Screen};

    fn waits(cx: &mut VisualTestContext, what: &str, mut done: impl FnMut(&mut VisualTestContext) -> bool) {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            cx.run_until_parked();
            if done(cx) {
                return;
            }
            assert!(Instant::now() < deadline, "never came to: {what}");
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Catches: a project on a box nobody has run a thread on picked on the New thread page and nothing opening, as
    /// its page was found only through a workspace the sidebar held, where today's opens the project's page and its
    /// first send makes the workspace.
    #[gpui_kit::test]
    fn a_project_with_no_workspace_opens_its_page_and_its_first_send_names_the_project(cx: &mut TestAppContext) {
        cx.executor().allow_parking();
        cx.update(|cx| {
            gpui_kit::init(cx);
            crate::ui::init(crate::ui::Side::Light, cx);
            super::super::init(cx);
        });
        let picked = [Project {
            id: "p1".into(),
            name: "wsp-vm".into(),
            path: "/home/me/wsp-vm".into(),
            computer: Some("box1".into()),
            here: false,
            workspace: None,
        }];
        assert_eq!(home_of_pick(&picked, "p1"), "project:p1", "the pick opens the project's page");

        let agent = json!({ "harness": "claude", "label": "Claude Code", "source": "table", "version": null, "efforts": [],
            "models": [{ "value": "opus", "label": "Opus" }], "contextWindows": [], "permissionModes": [],
            "steers": false, "renames": false, "images": false });
        let door = Door::start(json!({
            "harnesses.list": { "reply": { "harnesses": [agent] } },
            "sessions.start": { "error": "held by the test" },
        }));
        let runtime = door::runtime();
        let host = Host::new(door.client(&runtime), runtime);
        let snapshot = Snapshot {
            projects: vec![json!({ "id": "p1", "name": "wsp-vm", "path": "/home/me/wsp-vm", "computer": "box1" })],
            places: vec![json!({ "id": "box1", "kind": "computer", "name": "aws-mumbai-2" })],
            ..Snapshot::default()
        };
        let mut views = None;
        let (_, cx) = cx.add_window_view(|window, cx| {
            let sidebar = cx.new(Sidebar::new);
            sidebar.update(cx, |s, cx| s.apply(Change::Whole(Box::new(snapshot)), cx));
            crate::screens::terminal::read_workspaces_from(&sidebar, cx);
            let opened = Opened { arg: Some("project:p1".into()), ..Opened::default() };
            let page = cx.new(|cx| <ComposerScreen as Screen>::new(host, opened, window, cx));
            views = Some((sidebar, page.clone()));
            Root::new(page, window, cx)
        });
        let (sidebar, page) = views.unwrap();
        assert_eq!(sidebar.read_with(cx, |s, _| s.home_of("p1")), None, "the host holds no workspace for the project");
        waits(cx, "the page's link", |cx| page.read_with(cx, |p, _| p.link.is_some()));
        assert!(cx.debug_bounds("hero-question:What should we build in wsp-vm?").is_some(), "the page names the project");
        assert_eq!(door.asked_of("harnesses.list")[0]["project"], "p1", "the agents are read off the project");
        let composer = page.read_with(cx, |p, _| p.composer().clone());
        cx.update(|window, cx| composer.update(cx, |c, cx| c.set_text("hello", window, cx)));
        cx.update(|window, cx| composer.update(cx, |c, cx| c.submit(window, cx)));
        waits(cx, "the send", |_| !door.asked_of("sessions.start").is_empty());
        let start = &door.asked_of("sessions.start")[0];
        assert_eq!((start["project"].as_str(), start.get("workspaceId")), (Some("p1"), None), "the send names the project: {start}");
    }
}
