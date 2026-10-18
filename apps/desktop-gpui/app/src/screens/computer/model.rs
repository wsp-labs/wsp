// SPDX-License-Identifier: AGPL-3.0-only
//! What the Computer pane says, with no drawing in it: the two minutes of readings each computer keeps, where each
//! reading sits on its plot, the words and tones of its figures, and the words a slot holds in place of a figure
//! (`live.ts`, `units.ts`).

use std::collections::VecDeque;

use serde::Deserialize;

/// `LIVE_WINDOW`: two minutes at the daemon's two-second interval.
pub const LIVE_WINDOW: usize = 60;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub struct Usage {
    pub used: f64,
    pub total: f64,
}

/// `SysSample`, as `workspace.sys` and the daemon's `sys.sample` both carry it.
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq)]
pub struct Sample {
    pub cpu: f64,
    pub load1: f64,
    pub mem: Usage,
    pub disk: Usage,
    pub at: f64,
}

/// The newest `LIVE_WINDOW` samples of one computer, oldest first.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Ring(VecDeque<Sample>);

impl Ring {
    pub fn push(&mut self, sample: Sample) {
        if self.0.len() == LIVE_WINDOW {
            self.0.pop_front();
        }
        self.0.push_back(sample);
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn last(&self) -> Option<&Sample> {
        self.0.back()
    }

    pub fn iter(&self) -> impl Iterator<Item = &Sample> {
        self.0.iter()
    }
}

/// The three plots, in the pane's order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plot {
    Load,
    Memory,
    Disk,
}

impl Plot {
    pub const ALL: [Plot; 3] = [Plot::Load, Plot::Memory, Plot::Disk];

    pub fn label(self) -> &'static str {
        match self {
            Plot::Load => "Load",
            Plot::Memory => "Memory",
            Plot::Disk => "Disk",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Plot::Load => "load",
            Plot::Memory => "mem",
            Plot::Disk => "disk",
        }
    }

    /// The plot's name as the web's `aria-label` reads it.
    pub fn described(self) -> String {
        format!("{} over the last two minutes", self.label())
    }

    /// The figure beside the label.
    pub fn figure(self, s: &Sample) -> String {
        match self {
            Plot::Load => to_fixed(s.load1, 2),
            Plot::Memory => bytes_of_total(s.mem.used, s.mem.total),
            Plot::Disk => bytes_of_total(s.disk.used, s.disk.total),
        }
    }

    pub fn tone(self, s: &Sample) -> Tone {
        match self {
            Plot::Load => Tone::Muted,
            Plot::Memory => disk_tone(s.mem.used, s.mem.total),
            Plot::Disk => disk_tone(s.disk.used, s.disk.total),
        }
    }

    /// The sample's height as a share of the plot, 0 to 1, with load against `load_top`.
    fn share(self, s: &Sample, load_top: f64) -> f64 {
        match self {
            Plot::Load => s.load1 / load_top,
            Plot::Memory => share(s.mem),
            Plot::Disk => share(s.disk),
        }
    }

    /// `chartPoints`: sixty slots across with the newest sample in the last, so a young series grows in from the
    /// right; each point 0 to 1 across and 0 to 1 up.
    pub fn points(self, ring: &Ring) -> Vec<(f32, f32)> {
        let top = load_top(ring);
        let n = ring.len();
        ring.iter()
            .enumerate()
            .map(|(i, s)| {
                let x = (LIVE_WINDOW - n + i) as f64 / (LIVE_WINDOW - 1) as f64;
                (x as f32, self.share(s, top).clamp(0., 1.) as f32)
            })
            .collect()
    }
}

/// Load has no ceiling of its own, so its line is drawn against the highest reading in the window, never below one.
pub fn load_top(ring: &Ring) -> f64 {
    ring.iter().map(|s| s.load1).fold(1., f64::max)
}

fn share(m: Usage) -> f64 {
    if m.total > 0. {
        m.used / m.total
    } else {
        0.
    }
}

/// `SizeTone` as `diskTone` answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Muted,
    Warning,
    Danger,
}

/// `diskTone`: muted under 70 percent, the warning tone from 70, the danger tone from 90 and when there is no total.
pub fn disk_tone(used: f64, total: f64) -> Tone {
    let share = if total > 0. { used / total } else { 1. };
    if share >= 0.9 {
        Tone::Danger
    } else if share >= 0.7 {
        Tone::Warning
    } else {
        Tone::Muted
    }
}

/// JavaScript's `toFixed` on a figure that is not negative: the nearest, and the larger of two as near.
pub fn to_fixed(x: f64, places: usize) -> String {
    let scale = 10f64.powi(places as i32);
    let scaled = x * scale;
    if scaled.fract() == 0.5 {
        return format!("{:.*}", places, scaled.ceil() / scale);
    }
    format!("{x:.places$}")
}

/// A count as JavaScript writes a number: no decimals on a whole one.
fn js_number(n: f64) -> String {
    if n.fract() == 0. {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

/// `fmtBytes`: binary units, whole under a gigabyte, GB with one decimal unless whole.
pub fn fmt_bytes(n: f64) -> String {
    const K: f64 = 1024.;
    if n < K {
        return format!("{} B", js_number(n));
    }
    if n < K * K {
        return format!("{} KB", (n / K).round());
    }
    if n < K * K * K {
        return format!("{} MB", (n / (K * K)).round());
    }
    let gb = n / (K * K * K);
    let tenth = to_fixed(gb, 1);
    if tenth.ends_with(".0") {
        format!("{} GB", gb.round())
    } else {
        format!("{tenth} GB")
    }
}

/// `fmtBytesOfTotal`: "4.9 GB of 20 GB".
pub fn bytes_of_total(used: f64, total: f64) -> String {
    format!("{} of {}", fmt_bytes(used), fmt_bytes(total))
}

/// `fmtMemGb`: a size's memory, whole when whole, else one decimal.
pub fn fmt_mem_gb(mem_mb: f64) -> String {
    let gb: f64 = to_fixed(mem_mb / 1024., 1).parse().unwrap_or(0.);
    format!("{} GB", js_number(gb))
}

/// `fmtCpus`: "1 core", "2 cores", and a provider's vCPU at any count.
pub fn fmt_cpus(n: f64, cpu: &str) -> String {
    let word = if cpu == "cores" && n == 1. { "core" } else { cpu };
    format!("{} {word}", js_number(n))
}

/// `StaleWord` and `AwayWord`: what a figure's slot holds in place of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stale {
    Paused,
    Waiting,
    Deleted,
    Unreachable,
    NoAnswer,
    Stopped,
}

impl Stale {
    pub fn word(self) -> &'static str {
        match self {
            Stale::Paused => "paused",
            Stale::Waiting => "waiting",
            Stale::Deleted => "deleted",
            Stale::Unreachable => "unreachable",
            Stale::NoAnswer => "no answer",
            Stale::Stopped => "stopped",
        }
    }
}

/// `staleWord`: paused or pausing, a sub-machine not made yet or deleted, else unreachable while the readings are not
/// arriving.
pub fn stale_word(phase: &str, live: bool) -> Option<Stale> {
    match crate::screens::sidebar::menu::workspace_state(&serde_json::json!({ "phase": phase }), None) {
        "paused" | "pausing" => Some(Stale::Paused),
        "waiting" => Some(Stale::Waiting),
        "released" => Some(Stale::Deleted),
        _ if live => None,
        _ => Some(Stale::Unreachable),
    }
}

/// What each plot's slot says in place of a figure, the stale word first, then a refused watch, then no sample yet.
pub fn slot_word(stale: Option<Stale>, unavailable: bool, ring: &Ring) -> Option<&'static str> {
    match stale {
        Some(stale) => Some(stale.word()),
        None if unavailable => Some("unavailable"),
        None if ring.is_empty() => Some("pending"),
        None => None,
    }
}

/// `ReadingRoad` of the metrics: the host reads this computer itself; every other kind's own daemon reads it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Road {
    Host,
    Daemon,
}

/// `readingRoad(kind, "metrics")`; a record with no kind is a provider's fork.
pub fn metrics_road(kind: Option<&str>) -> Road {
    if kind == Some("local") {
        Road::Host
    } else {
        Road::Daemon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(load1: f64, mem: (f64, f64), disk: (f64, f64)) -> Sample {
        Sample { cpu: 0., load1, mem: Usage { used: mem.0, total: mem.1 }, disk: Usage { used: disk.0, total: disk.1 }, at: 0. }
    }

    #[test]
    fn the_ring_keeps_the_newest_sixty() {
        let mut ring = Ring::default();
        for i in 0..75 {
            ring.push(Sample { at: i as f64, ..Sample::default() });
        }
        assert_eq!(ring.len(), LIVE_WINDOW);
        assert_eq!(ring.iter().next().unwrap().at, 15.);
        assert_eq!(ring.last().unwrap().at, 74.);
    }

    #[test]
    fn a_young_series_grows_in_from_the_right() {
        let mut ring = Ring::default();
        for _ in 0..3 {
            ring.push(sample(0.5, (1., 2.), (1., 4.)));
        }
        let points = Plot::Memory.points(&ring);
        assert_eq!(points.len(), 3);
        assert_eq!(points[2].0, 1.);
        assert!((points[0].0 - 57. / 59.).abs() < 1e-6);
        assert_eq!(points[0].1, 0.5);
        assert_eq!(Plot::Disk.points(&ring)[0].1, 0.25);
    }

    #[test]
    fn load_is_drawn_against_the_window_peak_and_never_below_one() {
        let mut ring = Ring::default();
        ring.push(sample(0.25, (0., 1.), (0., 1.)));
        ring.push(sample(0.5, (0., 1.), (0., 1.)));
        assert_eq!(load_top(&ring), 1.);
        assert_eq!(Plot::Load.points(&ring)[1].1, 0.5);
        ring.push(sample(4., (0., 1.), (0., 1.)));
        assert_eq!(load_top(&ring), 4.);
        let points = Plot::Load.points(&ring);
        assert_eq!((points[0].1, points[2].1), (0.0625, 1.));
    }

    #[test]
    fn the_figures_read_as_the_web_writes_them() {
        let gib = 1024. * 1024. * 1024.;
        let s = sample(1.125, (4.9 * gib, 16. * gib), (512. * 1024. * 1024., 2.25 * gib));
        assert_eq!(Plot::Load.figure(&s), "1.13");
        assert_eq!(Plot::Memory.figure(&s), "4.9 GB of 16 GB");
        assert_eq!(Plot::Disk.figure(&s), "512 MB of 2.3 GB");
        assert_eq!(fmt_bytes(900.), "900 B");
        assert_eq!(fmt_bytes(2048.), "2 KB");
        assert_eq!(fmt_bytes(1.99 * gib), "2 GB");
        assert_eq!(fmt_mem_gb(16384.), "16 GB");
        assert_eq!(fmt_mem_gb(1536.), "1.5 GB");
        assert_eq!(
            (fmt_cpus(1., "cores"), fmt_cpus(10., "cores"), fmt_cpus(2., "vCPU")),
            ("1 core".into(), "10 cores".into(), "2 vCPU".into())
        );
    }

    #[test]
    fn memory_and_disk_take_the_caution_inks_from_seventy_and_ninety_percent() {
        assert_eq!(disk_tone(69., 100.), Tone::Muted);
        assert_eq!(disk_tone(70., 100.), Tone::Warning);
        assert_eq!(disk_tone(89.9, 100.), Tone::Warning);
        assert_eq!(disk_tone(90., 100.), Tone::Danger);
        assert_eq!(disk_tone(0., 0.), Tone::Danger);
        let s = sample(9., (95., 100.), (75., 100.));
        assert_eq!((Plot::Load.tone(&s), Plot::Memory.tone(&s), Plot::Disk.tone(&s)), (Tone::Muted, Tone::Danger, Tone::Warning));
    }

    #[test]
    fn the_slot_says_why_there_is_no_figure() {
        assert_eq!(stale_word("napping", true), Some(Stale::Paused));
        assert_eq!(stale_word("pausing", true), Some(Stale::Paused));
        assert_eq!(stale_word("queued", true), Some(Stale::Waiting));
        assert_eq!(stale_word("released", true), Some(Stale::Deleted));
        assert_eq!(stale_word("running", false), Some(Stale::Unreachable));
        assert_eq!(stale_word("running", true), None);
        let mut ring = Ring::default();
        assert_eq!(slot_word(None, false, &ring), Some("pending"));
        assert_eq!(slot_word(None, true, &ring), Some("unavailable"));
        ring.push(Sample::default());
        assert_eq!(slot_word(None, false, &ring), None);
        assert_eq!(slot_word(Some(Stale::NoAnswer), true, &ring), Some("no answer"));
        assert_eq!(slot_word(Some(Stale::Stopped), false, &ring), Some("stopped"));
    }
}
