// SPDX-License-Identifier: AGPL-3.0-only
//! The state word a workspace reads as, the words each kind of workspace takes, what a delete takes, what a
//! computer that is not answering says, and the refusals of a send or an action in a state. Port of the functions
//! of packages/protocol/src/workspace-state.ts at a3b870a6e; its flat constants are
//! wsp_wire::types::workspace_state. See spec/workspace_state.md.

use wsp_wire::types::changes::Checkout;
use wsp_wire::views::harness::HarnessCatalog;
use wsp_wire::wire::capabilities::PauseMode;
use wsp_wire::views::place::PlaceBack;
use wsp_wire::views::workspace::{MachineState, ReachState};
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::harness::ScreenCommand;
use wsp_wire::views::workspace::WorkspacePhase;
use wsp_wire::views::workspace::WorkspaceStatus;
use wsp_wire::views::workspace::WorkspaceView;

use serde::Serialize;
use wsp_wire::types::app_ports::LOOPBACK;
use wsp_wire::types::workspace_state::{BACK_OVER_SSH, CLOUD_SIGN_IN_ROAD, COMPUTER_LEFT, FORGET_NEEDS_GONE, SEND_BLOCK_WORDS, START_DAEMON_WORD};
use wsp_wire::views::harness::ScreenControl;

use crate::app_ports::authority;
use crate::js::number_string;
use crate::words::base::fmt_threads;
use crate::words::computer::{computer_word, JOINED_COMPUTER, MACHINE_WSP_FORKS, THIS_COMPUTER};
use crate::words::places::offline_for;
use crate::words::units::{CpuWord, LIST_PRICE_WORD};

// workspace-state.ts:11
// export type WorkspaceState = "running" | "pausing" | "paused" | "waking" | "unreachable" | "gone" | "waiting" | "released";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum WorkspaceState {
    #[serde(rename = "running")]
    Running,
    #[serde(rename = "pausing")]
    Pausing,
    #[serde(rename = "paused")]
    Paused,
    #[serde(rename = "waking")]
    Waking,
    #[serde(rename = "unreachable")]
    Unreachable,
    #[serde(rename = "gone")]
    Gone,
    #[serde(rename = "waiting")]
    Waiting,
    #[serde(rename = "released")]
    Released,
}

impl WorkspaceState {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            WorkspaceState::Running => "running",
            WorkspaceState::Pausing => "pausing",
            WorkspaceState::Paused => "paused",
            WorkspaceState::Waking => "waking",
            WorkspaceState::Unreachable => "unreachable",
            WorkspaceState::Gone => "gone",
            WorkspaceState::Waiting => "waiting",
            WorkspaceState::Released => "released",
        }
    }
}

/// A workspace's kind as every client must read it: a record written before local workspaces existed carries none
/// and is a provider fork. The one place an absent kind is resolved.
// workspace-state.ts:15
// export function workspaceKind(view: Pick<WorkspaceView, "kind">): WorkspaceKind {
//   return view.kind ?? "cloud";
// }
/// `kind` is the view's own field, None where the record carries none.
pub fn workspace_kind(kind: Option<&WorkspaceKind>) -> WorkspaceKind {
    kind.copied().unwrap_or(WorkspaceKind::Cloud)
}

/// Whether this workspace is this computer. Asked by every surface that offers to make one (the app's sidebar and
/// palette, wsp init's workspace step), and written once so the two cannot disagree about what counts.
// workspace-state.ts:21
// export const isLocalWorkspace = (view: Pick<WorkspaceView, "kind">): boolean => workspaceKind(view) === "local";
pub fn is_local_workspace(kind: Option<&WorkspaceKind>) -> bool {
    workspace_kind(kind) == WorkspaceKind::Local
}

/// The word a turn's cost figure carries, off the workspace it ran on: a turn on a computer of the person's own ran
/// on their own sign-in, so nobody is billed for it and the number is the agent's own list price; a workspace wsp
/// forked is billed by its provider and its figure stands alone. Nothing where the caller does not know which
/// workspace it was. The one reading, so the terminal's last line and the app's chat footer cannot part again.
// workspace-state.ts:27
// export const turnSpendWord = (view: Pick<WorkspaceView, "kind"> | null | undefined): string | undefined =>
//   view !== null && view !== undefined && isLocalWorkspace(view) ? LIST_PRICE_WORD : undefined;
/// `view` None is the TS null or undefined; `Some(kind)` carries the view's own `kind` field.
pub fn turn_spend_word(view: Option<Option<&WorkspaceKind>>) -> Option<String> {
    match view {
        Some(kind) if is_local_workspace(kind) => Some(LIST_PRICE_WORD.to_string()),
        _ => None,
    }
}

/// What a workspace's kind changes about the words a client shows for it.
// workspace-state.ts:31
// export interface WorkspaceKindWords {
//   /** What a sentence calls a machine of this kind, read through machineWord: every refusal that names the machine,
//    * the line a recorded machine is announced with and the notice a second record on one is refused with. Every kind
//    * has its own, so no sentence about one kind's machine borrows another kind's words. */
//   machine: string;
//   /** Whether the slot a row gives to what the machine is reads a fact off that machine: the size its status carries
//    * in this kind's own word for a cpu on the sidebar row and the Spaces header, the provider's id in the command
//    * line's table. False for a kind wsp holds no such fact about, whose rows print the word above there instead. */
//   rowReadsMachine: boolean;
//   /** What this kind calls one of its cpus in that size line: a provider's are virtual, a machine that exists has cores. */
//   cpu: CpuWord;
//   /** What a row calls where a workspace of this kind runs, in the lowercase a row's mono reads it. A fork's own
//    * record names the provider it runs at, since this host may be wired to any of them, so the kind's word is only
//    * what a row says when that record carries none (one written before the provider rode the wire): the kind
//    * itself, never the id a provider minted. Null where the kind holds neither, and the name wsp has for the
//    * machine stands, which on those kinds is a login or a name somebody gave it. */
//   where: string | null;
//   /** Whether wsp forks this machine, pauses it, wakes it, resizes it and pays for it by the hour, or it is a machine
//    * that already exists and simply runs while the host does. The state word beside the name, the state dot, the
//    * spend, the rate, the nap countdown, the usage chart and the pause and upgrade buttons all ride this. */
//   driven: boolean;
//   /** Whether a machine of this kind ever serves a daemon, which is what the terminal, files and ports ride. False
//    * says the reach word `unsupported` is this kind's steady state rather than something missing from one machine,
//    * so a row says what the machine is instead of that its daemon is not there. True on a kind whose machines take
//    * one, even before one has landed on a given machine: what a machine without one yet says is its own reach word. */
//   daemon: boolean;
//   /** Where the Machine tab's Live rows are read for a machine of this kind: off the daemon on that machine, or in
//    * the host process for the computer the host runs on. False says those rows read that the reading is not on this
//    * kind, rather than waiting on a sample that never comes. */
//   metrics: ReadingRoad;
//   /** Where the Processes tab's list is read, which the same rule holds for. */
//   processes: ReadingRoad;
//   /** Which kinds of project source a workspace of this kind can hold: this computer copies a folder of the person's
//    * own, a machine wsp forks clones a repo into itself, and a machine wsp only reaches takes neither yet.
//    * The one table every road that records a project or makes a workspace reads, so no road decides for itself
//    * what a computer can be given. */
//   projectSources: readonly ProjectSource["kind"][];
//   /** Whether a workspace of this kind is a copy of a folder on this computer, made by directory and forking no
//    * image: this computer alone. Its own word and not a reading of the sources above, since a computer that clones
//    * also takes a folder on this computer as a source, seeded onto the clone, and would otherwise read as one. */
//   copiesFolder: boolean;
//   /** Whether a thread of this kind runs in its project's folder on the computer holding it, so a start makes nothing:
//    * no machine, no copy, no name of its own, and every thread of the folder shares one record. This computer, and a
//    * computer the person joined, whose folder is reached over its link. */
//   inFolder: boolean;
//   /** Whether the agents on a machine of this kind could drive this host at all, which is what says the spawn switch
//    * means anything there. A fork's agents reach the host over the road a scoped token opens, and this computer's
//    * over its loopback with a token of the same scope; a machine somebody already owns is handed no wsp to drive
//    * one with, so there the switch would hand out a token that opens nothing. */
//   agents: boolean;
//   /** What a delete does to a machine of this kind, which is never the same sentence twice: wsp takes away the
//    * machine it forked, leaves this computer alone, and on a machine somebody owns takes off what it put there and
//    * leaves the machine standing. */
//   onDelete: MachineOnDelete;
// }
/// One kind's row of WORKSPACE_KIND_WORDS. `where_` is the TS `where`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceKindWords {
    pub machine: &'static str,
    pub row_reads_machine: bool,
    pub cpu: CpuWord,
    pub where_: Option<&'static str>,
    pub driven: bool,
    pub daemon: bool,
    pub metrics: ReadingRoad,
    pub processes: ReadingRoad,
    /// `ProjectSource["kind"]` values, in the TS order.
    pub project_sources: &'static [&'static str],
    pub copies_folder: bool,
    pub in_folder: bool,
    pub agents: bool,
    pub on_delete: MachineOnDelete,
}

/// What a delete does to a machine, in the two moods the two sentences need: the clause the confirmation asks
/// under "Its", and the clause the line after the delete states, which is given the machine's id since the only
/// kind whose machine goes is worth naming. Both read one entry, so a kind's words cannot say two things.
// workspace-state.ts:90
// export interface MachineOnDelete {
//   asked: string;
//   done(machineId: string): string;
// }
/// The TS `done(machineId)` closure is `done` with `{machineId}` where the sentence names the id; read it through
/// `done_for`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MachineOnDelete {
    pub asked: String,
    pub done: String,
}

impl MachineOnDelete {
    /// The TS `done(machineId)`.
    pub fn done_for(&self, machine_id: &str) -> String {
        self.done.replacen(MACHINE_ID_SLOT, machine_id, 1)
    }
}

/// Where a kind's `done` names the machine's id. A NUL, since no name, path or id wsp holds can carry one.
const MACHINE_ID_SLOT: char = '\0';

// workspace-state.ts:97
// in wsp_wire::types::workspace_state: COMPUTER_LEFT

/// What a row calls a fork whose record names no provider: what it is, since the id the provider minted for the
/// machine names nothing to the person reading the row.
// workspace-state.ts:101
const A_PROVIDER: &str = "a provider";

/// The two readings a pane waits on. Each is one module per kind: the cloud kind reads that machine's own /proc
/// through the daemon on it, and this computer reads its own host.
// workspace-state.ts:105
// export type KindReading = "metrics" | "processes";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum KindReading {
    #[serde(rename = "metrics")]
    Metrics,
    #[serde(rename = "processes")]
    Processes,
}

impl KindReading {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            KindReading::Metrics => "metrics",
            KindReading::Processes => "processes",
        }
    }
}

/// Who reads one of those two for a machine of this kind. `daemon` is the machine answering for itself over the
/// link a pane holds. `host` is the host process reading the computer it runs on, which is the whole machine there:
/// the reading then stands whether or not that computer's daemon is up, since nothing about cpu, memory or disk
/// needs a port, a token or a pty. False is a kind that reads it on no road.
// workspace-state.ts:111
// export type ReadingRoad = "daemon" | "host" | false;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ReadingRoad {
    Daemon,
    Host,
    /// The TS `false`: this kind reads it on no road.
    Off,
}

/// The words per kind, the one table every client reads instead of comparing a kind itself. Adding a kind is a row
/// here.
// workspace-state.ts:115
// export const WORKSPACE_KIND_WORDS: Record<WorkspaceKind, WorkspaceKindWords> = {
//   cloud: { machine: MACHINE_WSP_FORKS, rowReadsMachine: true, cpu: "vCPU", where: A_PROVIDER, driven: true, daemon: true, metrics: "daemon", processes: "daemon", projectSources: ["git", "github", "gitlab", "folder"], copiesFolder: false, inFolder: false, agents: true, onDelete: { asked: "computer is deleted in the cloud", done: machineId => `computer ${machineId} is gone in the cloud` } },
//   local: { machine: THIS_COMPUTER, rowReadsMachine: true, cpu: "cores", where: THIS_COMPUTER, driven: false, daemon: true, metrics: "host", processes: "daemon", projectSources: ["git", "github", "gitlab", "folder"], copiesFolder: true, inFolder: true, agents: true, onDelete: { asked: COMPUTER_LEFT, done: () => `its ${COMPUTER_LEFT}` } },
//   place: { machine: JOINED_COMPUTER, rowReadsMachine: false, cpu: "cores", where: null, driven: false, daemon: true, metrics: "daemon", processes: "daemon", projectSources: ["git", "github", "gitlab", "folder"], copiesFolder: false, inFolder: true, agents: true, onDelete: { asked: COMPUTER_LEFT, done: () => `its ${COMPUTER_LEFT}` } },
// };
// Read through `kind_words`: a row holds a String, so the table is not a constant.

// workspace-state.ts:121
// export function kindWords(kind: WorkspaceKind): WorkspaceKindWords {
//   return WORKSPACE_KIND_WORDS[kind];
// }
pub fn kind_words(kind: &WorkspaceKind) -> WorkspaceKindWords {
    const PROJECT_SOURCES: &[&str] = &["git", "github", "gitlab", "folder"];
    let left = || MachineOnDelete { asked: COMPUTER_LEFT.to_string(), done: format!("its {COMPUTER_LEFT}") };
    match kind {
        WorkspaceKind::Cloud => WorkspaceKindWords {
            machine: MACHINE_WSP_FORKS,
            row_reads_machine: true,
            cpu: CpuWord::VCPU,
            where_: Some(A_PROVIDER),
            driven: true,
            daemon: true,
            metrics: ReadingRoad::Daemon,
            processes: ReadingRoad::Daemon,
            project_sources: PROJECT_SOURCES,
            copies_folder: false,
            in_folder: false,
            agents: true,
            on_delete: MachineOnDelete {
                asked: "computer is deleted in the cloud".to_string(),
                done: format!("computer {MACHINE_ID_SLOT} is gone in the cloud"),
            },
        },
        WorkspaceKind::Local => WorkspaceKindWords {
            machine: THIS_COMPUTER,
            row_reads_machine: true,
            cpu: CpuWord::Cores,
            where_: Some(THIS_COMPUTER),
            driven: false,
            daemon: true,
            metrics: ReadingRoad::Host,
            processes: ReadingRoad::Daemon,
            project_sources: PROJECT_SOURCES,
            copies_folder: true,
            in_folder: true,
            agents: true,
            on_delete: left(),
        },
        WorkspaceKind::Place => WorkspaceKindWords {
            machine: JOINED_COMPUTER,
            row_reads_machine: false,
            cpu: CpuWord::Cores,
            where_: None,
            driven: false,
            daemon: true,
            metrics: ReadingRoad::Daemon,
            processes: ReadingRoad::Daemon,
            project_sources: PROJECT_SOURCES,
            copies_folder: false,
            in_folder: true,
            agents: true,
            on_delete: left(),
        },
    }
}

/// Whether a machine of this kind answers this reading at all. The one question a pane asks before it waits: a kind
/// that serves it shows pending until the first value lands, and a kind that serves it on no road says so at once,
/// so no slot sits at pending for a stream that will never come.
// workspace-state.ts:128
// export function servesReading(kind: WorkspaceKind, reading: KindReading): boolean {
//   return readingRoad(kind, reading) !== false;
// }
pub fn serves_reading(kind: &WorkspaceKind, reading: &KindReading) -> bool {
    reading_road(kind, reading) != ReadingRoad::Off
}

/// Who answers this reading for a machine of this kind: the one table both the pane that asks for it and the host
/// that serves it read, so neither can decide for itself where a kind's figures come from.
// workspace-state.ts:134
// export function readingRoad(kind: WorkspaceKind, reading: KindReading): ReadingRoad {
//   return WORKSPACE_KIND_WORDS[kind][reading];
// }
pub fn reading_road(kind: &WorkspaceKind, reading: &KindReading) -> ReadingRoad {
    let words = kind_words(kind);
    match reading {
        KindReading::Metrics => words.metrics,
        KindReading::Processes => words.processes,
    }
}

/// Whether the spawn switch means anything on a workspace of this kind, the one reading both doors that set it
/// take: the create that names it on a new workspace and the verb that turns it on for one that exists.
// workspace-state.ts:140
// export function agentsMayDrive(kind: WorkspaceKind): boolean {
//   return WORKSPACE_KIND_WORDS[kind].agents;
// }
pub fn agents_may_drive(kind: &WorkspaceKind) -> bool {
    kind_words(kind).agents
}

/// The one sentence both those doors refuse with, so a person reads the same thing whichever they typed.
// workspace-state.ts:145
// export function agentsKindRefusal(kind: WorkspaceKind): string {
//   return `agents on ${machineWord(kind)} cannot drive this host, so the spawn switch would hand out a token that opens nothing; it is for the machines wsp forks and this computer`;
// }
pub fn agents_kind_refusal(kind: &WorkspaceKind) -> String {
    format!(
        "agents on {} cannot drive this host, so the spawn switch would hand out a token that opens nothing; it is for the machines wsp forks and this computer",
        machine_word(kind)
    )
}

/// What a sentence calls this workspace's machine. Every kind answers for itself: a refusal on a machine wsp forks
/// says so, and this computer's words are this computer's alone.
// workspace-state.ts:151
// export function machineWord(kind: WorkspaceKind): string {
//   return WORKSPACE_KIND_WORDS[kind].machine;
// }
pub fn machine_word(kind: &WorkspaceKind) -> String {
    kind_words(kind).machine.to_string()
}

/// Where a thread runs, as a sentence to that thread names it: the computer by its own name where the kind's threads
/// run in a project's folder, else the kind's machine.
// workspace-state.ts:157
// export function threadPlace(kind: WorkspaceKind, computer: string): string {
//   return WORKSPACE_KIND_WORDS[kind].inFolder ? computer : machineWord(kind);
// }
pub fn thread_place(kind: &WorkspaceKind, computer: &str) -> String {
    if kind_words(kind).in_folder {
        computer.to_string()
    } else {
        machine_word(kind)
    }
}

/// What deleting a workspace takes, the one sentence every client's confirmation shows: what the delete does to
/// this kind's machine, in that kind's own words, and either way the record and the threads go from here. It sits
/// with the kind table rather than with the other notices, since the machine half is a kind's word.
// workspace-state.ts:164
// export function deleteNotice(threads: number, kind: WorkspaceKind, copy?: { path: string }, machineId?: string, on?: StandsOn): string {
//   return `Its ${onDeleteOf(kind, copy, machineId, on).asked}; its record and ${fmtThreads(threads)} leave this computer.`;
// }
pub fn delete_notice(threads: f64, kind: &WorkspaceKind, copy: Option<&CopyAt>, machine_id: Option<&str>, on: Option<&StandsOn>) -> String {
    format!("Its {}; its record and {} leave this computer.", on_delete_of(kind, copy, machine_id, on).asked, fmt_threads(threads))
}

/// One of the copies Keep this one deletes, as its row names it and as its kind decides what a delete does to it.
// workspace-state.ts:169
// export interface CopyToDelete {
//   name: string;
//   kind: WorkspaceKind;
//   copy?: { path: string };
//   machineId?: string;
//   /** The computer somebody joined that the copy stands on, which its sentence names. */
//   on?: StandsOn;
// }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyToDelete {
    pub name: String,
    pub kind: WorkspaceKind,
    pub copy: Option<CopyAt>,
    pub machine_id: Option<String>,
    pub on: Option<StandsOn>,
}

/// The TS `{ path: string }` of a wsp worktree's record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CopyAt {
    pub path: String,
}

/// What deleting the other copies one send to several models made takes, the sentence Keep this one confirms with:
/// what goes on each copy's computer in its own kind's words, one sentence where they all go the same way, then their
/// records and threads, which leave from here whatever the kind.
// workspace-state.ts:181
// export function deleteCopiesNotice(copies: readonly CopyToDelete[], threads: number): string {
//   const asked = copies.map(c => onDeleteOf(c.kind, c.copy, c.machineId, c.on).asked);
//   const records = `their records and ${fmtThreads(threads)} leave this computer.`;
//   if (new Set(asked).size === 1) return `Each one's ${asked[0]}; ${records}`;
//   const each = copies.map((c, i) => `${c.name}: its ${asked[i]}.`).join(" ");
//   return `${each} ${records.charAt(0).toUpperCase()}${records.slice(1)}`;
// }
pub fn delete_copies_notice(copies: &[CopyToDelete], threads: f64) -> String {
    let asked: Vec<String> = copies
        .iter()
        .map(|c| on_delete_of(&c.kind, c.copy.as_ref(), c.machine_id.as_deref(), c.on.as_ref()).asked)
        .collect();
    let records = format!("their records and {} leave this computer.", fmt_threads(threads));
    if asked.first().is_some_and(|first| asked.iter().all(|a| a == first)) {
        return format!("Each one's {}; {records}", asked[0]);
    }
    let each: Vec<String> = copies.iter().zip(&asked).map(|(c, asked)| format!("{}: its {asked}.", c.name)).collect();
    let mut chars = records.chars();
    let head: String = chars.next().map(|c| c.to_uppercase().collect()).unwrap_or_default();
    format!("{} {head}{}", each.join(" "), chars.as_str())
}

/// What a copy holds that its remote lacks, the line a delete's confirmation names before anything goes: its commits
/// not pushed and its uncommitted files, or what could not be counted, and nothing for a copy that holds neither.
// workspace-state.ts:191
// export function unpushedLine(name: string, checkout: Checkout | undefined): string | undefined {
//   if (checkout === undefined) return `${name}: could not read what is not pushed`;
//   const count = (n: number, one: string, many = `${one}s`): string => `${n} ${n === 1 ? one : many}`;
//   const parts = [
//     checkout.countsUnknown === true ? "commits it could not count" : checkout.ahead > 0 ? `${count(checkout.ahead, "commit")} not pushed` : null,
//     checkout.editsUnread === true ? "edits it could not read" : checkout.changed > 0 ? count(checkout.changed, "uncommitted file") : null,
//     checkout.stashes !== undefined && checkout.stashes > 0 ? count(checkout.stashes, "stash", "stashes") : null,
//   ].filter(part => part !== null);
//   if (parts.length === 0) return undefined;
//   const listed = parts.length === 1 ? parts[0]! : `${parts.slice(0, -1).join(", ")} and ${parts.at(-1)!}`;
//   return `${name} holds ${listed}`;
// }
pub fn unpushed_line(name: &str, checkout: Option<&Checkout>) -> Option<String> {
    let Some(checkout) = checkout else {
        return Some(format!("{name}: could not read what is not pushed"));
    };
    let count = |n: i64, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
    let (ahead, changed) = (checkout.ahead.get(), checkout.changed.get());
    let commits = if checkout.counts_unknown == Some(true) {
        Some("commits it could not count".to_string())
    } else if ahead > 0 {
        Some(format!("{} not pushed", count(ahead, "commit", "commits")))
    } else {
        None
    };
    let edits = if checkout.edits_unread == Some(true) {
        Some("edits it could not read".to_string())
    } else if changed > 0 {
        Some(count(changed, "uncommitted file", "uncommitted files"))
    } else {
        None
    };
    let stashes = checkout.stashes.map(|n| n.get()).filter(|n| *n > 0).map(|n| count(n, "stash", "stashes"));
    let parts: Vec<String> = [commits, edits, stashes].into_iter().flatten().collect();
    let (last, init) = parts.split_last()?;
    let listed = if init.is_empty() { last.clone() } else { format!("{} and {last}", init.join(", ")) };
    Some(format!("{name} holds {listed}"))
}

/// The line for a checkout whose read of what is not pushed did not finish, with why: what the read said last, or what
/// stopped it from running.
// workspace-state.ts:206
// export const unpushedUnreadLine = (name: string, why: string): string => `${unpushedLine(name, undefined)}: ${why}`;
pub fn unpushed_unread_line(name: &str, why: &str) -> String {
    format!("{}: {why}", unpushed_line(name, None).unwrap_or_default())
}

/// What a delete does to a create that failed before any machine was made, which is the one record with no machine id.
// workspace-state.ts:209
// private in the TS: port it as a private helper
// const NO_MACHINE_MADE: MachineOnDelete = { asked: "create failed before any computer was made, so there is none to delete", done: () => "its create had made no computer" };

/// A workspace on a computer somebody joined, as a person reads the two: the copy's name and that computer's.
// workspace-state.ts:212
// export interface StandsOn {
//   name: string;
//   computer: string;
// }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StandsOn {
    pub name: String,
    pub computer: String,
}

// workspace-state.ts:219
// in wsp_wire::types::workspace_state: UNNAMED_COMPUTER

/// What a delete does to a record of a wsp worktree: the worktree goes and the project folder stays. One on a computer somebody joined is deleted from that computer, by the names the person knows, since
/// a box is no cloud and the machine's own id is wsp's. Every other workspace takes its kind's words.
// workspace-state.ts:223
// export function onDeleteOf(kind: WorkspaceKind, copy?: { path: string }, machineId?: string, on?: StandsOn): MachineOnDelete {
//   if (machineId === "") return NO_MACHINE_MADE;
//   if (copy === undefined && on !== undefined) return { asked: `copy on ${on.computer} is deleted`, done: () => `${on.name} is deleted from ${on.computer}` };
//   if (copy === undefined) return WORKSPACE_KIND_WORDS[kind].onDelete;
//   const removed = `worktree at ${copy.path} is removed and the project folder is left as it is`;
//   return { asked: removed, done: () => `its ${removed}` };
// }
pub fn on_delete_of(kind: &WorkspaceKind, copy: Option<&CopyAt>, machine_id: Option<&str>, on: Option<&StandsOn>) -> MachineOnDelete {
    if machine_id == Some("") {
        return MachineOnDelete {
            asked: "create failed before any computer was made, so there is none to delete".to_string(),
            done: "its create had made no computer".to_string(),
        };
    }
    match (copy, on) {
        (None, Some(on)) => MachineOnDelete {
            asked: format!("copy on {} is deleted", on.computer),
            done: format!("{} is deleted from {}", on.name, on.computer),
        },
        (None, None) => kind_words(kind).on_delete,
        (Some(copy), _) => {
            let removed = format!("worktree at {} is removed and the project folder is left as it is", copy.path);
            MachineOnDelete { asked: removed.clone(), done: format!("its {removed}") }
        }
    }
}

// workspace-state.ts:231
// export interface WorkspaceStateInput {
//   phase: WorkspacePhase;
//   machineState?: MachineState | null | undefined;
//   reach?: ReachState | null | undefined;
//   /** Set on a record whose last wake ran its asks out with the machine still paused at the provider; the state words
//    * are unchanged by it (the machine is paused, and a wake may be tried again) but the rebuild is offered. */
//   wakeRefused?: string | null | undefined;
// }
/// None stands for the TS null and undefined alike.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceStateInput {
    pub phase: WorkspacePhase,
    pub machine_state: Option<MachineState>,
    pub reach: Option<ReachState>,
    pub wake_refused: Option<String>,
}

// workspace-state.ts:240
// export function workspaceState(input: WorkspaceStateInput): WorkspaceState {
//   const machine = input.machineState ?? null;
//   const reach = input.reach ?? null;
//   // The runtime reports a released sub-machine's machine as gone, which is what released means: its own word wins.
//   if (input.phase === "released") return "released";
//   if (machine === "gone") return "gone";
//   switch (input.phase) {
//     case "pausing":
//       return "pausing";
//     case "napping":
//       return "paused";
//     case "waking":
//       return "waking";
//     case "gone":
//       return "gone";
//     case "queued":
//       return "waiting";
//     case "running":
//       if (machine === "paused") return "paused";
//       if (machine === "starting") return "waking";
//       if (reach === "unreachable" || reach === "no-daemon" || reach === "zombie") return "unreachable";
//       return "running";
//     default: {
//       const _exhaustive: never = input.phase;
//       return "running";
//     }
//   }
// }
pub fn workspace_state(input: &WorkspaceStateInput) -> WorkspaceState {
    match input.phase {
        WorkspacePhase::Released => WorkspaceState::Released,
        _ if matches!(input.machine_state, Some(MachineState::Gone)) => WorkspaceState::Gone,
        WorkspacePhase::Pausing => WorkspaceState::Pausing,
        WorkspacePhase::Napping => WorkspaceState::Paused,
        WorkspacePhase::Waking => WorkspaceState::Waking,
        WorkspacePhase::Gone => WorkspaceState::Gone,
        WorkspacePhase::Queued => WorkspaceState::Waiting,
        WorkspacePhase::Running => {
            if matches!(input.machine_state, Some(MachineState::Paused)) {
                WorkspaceState::Paused
            } else if matches!(input.machine_state, Some(MachineState::Starting)) {
                WorkspaceState::Waking
            } else if matches!(input.reach, Some(ReachState::Unreachable | ReachState::NoDaemon | ReachState::Zombie)) {
                WorkspaceState::Unreachable
            } else {
                WorkspaceState::Running
            }
        }
    }
}

/// Whether a workspace is paused or on its way there: an expected state, often the work being done, so nothing that
/// reads it may read it as a fault.
// workspace-state.ts:271
// export const pausedOrPausing = (state: WorkspaceState): boolean => state === "paused" || state === "pausing";
pub fn paused_or_pausing(state: &WorkspaceState) -> bool {
    matches!(state, WorkspaceState::Paused | WorkspaceState::Pausing)
}

/// The one state word's key for a workspace as every client knows it: its phase, and the machine state and reach of
/// its status when one has arrived. A caller holding only the record passes null and reads the phase alone, which is
/// why a surface that shows a state asks for the status: the phase alone calls a paused or unreachable machine
/// running.
// workspace-state.ts:277
// export function workspaceStateOf(workspace: Pick<WorkspaceView, "phase">, status: Pick<WorkspaceStatus, "machineState" | "reach"> | null): WorkspaceState {
//   return workspaceState({ phase: workspace.phase, machineState: status?.machineState, reach: status?.reach.state });
// }
pub fn workspace_state_of(workspace: &WorkspaceView, status: Option<&WorkspaceStatus>) -> WorkspaceState {
    workspace_state(&WorkspaceStateInput {
        phase: workspace.phase,
        machine_state: status.map(|s| s.machine_state),
        reach: status.map(|s| s.reach.state),
        wake_refused: None,
    })
}

// workspace-state.ts:281
// private in the TS: port it as a private helper
// const WORDS: Record<WorkspaceState, string> = {
//   running: "Running",
//   pausing: "Stopping",
//   paused: "Stopped",
//   waking: "Waking",
//   unreachable: "Unreachable",
//   gone: "Gone",
//   waiting: "Waiting",
//   released: "Deleted",
// };

/// The two words a computer that keeps the processes and the bytes of a paused machine reads instead: a nap there
/// is a pause, and everywhere else it is a stop, which is what the machine does to every process it was running.
// workspace-state.ts:294
// private in the TS: port it as a private helper
// const MEMORY_WORDS: Partial<Record<WorkspaceState, string>> = { pausing: "Pausing", paused: "Paused" };

/// The one word for a workspace's state. The computer's pause mode rides where the caller holds it, since the same
/// state is a pause on a machine that keeps its memory and a stop on one that does not; a caller holding none reads
/// the stopping words, which is what every computer but a provider with memory pauses does.
// workspace-state.ts:299
// export function workspaceWord(state: WorkspaceState, pauseMode?: PauseMode): string {
//   return (pauseMode === "memory" ? MEMORY_WORDS[state] : undefined) ?? WORDS[state];
// }
pub fn workspace_word(state: &WorkspaceState, pause_mode: Option<&PauseMode>) -> String {
    let memory = matches!(pause_mode, Some(PauseMode::Memory));
    let word = match state {
        WorkspaceState::Pausing if memory => "Pausing",
        WorkspaceState::Paused if memory => "Paused",
        WorkspaceState::Running => "Running",
        WorkspaceState::Pausing => "Stopping",
        WorkspaceState::Paused => "Stopped",
        WorkspaceState::Waking => "Waking",
        WorkspaceState::Unreachable => "Unreachable",
        WorkspaceState::Gone => "Gone",
        WorkspaceState::Waiting => "Waiting",
        WorkspaceState::Released => "Deleted",
    };
    word.to_string()
}

/// What a row this workspace's machine stands on says under WHERE: the name this host has for the computer it runs
/// on, which only a caller holding the places list can give and which is what a person calls that machine; else the
/// provider its own record names, since this host may be wired to any of them and reading it off the kind would tell
/// somebody on Docker their workspace is at Solari; else the kind's own word; else the name wsp has for the machine,
/// which on those kinds is a login or a name somebody gave it and never an id a provider minted. Wherever that comes
/// out as the computer the host runs on, it is said in that machine's own words. The one place a surface asks where
/// a workspace runs.
// workspace-state.ts:310
// export function whereWord(record: Pick<WorkspaceView, "machineId"> & { kind?: WorkspaceKind | undefined; provider?: string | undefined; facts?: Pick<MachineFacts, "os"> | undefined }, named?: string): string {
//   const word = named ?? record.provider ?? kindWords(workspaceKind(record)).where ?? record.machineId;
//   return word === THIS_COMPUTER ? computerWord(record.facts?.os) : word;
// }
/// The fields of a workspace record `whereWord` reads, under the TS keys.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WhereRecord {
    pub machine_id: String,
    #[serde(default)]
    pub kind: Option<WorkspaceKind>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub facts: Option<WhereFacts>,
}

/// The one fact of a machine `whereWord` reads.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct WhereFacts {
    #[serde(default)]
    pub os: Option<String>,
}

pub fn where_word(record: &WhereRecord, named: Option<&str>) -> String {
    let word = named
        .map(str::to_string)
        .or_else(|| record.provider.clone())
        .or_else(|| kind_words(&workspace_kind(record.kind.as_ref())).where_.map(str::to_string))
        .unwrap_or_else(|| record.machine_id.clone());
    if word == THIS_COMPUTER {
        computer_word(record.facts.as_ref().and_then(|facts| facts.os.as_deref()))
    } else {
        word
    }
}

/// The line a verb that moved a workspace prints once the runtime has answered: the workspace and the word the next
/// listing will show for it, in the lowercase a line of work reads. The one place that word is lowered, so a pause
/// and a wake cannot spell one state two ways. The computer's pause mode rides where the caller holds it, as the
/// state word itself takes it: a nap on a provider that keeps the machine's memory is a pause, and a stop anywhere
/// else. A machine the provider has started and nothing on it answers gets a
/// sentence of its own, since one word there would say the wake failed when what happened is that the machine is up
/// and its daemon is not talking yet; a caller holding only a phase never reaches it.
// workspace-state.ts:322
// export function workspaceStateLine(name: string, state: WorkspaceState, pauseMode?: PauseMode): string {
//   return state === "unreachable" ? `${name} is up and not answering yet` : `${name} ${workspaceWord(state, pauseMode).toLowerCase()}`;
// }
pub fn workspace_state_line(name: &str, state: &WorkspaceState, pause_mode: Option<&PauseMode>) -> String {
    if matches!(state, WorkspaceState::Unreachable) {
        return format!("{name} is up and not answering yet");
    }
    format!("{name} {}", workspace_word(state, pause_mode).to_lowercase())
}

/// Every word a slot beside facts can hold for a computer that is not answering, which is what lets a reader of
/// that slot keep a closed set of words: a reading added here is a word added there, in one place.
// workspace-state.ts:328
// export type AwayWord = "no answer" | "stopped";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum AwayWord {
    #[serde(rename = "no answer")]
    NoAnswer,
    #[serde(rename = "stopped")]
    Stopped,
}

impl AwayWord {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            AwayWord::NoAnswer => "no answer",
            AwayWord::Stopped => "stopped",
        }
    }
}

/// What every surface says about a workspace whose computer is not answering, one reading per slot that has to
/// hold it: the mono word of a state slot, a row's third line, the length of the silence beside the computer's own
/// name, the two halves a pane refuses in, and the whole sentence. Six surfaces used to word this silence six ways
/// (a dead label, a state word that disagreed with the readings under it, two table cells, a launch refusal naming
/// a machine id and a toast in the sidebar's corner); they read these instead.
// workspace-state.ts:335
// export interface AbsentComputer {
//   /** The state slot's word, the protocol's own for a machine that answers nothing. */
//   readonly word: string;
//   /** The one word a table slot standing beside three fact columns holds: the silence itself, with no figure on it.
//    * How long it has been rides the row's title and the detail that already carries it. */
//   readonly away: AwayWord;
//   /** A workspace row's third line, which has room for the figure. Written to thirty characters because that is
//    * what the row leaves for text: a longer line is cut from the right, and the half that says what to do is the
//    * half that goes. The second half states the silence and asks; it never says the computer is off, which this
//    * host cannot know, since a computer that is on and not answering reads this same line. */
//   readonly line: string;
//   /** What happened, the first half a pane refuses in. */
//   readonly said: string;
//   /** What happens next, the second half: nobody has to reconnect it, so the one thing left to do is switch it on.
//    * Absent on a reading that carries a button instead, where the button is what happens next. */
//   readonly will?: string;
//   /** Both halves, for every slot with room for a sentence: the row's title, the pane's State row, the held send. */
//   readonly sentence: string;
//   /** The word on the button that puts back what the reading names, where this host owns the process that is
//    * missing rather than waiting on a computer of its own accord. A reading that carries one is also one a turn
//    * runs under, since what is missing never carried the turn: the composer stays open on it. */
//   readonly start?: string;
// }
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsentComputer {
    pub word: String,
    pub away: AwayWord,
    pub line: String,
    pub said: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub will: Option<String>,
    pub sentence: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
}

/// The one state of a computer that is not answering. awayMs is how long this host has not heard from it, null
/// where it never has.
// workspace-state.ts:361
// export function absentComputer(name: string, awayMs: number | null): AbsentComputer {
//   const away = "no answer";
//   const said = `${name} is not answering`;
//   const will = "it connects on its own when it is on";
//   const dated = awayMs === null ? away : `${away} ${offlineFor(awayMs)}`;
//   return { word: workspaceWord("unreachable"), away, line: `${dated}, is it on?`, said, will, sentence: `${said}; ${will}` };
// }
pub fn absent_computer(name: &str, away_ms: Option<f64>) -> AbsentComputer {
    let said = format!("{name} is not answering");
    let will = "it connects on its own when it is on";
    let dated = match away_ms {
        None => AwayWord::NoAnswer.as_str().to_string(),
        Some(ms) => format!("{} {}", AwayWord::NoAnswer.as_str(), offline_for(ms)),
    };
    AbsentComputer {
        word: workspace_word(&WorkspaceState::Unreachable, None),
        away: AwayWord::NoAnswer,
        line: format!("{dated}, is it on?"),
        sentence: format!("{said}; {will}"),
        said,
        will: Some(will.to_string()),
        start: None,
    }
}

// workspace-state.ts:370
// in wsp_wire::types::workspace_state: START_DAEMON_WORD

/// The one state of the daemon this host started for its own computer's workspace while it is not running. The
/// daemon is a child of the host, so this computer's reach is that child's reach and the app reads it as the
/// computer being absent, in this computer's own words; the host can start another, so the reading carries the
/// button rather than a second sentence, and a turn here runs without it. Unreachable is never one of these words:
/// the computer the app is drawn on is the one computer a person can see is on.
// workspace-state.ts:377
// export function ownDaemonDown(name: string): AbsentComputer {
//   const said = `${name}'s terminals and files stopped`;
//   // The row leaves thirty characters, which the sentence itself does not fit in, so the line says the same fact
//   // without the computer's name: the row already names the workspace, and the whole sentence rides its title.
//   return { word: "Stopped", away: "stopped", line: "stopped, start again", said, sentence: said, start: START_DAEMON_WORD };
// }
pub fn own_daemon_down(name: &str) -> AbsentComputer {
    let said = format!("{name}'s terminals and files stopped");
    AbsentComputer {
        word: "Stopped".to_string(),
        away: AwayWord::Stopped,
        line: "stopped, start again".to_string(),
        sentence: said.clone(),
        said,
        will: None,
        start: Some(START_DAEMON_WORD.to_string()),
    }
}

/// What the slot above the composer says while the workspace's computer is not answering. The box stays open and
/// keeps what is typed, so the line says what became of those words rather than that a send was refused: nothing
/// leaves until the computer answers, and then only on the person's own send. The computer is named because it is
/// the one thing the person can act on; the state word belongs to the row.
// workspace-state.ts:388
// export const composerHeldLine = (computer: string): string => `held until ${computer} answers`;
pub fn composer_held_line(computer: &str) -> String {
    format!("held until {computer} answers")
}

// workspace-state.ts:391
// in wsp_wire::types::workspace_state: WAITED_ROW_MS

/// The row a thread shows after one of its turns on a cloud machine waited for this host: nothing can show while the
/// host is away, so it is said once the turn goes on. Seconds under a minute, whole minutes after; a hold ends at an hour.
// workspace-state.ts:395
// export function waitedLine(ms: number): string {
//   const seconds = Math.round(ms / 1_000);
//   return `Waited ${seconds < 60 ? `${seconds} s` : `${Math.round(ms / 60_000)} min`} for your computer`;
// }
pub fn waited_line(ms: f64) -> String {
    let seconds = (ms / 1_000.0 + 0.5).floor();
    let span = if seconds < 60.0 {
        format!("{} s", number_string(seconds))
    } else {
        format!("{} min", number_string((ms / 60_000.0 + 0.5).floor()))
    };
    format!("Waited {span} for your computer")
}

/// How long this host has not heard from a computer, off the row it holds for it; null on a row that never
/// reported. One spelling of the silence's length, so the table and the sidebar date it alike.
// workspace-state.ts:402
// export function awayMsOf(place: { readonly lastSeenAt?: string | undefined }, now: number): number | null {
//   const since = place.lastSeenAt === undefined ? NaN : Date.parse(place.lastSeenAt);
//   return Number.isNaN(since) ? null : now - since;
// }
/// `last_seen_at` is the place's field of that name.
pub fn away_ms_of(last_seen_at: Option<&str>, now: f64) -> Option<f64> {
    let since: jiff::Timestamp = last_seen_at?.parse().ok()?;
    Some(now - since.as_millisecond() as f64)
}

/// A fact a computer reported before it went quiet, marked as the reading it is: what it was when it last spoke,
/// not what it is. Every slot that would otherwise stand at pending on a computer that is not answering says this
/// instead, since the host already holds the answer and pending reads as a fact still coming.
///
/// The word is the mark's, because the two spans a slot can be dated by are not the same: a fact that does not
/// change is dated by the silence (`last seen`), and one that grows while the computer is up is dated by the
/// report it was read in (`reported`), which can be hours older than the last frame.
// workspace-state.ts:414
// export function lastKnown(value: string, agoMs: number | null, word = "last seen"): string {
//   return agoMs === null ? value : `${value}, ${word} ${offlineFor(agoMs)} ago`;
// }
pub fn last_known(value: &str, ago_ms: Option<f64>, word: Option<&str>) -> String {
    match ago_ms {
        None => value.to_string(),
        Some(ms) => format!("{value}, {} {} ago", word.unwrap_or("last seen"), offline_for(ms)),
    }
}

// workspace-state.ts:419
// in wsp_wire::types::workspace_state: REPORTED_WORD

/// What this host knows about reaching one computer, for the surfaces that have to say it. absentComputer says the
/// computer is silent; this says where the app expects it, when it last spoke and what the last dial said, which
/// together are what tells a computer that is off from a road that is broken.
// workspace-state.ts:424
// export interface AbsentRoadInput {
//   name: string;
//   /** The ssh login the host was given for it, the address its last link dialled in from, and the forward on its own
//    * loopback it dials back through where it reaches this host no other way. */
//   road?: { readonly ssh?: string | undefined; readonly from?: string | undefined; readonly back?: PlaceBack | undefined } | undefined;
//   /** How long this host has not heard from it; null where it never has. */
//   awayMs: number | null;
//   /** What the last dial of it came to, where one has been made. */
//   dialled?: { readonly answered: boolean; readonly roundTripMs?: number | undefined; readonly said?: string | undefined } | undefined;
// }
#[derive(Clone, Debug, PartialEq)]
pub struct AbsentRoadInput {
    pub name: String,
    pub road: Option<PlaceRoad>,
    /// The TS `number | null`.
    pub away_ms: Option<f64>,
    pub dialled: Option<Dialled>,
}

/// The road fields of a place this module reads: its ssh address, the address it dials in from, and the port it
/// dials back on.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PlaceRoad {
    pub ssh: Option<String>,
    pub from: Option<String>,
    pub back: Option<PlaceBack>,
}

/// The answer of the last dial of a place.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Dialled {
    pub answered: bool,
    pub round_trip_ms: Option<f64>,
    pub said: Option<String>,
}

/// The road reading in the pieces its two surfaces need: the row detail draws them as rows of their own and the
/// pane says them as one sentence. Written once so the two cannot date the same silence differently.
// workspace-state.ts:437
// export interface AbsentRoad {
//   /** The Address row: the login the host dials, or the address the computer dialled in from, with the road it is.
//    * Null on a computer this host was not installed on and has never held a link from. */
//   address: string | null;
//   /** How it dials back through the forward on its own loopback, where it does; null for every other road. */
//   dialsBack: string | null;
//   /** The Answered row, without its label. */
//   answered: string;
//   /** What the last dial said, where it was refused; null where it answered or where none was made. */
//   refused: string | null;
//   /** The three as one sentence, for a pane with room for prose. */
//   sentence: string;
// }
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AbsentRoad {
    pub address: Option<String>,
    pub dials_back: Option<String>,
    pub answered: String,
    pub refused: Option<String>,
    pub sentence: String,
}

/// The word for each road, said the way a person would: a login this host dials, or an address that dials it.
// workspace-state.ts:452
const DIALS_IN: &str = "dials in";

// workspace-state.ts:453
const OVER_SSH_WORD: &str = "ssh";

/// The ssh login this host holds for a computer, where it holds one: the one road to a computer that is not
/// answering. A record written without a login carries the word empty rather than absent, and a sentence built on
/// an empty word names nothing, so every reading of a road asks here rather than comparing for itself.
// workspace-state.ts:458
// export const sshRoadOf = (road?: { readonly ssh?: string | undefined } | undefined): string | undefined =>
//   road?.ssh === undefined || road.ssh === "" ? undefined : road.ssh;
pub fn ssh_road_of(road: Option<&PlaceRoad>) -> Option<String> {
    road.and_then(|road| road.ssh.as_deref()).filter(|ssh| !ssh.is_empty()).map(str::to_string)
}

/// The address a box's link dials for a forward standing on its own loopback.
// workspace-state.ts:462
// export const backUrl = (boxPort: number): string => `http://${authority(LOOPBACK, boxPort)}`;
pub fn back_url(box_port: f64) -> String {
    format!("http://{}", authority(LOOPBACK, box_port))
}

/// The road back to this host through the forward on a box's own loopback, the one spelling every surface says.
// workspace-state.ts:465
// export const BACK_OVER_SSH = `back over ${OVER_SSH_WORD}`;
// in wsp_wire::types::workspace_state: BACK_OVER_SSH

/// A box's road back to this host through the forward on its own loopback, with where that forward stands.
// workspace-state.ts:468
// export const dialsBackWord = (boxPort: number, name: string): string => `dials ${BACK_OVER_SSH} (${authority(LOOPBACK, boxPort)} on ${name})`;
pub fn dials_back_word(box_port: f64, name: &str) -> String {
    format!("dials {BACK_OVER_SSH} ({} on {name})", authority(LOOPBACK, box_port))
}

/// The road a link came in on, off the address the computer says it dialled. The box writes that address, so one
/// this host did not hand out is never said: undefined, and the sentence names no road.
// workspace-state.ts:472
// export const linkedOver = (dialed: string, back: PlaceBack | undefined, handed: readonly string[]): string | undefined =>
//   back !== undefined && dialed === backUrl(back.boxPort) ? `over ${OVER_SSH_WORD}` : handed.includes(dialed) ? `at ${dialed}` : undefined;
pub fn linked_over(dialed: &str, back: Option<&PlaceBack>, handed: &[impl AsRef<str>]) -> Option<String> {
    if back.is_some_and(|back| dialed == back_url(back.box_port.as_f64())) {
        return Some(format!("over {OVER_SSH_WORD}"));
    }
    handed.iter().any(|h| h.as_ref() == dialed).then(|| format!("at {dialed}"))
}

// workspace-state.ts:475
// export function absentRoad(input: AbsentRoadInput): AbsentRoad {
//   const ssh = sshRoadOf(input.road);
//   const back = input.road?.back;
//   const dialsBack = back === undefined ? null : dialsBackWord(back.boxPort, input.name);
//   // A link through the forward comes from this computer's own loopback, which is no address of that computer's.
//   const from = back === undefined ? input.road?.from : undefined;
//   const address = ssh !== undefined ? `${ssh} over ${OVER_SSH_WORD}` : from !== undefined && from !== "" ? `${from} ${DIALS_IN}` : null;
//   const answered = input.awayMs === null ? "not since it joined" : `${offlineFor(input.awayMs)} ago`;
//   const refused = input.dialled !== undefined && !input.dialled.answered && input.dialled.said !== undefined ? input.dialled.said : null;
//   const where =
//     ssh !== undefined
//       ? `wsp logs in to ${input.name} at ${ssh} over ssh${dialsBack === null ? "" : `, and ${input.name} ${dialsBack}`}`
//       : dialsBack !== null
//         ? `${input.name} ${dialsBack}`
//         : from !== undefined && from !== ""
//           ? `wsp waits for ${input.name} to dial in, last from ${from}`
//           : `wsp waits for ${input.name} to dial in`;
//   const when = input.awayMs === null ? `it has not answered since it joined` : `it last answered ${offlineFor(input.awayMs)} ago`;
//   return { address, dialsBack, answered, refused, sentence: `${where}; ${when}.${refused === null ? "" : ` The last try said: ${refused}`}` };
// }
pub fn absent_road(input: &AbsentRoadInput) -> AbsentRoad {
    let ssh = ssh_road_of(input.road.as_ref());
    let back = input.road.as_ref().and_then(|road| road.back.as_ref());
    let dials_back = back.map(|back| dials_back_word(back.box_port.as_f64(), &input.name));
    let from = match back {
        None => input.road.as_ref().and_then(|road| road.from.as_deref()).filter(|from| !from.is_empty()),
        Some(_) => None,
    };
    let address = match (&ssh, from) {
        (Some(ssh), _) => Some(format!("{ssh} over {OVER_SSH_WORD}")),
        (None, Some(from)) => Some(format!("{from} {DIALS_IN}")),
        (None, None) => None,
    };
    let answered = match input.away_ms {
        None => "not since it joined".to_string(),
        Some(ms) => format!("{} ago", offline_for(ms)),
    };
    let refused = input.dialled.as_ref().filter(|dialled| !dialled.answered).and_then(|dialled| dialled.said.clone());
    let where_ = match (&ssh, &dials_back, from) {
        (Some(ssh), dials_back, _) => {
            let and = dials_back.as_ref().map_or(String::new(), |dials| format!(", and {} {dials}", input.name));
            format!("wsp logs in to {} at {ssh} over ssh{and}", input.name)
        }
        (None, Some(dials), _) => format!("{} {dials}", input.name),
        (None, None, Some(from)) => format!("wsp waits for {} to dial in, last from {from}", input.name),
        (None, None, None) => format!("wsp waits for {} to dial in", input.name),
    };
    let when = match input.away_ms {
        None => "it has not answered since it joined".to_string(),
        Some(ms) => format!("it last answered {} ago", offline_for(ms)),
    };
    let refused_tail = refused.as_ref().map_or(String::new(), |said| format!(" The last try said: {said}"));
    AbsentRoad { address, dials_back, answered, sentence: format!("{where_}; {when}.{refused_tail}"), refused }
}

/// What one dial came to, in the slot the button that asked stands in. A computer holding its link answers the
/// frame; one that does not is dialled over the road it was added on, and a road that answers while the link is
/// down is the reading a person came for: the computer is on and the agent on it is not calling home.
// workspace-state.ts:499
// export function placeDialLine(input: { name: string; road?: { readonly ssh?: string | undefined } | undefined; linked: boolean; dialled: { readonly answered: boolean; readonly roundTripMs?: number | undefined; readonly said?: string | undefined } }): string {
//   const { dialled } = input;
//   if (!dialled.answered) return dialled.said ?? `${input.name} answered nothing.`;
//   const took = dialled.roundTripMs === undefined ? "" : ` in ${dialled.roundTripMs} ms`;
//   if (input.linked) return `${input.name} answered${took}.`;
//   const at = sshRoadOf(input.road);
//   return `${at ?? input.name} answered over ${OVER_SSH_WORD}${took}, so the computer is on; the agent on it is not dialling this host.`;
// }
pub fn place_dial_line(name: &str, road: Option<&PlaceRoad>, linked: bool, dialled: &Dialled) -> String {
    if !dialled.answered {
        return dialled.said.clone().unwrap_or_else(|| format!("{name} answered nothing."));
    }
    let took = dialled.round_trip_ms.map_or(String::new(), |ms| format!(" in {} ms", number_string(ms)));
    if linked {
        return format!("{name} answered{took}.");
    }
    let at = ssh_road_of(road).unwrap_or_else(|| name.to_string());
    format!("{at} answered over {OVER_SSH_WORD}{took}, so the computer is on; the agent on it is not dialling this host.")
}

/// The one sentence a dial gets on a computer this host has no road to: it was joined by typing a code, so nothing
/// here can make it speak and the only thing to do is switch it on.
// workspace-state.ts:510
// export const placeNoDialLine = (name: string): string => `wsp has no road to dial ${name}: it joined by typing a code, so it connects on its own when it is on.`;
pub fn place_no_dial_line(name: &str) -> String {
    format!("wsp has no road to dial {name}: it joined by typing a code, so it connects on its own when it is on.")
}

/// The roads a dial can take, in the order the host tries them.
// workspace-state.ts:513
// export type PlaceDialRoad = "link" | "ssh";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum PlaceDialRoad {
    #[serde(rename = "link")]
    Link,
    #[serde(rename = "ssh")]
    Ssh,
}

impl PlaceDialRoad {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            PlaceDialRoad::Link => "link",
            PlaceDialRoad::Ssh => "ssh",
        }
    }
}

/// Which road a dial of this computer would take, and nothing at all where there is none: a computer holding its
/// link answers a frame on it, one that is down is dialled over the ssh login it was installed with, and a computer
/// that joined by typing a code and is not there can only be switched on, which is placeNoDialLine's whole subject.
/// The host reads this to pick the road and the app reads it to decide whether to draw the button at all, so no
/// screen offers a press whose only answer is that there was nothing to press.
// workspace-state.ts:520
// export function placeDialRoad(place: { present?: boolean | undefined; road?: { readonly ssh?: string | undefined; readonly from?: string | undefined } | undefined }): PlaceDialRoad | undefined {
//   if (place.present === true) return "link";
//   return sshRoadOf(place.road) === undefined ? undefined : "ssh";
// }
pub fn place_dial_road(present: Option<bool>, road: Option<&PlaceRoad>) -> Option<PlaceDialRoad> {
    if present == Some(true) {
        return Some(PlaceDialRoad::Link);
    }
    ssh_road_of(road).map(|_| PlaceDialRoad::Ssh)
}

/// Why an action that needs the machine (send, import, export) cannot run in this state; null while running.
/// goneWords are the provider's, quoted when the caller holds them (the runtime does, the composer does not), and
/// name is the workspace's, which the gone sentence leads with where the caller holds the record.
// workspace-state.ts:528
// export function actionRefusal(state: WorkspaceState, action: string, goneWords?: string, name?: string): string | null {
//   switch (state) {
//     case "running":
//       return null;
//     case "pausing":
//     case "paused":
//       return `Workspace is ${state}; wake it to ${action}`;
//     case "waking":
//       return `Workspace is waking; ${action}s open when it is running`;
//     case "unreachable":
//       return `Workspace is unreachable; ${action}s open when the machine answers`;
//     case "gone":
//       return goneRefusal(name, action, goneWords);
//     // A send into a sub-thread waiting for its machine waits with it; nothing else has a machine to act on yet.
//     case "waiting":
//       return action === "send" ? null : `Workspace is waiting for a machine; ${action}s open when it is running`;
//     case "released":
//       return `${name ?? "This workspace"}'s machine was deleted once its sub-thread was done, so there is none to ${action} on`;
//     default: {
//       const _exhaustive: never = state;
//       return null;
//     }
//   }
// }
pub fn action_refusal(state: &WorkspaceState, action: &str, gone_words: Option<&str>, name: Option<&str>) -> Option<String> {
    match state {
        WorkspaceState::Running => None,
        WorkspaceState::Pausing | WorkspaceState::Paused => Some(format!("Workspace is {}; wake it to {action}", state.as_str())),
        WorkspaceState::Waking => Some(format!("Workspace is waking; {action}s open when it is running")),
        WorkspaceState::Unreachable => Some(format!("Workspace is unreachable; {action}s open when the machine answers")),
        WorkspaceState::Gone => Some(gone_refusal(name, action, gone_words)),
        WorkspaceState::Waiting if action == "send" => None,
        WorkspaceState::Waiting => Some(format!("Workspace is waiting for a machine; {action}s open when it is running")),
        WorkspaceState::Released => Some(format!(
            "{}'s machine was deleted once its sub-thread was done, so there is none to {action} on",
            name.unwrap_or("This workspace")
        )),
    }
}

/// What refuses a send before the workspace's state is asked: the socket to wsp, the workspace lookup, the
/// transcript still loading, the agent catalog this workspace picks its model and access out of still unanswered.
// workspace-state.ts:555
// export type SendBlock = "connecting" | "reconnecting" | "closed" | "not-found" | "loading" | "no-agents";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum SendBlock {
    #[serde(rename = "connecting")]
    Connecting,
    #[serde(rename = "reconnecting")]
    Reconnecting,
    #[serde(rename = "closed")]
    Closed,
    #[serde(rename = "not-found")]
    NotFound,
    #[serde(rename = "loading")]
    Loading,
    #[serde(rename = "no-agents")]
    NoAgents,
}

impl SendBlock {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            SendBlock::Connecting => "connecting",
            SendBlock::Reconnecting => "reconnecting",
            SendBlock::Closed => "closed",
            SendBlock::NotFound => "not-found",
            SendBlock::Loading => "loading",
            SendBlock::NoAgents => "no-agents",
        }
    }
}

/// Every kind of send refusal: a block, or a state other than running.
// workspace-state.ts:558
// export type SendRefusalKind = WorkspaceState | SendBlock;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SendRefusalKind {
    State(WorkspaceState),
    Block(SendBlock),
}

/// What the composer says on the send button for each block, and the one table of those words: a harness driving
/// the built app reads them to know the app is not ready for a key press yet.
// workspace-state.ts:562
// export const SEND_BLOCK_WORDS: Record<SendBlock, string> = {
//   connecting: "Connecting to wsp",
//   reconnecting: "wsp is not running, reconnecting",
//   closed: "wsp is not running",
//   "not-found": "Task not found",
//   loading: "Loading transcript",
//   "no-agents": "the agents here have not answered yet",
// };
// in wsp_wire::types::workspace_state: SEND_BLOCK_WORDS (pairs in the TS order)

/// Why a turn cannot be sent, one sentence per kind; null while running. The composer draws every row; the runtime
/// throws the state rows, so the two say the same thing about a machine. A thread whose turn replied but still runs
/// has stillWorkingLine, which says the message waits for it.
// workspace-state.ts:574
// export function sendRefusal(kind: SendRefusalKind, goneWords?: string, name?: string): string | null {
//   return isBlock(kind) ? SEND_BLOCK_WORDS[kind] : actionRefusal(kind, "send", goneWords, name);
// }
pub fn send_refusal(kind: &SendRefusalKind, gone_words: Option<&str>, name: Option<&str>) -> Option<String> {
    match kind {
        SendRefusalKind::Block(block) => SEND_BLOCK_WORDS.iter().find(|(key, _)| *key == block.as_str()).map(|(_, words)| words.to_string()),
        SendRefusalKind::State(state) => action_refusal(state, "send", gone_words, name),
    }
}

// workspace-state.ts:578
// private in the TS: port it as a private helper
// const isBlock = (kind: SendRefusalKind): kind is SendBlock => kind in SEND_BLOCK_WORDS;

// workspace-state.ts:580
// private in the TS: port it as a private helper
// const CONTROL_WORDS: Record<Exclude<ScreenControl, "sign-in">, string> = {
//   model: "pick the model in the row under the box",
//   access: "pick the access mode in the row under the box",
//   settings: "wsp's settings open from the command palette",
//   docs: "wsp's docs are at usewsp.com/docs",
// };

/// How the person signs this workspace's agent in, in one place: the sign-in is the workspace's, so the Workspace
/// panel signs one in and this computer is signed in from its own terminal. Both the composer's line for a sign-in
/// command and a turn the agent refused for want of a sign-in read this rule, so the two never send a person two ways.
// workspace-state.ts:590
// export function signInRoad(view: Pick<WorkspaceView, "kind">): string {
//   if (isLocalWorkspace(view)) return `sign in from a terminal on ${THIS_COMPUTER}`;
//   return kindWords(workspaceKind(view)).inFolder ? "sign in from a terminal on the computer the thread runs on" : CLOUD_SIGN_IN_ROAD;
// }
pub fn sign_in_road(kind: Option<&WorkspaceKind>) -> String {
    if is_local_workspace(kind) {
        format!("sign in from a terminal on {THIS_COMPUTER}")
    } else if kind_words(&workspace_kind(kind)).in_folder {
        "sign in from a terminal on the computer the thread runs on".to_string()
    } else {
        CLOUD_SIGN_IN_ROAD.to_string()
    }
}

// workspace-state.ts:596
// in wsp_wire::types::workspace_state: CLOUD_SIGN_IN_ROAD

/// wsp's half of a turn the agent refused for want of a sign-in: the road above, and that the turn is the person's
/// to send again once they have taken it. The agent's own sentence names its login command; this names where.
// workspace-state.ts:600
// export const signInRefusalLine = (view: Pick<WorkspaceView, "kind">): string => `${signInRoad(view)}, then send again`;
pub fn sign_in_refusal_line(kind: Option<&WorkspaceKind>) -> String {
    format!("{}, then send again", sign_in_road(kind))
}

/// The composer's line before a thread's first send on the computer the app runs on, where the agents report says the
/// agent there has no sign-in, so the person is told before the turn rather than by its failure.
// workspace-state.ts:604
// export const signedOutLine = (agent: string, computer: string): string => `${agent} is not signed in on ${computer}; sign in from a terminal there before you send`;
pub fn signed_out_line(agent: &str, computer: &str) -> String {
    format!("{agent} is not signed in on {computer}; sign in from a terminal there before you send")
}

/// The composer's line for a slash command the CLI runs only in its own terminal, in place of a turn that would answer
/// the command is not available: what the command is, then the wsp control that serves the same intent.
// workspace-state.ts:608
// export function screenCommandLine(command: ScreenCommand, catalog: Pick<HarnessCatalog, "label">, view: Pick<WorkspaceView, "kind">): string {
//   const control = command.control === "sign-in" ? signInRoad(view) : CONTROL_WORDS[command.control];
//   return `/${command.name} works only in ${catalog.label}'s own terminal; ${control}`;
// }
pub fn screen_command_line(command: &ScreenCommand, catalog: &HarnessCatalog, kind: Option<&WorkspaceKind>) -> String {
    let control = match command.control {
        ScreenControl::SignIn => sign_in_road(kind),
        ScreenControl::Model => "pick the model in the row under the box".to_string(),
        ScreenControl::Access => "pick the access mode in the row under the box".to_string(),
        ScreenControl::Settings => "wsp's settings open from the command palette".to_string(),
        ScreenControl::Docs => "wsp's docs are at usewsp.com/docs".to_string(),
    };
    format!("/{} works only in {}'s own terminal; {control}", command.name, catalog.label)
}

/// A probe that found the machine: the edge answered, promptly or late.
// workspace-state.ts:614
// private in the TS: port it as a private helper
// const ANSWERED: ReadonlySet<ReachState> = new Set<ReachState>(["reachable", "slow"]);

/// A probe nothing answered: silence, or the edge dialling the guest and finding the daemon port dead.
// workspace-state.ts:616
// private in the TS: port it as a private helper
// const UNANSWERED: ReadonlySet<ReachState> = new Set<ReachState>(["unreachable", "no-daemon"]);

/// Whether a workspace's reach says nothing is answering on its daemon's road: the probe found silence, or found
/// the road open with the daemon port dead. On the computer the host runs on both mean one thing, the child this
/// host started is not running, which is why the one reading of it asks this and not a list of states.
// workspace-state.ts:621
// export const daemonSilent = (reach: ReachState | null | undefined): boolean => reach != null && UNANSWERED.has(reach);
pub fn daemon_silent(reach: Option<&ReachState>) -> bool {
    reach.is_some_and(unanswered)
}

fn unanswered(reach: &ReachState) -> bool {
    matches!(reach, ReachState::Unreachable | ReachState::NoDaemon)
}

/// The reach word a row shows after one probe. A single silence after an answer keeps the answer's word: one slow
/// edge answer, one DNS blip, one busy second on the box or one probe that landed while the daemon was restarting
/// is not the machine gone dark, so the word turns only on the second silence in a row. Every answer, and a silence
/// after anything but an answer, shows as it came. This is also the window the runtime waits out before it puts a
/// daemon back: the word the row is given is already the second unanswered probe.
// workspace-state.ts:628
// export function reachShown(lastProbe: ReachState | undefined, probed: ReachState): ReachState {
//   if (!UNANSWERED.has(probed)) return probed;
//   return lastProbe !== undefined && ANSWERED.has(lastProbe) ? lastProbe : probed;
// }
pub fn reach_shown(last_probe: Option<&ReachState>, probed: &ReachState) -> ReachState {
    if !unanswered(probed) {
        return *probed;
    }
    match last_probe {
        Some(last) if matches!(last, ReachState::Reachable | ReachState::Slow) => *last,
        _ => *probed,
    }
}

/// Whether the machine is up and billing in this state: running, or running with its edge or daemon dark (the
/// provider bills a machine it cannot be reached on). Paused, moving and gone ones bill nothing, and only a billing
/// machine has anything to nap, so the rate and the nap countdown on every surface read this one rule.
// workspace-state.ts:636
// export function isBilling(state: WorkspaceState): boolean {
//   return state === "running" || state === "unreachable";
// }
pub fn is_billing(state: &WorkspaceState) -> bool {
    matches!(state, WorkspaceState::Running | WorkspaceState::Unreachable)
}

/// A record whose create failed before any machine stood under it: there is nothing to rebuild or forget, and its
/// `gone` says why it did not start.
// workspace-state.ts:642
// export const startFailed = (w: { phase: WorkspacePhase; machineId: string }): boolean => w.phase === "gone" && w.machineId === "";
pub fn start_failed(w: &WorkspaceView) -> bool {
    w.phase == WorkspacePhase::Gone && w.machine_id.is_empty()
}

/// Rebuild is a road out: the machine is gone, a zombie the provider still calls running, or one the provider would
/// not resume for the whole of a wake's asking. Every surface that offers the rebuild (sidebar row, palette, Machine
/// tab, the command line and its tool) asks this and nothing else. It is the only one left on the first two; on the
/// third the wake can be tried again beside it, since the fault is the provider's and may pass.
// workspace-state.ts:648
// export function needsRebuild(input: WorkspaceStateInput): boolean {
//   return workspaceState(input) === "gone" || input.reach === "zombie" || (input.wakeRefused ?? null) !== null;
// }
pub fn needs_rebuild(input: &WorkspaceStateInput) -> bool {
    workspace_state(input) == WorkspaceState::Gone || input.reach == Some(ReachState::Zombie) || input.wake_refused.is_some()
}

/// Why a road out that opens only once the machine is gone is refused on one that is merely not answering. The
/// rebuild and the forget are both such roads and stand four rows apart in one list, so they read the workspace's
/// state off the same field and say it in the same words: a machine nothing has heard from does not answer, and a
/// person told both at once stops believing either. Two halves as every refusal here has them: what is so, then when
/// the road is there to take.
/// The two roads a gone workspace is offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GoneRoad {
    Rebuild,
    Forget,
}

// workspace-state.ts:657
// export const notAnsweringYet = (road: "rebuild" | "forget"): string => `This one is not answering yet; the ${road} is offered once it is gone`;
pub fn not_answering_yet(road: GoneRoad) -> String {
    format!("This one is not answering yet; the {} is offered once it is gone", road_name(road))
}

fn road_name(road: GoneRoad) -> &'static str {
    match road {
        GoneRoad::Rebuild => "rebuild",
        GoneRoad::Forget => "forget",
    }
}

// workspace-state.ts:661
// in wsp_wire::types::workspace_state: FORGET_NEEDS_GONE

/// Why a road that waits on the machine being gone is refused, for the two roads that wait on it: the rebuild and
/// the forget. Both read the one folded state and say it in the one word the row under them shows; each names its
/// own road in the second half, which is all that differs between them. The command line and the app refuse a
/// rebuild through this same sentence off the same state, so the terminal and the row can no longer say two things
/// about one workspace. Two halves as every refusal here has them: what is so, then when the road is there to take.
// workspace-state.ts:668
// export function goneRoadRefusal(state: WorkspaceState, road: "rebuild" | "forget", pauseMode?: PauseMode): string {
//   if (state === "unreachable") return notAnsweringYet(road);
//   const word = workspaceWord(state, pauseMode).toLowerCase();
//   return road === "rebuild" ? `This one is ${word}, so nothing needs rebuilding; the rebuild is offered once a workspace is gone` : `${FORGET_NEEDS_GONE}; this one is ${word}`;
// }
pub fn gone_road_refusal(state: &WorkspaceState, road: GoneRoad, pause_mode: Option<&PauseMode>) -> String {
    if matches!(state, WorkspaceState::Unreachable) {
        return not_answering_yet(road);
    }
    let word = workspace_word(state, pause_mode).to_lowercase();
    match road {
        GoneRoad::Rebuild => format!("This one is {word}, so nothing needs rebuilding; the rebuild is offered once a workspace is gone"),
        GoneRoad::Forget => format!("{FORGET_NEEDS_GONE}; this one is {word}"),
    }
}

/// The one sentence for a verb a gone machine cannot take (send, wake, fork), with the provider's words when the
/// caller holds them; rebuild and delete are the roads out. It says what went with the disk and what the rebuild
/// brings back, the home folder the last saved nap stored, and nothing more. Led by the workspace's name where the
/// caller holds it.
// workspace-state.ts:678
// export function goneRefusal(name: string | undefined, action: string, words?: string): string {
//   const sentence = `${name ?? "This workspace"}'s machine is gone with its disk, so work that was not pushed is lost; rebuild it to ${action}, which brings back its home folder from the last saved nap`;
//   return words === undefined || words === "" ? sentence : `${sentence} (${words})`;
// }
pub fn gone_refusal(name: Option<&str>, action: &str, words: Option<&str>) -> String {
    let sentence = format!(
        "{}'s machine is gone with its disk, so work that was not pushed is lost; rebuild it to {action}, which brings back its home folder from the last saved nap",
        name.unwrap_or("This workspace")
    );
    match words {
        Some(words) if !words.is_empty() => format!("{sentence} ({words})"),
        _ => sentence,
    }
}

