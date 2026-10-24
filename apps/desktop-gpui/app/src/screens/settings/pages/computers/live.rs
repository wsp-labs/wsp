// SPDX-License-Identifier: AGPL-3.0-only
//! The band under a computer's row on the list, after `LiveBand.tsx` and `computerLive.ts`: a CPU lane over a memory
//! lane, the last fifteen minutes in ten second steps, each lane's figure at its right. Each row follows its computer's
//! readings on the host's socket while it is drawn (sys.subscribe, the place.sys frames, sys.unsubscribe once the row
//! goes), asks again on a socket that comes back, and fills its window from places.readings as it starts and as the
//! computer comes back. A window outlives the list, so a computer that stops answering still shows what it last read.
//! The page is drawn again once a step, never once a sample.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt as _;
use gpui_kit::component::{h_flex, v_flex, ActiveTheme as _};
use gpui_kit::{div, prelude::*, px, AnyElement, App, Context, Entity, Global, Hsla, Pixels, SharedString, StyleRefinement, Task, Window};
use serde_json::{json, Value};
use wsp_host_client::protocol::{PlaceView, PlacesReadingsRequest, SysSubscribeRequest, SysUnsubscribeRequest};
use wsp_host_client::{Event, HostClient};

use super::words::{fmt_mem_gb, is_offline};
use crate::screens::settings::model::req;
use crate::screens::settings::page::Ctx;
use crate::screens::settings::Acts;
use crate::ui::{text, tip, ActivePalette, GridCell, Sparkline};

/// `STEP_MS`.
pub const STEP_MS: f64 = 10_000.;
/// `BAND_WINDOW_MS`, the window the daemon keeps and the band draws.
pub const WINDOW_MS: f64 = 15. * 60_000.;
/// `MEMORY_NEAR_FULL`: the share of memory past which the kernel's killer is one allocation away.
const MEMORY_NEAR_FULL: f64 = 0.9;
/// A lane's height, `h-4.5`, and the gap between the lanes, `gap-y-2.5`.
const LANE: Pixels = px(18.);
const LANE_GAP: Pixels = px(10.);
/// `w-12.5`: the word's slot, so both values start on one line.
const WORD_SLOT: Pixels = px(50.);
/// `pl-11`: the plot starts under the row's name.
const PLOT_INSET: Pixels = px(44.);

/// One reading as the band keeps it: when, the CPU in percent, and the memory used of its total in bytes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Reading {
    pub at: f64,
    pub cpu: f64,
    pub used: f64,
    pub total: f64,
}

/// One ten-second step: the mean CPU of the readings in it and its last memory, at its last reading, with how many
/// readings it holds and the CPU of its last, the figure the band says now.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Step {
    pub at: f64,
    pub cpu: f64,
    pub used: f64,
    pub total: f64,
    pub n: u32,
    pub cpu_now: f64,
}

fn step_of(at: f64) -> i64 {
    (at / STEP_MS).floor() as i64
}

/// Folds readings, oldest first, onto steps already held, then drops the steps older than the window.
fn fold(steps: &mut Vec<Step>, readings: impl IntoIterator<Item = Reading>) {
    for Reading { at, cpu, used, total } in readings {
        match steps.last_mut() {
            Some(last) if at <= last.at => continue,
            Some(last) if step_of(last.at) == step_of(at) => {
                let n = last.n as f64;
                *last = Step { at, cpu: (last.cpu * n + cpu) / (n + 1.), used, total, n: last.n + 1, cpu_now: cpu };
            }
            _ => steps.push(Step { at, cpu, used, total, n: 1, cpu_now: cpu }),
        }
    }
    if let Some(from) = steps.last().map(|s| s.at - WINDOW_MS) {
        steps.retain(|s| s.at >= from);
    }
}

/// `feedWindow`: the kept reads a history answered, with any step already held past its newest one after them.
pub fn feed_window(steps: &mut Vec<Step>, points: &[Reading]) {
    let Some(last) = points.last().map(|p| p.at) else { return };
    let mut next = Vec::new();
    fold(&mut next, points.iter().copied());
    next.extend(steps.iter().filter(|s| s.at > last).copied());
    fold(&mut next, []);
    *steps = next;
}

/// `feedSample`, and whether it began a step: the only time the band is drawn again.
pub fn feed_sample(steps: &mut Vec<Step>, reading: Reading) -> bool {
    let before = (steps.len(), steps.last().map(|s| step_of(s.at)));
    fold(steps, [reading]);
    (steps.len(), steps.last().map(|s| step_of(s.at))) != before
}

/// Every listed computer's steps, kept for the app's life as the web keeps them for the page's.
#[derive(Default)]
struct Readings(HashMap<String, Vec<Step>>);

impl Global for Readings {}

impl Readings {
    /// Whether any computer kept here is one the list no longer holds.
    fn holds_gone(&self, places: &[PlaceView]) -> bool {
        self.0.keys().any(|id| !places.iter().any(|p| p.id == *id))
    }

    fn keep_listed(&mut self, places: &[PlaceView]) {
        self.0.retain(|id, _| places.iter().any(|p| p.id == *id));
    }
}

/// Lets go of the steps of a computer the host's list no longer holds, so they are bounded by the computers there are.
pub fn forget_gone(places: &[PlaceView], cx: &mut App) {
    if cx.try_global::<Readings>().is_some_and(|r| r.holds_gone(places)) {
        cx.global_mut::<Readings>().keep_listed(places);
    }
}

fn reading_of(v: &Value) -> Option<Reading> {
    let num = |path: &str| v.pointer(path).and_then(Value::as_f64);
    Some(Reading { at: num("/at")?, cpu: num("/cpu")?, used: num("/mem/used")?, total: num("/mem/total")? })
}

/// What the road brings the band.
enum Arrival {
    Window(Vec<Reading>),
    Sample(Reading),
}

async fn read_window(host: &HostClient, place: &str) -> Option<Vec<Reading>> {
    let answer = host.call(req::<PlacesReadingsRequest>(json!({ "placeId": place, "range": "15m" }))).await.ok()?;
    let points = serde_json::to_value(&answer.points).ok()?;
    Some(points.as_array()?.iter().filter_map(reading_of).collect())
}

/// Asks for the computer's readings on the socket and forwards them, asking again on each socket that comes back,
/// since a watch dies with the socket that made it.
async fn follow(host: Arc<HostClient>, place: String, tx: futures::channel::mpsc::UnboundedSender<Arrival>) {
    loop {
        if let Ok(mut events) = host.events().await {
            _ = host.call(req::<SysSubscribeRequest>(json!({ "placeId": place }))).await;
            if let Some(points) = read_window(&host, &place).await {
                if tx.unbounded_send(Arrival::Window(points)).is_err() {
                    return;
                }
            }
            while let Some(next) = events.next().await {
                let Ok(Event::PlaceSys(e)) = next else { continue };
                if e.place_id != place {
                    continue;
                }
                let Some(reading) = serde_json::to_value(&e.sample).ok().as_ref().and_then(reading_of) else { continue };
                if tx.unbounded_send(Arrival::Sample(reading)).is_err() {
                    return;
                }
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

/// One row's following: the road on the host's runtime, ended with the row, and the arrivals laid on the steps.
struct Band {
    abort: Option<tokio::task::AbortHandle>,
    unwatch: Option<Box<dyn FnOnce()>>,
    present: bool,
    /// The plot and the figures, each a view of its own once the band stands, which a new step draws again in place.
    drawn: Option<(Entity<BandPart>, Entity<BandPart>)>,
    _hear: Option<Task<()>>,
}

impl Drop for Band {
    fn drop(&mut self) {
        if let Some(abort) = self.abort.take() {
            abort.abort();
        }
        if let Some(unwatch) = self.unwatch.take() {
            unwatch();
        }
    }
}

impl Band {
    /// Starts the road once the host has answered; a band drawn before then waits for it.
    fn start(&mut self, acts: &Acts, place: &str, cx: &mut Context<Band>) {
        let Some((host, runtime)) = acts.host(cx) else { return };
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        let id = place.to_owned();
        self.abort = Some(runtime.spawn(follow(host.clone(), id.clone(), tx)).abort_handle());
        self.unwatch = Some({
            let (id, rt) = (id.clone(), runtime.clone());
            Box::new(move || {
                rt.spawn(async move {
                    _ = host.call(req::<SysUnsubscribeRequest>(json!({ "placeId": id }))).await;
                });
            })
        });
        self._hear = Some(cx.spawn(async move |me, cx| {
            while let Some(arrival) = rx.next().await {
                let id = id.clone();
                let moved = cx.update(|cx| {
                    let steps = cx.default_global::<Readings>().0.entry(id).or_default();
                    match arrival {
                        Arrival::Window(points) => {
                            feed_window(steps, &points);
                            true
                        }
                        Arrival::Sample(reading) => feed_sample(steps, reading),
                    }
                });
                if moved && me.update(cx, |band, cx| band.moved(cx)).is_err() {
                    return;
                }
            }
        }));
    }

    /// New steps: the plot and the figures draw again where they stand, or the row lays the band out once it has none.
    fn moved(&mut self, cx: &mut Context<Band>) {
        match &self.drawn {
            Some((plot, figures)) => {
                plot.update(cx, |_, cx| cx.notify_in_place());
                figures.update(cx, |_, cx| cx.notify_in_place());
            }
            None => cx.notify(),
        }
    }

    /// The band's two parts, made the first time it is drawn, laid on `place` as the row reads it now.
    fn parts(&mut self, place: &PlaceView, away: bool, cx: &mut App) -> (Entity<BandPart>, Entity<BandPart>) {
        let part = |plot: bool, cx: &mut App| cx.new(|_| BandPart { plot, place: place.clone(), away });
        let (plot, figures) = self.drawn.get_or_insert_with(|| (part(true, cx), part(false, cx))).clone();
        for part in [&plot, &figures] {
            part.update(cx, |p, _| (p.place, p.away) = (place.clone(), away));
        }
        (plot, figures)
    }
}

/// One of a band's two parts: the plot under the name, or the figures under the threads.
struct BandPart {
    plot: bool,
    place: PlaceView,
    away: bool,
}

impl Render for BandPart {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        crate::frames::count("ComputerBand");
        let steps = cx.try_global::<Readings>().and_then(|r| r.0.get(&self.place.id)).cloned().unwrap_or_default();
        let (plot, figures) = band_parts(&self.place, &steps, self.away, cx);
        if self.plot {
            plot.into_any_element()
        } else {
            figures.into_any_element()
        }
    }
}

/// Keeps the row's following while the row is drawn, and fills its window again when the computer comes back.
fn keep(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Entity<Band> {
    let present = !is_offline(place);
    let band = window.use_keyed_state(SharedString::from(format!("computer-{}-live", place.id)), cx, move |_, _| Band {
        abort: None,
        unwatch: None,
        present,
        drawn: None,
        _hear: None,
    });
    if band.read(cx).abort.is_none() && ctx.acts.host(cx).is_some() {
        band.update(cx, |b, cx| b.start(&ctx.acts, &place.id, cx));
    }
    if present && !band.read(cx).present {
        let (id, me) = (place.id.clone(), band.downgrade());
        ctx.acts.call(req::<PlacesReadingsRequest>(json!({ "placeId": id, "range": "15m" })), cx, move |answer, cx| {
            let points: Vec<Reading> = answer
                .ok()
                .and_then(|a| serde_json::to_value(&a.points).ok())
                .and_then(|v| v.as_array().map(|a| a.iter().filter_map(reading_of).collect()))
                .unwrap_or_default();
            feed_window(cx.default_global::<Readings>().0.entry(id).or_default(), &points);
            _ = me.update(cx, |band, cx| band.moved(cx));
        });
    }
    band.update(cx, |b, _| b.present = present);
    band
}

/// `ChartPlot` at a lane's size: no rules, a fill from 40% of its ink.
fn lane(points: Vec<(f32, f32)>, ink: Hsla) -> Sparkline {
    Sparkline::new(points, ink).no_rules().fill(0.4).height(LANE)
}

/// `spanPoints`: each step across the window from 0 to 1, and its value from 0 to 1 up.
fn span_points(steps: &[Step], from: f64, to: f64, y: impl Fn(&Step) -> f64) -> Vec<(f32, f32)> {
    steps.iter().map(|s| ((((s.at - from) / (to - from).max(1.)).clamp(0., 1.)) as f32, y(s).clamp(0., 1.) as f32)).collect()
}

/// `lastRead`: the time of day, with its day where that is not today.
fn last_read(at: f64) -> String {
    use chrono::{Local, TimeZone as _};
    let Some(when) = Local.timestamp_millis_opt(at as i64).single() else { return String::new() };
    if when.date_naive() == Local::now().date_naive() {
        format!("Last read {}", when.format("%H:%M"))
    } else {
        format!("Last read {} {}", when.format("%-d %b"), when.format("%H:%M"))
    }
}

/// `Last read HH:MM` where the memory figure stood, in the page's face as the web's `font-sans` sets it inside the mono
/// figure.
fn away_figure(at: f64, sans: SharedString, ink: gpui_kit::Hsla) -> gpui_kit::Div {
    div().font_family(sans).text_color(ink).child(last_read(at))
}

/// The band's two cells on the list's template, the plot under the name, cores and memory and the figures under the
/// threads, the state and the chevron, each over two columns of the narrow template; none until the computer has read
/// once.
pub fn band(ctx: &Ctx, place: &PlaceView, window: &mut Window, cx: &mut App) -> Option<Vec<GridCell>> {
    let band = keep(ctx, place, window, cx);
    cx.try_global::<Readings>().and_then(|r| r.0.get(&place.id)).filter(|s| !s.is_empty())?;
    let away = is_offline(place) || ctx.acts.host(cx).is_none();
    let (plot, figures) = band.update(cx, |band, cx| band.parts(place, away, cx));
    let part = || StyleRefinement::default().w_full().h(LANE * 2. + LANE_GAP);
    Some(vec![GridCell::new(plot.cached(part()), 3, 2), GridCell::new(figures.cached(part()), 3, 2)])
}

/// The band's plot and its figures off the computer's steps.
fn band_parts(place: &PlaceView, steps: &[Step], away: bool, cx: &App) -> (impl IntoElement, impl IntoElement) {
    let last = steps.last().copied().unwrap_or(Step { at: 0., cpu: 0., used: 0., total: 0., n: 0, cpu_now: 0. });
    let (to, from) = (last.at, last.at - WINDOW_MS);
    let full = last.total > 0. && last.used / last.total >= MEMORY_NEAR_FULL;
    let p = cx.palette().clone();
    let away_ink = p.muted_foreground.opacity(0.45);
    let cpu_ink = if away { away_ink } else { p.hue_blue };
    let mem_ink = if away {
        away_ink
    } else if full {
        p.warning_foreground
    } else {
        p.hue_violet
    };
    let cpu = span_points(steps, from, to, |s| s.cpu / 100.);
    let mem = span_points(steps, from, to, |s| if s.total > 0. { s.used / s.total } else { 0. });
    let plot = v_flex()
        .id(SharedString::from(format!("live-band-{}", place.id)))
        .debug_selector(|| "live-band".into())
        .w_full()
        .pl(PLOT_INSET)
        .gap(LANE_GAP)
        .tooltip(tip("CPU and memory over the last 15 minutes"))
        .child(lane(cpu, cpu_ink))
        .child(lane(mem, mem_ink));
    let (mono, sans) = (cx.theme().mono_font_family.clone(), cx.theme().font_family.clone());
    let figure = |word: Option<&'static str>, value: AnyElement| {
        h_flex()
            .h(LANE)
            .gap(px(6.))
            .whitespace_nowrap()
            .font_family(mono.clone())
            .text_size(text::XS.0)
            .line_height(text::XS.1)
            .children(word.map(|w| div().flex_none().w(WORD_SLOT).font_family(sans.clone()).text_color(p.muted_foreground).child(w)))
            .child(value)
    };
    let value_ink = if away { p.muted_foreground } else { p.foreground };
    let cpu_figure = figure(Some("CPU"), div().text_color(value_ink).child(format!("{}%", last.cpu_now.round() as i64)).into_any_element());
    let mem_figure = if away {
        figure(None, away_figure(to, sans.clone(), p.muted_foreground).into_any_element())
    } else {
        let ink = if full { p.warning_foreground } else { p.foreground };
        figure(Some("Memory"), div().text_color(ink).child(fmt_mem_gb(last.used / 1024. / 1024.)).into_any_element())
    };
    let figures = v_flex().debug_selector(|| "live-figures".into()).w_full().gap(LANE_GAP).child(cpu_figure).child(mem_figure);
    (plot, figures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_read_is_set_in_the_pages_face_inside_the_mono_figure() {
        let mut away = away_figure(0., SharedString::from("Sans"), gpui_kit::black());
        assert_eq!(away.style().text.font_family, Some(SharedString::from("Sans")));
    }

    fn at(s: f64) -> Reading {
        Reading { at: s * 1000., cpu: s, used: 1., total: 2. }
    }

    #[test]
    fn a_removed_computer_is_let_go() {
        let place = |id: &str| -> PlaceView {
            serde_json::from_value(json!({ "id": id, "kind": "computer", "name": id, "default": false })).unwrap()
        };
        let mut readings = Readings::default();
        readings.0.insert("a".into(), Vec::new());
        readings.0.insert("b".into(), Vec::new());
        let listed = [place("a")];
        assert!(readings.holds_gone(&listed));
        readings.keep_listed(&listed);
        assert_eq!(readings.0.keys().collect::<Vec<_>>(), ["a"]);
        assert!(!readings.holds_gone(&listed));
    }

    #[test]
    fn samples_fold_into_ten_second_steps_and_only_a_new_step_draws() {
        let mut steps = Vec::new();
        assert!(feed_sample(&mut steps, at(100.)));
        assert!(!feed_sample(&mut steps, at(102.)));
        assert!(!feed_sample(&mut steps, at(104.)));
        assert!(!feed_sample(&mut steps, at(104.)));
        assert!(feed_sample(&mut steps, at(110.)));
        assert_eq!(steps.len(), 2);
        assert_eq!(steps[0].cpu, 102.);
        assert_eq!(steps[0].cpu_now, 104.);
        assert_eq!(steps[0].n, 3);
    }

    #[test]
    fn a_window_holds_at_most_fifteen_minutes() {
        let mut steps = Vec::new();
        let points: Vec<Reading> = (0..1200).map(|i| at(i as f64 * 2.)).collect();
        feed_window(&mut steps, &points);
        assert_eq!(steps.len(), 91);
        assert!(feed_sample(&mut steps, at(2400.)));
        assert_eq!(steps.len(), 91);
        let held = steps.last().copied();
        feed_window(&mut steps, &points[..10]);
        assert_eq!(steps.last().copied(), held);
    }
}
