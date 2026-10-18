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
use std::rc::Rc;
use std::sync::Arc;

use gpui_kit::component::notification::Notification;
use gpui_kit::component::{h_flex, WindowExt as _};
use gpui_kit::{div, img, prelude::*, px, Anchor, App, Context, Corners, Hsla, Image, ImageFormat, SharedString, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::{protocol, Failure};

use super::at::{self, request, Facts};
use super::words::open as words;
use crate::screens::Host;
use crate::ui::{menu_on, size, spinner, ActivePalette as _, AlertDialog, Button, Glyph, MenuAct, Run};

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
pub async fn editors(host: Host) -> Vec<Editor> {
    let Ok(list) = request::<protocol::EditorListRequest>(json!({})) else { return Vec::new() };
    let Ok(listed) = host.run(move |client| async move { client.call(list).await }).await else { return Vec::new() };
    listed
        .editors
        .iter()
        .filter_map(|e| {
            let v = serde_json::to_value(e).ok()?;
            Some(Editor {
                id: v.get("id")?.as_str()?.to_owned(),
                name: v.get("name")?.as_str()?.to_owned(),
                remote: v.get("remote").and_then(Value::as_bool).unwrap_or(false),
            })
        })
        .collect()
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
pub fn say_failed(failure: &Failure, window: &mut Window, cx: &mut App) {
    let note = if failure.kind.as_deref() == Some(SSH_BEHIND_KIND) {
        Notification::info(failure.message.clone())
    } else {
        Notification::error(failure.message.clone())
    };
    window.push_notification(note, cx);
}

/// Runs an Open on the host and says its refusal, as the retry after the ssh question does.
pub fn open_and_say(open: impl std::future::Future<Output = Result<(), Failure>> + 'static, window: &mut Window, cx: &mut App) {
    let handle = window.window_handle();
    cx.spawn(async move |cx| {
        if let Err(failure) = open.await {
            _ = handle.update(cx, |_, window, cx| say_failed(&failure, window, cx));
        }
    })
    .detach();
}

/// Asks whether wsp may add its one line to ~/.ssh/config; on Add the line goes in and `again` runs.
pub fn ask_ssh(host: Host, again: Run, window: &mut Window, cx: &mut App) {
    let computer = Rc::new(RefCell::new(String::new()));
    let named = computer.clone();
    let handle = window.window_handle();
    let reading = host.clone();
    cx.spawn(async move |cx| {
        let name = here_name(reading).await;
        *named.borrow_mut() = name;
        _ = handle.update(cx, |_, window, _| window.refresh());
    })
    .detach();
    window.open_dialog(cx, move |dialog, _, cx| {
        let p = cx.palette().clone();
        let (host, again) = (host.clone(), again.clone());
        let add = Button::primary("editor-ssh-add", words::SSH_ADD).on_click(move |_, window, cx| {
            window.close_dialog(cx);
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
        let cancel = Button::outline("editor-ssh-cancel", words::CANCEL).on_click(|_, window, cx| window.close_dialog(cx));
        let line = |text: String| div().text_size(px(13.)).line_height(px(20.)).text_color(p.muted_foreground).child(text);
        dialog.p_0().w(px(440.)).rounded(size::DIALOG_RADIUS).border_0().bg(gpui_kit::transparent_black()).close_button(false).child(
            div().debug_selector(|| "editor-ssh".into()).child(
                AlertDialog::new(words::SSH_TITLE)
                    .child(div().text_size(px(14.)).line_height(px(20.)).text_color(p.foreground).child(words::SSH_SAYS))
                    .child(line(words::ssh_reach(&computer.borrow())))
                    .child(line(words::SSH_OFF.to_owned()))
                    .act(cancel)
                    .act(add),
            ),
        )
    });
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
    let Some(svg) = mark_source(id).and_then(mark_svg) else { return blank() };
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
    img(image).size(px(16.)).flex_none().into_any_element()
}

/// The header's Open: build it with [`OpenSplit::new`] for the thread's workspace and draw it in the header's actions.
pub struct OpenSplit {
    host: Host,
    workspace_id: String,
    facts: Option<Facts>,
    editors: Option<Vec<Editor>>,
    picked: Option<String>,
    mem_mb: Option<u64>,
    /// The default's chord as the keybindings label it, as `⌘O`.
    chord: Option<SharedString>,
    opening: bool,
    _read: Task<()>,
}

impl OpenSplit {
    pub fn new(host: Host, workspace_id: String, cx: &mut Context<Self>) -> Self {
        let reading = host.clone();
        let workspace = workspace_id.clone();
        let read = cx.spawn(async move |this, cx| {
            // A workspace the host will not name draws no Open, as `OpenSplit` draws nothing for a missing one.
            let facts = Some(at::read(reading.clone(), workspace, None).await.unwrap_or_default());
            let picked = preference(reading.clone()).await;
            let editors = editors(reading).await;
            _ = this.update(cx, |this, cx| {
                this.facts = facts;
                this.picked = picked;
                this.editors = Some(editors);
                cx.notify();
            });
        });
        let chord = if cfg!(target_os = "macos") { "⌘O" } else { "Ctrl+O" };
        OpenSplit {
            host,
            workspace_id,
            facts: None,
            editors: None,
            picked: None,
            mem_mb: None,
            chord: Some(chord.into()),
            opening: false,
            _read: read,
        }
    }

    /// The workspace machine's memory as its status reports it, for the small machine's note.
    pub fn set_mem_mb(&mut self, mem_mb: Option<u64>, cx: &mut Context<Self>) {
        self.mem_mb = mem_mb;
        cx.notify();
    }

    /// The default's chord as the person's keybindings label it; none hides the keycap.
    pub fn set_chord(&mut self, chord: Option<SharedString>, cx: &mut Context<Self>) {
        self.chord = chord;
        cx.notify();
    }

    pub fn is_opening(&self) -> bool {
        self.opening
    }

    /// The editors Open offers: every one here, only those with a road there for a workspace on another computer.
    pub fn usable(&self) -> Option<Vec<Editor>> {
        let (editors, facts) = (self.editors.as_ref()?, self.facts.as_ref()?);
        Some(editors.iter().filter(|e| facts.here || e.remote).cloned().collect())
    }

    fn current(&self) -> Option<Editor> {
        let usable = self.usable()?;
        usable.iter().find(|e| Some(&e.id) == self.picked.as_ref()).or(usable.first()).cloned()
    }

    /// `mod+o` and the main part: the copy in the default editor.
    pub fn open_default(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let here = self.facts.as_ref().is_some_and(|f| f.here);
        let editor = if here { None } else { self.current().map(|e| e.id) };
        self.open_in(None, editor, window, cx);
    }

    /// Opens the copy, `pick` becoming the default first; a workspace on another computer opens in `editor`.
    fn open_in(&mut self, pick: Option<String>, editor: Option<String>, window: &mut Window, cx: &mut Context<Self>) {
        let Some(facts) = self.facts.clone() else { return };
        if self.opening || !opens_in_editor(&facts) {
            return;
        }
        let Some(folder) = facts.project.clone() else { return };
        self.opening = true;
        if let Some(pick) = &pick {
            self.picked = Some(pick.clone());
        }
        cx.notify();
        let named = if facts.here { None } else { pick.clone().or(editor) };
        let (host, workspace) = (self.host.clone(), self.workspace_id.clone());
        let handle = window.window_handle();
        cx.spawn(async move |this, cx| {
            if let Some(pick) = pick {
                _ = set_preference(host.clone(), pick).await;
            }
            let opened = open(host.clone(), workspace.clone(), folder.clone(), None, named.clone()).await;
            _ = this.update(cx, |this, cx| {
                this.opening = false;
                cx.notify();
            });
            if let Err(failure) = opened {
                if failure.kind.as_deref() == Some(SSH_INCLUDE_KIND) {
                    let again: Run = Rc::new(move |window, cx| {
                        let (host, workspace, folder, named) = (host.clone(), workspace.clone(), folder.clone(), named.clone());
                        open_and_say(open(host, workspace, folder, None, named), window, cx);
                    });
                    let host = this.read_with(cx, |this, _| this.host.clone());
                    if let Ok(host) = host {
                        _ = handle.update(cx, |_, window, cx| ask_ssh(host, again, window, cx));
                    }
                } else {
                    _ = handle.update(cx, |_, window, cx| say_failed(&failure, window, cx));
                }
            }
        })
        .detach();
    }
}

async fn preference(host: Host) -> Option<String> {
    let get: protocol::PreferencesGetRequest = request(json!({})).ok()?;
    let got = host.run(move |client| async move { client.call(get).await }).await.ok()?;
    serde_json::to_value(&got.preferences).ok()?.get("editor")?.as_str().map(str::to_owned)
}

async fn set_preference(host: Host, editor: String) -> Result<(), Failure> {
    let set: protocol::PreferencesSetRequest = request(json!({ "patch": { "editor": editor } }))?;
    host.run(move |client| async move { client.call(set).await }).await.map(|_| ())
}

impl Render for OpenSplit {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let empty = || div().into_any_element();
        let Some(facts) = self.facts.clone() else { return held_slot() };
        if !opens_in_editor(&facts) {
            return empty();
        }
        let Some(usable) = self.usable() else { return held_slot() };
        let Some(current) = self.current() else { return empty() };
        let me = cx.weak_entity();
        let lead = if self.opening { spinner(cx).into_any_element() } else { editor_mark(&current.id, p.foreground) };
        let main = div()
            .id("open-main")
            .debug_selector(|| "open-main".into())
            .child(
                h_flex()
                    .h(px(32.))
                    .px(px(12.))
                    .gap(px(6.))
                    .items_center()
                    .rounded_l(size::BUTTON_RADIUS)
                    .border_1()
                    .border_color(p.input)
                    .bg(p.input_fill)
                    .hover(|s| s.bg(p.input_fill_hover))
                    .cursor_pointer()
                    .text_size(px(13.))
                    .font_weight(gpui_kit::FontWeight::MEDIUM)
                    .text_color(p.foreground)
                    .child(lead)
                    .child(words::OPEN),
            )
            .tooltip(crate::ui::tip(words::open_in(&current.name)))
            .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.open_default(window, cx)));
        let small = !facts.here && self.mem_mb.is_some_and(|m| m <= SMALL_MB);
        let mut acts: Vec<MenuAct> = Vec::new();
        if small {
            acts.push(MenuAct {
                group: 0,
                glyph: None,
                title: words::SMALL.into(),
                destructive: false,
                held: true,
                why: None,
                keys: None,
                run: Rc::new(|_, _| {}),
            });
        }
        for editor in usable {
            let me = cx.weak_entity();
            let id = editor.id.clone();
            let run: Run = Rc::new(move |window, cx| {
                let id = id.clone();
                _ = me.update(cx, |this, cx| this.open_in(Some(id.clone()), Some(id), window, cx));
            });
            let keys = (editor.id == current.id).then(|| self.chord.clone()).flatten();
            acts.push(MenuAct {
                group: 1,
                glyph: None,
                title: editor.name.clone().into(),
                destructive: false,
                held: false,
                why: None,
                keys,
                run,
            });
        }
        let chevron = Button::outline("open-choose", "")
            .glyph(Glyph::ChevronDown)
            .corners(Corners { top_left: px(0.), bottom_left: px(0.), top_right: size::BUTTON_RADIUS, bottom_right: size::BUTTON_RADIUS })
            .tooltip(words::CHOOSE);
        h_flex()
            .debug_selector(|| "open-split".into())
            .flex_none()
            .items_center()
            .child(main)
            .child(div().ml(px(-1.)).child(menu_on("open-menu", chevron, Anchor::TopRight, Rc::new(acts))))
            .into_any_element()
    }
}

/// The slot held while the host lists its editors, so the header does not move when they arrive.
fn held_slot() -> gpui_kit::AnyElement {
    div().debug_selector(|| "open-split-held".into()).invisible().w(px(108.)).h(px(32.)).into_any_element()
}
