// SPDX-License-Identifier: AGPL-3.0-only
//! Open, in the thread header (`files/OpenSplit.tsx`), and the road it shares with a file tab's Open in editor
//! (`files/OpenInEditor.tsx`, `files/openCopy.ts`, `files/EditorConsent.tsx`): the thread's copy in an editor on the
//! computer the host runs on, or a running workspace's on another computer over ssh, where the host's refusal for want
//! of wsp's line in the person's ssh config asks the one question and a yes runs the same open again.
//!
//! The main part opens the default editor, the one last picked or else the first the host found, and wears its mark;
//! the menu lists every editor with its mark and the default's chord as a keycap, and a pick opens in that editor and
//! makes it the default. The slot is held, invisible, while the host lists its editors. While an open waits on the host
//! the main part wears the spinner in place of the mark.

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use gpui_kit::component::menu::{DropdownMenu as _, PopupMenuItem};
use gpui_kit::component::{h_flex, v_flex};
use gpui_kit::{
    div, img, prelude::*, px, Anchor, App, Context, Corners, Global, Hsla, Image, ImageFormat, SharedString, Subscription, Window,
};
use serde_json::{json, Value};
use wsp_host_client::{protocol, Failure};

use super::at::{self, request, Facts};
use super::words::open as words;
use crate::screens::app_shell::keys;
use crate::screens::dialogs::layer::{self, Closes};
use crate::screens::{notices, Host};
use crate::ui::menu::{surface as menu_surface, Trigger};
use crate::ui::{notify_in, size, spinner, ActivePalette as _, Button, Dialog, Glyph, KeyCaps, Notice, Run};

/// Memory at or under this, a workspace on another computer hears what an editor there costs: the editors' own
/// servers took 1.1 to 1.8 GB on the wsp repo, measured 2026-09-29.
pub const SMALL_MB: u64 = 4096;

/// The kind a refusal carries when the person's ssh config lacks wsp's line.
pub const SSH_INCLUDE_KIND: &str = "sshInclude";
/// `SSH_BEHIND_KIND`: the workspace's computer runs a daemon from before ssh.start, a wait and not a failure.
pub const SSH_BEHIND_KIND: &str = "sshBehind";

/// `opensInEditor`: its folder here, or its machine running.
pub fn opens_in_editor(facts: &Facts) -> bool {
    facts.here || facts.running
}

/// One editor the host found installed: its id, its name, and whether it has a road into another computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Editor {
    pub id: String,
    pub name: String,
    pub remote: bool,
}

/// `editor.list`; none where the host refused.
pub async fn editors(host: Host) -> Option<Vec<Editor>> {
    let list = request::<protocol::EditorListRequest>(json!({})).ok()?;
    let listed = host.run(move |client| async move { client.call(list).await }).await.ok()?;
    let editors = listed.editors.iter().filter_map(|e| {
        let v = serde_json::to_value(e).ok()?;
        Some(Editor {
            id: v.get("id")?.as_str()?.to_owned(),
            name: v.get("name")?.as_str()?.to_owned(),
            remote: v.get("remote").and_then(Value::as_bool).unwrap_or(false),
        })
    });
    Some(editors.collect())
}

/// The editors the host found installed and the person's pick, read once per host: the app holds one host.
#[derive(Default)]
pub struct Editors {
    /// None until the host lists them.
    pub list: Option<Vec<Editor>>,
    pub picked: Option<String>,
    asked: bool,
    /// The host refused the last list: Open draws nothing until a list lands, as `useEditors` takes a refusal as none.
    refused: bool,
}

impl Global for Editors {}

/// Takes what the host said of its editors and the pick; a pick read late does not undo one made since.
pub fn set_editors(list: Option<Vec<Editor>>, picked: Option<String>, cx: &mut App) {
    let editors = cx.default_global::<Editors>();
    if list.is_some() {
        editors.asked = true;
        editors.refused = false;
        editors.list = list;
    }
    editors.picked = picked.or(editors.picked.take());
}

/// The host refused to list its editors: Open stands down, and the next to want them asks again.
pub fn list_refused(cx: &mut App) {
    let editors = cx.default_global::<Editors>();
    editors.asked = false;
    editors.refused = true;
}

/// Lists the host's editors and reads the pick the first time Open wants them; a refused list stays unread, so the
/// next to want them asks again.
pub fn want_editors(host: &Host, cx: &mut App) {
    if cx.try_global::<Editors>().is_some_and(|e| e.asked) {
        return;
    }
    cx.default_global::<Editors>().asked = true;
    let host = host.clone();
    cx.spawn(async move |cx| {
        let picked = preference(host.clone()).await;
        let listed = editors(host).await;
        cx.update(|cx| {
            if listed.is_none() {
                list_refused(cx);
            }
            set_editors(listed, picked, cx);
        });
    })
    .detach();
}

/// `editor.open`: `path` at `line` in `editor`, or the host's default.
pub async fn open(host: Host, workspace_id: String, path: String, line: Option<usize>, editor: Option<String>) -> Result<(), Failure> {
    let mut params = json!({ "workspaceId": workspace_id, "path": path });
    if let Some(line) = line {
        params["line"] = json!(line);
    }
    if let Some(editor) = editor {
        params["editor"] = json!(editor);
    }
    let asked: protocol::EditorOpenRequest = request(params)?;
    host.run(move |client| async move { client.call(asked).await }).await.map(|_| ())
}

/// The computer the host runs on, by the name its row in the places list gives it; "" until known.
pub async fn here_name(host: Host) -> String {
    let Ok(list) = request::<protocol::PlacesListRequest>(json!({})) else { return String::new() };
    let Ok(listed) = host.run(move |client| async move { client.call(list).await }).await else { return String::new() };
    listed
        .places
        .iter()
        .filter_map(|p| serde_json::to_value(p).ok())
        .find(|p| p.get("id").and_then(Value::as_str) == Some("here"))
        .and_then(|p| p.get("label").or_else(|| p.get("name")).and_then(Value::as_str).map(str::to_owned))
        .unwrap_or_default()
}

/// `openFailed`: an Open that did not happen, said as a wait where it waits on the workspace's wsp to update, and as
/// the host's refusal otherwise.
pub fn failed_notice(failure: &Failure) -> Notice {
    if failure.kind.as_deref() == Some(SSH_BEHIND_KIND) {
        Notice::waiting(failure.message.clone())
    } else {
        Notice::error(failure.message.clone())
    }
}

/// [`failed_notice`] in the window's toasts.
pub fn say_failed(failure: &Failure, window: &mut Window, cx: &mut App) {
    notify_in(window, cx, failed_notice(failure));
}

/// The name the SSH question stands under on the dialogs' layer.
pub const SSH_DIALOG: &str = "editor-ssh";

/// Asks whether wsp may add its one line to ~/.ssh/config (`EditorConsent`); on Add the line goes in and `again` runs.
pub fn ask_ssh(host: Host, again: Run, window: &mut Window, cx: &mut App) {
    let view = cx.new(|_| SshAsk { host: host.clone(), again, computer: String::new() });
    let named = view.downgrade();
    cx.spawn(async move |cx| {
        let name = here_name(host).await;
        _ = named.update(cx, |ask, cx| {
            ask.computer = name;
            cx.notify();
        });
    })
    .detach();
    layer::open(SSH_DIALOG, view.into(), Closes::Outside, None, window, cx);
}

struct SshAsk {
    host: Host,
    again: Run,
    /// The computer the host runs on, by its name; "" until the places list answers.
    computer: String,
}

impl Render for SshAsk {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let (host, again) = (self.host.clone(), self.again.clone());
        let add = Button::primary("editor-ssh-add", words::SSH_ADD).on_click(move |_, window, cx| {
            layer::close(SSH_DIALOG, window, cx);
            let (host, again) = (host.clone(), again.clone());
            let handle = window.window_handle();
            cx.spawn(async move |cx| {
                let on = request::<protocol::SshIncludeRequest>(json!({ "on": true }));
                let added = match on {
                    Ok(on) => host.run(move |client| async move { client.call(on).await }).await.map(|_| ()),
                    Err(e) => Err(e),
                };
                _ = handle.update(cx, |_, window, cx| match added {
                    Ok(()) => again(window, cx),
                    Err(failure) => say_failed(&failure, window, cx),
                });
            })
            .detach();
        });
        let cancel = Button::outline("editor-ssh-cancel", words::CANCEL).on_click(|_, window, cx| layer::close(SSH_DIALOG, window, cx));
        let line = |text: String| div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(text);
        let body = v_flex()
            .gap_2()
            .child(div().text_size(px(14.)).line_height(px(20.)).text_color(p.foreground).child(words::SSH_SAYS))
            .child(line(words::ssh_reach(&self.computer)))
            .child(line(words::SSH_OFF.to_owned()));
        div()
            .debug_selector(|| "editor-ssh".into())
            .child(Dialog::new(words::SSH_TITLE).sheet().layered().on_outside(layer::dismiss).child(body).act(cancel).act(add))
    }
}

/// What an Open said in place of itself on a file tab: the line naming where the files are, or the host's refusal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Said {
    pub line: String,
    pub refused: bool,
}

/// The mark an editor's catalog row carries, from the catalog's own file.
fn mark_source(id: &str) -> Option<&'static str> {
    macro_rules! mark {
        ($file:literal) => {
            include_str!(concat!("../../../../../../../packages/catalog/src/marks/editors/", $file, ".ts"))
        };
    }
    Some(match id {
        "vscode" => mark!("vscode"),
        "vscode-insiders" => mark!("vscode-insiders"),
        "zed" => mark!("zed"),
        "idea" => mark!("idea"),
        "webstorm" => mark!("webstorm"),
        "pycharm" => mark!("pycharm"),
        "goland" => mark!("goland"),
        "rustrover" => mark!("jetbrains"),
        "clion" => mark!("clion"),
        "phpstorm" => mark!("phpstorm"),
        "rubymine" => mark!("rubymine"),
        "rider" => mark!("rider"),
        "finder" => mark!("finder"),
        "cursor" => include_str!("../../../../../../../packages/catalog/src/agents/cursor.ts"),
        _ => return None,
    })
}

/// The svg in a catalog row's `svg: \`...\``.
fn mark_svg(source: &str) -> Option<&str> {
    let start = source.find("svg: `")? + "svg: `".len();
    let end = source[start..].find('`')? + start;
    Some(&source[start..end])
}

/// The svg of the mark an editor wears, by the id the host opens it by; none where the catalog has none for it.
pub fn editor_svg(id: &str) -> Option<&'static str> {
    mark_source(id).and_then(mark_svg)
}

thread_local! {
    /// Each editor's mark drawn in one ink: at most the catalog's fourteen editors in the themes' few inks.
    static MARKS: RefCell<HashMap<(String, u32), Arc<Image>>> = RefCell::new(HashMap::new());
}

fn hex(ink: Hsla) -> (String, u32) {
    let rgba = ink.to_rgb();
    let (r, g, b) = ((rgba.r * 255.).round() as u32, (rgba.g * 255.).round() as u32, (rgba.b * 255.).round() as u32);
    (format!("#{r:02x}{g:02x}{b:02x}"), (r << 16) | (g << 8) | b)
}

/// An editor's mark at 16 px: its own colors, and the ink for a mark that names none, as `fill-current` draws it.
pub fn editor_mark(id: &str, ink: Hsla) -> gpui_kit::AnyElement {
    let blank = || div().size(px(16.)).flex_none().into_any_element();
    let Some(svg) = editor_svg(id) else { return blank() };
    let (fill, key) = hex(ink);
    let image = MARKS.with(|marks| {
        let mut marks = marks.borrow_mut();
        if marks.len() > 64 {
            marks.clear();
        }
        marks
            .entry((id.to_owned(), key))
            .or_insert_with(|| {
                let svg = svg.replacen("<svg ", &format!("<svg xmlns=\"http://www.w3.org/2000/svg\" fill=\"{fill}\" "), 1);
                Arc::new(Image::from_bytes(ImageFormat::Svg, svg.into_bytes()))
            })
            .clone()
    });
    let named = format!("editor-mark-{id}");
    img(image).debug_selector(move || named).size(px(16.)).flex_none().into_any_element()
}

/// The header's Open: one per header, told the thread's workspace on a switch with [`OpenSplit::show`] and its status
/// with [`OpenSplit::take_status`]; the editors come from [`Editors`], and every open goes the app's one road
/// (`notices::open_copy_in_editor`), as mod+o's does.
pub struct OpenSplit {
    workspace_id: Option<String>,
    facts: Option<Facts>,
    _editors: Subscription,
}

impl OpenSplit {
    pub fn new(cx: &mut Context<Self>) -> Self {
        OpenSplit { workspace_id: None, facts: None, _editors: cx.observe_global::<Editors>(|_, cx| cx.notify()) }
    }

    /// Open on another thread's workspace; its facts wait on that workspace's status.
    pub fn show(&mut self, workspace_id: String, cx: &mut Context<Self>) {
        if self.workspace_id.as_deref() != Some(workspace_id.as_str()) {
            self.workspace_id = Some(workspace_id);
            self.facts = None;
            cx.notify();
        }
    }

    /// The workspace's status as the header holds it; Open draws nothing without one.
    pub fn take_status(&mut self, status: Option<&Value>, cx: &mut Context<Self>) {
        let facts = status.map(at::facts_of);
        if facts != self.facts {
            self.facts = facts;
            cx.notify();
        }
    }

    /// Whether an open of the workspace's copy waits on the host, a pick's preference included.
    pub fn is_opening(&self, window: &Window, cx: &App) -> bool {
        self.workspace_id.as_deref().is_some_and(|w| notices::opening(w, window, cx))
    }

    /// The editors Open offers: every one here, only those with a road there for a workspace on another computer.
    pub fn usable(&self, cx: &App) -> Option<Vec<Editor>> {
        let (editors, facts) = (cx.try_global::<Editors>()?.list.as_ref()?, self.facts.as_ref()?);
        Some(editors.iter().filter(|e| facts.here || e.remote).cloned().collect())
    }

    fn current(&self, usable: &[Editor], cx: &App) -> Option<Editor> {
        let picked = cx.try_global::<Editors>().and_then(|e| e.picked.as_deref());
        usable.iter().find(|e| Some(e.id.as_str()) == picked).or(usable.first()).cloned()
    }

    /// The main part: the copy in the default editor.
    pub fn open_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(workspace), Some(facts)) = (self.workspace_id.clone(), self.facts.as_ref()) else { return };
        let editor = if facts.here { None } else { self.usable(cx).and_then(|u| self.current(&u, cx)).map(|e| e.id) };
        notices::open_copy_in_editor(workspace, None, editor, window, cx);
    }

    /// A pick in the menu: it becomes the default, then the copy opens in it, in one wait on the app's one road.
    fn pick(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(workspace) = self.workspace_id.clone() {
            notices::open_copy_in_editor(workspace, Some(id), None, window, cx);
        }
    }
}

async fn preference(host: Host) -> Option<String> {
    let get: protocol::PreferencesGetRequest = request(json!({})).ok()?;
    let got = host.run(move |client| async move { client.call(get).await }).await.ok()?;
    serde_json::to_value(&got.preferences).ok()?.get("editor")?.as_str().map(str::to_owned)
}

pub async fn set_preference(host: Host, editor: String) -> Result<(), Failure> {
    let patch: protocol::PreferencesSetRequest = request(json!({ "patch": { "editor": editor } }))?;
    host.run(move |client| async move { client.call(patch).await }).await.map(|_| ())
}

/// The chord `editor.open` stands on as the person's keybindings label it, as `⌘O`; none where nothing is bound.
fn open_chord(window: &Window, cx: &App) -> Option<String> {
    let mac = cfg!(target_os = "macos");
    match notices::shell(window, cx) {
        Some(shell) => keys::shortcut_label(shell.read(cx).rules(), "editor.open", mac),
        None => keys::shortcut_label(&keys::bound(mac, &Default::default()), "editor.open", mac),
    }
}

/// A chord's label as keycaps, one per key: `⌘O` on a Mac, `Ctrl+O` elsewhere.
pub fn chord_keys(label: &str) -> Vec<String> {
    if label.len() > 1 && label.contains('+') {
        return label.split('+').map(str::to_owned).collect();
    }
    let marks = label.chars().take_while(|c| "⌃⌥⇧⌘".contains(*c)).count();
    let mut keys: Vec<String> = label.chars().take(marks).map(String::from).collect();
    keys.push(label.chars().skip(marks).collect());
    keys.retain(|k| !k.is_empty());
    keys
}

impl Render for OpenSplit {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        // Out of the flow, so the header's gap-2 does not stand around an Open that draws nothing.
        let Some(facts) = self.facts.clone().filter(opens_in_editor) else { return div().absolute().into_any_element() };
        let usable = self.usable(cx);
        let current = match &usable {
            None if cx.try_global::<Editors>().is_some_and(|e| e.refused) => return div().absolute().into_any_element(),
            None => None,
            Some(usable) => match self.current(usable, cx) {
                Some(current) => Some(current),
                None => return div().absolute().into_any_element(),
            },
        };
        let left = Corners { top_left: size::BUTTON_RADIUS, bottom_left: size::BUTTON_RADIUS, top_right: px(0.), bottom_right: px(0.) };
        let right = Corners { top_left: px(0.), bottom_left: px(0.), top_right: size::BUTTON_RADIUS, bottom_right: size::BUTTON_RADIUS };
        let opening = self.is_opening(window, cx);
        let lead = match &current {
            None => div().size(px(16.)).flex_none().into_any_element(),
            Some(_) if opening => spinner(cx).size(px(16.)).into_any_element(),
            Some(current) => editor_mark(&current.id, p.foreground),
        };
        let lead = div().debug_selector(|| "open-mark".into()).flex_none().child(lead);
        let me = cx.weak_entity();
        // `max-sm:sr-only`: under 640 px the mark alone, its words on the hover.
        let label = if crate::ui::narrow(window) { "" } else { words::OPEN };
        let main = Button::outline("open-main", label).lead(lead).corners(left).when_some(current.as_ref(), |b, current| {
            b.tooltip(words::open_in(&current.name))
                .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.open_default(window, cx)))
        });
        let chevron = Button::outline("open-choose", "").xs().height(px(32.)).glyph(Glyph::ChevronDown).corners(right);
        let split = h_flex()
            .debug_selector(|| "open-split".into())
            .flex_none()
            .items_center()
            .child(div().debug_selector(|| "open-main".into()).child(main));
        // The slot stands, hidden and inert, while the host lists its editors, so the header does not move when they land.
        let Some(current) = current else {
            return split.invisible().child(div().ml(px(-1.)).child(chevron)).into_any_element();
        };
        let small = !facts.here && facts.mem_mb.is_some_and(|m| m <= SMALL_MB);
        let editors = usable.unwrap_or_default();
        let me = cx.weak_entity();
        let menu = Trigger(div().id("open-menu-trigger").flex().child(chevron.tooltip(words::CHOOSE))).dropdown_menu_with_anchor(
            Anchor::TopRight,
            move |menu, window, cx| {
                let keys = open_chord(window, cx).map(|label| chord_keys(&label));
                let mut menu = menu.surface(menu_surface(cx.palette()));
                if small {
                    let line = move |_: &mut Window, cx: &mut App| {
                        div()
                            .debug_selector(|| "open-small".into())
                            .max_w(px(256.))
                            .py(px(2.))
                            .text_size(px(12.))
                            .line_height(px(16.))
                            .text_color(cx.palette().muted_foreground)
                            .child(words::SMALL)
                    };
                    menu = menu.item(PopupMenuItem::element(line).disabled(true));
                }
                for editor in &editors {
                    let (id, name) = (editor.id.clone(), SharedString::from(editor.name.clone()));
                    let keys = keys.clone().filter(|_| editor.id == current.id);
                    let row = move |_: &mut Window, cx: &mut App| {
                        let ink = cx.palette().foreground;
                        let named = id.clone();
                        h_flex()
                            .debug_selector(move || format!("open-item-{named}"))
                            .w_full()
                            .gap(px(8.))
                            .child(editor_mark(&id, ink))
                            .child(name.clone())
                            .children(keys.clone().map(|keys| {
                                let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                                div().ml_auto().pl(px(12.)).child(KeyCaps::new(&[&keys]))
                            }))
                    };
                    let (me, id) = (me.clone(), editor.id.clone());
                    menu = menu.item(
                        PopupMenuItem::element(row)
                            .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.pick(id.clone(), window, cx))),
                    );
                }
                menu
            },
        );
        split.child(div().debug_selector(|| "open-choose".into()).ml(px(-1.)).child(menu)).into_any_element()
    }
}
