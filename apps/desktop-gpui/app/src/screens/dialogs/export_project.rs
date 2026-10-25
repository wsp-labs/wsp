// SPDX-License-Identifier: AGPL-3.0-only
//! Export project (`sidebar/ExportProjectDialog.tsx`, `exportProject.ts`, `ProjectTripRows.tsx`): a folder on a
//! workspace's machine brought to the computer the host runs on. Quiet sections in one container: the folder on the
//! task, the folder here off the system's picker, the agents with threads on the workspace as ticked rows, and what
//! came home; one act starts the export and the runtime's `project.export` events read in the slot above the foot. An
//! existing folder comes back as the runtime's refusal naming it, and the act becomes Replace and export.

use std::collections::HashSet;

use gpui_kit::component::input::{InputEvent, InputState};
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _, Icon, Sizable as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Entity, PathPromptOptions, SharedString, Subscription, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlacesListRequest, ProjectExportRequest, SessionsListRequest, WorkspacesGetRequest};
use wsp_host_client::{Event, Failure};

use super::layer::{self, Closes};
use super::EXPORT_PROJECT;
use crate::screens::sidebar::looks::place_name;
use crate::screens::Host;
use crate::ui::controls::checkbox;
use crate::ui::{catalog, fmt_bytes, text, ActivePalette, AlertDialog, Button, Field, Glyph, Palette};

pub const TITLE: &str = "Export a project";
pub const SOURCE: &str = "Folder on the task";
pub const SESSIONS: &str = "Sessions";
pub const COMES_HOME: &str = "What comes home";
pub const FILES: &str = "Files";
pub const CACHES: &str = "Caches left behind";
pub const SOURCE_PLACEHOLDER: &str = "/root/project";
pub const DEST_PLACEHOLDER: &str = "/Users/you/code/project";
pub const CHOOSE_FOLDER: &str = "Choose folder";
pub const CHANGE: &str = "Change";
pub const CANCEL: &str = "Cancel";
pub const EXPORT: &str = "Export";
pub const REPLACE: &str = "Replace and export";
pub const DONE: &str = "Done";
/// `NOT_LANDED_WORD`, `EXPORT_SESSIONS_NOTE`, `NO_THREADS_NOTE`.
pub const NOT_LANDED: &str = "when it lands";
pub const SESSIONS_NOTE: &str = "Ticked agents' sessions come home with the folder. The rest stay in the task.";
pub const NO_THREADS_NOTE: &str = "No threads here. Every agent's sessions for the folder come home with it.";
/// The runtime's events kept per export; a trip says a few dozen.
const EVENTS: usize = 256;

pub fn from_line(workspace: &str, here: &str) -> String {
    if here.is_empty() {
        String::new()
    } else {
        format!("From {workspace}, to {here}.")
    }
}

pub fn dest_label(here: &str) -> String {
    if here.is_empty() {
        String::new()
    } else {
        format!("Folder on {here}")
    }
}

/// `plural`.
pub fn count(n: u64, noun: &str) -> String {
    format!("{n} {noun}{}", if n == 1 { "" } else { "s" })
}

/// `folderName`.
pub fn folder_name(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path)
}

/// `pickedDest`: a picker names a folder that exists, so the project lands inside it under its own name.
pub fn picked_dest(picked: &str, source: &str) -> String {
    format!("{}/{}", picked.trim_end_matches('/'), folder_name(source))
}

/// `exportProgress`: the events folded into the one line and how far the bar is; none before any event or after a failure.
pub fn progress(events: &[Value], here: &str) -> Option<(String, f64)> {
    let (mut line, mut fraction, mut packings) = (String::new(), 0f64, 0);
    for e in events {
        let pass = |packings: u32| if packings > 1 { "sessions" } else { "the folder" };
        match e.get("stage").and_then(Value::as_str).unwrap_or_default() {
            "packing" => {
                packings += 1;
                line = format!("Packing {}", pass(packings));
            }
            "downloading" => {
                let total = e.get("total").and_then(Value::as_f64);
                line = format!("Downloading {}{}", pass(packings), total.map(|t| format!(", {}", fmt_bytes(t as u64))).unwrap_or_default());
                if let (Some(bytes), Some(total)) = (e.get("bytes").and_then(Value::as_f64), total.filter(|t| *t > 0.)) {
                    fraction = fraction.max(bytes / total);
                }
            }
            "landing" => {
                line = if here.is_empty() { "Landing".to_owned() } else { format!("Landing on {here}") };
                fraction = 1.;
            }
            "done" => {
                line = "Done".to_owned();
                fraction = 1.;
            }
            "failed" => return None,
            _ => {}
        }
    }
    (!events.is_empty()).then_some((line, fraction))
}

/// Keeps one runtime event for the fold. A download reports every chunk it reads, so a run of them keeps only its latest,
/// which holds the most bytes; the cap then never drops the landing that ends the line.
fn keep(events: &mut Vec<Value>, frame: Value) {
    let downloading = |e: &Value| e.get("stage").and_then(Value::as_str) == Some("downloading");
    if downloading(&frame) && events.last().is_some_and(downloading) {
        events.pop();
    }
    if events.len() < EVENTS {
        events.push(frame);
    }
}

/// `agentOutcome`.
pub fn agent_outcome(agent: &Value) -> String {
    let word = |k: &str| agent.get(k).and_then(Value::as_str);
    match word("outcome").unwrap_or_default() {
        "failed" => format!("failed: {}", word("error").unwrap_or("no reason given")),
        outcome => {
            let said = match outcome {
                "moved" => "moved",
                "transcript-only" => "landed, not listed yet",
                "carried" => "carried unchanged",
                _ => "nothing to bring",
            };
            let skipped = agent.get("skipped").and_then(Value::as_u64).filter(|n| *n > 0);
            format!("{said}{}", skipped.map(|n| format!(", {} skipped", count(n, "rollout"))).unwrap_or_default())
        }
    }
}

/// `exportLandedLine`.
pub fn landed_line(result: &Value, source: &str, here: &str) -> String {
    if here.is_empty() {
        return String::new();
    }
    let none = result.get("agents").and_then(Value::as_array).is_none_or(Vec::is_empty);
    let dest = result.get("dest").and_then(Value::as_str).unwrap_or_default();
    format!("{} is at {dest} on {here}{}.", folder_name(source), if none { "; no agent sessions for it on the task" } else { "" })
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Phase {
    Idle,
    Exporting,
    Done,
}

/// A refusal, and whether it is the runtime's `exists`, whose one follow-up is to replace.
#[derive(Clone, Debug, PartialEq)]
pub struct Refusal {
    pub message: String,
    pub exists: bool,
}

pub struct ExportProject {
    /// None on a dialog staged from fixtures, which asks nothing.
    host: Option<Host>,
    workspace_id: String,
    name: String,
    here: String,
    source: Entity<InputState>,
    /// The destination once picked; before that it mirrors the source.
    chosen: Option<String>,
    agents: Vec<String>,
    unticked: HashSet<String>,
    events: Vec<Value>,
    phase: Phase,
    result: Option<Value>,
    refusal: Option<Refusal>,
    /// The folder as sent, which the landed line names.
    sent_source: String,
    _read: Vec<Task<()>>,
    _export: Option<Task<()>>,
    _subscriptions: Vec<Subscription>,
}

/// Stands Export project for the workspace on the layer. Escape and the scrim close it unless an export is running.
pub fn open(host: &Host, workspace_id: String, window: &mut Window, cx: &mut App) -> Entity<ExportProject> {
    let view = cx.new(|cx| ExportProject::new(host.clone(), workspace_id, window, cx));
    layer::open(EXPORT_PROJECT, view.clone().into(), Closes::Outside, None, window, cx);
    view
}

fn ask<T: serde::de::DeserializeOwned>(params: Value) -> Result<T, Failure> {
    serde_json::from_value(params).map_err(|e| Failure::new(e.to_string()))
}

impl ExportProject {
    pub fn new(host: Host, workspace_id: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = ExportProject::with(Some(host), workspace_id, window, cx);
        this.read(window, cx);
        this
    }

    /// The dialog over fixture answers: the workspace's name and folder, the computer here, the agents with threads.
    pub fn staged(name: &str, here: &str, source: &str, agents: &[&str], window: &mut Window, cx: &mut Context<Self>) -> Self {
        let mut this = ExportProject::with(None, String::new(), window, cx);
        (this.name, this.here) = (name.to_owned(), here.to_owned());
        this.agents = agents.iter().map(|a| a.to_string()).collect();
        this.source.update(cx, |s, cx| s.set_value(source.to_owned(), window, cx));
        this
    }

    /// What a run ended on, for a staged dialog: what came home, or the refusal.
    pub fn stage_end(&mut self, landed: Result<Value, Refusal>, cx: &mut Context<Self>) {
        self.sent_source = self.source(cx);
        match landed {
            Ok(result) => (self.result, self.phase) = (Some(result), Phase::Done),
            Err(refusal) => self.refusal = Some(refusal),
        }
        cx.notify();
    }

    fn with(host: Option<Host>, workspace_id: String, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let source = cx.new(|cx| InputState::new(window, cx).placeholder(SOURCE_PLACEHOLDER));
        let typed = cx.subscribe(&source, |this, _, e: &InputEvent, cx| {
            if matches!(e, InputEvent::Change) {
                this.refusal = None;
                cx.notify();
            }
        });
        ExportProject {
            host,
            workspace_id,
            name: String::new(),
            here: String::new(),
            source,
            chosen: None,
            agents: Vec::new(),
            unticked: HashSet::new(),
            events: Vec::new(),
            phase: Phase::Idle,
            result: None,
            refusal: None,
            sent_source: String::new(),
            _read: Vec::new(),
            _export: None,
            _subscriptions: vec![typed],
        }
    }

    /// The workspace's name and folder, the computer here, and the agents that ran threads on the workspace.
    fn read(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(host) = self.host.clone() else { return };
        let id = self.workspace_id.clone();
        let workspace = host.run(move |c| async move {
            let got = c.call(ask::<WorkspacesGetRequest>(json!({ "workspaceId": id }))?).await?;
            serde_json::to_value(got.workspace).map_err(|e| Failure::new(e.to_string()))
        });
        let places = host.run(|c| async move { c.call(ask::<PlacesListRequest>(json!({}))?).await.map(|r| r.places) });
        let id = self.workspace_id.clone();
        let sessions = host.run(move |c| async move {
            let listed = c.call(ask::<SessionsListRequest>(json!({ "workspaceId": id }))?).await?;
            Ok(listed.sessions.iter().filter_map(|s| serde_json::to_value(s).ok()).collect::<Vec<Value>>())
        });
        self._read.push(cx.spawn_in(window, async move |this, cx| {
            let Ok(workspace) = workspace.await else { return };
            _ = this.update_in(cx, |this, window, cx| {
                this.name = workspace.get("name").and_then(Value::as_str).unwrap_or_default().to_owned();
                let folder = workspace.get("folder").or_else(|| workspace.get("project")?.get("path")).and_then(Value::as_str);
                if let Some(folder) = folder.filter(|_| this.source.read(cx).value().is_empty()) {
                    this.source.update(cx, |s, cx| s.set_value(folder.to_owned(), window, cx));
                }
                cx.notify();
            });
        }));
        self._read.push(cx.spawn_in(window, async move |this, cx| {
            let places = places.await.unwrap_or_default();
            _ = this.update(cx, |this, cx| {
                let here = places.iter().filter_map(|p| serde_json::to_value(p).ok()).find(|p| p.get("id") == Some(&json!("here")));
                this.here = here.as_ref().map(place_name).unwrap_or_default();
                cx.notify();
            });
        }));
        let id = self.workspace_id.clone();
        self._read.push(cx.spawn_in(window, async move |this, cx| {
            let sessions = sessions.await.unwrap_or_default();
            _ = this.update(cx, |this, cx| {
                let mut agents: Vec<String> = Vec::new();
                for s in sessions.iter().filter(|s| s.get("workspaceId").and_then(Value::as_str).is_none_or(|w| w == id)) {
                    if let Some(harness) = s.get("harness").and_then(Value::as_str).filter(|h| !agents.iter().any(|a| a == h)) {
                        agents.push(harness.to_owned());
                    }
                }
                this.agents = agents;
                cx.notify();
            });
        }));
    }

    pub fn phase(&self) -> Phase {
        self.phase
    }

    pub fn refusal(&self) -> Option<&Refusal> {
        self.refusal.as_ref()
    }

    pub fn result(&self) -> Option<&Value> {
        self.result.as_ref()
    }

    pub fn source(&self, cx: &App) -> String {
        self.source.read(cx).value().trim().to_owned()
    }

    pub fn dest(&self, cx: &App) -> String {
        self.chosen.clone().unwrap_or_else(|| self.source(cx))
    }

    pub fn set_source(&mut self, source: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.source.update(cx, |s, cx| s.set_value(source.to_owned(), window, cx));
        cx.notify();
    }

    /// A folder picked here: the project lands inside it.
    pub fn pick(&mut self, picked: &str, cx: &mut Context<Self>) {
        self.chosen = Some(picked_dest(picked, &self.source(cx)));
        self.refusal = None;
        cx.notify();
    }

    fn pick_native(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let picked = cx.prompt_for_paths(PathPromptOptions { files: false, directories: true, multiple: false, prompt: None });
        cx.spawn_in(window, async move |this, cx| {
            let Ok(Ok(Some(paths))) = picked.await else { return };
            let Some(path) = paths.into_iter().next() else { return };
            _ = this.update(cx, |this, cx| this.pick(&path.to_string_lossy(), cx));
        })
        .detach();
    }

    /// `agentsRequest`: nothing while every row is ticked, else the ticked ids.
    fn agents_request(&self) -> Option<Vec<String>> {
        let ticked: Vec<String> = self.agents.iter().filter(|a| !self.unticked.contains(*a)).cloned().collect();
        (ticked.len() != self.agents.len()).then_some(ticked)
    }

    pub fn start(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let (source, dest) = (self.source(cx), self.dest(cx));
        let Some(host) = self.host.clone() else { return };
        if self.phase != Phase::Idle || source.is_empty() || dest.is_empty() {
            return;
        }
        let replace = self.refusal.as_ref().is_some_and(|r| r.exists);
        let mut params = json!({ "workspaceId": self.workspace_id, "source": source, "dest": dest });
        if replace {
            params["replace"] = json!(true);
        }
        if let Some(agents) = self.agents_request() {
            params["agents"] = json!(agents);
        }
        self.sent_source = source.clone();
        self.phase = Phase::Exporting;
        layer::hold(EXPORT_PROJECT, true, cx);
        self.events.clear();
        self.refusal = None;
        cx.notify();
        let (pushed, mut heard) = futures::channel::mpsc::unbounded::<Value>();
        let (workspace, sent) = (self.workspace_id.clone(), dest);
        let exported = host.run(move |c| async move {
            let mut events = c.events().await?;
            let listen = tokio::spawn(async move {
                while let Some(event) = events.next().await {
                    let frame = match event {
                        Ok(Event::Other(frame)) => frame,
                        Ok(Event::Host(e)) => serde_json::to_value(e).unwrap_or_default(),
                        _ => continue,
                    };
                    let ours = frame.get("type") == Some(&json!("project.export"))
                        && frame.get("workspaceId") == Some(&json!(workspace))
                        && frame.get("dest") == Some(&json!(sent));
                    if ours && pushed.unbounded_send(frame).is_err() {
                        break;
                    }
                }
            });
            let done = c.call(ask::<ProjectExportRequest>(params)?).await;
            listen.abort();
            let exported = done?;
            serde_json::to_value(exported.exported).map_err(|e| Failure::new(e.to_string()))
        });
        self._export = Some(cx.spawn_in(window, async move |this, cx| {
            let listening = {
                let this = this.clone();
                let mut cx = cx.clone();
                async move {
                    use futures::StreamExt as _;
                    while let Some(frame) = heard.next().await {
                        _ = this.update(&mut cx, |this, cx| {
                            keep(&mut this.events, frame);
                            cx.notify();
                        });
                    }
                }
            };
            let (_, landed) = futures::join!(listening, exported);
            _ = this.update(cx, |this, cx| {
                layer::hold(EXPORT_PROJECT, false, cx);
                match landed {
                    Ok(result) => {
                        this.result = Some(result);
                        this.phase = Phase::Done;
                    }
                    Err(f) => {
                        this.refusal = Some(Refusal { exists: f.kind.as_deref() == Some("exists"), message: f.message });
                        this.phase = Phase::Idle;
                    }
                }
                cx.notify();
            });
        }));
    }

    fn close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.phase != Phase::Exporting {
            layer::close(EXPORT_PROJECT, window, cx);
        }
    }

    /// What the slot says and in which ink: the refusal first, then the landed line, then the step under way.
    fn slot(&self, p: &Palette) -> (String, gpui_kit::Hsla, Option<f64>) {
        let progress = if self.phase == Phase::Idle { None } else { progress(&self.events, &self.here) };
        let fraction = progress.as_ref().map(|(_, f)| *f);
        if let Some(r) = &self.refusal {
            return (r.message.clone(), if r.exists { p.warning_foreground } else { p.error_foreground }, fraction);
        }
        if let Some(result) = &self.result {
            return (landed_line(result, &self.sent_source, &self.here), p.muted_foreground, fraction);
        }
        (progress.map(|(line, _)| line).unwrap_or_default(), p.muted_foreground, fraction)
    }

    fn section(label: impl Into<SharedString>, first: bool, body: impl IntoElement, p: &Palette) -> AnyElement {
        v_flex()
            .gap(px(6.))
            .py_3()
            .when(first, |el| el.pt_0())
            .child(div().text_size(text::XS.0).line_height(text::XS.1).text_color(p.muted_foreground).child(label.into()))
            .child(body)
            .into_any_element()
    }

    fn fact(label: &'static str, value: impl IntoElement, p: &Palette) -> impl IntoElement {
        h_flex()
            .h(px(28.))
            .min_w_0()
            .justify_between()
            .gap_3()
            .text_size(text::XS.0)
            .child(div().flex_none().text_color(p.muted_foreground).child(label))
            .child(div().min_w_0().truncate().child(value))
    }
}

impl Render for ExportProject {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let p = cx.palette().clone();
        let mono = cx.theme().mono_font_family.clone();
        let busy = self.phase == Phase::Exporting;
        let settled = self.phase != Phase::Idle;
        let dest = self.dest(cx);
        let me = cx.weak_entity();
        let picker = h_flex()
            .h(px(32.))
            .gap_2()
            .child(Icon::new(Glyph::Folder).with_size(px(16.)).text_color(p.muted_foreground))
            .child(
                div()
                    .debug_selector(|| "path".into())
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .font_family(mono.clone())
                    .text_size(text::XS.0)
                    .text_color(if dest.is_empty() { p.muted_foreground } else { p.foreground })
                    .child(if dest.is_empty() { DEST_PLACEHOLDER.to_owned() } else { dest.clone() }),
            )
            .child(Button::outline("export-dest", if dest.is_empty() { CHOOSE_FOLDER } else { CHANGE }).held(settled).on_click({
                let me = me.clone();
                move |_, window, cx| _ = me.update(cx, |this, cx| this.pick_native(window, cx))
            }));
        let held = self.phase != Phase::Idle;
        let ticks: Vec<_> = self
            .agents
            .iter()
            .map(|agent| checkbox(SharedString::from(format!("agent-{agent}-tick")), !self.unticked.contains(agent), false, window, cx))
            .collect();
        let rows = self.agents.iter().zip(ticks).map(|(agent, tick)| {
            let landed = self
                .result
                .as_ref()
                .and_then(|r| r.get("agents")?.as_array()?.iter().find(|a| a.get("agent").and_then(Value::as_str) == Some(agent)).cloned());
            let (me, id) = (me.clone(), agent.clone());
            h_flex()
                .h(px(28.))
                .gap_2()
                .text_size(text::XS.0)
                .child(
                    div()
                        .id(SharedString::from(format!("agent-{agent}")))
                        .when(!held, |el| el.cursor_pointer())
                        .on_click(move |_, _, cx| {
                            _ = me.update(cx, |this, cx| {
                                if this.phase != Phase::Idle {
                                    return;
                                }
                                if !this.unticked.remove(&id) {
                                    this.unticked.insert(id.clone());
                                }
                                cx.notify();
                            });
                        })
                        .child(tick),
                )
                .child(div().flex_none().truncate().text_color(p.foreground).child(catalog::name(agent)))
                .children(landed.as_ref().and_then(|l| l.get("sessions")?.as_u64()).map(|n| {
                    div().flex_none().font_family(mono.clone()).text_size(px(11.)).text_color(p.muted_foreground).child(count(n, "session"))
                }))
                .child(
                    div()
                        .ml_auto()
                        .min_w_0()
                        .truncate()
                        .text_size(px(11.))
                        .text_color(p.muted_foreground)
                        .child(landed.as_ref().map(agent_outcome).unwrap_or_default()),
                )
        });
        let agents = v_flex().children(rows).child(
            div().text_size(px(11.)).line_height(px(16.)).text_color(p.muted_foreground).child(if self.agents.is_empty() {
                NO_THREADS_NOTE
            } else {
                SESSIONS_NOTE
            }),
        );
        let files = match &self.result {
            None => div().text_color(p.muted_foreground).child(NOT_LANDED),
            Some(r) => {
                let n = r.get("files").and_then(Value::as_f64).unwrap_or(0.) as u64;
                let bytes = fmt_bytes(r.get("bytes").and_then(Value::as_u64).unwrap_or(0));
                div().font_family(mono.clone()).text_color(p.foreground).child(format!("{}  {bytes}", count(n, "file")))
            }
        };
        let caches = match self.result.as_ref().and_then(|r| r.get("excluded")?.as_array().cloned()) {
            None => div().text_color(p.muted_foreground).child(NOT_LANDED),
            Some(list) if list.is_empty() => div().font_family(mono.clone()).text_color(p.foreground).child("none"),
            Some(list) => div().font_family(mono.clone()).text_color(p.foreground).child(count(list.len() as u64, "folder")),
        };
        let summary = v_flex().child(Self::fact(FILES, files, &p)).child(Self::fact(CACHES, caches, &p));
        let (said, ink, fraction) = self.slot(&p);
        let status = v_flex()
            .gap(px(6.))
            .child(
                div()
                    .debug_selector(|| "progress-line".into())
                    .min_h(px(36.))
                    .text_size(px(13.))
                    .line_height(px(18.))
                    .text_color(ink)
                    .child(said),
            )
            .child(div().h(px(4.)).w_full().children(fraction.map(|f| {
                div()
                    .h_full()
                    .w_full()
                    .rounded_full()
                    .bg(p.border)
                    .child(div().h_full().w(gpui_kit::relative(f as f32)).rounded_full().bg(p.foreground.opacity(0.7)))
            })));
        let panel = v_flex()
            .pt(px(4.))
            .pb(px(12.))
            .child(Self::section(SOURCE, true, Field::new(&self.source).sheet().held(settled), &p))
            .child(Self::section(dest_label(&self.here), false, picker, &p))
            .child(Self::section(SESSIONS, false, agents, &p))
            .child(Self::section(COMES_HOME, false, summary, &p))
            .child(status);
        let ready = !self.source(cx).is_empty() && !dest.is_empty();
        let primary = match (self.phase, self.refusal.as_ref().is_some_and(|r| r.exists)) {
            (Phase::Done, _) => DONE,
            (_, true) => REPLACE,
            _ => EXPORT,
        };
        let go = Button::primary("export", primary).held(self.phase != Phase::Done && (!ready || busy)).on_click({
            let me = me.clone();
            move |_, window, cx| {
                _ = me.update(cx, |this, cx| match this.phase {
                    Phase::Done => this.close(window, cx),
                    _ => this.start(window, cx),
                })
            }
        });
        let cancel = (self.phase != Phase::Done).then(|| {
            Button::outline("export-cancel", CANCEL)
                .held(busy)
                .on_click(move |_, window, cx| _ = me.update(cx, |this, cx| this.close(window, cx)))
        });
        let mut dialog =
            AlertDialog::new(TITLE).layered().sentence(from_line(&self.name, &self.here)).child(panel).on_outside(layer::dismiss);
        if let Some(cancel) = cancel {
            dialog = dialog.act(cancel);
        }
        div().debug_selector(|| "export-project".into()).child(dialog.act(go))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_events_fold_into_the_line_and_the_bar() {
        let e = |stage: &str, bytes: Option<f64>, total: Option<f64>| json!({ "stage": stage, "bytes": bytes, "total": total });
        assert_eq!(progress(&[], "Mac"), None);
        let events = [e("packing", None, None), e("downloading", Some(512.), Some(2048.))];
        assert_eq!(progress(&events, "Mac"), Some(("Downloading the folder, 2 KB".into(), 0.25)));
        let events = [e("packing", None, None), e("packing", None, None), e("landing", None, None)];
        assert_eq!(progress(&events, "Mac"), Some(("Landing on Mac".into(), 1.)));
        assert_eq!(progress(&[e("failed", None, None)], "Mac"), None);
    }

    #[test]
    fn a_download_of_many_chunks_still_lands_on_the_line() {
        let e = |stage: &str, bytes: Option<f64>| json!({ "stage": stage, "bytes": bytes, "total": 4096. });
        let mut heard = vec![e("packing", None)];
        heard.extend((0..=4096).step_by(4).map(|b| e("downloading", Some(b as f64))));
        heard.push(e("landing", None));
        let mut kept = Vec::new();
        for frame in heard {
            keep(&mut kept, frame);
        }
        assert_eq!(kept.len(), 3);
        assert_eq!(progress(&kept, "Mac"), Some(("Landing on Mac".into(), 1.)));
        let mut midway = vec![e("packing", None)];
        for b in [0., 1024., 2048.] {
            keep(&mut midway, e("downloading", Some(b)));
        }
        assert_eq!(progress(&midway, "Mac"), Some(("Downloading the folder, 4 KB".into(), 0.5)));
    }

    #[test]
    fn a_picked_folder_holds_the_project_under_its_own_name() {
        assert_eq!(picked_dest("/Users/me/code/", "/root/project"), "/Users/me/code/project");
        assert_eq!(fmt_bytes(1536 * 1024 * 1024), "1.5 GB");
        assert_eq!(fmt_bytes(2 * 1024 * 1024 * 1024), "2 GB");
        assert_eq!(agent_outcome(&json!({ "outcome": "moved", "skipped": 2 })), "moved, 2 rollouts skipped");
        assert_eq!(
            landed_line(&json!({ "dest": "/x/p", "agents": [] }), "/root/p", "Mac"),
            "p is at /x/p on Mac; no agent sessions for it on the task."
        );
    }
}
