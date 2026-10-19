// SPDX-License-Identifier: AGPL-3.0-only
//! The Computer pane of the right panel, after `components/machine/MachineSurface.tsx`: load, memory and disk over the
//! last two minutes, the workspace's facts as chips over its path, and the acts that apply to it in a footer. With no
//! thread the panel is this computer's own, whose facts are this computer's and which offers nothing to act on. What
//! it says of a workspace is read off the sidebar, whose table and runner its footer uses.

pub mod daemon_down;
pub mod folds;
pub mod live;
pub mod model;

use std::time::Duration;

use futures::StreamExt as _;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Entity, Hsla, Role, ScrollHandle, SharedString, Subscription, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlaceView, Preferences, SessionsListRequest};
use wsp_host_client::{Event, Failure};

use crate::screens::palette::overlays;
use crate::screens::sidebar::menu::{self, Act};
use crate::screens::{Host, Opened, Pane, Place, Screen};
use crate::sidebar::Sidebar;
use crate::ui::computer_glyph::ComputerIcon;
use crate::ui::{chips, notify_in, scroll_area, text, tip, ActivePalette, Button, Chip, Glyph, Notice, Palette, Sparkline};
use live::{Live, Stop, Target};
use model::{metrics_road, slot_word, stale_word, Plot, Road, Stale, Tone};

/// The pane's sides and its rows' gaps: `px-5 py-5`, `gap-5` between the plots and the facts, `gap-8` between plots.
const INSET: f32 = 20.;
const PLOT_GAP: f32 = 32.;
/// The path's and the plot's ends' `text-[11px]`, at the page's 1.5.
const SMALL: (f32, f32) = (11., 16.5);
/// How often a pane checks it was drawn: one the window did not draw across two checks, a shut panel or a tab behind
/// another, lets its readings go until it is drawn again (`live.ts`: a figure nobody draws is not sampled or sent).
pub const PROBE: Duration = Duration::from_secs(1);

/// What the pane holds of the readings while it is drawn: a share of the host's ask, or a daemon channel of its own.
enum Watching {
    Host(String),
    Daemon { stop: Stop, task: Task<()> },
}

/// What a sub-machine's thread row says and what it has cost so far, which the sidebar does not keep: the newest row
/// with a `sub`, and `workspace.cost`'s figure, followed only while the pane shows a sub-machine.
#[derive(Default)]
struct SubFacts {
    row: Option<Value>,
    accrued: Option<f64>,
}

pub struct Computer {
    host: Host,
    target: Target,
    live: Entity<Live>,
    sidebar: Option<Entity<Sidebar>>,
    watching: Option<Watching>,
    sub: Option<(SubFacts, Task<()>)>,
    scroll: ScrollHandle,
    /// Ticks each minute while a sub-machine's delete timer is drawn.
    clock: Option<Task<()>>,
    /// Whether the window draws the pane, which holds the readings; drawn since the last [`PROBE`], and asked to be.
    on_screen: bool,
    drawn: bool,
    probing: bool,
    /// The check, which runs only while the pane is on screen: a shut panel's pane wakes nothing.
    probe: Option<Task<()>>,
    _observed: Vec<Subscription>,
    _release: Subscription,
}

/// What one plot shows.
#[derive(Clone, Debug, PartialEq)]
pub struct PlotShown {
    pub plot: Plot,
    /// The figure, or the word in its place.
    pub figure: String,
    /// The figure's tone; none for a word, which reads muted.
    pub tone: Option<Tone>,
    /// The daemon's refusal, on the hover of an unavailable figure.
    pub refusal: Option<String>,
    pub points: Vec<(f32, f32)>,
    /// Drawn dimmed with no dot on the newest point.
    pub dimmed: bool,
}

/// Everything the pane draws, read off the sidebar and the readings as they stand.
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    pub plots: Vec<PlotShown>,
    pub chips: Vec<(String, ComputerIcon)>,
    pub path: Option<String>,
    pub footer: Vec<folds::Button>,
}

impl Computer {
    pub fn target(&self) -> &Target {
        &self.target
    }

    /// The workspace the pane shows and its live status; none for this computer's own pane or one not read yet.
    fn workspace<'a>(&self, s: &'a Sidebar) -> Option<(&'a Value, Option<&'a Value>)> {
        let Target::Workspace(id) = &self.target else { return None };
        Some((s.around().workspaces.get(id)?, s.around().statuses.get(id)))
    }

    /// Finds the sidebar through the window's layers, which the shell hands it once connected.
    fn find_sidebar(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.sidebar.is_some() {
            return;
        }
        let Some(layers) = overlays::of(window, cx) else { return };
        match layers.read(cx).sidebar.as_ref().and_then(|s| s.upgrade()) {
            Some(sidebar) => {
                self._observed.push(cx.observe_in(&sidebar, window, |this, _, window, cx| {
                    this.sync(window, cx);
                    cx.notify();
                }));
                self.sidebar = Some(sidebar);
                self.sync(window, cx);
            }
            None => self._observed.push(cx.observe_in(&layers, window, |this, _, window, cx| this.find_sidebar(window, cx))),
        }
    }

    /// The road the readings take now: the host's for this computer's own workspace, else a daemon's, none while a
    /// daemon-read workspace is not running or not read yet.
    fn road(&self, cx: &App) -> Option<Road> {
        match &self.target {
            Target::Here => Some(Road::Daemon),
            Target::Workspace(_) => {
                let (ws, status) = self.workspace(self.sidebar.as_ref()?.read(cx))?;
                match metrics_road(ws["kind"].as_str()) {
                    Road::Host => Some(Road::Host),
                    Road::Daemon => (phase(ws, status) == "running").then_some(Road::Daemon),
                }
            }
        }
    }

    /// Starts, moves or stops the watch to match the road, and the sub-machine's facts to match the sidebar.
    fn sync(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let road = if self.on_screen { self.road(cx) } else { None };
        let now = match self.watching {
            Some(Watching::Host(_)) => Some(Road::Host),
            Some(Watching::Daemon { .. }) => Some(Road::Daemon),
            None => None,
        };
        if now != road {
            self.stop_watch(cx);
            self.watching = match road {
                Some(Road::Host) => {
                    let id = self.target.key();
                    self.live.update(cx, |live, cx| live.hold_host(&self.host, &id, cx));
                    Some(Watching::Host(id))
                }
                Some(Road::Daemon) => {
                    let (run, mut heard, stop) = live::watch_daemon(&self.host, &self.target);
                    let (key, live) = (self.target.key(), self.live.clone());
                    let task = cx.spawn_in(window, async move |_, cx| {
                        let _run = run;
                        while let Some(heard) = heard.next().await {
                            live.update(cx, |live, cx| live.hear(&key, heard, cx));
                        }
                    });
                    Some(Watching::Daemon { stop, task })
                }
                None => None,
            };
        }
        let has_sub = self.sidebar.as_ref().is_some_and(|s| {
            let Target::Workspace(id) = &self.target else { return false };
            s.read(cx).threads().iter().any(|t| &t.workspace_id == id && t.sub.is_some())
        });
        if has_sub != self.sub.is_some() {
            self.sub = has_sub.then(|| (SubFacts::default(), self.follow_sub(cx)));
        }
    }

    fn stop_watch(&mut self, cx: &mut App) {
        match self.watching.take() {
            Some(Watching::Host(id)) => self.live.update(cx, |live, cx| live.release_host(&id, cx)),
            Some(Watching::Daemon { stop, task }) => {
                // The watch closes its own channel, one still opening included, and ends.
                _ = stop.send(futures::channel::oneshot::channel().0);
                task.detach();
                self.live.update(cx, |live, _| live.keep(&self.target.key(), |r| r.live = false));
            }
            None => {}
        }
    }

    /// Reads the sub-machine's newest row once, then takes each row and cost the host pushes for this workspace.
    fn follow_sub(&self, cx: &mut Context<Self>) -> Task<()> {
        let Target::Workspace(id) = self.target.clone() else { return Task::ready(()) };
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Value>();
        let run = self.host.run(move |client| async move {
            let mut events = client.events().await?;
            let ask: SessionsListRequest = serde_json::from_value(json!({ "workspaceId": id })).map_err(|e| Failure::new(e.to_string()))?;
            for row in client.call(ask).await?.sessions {
                _ = tx.unbounded_send(serde_json::to_value(row).unwrap_or_default());
            }
            while let Some(event) = events.next().await {
                let frame = match event {
                    Ok(Event::Host(e)) => serde_json::to_value(&e).unwrap_or_default(),
                    Ok(Event::Other(v)) => v,
                    _ => continue,
                };
                let row = match frame["type"].as_str() {
                    Some("session.row") => frame["row"].clone(),
                    Some("workspace.cost") if frame["workspaceId"] == id.as_str() => json!({ "accruedUsd": frame["accruedUsd"] }),
                    _ => continue,
                };
                if tx.unbounded_send(row).is_err() {
                    break;
                }
            }
            Ok(())
        });
        cx.spawn(async move |this, cx| {
            let _run = run;
            while let Some(row) = rx.next().await {
                let taken = this.update(cx, |this, cx| {
                    let (Some((facts, _)), Target::Workspace(id)) = (this.sub.as_mut(), &this.target) else { return };
                    if let Some(usd) = row["accruedUsd"].as_f64() {
                        facts.accrued = Some(usd);
                    } else if row["workspaceId"] == id.as_str() && !row["sub"].is_null() {
                        let newer = facts.row.as_ref().is_none_or(|held| held["startedAt"].as_f64() <= row["startedAt"].as_f64());
                        if newer {
                            facts.row = Some(row);
                        }
                    }
                    cx.notify();
                });
                if taken.is_err() {
                    return;
                }
            }
        })
    }

    /// Says the host's refusal of this computer's readings, once on each socket.
    fn say_refusal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let key = self.target.key();
        let Some(words) = self.live.update(cx, |live, _| live.unsaid(&key)) else { return };
        let name =
            self.sidebar.as_ref().and_then(|s| self.workspace(s.read(cx))).and_then(|(ws, _)| ws["name"].as_str().map(str::to_owned));
        let name = name.unwrap_or(key);
        notify_in(window, cx, Notice::error(live::refused_line(&name, &words)));
    }

    /// A pane drawn since the last check stays on screen; one that was not is asked to draw once, and lets its
    /// readings go if it still was not by the next.
    fn probe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.on_screen || std::mem::take(&mut self.drawn) {
            self.probing = false;
            return;
        }
        if !std::mem::replace(&mut self.probing, true) {
            return cx.notify();
        }
        self.probing = false;
        self.on_screen = false;
        self.probe = None;
        self.sync(window, cx);
    }

    fn start_probe(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.probe = Some(cx.spawn_in(window, async move |this, cx| loop {
            cx.background_executor().timer(PROBE).await;
            if this.update_in(cx, |this, window, cx| this.probe(window, cx)).is_err() {
                return;
            }
        }));
    }

    fn keep_clock(&mut self, ticking: bool, cx: &mut Context<Self>) {
        if !ticking {
            self.clock = None;
        } else if self.clock.is_none() {
            self.clock = Some(cx.spawn(async move |this, cx| loop {
                cx.background_executor().timer(Duration::from_secs(60)).await;
                if this.update(cx, |_, cx| cx.notify()).is_err() {
                    return;
                }
            }));
        }
    }

    /// Runs a footer act through the sidebar's runner, as its menu and the palette run it.
    fn run(&mut self, act: Act, window: &mut Window, cx: &mut Context<Self>) {
        let (Some(sidebar), Target::Workspace(id)) = (self.sidebar.clone(), self.target.clone()) else { return };
        sidebar.update(cx, |s, cx| s.run_workspace(&id, None, act, window, cx));
    }

    pub fn shown(&self, cx: &App) -> Shown {
        let sidebar = self.sidebar.as_ref().map(|s| s.read(cx));
        let ws = sidebar.and_then(|s| self.workspace(s));
        let readings = self.live.read(cx).readings(&self.target.key());
        // A local workspace's figures are read by the host, so its daemon being down says nothing about them.
        let daemon_read = ws.is_none_or(|(ws, _)| metrics_road(ws["kind"].as_str()) == Road::Daemon);
        let stale: Option<Stale> = sidebar.zip(ws).and_then(|(s, (ws, status))| {
            let Target::Workspace(id) = &self.target else { return None };
            let away = s.absence(id).filter(|_| daemon_read).map(|a| if a.start.is_some() { Stale::Stopped } else { Stale::NoAnswer });
            away.or_else(|| stale_word(phase(ws, status), readings.live))
        });
        let word = slot_word(stale, readings.unavailable.is_some(), &readings.ring);
        let last = readings.ring.last().copied();
        let plots = Plot::ALL
            .into_iter()
            .map(|plot| PlotShown {
                plot,
                figure: word.map(str::to_owned).or_else(|| last.map(|s| plot.figure(&s))).unwrap_or_default(),
                tone: last.filter(|_| word.is_none()).map(|s| plot.tone(&s)),
                refusal: readings.unavailable.clone().filter(|_| word == Some("unavailable")),
                points: plot.points(&readings.ring),
                dimmed: stale.is_some(),
            })
            .collect();
        let Some(s) = sidebar else { return Shown { plots, chips: Vec::new(), path: None, footer: Vec::new() } };
        let id = self.target.key();
        let sub = self.sub.as_ref().and_then(|(facts, _)| Some((facts.row.as_ref()?, facts.accrued)));
        let chips = match ws {
            Some((ws, status)) => {
                let (cost, goes) = match sub {
                    Some((row, accrued)) => {
                        let now = chrono::Utc::now().timestamp_millis() as f64;
                        folds::sub_facts(&row["sub"], row["status"] == "running", accrued, now, row["diskKeptUntil"].as_f64())
                    }
                    None => (None, None),
                };
                let mut chips = vec![(s.state_word(&id), Glyph::Activity.into()), (folds::where_runs(s, &id, ws), Glyph::Laptop.into())];
                if let Some(size) = status.and_then(|st| st.get("size")).filter(|z| !z.is_null()) {
                    chips.push((model::fmt_cpus(size["cpu"].as_f64().unwrap_or(0.), folds::cpu_word(ws)), Glyph::Cpu.into()));
                    chips.push((model::fmt_mem_gb(size["memMb"].as_f64().unwrap_or(0.)), Glyph::MemoryStick.into()));
                }
                chips.extend(cost.map(|c| (c, Glyph::Receipt.into())));
                chips.extend(goes.map(|g| (g, Glyph::Hourglass.into())));
                chips
            }
            None => here_chips(s),
        };
        let has_sub = self.sub.is_some();
        Shown {
            plots,
            chips,
            path: ws.and_then(|(ws, _)| ws["project"]["path"].as_str()).map(str::to_owned),
            footer: ws.map(|_| folds::footer(menu::workspace_items(s, &id), has_sub, &s.here_name())).unwrap_or_default(),
        }
    }
}

/// The phase a workspace's live status says, else its record's.
fn phase<'a>(ws: &'a Value, status: Option<&'a Value>) -> &'a str {
    status.and_then(|s| s["phase"].as_str()).or_else(|| ws["phase"].as_str()).unwrap_or_default()
}

/// This computer's own facts: its name and glyph, its system, its cores and its memory.
fn here_chips(s: &Sidebar) -> Vec<(String, ComputerIcon)> {
    let Some(place) = s.around().places.iter().find(|p| p["id"] == "here") else { return Vec::new() };
    let Ok(view) = serde_json::from_value::<PlaceView>(place.clone()) else { return Vec::new() };
    let preferences = s.around().preferences.as_deref().and_then(|p| serde_json::from_value::<Preferences>(p.clone()).ok());
    let mut chips = vec![(crate::screens::sidebar::looks::place_name(place), ComputerIcon::of(&view, preferences.as_ref()))];
    chips.extend(view.os.clone().map(|os| (os, Glyph::AppWindow.into())));
    if let Some(shape) = &view.shape {
        chips.push((model::fmt_cpus(shape.cpu, "cores"), Glyph::Cpu.into()));
        chips.push((model::fmt_mem_gb(shape.mem_mb), Glyph::MemoryStick.into()));
    }
    chips
}

impl Screen for Computer {
    const NAME: &'static str = "computer";
    const PLACE: Place = Place::Panel(Pane::Computer);

    fn new(host: Host, opened: Opened, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let target = opened.workspace_id.map_or(Target::Here, Target::Workspace);
        let live = live::live(cx);
        let observed = vec![cx.observe_in(&live, window, |this, _, window, cx| {
            this.say_refusal(window, cx);
            cx.notify();
        })];
        let release = cx.on_release(|this, cx| this.stop_watch(cx));
        let mut pane = Computer {
            host,
            target,
            live,
            sidebar: None,
            watching: None,
            sub: None,
            scroll: ScrollHandle::new(),
            clock: None,
            on_screen: false,
            drawn: false,
            probing: false,
            probe: None,
            _observed: observed,
            _release: release,
        };
        pane.find_sidebar(window, cx);
        pane
    }
}

/// `TONE_TEXT`: a figure's ink by its tone.
fn tone_ink(tone: Tone, p: &Palette) -> Hsla {
    match tone {
        Tone::Muted => p.muted_foreground,
        Tone::Warning => p.warning_foreground,
        Tone::Danger => p.error_foreground,
    }
}

fn plot_view(shown: PlotShown, p: &Palette, mono: SharedString) -> impl IntoElement {
    let key = shown.plot.key();
    let figure_ink = shown.tone.map_or(p.muted_foreground, |tone| tone_ink(tone, p));
    // `TONE_LINE`: the line takes the figure's tone where it has one, so memory near full draws in the caution ink.
    let line_ink = match shown.tone {
        _ if shown.dimmed => p.muted_foreground.opacity(0.4),
        Some(tone) if tone != Tone::Muted => tone_ink(tone, p),
        _ => p.foreground.opacity(0.8),
    };
    let sparkline = Sparkline::new(shown.points, line_ink);
    let sparkline = if shown.dimmed { sparkline } else { sparkline.end(p.background) };
    v_flex()
        .debug_selector(move || format!("plot-{key}"))
        .py_1()
        .child(
            h_flex()
                .items_baseline()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex_none()
                        .text_size(text::XS.0)
                        .line_height(text::XS.1)
                        .text_color(p.muted_foreground)
                        .child(shown.plot.label()),
                )
                .child(
                    div()
                        .id(SharedString::from(format!("figure-{key}")))
                        .debug_selector(move || format!("figure-{key}"))
                        .min_w_0()
                        .truncate()
                        .font_family(mono.clone())
                        .text_size(text::FACT.0)
                        .line_height(text::FACT.1)
                        .text_color(figure_ink)
                        .when_some(shown.refusal, |el, refusal| el.tooltip(tip(refusal)))
                        .child(shown.figure),
                ),
        )
        .child(
            div()
                .id(SharedString::from(format!("chart-{key}")))
                .debug_selector(move || format!("chart-{key}"))
                .mt_3()
                .role(Role::Image)
                .aria_label(shown.plot.described())
                .child(sparkline),
        )
        .child(
            h_flex()
                .mt(px(6.))
                .justify_between()
                .font_family(mono)
                .text_size(px(SMALL.0))
                .line_height(px(SMALL.1))
                .text_color(p.muted_foreground)
                .child("2 min")
                .child("now"),
        )
}

impl Render for Computer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.drawn = true;
        if !self.on_screen {
            self.on_screen = true;
            self.start_probe(window, cx);
            self.sync(window, cx);
        }
        let p = cx.palette().clone();
        let mono = cx.theme().mono_font_family.clone();
        let Shown { plots, chips: facts, path, footer } = self.shown(cx);
        self.keep_clock(self.sub.is_some(), cx);
        let facts = (!facts.is_empty()).then(|| {
            v_flex()
                .debug_selector(|| "facts".into())
                .gap(px(10.))
                .child(chips("computer-facts", facts.into_iter().map(|(text, glyph)| Chip::new(text).glyph(glyph)).collect(), cx))
                .children(path.map(|path| {
                    div()
                        .id("computer-path")
                        .debug_selector(|| "path".into())
                        .truncate()
                        .font_family(mono.clone())
                        .text_size(px(SMALL.0))
                        .line_height(px(SMALL.1))
                        .text_color(p.muted_foreground)
                        .tooltip(tip(path.clone()))
                        .child(path)
                }))
        });
        let footer = (!footer.is_empty()).then(|| {
            h_flex()
                .debug_selector(|| "computer-footer".into())
                .flex_none()
                .flex_wrap()
                .gap(px(6.))
                .border_t_1()
                .border_color(p.border.opacity(0.6))
                .px_3()
                .py(px(10.))
                .children(footer.into_iter().map(|b| footer_button(b, cx)))
        });
        let body = v_flex()
            .gap(px(INSET))
            .p(px(INSET))
            .child(v_flex().gap(px(PLOT_GAP)).children(plots.into_iter().map(|plot| plot_view(plot, &p, mono.clone()))))
            .children(facts);
        v_flex()
            .debug_selector(|| "computer-pane".into())
            .size_full()
            .min_h_0()
            .child(scroll_area("computer-scroll", &self.scroll, body, window, cx).flex_1().min_h_0())
            .children(footer)
    }
}

/// One footer button, its refusal or its hint on the hover; a held one keeps its tooltip on a box around it.
fn footer_button(b: folds::Button, cx: &mut Context<Computer>) -> AnyElement {
    let id = SharedString::from(format!("computer-{}", b.id()));
    let selector = format!("footer-{}", b.id());
    let tooltip = b.tooltip().map(str::to_owned);
    let held = b.refusal.is_some();
    let act = b.act.clone();
    let button = if b.destructive { Button::danger(id.clone(), b.word.clone()) } else { Button::outline(id.clone(), b.word.clone()) }
        .xs()
        .held(held)
        .on_click(cx.listener(move |this, _, window, cx| this.run(act.clone(), window, cx)));
    if held {
        div()
            .id(SharedString::from(format!("{id}-held")))
            .debug_selector(move || selector.clone())
            .when_some(tooltip, |el, t| el.tooltip(tip(t)))
            .child(button)
            .into_any_element()
    } else {
        div()
            .debug_selector(move || selector.clone())
            .child(match tooltip {
                Some(t) => button.tooltip(t),
                None => button,
            })
            .into_any_element()
    }
}
