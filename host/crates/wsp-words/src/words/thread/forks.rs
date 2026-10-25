// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/words/thread.ts:446-917 at a3b870a6e: asides, rewinds and forks, codex and
//! catalog lines, memory and daemon lines, vault, hostname, gone and name refusals, providers and places for an
//! image, and the lines a thread names a folder by. Each TS statement stands as a comment above its stub. See
//! spec/thread_forks.md.

use crate::js::{number_string, to_fixed, upper_first, utf16};
use crate::place_word::{names_place, place_said};
use crate::words::computer::thread_word;
use crate::words::units::{charges_nothing, fmt_bytes, fmt_bytes_of, fmt_duration, fmt_rate, fmt_size, DurationStyle};
use wsp_wire::types::js_number::JsNumber;
use wsp_wire::views::harness::{HarnessCatalog, HarnessCatalogSource};
use wsp_wire::wire::capabilities::{MachineSizeOffer, WorkspaceSize};

/// The refusal of a side question on a thread whose agent never announced a session: there is no conversation of
/// its own to copy and ask.
// thread.ts:449
pub const ASIDE_NO_SESSION_LINE: &str =
    "this thread's agent has not started a conversation yet, so there is nothing to ask about; wait for its first reply";

/// The refusal of a side question with no words in it.
// thread.ts:452
pub const BLANK_ASIDE_LINE: &str = "a side question needs words to ask";

/// The refusal of a side question on a thread whose agent cannot copy a session. A message the thread should keep is
/// a send, which every agent takes.
// thread.ts:456
// export function asideUnsupportedLine(harness: string): string {
//   return `${harness} takes no side question; send it as a message and the thread keeps it`;
// }
pub fn aside_unsupported_line(harness: &str) -> String {
    format!("{harness} takes no side question; send it as a message and the thread keeps it")
}

/// Why a side question came back with no answer: the harness said nothing for the whole wall and was stopped.
// thread.ts:461
// export function asideWallLine(ms: number): string {
//   return `the side question had no answer after ${fmtDuration(ms)} and was stopped`;
// }
pub fn aside_wall_line(ms: f64) -> String {
    format!("the side question had no answer after {} and was stopped", fmt_duration(ms, None))
}

/// Why a side question came back with no answer: the harness ended it with no words in it.
// thread.ts:466
pub const ASIDE_EMPTY_LINE: &str = "the side question came back with no words in its answer; ask it again";

/// Why a side question came back with no answer: the model reached for a tool, which a side question runs none of.
// thread.ts:469
pub const ASIDE_TOOL_LINE: &str =
    "the answer reached for a tool, and a side question runs none; ask it again, or send it as a message for the thread to work on";

/// A rewind refused on a thread whose turn is running: the person stops it, never the rewind.
// thread.ts:472
pub const REWIND_WORKING_LINE: &str = "this thread is working; stop its turn first, since a rewind never stops it for you";

/// A rewind refused on a thread with threads under it that still run, named, so the person stops the ones they mean.
// thread.ts:475
// export function rewindChildrenLine(titles: readonly string[]): string {
//   return `stop the threads under this one first (${titles.join(", ")}); a rewind never stops them for you`;
// }
pub fn rewind_children_line(titles: &[impl AsRef<str>]) -> String {
    let names: Vec<&str> = titles.iter().map(|t| t.as_ref()).collect();
    format!("stop the threads under this one first ({}); a rewind never stops them for you", names.join(", "))
}

/// An undo refused while another thread in the same folder runs: the files would go back under its agent mid-turn,
/// and nobody chose to lose what it is writing.
// thread.ts:481
// export function rewindBesideLine(title: string): string {
//   return `another thread in this folder is working (${title}), and its files would go back too; let its turn end or stop it first`;
// }
pub fn rewind_beside_line(title: &str) -> String {
    format!("another thread in this folder is working ({title}), and its files would go back too; let its turn end or stop it first")
}

/// Why a rewind put the conversation back and left the files: another thread worked in the folder after that reply,
/// and its work is in those files too.
// thread.ts:487
pub const REWIND_SHARED_LINE: &str = "another thread worked in this folder after that reply, so only the conversation goes back";

/// Files asked back to a turn whose end left no checkpoint of them.
// thread.ts:490
pub const REWIND_NO_CHECKPOINT_LINE: &str = "that reply kept no checkpoint of the files, so only its conversation can be rewound";

/// A conversation asked cut at a turn whose harness named no point to cut at.
// thread.ts:493
// export function rewindNoAnchorLine(agent: string): string {
//   return `${agent} left no point in that reply to cut its conversation at, so only the files can go back`;
// }
pub fn rewind_no_anchor_line(agent: &str) -> String {
    format!("{agent} left no point in that reply to cut its conversation at, so only the files can go back")
}

/// A rewind whose harness kept the conversation whole, `why` in its own clause: what moved and what stayed.
// thread.ts:498
// export function rewindKeptLine(why: string, filesBack: boolean): string {
//   return `${why}; ${filesBack ? "the files went back and every turn stays" : "every turn stays"}`;
// }
pub fn rewind_kept_line(why: &str, files_back: bool) -> String {
    format!("{why}; {}", if files_back { "the files went back and every turn stays" } else { "every turn stays" })
}

/// Why a Codex thread's conversation cannot be cut: the server cuts only the paginated history a Codex from 0.151.0
/// on writes.
// thread.ts:504
pub const CODEX_LEGACY_HISTORY: &str =
    "a Codex older than 0.151.0 made this thread and keeps its history in a form thread/revert cannot cut";

/// A Codex rewind by count whose server lists fewer turns than the cut would take: cutting there would take the
/// reply the person keeps too.
// thread.ts:508
pub const CODEX_FEWER_TURNS: &str = "codex lists fewer turns in this thread than the rewind would cut, so it cut nothing";

/// A fork at a Codex turn its server does not list in the thread.
// thread.ts:511
// export const codexNoTurnLine = (turnId: string): string => `codex lists no turn ${turnId} in this thread`;
pub fn codex_no_turn_line(turn_id: &str) -> String {
    format!("codex lists no turn {turn_id} in this thread")
}

/// A rewind to the thread's latest reply, after which nothing stands to cut.
// thread.ts:514
pub const REWIND_LATEST_LINE: &str = "that is the thread's latest reply, so nothing comes after it to rewind";

/// An undo with no rewind to undo, or one a turn in the same folder has ended since.
// thread.ts:517
pub const REWIND_NO_UNDO_LINE: &str = "this thread has no rewind to undo; undo lasts until the next turn in this folder ends";

/// What Undo rewind puts back, said beside it: the files, and never the turns the rewind cut.
// thread.ts:520
pub const UNDO_REWIND_LINE: &str = "Files come back; the cut conversation does not.";

/// Said beside every rewind that moves files: they go back only where no other thread has worked in the folder since
/// that reply, and otherwise the files stay where they are.
// thread.ts:524
const SHARED_FOLDER: &str = "If another thread has worked in this folder since this reply, the files stay as they are.";

// thread.ts:526
// private in the TS: port it as a private helper
// const turnsAfter = (turns: number): string => (turns === 1 ? "The turn after this reply leaves" : `The ${turns} turns after this reply leave`);

/// What a rewind to a reply takes, said before the click: how many turns go, whether the files go back, and what
/// Undo rewind brings back after. An agent that cuts no history of its own keeps every turn, and the note says so.
// thread.ts:530
// export function rewindNote(o: { turns: number; files: boolean; cutsConversation: boolean; agent: string; kept?: string }): string {
//   const undo = "Undo rewind puts them back until the next turn in this folder ends";
//   if (o.kept !== undefined) return `${o.kept.charAt(0).toUpperCase()}${o.kept.slice(1)}, so the conversation stays and the files go back to how they stood at this reply. ${SHARED_FOLDER} ${undo}.`;
//   if (!o.cutsConversation) return `${o.agent} keeps its own history, so the conversation stays and the files go back to how they stood at this reply. ${SHARED_FOLDER} ${undo}.`;
//   if (!o.files) return `${turnsAfter(o.turns)} the conversation; the files stay as they are.`;
//   return `${turnsAfter(o.turns)} the conversation, and the files go back to how they stood at this reply. ${SHARED_FOLDER} ${undo}; the conversation does not come back.`;
// }
/// What a rewind to a reply takes, as `rewind_note` reads it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RewindNote {
    pub turns: f64,
    pub files: bool,
    pub cuts_conversation: bool,
    pub agent: String,
    pub kept: Option<String>,
}

pub fn rewind_note(o: &RewindNote) -> String {
    let undo = "Undo rewind puts them back until the next turn in this folder ends";
    if let Some(kept) = &o.kept {
        return format!(
            "{}, so the conversation stays and the files go back to how they stood at this reply. {SHARED_FOLDER} {undo}.",
            upper_first(kept)
        );
    }
    if !o.cuts_conversation {
        return format!("{} keeps its own history, so the conversation stays and the files go back to how they stood at this reply. {SHARED_FOLDER} {undo}.", o.agent);
    }
    if !o.files {
        return format!("{} the conversation; the files stay as they are.", turns_after(o.turns));
    }
    format!("{} the conversation, and the files go back to how they stood at this reply. {SHARED_FOLDER} {undo}; the conversation does not come back.", turns_after(o.turns))
}

/// A rewind to a turn a fork carried over from its thread: the fork's own history starts after it.
// thread.ts:539
pub const REWIND_COPIED_LINE: &str =
    "that reply came with the fork from its thread, so a rewind can go back only to the fork's own replies";

/// What a fork's tile card and the rule line after its copied history say, naming the thread it came from.
// thread.ts:542
// export const forkedFromLine = (title: string): string => `Forked from ${title}`;
pub fn forked_from_line(title: &str) -> String {
    format!("Forked from {title}")
}

/// The banner over a fork's draft, before anything of it exists.
// thread.ts:545
// export const forkOfLine = (title: string): string => `Fork of ${title}`;
pub fn fork_of_line(title: &str) -> String {
    format!("Fork of {title}")
}

/// The fork dialog's line under its title, and each pick's note.
// thread.ts:548
// export const forkDialogLine = (title: string): string => `A new thread with this conversation up to here. ${title} does not change.`;
pub fn fork_dialog_line(title: &str) -> String {
    format!("A new thread with this conversation up to here. {title} does not change.")
}

// thread.ts:549
// export const forkFolderNote = (title: string): string => `The files as they are now, shared with ${title}.`;
pub fn fork_folder_note(title: &str) -> String {
    format!("The files as they are now, shared with {title}.")
}

// thread.ts:550
pub const FORK_BRANCH_NOTE: &str = "A worktree with the files as this turn left them.";

// thread.ts:551
pub const FORK_BRANCH_HEAD_NOTE: &str = "A worktree at today's commit, with this turn's files on top.";

/// Where the fork's draft will run, beside its banner, and the fork dialog's picks and acts by name.
// thread.ts:553
// export const forkWhereLine = (branch: string | undefined): string => (branch === undefined ? "in this folder" : `on ${branch}`);
pub fn fork_where_line(branch: Option<&str>) -> String {
    match branch {
        None => "in this folder".to_string(),
        Some(b) => format!("on {b}"),
    }
}

// thread.ts:554
// export const FORK_PICK_WORDS = { folder: "This folder", branch: "New branch", fork: "Fork", title: "Fork from here" } as const;
pub mod fork_pick_words {
    pub const FOLDER: &str = "This folder";
    pub const BRANCH: &str = "New branch";
    pub const FORK: &str = "Fork";
    pub const TITLE: &str = "Fork from here";
}

/// The branch a fork's worktree takes unless one is named: its source's branch with -fork, numbered past every name taken,
/// or the source thread's short id on a detached head.
// thread.ts:557
// export function forkBranchName(source: string | undefined, taken: ReadonlySet<string>, threadId: string): string {
//   const base = source === undefined || source === "" ? `fork-${threadId.slice(0, 8)}` : `${source}-fork`;
//   if (!taken.has(base)) return base;
//   for (let n = 2; ; n++) if (!taken.has(`${base}-${n}`)) return `${base}-${n}`;
// }
pub fn fork_branch_name(source: Option<&str>, taken: &indexmap::IndexSet<String>, thread_id: &str) -> String {
    let base = match source {
        None | Some("") => format!("fork-{}", utf16::cut(thread_id, 8)),
        Some(s) => format!("{s}-fork"),
    };
    if !taken.contains(base.as_str()) {
        return base;
    }
    let mut n = 2;
    loop {
        let candidate = format!("{base}-{n}");
        if !taken.contains(candidate.as_str()) {
            return candidate;
        }
        n += 1;
    }
}

/// A fork refused for naming what its thread already decides.
// thread.ts:564
pub const FORK_BESIDE_LINE: &str = "a fork starts in its thread's folder, or on a new branch with --branch";

// thread.ts:565
pub const FORK_BESIDE_FIX: &str = "Leave out the project, --cwd, --beside and --replaces.";

/// A fork beside --resume: one carries a wsp thread's conversation, the other one an agent kept outside wsp.
// thread.ts:567
pub const FORK_RESUME_LINE: &str =
    "a fork carries a thread's conversation and --resume carries one kept outside wsp, so a start takes one of them";

// thread.ts:568
pub const FORK_RESUME_FIX: &str = "Name the thread with --fork, or the conversation with --resume and --copy, not both.";

/// --at with no thread to fork, or a count that is no turn.
// thread.ts:571
pub const AT_WITHOUT_FORK_LINE: &str = "--at counts the turns of the thread --fork names";

// thread.ts:572
pub const AT_WITHOUT_FORK_FIX: &str = "Name the thread with --fork, or leave --at out.";

// thread.ts:573
// export const atNotATurnLine = (at: string): string => `--at takes a finished turn's number from 1, not ${at}`;
pub fn at_not_a_turn_line(at: &str) -> String {
    format!("--at takes a finished turn's number from 1, not {at}")
}

/// A fork refused for naming another agent than its thread's: no agent opens another agent's session.
// thread.ts:576
// export const forkAgentLine = (agent: string, asked: string): string => `a fork runs on its thread's agent, ${agent}, not ${asked}`;
pub fn fork_agent_line(agent: &str, asked: &str) -> String {
    format!("a fork runs on its thread's agent, {agent}, not {asked}")
}

// thread.ts:577
// export const forkAgentFix = (asked: string): string => `Leave the agent out, or open a new thread on ${asked}.`;
pub fn fork_agent_fix(asked: &str) -> String {
    format!("Leave the agent out, or open a new thread on {asked}.")
}

/// A fork refused on an agent that keeps no road to fork its sessions.
// thread.ts:580
// export const forksNotLine = (agent: string): string => `${agent} cannot fork a thread`;
pub fn forks_not_line(agent: &str) -> String {
    format!("{agent} cannot fork a thread")
}

// thread.ts:581
pub const FORKS_NOT_FIX: &str = "Open a new thread instead.";

/// A fork refused through a turn still running: a fork carries finished turns alone.
// thread.ts:584
pub const FORK_RUNNING_LINE: &str = "that turn is still running, and a fork carries finished turns alone";

// thread.ts:585
pub const FORK_RUNNING_FIX: &str = "Fork from an earlier reply, or wait for the turn to end.";

/// A fork refused for a turn the thread does not hold, counted as `--at` counts.
// thread.ts:588
// export const forkTurnsLine = (finished: number): string => (finished === 0 ? "that thread has no finished turn to fork from" : `that thread has ${finished} finished ${finished === 1 ? "turn" : "turns"}`);
pub fn fork_turns_line(finished: f64) -> String {
    if finished == 0.0 {
        "that thread has no finished turn to fork from".to_string()
    } else {
        let noun = if finished == 1.0 { "turn" } else { "turns" };
        format!("that thread has {} finished {noun}", number_string(finished))
    }
}

// thread.ts:589
// export const forkTurnsFix = (finished: number): string => (finished === 0 ? "Wait for its first turn to end." : `Name one from 1 to ${finished} with --at.`);
pub fn fork_turns_fix(finished: f64) -> String {
    if finished == 0.0 {
        "Wait for its first turn to end.".to_string()
    } else {
        format!("Name one from 1 to {} with --at.", number_string(finished))
    }
}

// thread.ts:590
// export const forkNoTurnLine = (turnId: string): string => `that thread holds no turn ${turnId}`;
pub fn fork_no_turn_line(turn_id: &str) -> String {
    format!("that thread holds no turn {turn_id}")
}

// thread.ts:591
pub const FORK_NO_TURN_FIX: &str = "Fork from a reply the thread shows.";

/// A fork refused at a turn whose agent named no point to cut its conversation at.
// thread.ts:594
// export const forkNoAnchorLine = (agent: string): string => `${agent} left no point in that turn to fork its conversation at`;
pub fn fork_no_anchor_line(agent: &str) -> String {
    format!("{agent} left no point in that turn to fork its conversation at")
}

// thread.ts:595
pub const FORK_NO_ANCHOR_FIX: &str = "Fork from another reply.";

/// A fork the agent itself refused, in its own words.
// thread.ts:598
// export const forkRefusedLine = (agent: string, why: string): string => `${agent} could not fork the conversation: ${why}`;
pub fn fork_refused_line(agent: &str, why: &str) -> String {
    format!("{agent} could not fork the conversation: {why}")
}

// thread.ts:599
pub const FORK_REFUSED_FIX: &str = "Fork from another reply, or send it in the thread itself.";

/// A fork onto a new branch whose turn's checkpoint of the files is gone, or never was.
// thread.ts:602
pub const FORK_NO_CHECKPOINT_LINE: &str = "that turn kept no checkpoint of the files to start a branch from";

// thread.ts:603
pub const FORK_NO_CHECKPOINT_FIX: &str = "Fork it in this folder instead, without --branch.";

/// The refusal of a send into a thread that names another agent. A thread's rows carry the agent its turns ran on
/// and the harness session those turns wrote, which another agent would open as a transcript of its own, at its own
/// access; the agent is picked where a thread is opened, so a second one is a second thread.
// thread.ts:608
// export function threadRunsOnLine(agent: string, asked: string): string {
//   return `this thread runs on ${agent}; open a new thread to run ${asked}`;
// }
pub fn thread_runs_on_line(agent: &str, asked: &str) -> String {
    format!("this thread runs on {agent}; open a new thread to run {asked}")
}

/// The refusal of a thread opened on no words: an empty or whitespace task would still start a process and a turn.
// thread.ts:613
pub const EMPTY_MESSAGE_LINE: &str = "the message is empty; say what the thread is to do";

/// The refusal of a rename to nothing: a blank name would take a thread's title away and leave nothing in its place.
// thread.ts:616
pub const EMPTY_TITLE_LINE: &str = "the name is empty; say what the thread is called";

/// The one line a codex turn fails with when its provider wants an OpenAI login the machine has not got: the CLI
/// itself only retries the 401 and dies. `login` is the catalog's command for signing in on a machine. It is also
/// what the composer's model menu shows in place of its footer's source line, which is why it names the workspace
/// rather than the machine it runs on.
// thread.ts:622
// export function codexNotSignedInLine(login: string): string {
//   return `Codex is not signed in where this thread runs; run ${login} there`;
// }
pub fn codex_not_signed_in_line(login: &str) -> String {
    format!("Codex is not signed in where this thread runs; run {login} there")
}

/// The line when codex's provider reads its key from an environment variable the machine does not set.
// thread.ts:627
// export function codexMissingEnvLine(name: string): string {
//   return `Codex's model provider reads its key from the environment variable ${name}, which is not set on this machine`;
// }
pub fn codex_missing_env_line(name: &str) -> String {
    format!("Codex's model provider reads its key from the environment variable {name}, which is not set on this machine")
}

/// The one line under the composer's model lists: the binary that filled them, else whose table stood in and what it
/// was pinned from, or the adapter's own words for why the binary gave nothing. Every word comes from the catalog being
/// shown, so a tab never borrows another agent's binary, reason or pin. `where` is the word for where the turn runs,
/// which the caller reads off the workspace: the binary the line names is the one on that computer and on no other.
/// The slot is one line at the popup's width, 290px of the 10px mono it draws in, so the agent is named once and the
/// pin's own words carry the rest.
// thread.ts:637
// export function catalogSourceLine(catalog: HarnessCatalog, where: string): string {
//   const version = catalog.version;
//   if (catalog.source === "harness") return `${catalog.label}${version === null ? "" : ` ${version}`} on ${where}`;
//   const why = catalog.refusal ?? `${catalog.harness} table`;
//   return version === null ? why : `${why}, ${version}`;
// }
pub fn catalog_source_line(catalog: &HarnessCatalog, r#where: &str) -> String {
    let version = catalog.version.as_deref();
    if catalog.source == HarnessCatalogSource::Harness {
        let shown = version.map_or(String::new(), |v| format!(" {v}"));
        return format!("{}{shown} on {}", catalog.label, r#where);
    }
    let why = catalog.refusal.clone().unwrap_or_else(|| format!("{} table", catalog.harness));
    match version {
        None => why,
        Some(v) => format!("{why}, {v}"),
    }
}

/// The one line in place of the model rows: what the binary reported, or what the table holds, and never a count the
/// source did not give.
// thread.ts:646
// export function noModelsLine(catalog: HarnessCatalog): string {
//   return catalog.source === "harness" ? `${catalog.label} reported no models` : `No model in the ${catalog.label} table`;
// }
pub fn no_models_line(catalog: &HarnessCatalog) -> String {
    if catalog.source == HarnessCatalogSource::Harness {
        format!("{} reported no models", catalog.label)
    } else {
        format!("No model in the {} table", catalog.label)
    }
}

/// The line when codex kept reconnecting to its provider and nothing ever answered: the CLI itself never gives up.
// thread.ts:651
// export function codexReconnectLine(elapsedMs: number): string {
//   return `stopped after ${fmtDuration(elapsedMs, "clock")} of Codex reconnecting to its model provider with no answer`;
// }
pub fn codex_reconnect_line(elapsed_ms: f64) -> String {
    format!(
        "stopped after {} of Codex reconnecting to its model provider with no answer",
        fmt_duration(elapsed_ms, Some(DurationStyle::Clock))
    )
}

/// What the daemon last read of the guest's memory and load before its link went quiet.
// thread.ts:656
// export interface MemoryReading {
//   used: number;
//   total: number;
//   load1: number;
// }
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MemoryReading {
    pub used: f64,
    pub total: f64,
    pub load1: f64,
}

/// The share of memory in use past which the kernel's killer is one allocation away: 3.59 of 3.94 GB (91 percent)
/// when a build took a daemon, measured 2026-09-06.
// thread.ts:664
pub const MEMORY_NEAR_FULL: f64 = 0.9;

// thread.ts:666
// export function memoryNearFull(mem: { used: number; total: number }): boolean {
//   return mem.total > 0 && mem.used / mem.total >= MEMORY_NEAR_FULL;
// }
pub fn memory_near_full(used: f64, total: f64) -> bool {
    total > 0.0 && used / total >= MEMORY_NEAR_FULL
}

/// The line every pane and row shows when a machine stopped answering with its memory near full: the last figures
/// the daemon sent, and that the work took the memory, so nobody rebuilds a machine that is fine.
// thread.ts:672
// export function outOfMemoryLine(r: MemoryReading): string {
//   return `Out of memory (${fmtBytes(r.used)} of ${fmtBytes(r.total)} used, load ${r.load1.toFixed(1)}) when the thread's computer last answered; the work there took the memory, not a fault in the computer`;
// }
pub fn out_of_memory_line(r: &MemoryReading) -> String {
    format!(
        "Out of memory ({} of {} used, load {}) when the thread's computer last answered; the work there took the memory, not a fault in the computer",
        fmt_bytes(r.used),
        fmt_bytes(r.total),
        to_fixed(r.load1, 1),
    )
}

/// The sidebar row's form of the same fact, in the shape the daemon note takes: the row's second line is about
/// thirty characters wide, so the sentence above would be cut at the figures.
// thread.ts:678
// export function outOfMemoryRowLine(r: MemoryReading): string {
//   return `out of memory, ${fmtBytesOf(r.used, r.total)}`;
// }
pub fn out_of_memory_row_line(r: &MemoryReading) -> String {
    format!("out of memory, {}", fmt_bytes_of(r.used, r.total, None))
}

/// What to do about it, shown ahead of any rebuild: the smallest size in the provider's table with more memory than
/// this machine, with its rate, for the next workspace. With none in the table, less at once is the only road.
// thread.ts:684
// export function biggerSizeLine(current: WorkspaceSize, offers: readonly MachineSizeOffer[]): string {
//   const bigger = offers.filter(o => o.memMb > current.memMb).sort((a, b) => a.memMb - b.memMb)[0];
//   if (bigger === undefined) return "No size with more memory is offered; run less in the thread at once";
//   return `A thread on ${fmtSize(bigger)} (${fmtRate(bigger.rateUsdPerHour)}) fits more; pick it when you start the next one`;
// }
pub fn bigger_size_line(current: &WorkspaceSize, offers: &[MachineSizeOffer]) -> String {
    let bigger = offers.iter().filter(|o| o.mem_mb.0 > current.mem_mb.0).min_by(|a, b| a.mem_mb.0.total_cmp(&b.mem_mb.0));
    match bigger {
        None => "No size with more memory is offered; run less in the thread at once".to_string(),
        Some(b) => {
            let size = WorkspaceSize { cpu: JsNumber(b.cpu.0), mem_mb: JsNumber(b.mem_mb.0) };
            format!(
                "A thread on {} ({}) fits more; pick it when you start the next one",
                fmt_size(&size, None),
                fmt_rate(b.rate_usd_per_hour.0)
            )
        }
    }
}

/// The machine row's line while the runtime replaces a daemon older than this wsp, and the line it shows instead
/// when the replacement failed. A person is never told the helper is called a daemon: they did not install it and
/// cannot run it, so its name would only be one more thing to know. Neither line carries the reason a deploy gave:
/// that is an npm log a person can do nothing with, hundreds of characters wide in a row that fits about thirty,
/// and it names the daemon in its own words. The runtime logs it for whoever runs the host.
// thread.ts:695
pub const DAEMON_UPDATING: &str = "updating the helper";

// thread.ts:696
pub const DAEMON_UPDATE_FAILED: &str = "could not update the helper";

/// The same two lines for the daemon the runtime is putting back after the machine answered with none, in the
/// words of the thing a person is waiting on: the row is grey because nothing answers, and this says wsp is on it
/// rather than that the machine is lost.
// thread.ts:701
pub const DAEMON_RESTARTING: &str = "restarting the helper";

// thread.ts:702
pub const DAEMON_RESTART_FAILED: &str = "could not restart the helper";

/// The same two lines for the first daemon a machine ever takes. A machine somebody already owned had none until
/// wsp put one there, so nothing about it is being updated or put back, and a person watching that machine's row
/// is told what is happening on it rather than that something they never installed is being replaced.
// thread.ts:707
pub const DAEMON_INSTALLING: &str = "installing the helper";

// thread.ts:708
pub const DAEMON_INSTALL_FAILED: &str = "could not install the helper";

/// Why a nap's vault export was refused: its size against the cap, both in the one byte rule.
// thread.ts:711
// export function vaultOverCapLine(bytes: number, capBytes: number): string {
//   return `the export was ${fmtBytes(bytes)}, over the ${fmtBytes(capBytes)} cap`;
// }
pub fn vault_over_cap_line(bytes: f64, cap_bytes: f64) -> String {
    format!("the export was {}, over the {} cap", fmt_bytes(bytes), fmt_bytes(cap_bytes))
}

/// A store an agent keeps for every project that an export could not read on the machine: nothing from it travelled,
/// so the project's rows in it stayed there rather than every other project's leaving with them.
// thread.ts:717
// export function storeUnreadLine(store: string, why: string): string {
//   return `could not read ${store} on the machine, so nothing from it travelled: ${why}`;
// }
pub fn store_unread_line(store: &str, why: &str) -> String {
    format!("could not read {store} on the machine, so nothing from it travelled: {why}")
}

/// What a landing is refused with when the agents' state a machine answered with holds something that is neither a
/// folder nor a regular file, or a path that reaches out of the folder it was opened in: the entry as it sits in the
/// archive and what it is. The bytes are the machine's, the landing is the person's, and one such entry is a road
/// into the agents' own files on this computer, so nothing of that archive lands.
// thread.ts:725
// export function stateEntryRefusal(entry: string, what: string): string {
//   return `the agents' state from the machine holds ${entry}, which is ${what}; nothing of it was landed`;
// }
pub fn state_entry_refusal(entry: &str, what: &str) -> String {
    format!("the agents' state from the machine holds {entry}, which is {what}; nothing of it was landed")
}

/// The verdict when a nap could not store a fresh backup, said once with whatever the machine answered on the
/// line's title: the machine's own words name folders and commands nobody asked for, and a person reading this
/// needs to know where their files stand. The second clause is what is true of this workspace: an earlier nap's
/// backup is what a rebuild would restore, older than the files the person left, and a workspace whose naps have
/// never stored one has nothing off the machine at all.
// thread.ts:734
// export function vaultKeptLine(w: Pick<WorkspaceView, "vaultedAt">): string {
//   return `the nap saved no backup; ${w.vaultedAt === undefined ? "nothing is saved off the machine" : "what was saved before is kept"}`;
// }
/// `vaulted_at` is the workspace view's `vaultedAt`.
pub fn vault_kept_line(vaulted_at: Option<&str>) -> String {
    let said = if vaulted_at.is_none() { "nothing is saved off the machine" } else { "what was saved before is kept" };
    format!("the nap saved no backup; {said}")
}

/// The verdict when a guest refused the hostname the fork asked for. Naming a fork is cosmetic, so the create goes
/// on and the workspace answers to the name the machine booted with; the guest's own refusal rides the title.
// thread.ts:740
pub const HOSTNAME_KEPT: &str = "hostname not set; the workspace keeps the machine's own name";

/// The same step where the guest took the name, in the one form the creation log's lines are written in: lower
/// case, no full stop, the workspace and never the machine's own id.
// thread.ts:744
// export function hostnameSetLine(host: string): string {
//   return `hostname set to ${host}`;
// }
pub fn hostname_set_line(host: &str) -> String {
    format!("hostname set to {host}")
}

/// The creation log's line for the fork itself, in the words the app says a workspace and a computer in.
// thread.ts:749
// export function startingLine(name: string, where: string): string {
//   return `starting ${name} on ${where}`;
// }
pub fn starting_line(name: &str, r#where: &str) -> String {
    format!("starting {name} on {}", r#where)
}

/// A copy's first line on the computer the host runs on: the folder it is made of and the folder it becomes.
// thread.ts:754
// export const copyingFolderLine = (from: string, to: string): string => `copying ${from} to ${to}`;
pub fn copying_folder_line(from: &str, to: &str) -> String {
    format!("copying {from} to {to}")
}

/// How long a build is said to take where none has been measured.
// thread.ts:757
pub const BUILD_TAKES_UNMEASURED: &str = "about ten minutes";

/// The creation log's first line for a fork at a place that holds no current copy of the image: the copy is built
/// there before the fork, which takes a build's time and, where the place charges, a builder's hours.
// thread.ts:761
// export function copyFirstLine(where: string, name: string, rateUsdPerHour: number): string {
//   const billed = chargesNothing(rateUsdPerHour) ? "" : `, billed at ${fmtRate(rateUsdPerHour)} while it builds`;
//   return `building your image on ${where} first, ${BUILD_TAKES_UNMEASURED}${billed}, then ${name} forks from it`;
// }
pub fn copy_first_line(r#where: &str, name: &str, rate_usd_per_hour: f64) -> String {
    let billed = if charges_nothing(Some(rate_usd_per_hour)) {
        String::new()
    } else {
        format!(", billed at {} while it builds", fmt_rate(rate_usd_per_hour))
    };
    format!("building your image on {} first, {BUILD_TAKES_UNMEASURED}{billed}, then {name} forks from it", r#where)
}

/// The last line of a create, as the word table ends it.
// thread.ts:767
pub const CREATE_READY: &str = "ready";

// The day of a stamp in UTC, which is as far as this fact goes: the vault that stands can be days old, and the
// time of day is noise on a row about thirty characters wide.
// thread.ts:771
// private in the TS: port it as a private helper
// const onDay = (iso: string): string => iso.slice(0, 10);

// thread.ts:773
// private in the TS: port it as a private helper
// const staleWord = (w: Pick<WorkspaceView, "vaultedAt">): string => (w.vaultedAt === undefined ? "no backup" : `no backup since ${onDay(w.vaultedAt)}`);

/// The row's and the Machine tab's word for a machine whose last nap could not store a fresh vault: a rebuild
/// restores the vault that still stands, which is as old as this says, and a machine that never stored one has
/// nothing to restore at all. Null while the last nap stored its vault, which is every machine's steady state. One
/// form on every surface, short enough that the sidebar row shows all of it: the sizes it was refused for are on
/// the Machine tab's own line, not in here.
// thread.ts:780
// export function vaultStaleLine(w: Pick<WorkspaceView, "vaultedAt" | "vaultRefused">): string | null {
//   return w.vaultRefused === undefined ? null : staleWord(w);
// }
/// None is the TS null.
pub fn vault_stale_line(vaulted_at: Option<&str>, vault_refused: Option<&str>) -> Option<String> {
    vault_refused.map(|_| match vaulted_at {
        None => "no backup".to_string(),
        Some(at) => format!("no backup since {}", utf16::cut(at, 10)),
    })
}

/// Which call found the provider no longer knew a record's machine: the status poll's read, a pause, a wake's read,
/// the sweep's read of a machine its listing lacked, or the record load at host start.
// thread.ts:786
// export type GoneSeenBy = "status poll" | "pause" | "wake" | "sweep" | "record load";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GoneSeenBy {
    #[serde(rename = "status poll")]
    StatusPoll,
    #[serde(rename = "pause")]
    Pause,
    #[serde(rename = "wake")]
    Wake,
    #[serde(rename = "sweep")]
    Sweep,
    #[serde(rename = "record load")]
    RecordLoad,
}

impl GoneSeenBy {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            GoneSeenBy::StatusPoll => "status poll",
            GoneSeenBy::Pause => "pause",
            GoneSeenBy::Wake => "wake",
            GoneSeenBy::Sweep => "sweep",
            GoneSeenBy::RecordLoad => "record load",
        }
    }
}

/// One sighting of a machine gone at the provider: who saw it, when (epoch ms), and the provider's answer to that
/// call when it answered in words (its status and message); a state read that came back gone carries none.
// thread.ts:790
// export interface GoneSighting {
//   by: GoneSeenBy;
//   at: number;
//   answer?: string;
// }
#[derive(Clone, Debug, PartialEq)]
pub struct GoneSighting {
    pub by: GoneSeenBy,
    pub at: f64,
    pub answer: Option<String>,
}

/// A moment as a person reads it in a line of wsp's own: UTC to the second, since the milliseconds are noise in a
/// sentence and a second is what a reader compares two lines by. The one spelling, read wherever a line carries a
/// time of its own.
// thread.ts:799
// export const isoSeconds = (at: number): string => new Date(at).toISOString().replace(/\.\d+Z$/, "Z");
pub fn iso_seconds(at: f64) -> String {
    let moment = jiff::Timestamp::from_millisecond(at as i64).expect("a moment jiff holds");
    moment.to_zoned(jiff::tz::TimeZone::UTC).strftime("%Y-%m-%dT%H:%M:%SZ").to_string()
}

/// What a record says about a machine the provider stopped knowing: which call found it gone and the second it did,
/// quoting the provider where it said anything. Without a sighting, only that it is gone.
// thread.ts:803
// export function goneWords(machineId: string, seen?: GoneSighting): string {
//   const base = `machine ${machineId} is gone at the provider`;
//   if (seen === undefined) return base;
//   const at = isoSeconds(seen.at);
//   const answer = seen.answer === undefined || seen.answer === "" ? "" : ` (${seen.answer})`;
//   return `${base}: the ${seen.by} found it gone at ${at}${answer}`;
// }
pub fn gone_words(machine_id: &str, seen: Option<&GoneSighting>) -> String {
    let base = format!("machine {machine_id} is gone at the provider");
    let Some(seen) = seen else {
        return base;
    };
    let answer = match seen.answer.as_deref() {
        None | Some("") => String::new(),
        Some(answer) => format!(" ({answer})"),
    };
    format!("{base}: the {} found it gone at {}{answer}", seen.by.as_str(), iso_seconds(seen.at))
}

/// The machine row's line when a record that said paused met a machine the provider was running all along (a nap
/// whose pause never took, a resume nobody wrote): the record followed the fact and nothing was resumed.
// thread.ts:813
pub const ALREADY_RUNNING: &str = "already running at the provider";

/// The machine row's line when a record marked gone met a machine the provider still holds, running or paused: the
/// state read by id decides, so the record is gone no more and no rebuild abandoned a healthy machine.
// thread.ts:817
pub const NOT_GONE: &str = "not gone at the provider after all; the record follows the state read";

/// The machine row's line when a 404 was to be confirmed and the reads that followed failed: nothing is known
/// either way, so the record keeps its word and the next sweep asks again.
// thread.ts:821
pub const GONE_UNCHECKED: &str = "not known to be gone; wsp could not reach the provider to check it";

/// What a record held on one 404 at host start answers until the provider has been read again.
// thread.ts:824
// export const goneUnconfirmedLine = (machineId: string, answer: string): string =>
//   `machine ${machineId} answered ${answer} when this host started; wsp is asking the provider again before it calls it gone`;
pub fn gone_unconfirmed_line(machine_id: &str, answer: &str) -> String {
    format!("machine {machine_id} answered {answer} when this host started; wsp is asking the provider again before it calls it gone")
}

/// The machine row's line on a workspace the sweep recorded from the provider's listing: a machine of this setup's
/// that no record claimed, kept rather than killed, since a machine nobody records bills unseen.
// thread.ts:829
pub const RECORD_RESTORED: &str = "record restored from the provider's listing";

/// The host's line for it, with the name the fork stamped on the machine (or the machine id where it stamped none)
/// and the verb that removes it.
// thread.ts:833
// export function recordRestoredLine(machineId: string, name: string, workspaceId: string): string {
//   return `reap: recorded ${machineId} as workspace ${name} (${workspaceId}): a machine from this setup that no record claimed; it bills until wsp delete ${name}`;
// }
pub fn record_restored_line(machine_id: &str, name: &str, workspace_id: &str) -> String {
    format!("reap: recorded {machine_id} as workspace {name} ({workspace_id}): a machine from this setup that no record claimed; it bills until wsp delete {name}")
}

/// The refusal a fork gets for a name another workspace holds; a name names at most one workspace.
// thread.ts:838
// export function nameTakenRefusal(name: string): string {
//   return `${name} is already a workspace; pick another name, or delete it first`;
// }
pub fn name_taken_refusal(name: &str) -> String {
    format!("{name} is already a workspace; pick another name, or delete it first")
}

/// The refusal a fork gets for a name whose workspace is being deleted this moment: the two never interleave.
// thread.ts:843
// export function nameDeletingRefusal(name: string): string {
//   return `${name} is being deleted; wait for the delete to finish, then fork it again`;
// }
pub fn name_deleting_refusal(name: &str) -> String {
    format!("{name} is being deleted; wait for the delete to finish, then fork it again")
}

/// A delete whose machine the provider still reads after three asks: the record stays, so the machine is still named.
// thread.ts:848
// export function deleteRefusedLine(name: string, machineId: string, state: MachineState): string {
//   return `${name}'s machine ${machineId} is still ${state} after three asks; run wsp delete again or delete it at the provider`;
// }
/// `state` is a MachineState's wire word.
pub fn delete_refused_line(name: &str, machine_id: &str, state: &str) -> String {
    format!("{name}'s machine {machine_id} is still {state} after three asks; run wsp delete again or delete it at the provider")
}

/// The refusal a fork or a rename gets for a blank name: a person and an agent both address a workspace by its name.
// thread.ts:853
pub const BLANK_NAME_REFUSAL: &str = "a workspace name cannot be blank";

/// The one sentence every machine road answers with on a computer set up with no machine provider key: wsp init took
/// the local road, so this computer is a workspace and there is nothing to fork, pause or seal until a provider is
/// added. The provider module a keyless host wires says it, and so does the command line before it asks for anything.
///
/// It ends on the verb off the front page that fixes it, never on a shell variable: this is the first refusal a
/// person who typed wsp new on a fresh home reads, and a variable named there is neither one of the sixteen words
/// nor true of the other providers. wsp add with no argument prints the words it takes.
// thread.ts:862
pub const NO_PROVIDER_LINE: &str = "no machine provider is set up on this computer, so wsp forks no machines here; wsp add <provider> connects one, and wsp init then builds your image on it";

/// The same refusal where no provider can be added at all: what is missing, and the one road that brings it.
// thread.ts:865
pub const NO_MACHINES_LINE: &str =
    "this needs a machine wsp starts, and nothing here starts one; wsp add user@host joins a computer of yours that does";

/// What an image build is refused with when no place this host holds runs workspaces: a joined computer whose doctor
/// said yes is such a place, and so is a provider with a key. The init job and the command line beside it both say it.
// thread.ts:869
pub const NO_BUILD_PLACE_LINE: &str = "no place here runs workspaces, so there is nowhere to build your image; join a computer that runs them with wsp add, or save a provider key";

/// The same when the default place runs no workspaces and more than one other place does: the run does not guess.
// thread.ts:872
// export const buildPlaceAskLine = (names: readonly string[]): string => `${names.join(", ")} run workspaces and none of them is the default place; name the one to build your image on with --on`;
pub fn build_place_ask_line(names: &[impl AsRef<str>]) -> String {
    format!("{} run workspaces and none of them is the default place; name the one to build your image on with --on", join_names(names))
}

/// What a fork is refused with when the place it would land on forks nothing while other places run workspaces: the
/// places that do, so the sentence never sends a person to a provider they do not need. With none at all the
/// refusal is NO_PROVIDER_LINE.
// thread.ts:877
// export const placeForksNothingPickLine = (place: string, names: readonly string[]): string => `${place} forks no machines; name a place that runs workspaces with --on: ${names.join(", ")}`;
pub fn place_forks_nothing_pick_line(place: &str, names: &[impl AsRef<str>]) -> String {
    format!("{place} forks no machines; name a place that runs workspaces with --on: {}", join_names(names))
}

/// The line under wsp init's opening that names where the image is built, so the first screen says it.
// thread.ts:880
// export const imageBuiltOnLine = (place: string): string => `your image is built on ${place}`;
pub fn image_built_on_line(place: &str) -> String {
    format!("your image is built on {place}")
}

/// The line beside it when --on named a place other than the image's own: a rebuild never moves the image's home.
// thread.ts:883
// export const imageHomeKeptLine = (on: string | undefined, home: { id: string; name: string; kind?: string }): string | undefined =>
//   on === undefined || namesPlace(home, on) ? undefined : `your image lives on ${placeSaid(home)}, so it is built there and not on ${on}`;
/// A place as `namesPlace` and `placeSaid` read it.
pub type PlaceNamed = crate::place_word::PlaceRef;

pub fn image_home_kept_line(on: Option<&str>, home: &PlaceNamed) -> Option<String> {
    on.filter(|on| !names_place(home, on))
        .map(|on| format!("your image lives on {}, so it is built there and not on {on}", place_said(home)))
}

/// What a line that takes a thread answers a word naming no thread but a project, or with a cloud a machine: what the
/// word is, the line's own name and where its threads are listed. The lines that read an agent's config take a
/// computer too.
// thread.ts:889
// export function notAThreadLine(word: string, is: "project" | "machine", line: string, orComputer = false): string {
//   const takes = `${line} takes a thread${orComputer ? ", or a computer with --on" : ""}`;
//   return is === "project" ? `${word} is a project; ${takes}, as wsp threads ${word} lists them.` : `${word} is a machine; ${takes}, as wsp threads lists them.`;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WordIs {
    Project,
    Machine,
}

/// `orComputer` defaults to false.
pub fn not_a_thread_line(word: &str, is: WordIs, line: &str, or_computer: bool) -> String {
    let takes = format!("{line} takes a thread{}", if or_computer { ", or a computer with --on" } else { "" });
    match is {
        WordIs::Project => format!("{word} is a project; {takes}, as wsp threads {word} lists them."),
        WordIs::Machine => format!("{word} is a machine; {takes}, as wsp threads lists them."),
    }
}

/// Its fix, the same on the command line and the tool.
// thread.ts:895
pub const NAME_A_THREAD_FIX: &str = "Name a thread by its id.";

/// The line under a commit, a discard or an update where other threads work in the same folder, since the act was
/// the folder's and took in their changes too; each thread by the short id a person reads.
// thread.ts:899
// export function sharedFolderLine(others: readonly string[]): string {
//   return others.length === 1 ? `thread ${others[0]!} works in this folder too` : `threads ${others.slice(0, -1).join(", ")} and ${others.at(-1)!} work in this folder too`;
// }
pub fn shared_folder_line(others: &[impl AsRef<str>]) -> String {
    let names: Vec<&str> = others.iter().map(|o| o.as_ref()).collect();
    match names.split_last() {
        Some((last, [])) => format!("thread {last} works in this folder too"),
        Some((last, rest)) => format!("threads {} and {last} work in this folder too", rest.join(", ")),
        // JavaScript prints an empty list as `threads  and undefined work in this folder too`.
        None => "threads  and undefined work in this folder too".to_string(),
    }
}

/// A thread named to act in a folder it does not work in.
// thread.ts:904
// export const threadElsewhereLine = (threadId: string): string => `thread ${threadWord(threadId)} does not work in this folder`;
pub fn thread_elsewhere_line(thread_id: &str) -> String {
    format!("thread {} does not work in this folder", thread_word(thread_id))
}

/// A thread named to merge that works in the lead's own folder, its child or not: nothing of it is apart to merge.
// thread.ts:907
// export const childBesideLeadLine = (child: string, lead: string): string => `thread ${threadWord(child)} works in the same folder as thread ${threadWord(lead)}; there is nothing apart to merge.`;
pub fn child_beside_lead_line(child: &str, lead: &str) -> String {
    format!("thread {} works in the same folder as thread {}; there is nothing apart to merge.", thread_word(child), thread_word(lead))
}

// thread.ts:908
pub const CHILD_BESIDE_LEAD_FIX: &str = "Name a child that works in a folder of its own.";

/// A run beside a thread starts in that thread's folder, so it names no project and no branch of its own.
// thread.ts:911
pub const BESIDE_ALONE_LINE: &str =
    "--beside starts the thread in the folder another thread works in, so it takes no project and no --branch.";

/// Its fix where a tool was called, which has no usage line to show.
// thread.ts:913
pub const BESIDE_ALONE_FIX: &str = "Drop project and branch, or beside.";

/// What a run or a listing naming no project answers, and its fix.
// thread.ts:916
// export const noProjectLine = (word: string): string => `no project ${word}`;
pub fn no_project_line(word: &str) -> String {
    format!("no project {word}")
}

// thread.ts:917
pub const READ_PROJECTS_FIX: &str = "Run wsp projects to read the names.";

fn turns_after(turns: f64) -> String {
    if turns == 1.0 {
        "The turn after this reply leaves".to_string()
    } else {
        format!("The {} turns after this reply leave", number_string(turns))
    }
}

fn join_names(names: &[impl AsRef<str>]) -> String {
    let names: Vec<&str> = names.iter().map(|n| n.as_ref()).collect();
    names.join(", ")
}
