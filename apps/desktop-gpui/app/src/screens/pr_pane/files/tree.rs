// SPDX-License-Identifier: AGPL-3.0-only
//! The Files tab (`files/FilesSurface.tsx` over `components/files/FileBrowserPanel.tsx`): the crumb row naming the
//! folder with the pin and New thread here, then refresh and Search files, then the tree rooted at the folder, filled
//! one folder at a time over the daemon's `fs.list` with gitignore on. A file opens as its own tab beside this one.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Duration;

use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{
    canvas, div, prelude::*, px, uniform_list, App, ClipboardItem, Context, Entity, FocusHandle, Focusable, KeyDownEvent, MouseButton,
    MouseDownEvent, Pixels, Point, SharedString, Subscription, Task, UniformListScrollHandle, Window,
};
use serde_json::json;

use super::at::{self, Facts};
use super::crumbs::CrumbRow;
use super::paths::{parent_within, roots};
use super::rows::{self, level_of, Kind, Level, Levels, Row};
use super::words;
use crate::screens::pr_pane::daemon::DaemonLink;
use crate::screens::pr_pane::diff::fold_chevron;
use crate::screens::{navigate, Host, Route};
use crate::ui::context_menu::{ContextMenu, MenuItem};
use crate::ui::{motion, ActivePalette as _, Button, Glyph};

/// The tree's compact rows: 24 px, 12 px type, each level 20 px in.
pub const ROW_HEIGHT: Pixels = px(24.);
const INDENT: f32 = 20.;
/// What a row keeps beside its name: its side pads, the chevron or glyph and the gap after it.
const ROW_CHROME: f32 = 6.4 * 2. + 16. + 5. + 4.;

pub struct Tree {
    workspace_id: String,
    thread_id: Option<String>,
    link: Entity<DaemonLink>,
    /// The workspace's facts as the host last gave them, or its sentence for why not.
    facts: Option<at::Read>,
    /// The link's dial was refused, with the host's sentence; dialled again on the link's or the workspace's next move.
    down: Option<String>,
    dialled: bool,
    dialling: Option<Task<()>>,
    /// The folder the tree is rooted at now.
    folder: Option<String>,
    levels: Levels,
    open: HashSet<String>,
    loading: HashMap<String, Task<()>>,
    search: Entity<InputState>,
    query: String,
    rows: Vec<Row>,
    /// The folders under the folder shown that failed to list, kept with the rows.
    errors: Vec<(String, String)>,
    selected: Option<String>,
    list: UniformListScrollHandle,
    width: Pixels,
    focus: FocusHandle,
    focused_once: bool,
    /// A row's right-click menu, drawn here off the Mac.
    menu: ContextMenu,
    _subs: Vec<Subscription>,
    _watch: Task<()>,
    _hear: Task<()>,
}

/// Past this many folders listed, the closed ones' listings go; the folder shown and every open folder stay.
pub const KEPT_LEVELS: usize = 200;

impl Tree {
    pub fn new(host: Host, workspace_id: String, thread_id: Option<String>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let link = DaemonLink::of(&host, &workspace_id, cx);
        let typed = crate::screens::pr_pane::shot().and_then(|s| s.strip_prefix("search:")).unwrap_or_default().to_owned();
        let search = cx.new(|cx| {
            let mut state = InputState::new(window, cx).placeholder(words::SEARCH_FILES).clean_on_escape();
            state.set_value(typed.clone(), window, cx);
            state
        });
        let subs = vec![
            cx.observe(&link, |this, _, cx| {
                this.follow(cx);
                if this.down.is_some() {
                    this.dial(cx);
                }
            }),
            cx.subscribe(&search, |this, search, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    this.query = search.read(cx).value().to_string();
                    this.rebuild(cx);
                }
            }),
        ];
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let watching = at::watch(&host, workspace_id.clone(), thread_id.clone(), tx);
        let watch = cx.spawn(async move |_, _| watching.await);
        let hear = cx.spawn(async move |this, cx| {
            while let Some(read) = futures::StreamExt::next(&mut rx).await {
                if this.update(cx, |this, cx| this.take_facts(read, cx)).is_err() {
                    return;
                }
            }
        });
        Tree {
            workspace_id,
            thread_id,
            link,
            facts: None,
            down: None,
            dialled: false,
            dialling: None,
            folder: None,
            levels: Levels::new(),
            open: HashSet::new(),
            loading: HashMap::new(),
            search,
            query: typed,
            rows: Vec::new(),
            errors: Vec::new(),
            selected: None,
            list: UniformListScrollHandle::new(),
            width: px(0.),
            focus: cx.focus_handle(),
            focused_once: false,
            menu: ContextMenu::default(),
            _subs: subs,
            _watch: watch,
            _hear: hear,
        }
    }

    fn facts(&self) -> Option<&Facts> {
        self.facts.as_ref().and_then(|f| f.as_ref().ok())
    }

    fn running(&self) -> bool {
        self.facts().is_some_and(|f| f.running)
    }

    /// Takes what the host says of the workspace now: the tree follows the agent's folder as it moves, and a link
    /// refused or never dialled is dialled once the workspace runs.
    fn take_facts(&mut self, read: at::Read, cx: &mut Context<Self>) {
        self.facts = Some(read);
        if self.running() && (!self.dialled || self.down.is_some()) {
            self.dial(cx);
        }
        self.follow(cx);
    }

    /// Dials the link with a ping and waits for the daemon's hello to name its home; a refusal is kept to be said.
    fn dial(&mut self, cx: &mut Context<Self>) {
        if self.dialling.is_some() || !self.running() {
            return;
        }
        self.dialled = true;
        let link = self.link.clone();
        let ping = link.read(cx).request("ping", json!({}));
        self.dialling = Some(cx.spawn(async move |this, cx| {
            let dialled = ping.await;
            let up = dialled.is_ok();
            _ = this.update(cx, |this, cx| {
                this.down = dialled.err().map(|refused| refused.message);
                cx.notify();
            });
            for _ in 0..50 {
                if !up || cx.update(|cx| link.read(cx).root().is_some()) {
                    break;
                }
                cx.background_executor().timer(Duration::from_millis(100)).await;
            }
            _ = this.update(cx, |this, cx| {
                this.dialling = None;
                this.follow(cx);
            });
        }));
    }

    pub fn levels(&self) -> &Levels {
        &self.levels
    }

    pub fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub fn folder(&self) -> Option<&str> {
        self.folder.as_deref()
    }

    /// The roots the daemon browses: its home, then the project's folder.
    pub fn roots(&self, cx: &App) -> Vec<String> {
        let project = self.facts().and_then(|f| f.project.as_deref());
        roots(self.link.read(cx).root().as_deref(), project)
    }

    fn pinned(&self, cx: &App) -> bool {
        self.thread_id.as_deref().is_some_and(|t| self.link.read(cx).pinned(t).is_some())
    }

    /// Moves the tree to the folder the rule picks now, and asks again for what failed once the link is back.
    fn follow(&mut self, cx: &mut Context<Self>) {
        let Some(facts) = self.facts() else { return };
        let link = self.link.read(cx);
        let folder = link.folder(self.thread_id.as_deref(), facts.cwd.as_deref(), facts.project.as_deref());
        let up = link.up();
        if folder != self.folder {
            self.folder = folder.clone();
            self.open.clear();
            if let Some(folder) = &folder {
                let inside = |dir: &str| dir == folder || dir.starts_with(&format!("{}/", folder.trim_end_matches('/')));
                self.levels.retain(|dir, _| inside(dir));
                self.loading.retain(|dir, _| inside(dir));
            }
            self.search.update(cx, |_, cx| cx.notify());
            if let Some(folder) = folder {
                self.ensure(&folder, cx);
            }
        } else if up {
            let failed: Vec<String> = self.levels.iter().filter(|(_, l)| l.error.is_some()).map(|(d, _)| d.clone()).collect();
            for dir in failed {
                self.load(&dir, cx);
            }
        }
        self.rebuild(cx);
    }

    /// Lists a folder nobody asked for yet.
    fn ensure(&mut self, dir: &str, cx: &mut Context<Self>) {
        if !self.levels.contains_key(dir) {
            self.load(dir, cx);
        }
    }

    /// Lists `dir` again, keeping its last good listing up meanwhile; a read already in flight is left to land.
    pub fn load(&mut self, dir: &str, cx: &mut Context<Self>) {
        if self.loading.contains_key(dir) {
            return;
        }
        self.levels.entry(dir.to_owned()).or_default().pending = true;
        let asked = self.link.read(cx).request("fs.list", json!({ "path": dir, "gitignore": true }));
        let at = dir.to_owned();
        let task = cx.spawn(async move |this, cx| {
            let answer = asked.await;
            _ = this.update(cx, |this, cx| {
                this.loading.remove(&at);
                let next = match answer {
                    Ok(reply) => level_of(&at, &reply),
                    Err(refused) => {
                        let last = this.levels.get(&at).cloned().unwrap_or_default();
                        Level { pending: false, error: Some(refused.message), ..last }
                    }
                };
                this.levels.insert(at, next);
                this.trim();
                this.rebuild(cx);
            });
        });
        self.loading.insert(dir.to_owned(), task);
        cx.notify();
    }

    /// Past [`KEPT_LEVELS`], drops the listings of folders not shown open, to be listed again when opened.
    fn trim(&mut self) {
        if self.levels.len() <= KEPT_LEVELS {
            return;
        }
        let (folder, open, loading) = (self.folder.clone(), &self.open, &self.loading);
        self.levels.retain(|dir, _| Some(dir) == folder.as_ref() || open.contains(dir) || loading.contains_key(dir));
    }

    fn rebuild(&mut self, cx: &mut Context<Self>) {
        (self.rows, self.errors) = match &self.folder {
            Some(folder) => (rows::rows(folder, &self.levels, &self.open, &self.query), rows::errors(folder, &self.levels)),
            None => (Vec::new(), Vec::new()),
        };
        cx.notify();
    }

    /// A press on a row: a folder opens or closes, asking for its listing the first time; a file opens its tab.
    pub fn press(&mut self, row: &Row, cx: &mut Context<Self>) {
        match row.kind {
            Kind::Folder { open } => {
                if open {
                    self.open.remove(&row.path);
                } else {
                    self.open.insert(row.path.clone());
                    self.ensure(&row.path, cx);
                }
                self.rebuild(cx);
            }
            Kind::File => {
                self.selected = Some(row.path.clone());
                super::open_file(&self.workspace_id, &row.path, None, cx);
                cx.notify();
            }
            Kind::Note => {}
        }
    }

    /// A right-click on a row (`fileActions.ts`): Open file and Show in Changes, held on a folder with why, then Copy
    /// path.
    pub fn context(&mut self, row: &Row, at: Point<Pixels>, window: &mut Window, cx: &mut Context<Self>) {
        if row.kind == Kind::Note {
            return;
        }
        let folder = matches!(row.kind, Kind::Folder { .. });
        let item = |label: &'static str, group: &'static str, refusal: Option<&'static str>| MenuItem {
            label: label.into(),
            group,
            refusal: refusal.filter(|_| folder).map(Into::into),
            keys: None,
            destructive: false,
        };
        let items = vec![
            item(words::OPEN_FILE, "open", Some(words::FOLDER_OPENS_IN_TREE)),
            item(words::SHOW_IN_CHANGES, "open", Some(words::ONLY_FILES_HAVE_CHANGES)),
            item(words::COPY_PATH, "copy", None),
        ];
        let path = row.path.clone();
        self.menu.open(at, items, window, cx, move |this, pick, _, cx| match pick {
            Some(0) => super::open_file(&this.workspace_id, &path, None, cx),
            Some(1) => super::show_in_changes(&this.workspace_id, &path, cx),
            Some(2) => cx.write_to_clipboard(ClipboardItem::new_string(path)),
            _ => {}
        });
    }

    pub fn menu_open(&self) -> bool {
        self.menu.is_open()
    }

    pub fn refresh(&mut self, cx: &mut Context<Self>) {
        let Some(folder) = self.folder.clone() else { return };
        let mut dirs = rows::loaded(&folder, &self.levels);
        if dirs.is_empty() {
            dirs.push(folder);
        }
        for dir in dirs {
            self.load(&dir, cx);
        }
    }

    pub fn pin_toggle(&mut self, cx: &mut Context<Self>) {
        let (Some(thread), Some(folder)) = (self.thread_id.clone(), self.folder.clone()) else { return };
        let pinned = self.pinned(cx);
        self.link.update(cx, |link, cx| if pinned { link.unpin(&thread, cx) } else { link.pin(&thread, &folder, cx) });
    }

    /// Goes to `folder` by pinning the panes there, as a crumb, the root menu and the up keys do.
    pub fn go(&mut self, folder: &str, cx: &mut Context<Self>) {
        let Some(thread) = self.thread_id.clone() else { return };
        self.link.update(cx, |link, cx| link.pin(&thread, folder, cx));
    }

    /// Backspace or Alt+Up on the pane, unless the search field has them: the folder above, inert at a root's edge.
    fn up_key(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let k = &event.keystroke;
        let m = &k.modifiers;
        let up = if m.control || m.platform || m.shift { false } else { (k.key == "backspace" && !m.alt) || (k.key == "up" && m.alt) };
        if !up || self.search.read(cx).focus_handle(cx).is_focused(window) {
            return;
        }
        let Some(folder) = self.folder.clone() else { return };
        if let Some(above) = parent_within(&self.roots(cx), &folder) {
            cx.stop_propagation();
            self.go(&above, cx);
        }
    }

    fn sentence(text: impl Into<SharedString>, refused: bool, cx: &App) -> gpui_kit::Div {
        div()
            .flex()
            .flex_1()
            .min_h_0()
            .items_center()
            .justify_center()
            .px_6()
            .text_center()
            .text_size(px(12.))
            .line_height(px(19.5))
            .text_color(if refused { cx.palette().error_foreground } else { cx.palette().muted_foreground })
            .child(text.into())
    }

    fn location(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.weak_entity();
        let pick: super::crumbs::Pick = Rc::new(move |folder, _, cx| _ = me.update(cx, |this, cx| this.go(folder, cx)));
        let pinned = self.pinned(cx);
        let me = cx.weak_entity();
        let me2 = cx.weak_entity();
        let workspace = self.workspace_id.clone();
        h_flex()
            .debug_selector(|| "files-location".into())
            .flex_none()
            .h(px(28.))
            .px(px(6.))
            .gap(px(2.))
            .items_center()
            .child(CrumbRow::new("files-crumbs", self.roots(cx), self.folder.clone().unwrap_or_default(), pick))
            .child(
                div().debug_selector(|| "files-pin-button".into()).child(
                    Button::icon_xs("files-pin", if pinned { Glyph::PinOff } else { Glyph::Pin })
                        .side(px(20.))
                        .tooltip(if pinned { words::FOLLOW } else { words::STAY })
                        .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.pin_toggle(cx))),
                ),
            )
            .child(Button::icon_xs("files-new-thread", Glyph::MessageSquarePlus).side(px(20.)).tooltip(words::NEW_THREAD_HERE).on_click(
                move |_, _, cx| {
                    _ = me2.update(cx, |_, cx| navigate(Route::Screen(format!("composer:{workspace}").into()), cx));
                },
            ))
    }

    fn subheader(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let pending = self.folder.as_ref().and_then(|f| self.levels.get(f)).is_some_and(|l| l.pending);
        let me = cx.weak_entity();
        h_flex()
            .debug_selector(|| "files-subheader".into())
            .flex_none()
            .h(px(40.))
            .px_2()
            .gap_1()
            .items_center()
            .child(
                Button::icon_xs("files-refresh", Glyph::RotateCw)
                    .busy(pending)
                    .tooltip(if pending { words::REFRESHING } else { words::REFRESH })
                    .on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.refresh(cx))),
            )
            .child(
                div().debug_selector(|| "files-search".into()).flex_1().min_w_0().h(px(28.)).child(
                    Input::new(&self.search)
                        .appearance(false)
                        .h(px(28.))
                        .px_2()
                        .text_size(px(13.))
                        .text_color(p.foreground)
                        .prefix(Icon::new(Glyph::Search).with_size(px(14.)).text_color(p.muted_foreground)),
                ),
            )
    }

    fn tree(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let me = cx.weak_entity();
        let count = self.rows.len();
        let measure = cx.weak_entity();
        div()
            .relative()
            .flex_1()
            .min_h_0()
            .child(
                canvas(
                    move |bounds, _, cx| {
                        _ = measure.update(cx, |this, cx| {
                            if this.width != bounds.size.width {
                                this.width = bounds.size.width;
                                cx.notify();
                            }
                        });
                    },
                    |_, _, _, _| {},
                )
                .absolute()
                .size_full(),
            )
            .child(
                uniform_list(
                    "files-tree",
                    count,
                    cx.processor(move |this, range: std::ops::Range<usize>, window, cx| {
                        range.filter_map(|ix| this.rows.get(ix).cloned()).map(|row| this.row(row, &me, window, cx)).collect::<Vec<_>>()
                    }),
                )
                .debug_selector(|| "files-tree".into())
                .size_full()
                .track_scroll(&self.list),
            )
    }

    fn row(&self, row: Row, me: &gpui_kit::WeakEntity<Self>, window: &mut Window, cx: &mut Context<Self>) -> gpui_kit::AnyElement {
        let p = cx.palette().clone();
        let id = SharedString::from(format!("tree-row-{}", row.path));
        let hovered = motion::hovered(id.clone(), window, cx);
        let selected = self.selected.as_deref() == Some(row.path.as_str());
        let note = row.kind == Kind::Note;
        let fill = match (selected, *hovered.read(cx) && !note) {
            (true, _) => p.foreground.opacity(0.12),
            (false, true) => p.foreground.opacity(0.07),
            (false, false) => p.foreground.opacity(0.),
        };
        let fill = motion::ease_ink(id.clone(), "fill", fill, motion::COLORS, window, cx);
        let indent = px(6.4 + INDENT * row.depth as f32);
        let name_w = (self.width - indent - px(ROW_CHROME)).max(px(24.));
        let lead = match row.kind {
            Kind::Folder { open } => fold_chevron(id.clone(), open, window, cx).into_any_element(),
            Kind::File => Icon::new(Glyph::File).with_size(px(14.)).flex_none().text_color(p.muted_foreground).into_any_element(),
            Kind::Note => div().w(px(14.)).flex_none().into_any_element(),
        };
        let path = row.path.clone();
        let (me, pressed) = (me.clone(), row.clone());
        h_flex()
            .id(id)
            .debug_selector(move || format!("tree-{path}"))
            .h(ROW_HEIGHT)
            .mx(px(1.6))
            .pl(indent)
            .pr(px(6.4))
            .gap(px(5.))
            .items_center()
            .rounded(px(5.))
            .bg(fill)
            .text_size(px(12.))
            .line_height(px(16.))
            .text_color(if note { p.muted_foreground } else { p.foreground })
            .on_hover(motion::track_hover(hovered))
            .when(!note, |el| {
                let (me2, right) = (me.clone(), pressed.clone());
                el.cursor_pointer().on_click(move |_, _, cx| _ = me.update(cx, |this, cx| this.press(&pressed, cx))).on_mouse_down(
                    MouseButton::Right,
                    move |e: &MouseDownEvent, window, cx| {
                        _ = me2.update(cx, |this, cx| this.context(&right, e.position, window, cx));
                    },
                )
            })
            .child(lead)
            .child(div().w(name_w).flex_none().truncate().child(SharedString::from(row.name)))
            .into_any_element()
    }
}

impl Focusable for Tree {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl Render for Tree {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        // The pane takes focus as it is shown, so Backspace goes up a folder at once.
        if !self.focused_once {
            self.focused_once = true;
            self.focus.focus(window, cx);
        }
        let base = v_flex().id("files-pane").track_focus(&self.focus).on_key_down(cx.listener(Self::up_key)).size_full().bg(p.background);
        let waiting = || div().flex().flex_1().items_center().justify_center().child(crate::ui::spinner(cx)).into_any_element();
        let said = match (&self.facts, &self.down) {
            (None, _) => return base.child(waiting()),
            (Some(Err(refused)), _) => Some((refused.clone(), true)),
            (Some(Ok(facts)), _) if !facts.running => Some((words::NOT_RUNNING.to_owned(), false)),
            (_, Some(refused)) => Some((refused.clone(), true)),
            _ => None,
        };
        if let Some((line, refused)) = said {
            return base.child(Self::sentence(line, refused, cx).debug_selector(|| "files-said".into()));
        }
        if self.folder.is_none() {
            return base.child(waiting());
        }
        let folder = self.folder.clone().unwrap_or_default();
        let root_level = self.levels.get(&folder);
        let root_error = root_level.filter(|l| l.entries.is_none()).and_then(|l| l.error.clone());
        let errors = self.errors.clone();
        let mono = cx.theme().mono_font_family.clone();
        let menu = self.menu.overlay();
        base.child(self.location(cx))
            .child(self.subheader(cx))
            .map(|el| match root_error {
                Some(error) => el.child(
                    div()
                        .debug_selector(|| "files-root-error".into())
                        .p_4()
                        .text_size(px(12.))
                        .line_height(px(19.5))
                        .text_color(p.error_foreground)
                        .child(error),
                ),
                None => el.child(self.tree(cx)),
            })
            .children(errors.into_iter().map(|(dir, message)| {
                div()
                    .debug_selector({
                        let dir = dir.clone();
                        move || format!("files-error-{dir}")
                    })
                    .flex_none()
                    .px_3()
                    .py(px(6.))
                    .truncate()
                    .font_family(mono.clone())
                    .text_size(px(11.))
                    .text_color(p.error_foreground)
                    .child(format!("{dir}: {message}"))
            }))
            .children(menu)
    }
}
