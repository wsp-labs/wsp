// SPDX-License-Identifier: AGPL-3.0-only
//! Quick open and search in files (`files/FileFinder.tsx`), on gpui-kit's dialog layer as the palette's dialog: the
//! thread's project folder searched over its daemon with `fs.search`, a file by the letters of its name or a line by
//! the words in it. Picking one opens it as its own tab in the Files pane, at the line for a line.

use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    div, prelude::*, px, uniform_list, App, Context, Entity, FocusHandle, Focusable, KeyDownEvent, MouseButton, Pixels, ScrollStrategy,
    SharedString, Subscription, Task, UniformListScrollHandle, Window,
};
use serde_json::{json, Value};

use super::at;
use super::paths::join;
use super::rank::{rank, LIMIT};
use super::words::finder as words;
use crate::screens::dialogs::layer::{self, Closes};
use crate::screens::pr_pane::daemon::DaemonLink;
use crate::screens::Host;
use crate::ui::card::keycap;
use crate::ui::{size, ActivePalette as _, Glyph};

/// How long the finder waits for the typing to rest before it asks the daemon.
const SETTLE: Duration = Duration::from_millis(120);
/// `max-w-xl` and `max-h-105`: the popup.
const WIDTH: Pixels = px(576.);
const MAX_HEIGHT: Pixels = px(420.);
/// One hit: its title over its folder or `path:line`.
const ROW_HEIGHT: Pixels = px(44.);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// `mod+p`: a file by its name.
    Files,
    /// `mod+shift+f`: a line by its words.
    Text,
}

impl Mode {
    fn wire(self) -> &'static str {
        match self {
            Mode::Files => "files",
            Mode::Text => "text",
        }
    }
}

/// One hit as the finder lists it: the path relative to the folder searched, and the line for a line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Hit {
    pub title: SharedString,
    pub description: SharedString,
    pub path: String,
    pub line: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Answer {
    Idle,
    Ready { hits: Vec<Hit>, truncated: bool },
    Error(String),
}

pub struct Finder {
    workspace_id: String,
    mode: Mode,
    input: Entity<InputState>,
    link: Entity<DaemonLink>,
    facts: Option<at::Read>,
    answer: Answer,
    highlighted: usize,
    list: UniformListScrollHandle,
    focus: FocusHandle,
    asking: Option<Task<()>>,
    _read: Task<()>,
    _subs: Vec<Subscription>,
}

/// The name the finder stands under on the dialogs' layer.
pub const FINDER: &str = "file-finder";

/// Opens the finder on the dialogs' layer for the thread's workspace, its field focused.
pub fn open(mode: Mode, host: &Host, workspace_id: &str, window: &mut Window, cx: &mut App) -> Entity<Finder> {
    let finder = cx.new(|cx| Finder::new(mode, host.clone(), workspace_id.to_owned(), window, cx));
    layer::open(FINDER, finder.clone().into(), Closes::Outside, None, window, cx);
    let focus = finder.read(cx).input.read(cx).focus_handle(cx);
    window.focus(&focus, cx);
    finder
}

impl Finder {
    fn new(mode: Mode, host: Host, workspace_id: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let placeholder = if mode == Mode::Files { words::FILES_PLACEHOLDER } else { words::TEXT_PLACEHOLDER };
        let input = cx.new(|cx| InputState::new(window, cx).placeholder(placeholder));
        let link = DaemonLink::of(&host, &workspace_id, cx);
        let subs = vec![cx.subscribe(&input, |this, _, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                this.ask(cx)
            }
        })];
        // A workspace that naps or wakes while the finder is open moves the folder searched, so the search runs again.
        let read = at::follow(&host, workspace_id.clone(), None, cx, |this: &mut Self, read, cx| {
            let was = this.facts.is_some().then(|| this.folder());
            this.facts = Some(read);
            if was != Some(this.folder()) {
                this.ask(cx);
            }
            cx.notify();
        });
        Finder {
            workspace_id,
            mode,
            input,
            link,
            facts: None,
            answer: Answer::Idle,
            highlighted: 0,
            list: UniformListScrollHandle::new(),
            focus: cx.focus_handle(),
            asking: None,
            _read: read,
            _subs: subs,
        }
    }

    /// Types `query` into the field, as a capture opens the finder on it.
    pub fn set_query(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.input.update(cx, |input, cx| input.set_value(query.to_owned(), window, cx));
        self.ask(cx);
    }

    pub fn mode(&self) -> Mode {
        self.mode
    }

    pub fn answer(&self) -> &Answer {
        &self.answer
    }

    pub fn input(&self) -> &Entity<InputState> {
        &self.input
    }

    /// The folder searched: the project's, while its workspace runs.
    fn folder(&self) -> Option<String> {
        self.facts.as_ref().and_then(|f| f.as_ref().ok()).filter(|f| f.running).and_then(|f| f.project.clone())
    }

    fn query(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }

    /// Asks the daemon once the typing rests; an empty search in text asks nothing.
    fn ask(&mut self, cx: &mut Context<Self>) {
        let raw = self.query(cx);
        self.highlighted = 0;
        let Some(folder) = self.folder() else {
            self.answer = Answer::Idle;
            self.asking = None;
            cx.notify();
            return;
        };
        if self.mode == Mode::Text && raw.trim().is_empty() {
            self.answer = Answer::Idle;
            self.asking = None;
            cx.notify();
            return;
        }
        let asked = if self.mode == Mode::Files { super::rank::file_query(&raw) } else { raw.trim().to_owned() };
        let link = self.link.clone();
        let mode = self.mode;
        self.asking = Some(cx.spawn(async move |this, cx| {
            cx.background_executor().timer(SETTLE).await;
            let request =
                cx.update(|cx| link.read(cx).request("fs.search", json!({ "path": folder, "query": asked, "mode": mode.wire() })));
            // A files search carries up to 5,000 paths to rank, which stays off the window's thread.
            let answer = match request.await {
                Ok(reply) => cx.background_executor().spawn(async move { read_hits(mode, &reply, &raw) }).await,
                Err(refused) => Answer::Error(refused.message),
            };
            _ = this.update(cx, |this, cx| {
                this.answer = answer;
                this.highlighted = 0;
                cx.notify();
            });
        }));
        cx.notify();
    }

    fn hits(&self) -> &[Hit] {
        match &self.answer {
            Answer::Ready { hits, .. } => hits,
            _ => &[],
        }
    }

    fn step(&mut self, by: isize, cx: &mut Context<Self>) {
        let n = self.hits().len();
        if n == 0 {
            return;
        }
        self.highlighted = (self.highlighted as isize + by).clamp(0, n as isize - 1) as usize;
        self.list.scroll_to_item(self.highlighted, ScrollStrategy::Top);
        cx.notify();
    }

    fn enter(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(hit) = self.hits().get(self.highlighted).cloned() {
            self.run(&hit, window, cx);
        }
    }

    /// Opens the hit's file as a tab in the Files pane, at its line for a line, and closes the finder.
    pub fn run(&mut self, hit: &Hit, window: &mut Window, cx: &mut Context<Self>) {
        let Some(folder) = self.folder() else { return };
        super::open_file(&self.workspace_id, &join(&folder, &hit.path), hit.line, cx);
        layer::close(FINDER, window, cx);
    }

    fn keys(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        match event.keystroke.key.as_str() {
            "up" => self.step(-1, cx),
            "down" => self.step(1, cx),
            "enter" => self.enter(window, cx),
            "escape" => layer::close(FINDER, window, cx),
            _ => return,
        }
        cx.stop_propagation();
    }

    /// What the list says where it has no hits.
    pub fn empty_line(&self, cx: &App) -> SharedString {
        let typed = !self.query(cx).trim().is_empty();
        if let Some(Err(refused)) = &self.facts {
            return refused.clone().into();
        }
        let line = match (&self.answer, self.folder()) {
            (_, None) if self.facts.is_some() => words::NOT_RUNNING,
            (_, None) => "",
            (Answer::Error(_), _) => "",
            (_, _) if self.mode == Mode::Text && !typed => words::TYPE_WORD,
            (Answer::Ready { .. }, _) => {
                if self.mode == Mode::Files {
                    words::FILES_NONE
                } else {
                    words::TEXT_NONE
                }
            }
            _ => "",
        };
        line.into()
    }

    fn row(&self, ix: usize, hit: &Hit, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let p = cx.palette().clone();
        let mono = cx.theme().mono_font_family.clone();
        let on = ix == self.highlighted;
        let glyph = if self.mode == Mode::Files { Glyph::File } else { Glyph::TextSearch };
        let pressed = hit.clone();
        h_flex()
            .id(("finder-hit", ix))
            .debug_selector(move || format!("finder-hit-{ix}"))
            .w_full()
            .h(ROW_HEIGHT)
            .px_2()
            .gap_2()
            .items_center()
            .rounded(px(6.))
            .cursor_pointer()
            .when(on, |el| el.bg(p.accent))
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                if this.highlighted != ix {
                    this.highlighted = ix;
                    cx.notify();
                }
            }))
            .on_click(cx.listener(move |this, _, window, cx| this.run(&pressed, window, cx)))
            .child(Icon::new(glyph).with_size(px(16.)).flex_none().text_color(p.muted_foreground))
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .child(
                        div()
                            .truncate()
                            .text_size(if self.mode == Mode::Text { px(12.) } else { px(14.) })
                            .line_height(px(20.))
                            .text_color(p.foreground)
                            .when(self.mode == Mode::Text, |el| el.font_family(mono.clone()))
                            .child(hit.title.clone()),
                    )
                    .when(!hit.description.is_empty(), |el| {
                        el.child(
                            div()
                                .truncate()
                                .font_family(mono.clone())
                                .text_size(px(12.))
                                .line_height(px(16.))
                                .text_color(p.muted_foreground)
                                .child(hit.description.clone()),
                        )
                    }),
            )
            .into_any_element()
    }
}

/// The hits of an fs.search reply: a file's ranked by its name before its folders, at most fifty; a line's as the
/// daemon found them, at most its 500.
pub fn read_hits(mode: Mode, reply: &serde_json::Map<String, Value>, raw: &str) -> Answer {
    let hits = reply.get("hits").and_then(Value::as_array).cloned().unwrap_or_default();
    let truncated = reply.get("truncated").and_then(Value::as_bool).unwrap_or(false);
    let hits = match mode {
        Mode::Files => {
            let paths: Vec<String> = hits.iter().filter_map(|h| h.get("path")?.as_str().map(str::to_owned)).collect();
            rank(&paths, raw, LIMIT)
                .into_iter()
                .map(|m| Hit { title: m.name.into(), description: m.folder.into(), path: m.path, line: None })
                .collect()
        }
        Mode::Text => hits
            .iter()
            .filter_map(|h| {
                let path = h.get("path")?.as_str()?.to_owned();
                let line = h.get("line").and_then(Value::as_u64).map(|l| l as usize);
                let text = h.get("text").and_then(Value::as_str).unwrap_or_default().trim().to_owned();
                Some(Hit { title: text.into(), description: format!("{path}:{}", line.unwrap_or(1)).into(), path, line })
            })
            .collect(),
    };
    Answer::Ready { hits, truncated }
}

impl Focusable for Finder {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.input.read(cx).focus_handle(cx)
    }
}

impl Render for Finder {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let (label, group) = match self.mode {
            Mode::Files => (words::FILES_LABEL, words::FILES_GROUP),
            Mode::Text => (words::TEXT_LABEL, words::TEXT_GROUP),
        };
        let count = self.hits().len();
        let more = match &self.answer {
            Answer::Ready { truncated: true, .. } => Some(if self.mode == Mode::Files { words::MORE_FILES } else { words::MORE_LINES }),
            _ => None,
        };
        let empty: SharedString = match &self.answer {
            Answer::Error(message) if self.folder().is_some() => message.clone().into(),
            _ => self.empty_line(cx),
        };
        let field = h_flex().flex_none().px(px(12.)).py(px(6.)).child(
            Input::new(&self.input)
                .appearance(false)
                .h(px(40.))
                .text_size(px(15.))
                .prefix(Icon::new(Glyph::Search).with_size(px(16.)).text_color(p.muted_foreground)),
        );
        let results = if count == 0 {
            div()
                .flex_none()
                .debug_selector(|| "finder-empty".into())
                .py(px(40.))
                .text_center()
                .text_size(px(14.))
                .text_color(p.muted_foreground)
                .child(empty)
                .into_any_element()
        } else {
            v_flex()
                .flex_1()
                .min_h_0()
                .overflow_hidden()
                .p_2()
                .child(
                    div()
                        .px(px(9.))
                        .py(px(6.))
                        .text_size(px(12.))
                        .font_weight(gpui_kit::FontWeight::MEDIUM)
                        .text_color(p.muted_foreground)
                        .child(group),
                )
                .child(
                    uniform_list(
                        "finder-hits",
                        count,
                        cx.processor(|this, range: std::ops::Range<usize>, _, cx| {
                            let hits: Vec<(usize, Hit)> = range.filter_map(|ix| this.hits().get(ix).cloned().map(|h| (ix, h))).collect();
                            hits.into_iter().map(|(ix, hit)| this.row(ix, &hit, cx)).collect::<Vec<_>>()
                        }),
                    )
                    .debug_selector(|| "finder-hits".into())
                    .h(ROW_HEIGHT * count as f32)
                    .flex_shrink(1.)
                    .min_h_0()
                    .track_scroll(&self.list),
                )
                .into_any_element()
        };
        let foot = h_flex()
            .flex_none()
            .debug_selector(|| "finder-foot".into())
            .justify_between()
            .gap_3()
            .px_4()
            .py(px(10.))
            .rounded_b(px(13.))
            .bg(p.foreground.opacity(0.025))
            .text_size(px(14.))
            .font_weight(gpui_kit::FontWeight::MEDIUM)
            .text_color(p.muted_foreground)
            .child(
                h_flex()
                    .gap_3()
                    .child(h_flex().gap(px(6.)).child(keycap("↑".into(), cx)).child(keycap("↓".into(), cx)).child(words::NAVIGATE))
                    .child(h_flex().gap(px(6.)).child(keycap("Enter".into(), cx)).child(words::OPEN))
                    .child(h_flex().gap(px(6.)).child(keycap("Esc".into(), cx)).child(words::CLOSE)),
            )
            .children(more.map(|more| {
                div().debug_selector(|| "finder-more".into()).text_size(px(12.)).font_weight(gpui_kit::FontWeight::NORMAL).child(more)
            }));
        let popup = v_flex()
            .id("file-finder")
            .debug_selector(move || format!("file-finder-{}", label))
            .track_focus(&self.focus)
            .capture_key_down(cx.listener(Self::keys))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .w(WIDTH)
            .max_w_full()
            .max_h(MAX_HEIGHT)
            .overflow_hidden()
            .rounded(size::DIALOG_RADIUS)
            .border_1()
            .border_color(p.popover_edge)
            .bg(p.popover)
            .shadow(p.dialog_shadow.clone())
            .child(field)
            .child(results)
            .child(foot);
        // The layer centres a dialog; the command dialog's viewport keeps it 10vh from top and foot (4vh, at least 16 px,
        // when narrow) inside the layer's 16 px, and a press beside it is a press on the scrim.
        let view = window.viewport_size();
        let gap = if view.width >= px(640.) { view.height * 0.1 } else { (view.height * 0.04).max(px(16.)) };
        div()
            .w(view.width - px(32.))
            .h(view.height - px(32.))
            .flex()
            .flex_col()
            .items_center()
            .py((gap - px(16.)).max(px(0.)))
            .on_mouse_down(MouseButton::Left, |_, window, cx| layer::dismiss(window, cx))
            .child(popup)
    }
}
