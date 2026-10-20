// SPDX-License-Identifier: AGPL-3.0-only
//! Bench mode's flags and its result, in the shape the Electron bench writes too.

use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use gpui_kit::{App, Global};

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

/// A capture's flags, each beside `--open`: `--theme <id>`, `--unfold`, `--now <ms>`, `--reveal <text>@<y>`.
#[derive(Debug, Default, PartialEq)]
pub struct Capture {
    pub theme: Option<String>,
    pub unfold: bool,
    pub now: Option<f64>,
    pub reveal: Option<(String, f32)>,
}

#[derive(Debug, PartialEq)]
pub struct Bench {
    pub out: PathBuf,
    pub thread: String,
    pub switch: Vec<String>,
    /// A line typed into the thread's terminal drawer, in place of the scroll: the frames its output draws are timed.
    pub terminal: Option<String>,
    /// Wheel events to play over the thread from its end, in place of the scroll; `out` takes their frame-time trace.
    pub replay: Option<PathBuf>,
    /// Where the pointer rests for every replayed event, in the list's own pixels, in place of where each says.
    pub pointer: Option<(f32, f32)>,
    /// Every turn's fold opened before the replay, so tool output stands in the transcript.
    pub unfold: bool,
}

/// `desktop-gpui [--state <state.json>] [--glass] [--open <id>] [--screen <name>]`, `desktop-gpui [--state <state.json>] [--glass]
/// --bench <out.json> --bench-thread <id> [--bench-switch <id,id,...> | --bench-terminal <line> | --bench-replay <events.jsonl>
/// [--bench-pointer <x>,<y>] [--bench-unfold]]`,
/// `desktop-gpui --open <id> [--theme <id>] [--unfold] [--now <ms>] [--reveal <text>@<y>]` for a capture,
/// `desktop-gpui --open <id> [--record-scroll <events.jsonl>] [--trace <trace.json>]`, or `desktop-gpui --gallery`.
pub fn parse(mut words: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut state, mut out, mut thread, mut switch, mut open, mut glass, mut gallery, mut screen, mut terminal) =
        (None, None, None, Vec::new(), None, false, false, None, None);
    let (mut replay, mut record, mut trace) = (None, None, None);
    let (mut pointer, mut unfold) = (None, false);
    let mut capture = Capture::default();
    while let Some(flag) = words.next() {
        let mut value = || words.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--state" => state = Some(PathBuf::from(value()?)),
            "--open" => open = Some(value()?),
            "--screen" => screen = Some(value()?),
            "--bench" => out = Some(PathBuf::from(value()?)),
            "--bench-thread" => thread = Some(value()?),
            "--gallery" => gallery = true,
            "--bench-switch" => switch = value()?.split(',').filter(|id| !id.is_empty()).map(str::to_owned).collect(),
            "--bench-terminal" => terminal = Some(value()?),
            "--bench-replay" => replay = Some(PathBuf::from(value()?)),
            "--bench-pointer" => {
                let said = value()?;
                let (x, y) = said.split_once(',').ok_or("--bench-pointer takes <x>,<y>")?;
                let num = |n: &str| n.trim().parse::<f32>().map_err(|e| format!("--bench-pointer: {e}"));
                pointer = Some((num(x)?, num(y)?));
            }
            "--bench-unfold" => unfold = true,
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
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let bench = match (out, thread) {
        (Some(out), Some(thread)) if [!switch.is_empty(), terminal.is_some(), replay.is_some()].iter().filter(|x| **x).count() <= 1 => {
            Some(Bench { out, thread, switch, terminal, replay, pointer, unfold })
        }
        (Some(_), Some(_)) => return Err("--bench-switch, --bench-terminal and --bench-replay go one at a time".to_owned()),
        (None, None) if switch.is_empty() && terminal.is_none() && replay.is_none() => None,
        _ => return Err("--bench and --bench-thread go together".to_owned()),
    };
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
        return Err("--theme, --unfold, --now and --reveal go with --open".to_owned());
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
                switch: vec!["a".into(), "b".into(), "c".into()],
                terminal: None,
                replay: None,
                pointer: None,
                unfold: false
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
        assert_eq!(shot, Capture { theme: Some("graphite".into()), unfold: true, now: Some(5.), reveal: Some(("turn".into(), 189.)) });
        assert!(parse(words("--theme graphite")).is_err());
        let replay = parse(words("--bench /t.json --bench-thread real --bench-replay /e.jsonl")).unwrap().bench.unwrap();
        assert_eq!(replay.replay, Some(PathBuf::from("/e.jsonl")));
        assert!(parse(words("--bench /t.json --bench-thread real --bench-replay /e.jsonl --bench-switch a")).is_err());
        assert!(parse(words("--bench-replay /e.jsonl")).is_err());
        let over =
            parse(words("--bench /t.json --bench-thread real --bench-replay /e.jsonl --bench-pointer 380,240.5 --bench-unfold")).unwrap();
        assert_eq!(over.bench.map(|b| (b.pointer, b.unfold)), Some((Some((380., 240.5)), true)));
        assert!(parse(words("--bench /t.json --bench-thread real --bench-pointer 1,2")).is_err());
        assert!(parse(words("--bench /t.json --bench-thread real --bench-replay /e.jsonl --bench-pointer 12")).is_err());
        let recorded = parse(words("--open t --record-scroll /e.jsonl --trace /t.json")).unwrap();
        assert_eq!((recorded.record, recorded.trace), (Some(PathBuf::from("/e.jsonl")), Some(PathBuf::from("/t.json"))));
        assert!(parse(words("--record-scroll /e.jsonl")).is_err());
        assert!(parse(words("--trace /t.json")).is_err());
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
}
