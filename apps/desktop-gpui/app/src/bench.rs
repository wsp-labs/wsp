// SPDX-License-Identifier: AGPL-3.0-only
//! Bench mode's flags and its result, in the shape the Electron bench writes too.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{App, Global};

use crate::ui::theme::{Side, THEMES};

/// The scroll rate both apps' benches run at.
pub const PX_PER_S: f32 = 2400.;

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub state: Option<PathBuf>,
    pub bench: Option<Bench>,
    /// A thread to open at launch and keep open, as memory with that thread open is read.
    pub open: Option<String>,
    /// A frosted, composer-sized panel over the bottom of the thread view, to measure what backdrop blur costs.
    pub glass: bool,
    /// The gallery of shared pieces in place of the app.
    pub gallery: bool,
    /// A screen to open at launch by its name, as captures of one screen start; after `--open`'s thread, so a pane
    /// opens on it.
    pub screen: Option<String>,
    /// How a capture stands the thread view: its theme, its clicks, its clock and where it scrolls.
    pub capture: Capture,
    /// Beside `--open`: where the wheel events over the thread go, for `--bench-replay` to play again.
    pub record: Option<PathBuf>,
    /// Beside `--open`: where the frame-time trace of the first scroll's 60 s goes.
    pub trace: Option<PathBuf>,
}

/// A capture's flags, each beside `--open`: `--theme <id>`, `--unfold`, `--now <ms>`, `--reveal <text>@<y>`,
/// `--drop <x>,<y>`.
#[derive(Debug, Default, PartialEq)]
pub struct Capture {
    pub theme: Option<String>,
    pub unfold: bool,
    pub now: Option<f64>,
    pub reveal: Option<(String, f32)>,
    /// Where in the window a file is held dragged, never let go, so the chat's drop frame stands for the shot.
    pub drop: Option<(f32, f32)>,
}

#[derive(Debug, PartialEq)]
pub struct Bench {
    pub out: PathBuf,
    /// The thread the bench opens, or with `screen` the screen's route.
    pub thread: String,
    /// The bench scrolls a screen, Settings > Activity, in place of a thread.
    pub screen: bool,
    pub switch: Vec<String>,
    /// A line typed into the thread's terminal drawer, in place of the scroll: the frames its output draws are timed.
    pub terminal: Option<String>,
    /// Wheel events to play over the thread from its end, in place of the scroll; `out` takes their frame-time trace.
    pub replay: Option<PathBuf>,
    /// Where the pointer rests for every replayed event, in the list's own pixels, in place of where each says.
    pub pointer: Option<(f32, f32)>,
    /// Every turn's fold opened before the replay, so tool output stands in the transcript.
    pub unfold: bool,
    /// A held Ctrl+Tab played from the thread, in place of the scroll; `out` takes each step's trace.
    pub hold: Option<Hold>,
    /// Threads opened one after another, as a person moves down the sidebar, in place of the scroll: memory after each.
    pub session: Vec<String>,
    /// With `screen` on Settings > Appearance: themes picked and transparency set one after another through the page's
    /// own writes, each a theme's id, `solid` or `transparent`; `out` takes each switch's frames.
    pub themes: Vec<String>,
}

/// A held Ctrl+Tab: its press and `steps - 1` repeats, the first `delay_ms` after the press and each next `every_ms`
/// after the last, as a Mac's key repeat sends them.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Hold {
    pub steps: usize,
    pub delay_ms: f64,
    pub every_ms: f64,
}

/// The fastest key repeat a Mac's Keyboard settings offer, `InitialKeyRepeat` 15 and `KeyRepeat` 2 in 15 ms ticks.
pub const MAC_REPEAT: (f64, f64) = (225., 30.);

impl Hold {
    /// When step `n` is sent, in ms from the press.
    pub fn due_ms(&self, n: usize) -> f64 {
        if n == 0 {
            0.
        } else {
            self.delay_ms + self.every_ms * (n - 1) as f64
        }
    }
}

/// The bench's modes, of which a run takes one.
const MODE_FLAGS: [&str; 6] =
    ["--bench-switch", "--bench-terminal", "--bench-replay", "--bench-ctrl-tab", "--bench-session", "--bench-themes"];

/// `desktop-gpui [--state <state.json>] [--glass] [--open <id>] [--screen <name>]`, `desktop-gpui [--state <state.json>] [--glass]
/// --bench <out.json> --bench-thread <id> [--bench-switch <id,id,...> | --bench-terminal <line> | --bench-replay <events.jsonl>
/// [--bench-pointer <x>,<y>] [--bench-unfold] | --bench-ctrl-tab <steps> [--bench-repeat <delay ms>,<every ms>] | --bench-session <id,id,...>]`,
/// `desktop-gpui [--state <state.json>] --bench <out.json> --bench-screen settings:activity --bench-replay <events.jsonl>`,
/// `desktop-gpui [--state <state.json>] --bench <out.json> --bench-screen settings:appearance --bench-themes <all | id,solid,...>`,
/// `desktop-gpui --open <id> [--theme <id>] [--unfold] [--now <ms>] [--reveal <text>@<y>]` for a capture,
/// `desktop-gpui --open <id> [--record-scroll <events.jsonl>] [--trace <trace.json>]`, or `desktop-gpui --gallery`.
pub fn parse(mut words: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut state, mut out, mut thread, mut switch, mut open, mut glass, mut gallery, mut screen, mut terminal) =
        (None, None, None, Vec::new(), None, false, false, None, None);
    let (mut replay, mut record, mut trace, mut session, mut themes) = (None, None, None, Vec::new(), Vec::new());
    let (mut pointer, mut unfold, mut on_screen) = (None, false, false);
    let (mut steps, mut repeat) = (None, None);
    let mut capture = Capture::default();
    while let Some(flag) = words.next() {
        let mut value = || words.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--state" => state = Some(PathBuf::from(value()?)),
            "--open" => open = Some(value()?),
            "--screen" => screen = Some(value()?),
            "--bench" => out = Some(PathBuf::from(value()?)),
            "--bench-thread" => thread = Some(value()?),
            "--bench-screen" => {
                thread = Some(value()?);
                on_screen = true;
            }
            "--gallery" => gallery = true,
            "--bench-switch" => switch = value()?.split(',').filter(|id| !id.is_empty()).map(str::to_owned).collect(),
            "--bench-terminal" => terminal = Some(value()?),
            "--bench-session" => session = value()?.split(',').filter(|id| !id.is_empty()).map(str::to_owned).collect(),
            "--bench-replay" => replay = Some(PathBuf::from(value()?)),
            "--bench-themes" => themes = theme_words(&value()?)?,
            "--bench-pointer" => {
                let said = value()?;
                let (x, y) = said.split_once(',').ok_or("--bench-pointer takes <x>,<y>")?;
                let num = |n: &str| n.trim().parse::<f32>().map_err(|e| format!("--bench-pointer: {e}"));
                pointer = Some((num(x)?, num(y)?));
            }
            "--bench-unfold" => unfold = true,
            "--bench-ctrl-tab" => steps = Some(value()?.parse::<usize>().map_err(|e| format!("--bench-ctrl-tab: {e}"))?),
            "--bench-repeat" => {
                let said = value()?;
                let (delay, every) = said.split_once(',').ok_or("--bench-repeat takes <delay ms>,<every ms>")?;
                let ms = |n: &str| n.trim().parse::<f64>().map_err(|e| format!("--bench-repeat: {e}"));
                repeat = Some((ms(delay)?, ms(every)?));
            }
            "--record-scroll" => record = Some(PathBuf::from(value()?)),
            "--trace" => trace = Some(PathBuf::from(value()?)),
            "--glass" => glass = true,
            "--theme" => capture.theme = Some(value()?),
            "--unfold" => capture.unfold = true,
            "--now" => capture.now = Some(value()?.parse::<f64>().map_err(|e| format!("--now: {e}"))?),
            "--reveal" => {
                let said = value()?;
                let (text, y) = said.rsplit_once('@').ok_or("--reveal takes <text>@<y>")?;
                capture.reveal = Some((text.to_owned(), y.parse::<f32>().map_err(|e| format!("--reveal: {e}"))?));
            }
            "--drop" => {
                let said = value()?;
                let (x, y) = said.split_once(',').ok_or("--drop takes <x>,<y>")?;
                let num = |n: &str| n.trim().parse::<f32>().map_err(|e| format!("--drop: {e}"));
                capture.drop = Some((num(x)?, num(y)?));
            }
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    if repeat.is_some() && steps.is_none() {
        return Err("--bench-repeat goes with --bench-ctrl-tab".to_owned());
    }
    let (delay_ms, every_ms) = repeat.unwrap_or(MAC_REPEAT);
    let hold = steps.map(|steps| Hold { steps, delay_ms, every_ms });
    let modes = [!switch.is_empty(), terminal.is_some(), replay.is_some(), hold.is_some(), !session.is_empty(), !themes.is_empty()];
    let bench = match (out, thread) {
        (Some(out), Some(thread)) if modes.iter().filter(|x| **x).count() <= 1 => {
            Some(Bench { out, thread, screen: on_screen, switch, terminal, replay, pointer, unfold, hold, session, themes })
        }
        (Some(_), Some(_)) => return Err(format!("{} go one at a time", MODE_FLAGS.join(", "))),
        (None, None) if !modes.contains(&true) => None,
        _ => return Err("--bench and --bench-thread go together".to_owned()),
    };
    if on_screen && bench.as_ref().is_none_or(|b| (b.replay.is_none() && b.themes.is_empty()) || b.unfold || !b.switch.is_empty()) {
        return Err("--bench-screen goes with --bench and --bench-replay or --bench-themes".to_owned());
    }
    if bench.as_ref().is_some_and(|b| !b.themes.is_empty() && !b.screen) {
        return Err("--bench-themes goes with --bench-screen settings:appearance".to_owned());
    }
    if bench.is_some() && open.is_some() {
        return Err("--open and --bench do not go together".to_owned());
    }
    if gallery && (bench.is_some() || open.is_some() || glass) {
        return Err("--gallery goes alone".to_owned());
    }
    if screen.is_some() && (bench.is_some() || gallery) {
        return Err("--screen goes without --bench and --gallery".to_owned());
    }
    if capture != Capture::default() && open.is_none() {
        return Err("--theme, --unfold, --now, --reveal and --drop go with --open".to_owned());
    }
    if (pointer.is_some() || unfold) && bench.as_ref().is_none_or(|b| b.replay.is_none()) {
        return Err("--bench-pointer and --bench-unfold go with --bench-replay".to_owned());
    }
    if (record.is_some() || trace.is_some()) && open.is_none() {
        return Err("--record-scroll and --trace go with --open".to_owned());
    }
    Ok(Args { state, bench, open, glass, gallery, screen, capture, record, trace })
}

/// The result JSON: frame times are milliseconds from the start of the scroll. `timed` are the switches after the
/// bench's own, each to the first presented frame holding the thread's rows, with their median, 90th percentile and
/// slowest; the Electron bench writes none.
pub fn result(thread_rows: usize, frame_ms: &[f64], switch: &[(String, f64)], timed: &[(String, f64)]) -> serde_json::Value {
    let each =
        |list: &[(String, f64)]| list.iter().map(|(thread, ms)| serde_json::json!({ "thread": thread, "ms": ms })).collect::<Vec<_>>();
    let mut sorted: Vec<f64> = timed.iter().map(|(_, ms)| *ms).collect();
    sorted.sort_by(f64::total_cmp);
    let rank = |share: f64| sorted.get(((share * sorted.len() as f64).ceil() as usize).max(1) - 1).copied();
    let median = match sorted.len() {
        0 => None,
        n if n % 2 == 1 => Some(sorted[n / 2]),
        n => Some((sorted[n / 2 - 1] + sorted[n / 2]) / 2.),
    };
    serde_json::json!({
        "app": "gpui",
        "thread_rows": thread_rows,
        "scroll": { "px_per_s": PX_PER_S, "frame_ms": frame_ms },
        "switch": each(switch),
        "switches": { "each": each(timed), "median_ms": median, "p90_ms": rank(0.9), "max_ms": sorted.last() },
    })
}

/// The terminal bench's result: when each terminal frame was painted and when each batch of output reached the
/// shell, in milliseconds from the moment the line was sent.
pub fn terminal_result(line: &str, frame_ms: &[f64], output_ms: &[f64]) -> serde_json::Value {
    serde_json::json!({ "app": "gpui", "terminal": { "line": line, "frame_ms": frame_ms, "output_ms": output_ms } })
}

/// One step of a held Ctrl+Tab as the bench saw it, in ms from the press: when it was due, sent through the window
/// and handled, the threads open before and after it, the first frame presented after it, and the first frame
/// holding the opened thread's rows, none where the next step came first.
#[derive(Clone, Debug, PartialEq)]
pub struct Step {
    pub due_ms: f64,
    pub sent_ms: f64,
    pub handled_ms: f64,
    pub from: Option<String>,
    pub to: Option<String>,
    pub presented_ms: Option<f64>,
    pub rows_ms: Option<f64>,
    /// A frame presented between this key and the next drew the transcript's loading line.
    pub loading: bool,
}

/// The Ctrl+Tab bench's trace: each step with how long its key took to reach the screen, and whether the opened
/// thread's rows stood in the first frame presented after the key; the frame interval is read off the frames
/// presented while the key was held.
pub fn hold_result(hold: &Hold, steps: &[Step], frames_ms: &[f64]) -> serde_json::Value {
    let mut gaps: Vec<f64> = frames_ms.windows(2).map(|w| w[1] - w[0]).collect();
    gaps.sort_by(f64::total_cmp);
    let frame_ms = gaps.get(gaps.len() / 2).copied();
    let opened = |s: &Step| s.to.is_some() && s.to != s.from;
    let after = |s: &Step, at: Option<f64>| at.map(|at| at - s.sent_ms);
    // The frames presented after the key and before the one holding the rows; the frame holding them is noted a moment
    // after them, so only a frame a whole millisecond earlier counts.
    let frames_before = |s: &Step| s.rows_ms.map(|rows| frames_ms.iter().filter(|&&f| f > s.sent_ms && f < rows - 1.).count());
    let within = |s: &Step| opened(s) && frames_before(s) == Some(0);
    let spread = |of: &dyn Fn(&Step) -> Option<f64>| {
        let mut sorted: Vec<f64> = steps.iter().filter_map(of).collect();
        sorted.sort_by(f64::total_cmp);
        let rank = |share: f64| sorted.get(((share * sorted.len() as f64).ceil() as usize).max(1) - 1).copied();
        serde_json::json!({ "median_ms": rank(0.5), "p90_ms": rank(0.9), "max_ms": sorted.last() })
    };
    let distinct: std::collections::BTreeSet<&str> = steps.iter().filter(|s| opened(s)).filter_map(|s| s.to.as_deref()).collect();
    let each: Vec<serde_json::Value> = steps
        .iter()
        .enumerate()
        .map(|(n, s)| {
            serde_json::json!({
                "n": n, "held": n > 0, "due_ms": s.due_ms, "sent_ms": s.sent_ms, "handled_ms": s.handled_ms, "from": s.from,
                "to": s.to, "presented_ms": s.presented_ms, "rows_ms": s.rows_ms, "latency_ms": after(s, s.presented_ms),
                "rows_latency_ms": after(s, s.rows_ms), "frames_before_rows": frames_before(s), "within_frame": within(s),
                "drew_loading": s.loading,
            })
        })
        .collect();
    serde_json::json!({
        "app": "gpui",
        "mode": "ctrl-tab",
        "release": !cfg!(debug_assertions),
        "repeat": { "delay_ms": hold.delay_ms, "every_ms": hold.every_ms },
        "frame_ms": frame_ms,
        "summary": {
            "steps": steps.len(),
            "opened": steps.iter().filter(|s| opened(s)).count(),
            "threads": distinct.len(),
            "within_frame": steps.iter().filter(|s| within(s)).count(),
            "drew_loading": steps.iter().filter(|s| s.loading).count(),
            "key_to_frame": spread(&|s| after(s, s.presented_ms)),
            "key_to_rows": spread(&|s| after(s, s.rows_ms)),
        },
        "steps": each,
        "frames_ms": frames_ms,
    })
}

/// `--bench-themes`' words: every theme's id, `solid` or `transparent`; `all` is every theme in the picker's order, then
/// transparency off and on.
fn theme_words(said: &str) -> Result<Vec<String>, String> {
    if said == "all" {
        return Ok(THEMES.iter().map(|t| t.id.to_owned()).chain(["solid".to_owned(), "transparent".to_owned()]).collect());
    }
    let words: Vec<String> = said.split(',').filter(|w| !w.is_empty()).map(str::to_owned).collect();
    match words.iter().find(|w| theme_patch(w).is_none()) {
        Some(w) => Err(format!("--bench-themes: {w} is no theme, solid or transparent")),
        None if words.is_empty() => Err("--bench-themes takes all or theme ids".to_owned()),
        None => Ok(words),
    }
}

/// The preferences patch Settings > Appearance writes for a word of `--bench-themes`: a theme asks for its side too,
/// so it shows whatever side the system is on.
pub fn theme_patch(word: &str) -> Option<serde_json::Value> {
    match word {
        "solid" => Some(serde_json::json!({ "transparency": false })),
        "transparent" => Some(serde_json::json!({ "transparency": true })),
        id => THEMES.iter().find(|t| t.id == id).map(|t| match t.side {
            Side::Light => serde_json::json!({ "theme": "light", "lightTheme": t.id }),
            Side::Dark => serde_json::json!({ "theme": "dark", "darkTheme": t.id }),
        }),
    }
}

/// A switch is settled this long after it was sent: a frame drawn later, before the next switch, is a loop of renders.
pub const SETTLED_MS: f64 = 300.;

/// One switch as the theme bench sent it: its word, when, in ms from the trace's start, and the ms the page's write took
/// on the UI thread.
#[derive(Clone, Debug, PartialEq)]
pub struct Sent {
    pub word: String,
    pub at_ms: f64,
    pub write_ms: f64,
}

/// Each switch read off the trace's frames, from when it was sent to the next or `pause_ms` on: when the last frame
/// started within `SETTLED_MS` of it was on screen (`took_ms`, and `took_app_ms` without the presents, which on a
/// machine with no GPU are the software rasteriser's), the slowest of those frames with the write before the first,
/// and the frames and renders after it settled.
pub fn theme_switches(frames: &[serde_json::Value], sent: &[Sent], pause_ms: f64) -> serde_json::Value {
    let num = |f: &serde_json::Value, key: &str| f[key].as_f64().unwrap_or(0.);
    let views = |frames: &[&serde_json::Value]| {
        let mut views = std::collections::BTreeMap::<String, u64>::new();
        for f in frames {
            for (view, n) in f["renders"].as_object().into_iter().flatten().filter(|(view, _)| *view != "frame") {
                *views.entry(view.clone()).or_default() += n.as_u64().unwrap_or(0);
            }
        }
        views
    };
    let each: Vec<serde_json::Value> = sent
        .iter()
        .enumerate()
        .map(|(ix, s)| {
            let end = sent.get(ix + 1).map_or(s.at_ms + pause_ms, |next| next.at_ms.min(s.at_ms + pause_ms));
            let within: Vec<&serde_json::Value> = frames.iter().filter(|f| num(f, "at") >= s.at_ms && num(f, "at") < end).collect();
            let (settling, after): (Vec<&serde_json::Value>, Vec<&serde_json::Value>) =
                within.iter().partition(|f| num(f, "at") < s.at_ms + SETTLED_MS);
            let took = settling.last().map(|f| num(f, "at") + num(f, "draw") + num(f, "present") - s.at_ms);
            let presents: f64 = settling.iter().map(|f| num(f, "present")).sum();
            let slowest = |cost: &dyn Fn(&serde_json::Value) -> f64| settling.iter().map(|f| cost(f)).fold(0., f64::max);
            let renders_after: u64 = views(&after).values().sum();
            serde_json::json!({
                "word": s.word, "patch": theme_patch(&s.word), "at_ms": s.at_ms, "write_ms": s.write_ms, "frames": within.len(),
                "took_ms": took, "took_app_ms": took.map(|t| t - presents),
                "slowest_ms": slowest(&|f| num(f, "draw") + num(f, "present") + num(f, "input_ms")),
                "slowest_app_ms": slowest(&|f| num(f, "app")),
                "frames_after": after.len(),
                "renders_after_per_frame": if after.is_empty() { 0. } else { renders_after as f64 / after.len() as f64 },
                "views": views(&within), "views_after": views(&after),
            })
        })
        .collect();
    serde_json::Value::Array(each)
}

/// Where a terminal bench keeps the times its terminals paint at; absent outside one.
#[derive(Default)]
pub struct Frames(pub Option<Rc<RefCell<Vec<Instant>>>>);

impl Global for Frames {}

/// A terminal painted a frame.
pub fn painted(cx: &App) {
    if let Some(frames) = cx.try_global::<Frames>().and_then(|f| f.0.as_ref()) {
        frames.borrow_mut().push(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(line: &str) -> impl Iterator<Item = String> + '_ {
        line.split_whitespace().map(str::to_owned)
    }

    #[test]
    fn reads_the_bench_flags() {
        let args = parse(words("--state /s/state.json --bench /o.json --bench-thread long --bench-switch a,b,c")).unwrap();
        assert_eq!(args.state, Some(PathBuf::from("/s/state.json")));
        assert_eq!(
            args.bench,
            Some(Bench {
                out: "/o.json".into(),
                thread: "long".into(),
                screen: false,
                switch: vec!["a".into(), "b".into(), "c".into()],
                terminal: None,
                replay: None,
                pointer: None,
                unfold: false,
                hold: None,
                session: Vec::new(),
                themes: Vec::new()
            })
        );
        let typed =
            parse(["--bench", "/o.json", "--bench-thread", "short", "--bench-terminal", "seq 1 100000"].map(str::to_owned).into_iter())
                .unwrap();
        assert_eq!(typed.bench.and_then(|b| b.terminal), Some("seq 1 100000".to_owned()));
        assert!(parse(words("--bench-terminal vim")).is_err());
        assert!(parse(words("--bench /o.json --bench-thread long --bench-switch a --bench-terminal vim")).is_err());
        assert_eq!(parse(words("--state /s")).unwrap().bench, None);
        assert!(parse(words("--bench /o.json")).is_err());
        assert!(parse(words("--bench-thread")).is_err());
        assert_eq!(parse(words("--state /s --open long")).unwrap().open, Some("long".to_owned()));
        assert!(parse(words("--open long --bench /o.json --bench-thread long")).is_err());
        assert!(parse(words("--glass --bench /o.json --bench-thread long")).unwrap().glass);
        assert!(!parse(words("--bench /o.json --bench-thread long")).unwrap().glass);
        assert!(parse(words("--gallery")).unwrap().gallery);
        assert!(parse(words("--gallery --open long")).is_err());
        assert!(parse(words("--gallery --glass")).is_err());
        assert_eq!(parse(words("--screen settings --state /s")).unwrap().screen, Some("settings".to_owned()));
        assert_eq!(parse(words("--screen changes --open long")).unwrap().open, Some("long".to_owned()));
        assert!(parse(words("--screen settings --gallery")).is_err());
        let shot = parse(words("--open t --theme graphite --unfold --now 5 --reveal turn@189")).unwrap().capture;
        assert_eq!(
            shot,
            Capture { theme: Some("graphite".into()), unfold: true, now: Some(5.), reveal: Some(("turn".into(), 189.)), drop: None }
        );
        assert!(parse(words("--theme graphite")).is_err());
        assert_eq!(parse(words("--open t --drop 700,300.5")).unwrap().capture.drop, Some((700., 300.5)));
        assert!(parse(words("--drop 700,300")).is_err());
        assert!(parse(words("--open t --drop 700")).is_err());
        let replay = parse(words("--bench /t.json --bench-thread real --bench-replay /e.jsonl")).unwrap().bench.unwrap();
        assert_eq!(replay.replay, Some(PathBuf::from("/e.jsonl")));
        assert!(parse(words("--bench /t.json --bench-thread real --bench-replay /e.jsonl --bench-switch a")).is_err());
        assert!(parse(words("--bench-replay /e.jsonl")).is_err());
        let over =
            parse(words("--bench /t.json --bench-thread real --bench-replay /e.jsonl --bench-pointer 380,240.5 --bench-unfold")).unwrap();
        assert_eq!(over.bench.map(|b| (b.pointer, b.unfold)), Some((Some((380., 240.5)), true)));
        assert!(parse(words("--bench /t.json --bench-thread real --bench-pointer 1,2")).is_err());
        assert!(parse(words("--bench /t.json --bench-thread real --bench-replay /e.jsonl --bench-pointer 12")).is_err());
        let session = parse(words("--bench /s.json --bench-thread a --bench-session a,b,c")).unwrap().bench.unwrap();
        assert_eq!(session.session, vec!["a".to_owned(), "b".into(), "c".into()]);
        assert!(parse(words("--bench /s.json --bench-thread a --bench-session a,b --bench-switch a")).is_err());
        assert!(parse(words("--bench-session a,b")).is_err());
        let recorded = parse(words("--open t --record-scroll /e.jsonl --trace /t.json")).unwrap();
        assert_eq!((recorded.record, recorded.trace), (Some(PathBuf::from("/e.jsonl")), Some(PathBuf::from("/t.json"))));
        let held = parse(words("--bench /h.json --bench-thread t00 --bench-ctrl-tab 24")).unwrap().bench.unwrap();
        assert_eq!(held.hold, Some(Hold { steps: 24, delay_ms: 225., every_ms: 30. }));
        let slow = parse(words("--bench /h.json --bench-thread t00 --bench-ctrl-tab 3 --bench-repeat 500,83.3")).unwrap().bench.unwrap();
        assert_eq!(slow.hold, Some(Hold { steps: 3, delay_ms: 500., every_ms: 83.3 }));
        assert!(parse(words("--bench /h.json --bench-thread t00 --bench-repeat 500,83")).is_err());
        assert!(parse(words("--bench /h.json --bench-thread t00 --bench-ctrl-tab 3 --bench-switch a")).is_err());
        assert!(parse(words("--bench-ctrl-tab 3")).is_err());
        assert!(parse(words("--record-scroll /e.jsonl")).is_err());
        assert!(parse(words("--trace /t.json")).is_err());
    }

    #[test]
    fn reads_the_theme_bench_flags() {
        let all = parse(words("--bench /t.json --bench-screen settings:appearance --bench-themes all")).unwrap().bench.unwrap();
        assert_eq!(all.themes.len(), THEMES.len() + 2);
        assert_eq!(all.themes[THEMES.len()..], ["solid".to_owned(), "transparent".into()]);
        let some = parse(words("--bench /t.json --bench-screen settings:appearance --bench-themes moss,solid")).unwrap().bench.unwrap();
        assert_eq!((some.screen, some.themes), (true, vec!["moss".to_owned(), "solid".into()]));
        assert!(parse(words("--bench /t.json --bench-screen settings:appearance --bench-themes moss,nope")).is_err());
        assert!(parse(words("--bench /t.json --bench-thread t --bench-themes moss")).is_err(), "Settings must be on screen");
        assert!(parse(words("--bench /t.json --bench-screen settings:appearance --bench-themes moss --bench-replay /e.jsonl")).is_err());
        assert_eq!(theme_patch("paper"), Some(serde_json::json!({ "theme": "light", "lightTheme": "paper" })));
        assert_eq!(theme_patch("pitch"), Some(serde_json::json!({ "theme": "dark", "darkTheme": "pitch" })));
    }

    /// Catches: a frame drawn after the switch settled counted into its time rather than as a loop, a frame of the next
    /// switch counted into this one, and the presents left in the time judged by the app's own.
    #[test]
    fn each_theme_switch_reads_its_own_frames() {
        let frame = |at: f64, draw: f64, present: f64, renders: serde_json::Value| {
            let app = draw + 1.;
            serde_json::json!({ "at": at, "draw": draw, "present": present, "app": app, "input_ms": 1., "renders": renders })
        };
        let frames = [
            frame(90., 5., 5., serde_json::json!({})),
            frame(101., 30., 10., serde_json::json!({ "Shell": 1, "SettingsScreen": 1, "frame": 1 })),
            frame(150., 4., 6., serde_json::json!({ "Shell": 1 })),
            frame(450., 2., 2., serde_json::json!({ "Spinner": 2 })),
            frame(500., 2., 2., serde_json::json!({ "Spinner": 2 })),
            frame(705., 20., 5., serde_json::json!({ "Shell": 1 })),
        ];
        let sent = |word: &str, at_ms: f64| Sent { word: word.into(), at_ms, write_ms: 1. };
        let json = theme_switches(&frames, &[sent("moss", 100.), sent("solid", 700.)], 600.);
        let first = &json[0];
        assert_eq!(first["frames"], 4);
        assert_eq!((first["took_ms"].as_f64(), first["took_app_ms"].as_f64()), (Some(60.), Some(44.)));
        assert_eq!((first["slowest_ms"].as_f64(), first["slowest_app_ms"].as_f64()), (Some(41.), Some(31.)));
        assert_eq!((first["frames_after"].as_u64(), first["renders_after_per_frame"].as_f64()), (Some(2), Some(2.)));
        assert_eq!(first["views"], serde_json::json!({ "SettingsScreen": 1, "Shell": 2, "Spinner": 4 }));
        assert_eq!(json[1]["took_ms"].as_f64(), Some(30.));
        assert_eq!(json[1]["frames_after"], 0);
    }

    #[test]
    fn writes_the_shape_both_apps_share() {
        let json = result(3, &[0.0, 16.7], &[("a".to_owned(), 41.0)], &[]);
        assert_eq!(
            json,
            serde_json::json!({"app": "gpui", "thread_rows": 3, "scroll": {"px_per_s": 2400.0, "frame_ms": [0.0, 16.7]}, "switch": [{"thread": "a", "ms": 41.0}],
                "switches": {"each": [], "median_ms": null, "p90_ms": null, "max_ms": null}})
        );
    }

    #[test]
    fn the_timed_switches_carry_their_median_90th_percentile_and_slowest() {
        let timed: Vec<(String, f64)> = (1..=30).map(|i| (format!("t{}", i % 3), f64::from(i) * 10.)).collect();
        let json = result(3, &[], &[], &timed);
        let switches = &json["switches"];
        assert_eq!(switches["each"].as_array().unwrap().len(), 30);
        assert_eq!(switches["each"][0], serde_json::json!({"thread": "t1", "ms": 10.0}));
        assert_eq!(
            (switches["median_ms"].as_f64(), switches["p90_ms"].as_f64(), switches["max_ms"].as_f64()),
            (Some(155.), Some(270.), Some(300.))
        );
    }

    /// Catches: a step whose rows came a frame after the first frame past its key judged within one frame, a step that
    /// opened nothing counted, and the repeat's schedule drifting from a Mac's: the press at once, the first repeat
    /// after the delay, then one each interval.
    #[test]
    fn the_ctrl_tab_trace_judges_each_step_by_the_first_frame_after_its_key() {
        let hold = Hold { steps: 3, delay_ms: 225., every_ms: 30. };
        assert_eq!([hold.due_ms(0), hold.due_ms(1), hold.due_ms(2)], [0., 225., 255.]);
        let step = |due: f64, to: &str, presented: Option<f64>, rows: Option<f64>| Step {
            due_ms: due,
            sent_ms: due + 0.5,
            handled_ms: due + 1.,
            from: Some("t00".into()),
            to: Some(to.into()),
            presented_ms: presented,
            rows_ms: rows,
            loading: to == "t02",
        };
        let frames = [9., 25., 241., 258., 275.];
        let steps =
            [step(0., "t01", Some(9.), Some(9.)), step(225., "t02", Some(241.), Some(258.)), step(255., "t00", Some(258.), Some(258.))];
        let json = hold_result(&hold, &steps, &frames);
        assert_eq!(json["frame_ms"], 17., "the median gap between presented frames");
        let within: Vec<bool> = json["steps"].as_array().unwrap().iter().map(|s| s["within_frame"].as_bool().unwrap()).collect();
        assert_eq!(within, [true, false, false], "the second's rows came a frame late, the third opened the thread it left");
        assert_eq!(json["steps"][1]["frames_before_rows"], 1);
        assert_eq!((json["steps"][1]["latency_ms"].as_f64(), json["steps"][1]["rows_latency_ms"].as_f64()), (Some(15.5), Some(32.5)));
        let summary = &json["summary"];
        assert_eq!(
            (summary["opened"].as_u64(), summary["threads"].as_u64(), summary["within_frame"].as_u64()),
            (Some(2), Some(2), Some(1))
        );
        assert_eq!(summary["key_to_rows"]["max_ms"], 32.5);
        assert_eq!((json["steps"][1]["drew_loading"].as_bool(), summary["drew_loading"].as_u64()), (Some(true), Some(1)));
    }
}
