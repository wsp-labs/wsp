// SPDX-License-Identifier: AGPL-3.0-only
//! Every sentence a terminal pane says about its state, as today's app words them (`adapt/terminal-pane.ts`,
//! `components/DaemonDown.tsx`, `protocol:workspace-state.ts`): the overlay's title and hints, the empty pane's line
//! and why a keystroke is refused.

use super::state::{Absent, PaneState, PauseMode};

/// SHELL_ENDED_LINE: the overlay and the scrollback of a shell whose daemon went away read this one sentence.
pub const SHELL_ENDED_LINE: &str = "This shell ended when the terminal process on its computer restarted";

/// SHELL_STOPPED_LINE: a shell open before a nap that ends every process, once the thread is back.
pub const SHELL_STOPPED_LINE: &str = "This shell ended when the thread stopped";

/// START_DAEMON_WORD, the button under a pane whose daemon this host started and lost.
pub const START_DAEMON_WORD: &str = "Start again";

/// The same button while its start is in flight.
pub const STARTING_WORD: &str = "Starting…";

/// NO_REASON in `protocol/client.ts`: a refusal the host sent no sentence with, which already asks for another try.
pub const NO_REASON: &str = "The host answered with no reason. Try again.";

/// A panel tab on a workspace this window holds no record of (`WorkspaceTerminalPanel.tsx`).
pub const NO_LINK_TITLE: &str = "No terminal link for this thread yet.";

const WORKSPACE_ROW: &str = "thread's row";

/// The line under [`NO_LINK_TITLE`]: the person's own computer by its name, else the thread's computer.
pub fn no_link_line(computer: Option<&str>) -> String {
    format!("Terminals connect once {} answers.", computer.unwrap_or("the thread's computer"))
}

pub fn rebuild_hint() -> String {
    format!("Rebuild it from the {WORKSPACE_ROW}")
}

/// `refusalLine`: what happened, closed with a full stop where it is not closed already, then what to do.
pub fn refusal_line(happened: &str, fix: &str) -> String {
    let said = happened.trim_end();
    let closed = said.ends_with(['.', '!', '?', ':']);
    format!("{said}{} {fix}", if closed { "" } else { "." })
}

/// `ownDaemonDown`: the daemon this host started for its own computer is not running.
pub fn own_daemon_down(name: &str) -> Absent {
    let said = format!("{name}'s terminals and files stopped");
    Absent { sentence: said.clone(), said, will: None, start: Some(START_DAEMON_WORD.into()) }
}

/// `absentComputer`: a computer of the person's own that is not answering.
pub fn absent_computer(name: &str) -> Absent {
    let said = format!("{name} is not answering");
    let will = "it connects on its own when it is on".to_owned();
    Absent { sentence: format!("{said}; {will}"), said, will: Some(will), start: None }
}

/// `MEMORY_NEAR_FULL` in `words/thread.ts`: the share of memory in use past which the kernel's killer is one
/// allocation away.
const MEMORY_NEAR_FULL: f64 = 0.9;

/// `MemoryReading`: what the daemon last read of the machine before its link went: bytes used of the total, and the
/// one-minute load.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Memory {
    pub used: f64,
    pub total: f64,
    pub load1: f64,
}

impl Memory {
    /// `memoryNearFull`.
    pub fn near_full(&self) -> bool {
        self.total > 0. && self.used / self.total >= MEMORY_NEAR_FULL
    }
}

/// `outOfMemoryLine`: the last figures the daemon sent, and that the work took the memory.
pub fn out_of_memory_line(m: &Memory) -> String {
    format!(
        "Out of memory ({} of {} used, load {:.1}) when the thread's computer last answered; the work there took the memory, not a \
         fault in the computer",
        fmt_bytes(m.used),
        fmt_bytes(m.total),
        (m.load1 * 10.).round() / 10.
    )
}

/// A size a provider offers (`MachineSizeOffer`): its cpus, its memory in MB and its rate.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Offer {
    pub cpu: f64,
    pub mem_mb: f64,
    pub rate_usd_per_hour: f64,
}

/// `biggerSizeLine`: the smallest offer with more memory than the machine's `mem_mb`, with its rate.
pub fn bigger_size_line(mem_mb: f64, offers: &[Offer]) -> String {
    let bigger = offers.iter().filter(|o| o.mem_mb > mem_mb).min_by(|a, b| a.mem_mb.total_cmp(&b.mem_mb));
    match bigger {
        None => "No size with more memory is offered; run less in the thread at once".into(),
        Some(o) => format!(
            "A thread on {}\u{a0}vCPU,\u{a0}{}\u{a0}GB ({}) fits more; pick it when you start the next one",
            o.cpu,
            (o.mem_mb / 1024. * 10.).round() / 10.,
            fmt_rate(o.rate_usd_per_hour)
        ),
    }
}

/// `fmtBytes`: binary units, whole under a gigabyte, GB with one decimal unless whole.
fn fmt_bytes(n: f64) -> String {
    const K: f64 = 1024.;
    if n < K {
        return format!("{n} B");
    }
    if n < K * K {
        return format!("{} KB", (n / K).round());
    }
    if n < K * K * K {
        return format!("{} MB", (n / (K * K)).round());
    }
    let gb = (n / (K * K * K) * 10.).round() / 10.;
    if gb.fract() == 0. {
        format!("{gb} GB")
    } else {
        format!("{gb:.1} GB")
    }
}

/// `fmtRate`: dollars an hour to the mil where the mil is not zero, else to the cent.
fn fmt_rate(usd_per_hour: f64) -> String {
    let mils = format!("{usd_per_hour:.3}");
    let dollars = if mils.ends_with('0') { format!("{usd_per_hour:.2}") } else { mils };
    format!("${dollars}/hr")
}

/// `workspaceWord` for the nap states: a pause where the computer keeps the memory, a stop everywhere else.
fn nap_word(pausing: bool, pause_mode: Option<PauseMode>) -> &'static str {
    match (pausing, pause_mode == Some(PauseMode::Memory)) {
        (true, true) => "Pausing",
        (false, true) => "Paused",
        (true, false) => "Stopping",
        (false, false) => "Stopped",
    }
}

fn named(computer: &Option<String>, opens: bool) -> String {
    match computer {
        Some(name) => name.clone(),
        None if opens => "The thread's computer".into(),
        None => "the thread's computer".into(),
    }
}

/// `terminalPaneTitle`: the overlay's one line; none while live.
pub fn title(pane: &PaneState) -> Option<String> {
    Some(match pane {
        PaneState::Live => return None,
        PaneState::Starting { where_, .. } => format!("Starting a terminal on {where_}"),
        PaneState::Unanswered { where_, .. } => format!("Nothing has answered on {where_}"),
        PaneState::Reconnecting { computer, .. } | PaneState::Reauth { computer } => format!("Reconnecting to {}", named(computer, false)),
        PaneState::NotAnswering { out_of_memory: Some(line), .. } => line.clone(),
        PaneState::NotAnswering { computer, .. } => format!("{} is not answering", named(computer, true)),
        PaneState::Absent(absent) => absent.said.clone(),
        PaneState::NoDaemon { computer } => format!("No terminal runs on {}", named(computer, false)),
        PaneState::Refused { computer, .. } => {
            format!("The connection to {} was refused; its threads keep running", named(computer, false))
        }
        PaneState::Paused { pausing, pause_mode } => {
            let word = nap_word(*pausing, *pause_mode);
            if *pause_mode == Some(PauseMode::Memory) {
                format!("{word}. The shell is kept; wake the thread to continue")
            } else {
                format!("{word}. The shell {} with the stop; wake the thread to open a new one", if *pausing { "ends" } else { "ended" })
            }
        }
        PaneState::Waking { pause_mode: Some(PauseMode::Memory) } => "Waking the thread. The shell continues when it is back".into(),
        PaneState::Waking { .. } => "Waking the thread. The shell ended with the stop; open a new one when it is back".into(),
        PaneState::Gone => "What this thread ran on is gone".into(),
    })
}

/// `terminalPaneHints`: the lines under the title, in order: the memory line, the next size up, the rebuild.
/// `bigger` is `biggerSizeLine` for the machine's size, where the caller holds the provider's table.
pub fn hints(pane: &PaneState, bigger: Option<&str>) -> Vec<String> {
    let bigger = bigger.map(str::to_owned);
    match pane {
        PaneState::Unanswered { local: true, where_ } => {
            vec![format!("wsp keeps trying; look at the terminal you started wsp in on {where_}")]
        }
        PaneState::Unanswered { local: false, where_ } => {
            vec![format!("wsp keeps trying; the {WORKSPACE_ROW} says what {where_} is doing")]
        }
        PaneState::Reconnecting { out_of_memory: Some(line), .. } => [Some(line.clone()), bigger].into_iter().flatten().collect(),
        PaneState::NotAnswering { out_of_memory: Some(_), .. } => [bigger, Some(rebuild_hint())].into_iter().flatten().collect(),
        PaneState::NotAnswering { .. } | PaneState::Gone => vec![rebuild_hint()],
        PaneState::Absent(absent) => absent.will.iter().cloned().collect(),
        PaneState::Refused { reason, .. } if !reason.is_empty() => vec![reason.clone()],
        _ => Vec::new(),
    }
}

/// `terminalEmptyLine`: what an empty pane says instead of offering a terminal; none while live.
pub fn empty_line(pane: &PaneState) -> Option<String> {
    Some(match pane {
        PaneState::Live => return None,
        PaneState::Starting { where_, .. } => format!("Starting a terminal on {where_}; the first one opens when it is ready"),
        PaneState::Unanswered { where_, .. } => format!("Nothing has answered on {where_}, so no terminal opens yet"),
        PaneState::Reconnecting { computer, .. } | PaneState::Reauth { computer } => {
            format!("Reconnecting to {}; terminals open when it is back", named(computer, false))
        }
        PaneState::NotAnswering { computer, .. } => format!("{} is not answering; terminals open when it does", named(computer, true)),
        PaneState::Absent(absent) if absent.start.is_some() => absent.said.clone(),
        PaneState::Absent(absent) => format!("{}, and a terminal opens then", absent.sentence),
        PaneState::NoDaemon { computer } => format!("No terminal runs on {}, so none opens here", named(computer, false)),
        PaneState::Refused { reason, .. } => format!("No terminal opens from this window: {reason}. Your threads keep running."),
        PaneState::Paused { pausing, pause_mode } => {
            format!("The thread is {}; wake it to open a terminal", nap_word(*pausing, *pause_mode).to_lowercase())
        }
        PaneState::Waking { .. } => "The thread is waking; terminals open when it is running".into(),
        PaneState::Gone => format!("What this thread ran on is gone; rebuild it from the {WORKSPACE_ROW}"),
    })
}

/// `terminalInputRefusal`: why a keystroke is not sent; none while live.
pub fn input_refusal(pane: &PaneState) -> Option<String> {
    Some(match pane {
        PaneState::Live => return None,
        PaneState::Starting { .. } => "Typing is refused while the terminal starts".into(),
        PaneState::Unanswered { where_, .. } => format!("Typing is refused: nothing has answered on {where_}"),
        PaneState::Reconnecting { computer, .. } | PaneState::Reauth { computer } => {
            format!("Typing is refused while {} reconnects", named(computer, false))
        }
        PaneState::NotAnswering { computer, .. } => format!("Typing is refused: {} is not answering", named(computer, false)),
        PaneState::Absent(absent) => format!("Typing is refused: {}", absent.said),
        PaneState::NoDaemon { computer } => format!("Typing is refused: no terminal runs on {}", named(computer, false)),
        PaneState::Refused { computer, .. } => format!("Typing is refused: the connection to {} was refused", named(computer, false)),
        PaneState::Paused { pausing, pause_mode } => {
            format!("Typing is refused: the thread is {}", nap_word(*pausing, *pause_mode).to_lowercase())
        }
        PaneState::Waking { .. } => "Typing is refused while the thread wakes".into(),
        PaneState::Gone => "Typing is refused: what this thread ran on is gone".into(),
    })
}
