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
    /// A screen to open at launch by its name, as captures of one screen start.
    pub screen: Option<String>,
}

#[derive(Debug, PartialEq)]
pub struct Bench {
    pub out: PathBuf,
    pub thread: String,
    pub switch: Vec<String>,
    /// A line typed into the thread's terminal drawer, in place of the scroll: the frames its output draws are timed.
    pub terminal: Option<String>,
}

/// `desktop-gpui [--state <state.json>] [--glass] [--screen <name>] [--open <id> | --bench <out.json> --bench-thread <id> [--bench-switch <id,id,...> | --bench-terminal <line>]]`,
/// or `desktop-gpui --gallery`.
pub fn parse(mut words: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut state, mut out, mut thread, mut switch, mut open, mut glass, mut gallery, mut screen, mut terminal) =
        (None, None, None, Vec::new(), None, false, false, None, None);
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
            "--glass" => glass = true,
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let bench = match (out, thread) {
        (Some(out), Some(thread)) if switch.is_empty() || terminal.is_none() => Some(Bench { out, thread, switch, terminal }),
        (Some(_), Some(_)) => return Err("--bench-switch and --bench-terminal do not go together".to_owned()),
        (None, None) if switch.is_empty() && terminal.is_none() => None,
        _ => return Err("--bench and --bench-thread go together".to_owned()),
    };
    if bench.is_some() && open.is_some() {
        return Err("--open and --bench do not go together".to_owned());
    }
    if gallery && (bench.is_some() || open.is_some() || glass) {
        return Err("--gallery goes alone".to_owned());
    }
    if screen.is_some() && (bench.is_some() || open.is_some() || gallery) {
        return Err("--screen goes without --open, --bench and --gallery".to_owned());
    }
    Ok(Args { state, bench, open, glass, gallery, screen })
}

/// The result JSON: frame times are milliseconds from the start of the scroll.
pub fn result(thread_rows: usize, frame_ms: &[f64], switch: &[(String, f64)]) -> serde_json::Value {
    serde_json::json!({
        "app": "gpui",
        "thread_rows": thread_rows,
        "scroll": { "px_per_s": PX_PER_S, "frame_ms": frame_ms },
        "switch": switch.iter().map(|(thread, ms)| serde_json::json!({ "thread": thread, "ms": ms })).collect::<Vec<_>>(),
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
            Some(Bench { out: "/o.json".into(), thread: "long".into(), switch: vec!["a".into(), "b".into(), "c".into()], terminal: None })
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
        assert!(parse(words("--screen settings --open long")).is_err());
    }

    #[test]
    fn writes_the_shape_both_apps_share() {
        let json = result(3, &[0.0, 16.7], &[("a".to_owned(), 41.0)]);
        assert_eq!(
            json,
            serde_json::json!({"app": "gpui", "thread_rows": 3, "scroll": {"px_per_s": 2400.0, "frame_ms": [0.0, 16.7]}, "switch": [{"thread": "a", "ms": 41.0}]})
        );
    }
}
