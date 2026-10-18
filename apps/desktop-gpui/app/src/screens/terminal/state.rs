// SPDX-License-Identifier: AGPL-3.0-only
//! What a terminal pane says over its frozen screen (`adapt/terminal-pane.ts`): read from the workspace's state as
//! the host reports it, the reach of its daemon and how this window's channel to it stands. A shell survives a nap
//! only where the computer keeps the machine's memory; a nap anywhere else is a stop that ends every process.

use gpui_kit::SharedString;
use serde_json::Value;
use wsp_host_client::protocol::{WorkspaceOut, WorkspaceStatus};
use wsp_host_client::Failure;

use super::link::Status;
use super::words;
use crate::screens::Host;

/// The one state word's key (`WorkspaceState` in `protocol:workspace-state.ts`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WorkspaceState {
    Running,
    Pausing,
    Paused,
    Waking,
    Unreachable,
    Gone,
    Waiting,
    Released,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseMode {
    Memory,
    Disk,
}

/// `AbsentComputer`, the halves a pane refuses in while the workspace's computer is not answering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Absent {
    pub said: String,
    pub will: Option<String>,
    pub sentence: String,
    /// The button's word where this host can start what is missing.
    pub start: Option<String>,
}

/// `TerminalPaneState`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PaneState {
    Live,
    Starting { local: bool, where_: String },
    Unanswered { local: bool, where_: String },
    Reconnecting { out_of_memory: Option<String>, computer: Option<String> },
    Reauth { computer: Option<String> },
    Refused { reason: String, computer: Option<String> },
    NotAnswering { out_of_memory: Option<String>, computer: Option<String> },
    NoDaemon { computer: Option<String> },
    Absent(Absent),
    Paused { pausing: bool, pause_mode: Option<PauseMode> },
    Waking { pause_mode: Option<PauseMode> },
    Gone,
}

/// `TerminalPaneInput`: what the pane reads its state from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaneInput {
    pub state: WorkspaceState,
    /// The runtime's reach word for the workspace's daemon (`ReachState`), none before a status arrived.
    pub reach: Option<String>,
    pub socket: Status,
    /// `outOfMemoryLine` of the daemon's last reading before its link went, where memory was near full.
    pub out_of_memory: Option<String>,
    /// Whether the workspace is the person's own computer.
    pub local: bool,
    pub absent: Option<Absent>,
    /// What the sentences call where a terminal is being opened.
    pub where_: String,
    /// The person's own computer by its own name; none for a machine a cloud lends.
    pub computer: Option<String>,
    pub pause_mode: Option<PauseMode>,
}

/// `workspaceState`: the phase, then the machine's state and the daemon's reach where a status says them.
pub fn workspace_state(phase: &str, machine: Option<&str>, reach: Option<&str>) -> WorkspaceState {
    if phase == "released" {
        return WorkspaceState::Released;
    }
    if machine == Some("gone") {
        return WorkspaceState::Gone;
    }
    match phase {
        "pausing" => WorkspaceState::Pausing,
        "napping" => WorkspaceState::Paused,
        "waking" => WorkspaceState::Waking,
        "gone" => WorkspaceState::Gone,
        "queued" => WorkspaceState::Waiting,
        _ if machine == Some("paused") => WorkspaceState::Paused,
        _ if machine == Some("starting") => WorkspaceState::Waking,
        _ if matches!(reach, Some("unreachable" | "no-daemon" | "zombie")) => WorkspaceState::Unreachable,
        _ => WorkspaceState::Running,
    }
}

/// `terminalPaneState`.
pub fn pane_state(input: &PaneInput) -> PaneState {
    let from_link = || match &input.socket {
        Status::Opening => PaneState::Starting { local: input.local, where_: input.where_.clone() },
        Status::Unanswered => PaneState::Unanswered { local: input.local, where_: input.where_.clone() },
        _ => PaneState::Reconnecting { out_of_memory: input.out_of_memory.clone(), computer: input.computer.clone() },
    };
    if let Some(absent) = &input.absent {
        return PaneState::Absent(absent.clone());
    }
    match input.state {
        WorkspaceState::Gone | WorkspaceState::Released => PaneState::Gone,
        WorkspaceState::Waiting | WorkspaceState::Waking => PaneState::Waking { pause_mode: input.pause_mode },
        WorkspaceState::Pausing => PaneState::Paused { pausing: true, pause_mode: input.pause_mode },
        WorkspaceState::Paused => PaneState::Paused { pausing: false, pause_mode: input.pause_mode },
        WorkspaceState::Unreachable if input.reach.as_deref() == Some("zombie") => {
            PaneState::NotAnswering { out_of_memory: input.out_of_memory.clone(), computer: input.computer.clone() }
        }
        WorkspaceState::Unreachable => from_link(),
        WorkspaceState::Running => match &input.socket {
            Status::Live => PaneState::Live,
            Status::Refused(reason) => PaneState::Refused { reason: reason.to_string(), computer: input.computer.clone() },
            _ if input.reach.as_deref() == Some("unsupported") => PaneState::NoDaemon { computer: input.computer.clone() },
            Status::Reauth => PaneState::Reauth { computer: input.computer.clone() },
            _ => from_link(),
        },
    }
}

/// The record and status fields the pane reads, off the host's own types.
pub struct WorkspaceRead<'a> {
    pub record: &'a WorkspaceOut,
    pub status: Option<&'a WorkspaceStatus>,
    /// The person's own computer the workspace runs on, by its own name (`ownComputerName`); none for a machine a
    /// cloud lends, or before the places list holds its row.
    pub computer: Option<&'a str>,
}

fn field(value: &Value, key: &str) -> Option<String> {
    value.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// A pane's input for a workspace, as `paneOf` in `terminal/paneWords.ts` builds it, with `absenceOf` read off the
/// places list by the caller.
pub fn input_for(read: &WorkspaceRead, socket: Status, absent: Option<Absent>) -> PaneInput {
    let record = serde_json::to_value(read.record).unwrap_or(Value::Null);
    let status = read.status.and_then(|s| serde_json::to_value(s).ok());
    let view = status.as_ref().unwrap_or(&record);
    let phase = field(view, "phase").unwrap_or_else(|| "running".into());
    let machine = status.as_ref().and_then(|s| field(s, "machineState"));
    let reach = status.as_ref().and_then(|s| s.get("reach")).and_then(|r| field(r, "state"));
    let local = field(&record, "kind").as_deref() == Some("local");
    let name = field(&record, "name").unwrap_or_default();
    let computer = read.computer.map(str::to_owned);
    let where_ = computer.clone().unwrap_or(if name.is_empty() { "the thread's computer".into() } else { name });
    PaneInput {
        state: workspace_state(&phase, machine.as_deref(), reach.as_deref()),
        reach,
        socket,
        out_of_memory: None,
        local,
        absent,
        where_,
        computer,
        pause_mode: match field(&record, "pauseMode").as_deref() {
            Some("memory") => Some(PauseMode::Memory),
            Some("disk") => Some(PauseMode::Disk),
            _ => None,
        },
    }
}

/// This computer's own drawer, which reads no workspace: only its channel says how it stands.
pub fn here_input(socket: Status, here_name: &str) -> PaneInput {
    let where_ = if here_name.is_empty() { "this computer".to_owned() } else { here_name.to_owned() };
    PaneInput {
        state: WorkspaceState::Running,
        reach: None,
        socket,
        out_of_memory: None,
        local: true,
        absent: None,
        computer: (!here_name.is_empty()).then(|| here_name.to_owned()),
        where_,
        pause_mode: None,
    }
}

/// Wake, the paused overlay's button (`workspaces.wake`): the host's refusal sentence, if it refused.
pub async fn wake(host: &Host, workspace_id: &str) -> Result<(), SharedString> {
    let id = workspace_id.to_owned();
    let asked = host
        .run(move |client| async move {
            let req = serde_json::from_value(serde_json::json!({ "workspaceId": id })).map_err(|e| Failure::new(e.to_string()))?;
            client.call::<wsp_host_client::protocol::WorkspacesWakeRequest>(req).await.map(|_| ())
        })
        .await;
    asked.map_err(|f| f.message.into())
}

/// Start again, under a pane whose daemon this host started and lost (`workspaces.restartDaemon`): the line its
/// refusal shows under the button.
pub async fn restart_daemon(host: &Host, workspace_id: &str) -> Result<(), SharedString> {
    let id = workspace_id.to_owned();
    let asked = host
        .run(move |client| async move {
            let req = serde_json::from_value(serde_json::json!({ "workspaceId": id })).map_err(|e| Failure::new(e.to_string()))?;
            client.call::<wsp_host_client::protocol::WorkspacesRestartDaemonRequest>(req).await.map(|_| ())
        })
        .await;
    asked.map_err(|f| words::start_refused_line(&f.message).into())
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;
    use crate::screens::terminal::words::{empty_line, hints, input_refusal, title};

    fn input(state: WorkspaceState, socket: Status) -> PaneInput {
        PaneInput {
            state,
            reach: None,
            socket,
            out_of_memory: None,
            local: false,
            absent: None,
            where_: "box-1".into(),
            computer: Some("studio".into()),
            pause_mode: None,
        }
    }

    fn words_of(pane: &PaneState) -> (Option<String>, Vec<String>, Option<String>, Option<String>) {
        (title(pane), hints(pane, None), empty_line(pane), input_refusal(pane))
    }

    #[test]
    fn a_running_workspace_reads_its_channel() {
        assert_eq!(pane_state(&input(WorkspaceState::Running, Status::Live)), PaneState::Live);
        assert_eq!(words_of(&PaneState::Live), (None, vec![], None, None));
        let starting = pane_state(&input(WorkspaceState::Running, Status::Opening));
        assert_eq!(
            words_of(&starting),
            (
                Some("Starting a terminal on box-1".into()),
                vec![],
                Some("Starting a terminal on box-1; the first one opens when it is ready".into()),
                Some("Typing is refused while the terminal starts".into())
            )
        );
        let unanswered = pane_state(&input(WorkspaceState::Running, Status::Unanswered));
        assert_eq!(
            words_of(&unanswered),
            (
                Some("Nothing has answered on box-1".into()),
                vec!["wsp keeps trying; the thread's row says what box-1 is doing".into()],
                Some("Nothing has answered on box-1, so no terminal opens yet".into()),
                Some("Typing is refused: nothing has answered on box-1".into())
            )
        );
        let reconnecting = pane_state(&input(WorkspaceState::Running, Status::Connecting { since: Instant::now() }));
        assert_eq!(
            words_of(&reconnecting),
            (
                Some("Reconnecting to studio".into()),
                vec![],
                Some("Reconnecting to studio; terminals open when it is back".into()),
                Some("Typing is refused while studio reconnects".into())
            )
        );
        let refused = pane_state(&input(WorkspaceState::Running, Status::Refused("the door said 403".into())));
        assert_eq!(
            words_of(&refused),
            (
                Some("The connection to studio was refused; its threads keep running".into()),
                vec!["the door said 403".into()],
                Some("No terminal opens from this window: the door said 403. Your threads keep running.".into()),
                Some("Typing is refused: the connection to studio was refused".into())
            )
        );
        assert_eq!(pane_state(&input(WorkspaceState::Running, Status::Reauth)), PaneState::Reauth { computer: Some("studio".into()) });
    }

    #[test]
    fn a_local_link_that_never_answered_points_at_the_terminal_wsp_runs_in() {
        let mut local = input(WorkspaceState::Running, Status::Unanswered);
        local.local = true;
        assert_eq!(hints(&pane_state(&local), None), ["wsp keeps trying; look at the terminal you started wsp in on box-1"]);
    }

    #[test]
    fn a_nap_reads_as_a_stop_unless_the_computer_keeps_the_memory() {
        let paused = pane_state(&input(WorkspaceState::Paused, Status::Live));
        assert_eq!(title(&paused).unwrap(), "Stopped. The shell ended with the stop; wake the thread to open a new one");
        assert_eq!(empty_line(&paused).unwrap(), "The thread is stopped; wake it to open a terminal");
        assert_eq!(input_refusal(&paused).unwrap(), "Typing is refused: the thread is stopped");
        let mut pausing = input(WorkspaceState::Pausing, Status::Live);
        pausing.pause_mode = Some(PauseMode::Memory);
        let pausing = pane_state(&pausing);
        assert_eq!(title(&pausing).unwrap(), "Pausing. The shell is kept; wake the thread to continue");
        assert_eq!(input_refusal(&pausing).unwrap(), "Typing is refused: the thread is pausing");
        let waking = pane_state(&input(WorkspaceState::Waking, Status::Opening));
        assert_eq!(title(&waking).unwrap(), "Waking the thread. The shell ended with the stop; open a new one when it is back");
        assert_eq!(empty_line(&waking).unwrap(), "The thread is waking; terminals open when it is running");
    }

    #[test]
    fn gone_and_not_answering_offer_the_rebuild_last() {
        let gone = pane_state(&input(WorkspaceState::Released, Status::Live));
        assert_eq!(
            words_of(&gone),
            (
                Some("What this thread ran on is gone".into()),
                vec!["Rebuild it from the thread's row".into()],
                Some("What this thread ran on is gone; rebuild it from the thread's row".into()),
                Some("Typing is refused: what this thread ran on is gone".into())
            )
        );
        let mut zombie = input(WorkspaceState::Unreachable, Status::Opening);
        zombie.reach = Some("zombie".into());
        zombie.out_of_memory = Some("Out of memory".into());
        let zombie = pane_state(&zombie);
        assert_eq!(title(&zombie).unwrap(), "Out of memory");
        assert_eq!(
            hints(&zombie, Some("A thread on a bigger size fits more")),
            ["A thread on a bigger size fits more", "Rebuild it from the thread's row"]
        );
        let mut silent = input(WorkspaceState::Unreachable, Status::Opening);
        silent.reach = Some("zombie".into());
        silent.computer = None;
        assert_eq!(title(&pane_state(&silent)).unwrap(), "The thread's computer is not answering");
    }

    #[test]
    fn a_daemon_this_host_lost_reads_as_absent_with_its_button() {
        let mut down = input(WorkspaceState::Running, Status::Connecting { since: Instant::now() });
        down.absent = Some(words::own_daemon_down("studio"));
        let pane = pane_state(&down);
        assert_eq!(title(&pane).unwrap(), "studio's terminals and files stopped");
        assert!(hints(&pane, None).is_empty());
        assert_eq!(empty_line(&pane).unwrap(), "studio's terminals and files stopped");
        assert_eq!(input_refusal(&pane).unwrap(), "Typing is refused: studio's terminals and files stopped");
        let away = PaneState::Absent(words::absent_computer("studio"));
        assert_eq!(hints(&away, None), ["it connects on its own when it is on"]);
        assert_eq!(empty_line(&away).unwrap(), "studio is not answering; it connects on its own when it is on, and a terminal opens then");
        assert_eq!(words::start_refused_line("the daemon did not start"), "the daemon did not start. Start again.");
        assert_eq!(words::start_refused_line(words::NO_REASON), words::NO_REASON);
    }

    #[test]
    fn the_memory_line_and_the_next_size_up_read_as_todays() {
        let gib = 1024. * 1024. * 1024.;
        let full = words::Memory { used: 3.6 * gib, total: 4. * gib, load1: 1.25 };
        assert!(full.near_full());
        assert!(!words::Memory { used: 3. * gib, ..full }.near_full());
        assert_eq!(
            words::out_of_memory_line(&full),
            "Out of memory (3.6 GB of 4 GB used, load 1.3) when the thread's computer last answered; the work there took the memory, \
             not a fault in the computer"
        );
        let offers = [
            words::Offer { cpu: 4., mem_mb: 16384., rate_usd_per_hour: 0.2 },
            words::Offer { cpu: 2., mem_mb: 8192., rate_usd_per_hour: 0.125 },
            words::Offer { cpu: 1., mem_mb: 2048., rate_usd_per_hour: 0.05 },
        ];
        assert_eq!(
            words::bigger_size_line(4096., &offers),
            "A thread on 2\u{a0}vCPU,\u{a0}8\u{a0}GB ($0.125/hr) fits more; pick it when you start the next one"
        );
        assert_eq!(words::bigger_size_line(16384., &offers), "No size with more memory is offered; run less in the thread at once");
    }

    #[test]
    fn no_daemon_road_is_said_as_none() {
        let mut none = input(WorkspaceState::Running, Status::Opening);
        none.reach = Some("unsupported".into());
        assert_eq!(empty_line(&pane_state(&none)).unwrap(), "No terminal runs on studio, so none opens here");
    }

    #[test]
    fn the_state_word_reads_phase_then_machine_then_reach() {
        assert_eq!(workspace_state("running", Some("paused"), None), WorkspaceState::Paused);
        assert_eq!(workspace_state("running", Some("starting"), None), WorkspaceState::Waking);
        assert_eq!(workspace_state("running", None, Some("no-daemon")), WorkspaceState::Unreachable);
        assert_eq!(workspace_state("napping", None, None), WorkspaceState::Paused);
        assert_eq!(workspace_state("queued", None, None), WorkspaceState::Waiting);
        assert_eq!(workspace_state("running", Some("gone"), None), WorkspaceState::Gone);
        assert_eq!(workspace_state("released", Some("gone"), None), WorkspaceState::Released);
        assert_eq!(workspace_state("running", Some("running"), Some("reachable")), WorkspaceState::Running);
    }
}
