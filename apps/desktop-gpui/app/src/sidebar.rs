// SPDX-License-Identifier: AGPL-3.0-only
//! The threads the host lists, under their projects; clicking one asks the shell to open it.

use gpui::{div, prelude::*, px, Context, EventEmitter, SharedString, Window};
use gpui_component::{v_flex, ActiveTheme};
use wsp_host_client::protocol::SessionView;

pub const WIDTH: gpui::Pixels = px(260.);

#[derive(Clone, Debug, PartialEq)]
pub struct ThreadItem {
    pub workspace_id: String,
    pub thread_id: String,
    pub project: String,
    pub title: String,
}

impl ThreadItem {
    /// A row of the host's thread list; a sub-thread lives under its lead and is not listed here.
    pub fn of(view: &SessionView) -> Option<ThreadItem> {
        if view.parent_thread_id.is_some() {
            return None;
        }
        Some(ThreadItem {
            workspace_id: view.workspace_id.clone(),
            thread_id: view.thread_id.clone().unwrap_or_else(|| view.id.clone()),
            project: view.project.as_ref().map_or_else(String::new, |p| p.name.clone()),
            title: view.harness_title.clone().or_else(|| view.prompt.clone()).unwrap_or_else(|| "New thread".to_owned()),
        })
    }
}

pub struct Selected(pub ThreadItem);

pub struct Sidebar {
    items: Vec<ThreadItem>,
    selected: Option<String>,
    note: Option<String>,
}

impl EventEmitter<Selected> for Sidebar {}

impl Sidebar {
    pub fn new(_: &mut Context<Self>) -> Self {
        Sidebar { items: Vec::new(), selected: None, note: Some("Connecting".to_owned()) }
    }

    pub fn set_items(&mut self, mut items: Vec<ThreadItem>, cx: &mut Context<Self>) {
        items.sort_by(|a, b| a.project.cmp(&b.project));
        self.note = items.is_empty().then(|| "No threads".to_owned());
        self.items = items;
        cx.notify();
    }

    pub fn set_note(&mut self, note: String, cx: &mut Context<Self>) {
        self.note = Some(note);
        cx.notify();
    }

    pub fn items(&self) -> &[ThreadItem] {
        &self.items
    }

    /// Marks a thread as the open one, without asking the shell to open it.
    pub fn mark(&mut self, thread_id: &str, cx: &mut Context<Self>) -> Option<ThreadItem> {
        let item = self.items.iter().find(|i| i.thread_id == thread_id).cloned()?;
        self.selected = Some(item.thread_id.clone());
        cx.notify();
        Some(item)
    }

    fn select(&mut self, thread_id: &str, cx: &mut Context<Self>) {
        if let Some(item) = self.mark(thread_id, cx) {
            cx.emit(Selected(item));
        }
    }
}

impl Render for Sidebar {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme();
        let (muted, active) = (theme.muted_foreground, theme.sidebar_accent);
        let mut column = v_flex().id("sidebar").w(WIDTH).h_full().flex_none().overflow_y_scroll().py_2().bg(theme.sidebar);
        let mut project: Option<&str> = None;
        for item in &self.items {
            if project != Some(item.project.as_str()) {
                project = Some(item.project.as_str());
                column =
                    column.child(div().px_3().pt_3().pb_1().text_xs().text_color(muted).child(SharedString::from(item.project.clone())));
            }
            let id = item.thread_id.clone();
            column = column.child(
                div()
                    .id(SharedString::from(format!("thread-{id}")))
                    .w(WIDTH)
                    .px_3()
                    .py_1()
                    .text_sm()
                    .truncate()
                    .cursor_pointer()
                    .when(self.selected.as_deref() == Some(id.as_str()), |this| this.bg(active))
                    .on_click(cx.listener(move |this, _, _, cx| this.select(&id, cx)))
                    .child(SharedString::from(item.title.clone())),
            );
        }
        column.children(self.note.clone().map(|note| div().px_3().py_1().text_sm().text_color(muted).child(note)))
    }
}
