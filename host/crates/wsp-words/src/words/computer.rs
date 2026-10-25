// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/words/computer.ts at a3b870a6e: what this computer, a joined computer and a fork are
//! called, every refusal a thread's own token, a guest line and a paired device meet, the host-key and root-shell
//! checks of a computer being added, and the lines a pause, a wake and a stop on a joined computer end with. Each TS
//! statement stands as a comment above its stub. See spec/computer.md.

use crate::exit::HostError;
use crate::js::{is_js_whitespace, js_trim, number_string, utf16};
use crate::words::base::{marked, plural};
use crate::words::places::spend_meter_word;
use crate::words::units::fmt_duration;
use wsp_wire::types::shell_quote::shell_line;
use wsp_wire::wire::capabilities::Capabilities;

/// The platform a sentence names this computer for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    Darwin,
    Linux,
}

/// What a local workspace's machine is, in every sentence and every row that names it: the refusals below, the
/// sidebar row's second line and the Machine tab's lineage all read this one phrase.
// computer.ts:9
pub const THIS_COMPUTER: &str = "this computer";

/// The one sentence for a project a run just recorded on this computer: wsp init's last tick says it, so where its
/// threads run is named the same way whichever road wrote the record.
// computer.ts:13
// export const thisComputerLine = (name: string, folder: string): string => `Threads on ${name} run in ${folder} on ${THIS_COMPUTER}, under your own sign-ins.`;
pub fn this_computer_line(name: &str, folder: &str) -> String {
    format!("Threads on {name} run in {folder} on {THIS_COMPUTER}, under your own sign-ins.")
}

/// What a place's machine is, in every sentence and every row that names it: a computer of the person's own that
/// dialled this host and holds the link, so wsp drives it with the daemon protocol and never made it.
// computer.ts:17
pub const JOINED_COMPUTER: &str = "a computer you joined";

/// What a cloud workspace's machine is in a sentence that names it: a machine wsp forked at a provider and pays for,
/// whether the provider runs virtual machines or containers. A refusal on one says this rather than borrowing this
/// computer's words, since what it cannot do is the provider's limit and not the machine being the person's own.
// computer.ts:22
pub const MACHINE_WSP_FORKS: &str = "a machine wsp forks";

/// What the computer wsp is reading is called on the screens that read it: a Mac by the name its owner uses for it,
/// any other computer the plain word. The one place that word is decided, so no screen tells a Linux reader the tool
/// was built for somebody else.
// computer.ts:27
// export const thisComputer = (platform: "darwin" | "linux"): string => (platform === "darwin" ? "this Mac" : THIS_COMPUTER);
pub fn this_computer(platform: Platform) -> String {
    match platform {
        Platform::Darwin => "this Mac".to_string(),
        Platform::Linux => THIS_COMPUTER.to_string(),
    }
}

/// Whether the machine that reported this system name is a Mac: the name its maker gives it, and the kernel's own
/// word where the machine answered nothing better, which is what a machine on this computer falls back to. The
/// folder browsers read it beside the home to know whether that home keeps a Library. Absent is not a Mac: what
/// reads this hides a folder, and a machine that said nothing has said nothing to hide.
// computer.ts:33
// export function isMacMachine(osName: string | null | undefined): boolean {
//   return /^(?:macOS|Darwin)\b/.test(osName ?? "");
// }
pub fn is_mac_machine(os_name: Option<&str>) -> bool {
    let text = os_name.unwrap_or("");
    ["macOS", "Darwin"]
        .iter()
        .any(|word| text.strip_prefix(word).is_some_and(|rest| !rest.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '_')))
}

/// The computer the host runs on as a row names it, off what that machine itself reported: the same word every
/// screen that names this computer uses, so a table and the settings list cannot call one computer two things.
// computer.ts:39
// export const computerWord = (os: string | null | undefined): string => thisComputer(isMacMachine(os) ? "darwin" : "linux");
pub fn computer_word(os: Option<&str>) -> String {
    this_computer(if is_mac_machine(os) { Platform::Darwin } else { Platform::Linux })
}

/// What a workspace's copy of its project is, in the words a person uses for it: a copy of the folder. The one home
/// for the word, read by the first-run screen before any road is taken and by every row after.
// computer.ts:43
pub const COPY_WORD: &str = "a copy";

/// The word for what a workspace made by this road is. A directory clone and a git worktree are two roads to the
/// one thing a person reads, so both read the same; which road was taken, and why one was passed over, rides the
/// row's hover text. The one home for the word, so a row in the app and a cell in the command line's table cannot
/// say two things about one workspace.
// computer.ts:49
// export const madeOfWord = (_road: CopyRoad): string => COPY_WORD;
/// `road` is a CopyRoad's wire word; every road reads the same.
pub fn made_of_word(_road: &str) -> String {
    COPY_WORD.to_string()
}

/// What a computer's threads have for a network: their own where the computer gives them one, else the computer's own
/// ports, shared as panes in one terminal share them. Nothing at all on a computer that copies nothing. Reads the two
/// capability flags and nothing about the kind, so a computer that gains its own network changes one flag.
// computer.ts:54
// export function portsWord(c: Pick<Capabilities, "copies" | "ownNetwork">, computer: string): string {
//   if (c.ownNetwork) return "own network";
//   if (!c.copies) return "";
//   return `shares ${computer}'s ports`;
// }
pub fn ports_word(c: &Capabilities, computer: &str) -> String {
    if c.own_network {
        "own network".to_string()
    } else if !c.copies {
        String::new()
    } else {
        format!("shares {computer}'s ports")
    }
}

/// What the browser pane says for a port it cannot open: the computer it runs on gives this pane no address, and
/// the address that does answer is the one a terminal on that computer opens. Written here because it is the one
/// sentence a person reads on that card; what the engine threw is the engine's own words about its backends and
/// reaches no screen.
// computer.ts:64
// export function noPreviewRouteLine(port: number, computer: string): string {
//   return `${computer} opens no address this pane can reach for port ${port}; it answers as localhost:${port} on that computer.`;
// }
pub fn no_preview_route_line(port: f64, computer: &str) -> String {
    let port = number_string(port);
    format!("{computer} opens no address this pane can reach for port {port}; it answers as localhost:{port} on that computer.")
}

/// The heading over the tools a package manager here has that no catalog row carries: the wizard's own screen and
/// the `wsp recipe scan` section are one section, so they carry one name.
// computer.ts:70
// export const alsoTitle = (platform: "darwin" | "linux"): string => `Also on ${thisComputer(platform)}`;
pub fn also_title(platform: Platform) -> String {
    format!("Also on {}", this_computer(platform))
}

/// The one sentence a local workspace refuses a request relayed from a machine with. A local workspace is this
/// computer; it answers only its own person, so a request that reached the host from a machine wsp runs cannot drive
/// it. Today no machine has a road into the host, so nothing relays yet; the rule and its test land now.
// computer.ts:75
// export function relayedRefusal(name: string): string {
//   return `${name} is ${THIS_COMPUTER}; it answers only requests from ${THIS_COMPUTER}, never one relayed from a machine`;
// }
pub fn relayed_refusal(name: &str) -> String {
    format!("{name} is {THIS_COMPUTER}; it answers only requests from {THIS_COMPUTER}, never one relayed from a machine")
}

/// The one sentence a computer the person paired with this host is refused an op with, at the socket and on the
/// JSON routes alike. It names the op and not a reason: the door is shut until the person gives the device a role,
/// and what the device may still do is the list the door reads, listing, reading, watching and managing wsp's own
/// machines and records.
// computer.ts:83
// export function deviceHeldRefusal(op: string): string {
//   return `${op} is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on`;
// }
pub fn device_held_refusal(op: &str) -> String {
    format!("{op} is not a paired computer's to ask for until the owner gives this device a role; run it on the computer the host runs on")
}

/// The one sentence a request relayed from a machine is refused with when it would record a machine that already
/// exists. Which of the person's own machines wsp holds is theirs to say, whatever the kind: the address and the key
/// a record stands on are named on this computer, so nothing a machine asks for reaches that road.
// computer.ts:90
// export function relayedRecordRefusal(named: string): string {
//   return `recording ${named} is this computer's own act; a request relayed from a machine cannot record a machine here`;
// }
pub fn relayed_record_refusal(named: &str) -> String {
    format!("recording {named} is this computer's own act; a request relayed from a machine cannot record a machine here")
}

/// The short form of a thread id every sentence about a thread uses, so a refusal, a table and a tree all name a
/// thread the same way.
// computer.ts:96
// export const threadWord = (threadId: string): string => threadId.slice(0, 8);
pub fn thread_word(thread_id: &str) -> String {
    utf16::cut(thread_id, 8)
}

/// The one sentence a forget is refused with once a turn of the thread did work: the runtime raises it, the command
/// line prints it and the app's row action shows it without asking, so all three say the same thing.
// computer.ts:100
// export function threadForgetRefusal(threadId: string): string {
//   return `thread ${threadWord(threadId)} has a turn that ran; only a thread no turn ever ran on can be forgotten`;
// }
pub fn thread_forget_refusal(thread_id: &str) -> String {
    format!("thread {} has a turn that ran; only a thread no turn ever ran on can be forgotten", thread_word(thread_id))
}

/// Why a settle left a thread it was named: something in its tree works or asks, or the fold holds it already.
// computer.ts:105
pub const SETTLE_WORKING: &str = "still working: stop it first";

// computer.ts:106
pub const SETTLE_ALREADY: &str = "already settled";

/// A settle or a restore naming one of an agent's own subagents, which folds with the thread it runs in.
// computer.ts:109
// export const subagentSettleLine = (id: string): string => `${id} is a subagent, and a subagent settles with its lead's turn`;
pub fn subagent_settle_line(id: &str) -> String {
    format!("{id} is a subagent, and a subagent settles with its lead's turn")
}

// computer.ts:110
pub const SUBAGENT_SETTLE_FIX: &str = "settle the thread it runs in.";

/// A thread's own token naming its lead or a thread beside it in a settle or a restore.
// computer.ts:113
// export const notUnderLine = (threadId: string): string => `thread ${threadWord(threadId)} is not under this thread, and a thread settles and restores only itself and the threads it started`;
pub fn not_under_line(thread_id: &str) -> String {
    format!(
        "thread {} is not under this thread, and a thread settles and restores only itself and the threads it started",
        thread_word(thread_id)
    )
}

// computer.ts:114
pub const NOT_UNDER_FIX: &str = "leave its lead and the threads beside it to the person.";

/// A restart naming a thread that still works or asks, which would leave two rows doing one job.
// computer.ts:117
// export const replacesWorkingLine = (threadId: string): string => `thread ${threadWord(threadId)} is still working, and a restart replaces only a thread that stopped`;
pub fn replaces_working_line(thread_id: &str) -> String {
    format!("thread {} is still working, and a restart replaces only a thread that stopped", thread_word(thread_id))
}

// computer.ts:118
// export const replacesWorkingFix = (threadId: string): string => `stop it first: wsp stop ${threadWord(threadId)}.`;
pub fn replaces_working_fix(thread_id: &str) -> String {
    format!("stop it first: wsp stop {}.", thread_word(thread_id))
}

/// A restart naming a thread that already has one, which would split one job across two restarts.
// computer.ts:120
// export const replacedAlreadyLine = (threadId: string, restart: string): string => `thread ${threadWord(threadId)} was restarted as ${threadWord(restart)}, and a thread has one restart`;
pub fn replaced_already_line(thread_id: &str, restart: &str) -> String {
    format!("thread {} was restarted as {}, and a thread has one restart", thread_word(thread_id), thread_word(restart))
}

// computer.ts:121
// export const replacedAlreadyFix = (restart: string): string => `name its restart instead: --replaces ${threadWord(restart)}.`;
pub fn replaced_already_fix(restart: &str) -> String {
    format!("name its restart instead: --replaces {}.", thread_word(restart))
}

/// A restart named on a send into a thread that has run.
// computer.ts:123
pub const RESTART_OPENS_LINE: &str = "a restart opens a thread, and this send goes into one that has run";

// computer.ts:124
pub const RESTART_OPENS_FIX: &str = "leave out --replaces, or run the restart as a new thread.";

/// The one sentence a forget is refused with for a row from before threads: the fold keys such a row by its own
/// turn id, so no thread here answers to it and nothing a forget could take is named.
// computer.ts:128
// export function threadWithoutIdRefusal(rowId: string): string {
//   return `${rowId} is a turn from before threads and carries no thread id of its own; no forget can name it`;
// }
pub fn thread_without_id_refusal(row_id: &str) -> String {
    format!("{row_id} is a turn from before threads and carries no thread id of its own; no forget can name it")
}

/// Every act a thread scoped token can be refused for, and the word each is refused by name with. The table is
/// the whole rule: an act absent from it is one no thread may ask for, and adding an act is one row here.
// computer.ts:134
// export const SPAWN_ACTS = {
//   thread_new: "open a thread",
//   fork: "fork a machine",
//   send: "send into a thread",
//   bring_back: "bring its work back",
//   commit: "commit its work",
//   fix: "ask its agent to fix",
//   update: "update its branch from the base",
//   merge: "merge its pull request",
//   merge_in: "merge a child's branch into its own",
//   start: "start work from a link",
//   review: "start a review",
//   review_post: "post a review",
//   delete: "delete anything",
//   delete_sub: "delete a sub-thread it started",
//   pause: "pause a machine",
//   resize: "move a machine to another size",
//   import: "import a folder",
//   export: "export a folder",
//   agents: "change what agents may do",
//   size: "pick a machine's size",
// } as const;
/// Every act a thread scoped token can be refused for (`keyof SPAWN_ACTS`), in the table's order; serialized as its key.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpawnAct {
    ThreadNew,
    Fork,
    Send,
    BringBack,
    Commit,
    Fix,
    Update,
    Merge,
    MergeIn,
    Start,
    Review,
    ReviewPost,
    Delete,
    DeleteSub,
    Pause,
    Resize,
    Import,
    Export,
    Agents,
    Size,
}

impl SpawnAct {
    /// Every act in the table's order.
    pub const ALL: [SpawnAct; 20] = [
        SpawnAct::ThreadNew,
        SpawnAct::Fork,
        SpawnAct::Send,
        SpawnAct::BringBack,
        SpawnAct::Commit,
        SpawnAct::Fix,
        SpawnAct::Update,
        SpawnAct::Merge,
        SpawnAct::MergeIn,
        SpawnAct::Start,
        SpawnAct::Review,
        SpawnAct::ReviewPost,
        SpawnAct::Delete,
        SpawnAct::DeleteSub,
        SpawnAct::Pause,
        SpawnAct::Resize,
        SpawnAct::Import,
        SpawnAct::Export,
        SpawnAct::Agents,
        SpawnAct::Size,
    ];

    /// `SPAWN_ACTS[act]`, word for word ("open a thread", "fork a machine", ...).
    pub fn words(self) -> &'static str {
        match self {
            SpawnAct::ThreadNew => "open a thread",
            SpawnAct::Fork => "fork a machine",
            SpawnAct::Send => "send into a thread",
            SpawnAct::BringBack => "bring its work back",
            SpawnAct::Commit => "commit its work",
            SpawnAct::Fix => "ask its agent to fix",
            SpawnAct::Update => "update its branch from the base",
            SpawnAct::Merge => "merge its pull request",
            SpawnAct::MergeIn => "merge a child's branch into its own",
            SpawnAct::Start => "start work from a link",
            SpawnAct::Review => "start a review",
            SpawnAct::ReviewPost => "post a review",
            SpawnAct::Delete => "delete anything",
            SpawnAct::DeleteSub => "delete a sub-thread it started",
            SpawnAct::Pause => "pause a machine",
            SpawnAct::Resize => "move a machine to another size",
            SpawnAct::Import => "import a folder",
            SpawnAct::Export => "export a folder",
            SpawnAct::Agents => "change what agents may do",
            SpawnAct::Size => "pick a machine's size",
        }
    }

    /// The table's key: "thread_new", "bring_back", "merge_in", ...
    pub fn key(self) -> &'static str {
        match self {
            SpawnAct::ThreadNew => "thread_new",
            SpawnAct::Fork => "fork",
            SpawnAct::Send => "send",
            SpawnAct::BringBack => "bring_back",
            SpawnAct::Commit => "commit",
            SpawnAct::Fix => "fix",
            SpawnAct::Update => "update",
            SpawnAct::Merge => "merge",
            SpawnAct::MergeIn => "merge_in",
            SpawnAct::Start => "start",
            SpawnAct::Review => "review",
            SpawnAct::ReviewPost => "review_post",
            SpawnAct::Delete => "delete",
            SpawnAct::DeleteSub => "delete_sub",
            SpawnAct::Pause => "pause",
            SpawnAct::Resize => "resize",
            SpawnAct::Import => "import",
            SpawnAct::Export => "export",
            SpawnAct::Agents => "agents",
            SpawnAct::Size => "size",
        }
    }
}

// computer.ts:156
// export type SpawnAct = keyof typeof SPAWN_ACTS;

/// The acts a thread may ask for at all; every other act in the table is refused whatever the caps say.
// computer.ts:159
// export const SPAWN_ACTS_ALLOWED: readonly SpawnAct[] = ["thread_new", "fork", "send", "bring_back", "commit", "fix", "update", "merge_in", "delete_sub"];
pub const SPAWN_ACTS_ALLOWED: [SpawnAct; 9] = [
    SpawnAct::ThreadNew,
    SpawnAct::Fork,
    SpawnAct::Send,
    SpawnAct::BringBack,
    SpawnAct::Commit,
    SpawnAct::Fix,
    SpawnAct::Update,
    SpawnAct::MergeIn,
    SpawnAct::DeleteSub,
];

/// The one sentence a thread's own token is refused with when the folder or machine it runs on was made with its
/// agents' switch off, which no line turns on again.
// computer.ts:163
// export function agentsOffRefusal(workspace: string, act: SpawnAct): string {
//   return `agents on ${workspace} may not ${SPAWN_ACTS[act]}, since it was made with them off; ask the person`;
// }
pub fn agents_off_refusal(workspace: &str, act: SpawnAct) -> String {
    format!("agents on {workspace} may not {}, since it was made with them off; ask the person", act.words())
}

/// The refusal where the switch that is off is a computer's, over a folder there that holds none of its own: the
/// thread refused, and the line that turns the switch back on.
// computer.ts:169
// export function agentsOffComputerRefusal(threadId: string, computer: string, act: SpawnAct): string {
//   return `thread ${threadWord(threadId)} may not ${SPAWN_ACTS[act]}, since agents on ${computer} are off; turn them on with ${shellLine(["wsp", "computers", "set", computer, "--spawn", "on"])}`;
// }
pub fn agents_off_computer_refusal(thread_id: &str, computer: &str, act: SpawnAct) -> String {
    format!(
        "thread {} may not {}, since agents on {computer} are off; turn them on with {}",
        thread_word(thread_id),
        act.words(),
        shell_line(&["wsp", "computers", "set", computer, "--spawn", "on"])
    )
}

/// Why a thread may not act where this host no longer holds the folder it works in.
// computer.ts:174
// export function folderGoneRefusal(threadId: string, act: SpawnAct): string {
//   return `thread ${threadWord(threadId)} may not ${SPAWN_ACTS[act]}, since this host no longer holds the folder it works in; ask the person`;
// }
pub fn folder_gone_refusal(thread_id: &str, act: SpawnAct) -> String {
    format!(
        "thread {} may not {}, since this host no longer holds the folder it works in; ask the person",
        thread_word(thread_id),
        act.words()
    )
}

/// The refusal where no switch is off but the thread a tree started from was deleted, so nothing governs it.
// computer.ts:179
// export function rootGoneRefusal(threadId: string, act: SpawnAct): string {
//   return `thread ${threadWord(threadId)} may not ${SPAWN_ACTS[act]}, since the thread that started its tree was deleted; finish the work in this thread, or ask the person to start the one you need`;
// }
pub fn root_gone_refusal(thread_id: &str, act: SpawnAct) -> String {
    format!(
        "thread {} may not {}, since the thread that started its tree was deleted; finish the work in this thread, or ask the person to start the one you need",
        thread_word(thread_id),
        act.words()
    )
}

/// The one sentence the guest door refuses a line that carries neither a thread's token nor a turn's with: a
/// person's shell or a `wsp exec` on the machine. The switch may well be on, so the sentence names the missing
/// identity and never tells anyone to turn it on.
// computer.ts:186
// export function guestNoTokenRefusal(workspace: string): string {
//   return `this line came from no thread's turn on ${workspace}, so it carries no thread's token; wsp hands one to each turn it starts`;
// }
pub fn guest_no_token_refusal(workspace: &str) -> String {
    format!("this line came from no thread's turn on {workspace}, so it carries no thread's token; wsp hands one to each turn it starts")
}

/// The sentence the guest door refuses a turn's line that carries no thread's token with: every turn is launched
/// with one whatever the switch says, so this turn was launched by a host that minted it none, and the switch is not
/// what would change it.
// computer.ts:193
// export function guestTurnNoTokenRefusal(workspace: string): string {
//   return `this line came from a turn on ${workspace} that was launched without a thread's token; a turn started now carries one`;
// }
pub fn guest_turn_no_token_refusal(workspace: &str) -> String {
    format!("this line came from a turn on {workspace} that was launched without a thread's token; a turn started now carries one")
}

/// The one word the socket door closes a socket whose token names nobody with, and the one the guest door refuses a
/// session with. Spelled once so the two roads into this host cannot drift apart in what they say.
// computer.ts:199
pub const UNAUTHORIZED: &str = "unauthorized";

/// Whether parsed JSON is a frame with fields to read: an object and not an array. Null, a number, a string, a
/// boolean and an array all parse as JSON and carry no op and no id, and reading a field off null throws, so every
/// door reads this ahead of anything else it reads off a frame.
// computer.ts:204
// export function isObjectFrame(parsed: unknown): parsed is Record<string, unknown> {
//   return typeof parsed === "object" && parsed !== null && !Array.isArray(parsed);
// }
pub fn is_object_frame(parsed: &serde_json::Value) -> bool {
    parsed.is_object()
}

/// The one sentence such a frame is refused with, on the socket and on the JSON routes alike, under a null id
/// since none could be read.
// computer.ts:210
pub const REQUEST_NOT_AN_OBJECT: &str = "a request is one JSON object, not a bare value or an array";

/// The most a JSON route reads off a request body, in bytes, and the one sentence a body past it is refused with:
/// a stranger down a tunnel reaches those routes, so what they may hand a route is bounded before it is held.
// computer.ts:214
// export const REQUEST_BODY_MAX_BYTES = 64 * 1024;
pub const REQUEST_BODY_MAX_BYTES: usize = 64 * 1024;

// computer.ts:215
// export const REQUEST_BODY_TOO_LARGE = `a request body is at most ${REQUEST_BODY_MAX_BYTES / 1024} KiB`;
pub const REQUEST_BODY_TOO_LARGE: &str = "a request body is at most 64 KiB";

/// The one sentence a body that is no JSON at all is refused with. The parser's own words carry the offset it
/// stopped at and the text around it, which is a stranger's request read back to them.
// computer.ts:219
pub const REQUEST_BODY_NOT_JSON: &str = "a request body is JSON; this one did not parse";

/// What a guest session is refused with when it asks for a kind this host serves no module for; its own words, since
/// a kind nobody built is not a token nobody holds.
// computer.ts:223
// export function guestNoKindLine(kind: string): string {
//   return `this host serves no ${kind} guest session`;
// }
pub fn guest_no_kind_line(kind: &str) -> String {
    format!("this host serves no {kind} guest session")
}

/// What the guest's next frame is answered with once the host no longer holds its session: a host that restarted, or
/// a link that ended, leaves the machine's daemon holding a session nothing on this side can answer.
// computer.ts:229
pub const GUEST_NO_SESSION_LINE: &str = "the host holds no such session";

/// What a scoped wsp tool server says when the turn that launched it handed it no pair: its agent dropped the
/// environment, and serving on this computer's own token would act as the person.
// computer.ts:233
pub const SCOPED_NO_PAIR_LINE: &str = "this thread's tools were launched without its own host token, so they refuse rather than act as the person; the agent did not pass WSP_HOST_URL and WSP_HOST_TOKEN to its MCP server";

/// What a session is ended with on a host bound to one address beyond loopback: its wsp dials this host's loopback,
/// and nothing answers there.
// computer.ts:237
pub const GUEST_NO_LOOPBACK_LINE: &str = "this host listens on no loopback address, so a thread's wsp has nowhere on this computer to reach it; run wsp up with --listen 0.0.0.0 or 127.0.0.1";

/// What a line from inside a machine is told when it names a path: the path would be resolved and read on the
/// person's computer, and the file the caller means is on the machine the line was typed on.
// computer.ts:241
pub const GUEST_NO_FILE_LINE: &str = "a path on this line would be read on the person's computer, not on the machine the line was typed on";

/// What a line from inside a machine is told when it leaves the workspace to the folder it was typed in: that folder
/// is on the machine, and reading it here would answer about the person's own checkouts instead.
// computer.ts:245
pub const GUEST_NAMES_WORKSPACE_LINE: &str = "a line from a machine names the project it means, since the folder it was typed in is not one this host can read";

// computer.ts:246
pub const GUEST_NAMES_FIX: &str = "Name the project on the line.";

/// What a line typed inside a workspace is told when it names a verb that runs at the person's own keyboard: the
/// guest road carries the verbs an agent has business with, and the rest happen where the person is.
// computer.ts:250
// export function guestPersonsComputerLine(word: string): string {
//   return `wsp ${word} runs on the person's computer, not from a thread on another computer`;
// }
pub fn guest_persons_computer_line(word: &str) -> String {
    format!("wsp {word} runs on the person's computer, not from a thread on another computer")
}

/// What a line typed inside a workspace is told when it tries to aim itself somewhere else. The host a turn's
/// launch named is the one a line from that turn reaches, and the state file it would name is on another computer.
// computer.ts:256
pub const GUEST_HOST_FLAG_LINE: &str = "a line from a thread goes to the host that started it; --host and --state are not read here";

/// Thread and where it runs, as a refusal to a thread's own token names it; `on` absent for a thread whose workspace
/// this host no longer holds.
// computer.ts:260
// export function threadAt(threadId: string, on: string | undefined): string {
//   return `thread ${threadWord(threadId)}${on === undefined ? "" : ` on ${on}`}`;
// }
pub fn thread_at(thread_id: &str, on: Option<&str>) -> String {
    match on {
        None => format!("thread {}", thread_word(thread_id)),
        Some(on) => format!("thread {} on {on}", thread_word(thread_id)),
    }
}

/// The one sentence a thread's own token is refused with for an act no thread may ask for, whatever the caps.
// computer.ts:265
// export function spawnActRefusal(threadId: string, act: SpawnAct, on: string | undefined): string {
//   return `this request came out of ${threadAt(threadId, on)}, and a thread may only ${SPAWN_ACTS_ALLOWED.map(a => SPAWN_ACTS[a]).join(", ")}, never ${SPAWN_ACTS[act]}`;
// }
pub fn spawn_act_refusal(thread_id: &str, act: SpawnAct, on: Option<&str>) -> String {
    let allowed: Vec<&str> = SPAWN_ACTS_ALLOWED.iter().map(|a| a.words()).collect();
    format!("this request came out of {}, and a thread may only {}, never {}", thread_at(thread_id, on), allowed.join(", "), act.words())
}

/// The one sentence a fork of a project whose threads run in its folder is refused with, the person's and a thread's
/// alike: the folder is the project itself, and nothing about it is a machine to fork.
// computer.ts:271
// export function folderForkRefusal(project: string): string {
//   return `${project} is a project folder, not a machine to fork`;
// }
pub fn folder_fork_refusal(project: &str) -> String {
    format!("{project} is a project folder, not a machine to fork")
}

/// The roads that carry a copy's commits to its remote, each refused alike on the project's default branch.
// computer.ts:276
// export type DefaultBranchRoad = "fork" | "run" | "bring back";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum DefaultBranchRoad {
    #[serde(rename = "fork")]
    Fork,
    #[serde(rename = "run")]
    Run,
    #[serde(rename = "bring back")]
    BringBack,
}

impl DefaultBranchRoad {
    /// The TS string literal.
    pub fn as_str(self) -> &'static str {
        match self {
            DefaultBranchRoad::Fork => "fork",
            DefaultBranchRoad::Run => "run",
            DefaultBranchRoad::BringBack => "bring back",
        }
    }
}

/// The one sentence a copy is refused with on any of those roads when it holds commits on the branch its remote
/// starts every copy on, as the copy's own daemon reads that branch: no push of wsp's moves it, and a copy started
/// from it would start without them.
// computer.ts:281
// export function defaultBranchRefusal(workspace: string, branch: string): string {
//   return `${workspace} is on ${branch}, the project's default branch, with commits the remote lacks, and wsp never pushes ${branch}`;
// }
pub fn default_branch_refusal(workspace: &str, branch: &str) -> String {
    format!("{workspace} is on {branch}, the project's default branch, with commits the remote lacks, and wsp never pushes {branch}")
}

// computer.ts:285
// private in the TS: port it as a private helper
// const DEFAULT_BRANCH_AGAIN: Readonly<Record<DefaultBranchRoad, string>> = { fork: "fork it again", run: "start the thread again", "bring back": "bring it back again" };

/// What to do about that refusal on each road: carry the commits on a branch of their own, then ask again.
// computer.ts:288
// export function defaultBranchFix(road: DefaultBranchRoad, workspace: string): string {
//   return `Move the commits in ${workspace} onto a branch of their own, then ${DEFAULT_BRANCH_AGAIN[road]}.`;
// }
pub fn default_branch_fix(road: DefaultBranchRoad, workspace: &str) -> String {
    let again = match road {
        DefaultBranchRoad::Fork => "fork it again",
        DefaultBranchRoad::Run => "start the thread again",
        DefaultBranchRoad::BringBack => "bring it back again",
    };
    format!("Move the commits in {workspace} onto a branch of their own, then {again}.")
}

/// What to do about that refusal: open a thread in the folder instead.
// computer.ts:293
// export function folderForkFix(project: string): string {
//   return `Open a thread in it with ${shellLine(["wsp", "run", project])} "<task>".`;
// }
pub fn folder_fork_fix(project: &str) -> String {
    format!("Open a thread in it with {} \"<task>\".", shell_line(&["wsp", "run", project]))
}

/// The one sentence a token scoped to a thread is refused with for arriving on a road this host does not serve its
/// own workspaces' guests on. Which road that is, and why, is on the host's own `ownRoad`.
// computer.ts:299
pub const SCOPED_TOKEN_ROAD_REFUSAL: &str = "a thread's token opens this host over the road its own computer's guest is served on, and this request came by another";

/// The one sentence a thread naming the image its fork starts from is refused with: a thread forks the image its
/// own workspace's project runs, and the manifests that hold every snapshot id are not a thread's to read, so an id
/// it names is one it read outside the tree it may read.
// computer.ts:305
// export function spawnGoldenRefusal(threadId: string, on: string | undefined): string {
//   return `this request came out of ${threadAt(threadId, on)}, and a thread forks the image its own workspace runs; naming an image to fork from is not a thread's to ask for`;
// }
pub fn spawn_golden_refusal(thread_id: &str, on: Option<&str>) -> String {
    format!(
        "this request came out of {}, and a thread forks the image its own workspace runs; naming an image to fork from is not a thread's to ask for",
        thread_at(thread_id, on)
    )
}

/// The one sentence a fork past the machine cap is refused with, naming the root the machines were counted under.
// computer.ts:310
// export function spawnCapRefusal(rootThreadId: string, standing: number, cap: number): string {
//   return `thread ${threadWord(rootThreadId)} already holds ${standing} of its ${cap} machines; delete one before forking another`;
// }
pub fn spawn_cap_refusal(root_thread_id: &str, standing: f64, cap: f64) -> String {
    format!(
        "thread {} already holds {} of its {} machines; delete one before forking another",
        thread_word(root_thread_id),
        number_string(standing),
        number_string(cap)
    )
}

/// What a cloud at its spend per day says: its row's state sentence and the head of the refusal below.
// computer.ts:315
pub const SPEND_LIMIT_LINE: &str = "spend limit reached today";

/// The one sentence a new machine on a cloud at its spend per day is refused with; the machines already running there
/// are left running.
// computer.ts:319
// export function spendCapRefusal(name: string, todayUsd: number, capUsd: number): string {
//   return `${SPEND_LIMIT_LINE} on ${name} (${spendMeterWord(todayUsd, capUsd)}); raise its spend per day or start the machine after midnight`;
// }
pub fn spend_cap_refusal(name: &str, today_usd: f64, cap_usd: f64) -> String {
    format!(
        "{SPEND_LIMIT_LINE} on {name} ({}); raise its spend per day or start the machine after midnight",
        spend_meter_word(today_usd, cap_usd)
    )
}

/// The one sentence a spawn deeper than the tree allows is refused with, and how to raise it where the cap is held:
/// the computer's Levels deep, or the cap a machine was made with, which stands for its life.
// computer.ts:325
// export function spawnDepthRefusal(threadId: string, depth: number, cap: number, held: { computer: string } | { workspace: string }): string {
//   const deeper = String(depth + 1);
//   const raise =
//     "computer" in held
//       ? `raise Levels deep in Settings > Computers or run ${shellLine(["wsp", "computers", "set", held.computer, "--max-depth", deeper])}`
//       : `${held.workspace} was made with that cap; ask the person`;
//   return `thread ${threadWord(threadId)} is ${depth} deep under its root and ${"computer" in held ? "this computer" : held.workspace} allows ${cap}, so a thread this deep may not spawn; ${raise}`;
// }
/// Where a depth cap is held: a computer's Levels deep, or the cap a machine was made with.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DepthHeld {
    Computer(String),
    Workspace(String),
}

pub fn spawn_depth_refusal(thread_id: &str, depth: f64, cap: f64, held: &DepthHeld) -> String {
    let deeper = number_string(depth + 1.0);
    let (who, raise) = match held {
        DepthHeld::Computer(computer) => (
            THIS_COMPUTER.to_string(),
            format!(
                "raise Levels deep in Settings > Computers or run {}",
                shell_line(&["wsp", "computers", "set", computer.as_str(), "--max-depth", deeper.as_str()])
            ),
        ),
        DepthHeld::Workspace(workspace) => (workspace.clone(), format!("{workspace} was made with that cap; ask the person")),
    };
    format!(
        "thread {} is {} deep under its root and {who} allows {}, so a thread this deep may not spawn; {raise}",
        thread_word(thread_id),
        number_string(depth),
        number_string(cap)
    )
}

/// The one sentence a thread is refused with for naming a project of another repository than the one its own
/// workspace holds. A child is a second checkout of its lead's repository on the lead's branch, so a thread starts
/// children on that repository's projects and on no other. A thread's refusal carries the word it typed.
// computer.ts:337
// export function spawnRepositoryRefusal(threadId: string, project: string, other: string): string {
//   return `thread ${threadWord(threadId)} works on ${project}, and ${other} is a project of another repository; a thread starts children on its own repository's projects alone`;
// }
pub fn spawn_repository_refusal(thread_id: &str, project: &str, other: &str) -> String {
    format!("thread {} works on {project}, and {other} is a project of another repository; a thread starts children on its own repository's projects alone", thread_word(thread_id))
}

/// What a thread does about that refusal.
// computer.ts:342
pub const SPAWN_REPOSITORY_FIX: &str = "Name a project of the same repository, or ask the person to start this one.";

/// The one sentence a thread is refused with for naming a workspace of another repository: the word it typed and
/// nothing of the workspace's own, since its project is one the thread may not see.
// computer.ts:346
// export function spawnRepositoryWorkspaceRefusal(threadId: string, project: string, word: string): string {
//   return `thread ${threadWord(threadId)} works on ${project}, and the workspace ${word} holds another repository; a thread reaches its own repository's workspaces alone`;
// }
pub fn spawn_repository_workspace_refusal(thread_id: &str, project: &str, word: &str) -> String {
    format!("thread {} works on {project}, and the workspace {word} holds another repository; a thread reaches its own repository's workspaces alone", thread_word(thread_id))
}

/// What a thread does about that refusal, whichever verb named the workspace.
// computer.ts:351
pub const SPAWN_REPOSITORY_WORKSPACE_FIX: &str = "Name a workspace of your own tree or a project of your own repository, or ask the person.";

/// The one sentence a thread is refused with for naming a folder the person keeps of its repository on a computer
/// that copies folders: a thread started there runs in the person's folder and stands outside the tree, so a thread
/// starts children on its own project and its repository's box and cloud projects alone.
// computer.ts:356
// export function spawnFolderRefusal(threadId: string, word: string): string {
//   return `thread ${threadWord(threadId)} may not start children in ${word}: it is a folder the person keeps of this repository, and a thread started there would stand outside your tree`;
// }
pub fn spawn_folder_refusal(thread_id: &str, word: &str) -> String {
    format!("thread {} may not start children in {word}: it is a folder the person keeps of this repository, and a thread started there would stand outside your tree", thread_word(thread_id))
}

/// What a thread does about that refusal.
// computer.ts:361
pub const SPAWN_FOLDER_FIX: &str = "Name a project wsp projects lists, or ask the person to start this one.";

/// The one sentence a thread is refused with for reaching a workspace outside its own tree.
// computer.ts:364
// export function spawnReachRefusal(threadId: string, name: string): string {
//   return `thread ${threadWord(threadId)} may drive the workspace it runs on and the ones it forked, and ${name} is neither`;
// }
pub fn spawn_reach_refusal(thread_id: &str, name: &str) -> String {
    format!("thread {} may drive the workspace it runs on and the ones it forked, and {name} is neither", thread_word(thread_id))
}

/// The one sentence a thread on a computer the person joined is refused with for naming a project on another computer
/// that its lead reaches: nothing carries a thread's work back across yet, and a thread talks to the lead of its tree
/// by message alone, so the road is a message asking the lead to start it.
// computer.ts:371
// export function childToLeadsComputerLine(from: string, to: string, lead: string): string {
//   return `a thread on ${from} cannot start one on ${to} yet; ask thread ${threadWord(lead)}, your lead, with ${shellLine(["wsp", "send", threadWord(lead)])} "<message>", or name a project on ${from}`;
// }
pub fn child_to_leads_computer_line(from: &str, to: &str, lead: &str) -> String {
    let lead = thread_word(lead);
    format!(
        "a thread on {from} cannot start one on {to} yet; ask thread {lead}, your lead, with {} \"<message>\", or name a project on {from}",
        shell_line(&["wsp", "send", lead.as_str()])
    )
}

/// What a thread on a computer the person joined asked to do on a workspace of another computer, as its refusal names
/// it: `work` where the door it came in by does not know the verb.
// computer.ts:377
// export const ACROSS_ACTS = { start: "start a thread", exec: "run commands", commit: "commit", update: "update a copy", fix: "ask for a fix", wake: "wake it", work: "work" } as const;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AcrossAct {
    Start,
    Exec,
    Commit,
    Update,
    Fix,
    Wake,
    Work,
}

impl AcrossAct {
    /// `ACROSS_ACTS[act]`, word for word ("start a thread", "run commands", ...).
    pub fn words(self) -> &'static str {
        match self {
            AcrossAct::Start => "start a thread",
            AcrossAct::Exec => "run commands",
            AcrossAct::Commit => "commit",
            AcrossAct::Update => "update a copy",
            AcrossAct::Fix => "ask for a fix",
            AcrossAct::Wake => "wake it",
            AcrossAct::Work => "work",
        }
    }
}

// computer.ts:378
// export type AcrossAct = keyof typeof ACROSS_ACTS;

/// The sentence a thread is refused with for a thread of its own tree working in a folder it may not act on: the
/// road is a message to that thread, which may.
// computer.ts:382
// export function treeThreadOutOfReachLine(thread: string, caller: string): string {
//   return `thread ${threadWord(thread)} works in a folder thread ${threadWord(caller)} cannot act on; ask it with ${shellLine(["wsp", "send", threadWord(thread)])} "<message>"`;
// }
pub fn tree_thread_out_of_reach_line(thread: &str, caller: &str) -> String {
    let short = thread_word(thread);
    format!(
        "thread {short} works in a folder thread {} cannot act on; ask it with {} \"<message>\"",
        thread_word(caller),
        shell_line(&["wsp", "send", short.as_str()])
    )
}

/// The sentence a thread on a computer the person joined is refused with for acting on a workspace of another
/// computer: a message to its lead where the lead may do that act there itself, the person otherwise.
// computer.ts:388
// export function elsewhereWorkspaceLine(name: string, on: string, from: string, act: AcrossAct, lead?: string): string {
//   const road = lead === undefined ? "ask the person" : `ask thread ${threadWord(lead)}, your lead, with ${shellLine(["wsp", "send", threadWord(lead)])} "<message>"`;
//   return `${name} is on ${on}, and a thread on ${from} cannot ${ACROSS_ACTS[act]} there yet; ${road}`;
// }
pub fn elsewhere_workspace_line(name: &str, on: &str, from: &str, act: AcrossAct, lead: Option<&str>) -> String {
    let road = match lead {
        None => "ask the person".to_string(),
        Some(lead) => {
            let lead = thread_word(lead);
            format!("ask thread {lead}, your lead, with {} \"<message>\"", shell_line(&["wsp", "send", lead.as_str()]))
        }
    };
    format!("{name} is on {on}, and a thread on {from} cannot {} there yet; {road}", act.words())
}

/// A thread on a computer the person joined reaches a thread of its tree on another computer by its words alone: no
/// file of its lands there, and nothing of that thread's turn comes back but a message that thread sends.
// computer.ts:395
// export function sendFilesAcrossLine(from: string, to: string, thread: string): string {
//   return `a thread on ${from} cannot send files to thread ${threadWord(thread)} on ${to}`;
// }
pub fn send_files_across_line(from: &str, to: &str, thread: &str) -> String {
    format!("a thread on {from} cannot send files to thread {} on {to}", thread_word(thread))
}

// computer.ts:398
pub const SEND_FILES_ACROSS_FIX: &str = "Send the message without them, with what they hold written into it.";

// computer.ts:399
// export function waitAcrossLine(from: string, to: string, thread: string): string {
//   return `a thread on ${from} cannot wait on thread ${threadWord(thread)} on ${to} for its reply`;
// }
pub fn wait_across_line(from: &str, to: &str, thread: &str) -> String {
    format!("a thread on {from} cannot wait on thread {} on {to} for its reply", thread_word(thread))
}

// computer.ts:402
pub const WAIT_ACROSS_FIX: &str = "Send it again with --detach and end your turn; that thread answers you with a message of its own.";

/// What a thread does about a workspace outside its tree: a child on that workspace's project, in the words of what a
/// run does there, a machine of its own on a cloud and a thread in the folder on this computer or a box.
// computer.ts:406
// export function spawnReachFix(project: string, forks: boolean): string {
//   const line = `${shellLine(["wsp", "run", project])} "<message>"`;
//   return forks
//     ? `Start a child on ${project} instead with ${line}, which forks a machine in your own tree.`
//     : `Start a child in ${project}'s folder instead with ${line}, which opens a thread there in your own tree.`;
// }
pub fn spawn_reach_fix(project: &str, forks: bool) -> String {
    let line = format!("{} \"<message>\"", shell_line(&["wsp", "run", project]));
    if forks {
        format!("Start a child on {project} instead with {line}, which forks a machine in your own tree.")
    } else {
        format!("Start a child in {project}'s folder instead with {line}, which opens a thread there in your own tree.")
    }
}

/// The one sentence a fork is refused with when the workspace it would be a child of did not say which branch it
/// is on: a child starts on its parent's branch and its work lands back in that branch, so a parent that could not
/// be read stops the fork rather than quietly starting it somewhere else. The machine's own words ride it.
// computer.ts:416
// export function branchUnreadRefusal(workspace: string, said: string): string {
//   return `${workspace} did not say which branch it is on (${said}); wake it and try again, since a child of it starts on that branch and its work lands back in it`;
// }
pub fn branch_unread_refusal(workspace: &str, said: &str) -> String {
    format!("{workspace} did not say which branch it is on ({said}); wake it and try again, since a child of it starts on that branch and its work lands back in it")
}

/// The one sentence a create naming a parent this host does not hold is refused with: a child is made out of a
/// workspace that is here, since its branch and its work are what the child starts from and lands in.
// computer.ts:422
// export function noParentWorkspaceLine(r#ref: string): string {
//   return `no workspace ${ref} to fork from; a child workspace is made out of one this computer holds`;
// }
pub fn no_parent_workspace_line(r#ref: &str) -> String {
    format!("no workspace {} to fork from; a child workspace is made out of one this computer holds", r#ref)
}

/// The one sentence a create naming a parent of another project is refused with: a child is a second checkout of
/// its parent's project on the branch that parent is on, so a parent holding another project has no branch the
/// child could start from and nowhere for its work to land back in.
// computer.ts:429
// export function parentProjectRefusal(parent: string, holds: string, made: string): string {
//   return `${parent} is a workspace of ${holds} and this one is made for ${made}; a child starts on its parent's branch, so both hold one project`;
// }
pub fn parent_project_refusal(parent: &str, holds: &str, made: &str) -> String {
    format!("{parent} is a workspace of {holds} and this one is made for {made}; a child starts on its parent's branch, so both hold one project")
}

/// The one sentence a name no workspace this caller may drive carries is refused with. Absence is the only thing it
/// says, and for a thread it is the whole answer: a workspace outside its tree is refused as missing rather than by
/// the rule that hides it, since a sentence naming one is how a thread learns what else this host holds. The word
/// rides it where the caller named one and stays off where the verb found the workspace itself. A person still reads
/// the rule, since what this host holds is theirs.
// computer.ts:438
// export function noWorkspaceRefusal(ref?: string): string {
//   return ref === undefined ? "no such workspace" : `no workspace ${ref}`;
// }
pub fn no_workspace_refusal(r#ref: Option<&str>) -> String {
    match r#ref {
        None => "no such workspace".to_string(),
        Some(r) => format!("no workspace {r}"),
    }
}

/// How much of an id a word has to carry before it names a workspace by its start: enough that a name with spaces
/// has a way round the quotes, and enough that two or three characters never reach for a workspace nobody meant.
// computer.ts:444
pub const ID_PREFIX_MIN: usize = 4;

/// What a word that starts more than one workspace's id is refused with: both ids, so the next word is typed off
/// this line rather than off another listing.
// computer.ts:448
// export function idPrefixRefusal(r#ref: string, ids: readonly string[]): string {
//   return `${ids.length} workspaces start with ${ref} (${ids.join(", ")}); give more of the id`;
// }
pub fn id_prefix_refusal(r#ref: &str, ids: &[impl AsRef<str>]) -> String {
    let ids: Vec<&str> = ids.iter().map(|id| id.as_ref()).collect();
    format!("{} workspaces start with {} ({}); give more of the id", ids.len(), r#ref, ids.join(", "))
}

/// The workspace table's cell for the switch: the machines a root thread there may hold, and empty where a person
/// turned it off.
// computer.ts:454
// export function agentsWord(agents: { spawn: boolean; maxMachines: number } | undefined): string {
//   return agents?.spawn !== true ? "" : `${agents.maxMachines} ${agents.maxMachines === 1 ? "machine" : "machines"}`;
// }
/// A switch for agents spawning, as a record holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AgentsSwitch {
    pub spawn: bool,
    pub max_machines: f64,
    pub max_depth: f64,
}

pub fn agents_word(agents: Option<&AgentsSwitch>) -> String {
    match agents {
        Some(a) if a.spawn => {
            format!("{} {}", number_string(a.max_machines), if a.max_machines == 1.0 { "machine" } else { "machines" })
        }
        _ => String::new(),
    }
}

/// What a listing and a computer's row say about a switch, one line either way.
// computer.ts:459
// export function agentsLine(agents: { spawn: boolean; maxMachines: number; maxDepth: number } | undefined): string {
//   return agents?.spawn !== true ? "agents may not spawn" : `agents may spawn: up to ${agents.maxMachines} ${agents.maxMachines === 1 ? "machine" : "machines"}`;
// }
pub fn agents_line(agents: Option<&AgentsSwitch>) -> String {
    match agents {
        Some(a) if a.spawn => format!(
            "agents may spawn: up to {} {}",
            number_string(a.max_machines),
            if a.max_machines == 1.0 { "machine" } else { "machines" }
        ),
        _ => "agents may not spawn".to_string(),
    }
}

/// Where the ssh client on this computer writes the key a machine first answered with under its own default
/// config, in the words a person would type, since the file is theirs and nothing wsp owns. This is what a screen
/// can name before anything is dialled, which is where a person reads what pressing Add will do; the file that was
/// actually written is the client's own answer, and the step that writes it says which one it was.
// computer.ts:467
pub const KNOWN_HOSTS: &str = "~/.ssh/known_hosts";

/// What the step that wrote a machine's key into this computer's known_hosts says it left there: the fingerprint
/// whole, so it can be checked against the machine's own, and the file the client answered with where that is not
/// the one the plan line named. A person with UserKnownHostsFile pointed elsewhere reads the true path here rather
/// than the default above it.
// computer.ts:473
// export function hostKeyKeptNote(hostKey: string, file?: string): string {
//   return file === undefined || file.endsWith(KNOWN_HOSTS.slice(1)) ? hostKey : `${hostKey} in ${file}`;
// }
pub fn host_key_kept_note(host_key: &str, file: Option<&str>) -> String {
    match file {
        Some(file) if !file.ends_with(&KNOWN_HOSTS[1..]) => format!("{host_key} in {file}"),
        _ => host_key.to_string(),
    }
}

/// The login shells a computer's root may run for wsp to work it. sshd hands every command the host sends to
/// root's own shell with -c before wsp's own `bash -c` inside it, and on a computer wsp joins root's home is the
/// one every workspace on it writes, so a shell that reads a file under that home runs a workspace's file as that
/// computer's root, outside every namespace, on each dial.
///
/// What the list stands on and what it does not: `env -i HOME=<home> <shell> -c true` against a home whose every
/// startup file writes a marker wrote none for bash, sh, dash and ksh and wrote one for zsh. dash and ksh are off
/// the list all the same, since the list is the owner's ruling and not the measurement. And the measurement has a
/// hole this end cannot close: a bash built with SSH_SOURCE_BASHRC reads `~/.bashrc` under -c when the environment
/// carries SSH_CLIENT, which sshd sets for every session, and the bash on the computer this was measured on (3.2.57,
/// Apple's) is such a build. Which build a given box carries is not something the host can read before it dials,
/// and the shell sshd runs takes no flags from this end, so bash stays on the list by the ruling.
// computer.ts:489
// export const PLACE_ROOT_SHELLS: readonly string[] = ["bash", "sh"];
pub const PLACE_ROOT_SHELLS: [&str; 2] = ["bash", "sh"];

// The file a shell reads under the home before running a command, where wsp has read one: the measurement above.
// fish is not on the computer that measurement was made on, and its own documentation names the other. A shell in
// neither list is refused for not having been read, and the sentence says so rather than claiming a file it has
// not seen.
// computer.ts:495
// private in the TS: port it as a private helper
// const SHELL_STARTUP_FILE: Readonly<Record<string, string>> = { zsh: "~/.zshenv", fish: "config.fish" };

/// What a computer is refused with for running a shell that is not on the list: the shell, why it is a road, and
/// the one command that changes it. Said before anything of wsp's lands there.
// computer.ts:499
// export function placeRootShellRefusal(address: string, shell: string): string {
//   const file = SHELL_STARTUP_FILE[shell];
//   const why =
//     file === undefined
//       ? `${shell} is not a shell wsp has read as opening no file of that computer's, and root's home there is the one every workspace on it writes, so a file read under it would run as root`
//       : `${shell} reads ${file} before running anything, and root's home there is the one every workspace on it writes, so a dial would run a workspace's file as root`;
//   return `${address} runs ${shell} as its root's login shell, and sshd hands every command wsp sends to that shell: ${why}; run chsh -s /bin/bash root on it and add it again`;
// }
pub fn place_root_shell_refusal(address: &str, shell: &str) -> String {
    let startup = match shell {
        "zsh" => Some("~/.zshenv"),
        "fish" => Some("config.fish"),
        _ => None,
    };
    let why = match startup {
        None => format!("{shell} is not a shell wsp has read as opening no file of that computer's, and root's home there is the one every workspace on it writes, so a file read under it would run as root"),
        Some(file) => format!("{shell} reads {file} before running anything, and root's home there is the one every workspace on it writes, so a dial would run a workspace's file as root"),
    };
    format!("{address} runs {shell} as its root's login shell, and sshd hands every command wsp sends to that shell: {why}; run chsh -s /bin/bash root on it and add it again")
}

/// Whether the key a dial read is the one the person pinned. A person passes either the whole known_hosts word
/// (`ssh-ed25519 SHA256:...`) or the fingerprint on its own, which is what ssh-keygen prints beside the size and the
/// comment, so both sides are read down to the fingerprint where they carry one.
// computer.ts:511
// export function hostKeyMatches(pinned: string, read: string): boolean {
//   const mark = (word: string): string => word.trim().split(/\s+/).find(w => w.startsWith("SHA256:")) ?? word.trim();
//   return mark(pinned) !== "" && mark(pinned) === mark(read);
// }
pub fn host_key_matches(pinned: &str, read: &str) -> bool {
    let mark = |word: &str| -> String {
        let trimmed = js_trim(word);
        trimmed.split(is_js_whitespace).find(|w| w.starts_with("SHA256:")).unwrap_or(trimmed).to_string()
    };
    let pinned_mark = mark(pinned);
    !pinned_mark.is_empty() && pinned_mark == mark(read)
}

/// What a computer this computer's ssh client has never met is asked before the first dial: the key it answered a
/// scan with, for the person to check against the computer in front of them. What the first dial writes is the
/// identity every later dial of that computer trusts, so it is confirmed before anything is sent there.
// computer.ts:519
// export function hostKeyAsk(address: string, hostKey: string): string {
//   return `${address} answers with the key ${hostKey}; trust it and continue?`;
// }
pub fn host_key_ask(address: &str, host_key: &str) -> String {
    format!("{address} answers with the key {host_key}; trust it and continue?")
}

/// What the add is refused with for such a computer where nobody confirmed that key: the key whole, so it can be
/// read against the computer itself, and the line that pins it. Said before anything is dialled.
// computer.ts:525
// export function hostKeyUnconfirmedRefusal(address: string, hostKey: string): string {
//   return `${address} has never been reached from this computer and answers with the key ${hostKey}; check it against the computer itself, then run wsp add ${address} --host-key '${hostKey}'`;
// }
pub fn host_key_unconfirmed_refusal(address: &str, host_key: &str) -> String {
    format!("{address} has never been reached from this computer and answers with the key {host_key}; check it against the computer itself, then run wsp add {address} --host-key '{host_key}'")
}

/// The same refusal where the key could not be asked for at all: a jump host or a proxy command in the person's own
/// ssh config, which a scan cannot follow, or a computer that answered no scan. The key is theirs to read on the
/// computer and pass here.
// computer.ts:532
// export function hostKeyUnscannableRefusal(address: string, stoppedBy?: string): string {
//   const why = stoppedBy === undefined ? "answered no key to a scan" : `is reached through ${stoppedBy} in your ssh config, which a key scan cannot follow`;
//   return `${address} has never been reached from this computer and ${why}; read its host key on the computer itself and run wsp add ${address} --host-key '<type> <fingerprint>'`;
// }
pub fn host_key_unscannable_refusal(address: &str, stopped_by: Option<&str>) -> String {
    let why = match stopped_by {
        None => "answered no key to a scan".to_string(),
        Some(by) => format!("is reached through {by} in your ssh config, which a key scan cannot follow"),
    };
    format!("{address} has never been reached from this computer and {why}; read its host key on the computer itself and run wsp add {address} --host-key '<type> <fingerprint>'")
}

/// What an install is refused with when the computer that answered the first dial holds a key other than the one
/// the person pinned: what it answered with, the file ssh wrote that into on its way in, and the line that takes it
/// out again. Nothing of wsp's has left this computer at that point.
// computer.ts:540
// export function hostKeyMismatchRefusal(o: { address: string; pinned: string; wrote?: string; target: string; file: string }): string {
//   return `${o.address} answered with ${o.wrote ?? "a key this computer could not read"}, not the ${o.pinned} you pinned; ssh wrote it into ${o.file}, and ssh-keygen -R ${o.target} -f ${o.file} takes it out`;
// }
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HostKeyMismatch {
    pub address: String,
    pub pinned: String,
    pub wrote: Option<String>,
    pub target: String,
    pub file: String,
}

pub fn host_key_mismatch_refusal(o: &HostKeyMismatch) -> String {
    let wrote = o.wrote.as_deref().unwrap_or("a key this computer could not read");
    format!(
        "{} answered with {wrote}, not the {} you pinned; ssh wrote it into {}, and ssh-keygen -R {} -f {} takes it out",
        o.address, o.pinned, o.file, o.target, o.file
    )
}

/// What a first dial says about the machine it reached: the host key it answered with, for the person to compare
/// against the machine's own before they trust the road. Printed once, when the workspace is recorded.
// computer.ts:546
// export function sshHostKeyNotice(hostKey: string): string {
//   return `its host key is ${hostKey}; compare it with the machine's own before a thread runs there`;
// }
pub fn ssh_host_key_notice(host_key: &str) -> String {
    format!("its host key is {host_key}; compare it with the machine's own before a thread runs there")
}

/// What a verb is refused with when this host wired no module for the kind it names.
// computer.ts:551
// export function noKindLine(kind: string): string {
//   return `this host has no ${kind} backend wired, so it serves nothing of that kind`;
// }
pub fn no_kind_line(kind: &str) -> String {
    format!("this host has no {kind} backend wired, so it serves nothing of that kind")
}

/// What import is refused with on a kind no road lands a folder on, said before the folder is read. Every kind
/// has a road today; the sentence stands for the next kind added without one, which is what the words table's
/// null import road means.
// computer.ts:558
// export function noImportRoadLine(name: string, machine: string): string {
//   return `${name} is ${machine}, which lands no folder yet; import to a fork, or register the folder on this computer`;
// }
pub fn no_import_road_line(name: &str, machine: &str) -> String {
    format!("{name} is {machine}, which lands no folder yet; import to a fork, or register the folder on this computer")
}

/// What a machine without a node the wsp command runs on is refused with. The daemon itself is one static binary
/// and asks nothing of the machine; the wsp command that rides beside it, which every turn's agent drives the host
/// through, still runs on node. A machine wsp builds carries the floor's; a machine somebody already owns may carry
/// none, or one too old. Said before the install rather than after, so nothing lands on a machine that refuses.
// computer.ts:566
pub const NO_NODE_LINE: &str = "this machine has no Node 22, which the wsp command beside the daemon runs on; install Node 22 or newer on it and deploy the daemon again";

/// What a machine whose login does not linger is refused with. Its own systemd stops when its last session ends
/// and takes the daemon with it, so a daemon deployed there is gone the moment the host's connection closes; the
/// person turns linger on once and it holds for every login after.
// computer.ts:572
pub const NO_LINGER_LINE: &str = "this login does not linger, so its services stop when you log out and the daemon would not outlive the connection; run loginctl enable-linger on it and deploy the daemon again";

/// What a machine whose daemon would be held up by systemd, and which has none, is refused with. An unsupervised
/// daemon is a daemon that goes with the first crash and never comes back from a reboot, and nothing on such a
/// machine would put it up again. Said before anything is installed rather than as the install starts a service
/// manager that is not there, so a machine that cannot take a daemon is left exactly as wsp found it.
// computer.ts:579
pub const NO_SYSTEMD_LINE: &str = "this machine runs no systemd, which is what starts the daemon here and brings it back after a reboot; deploy it on a machine that runs systemd";

/// Every sentence a machine's own checks refuse with, in one place beside them. Read by the rule that keeps each
/// one's first clause short enough for a row, so a refusal added later takes that rule without anyone remembering
/// where it is checked. Nothing decides anything by searching this: which ending a throw is comes off its mark.
// computer.ts:585
// export const MACHINE_LACKS_LINES: readonly string[] = [NO_NODE_LINE, NO_LINGER_LINE, NO_SYSTEMD_LINE];
pub const MACHINE_LACKS_LINES: [&str; 3] = [NO_NODE_LINE, NO_LINGER_LINE, NO_SYSTEMD_LINE];

/// The marks the checks a machine takes before a daemon is put on it end with, put on where the throw happens
/// rather than matched against text: a check added to a place's preflight is then one shell line and one sentence,
/// and nothing keeps a second copy of which sentences mean what. Two of them, because the two endings lead
/// opposite ways. A machine that answered and refused has said what it has not got, which a row shows and which
/// stands until a person puts that thing there. A check that never reached the machine has said nothing about it
/// either way: those are the client's own words, they belong in no row, and whatever the record already knew about
/// that machine still holds. Every other failure is a deploy log and carries neither mark.
// computer.ts:594
const MACHINE_LACKS: &str = "wspMachineLacks";

// computer.ts:595
const MACHINE_UNANSWERED: &str = "wspMachineUnanswered";

// computer.ts:597
// export function machineLacking(said: string): Error {
//   return Object.assign(new Error(said), { [MACHINE_LACKS]: true });
// }
/// A HostError with the message as given and the mark `wspMachineLacks`, nothing else set.
pub fn machine_lacking(said: &str) -> HostError {
    let mut error = HostError::new(said);
    error.marks.push(MACHINE_LACKS.to_string());
    error
}

// computer.ts:601
// export function machineUnanswered(said: string): Error {
//   return Object.assign(new Error(said), { [MACHINE_UNANSWERED]: true });
// }
/// A HostError with the message as given and the mark `wspMachineUnanswered`, nothing else set.
pub fn machine_unanswered(said: &str) -> HostError {
    let mut error = HostError::new(said);
    error.marks.push(MACHINE_UNANSWERED.to_string());
    error
}

/// The sentence a machine refused with, or undefined for every other failure.
// computer.ts:606
// export function machineLacksLine(e: unknown): string | undefined {
//   return marked(e, MACHINE_LACKS) ? e.message : undefined;
// }
pub fn machine_lacks_line(e: &HostError) -> Option<String> {
    marked(e, MACHINE_LACKS).then(|| e.message.clone())
}

/// Whether the check never reached the machine, so nothing about what that machine has was learned.
// computer.ts:611
// export function machineNeverAnswered(e: unknown): boolean {
//   return marked(e, MACHINE_UNANSWERED);
// }
pub fn machine_never_answered(e: &HostError) -> bool {
    marked(e, MACHINE_UNANSWERED)
}

/// A refusal cut to its first clause, which is what the machine has not got. Every sentence above is written in
/// that order, what is wrong, then why it matters, then what to do, and its head is short enough for a row about
/// thirty characters wide; the whole sentence goes where there is room, since the instruction is at the end of it
/// and a row that cut from the right would take the instruction off. A refusal added later is written to the same
/// shape rather than carrying a second, shorter copy of itself.
// computer.ts:620
// export function machineLacksShort(said: string): string {
//   return said.split(",")[0]!.trim();
// }
pub fn machine_lacks_short(said: &str) -> String {
    js_trim(said.split(',').next().unwrap_or("")).to_string()
}

/// The one sentence a socket a machine's requests arrive on is refused a ticket with. A ticket authenticates the
/// next socket, and a socket this host minted no relay ticket for is one of the person's own, so a machine that
/// could mint one would hand itself the origin the relay stamps on it.
// computer.ts:627
pub const RELAY_TICKET_REFUSAL: &str = "a request relayed from a machine cannot mint a ticket into this host";

/// The one sentence a workspace on a machine wsp does not run refuses a verb that machine cannot take with. This
/// computer and a machine reached over ssh are not machines wsp forks, pauses or snapshots, so the verbs that move a
/// fork have no meaning on either; `machine` is the kind's own word for what it is and `action` is the verb as the
/// person typed it. The capability behind each is false, so the road that reads the capability says this.
// computer.ts:633
// export function undrivenRefusal(name: string, machine: string, action: string): string {
//   return `${name} is ${machine}, which wsp does not run; it cannot ${action}`;
// }
pub fn undriven_refusal(name: &str, machine: &str, action: &str) -> String {
    format!("{name} is {machine}, which wsp does not run; it cannot {action}")
}

/// The one sentence a forget on a workspace wsp does not run the machine of is refused with. Such a machine is
/// never gone, so the sentence about a machine still standing at a provider says two impossible things on it: there
/// is no provider to pause it at, and the road that takes the record away is the delete, which asks a provider for
/// nothing either.
// computer.ts:641
// export function forgetUndrivenRefusal(name: string, machine: string): string {
//   return `${name} runs on ${machine}, which is not at a provider; run wsp delete ${name}`;
// }
pub fn forget_undriven_refusal(name: &str, machine: &str) -> String {
    format!("{name} runs on {machine}, which is not at a provider; run wsp delete {name}")
}

/// The one sentence a workspace on a machine wsp does run refuses a verb with when the provider under it has no
/// road for that verb: the machine is wsp's to move, so the refusal names the provider's limit rather than telling a
/// person their fork is their own computer. Each verb reads its own capability, so what is missing is that verb's
/// road and nothing else about the machine. The machine stays the subject the action was written for, since every
/// action is the machine's own verb phrase, and where the road is missing comes after it.
// computer.ts:650
// export function providerCannotRefusal(name: string, machine: string, action: string): string {
//   return `${name} is ${machine}, and it cannot ${action} on the provider it runs on`;
// }
pub fn provider_cannot_refusal(name: &str, machine: &str, action: &str) -> String {
    format!("{name} is {machine}, and it cannot {action} on the provider it runs on")
}

/// The row's line when a pause or a wake ran its deadline out, once and once more after the retry: which move, how
/// long it was given in all, and what the provider reads about the machine after it, or that the provider could not
/// be read. The person's road is to try again; the runtime never leaves the row at Pausing or Waking.
// computer.ts:657
// export function moveTimedOutLine(move: "pause" | "wake", elapsedMs: number, reads: MachineState | undefined): string {
//   const provider = reads === undefined ? "could not be read about the machine" : `reads the machine ${reads}`;
//   return `${move} did not complete in ${fmtDuration(elapsedMs)}; the provider did not answer and ${provider}; try again`;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Move {
    Pause,
    Wake,
}

/// `reads` is a MachineState's wire word.
pub fn move_timed_out_line(r#move: Move, elapsed_ms: f64, reads: Option<&str>) -> String {
    let word = match r#move {
        Move::Pause => "pause",
        Move::Wake => "wake",
    };
    let provider = match reads {
        None => "could not be read about the machine".to_string(),
        Some(state) => format!("reads the machine {state}"),
    };
    format!("{word} did not complete in {}; the provider did not answer and {provider}; try again", fmt_duration(elapsed_ms, None))
}

/// How many asks a window of asking holds at a cadence, which is the count the row counts against and the last ask
/// a wake makes. An ask starting exactly as the window runs out is not made, so a window of one cadence holds one.
/// The window and the cadence are the backend's to declare.
// computer.ts:665
// export const wakeAsksIn = (forMs: number, everyMs: number): number => Math.ceil(forMs / everyMs);
pub fn wake_asks_in(for_ms: f64, every_ms: f64) -> f64 {
    (for_ms / every_ms).ceil()
}

/// The row's line the moment a resume runs its cap out with nothing back. Not a refusal and not a failed resume: the
/// provider's mutating calls hang at the HTTP level while the operation goes through (probed 2026-09-10, a pause that
/// answered nothing in 30 s had the machine paused 40 s later), so the machine's own state is what settles it and the
/// sentence says that is what is happening next.
// computer.ts:671
pub const RESUME_UNANSWERED: &str = "the provider has not answered the resume request; reading the machine";

/// The line for an ask the host is making on its own: which ask this is, of the ones it will make. Two readings of
/// the one fact, since two surfaces show it: the long one is the Machine tab's, which has a line to spend on prose,
/// and the short one is the sidebar row's, whose meta slot holds a couple of words beside the spend and the nap
/// countdown and cuts from the right. The count is passed rather than worked out here so a runtime given a
/// shorter cadence says the number it will keep to.
// computer.ts:678
// export function wakeAskingAgainLine(ask: number, of: number, style: "long" | "short" = "long"): string {
//   return style === "short" ? `asking ${ask}/${of}` : `waking, asking again (${ask} of ${of})`;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LineStyle {
    Long,
    Short,
}

/// `style` defaults to Long.
pub fn wake_asking_again_line(ask: f64, of: f64, style: Option<LineStyle>) -> String {
    match style.unwrap_or(LineStyle::Long) {
        LineStyle::Short => format!("asking {}/{}", number_string(ask), number_string(of)),
        LineStyle::Long => format!("waking, asking again ({} of {})", number_string(ask), number_string(of)),
    }
}

/// The row's line, and what the wake ends with, when the person stopped the host asking from the row.
// computer.ts:683
pub const WAKE_STOPPED: &str = "waking stopped; the machine is still paused";

/// The row's line, and what the wake ends with, when the person stopped the host waiting for a machine the provider
/// runs to take commands: it stays up, and the next wake waits for it again.
// computer.ts:687
pub const WAKE_STOPPED_UP: &str = "waking stopped; the machine is up and has not taken a command yet";

/// What a wake ends with when the provider runs the machine and it still refuses commands at its backend's deadline,
/// as a Boat box does while its disk streams in behind a running reading: the provider's own words, and that a later
/// wake waits for it again.
// computer.ts:692
// export function noCommandsYetLine(name: string, said: string): string {
//   return `${name} is up and takes no commands yet (${said}); wake it again in a minute or two`;
// }
pub fn no_commands_yet_line(name: &str, said: &str) -> String {
    format!("{name} is up and takes no commands yet ({said}); wake it again in a minute or two")
}

/// The row's line once the host's asking ran out and the machine is still paused there, which is the fault probed on
/// 2026-09-10: machines paused for days never resume, while a fresh pause resumes in seconds. Three facts in the order
/// a person needs them: what the provider did, that their work is where they left it, and the one road to a machine
/// now. The disk clause is about the machine as it stands, not about the rebuild, which kills the old machine and
/// lands the nap-time vault instead; the rebuild's own dialog says that.
// computer.ts:701
// export function wakeGaveUpLine(asks: number, overMs: number): string {
//   return `the provider answered none of ${plural(asks, "resume request")} over ${fmtDuration(overMs)}; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image`;
// }
pub fn wake_gave_up_line(asks: f64, over_ms: f64) -> String {
    format!(
        "the provider answered none of {} over {}; the work on this machine's disk stays with the provider, and a rebuild starts a new machine from the image",
        plural(asks, "resume request"),
        fmt_duration(over_ms, None)
    )
}

/// What a wake ends with when the machine came back and failed every check its backend allows: the faults in order,
/// then that the workspace still stands on that machine, since its disk is the only copy of the agents' sessions and
/// of the work nobody pushed, and no wake trades it for a fresh fork of the image.
// computer.ts:708
// export function wakeFailedLine(machineId: string, faults: string): string {
//   return `the wake of ${machineId} did not finish: ${faults}; the workspace keeps this machine and its disk`;
// }
pub fn wake_failed_line(machine_id: &str, faults: &str) -> String {
    format!("the wake of {machine_id} did not finish: {faults}; the workspace keeps this machine and its disk")
}

/// What the needs-you road says outside the app when a machine came up while the person was looking elsewhere.
// computer.ts:713
// export const workspaceAwakeLine = (name: string): string => `${name} is awake`;
pub fn workspace_awake_line(name: &str) -> String {
    format!("{name} is awake")
}

/// What a stop of a thread on a computer the person joined says when what the thread started there did not all end:
/// the processes still running, by pid as the Processes pane lists them, or none where that computer did not answer.
// computer.ts:717
// export function threadLeftLine(name: string, pids: readonly number[]): string {
//   if (pids.length === 0) return `${name} did not say that what this thread started there has ended; look in Processes and end what still runs there`;
//   const one = pids.length === 1;
//   return `${one ? "a process" : `${pids.length} processes`} this thread started on ${name} still ${one ? "runs" : "run"} after Stop (${pids.join(", ")}); end ${one ? "it" : "them"} in Processes, or restart ${name}`;
// }
pub fn thread_left_line(name: &str, pids: &[i64]) -> String {
    if pids.is_empty() {
        return format!(
            "{name} did not say that what this thread started there has ended; look in Processes and end what still runs there"
        );
    }
    let one = pids.len() == 1;
    let list = pids.iter().map(|pid| pid.to_string()).collect::<Vec<_>>().join(", ");
    let count = if one { "a process".to_string() } else { format!("{} processes", pids.len()) };
    let (verb, object) = if one { ("runs", "it") } else { ("run", "them") };
    format!("{count} this thread started on {name} still {verb} after Stop ({list}); end {object} in Processes, or restart {name}")
}

/// What a stop of a running turn on a computer the person joined says when that computer is away or has not answered:
/// the thread reads stopped here, and the end of what it runs there is owed to that computer's next link.
// computer.ts:725
// export const threadEndAwayLine = (name: string): string => `${name} is not answering, so the turn ends there, with what this thread started, once ${name} connects again`;
pub fn thread_end_away_line(name: &str) -> String {
    format!("{name} is not answering, so the turn ends there, with what this thread started, once {name} connects again")
}

/// The same where that computer's link still stands but it did not answer a ping in time: it may only be slow, and the
/// end runs the moment it answers.
// computer.ts:729
// export const threadEndLateLine = (name: string, seconds: number): string => `${name} did not answer within ${seconds} s, so the turn ends there, with what this thread started, once ${name} answers`;
pub fn thread_end_late_line(name: &str, seconds: f64) -> String {
    format!(
        "{name} did not answer within {} s, so the turn ends there, with what this thread started, once {name} answers",
        number_string(seconds)
    )
}

/// What a send says while it waits on the end a stop owed a computer that was away, in the app and on the command line.
// computer.ts:732
// export const sendWaitsForLine = (name: string): string => `waiting for ${name} to connect; this message runs once the stopped turn has ended there`;
pub fn send_waits_for_line(name: &str) -> String {
    format!("waiting for {name} to connect; this message runs once the stopped turn has ended there")
}

/// What a send that waited on such an end says once its turn started.
// computer.ts:735
// export const sendWaitedForLine = (name: string): string => `${name} connected and the stopped turn ended there; this turn started`;
pub fn send_waited_for_line(name: &str) -> String {
    format!("{name} connected and the stopped turn ended there; this turn started")
}

/// What a Stop or a delete of a thread says of a message it gave up while that message waited on such an end, and what
/// the send itself is answered.
// computer.ts:739
// export const sendGivenUpLine = (name: string): string => `the message waiting for ${name} to connect was given up and never ran`;
pub fn send_given_up_line(name: &str) -> String {
    format!("the message waiting for {name} to connect was given up and never ran")
}

/// What a delete of a thread on a computer the person joined says when what the thread started there did not all end:
/// the delete goes on, and that computer's next link ends it.
// computer.ts:743
// export const threadEndOwedLine = (name: string): string => `what this thread started on ${name} did not all end; wsp ends it when ${name} next connects`;
pub fn thread_end_owed_line(name: &str) -> String {
    format!("what this thread started on {name} did not all end; wsp ends it when {name} next connects")
}

