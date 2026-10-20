// SPDX-License-Identifier: AGPL-3.0-only
//! The sidebar's writes to the host: bring back, settle and restore, stop, delete and the threads' marks.

use std::time::Instant;

use gpui_kit::Context;
use serde_json::{json, Value};
use wsp_host_client::protocol::{
    SessionsDeleteRequest, SessionsForgetRequest, SessionsInterruptRequest, SessionsMarkRequest, SessionsRestoreRequest,
    SessionsSettleRequest, WorkspacesBringBackRequest,
};
use wsp_host_client::Op;

use super::model::{self, Prefs, Thread};
use super::{host, Processes, Sidebar, ARMED};

impl Sidebar {
    /// One write; a refusal is said where the note stands, and the host's push redraws what changed.
    pub(super) fn write<O: Op + serde::de::DeserializeOwned + Send + 'static>(&mut self, params: Value, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        self.refusal = None;
        let task = link.runtime.spawn(host::write::<O>(link.host.clone(), params));
        cx.spawn(async move |this, cx| {
            let done = task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string())));
            if let Err(failure) = done {
                _ = this.update(cx, |this, cx| this.refuse(failure.message, cx));
            }
        })
        .detach();
    }

    /// Pushes a workspace's branch and opens its pull request; what it brought back is a done toast (`broughtBackLine`),
    /// a refusal is said over the list.
    pub fn bring_back(&mut self, workspace_id: &str, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        self.refusal = None;
        let params = json!({ "workspaceId": workspace_id });
        let place = self.around.workspaces.get(workspace_id).and_then(|w| crate::screens::notices::host::copy_name(w, &self.around.places));
        let task = link.runtime.spawn(host::write_read::<WorkspacesBringBackRequest>(link.host.clone(), params));
        cx.spawn(async move |this, cx| match task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
            Ok(back) => {
                let notice = crate::ui::Notice::done(brought_back_line(&back));
                cx.update(|cx| crate::ui::notify(cx, if let Some(place) = place { notice.place(place) } else { notice }))
            }
            Err(failure) => _ = this.update(cx, |this, cx| this.refuse(failure.message, cx)),
        })
        .detach();
    }

    pub fn settle(&mut self, key: &str, cx: &mut Context<Self>) {
        self.settle_with(key, Processes::Ask, cx);
    }

    /// Settles a thread's tree; a thread the host leaves standing is said in the note, with the host's reason.
    pub fn settle_with(&mut self, key: &str, processes: Processes, cx: &mut Context<Self>) {
        // The host settles a thread with everything under it, so the web names the tree's top alone.
        self.settle_ids(vec![key.to_owned()], processes, cx);
    }

    /// `settleOpenThread`: settles the root tree of `open`, else of the workspace's top thread, as the tile's Settle
    /// does; nothing while a thread of the tree works or the tree is settled already.
    pub fn settle_open(&mut self, workspace_id: &str, open: Option<&str>, cx: &mut Context<Self>) {
        if let Some(root) = self.open_root(workspace_id, open) {
            self.settle(&root, cx);
        }
    }

    /// The root [`Sidebar::settle_open`] settles, by its key; None where it settles nothing.
    pub fn open_root(&self, workspace_id: &str, open: Option<&str>) -> Option<String> {
        fn holds(node: &model::Tree, all: &[Thread], key: &str) -> bool {
            node.thread.is_some_and(|i| all[i].key == key) || node.children.iter().any(|c| holds(c, all, key))
        }
        self.around.workspaces.get(workspace_id)?;
        let top = || {
            let walk = crate::screens::palette::switcher::walk::in_workspace(&self.threads, &self.around.workspaces, workspace_id);
            walk.into_iter().next().map(|t| t.thread_id)
        };
        let open = open.filter(|key| self.thread(key).is_some()).map(str::to_owned).or_else(top)?;
        let prefs = Prefs { project: None, ..self.prefs.clone() };
        let view = model::sidebar(&self.threads, &prefs, self.now, Some(&open));
        let mut live = view.pinned.iter().chain(view.needs_you.iter().filter(|n| n.inbox.is_none())).chain(&view.threads);
        let root = live.find(|n| holds(n, &self.threads, &open)).filter(|n| n.thread.is_some())?.key.clone();
        (!self.tree_works(&root)).then_some(root)
    }

    /// Settles threads; where the host leaves them for processes still running, asks as the web does.
    pub(crate) fn settle_ids(&mut self, ids: Vec<String>, processes: Processes, cx: &mut Context<Self>) {
        let Some(link) = self.link.clone() else { return };
        self.refusal = None;
        let params = json!({ "threadIds": ids, "processes": processes.word() });
        let task = link.runtime.spawn(host::write_read::<SessionsSettleRequest>(link.host.clone(), params));
        cx.spawn(async move |this, cx| {
            let said = match task.await.unwrap_or_else(|e| Err(wsp_host_client::Failure::new(e.to_string()))) {
                Err(failure) if failure.kind.as_deref() == Some("processes-left") => {
                    _ = this.update(cx, |this, cx| this.ask_settle(ids, failure.message, cx));
                    None
                }
                Err(failure) => Some(format!("Not settled: {}", failure.message)),
                Ok(reply) => {
                    let back = this.clone();
                    let restore = move |ids: Vec<String>, cx: &mut gpui_kit::App| _ = back.update(cx, |this, cx| this.restore_ids(ids, cx));
                    cx.update(|cx| crate::ui::notify(cx, crate::screens::notices::settled(&reply, restore)));
                    None
                }
            };
            if let Some(said) = said {
                _ = this.update(cx, |this, cx| this.refuse(said, cx));
            }
        })
        .detach();
    }

    pub fn restore(&mut self, key: &str, cx: &mut Context<Self>) {
        self.restore_ids(vec![key.to_owned()], cx);
    }

    pub(crate) fn restore_ids(&mut self, ids: Vec<String>, cx: &mut Context<Self>) {
        self.write::<SessionsRestoreRequest>(json!({ "threadIds": ids }), cx);
    }

    /// One write by its op; a refusal is said over the list.
    pub(crate) fn write_op<O: Op + serde::de::DeserializeOwned + Send + 'static>(&mut self, params: Value, cx: &mut Context<Self>) {
        self.write::<O>(params, cx);
    }

    pub fn stop(&mut self, key: &str, cx: &mut Context<Self>) {
        let Some(t) = self.thread(key) else { return };
        let session = t.session_id.clone();
        self.armed = None;
        self.write::<SessionsInterruptRequest>(json!({ "sessionId": session }), cx);
    }

    /// The hover act's Stop: the first press arms it, a second inside two seconds stops.
    pub(super) fn press_stop(&mut self, key: &str, cx: &mut Context<Self>) {
        match &self.armed {
            Some((armed, at)) if armed == key && at.elapsed() < ARMED => self.stop(key, cx),
            _ => {
                self.armed = Some((key.to_owned(), Instant::now()));
                cx.notify();
            }
        }
    }

    pub fn delete(&mut self, key: &str, cx: &mut Context<Self>) {
        self.delete_with(key, Processes::Ask, cx);
    }

    pub fn delete_with(&mut self, key: &str, processes: Processes, cx: &mut Context<Self>) {
        let Some(t) = self.thread(key) else { return };
        if self.in_folder(t) {
            self.write::<SessionsDeleteRequest>(json!({ "threadId": key, "processes": processes.word() }), cx);
        } else {
            self.write::<SessionsForgetRequest>(json!({ "threadId": key }), cx);
        }
    }

    pub fn mark_threads(&mut self, ids: Vec<String>, marks: Value, cx: &mut Context<Self>) {
        self.write::<SessionsMarkRequest>(json!({ "threadIds": ids, "marks": marks }), cx);
    }
}

/// `broughtBackLine`: the branch, then the pull request it opened or that there is none, then the host's note.
fn brought_back_line(back: &Value) -> String {
    let branch = back["branch"].as_str().unwrap_or_default();
    let note = back["note"].as_str();
    let line = match (back["pr"]["number"].as_u64(), note) {
        (Some(number), _) => format!("{branch}: pull request #{number} {}", back["pr"]["state"].as_str().unwrap_or_default()),
        (None, None) => format!("{branch} pushed"),
        (None, Some(_)) => format!("{branch} pushed, no pull request"),
    };
    match note {
        Some(note) => format!("{line}: {note}"),
        None => line,
    }
}
