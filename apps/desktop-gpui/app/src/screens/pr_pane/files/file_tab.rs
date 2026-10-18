// SPDX-License-Identifier: AGPL-3.0-only
//! One open file, a tab of its own beside the tree (`files/FilePreviewSurface.tsx`): a crumb row whose folders go back
//! to the tree there, the markdown toggle, reload and Open in editor, then the file from `fs.read` as code with line
//! numbers or, for markdown, rendered. A line asked for is lit and scrolled to, and asked again is scrolled to again.

use std::rc::Rc;

use gpui_kit::component::text::TextViewState;
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{div, prelude::*, px, App, Context, Entity, ScrollHandle, SharedString, Subscription, Task, Window};
use serde_json::{json, Value};

use super::at::{self, Facts};
use super::crumbs::CrumbRow;
use super::open_split::{self, opens_in_editor, Said, SSH_BEHIND_KIND, SSH_INCLUDE_KIND};
use super::paths::{base_name, is_binary, is_markdown, roots};
use super::words;
use super::{RenderMarkdown, Reveal};
use crate::screens::pr_pane::daemon::DaemonLink;
use crate::screens::pr_pane::diff::FileView;
use crate::screens::{navigate, Host, Route};
use crate::ui::{scroll_area, spinner, ActivePalette as _, Button, Glyph, Run};

/// What fs.read answered, its text handed to the views and not kept here: the bytes shown, the whole file's size,
/// whether the daemon cut it at 2 MiB, and whether a NUL in it made it binary.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reply {
    pub shown: u64,
    pub size: u64,
    pub truncated: bool,
    pub binary: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Read {
    Pending { last: Option<Reply> },
    Ready(Reply),
    Error { message: String, last: Option<Reply> },
}

impl Read {
    pub fn reply(&self) -> Option<&Reply> {
        match self {
            Read::Ready(reply) => Some(reply),
            Read::Pending { last } | Read::Error { last, .. } => last.as_ref(),
        }
    }
}

pub struct FileTab {
    host: Host,
    workspace_id: String,
    thread_id: Option<String>,
    path: String,
    line: Option<usize>,
    link: Entity<DaemonLink>,
    facts: Option<at::Read>,
    read: Read,
    code: Entity<FileView>,
    markdown: Entity<TextViewState>,
    /// The rendered markdown's scroll, under the app's one scrollbar (`ScrollArea`).
    scroll: ScrollHandle,
    said: Option<Said>,
    loading: Option<Task<()>>,
    _subs: Vec<Subscription>,
    _facts: Task<()>,
}

impl FileTab {
    pub fn new(host: Host, workspace_id: String, thread_id: Option<String>, path: String, cx: &mut Context<Self>) -> Self {
        let link = DaemonLink::of(&host, &workspace_id, cx);
        let code = cx.new(FileView::new);
        let markdown = cx.new(|cx| TextViewState::markdown("", cx));
        let asked = cx.try_global::<Reveal>().filter(|r| r.workspace_id == workspace_id && r.path == path).and_then(|r| r.line);
        let line = asked.or_else(|| crate::screens::pr_pane::shot()?.strip_prefix("line:")?.parse().ok());
        let subs = vec![
            cx.observe(&link, |_, _, cx| cx.notify()),
            cx.observe_global::<Reveal>(|this, cx| {
                let Some(reveal) = cx.try_global::<Reveal>().cloned() else { return };
                if reveal.workspace_id == this.workspace_id && reveal.path == this.path {
                    this.line = reveal.line;
                    this.code.update(cx, |code, cx| code.set_lit(reveal.line, cx));
                    cx.notify();
                }
            }),
            cx.observe_global::<RenderMarkdown>(|_, cx| cx.notify()),
        ];
        let facts = at::follow(&host, workspace_id.clone(), thread_id.clone(), cx, Self::take_facts);
        let mut tab = FileTab {
            host,
            workspace_id,
            thread_id,
            path,
            line,
            link,
            facts: None,
            read: Read::Pending { last: None },
            code,
            markdown,
            scroll: ScrollHandle::new(),
            said: None,
            loading: None,
            _subs: subs,
            _facts: facts,
        };
        if tab.path.starts_with('/') {
            tab.load(cx);
        }
        tab
    }

    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn line(&self) -> Option<usize> {
        self.line
    }

    pub fn read(&self) -> &Read {
        &self.read
    }

    pub fn code(&self) -> &Entity<FileView> {
        &self.code
    }

    pub fn said(&self) -> Option<&Said> {
        self.said.as_ref()
    }

    fn facts(&self) -> Option<&Facts> {
        self.facts.as_ref().and_then(|f| f.as_ref().ok())
    }

    /// Whether the file shows rendered: markdown, the preference on, and no line asked for, since rendered markdown
    /// has no lines to light.
    pub fn rendered(&self, cx: &App) -> bool {
        is_markdown(&self.path) && RenderMarkdown::on(cx) && self.line.is_none()
    }

    /// Takes what the host says of the workspace now: a relative path is read once the facts are in, and a workspace
    /// that wakes reads the file again.
    fn take_facts(&mut self, read: at::Read, cx: &mut Context<Self>) {
        let woke = self.facts().is_some_and(|f| !f.running) && read.as_ref().is_ok_and(|f| f.running);
        self.facts = Some(read);
        if !self.path.starts_with('/') {
            self.resolve(cx);
        } else if woke {
            self.load(cx);
        }
        cx.notify();
    }

    /// A path named relative to the thread's project, as a route may name it: read against the project's folder, else
    /// the panes' folder, once the workspace's facts and the daemon's hello are in.
    fn resolve(&mut self, cx: &mut Context<Self>) {
        let facts = match &self.facts {
            Some(Ok(facts)) => facts,
            Some(Err(refused)) => {
                self.read = Read::Error { message: refused.clone(), last: None };
                return;
            }
            None => return,
        };
        let link = self.link.read(cx);
        let base = facts.project.clone().or_else(|| link.folder(self.thread_id.as_deref(), facts.cwd.as_deref(), None));
        let Some(base) = base else {
            self.read = Read::Error { message: words::NOT_RUNNING.to_owned(), last: None };
            return;
        };
        self.path = super::paths::join(&base, self.path.trim_start_matches("./"));
        self.load(cx);
    }

    /// Reads the file again, keeping the last reply up meanwhile.
    pub fn load(&mut self, cx: &mut Context<Self>) {
        if !self.path.starts_with('/') {
            return;
        }
        self.read = Read::Pending { last: self.read.reply().copied() };
        let asked = self.link.read(cx).request("fs.read", json!({ "path": self.path }));
        self.loading = Some(cx.spawn(async move |this, cx| {
            let answer = asked.await;
            _ = this.update(cx, |this, cx| {
                let last = this.read.reply().copied();
                this.read = match answer {
                    Ok(mut reply) => {
                        let content = match reply.remove("content") {
                            Some(Value::String(content)) => content,
                            _ => String::new(),
                        };
                        let binary = is_binary(&content);
                        let read = Reply {
                            shown: content.len() as u64,
                            size: reply.get("size").and_then(Value::as_u64).unwrap_or(0),
                            truncated: reply.get("truncated").and_then(Value::as_bool).unwrap_or(false),
                            binary,
                        };
                        if !binary {
                            let (path, line) = (this.path.clone(), this.line);
                            this.code.update(cx, |code, cx| {
                                code.set_file(path, &content, cx);
                                code.set_lit(line, cx);
                            });
                            if is_markdown(&this.path) {
                                this.markdown.update(cx, |md, cx| md.set_text(&content, cx));
                            }
                        }
                        Read::Ready(read)
                    }
                    Err(refused) => Read::Error { message: refused.message, last },
                };
                cx.notify();
            });
        }));
        cx.notify();
    }

    /// A crumb's folder: the panes pin there and the Files tab shows it.
    fn pick_folder(&mut self, folder: &str, cx: &mut Context<Self>) {
        if let Some(thread) = self.thread_id.clone() {
            self.link.update(cx, |link, cx| link.pin(&thread, folder, cx));
        }
        navigate(Route::Screen("files:files".into()), cx);
    }

    /// Open in editor: the file at its line, or the sentence that takes the button's place.
    pub fn open_in_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(facts) = self.facts().cloned() else { return };
        if !opens_in_editor(&facts) {
            self.said = Some(Said { line: words::open::opens_here(&facts.name), refused: false });
            cx.notify();
            return;
        }
        let (host, workspace, path, line) = (self.host.clone(), self.workspace_id.clone(), self.path.clone(), self.line);
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            let opened = open_split::open(host.clone(), workspace.clone(), path.clone(), line, None).await;
            let Err(failure) = opened else { return };
            if failure.kind.as_deref() == Some(SSH_INCLUDE_KIND) {
                let me = this.clone();
                let again: Run = Rc::new(move |window, cx| _ = me.update(cx, |this, cx| this.open_in_editor(window, cx)));
                _ = handle.update(cx, |_, window, cx| open_split::ask_ssh(host, again, window, cx));
                return;
            }
            _ = this.update(cx, |this, cx| {
                this.said = Some(Said { refused: failure.kind.as_deref() != Some(SSH_BEHIND_KIND), line: failure.message });
                cx.notify();
            });
        })
        .detach();
    }

    fn head(&self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let me = cx.weak_entity();
        let pick: super::crumbs::Pick = Rc::new(move |folder, _, cx| _ = me.update(cx, |this, cx| this.pick_folder(folder, cx)));
        let project = self.facts().and_then(|f| f.project.clone());
        let roots = roots(self.link.read(cx).root().as_deref(), project.as_deref());
        let pending = matches!(self.read, Read::Pending { .. });
        let rendered = self.rendered(cx);
        let me = cx.weak_entity();
        let reload = Button::icon_xs("file-reload", Glyph::RotateCw).busy(pending).tooltip(words::RELOAD).on_click({
            let me = me.clone();
            move |_, _, cx| _ = me.update(cx, |this, cx| this.load(cx))
        });
        let toggle = (is_markdown(&self.path) && self.line.is_none()).then(|| {
            let word = if rendered { words::SHOW_SOURCE } else { words::SHOW_RENDERED };
            div().debug_selector(|| "file-markdown-toggle".into()).child(
                Button::icon_xs("file-markdown-toggle", if rendered { Glyph::CodeXml } else { Glyph::Eye })
                    .tooltip(word)
                    .on_click(move |_, _, cx| RenderMarkdown::set(!rendered, cx)),
            )
        });
        let open = match &self.said {
            Some(said) => div()
                .id("file-open-said")
                .debug_selector(|| "open-in-editor-said".into())
                .min_w_0()
                .flex_shrink(1.)
                .truncate()
                .text_size(px(12.))
                .text_color(if said.refused { p.error_foreground } else { p.muted_foreground })
                .tooltip(crate::ui::tip(said.line.clone()))
                .child(said.line.clone())
                .into_any_element(),
            None => Button::icon_xs("file-open-in-editor", Glyph::SquareArrowOutUpRight)
                .tooltip(words::OPEN_IN_EDITOR)
                .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.open_in_editor(window, cx)))
                .into_any_element(),
        };
        let _ = window;
        h_flex()
            .debug_selector(|| "file-head".into())
            .flex_none()
            .h(px(40.))
            .px_3()
            .gap_2()
            .items_center()
            .bg(p.background)
            .child(CrumbRow::new("file-crumbs", roots, self.path.clone(), pick).size(px(12.)))
            .children(toggle)
            .child(reload)
            .child(open)
    }

    fn body(&self, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let p = cx.palette().clone();
        let centered = |el: gpui_kit::Div| el.flex().flex_1().min_h_0().items_center().justify_center().px_6().text_center();
        match (&self.read, self.read.reply()) {
            (Read::Error { message, .. }, None) => centered(div())
                .debug_selector(|| "file-error".into())
                .text_size(px(12.))
                .line_height(px(19.5))
                .text_color(p.error_foreground)
                .child(message.clone())
                .into_any_element(),
            (_, None) => centered(div()).child(spinner(cx)).into_any_element(),
            (_, Some(reply)) if reply.binary => centered(div())
                .debug_selector(|| "file-binary".into())
                .text_size(px(12.))
                .text_color(p.muted_foreground)
                .child(words::binary(reply.size))
                .into_any_element(),
            _ if self.rendered(cx) => {
                let preview = super::markdown::preview(&self.markdown, cx);
                scroll_area("file-markdown", &self.scroll, preview, window, cx).flex_1().min_h_0().into_any_element()
            }
            _ => div().debug_selector(|| "file-code".into()).flex_1().min_h_0().child(self.code.clone()).into_any_element(),
        }
    }
}

impl Render for FileTab {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let running = self.facts().map(|f| f.running);
        let base = v_flex().id("file-tab").size_full().overflow_hidden().bg(p.background);
        if running == Some(false) {
            return base.child(
                div()
                    .flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .px_6()
                    .text_center()
                    .text_size(px(12.))
                    .text_color(p.muted_foreground)
                    .child(words::NOT_RUNNING),
            );
        }
        let note = self.read.reply().filter(|r| r.truncated).map(|r| {
            div()
                .debug_selector(|| "file-truncated".into())
                .flex_none()
                .px_3()
                .py(px(6.))
                .text_size(px(11.))
                .text_color(p.muted_foreground)
                .child(words::truncated(r.shown, r.size))
        });
        base.child(self.head(window, cx)).children(note).child(self.body(window, cx))
    }
}

/// The name a file tab is titled by.
pub fn title(path: &str) -> SharedString {
    base_name(path).to_owned().into()
}
