// SPDX-License-Identifier: AGPL-3.0-only
//! The composer alone in the main column, as `desktop-gpui --screen composer` opens it for captures: at the foot of a
//! thread column on the page's ground, on the thread the route opens, else the host's newest. `composer:<workspace>`
//! opens it on a new thread there, as New thread does.

use gpui_kit::component::v_flex;
use gpui_kit::{prelude::*, Context, Entity, FocusHandle, KeyDownEvent, Task, Window};
use wsp_host_client::Failure;

use super::link::{Link, Target};
use super::model::Hold;
use super::Composer;
use crate::screens::{Host, Opened, Place, Screen};
use crate::ui::ActivePalette;

pub struct ComposerScreen {
    composer: Entity<Composer>,
    /// The column's own focus, which a key typed outside any field lands on.
    focus: FocusHandle,
    host: Host,
    link: Option<Entity<Link>>,
    note: Option<String>,
    _connect: Task<()>,
}

impl Screen for ComposerScreen {
    const NAME: &'static str = "composer";
    const PLACE: Place = Place::Main;

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let composer = cx.new(|cx| Composer::new(window, cx));
        let mut screen = ComposerScreen { composer, focus: cx.focus_handle(), host, link: None, note: None, _connect: Task::ready(()) };
        screen.connect(opened, window, cx);
        screen
    }

    /// Another `composer:<workspace>` while this one is open is another new thread, on a composer of its own.
    fn show(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        self.composer = cx.new(|cx| Composer::new(window, cx));
        self.link = None;
        self.note = None;
        self.connect(opened, window, cx);
    }
}

impl ComposerScreen {
    /// Ties the composer to the thread `opened` names, else the host's newest, or to a new thread on its workspace.
    fn connect(&mut self, opened: Opened, window: &mut Window, cx: &mut Context<Self>) {
        self.composer.update(cx, |c, cx| c.set_hold(Some(Hold::Connecting), cx));
        let host = self.host.clone();
        let wanted = opened.thread_id;
        let new_on = opened.arg.map(String::from);
        let threads = host.run(|client| async move { client.threads().await });
        let reading = host.clone();
        self._connect = cx.spawn_in(window, async move |this, cx| {
            let read = async {
                if let Some(workspace_id) = new_on {
                    let target = Target { workspace_id, thread_id: None };
                    let opened = Link::read(&reading, &target).await?;
                    return Ok::<_, Failure>((target, opened));
                }
                let threads = threads.await?;
                let row = match &wanted {
                    Some(id) => threads.into_iter().find(|t| t.thread_id.as_deref() == Some(id.as_str())),
                    None => threads.into_iter().max_by(|a, b| a.started_at.unwrap_or(0.).total_cmp(&b.started_at.unwrap_or(0.))),
                };
                let row = row.ok_or_else(|| Failure::new("the host lists no such thread"))?;
                let target = Target { workspace_id: row.workspace_id.clone(), thread_id: row.thread_id.clone() };
                let opened = Link::read(&reading, &target).await?;
                Ok::<_, Failure>((target, opened))
            }
            .await;
            _ = this.update_in(cx, |this, window, cx| match read {
                Ok((target, opened)) => {
                    let composer = this.composer.clone();
                    this.link = Some(cx.new(|cx| Link::new(host, target, opened, composer, window, cx)));
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
}

/// The thread column the composer sits in, as ChatView.tsx docks it: at the foot, or on a thread with no message
/// yet centred in the view under the thread's 52 px header, over the 88 px its heading takes.
pub fn column(fresh: bool, ground: gpui_kit::Hsla, composer: impl IntoElement) -> gpui_kit::Div {
    let column = v_flex().size_full().bg(ground);
    if fresh {
        column
            .child(gpui_kit::div().h(gpui_kit::px(52.)))
            .child(v_flex().flex_1().justify_center().child(gpui_kit::div().h(gpui_kit::px(88.))).child(composer))
    } else {
        column.justify_end().child(composer)
    }
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
        if window.focused(cx).is_none() {
            self.focus.focus(window, cx);
        }
        let p = cx.palette();
        let fresh = self.composer.read(cx).fresh();
        let focus = self.focus.clone();
        column(fresh, p.background, self.composer.clone())
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
            .children(self.note.clone().map(|note| gpui_kit::div().p_4().text_sm().text_color(p.muted_foreground).child(note)))
    }
}
