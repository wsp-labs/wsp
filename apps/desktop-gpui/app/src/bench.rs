// SPDX-License-Identifier: AGPL-3.0-only
//! Bench mode's flags and its result, in the shape the Electron bench writes too.

use std::path::PathBuf;

/// The scroll rate both apps' benches run at.
pub const PX_PER_S: f32 = 2400.;

#[derive(Debug, Default, PartialEq)]
pub struct Args {
    pub state: Option<PathBuf>,
    pub bench: Option<Bench>,
    /// A thread to open at launch and keep open, as memory with that thread open is read.
    pub open: Option<String>,
    /// The gallery of shared pieces in place of the app.
    pub gallery: bool,
}

#[derive(Debug, PartialEq)]
pub struct Bench {
    pub out: PathBuf,
    pub thread: String,
    pub switch: Vec<String>,
}

/// `desktop-gpui [--state <state.json>] [--open <id> | --bench <out.json> --bench-thread <id> [--bench-switch <id,id,...>]]`,
/// or `desktop-gpui --gallery`.
pub fn parse(mut words: impl Iterator<Item = String>) -> Result<Args, String> {
    let (mut state, mut out, mut thread, mut switch, mut open, mut gallery) = (None, None, None, Vec::new(), None, false);
    while let Some(flag) = words.next() {
        let mut value = || words.next().ok_or(format!("{flag} takes a value"));
        match flag.as_str() {
            "--state" => state = Some(PathBuf::from(value()?)),
            "--open" => open = Some(value()?),
            "--bench" => out = Some(PathBuf::from(value()?)),
            "--bench-thread" => thread = Some(value()?),
            "--gallery" => gallery = true,
            "--bench-switch" => switch = value()?.split(',').filter(|id| !id.is_empty()).map(str::to_owned).collect(),
            _ => return Err(format!("unknown flag {flag}")),
        }
    }
    let bench = match (out, thread) {
        (Some(out), Some(thread)) => Some(Bench { out, thread, switch }),
        (None, None) if switch.is_empty() => None,
        _ => return Err("--bench and --bench-thread go together".to_owned()),
    };
    if bench.is_some() && open.is_some() {
        return Err("--open and --bench do not go together".to_owned());
    }
    if gallery && (bench.is_some() || open.is_some()) {
        return Err("--gallery goes alone".to_owned());
    }
    Ok(Args { state, bench, open, gallery })
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
            Some(Bench { out: "/o.json".into(), thread: "long".into(), switch: vec!["a".into(), "b".into(), "c".into()] })
        );
        assert_eq!(parse(words("--state /s")).unwrap().bench, None);
        assert!(parse(words("--bench /o.json")).is_err());
        assert!(parse(words("--bench-thread")).is_err());
        assert_eq!(parse(words("--state /s --open long")).unwrap().open, Some("long".to_owned()));
        assert!(parse(words("--open long --bench /o.json --bench-thread long")).is_err());
        assert!(parse(words("--gallery")).unwrap().gallery);
        assert!(parse(words("--gallery --open long")).is_err());
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
