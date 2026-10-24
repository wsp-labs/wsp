// SPDX-License-Identifier: AGPL-3.0-only
//! The slate states the parity spec names, staged on a throwaway host as `parity/wave-2/states.mjs` stages them: a
//! project whose folder holds the image pieces' pictures, and per state a fresh thread with one `.slate` written on it.
// Each test crate that includes this reads only some of it.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use super::common::{root, Throwaway};

/// host-client's stand-in claude, saying the folder it runs in as its cwd, so a slate's image paths read off the
/// thread's own folder as they do on a real turn.
pub fn stand_in() -> String {
    let at = r#""cwd":"/","tools":[]}\n' "$session""#;
    assert!(super::common::STAND_IN.contains(at), "the stand-in still says its cwd as /");
    super::common::STAND_IN.replace(at, r#""cwd":"%s","tools":[]}\n' "$session" "$PWD""#)
}

pub struct State {
    pub name: &'static str,
    /// The slate file under `parity/wave-2/data/`, [`MANY`] for `many()`, or None for a thread with no slate.
    pub file: Option<&'static str>,
}

pub const STATES: &[State] = &[
    State { name: "every", file: Some("every.slate") },
    State { name: "empty-data", file: Some("empty.slate") },
    State { name: "long", file: Some("long.slate") },
    State { name: "many", file: Some(MANY) },
    State { name: "failed", file: Some("failed.slate") },
    State { name: "held", file: Some("held.slate") },
    State { name: "variants", file: Some("variants.slate") },
    State { name: "runs", file: Some("runs.slate") },
    State { name: "timers", file: Some("timers.slate") },
    State { name: "no-slate", file: None },
    State { name: "glance-decision", file: Some("glance-decision.slate") },
    State { name: "glance-stats", file: Some("glance-stats.slate") },
];

/// The state whose slate `many()` makes rather than a data file.
pub const MANY: &str = "many";

/// `manySlate()` in states.mjs: a 400-point chart, 25 bars, a 60-part treemap and a 250-row table.
pub fn many() -> String {
    let rows: Vec<String> = (0..250).map(|i| format!("{{ name: \"check-{}\", took: {} }}", i + 1, (i * 37) % 600)).collect();
    let t0: u64 = 1_792_281_600_000;
    let pts: Vec<String> = (0..400u64)
        .map(|i| format!("{{ at: {}, v: {} }}", t0 + i * 60000, (2400. + 30. * (i as f64 / 20.).sin() + (i % 7) as f64 + 0.5).floor()))
        .collect();
    let bars: Vec<String> = (0..25).map(|i| format!("{{ n: \"proc-{}\", v: {} }}", i + 1, 100 - i * 3)).collect();
    let parts: Vec<String> =
        (0..60).map(|i| format!("{{ name: \"mod-{}\", b: {}, g: \"pkg-{}\" }}", i + 1, 2000 + ((i * 7919) % 90000), i % 5)).collect();
    format!(
        r#"<slate title="Many rows">
  <column>
    <chart label="Gold, per ounce" items={{[{}]}} x={{item.at}} value={{item.v}} format="usd" />
    <bars label="Busiest" items={{[{}]}} name={{item.n}} value={{item.v}} />
    <treemap label="Bundle" items={{[{}]}} name={{item.name}} value={{item.b}} group={{item.g}} format="bytes" />
    <table items={{[{}]}} key={{item.name}}><col title="Check" value={{item.name}} /><col title="Took, s" value={{item.took}} /></table>
  </column>
</slate>
"#,
        pts.join(", "),
        bars.join(", "),
        parts.join(", "),
        rows.join(", ")
    )
}

/// `png()` in states.mjs: a smooth two-colour field, each channel truncated as a Node Buffer stores it.
pub fn png(width: u32, height: u32, from: [u8; 3], to: [u8; 3]) -> image::RgbImage {
    image::RgbImage::from_fn(width, height, |x, y| {
        let t = (x + y) as f64 / (width + height) as f64;
        let c = |i: usize| (from[i] as f64 + (to[i] as f64 - from[i] as f64) * t) as u8;
        image::Rgb([c(0), c(1), c(2)])
    })
}

/// A project `bench` whose folder holds the pictures every.slate and failed.slate name.
pub fn project(host: &Throwaway) -> PathBuf {
    let folder = host.repo_with("bench", &[(".gitignore", "shots/\n")]);
    let shots = folder.join("shots");
    std::fs::create_dir_all(&shots).unwrap();
    png(640, 360, [40, 90, 160], [230, 180, 90]).save(shots.join("plot.png")).unwrap();
    png(480, 300, [200, 60, 60], [60, 60, 200]).save(shots.join("before.png")).unwrap();
    png(480, 300, [60, 160, 90], [240, 230, 120]).save(shots.join("after.png")).unwrap();
    std::fs::write(shots.join("notes.png"), "not a picture\n").unwrap();
    host.wsp(&["add", &folder.to_string_lossy(), "--name", "bench"]);
    folder
}

/// The thread id a `wsp run` line names.
fn thread_of(said: &str) -> String {
    said.split_whitespace()
        .skip_while(|w| *w != "thread")
        .nth(1)
        .map(|w| w.trim_matches(|c: char| !c.is_ascii_hexdigit() && c != '-').to_owned())
        .unwrap_or_else(|| panic!("wsp run named no thread: {said}"))
}

/// A fresh thread on `bench` with the state's slate written on it.
pub fn staged(host: &Throwaway, _folder: &Path, state: &State) -> String {
    let said = host.wsp(&["run", "bench", &format!("Parity state {}.", state.name), "--agent", "claude"]);
    let thread = thread_of(&said);
    match state.file {
        Some(MANY) => write(host, &thread, &many()),
        Some(file) => {
            let path = root().join("apps/desktop-gpui/parity/wave-2/data").join(file);
            host.wsp(&["slate", "write", &thread, &path.to_string_lossy()]);
        }
        None => {}
    }
    thread
}

/// Writes `text` as the thread's slate.
pub fn write(host: &Throwaway, thread: &str, text: &str) {
    let file = host.dir.path().join(format!("{thread}.slate"));
    std::fs::write(&file, text).unwrap();
    host.wsp(&["slate", "write", thread, &file.to_string_lossy()]);
}

/// One step a press-driven state takes after its slate draws, as the web capture's steps do.
pub enum Act {
    /// Presses a piece by id.
    Press(&'static str),
    /// Answers the open sheet: `once`, `thread` or `refuse`.
    Answer(&'static str),
    /// Waits until a run's record says this state.
    Until(&'static str, &'static str),
    /// Waits until a run has streamed this many lines.
    Lines(&'static str, usize),
    /// Waits until a run is refreshing: started again over its last result.
    Refreshing(&'static str),
    /// The menu's Stop runs.
    Stop,
    /// Opens the slate menu with a press on its trigger.
    Menu,
    /// Escape to whatever has the keys.
    Escape,
    /// The menu's Approvals.
    Approvals,
    /// The menu's Forget secrets.
    Forget,
    /// The menu's Clear, waiting for the empty state.
    Clear,
    /// Moves the pointer to this point of the panel, in its px.
    Pointer(f32, f32),
}

/// A state reached by acts on a fresh thread, named by what it shows; its spec id is in the comment.
pub struct Pressed {
    pub name: &'static str,
    pub file: &'static str,
    pub acts: &'static [Act],
}

pub const PRESSED: &[Pressed] = &[
    // W2-SLT-060
    Pressed { name: "approval", file: "runs.slate", acts: &[Act::Press("go-quick")] },
    // W2-SLT-061
    Pressed { name: "running", file: "runs.slate", acts: &[Act::Press("go-slow"), Act::Answer("once"), Act::Lines("slow", 3)] },
    // W2-SLT-062
    Pressed { name: "done", file: "runs.slate", acts: &[Act::Press("go-quick"), Act::Answer("once"), Act::Until("quick", "done")] },
    // W2-SLT-063
    Pressed {
        name: "failed-run",
        file: "runs.slate",
        acts: &[Act::Press("go-broken"), Act::Answer("once"), Act::Until("broken", "failed")],
    },
    // W2-SLT-064
    Pressed {
        name: "cancelled",
        file: "runs.slate",
        acts: &[Act::Press("go-slow"), Act::Answer("once"), Act::Until("slow", "running"), Act::Stop, Act::Until("slow", "cancelled")],
    },
    // W2-SLT-068
    Pressed { name: "confirm", file: "runs.slate", acts: &[Act::Press("go-danger")] },
    // W2-SLT-069: the batch opens on its own.
    Pressed { name: "batch", file: "timers.slate", acts: &[] },
    // W2-SLT-070
    Pressed { name: "held-row", file: "runs.slate", acts: &[Act::Press("go-quick"), Act::Escape] },
    // W2-SLT-072
    // The pointer over the slate shows its scrollbar; on the thumb (x 535, 4 px in from the right), its hover ink.
    Pressed { name: "bar", file: "long.slate", acts: &[Act::Pointer(270., 400.)] },
    Pressed { name: "bar-hover", file: "long.slate", acts: &[Act::Pointer(270., 400.), Act::Pointer(535., 300.)] },
    Pressed {
        name: "refreshing",
        file: "timers.slate",
        acts: &[Act::Answer("thread"), Act::Until("clock", "done"), Act::Refreshing("clock")],
    },
    // W2-SLT-073
    Pressed { name: "link", file: "runs.slate", acts: &[Act::Press("good-link")] },
    // W2-SLT-074
    Pressed {
        name: "approvals",
        file: "runs.slate",
        acts: &[Act::Press("go-quick"), Act::Answer("thread"), Act::Until("quick", "done"), Act::Approvals],
    },
    // W2-SLT-075
    Pressed { name: "menu", file: "runs.slate", acts: &[Act::Menu] },
    // W2-SLT-076
    Pressed { name: "forget", file: "runs.slate", acts: &[Act::Forget] },
    // W2-SLT-078
    Pressed { name: "cleared", file: "runs.slate", acts: &[Act::Clear] },
];
