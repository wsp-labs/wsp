// SPDX-License-Identifier: AGPL-3.0-only
//! Renaming a thread or a workspace: the name box on its tile and the name sent to the host.

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::{div, prelude::*, px, App, Context, Entity, KeyDownEvent, Window};
use serde_json::json;
use wsp_host_client::protocol::{SessionsRenameRequest, WorkspacesRenameRequest, WorkspacesWakeRequest};

use super::{host, menu, Renaming, Sidebar};
use crate::ui::catalog;
use crate::ui::ActivePalette;

impl Sidebar {
    /// Renames a thread through its agent; an outcome other than renamed is said over the list with the host's words.
    pub fn rename(&mut self, key: &str, title: &str, cx: &mut Context<Self>) {
        let Some(task) = self.send_name(key, title) else { return };
        cx.spawn(async move |this, cx| {
            if let Some(said) = task.await.unwrap_or_else(|e| Some(e.to_string())) {
                _ = this.update(cx, |this, cx| this.refuse(said, cx));
            }
        })
        .detach();
    }

    /// Sends a thread's name, or a workspace's for a key `w:<workspace>`: none once the host takes it, else what to say.
    fn send_name(&mut self, key: &str, name: &str) -> Option<tokio::task::JoinHandle<Option<String>>> {
        let link = self.link.clone()?;
        let task = match key.strip_prefix("w:") {
            Some(id) => {
                let params = json!({ "workspaceId": id, "name": name });
                link.runtime.spawn(async move { host::write::<WorkspacesRenameRequest>(link.host, params).await.err().map(|f| f.message) })
            }
            None => {
                let t = self.thread(key)?;
                let params = json!({ "sessionId": t.session_id, "title": name });
                // The name is kept on the machine, so a napping one is woken first, as `renameThread` does.
                let napping = self.around.workspaces.get(&t.workspace_id).is_some_and(|w| w["phase"] != "running");
                let wake = napping.then(|| json!({ "workspaceId": t.workspace_id }));
                let (harness, title) = (t.harness.clone(), name.to_owned());
                link.runtime.spawn(async move {
                    let renamed = async {
                        if let Some(wake) = wake {
                            host::write::<WorkspacesWakeRequest>(link.host.clone(), wake).await?;
                        }
                        host::write_read::<SessionsRenameRequest>(link.host, params).await
                    };
                    match renamed.await {
                        Err(failure) => Some(format!("{title}: {}", failure.message)),
                        Ok(reply) if reply["outcome"] == "renamed" => None,
                        Ok(reply) => {
                            Some(rename_not_taken_line(&harness, reply["outcome"].as_str().unwrap_or_default(), reply["error"].as_str()))
                        }
                    }
                })
            }
        };
        self.refusal = None;
        Some(task)
    }

    /// Opens the name box on a tile, as the menu's Rename and a double-click on the title do.
    /// Opens the name box on a thread's tile, or on a workspace's own tile for a key `w:<workspace>`.
    pub fn open_rename(&mut self, key: &str, window: &mut Window, cx: &mut Context<Self>) {
        let title = match key.strip_prefix("w:") {
            Some(id) => self.around.workspaces.get(id).and_then(|w| w.get("name")?.as_str()).map(str::to_owned),
            // A rename the menu refuses opens no box on a double-click either (`openerOf`).
            None => self.thread(key).filter(|t| menu::rename_refusal(self, t).is_none()).map(|t| t.title.clone()),
        };
        let Some(title) = title else { return };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(title.clone()));
        let events = cx.subscribe_in(&input, window, move |this, input, event: &InputEvent, window, cx| match event {
            InputEvent::PressEnter { .. } => {
                let typed = input.read(cx).value().trim().to_owned();
                let Some(key) = this.renaming.as_ref().filter(|r| !r.saving).map(|r| r.key.clone()) else { return };
                // An empty name or the same one closes the box, as the web's does.
                if typed.is_empty() || typed == title {
                    return this.close_rename(window, cx);
                }
                let Some(task) = this.send_name(&key, &typed) else { return };
                if let Some(r) = this.renaming.as_mut() {
                    r.saving = true;
                }
                // The box closes once the host takes the name; a refusal leaves the name in it to try again (`sendName`).
                cx.spawn_in(window, async move |this, cx| {
                    let said = task.await.unwrap_or_else(|e| Some(e.to_string()));
                    _ = this.update_in(cx, |this, window, cx| {
                        let Some(r) = this.renaming.as_mut().filter(|r| r.key == key) else { return };
                        match said {
                            None => this.close_rename(window, cx),
                            Some(said) => {
                                r.saving = false;
                                this.refuse(said, cx);
                            }
                        }
                    });
                })
                .detach();
                cx.notify();
            }
            InputEvent::Blur if this.renaming.as_ref().is_some_and(|r| !r.saving) => {
                this.renaming = None;
                cx.notify();
            }
            _ => {}
        });
        input.update(cx, |state, cx| {
            state.focus(window, cx);
            state.select_all(window, cx);
        });
        self.renaming = Some(Renaming { key: key.to_owned(), input, saving: false, _events: events });
        cx.notify();
    }

    fn close_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            window.focus(&self.focus, cx);
            cx.notify();
        }
    }

    /// The name in the open rename box, none while no box is open.
    pub fn rename_box(&self, cx: &App) -> Option<String> {
        self.renaming.as_ref().map(|r| r.input.read(cx).value().to_string())
    }

    /// The name box, where Escape leaves the name as it was.
    pub(crate) fn rename_field(&self, input: &Entity<InputState>, cx: &mut Context<Self>) -> gpui_kit::Div {
        let p = cx.palette().clone();
        // `rounded-sm border border-input px-1`, the ring's edge while it has the keys.
        div()
            .flex()
            .items_center()
            .rounded(px(6.))
            .border_1()
            .border_color(p.ring)
            .px(px(4.))
            .capture_key_down(cx.listener(|this, e: &KeyDownEvent, window, cx| {
                if e.keystroke.key == "escape" && this.renaming.as_ref().is_some_and(|r| !r.saving) {
                    this.close_rename(window, cx);
                    cx.stop_propagation();
                }
            }))
            .child(gpui_kit::component::input::Input::new(input).appearance(false).p_0().h(px(16.)).text_size(px(14.)).line_height(px(16.)))
    }
}

/// `renameNotTakenLine`: why the agent kept no name, in the person's words.
pub(crate) fn rename_not_taken_line(harness: &str, outcome: &str, error: Option<&str>) -> String {
    match outcome {
        "unsupported" => format!("Rename in {} is not kept", catalog::name(harness)),
        "no-session" => format!("{} on the task has no session for this thread yet", catalog::name(harness)),
        "not-found" => "The runtime has no such thread any more".to_owned(),
        _ => error.unwrap_or("The task said nothing about the write").to_owned(),
    }
}
