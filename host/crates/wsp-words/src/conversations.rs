// SPDX-License-Identifier: AGPL-3.0-only
//! The words of a conversation a thread picks up from an agent's own store: the shapes an adapter reads it into,
//! the command that stamps its folders, and the lines and refusals a resume says. Port of the functions of
//! packages/protocol/src/conversations.ts at a3b870a6e; its zod shapes and flat constants are
//! wsp_wire::types::conversations. See spec/conversations.md.

use wsp_wire::types::conversations::ConversationOrigin;

use serde::Serialize;

use crate::exit::HostError;

// Who opened a conversation, as its agent's store says: the person in a terminal or an editor, wsp, or a script.
// conversations.ts:10
// in wsp_wire::types::conversations: ConversationOrigin

// conversations.ts:11
// in wsp_wire::types::conversations: ConversationOrigin

// One conversation in a project's folder or one of its worktrees. `lastAt` is ms epoch, `bytes` the size of the file
// the agent keeps it in where that was read; `live` says a process outside wsp holds it open now; `thread` is the wsp
// thread that already runs on it; `letsGo` says, in the agent's own sentence, that it refuses a second writer and
// how soon it lets go of a closed one, so trying again later is an answer.
// conversations.ts:17
// in wsp_wire::types::conversations: OutsideConversation

// conversations.ts:31
// in wsp_wire::types::conversations: OutsideConversation

// One agent whose conversations were not read, in two halves; `why` is the store's own error where the read failed
// for any reason but the computer's update.
// conversations.ts:35
// in wsp_wire::types::conversations: ConversationsHeld

// conversations.ts:36
// in wsp_wire::types::conversations: ConversationsHeld

// A project's conversations, newest first, and the agents whose store did not answer.
// conversations.ts:39
// in wsp_wire::types::conversations: ConversationsAnswer

// conversations.ts:40
// in wsp_wire::types::conversations: ConversationsAnswer

// What a start names to pick one up: the agent's id for it, and whether the thread runs on a copy of it.
// conversations.ts:43
// in wsp_wire::types::conversations: ResumeAsk

// conversations.ts:44
// in wsp_wire::types::conversations: ResumeAsk

/// One row of a conversation's earlier messages: the person's, the agent's, or one tool call by the agent's own name
/// for the tool with its input as text, and when the agent wrote it, ms epoch, where its store says.
// conversations.ts:48
// export interface ConversationLine {
//   who: "person" | "agent" | "tool";
//   text: string;
//   tool?: string;
//   at?: number;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LineWho {
    Person,
    Agent,
    Tool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConversationLine {
    pub who: LineWho,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "crate::js::serialize_opt_f64")]
    pub at: Option<f64>,
}

/// A conversation as an adapter reads it off its agent's store; the runtime adds what only wsp knows.
// conversations.ts:56
// export interface StoredConversation {
//   id: string;
//   title: string;
//   firstPrompt?: string;
//   branch?: string;
//   cwd: string;
//   lastAt: number;
//   bytes?: number;
//   origin: ConversationOrigin;
// }
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoredConversation {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_prompt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    pub cwd: String,
    #[serde(serialize_with = "crate::js::serialize_f64")]
    pub last_at: f64,
    #[serde(skip_serializing_if = "Option::is_none", serialize_with = "crate::js::serialize_opt_f64")]
    pub bytes: Option<f64>,
    pub origin: ConversationOrigin,
}

/// The earlier conversation a thread opens with: where it ran, its name, its newest messages and how many came before.
// conversations.ts:68
// export interface ConversationEarlier {
//   cwd?: string;
//   title?: string;
//   lines: ConversationLine[];
//   earlier: number;
// }
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct ConversationEarlier {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub lines: Vec<ConversationLine>,
    #[serde(serialize_with = "crate::js::serialize_f64")]
    pub earlier: f64,
}

// How an adapter reaches the computer the store is on: one shell line, or one frame to its daemon.
// conversations.ts:76
// export interface ConversationRoad {
//   exec: HarnessExec;
//   ask: (frame: { op: string } & Record<string, unknown>) => Promise<Record<string, unknown>>;
// }
// The runtime's road to an adapter's store (async calls over a computer); ported with the adapters, not here.

// An agent's own store of conversations, read on the project's computer. `list` answers the conversations whose
// recorded folder is one of `cwds`; `live` the ids a running process holds now, null where nothing could say;
// `stamp` a few words that change when a conversation is added to the folders' store, read without starting the agent;
// `earlier` one conversation's newest messages, gone where the store holds no such id, and open where `probe` asked it
// and another process is writing it; `tail` asks a store that reads a file to read only that many bytes of its end,
// and `count: false` lets a store skip what it would read only to count the messages before them, `earlier` then 0. `letsGo` says the agent refuses a second writer and lets go of a closed one,
// and how soon, in a sentence.
// conversations.ts:88
// export interface ConversationStore {
//   list(cwds: readonly string[], road: ConversationRoad): Promise<StoredConversation[]>;
//   live?(road: ConversationRoad): Promise<ReadonlySet<string> | null>;
//   stamp?(cwds: readonly string[], road: ConversationRoad): Promise<string>;
//   earlier(id: string, o: { cwds: readonly string[]; last: number; probe: boolean; tail?: number; count?: false }, road: ConversationRoad): Promise<ConversationEarlier | "gone" | "open">;
//   readonly letsGo?: string;
// }
// The adapter's store contract (async reads); ported with the adapters, not here.

/// A store read that fails the same way until something changes on the computer, the agent not installed there; a
/// list kept with one answers the visits of its minute, where any other failure is read again on the next visit.
// conversations.ts:98
// export const lastingStoreError = (message: string): Error => Object.assign(new Error(message), { lasting: true });
/// The TS `lasting: true` is the mark "lasting" on the error.
pub fn lasting_store_error(message: &str) -> HostError {
    let mut error = HostError::new(message);
    error.marks.push("lasting".to_string());
    error
}

// conversations.ts:99
// export const isLastingStoreError = (e: unknown): boolean => e instanceof Error && (e as { lasting?: unknown }).lasting === true;
pub fn is_lasting_store_error(e: &HostError) -> bool {
    crate::words::base::marked(e, "lasting")
}

/// A line per folder, each a shell word: its mtime in seconds and how many names it holds, blank where it is not
/// there; GNU stat, then BSD's. A transcript or rollout added moves its folder, one appended to does not.
// conversations.ts:103
// export const foldersStampCommand = (folders: readonly string[]): string =>
//   `for d in ${folders.join(" ")}; do printf '%s %s\\n' "$(stat -c %Y "$d" 2>/dev/null || stat -f %m "$d" 2>/dev/null)" "$(ls -A "$d" 2>/dev/null | wc -l)"; done`;
pub fn folders_stamp_command(folders: &[impl AsRef<str>]) -> String {
    let list = folders.iter().map(|folder| folder.as_ref()).collect::<Vec<_>>().join(" ");
    format!("for d in {list}; do printf '%s %s\\n' \"$(stat -c %Y \"$d\" 2>/dev/null || stat -f %m \"$d\" 2>/dev/null)\" \"$(ls -A \"$d\" 2>/dev/null | wc -l)\"; done")
}

// conversations.ts:107
// in wsp_wire::types::conversations: EARLIER_MESSAGES

// conversations.ts:110
// in wsp_wire::types::conversations: PREVIEW_MESSAGES

// conversations.ts:111
// in wsp_wire::types::conversations: PREVIEW_TAIL_BYTES

// One conversation read before any thread opens on it: where it ran, its name, and its newest rows as the thread's
// transcript would open with them, a tool call as its one line and a note for the messages left out, each at the time
// its agent wrote it where the store says.
// conversations.ts:116
// in wsp_wire::types::conversations: ConversationRead

// conversations.ts:121
// in wsp_wire::types::conversations: ConversationRead

// conversations.ts:124
// in wsp_wire::types::conversations: CONVERSATION_OPEN_KIND

// conversations.ts:126
// export const CONVERSATION_WORDS = {
//   menu: "Resume a conversation...",
//   row: "Resume a conversation",
//   search: "Search conversations...",
//   openProjectFirst: "Open a project first",
//   noMatch: "No matching conversations.",
//   allAgents: "All agents",
//   allBranches: "All branches",
//   allProjects: "All projects",
//   onBranch: (branch: string): string => `On ${branch}`,
//   everyProject: "Conversations in every project",
//   emptyIn: (project: string): string => `No conversations in ${project} yet`,
//   runsIn: "Runs in",
//   previewReading: "Reading the last messages",
//   earlierFold: "Earlier, outside wsp",
//   navigate: "Navigate",
//   close: "Close",
//   live: "Open in another app",
//   thread: "Already a thread",
//   startFresh: "Start fresh",
//   send: "Continue",
//   copy: "Continue in a copy",
//   tryAgain: "Try again",
//   cancel: "Cancel",
//   group: (project: string): string => `Conversations in ${project}`,
//   reading: (computer: string): string => `Reading conversations on ${computer}`,
//   continues: (title: string, copy: boolean): string => (copy ? `Continues a copy of ${title}` : `Continues ${title}`),
//   openTitle: (title: string, computer: string): string => `${title} is open in another app on ${computer}`,
//   openNote: (letsGo: string | undefined): string =>
//     letsGo === undefined ? "Continuing it here too would split it. Continue in a copy, or close it there first." : `Continue in a copy, or close it there and try again. ${letsGo}`,
// } as const;
/// The TS CONVERSATION_WORDS: its fixed words as constants, its sentences as functions.
pub mod conversation_words {
    pub const MENU: &str = "Resume a conversation...";
    pub const ROW: &str = "Resume a conversation";
    pub const SEARCH: &str = "Search conversations...";
    pub const OPEN_PROJECT_FIRST: &str = "Open a project first";
    pub const NO_MATCH: &str = "No matching conversations.";
    pub const ALL_AGENTS: &str = "All agents";
    pub const ALL_BRANCHES: &str = "All branches";
    pub const ALL_PROJECTS: &str = "All projects";
    pub const EVERY_PROJECT: &str = "Conversations in every project";
    pub const RUNS_IN: &str = "Runs in";
    pub const PREVIEW_READING: &str = "Reading the last messages";
    pub const EARLIER_FOLD: &str = "Earlier, outside wsp";
    pub const NAVIGATE: &str = "Navigate";
    pub const CLOSE: &str = "Close";
    pub const LIVE: &str = "Open in another app";
    pub const THREAD: &str = "Already a thread";
    pub const START_FRESH: &str = "Start fresh";
    pub const SEND: &str = "Continue";
    pub const COPY: &str = "Continue in a copy";
    pub const TRY_AGAIN: &str = "Try again";
    pub const CANCEL: &str = "Cancel";
    pub fn on_branch(branch: &str) -> String {
        format!("On {branch}")
    }
    pub fn empty_in(project: &str) -> String {
        format!("No conversations in {project} yet")
    }
    pub fn group(project: &str) -> String {
        format!("Conversations in {project}")
    }
    pub fn reading(computer: &str) -> String {
        format!("Reading conversations on {computer}")
    }
    pub fn continues(title: &str, copy: bool) -> String {
        if copy {
            format!("Continues a copy of {title}")
        } else {
            format!("Continues {title}")
        }
    }
    pub fn open_title(title: &str, computer: &str) -> String {
        format!("{title} is open in another app on {computer}")
    }
    pub fn open_note(lets_go: Option<&str>) -> String {
        match lets_go {
            None => "Continuing it here too would split it. Continue in a copy, or close it there first.".to_string(),
            Some(lets_go) => format!("Continue in a copy, or close it there and try again. {lets_go}"),
        }
    }
}

/// The line that stands for the messages a thread did not open with.
// conversations.ts:159
// export const earlierLine = (count: number, agent: string): string => `${count} earlier ${count === 1 ? "message stays" : "messages stay"} in ${agent}'s own history`;
pub fn earlier_line(count: f64, agent: &str) -> String {
    let stays = if count == 1.0 { "message stays" } else { "messages stay" };
    format!("{} earlier {stays} in {agent}'s own history", crate::js::number_string(count))
}

// conversations.ts:161
// export const conversationOpenLine = (title: string, computer: string): string => `${CONVERSATION_WORDS.openTitle(title, computer)}, so continuing it here would split it`;
pub fn conversation_open_line(title: &str, computer: &str) -> String {
    format!("{}, so continuing it here would split it", conversation_words::open_title(title, computer))
}

// conversations.ts:162
// export const conversationOpenFix = (letsGo: string | undefined): string => `Continue in a copy with --copy, or close it there and try again.${letsGo === undefined ? "" : ` ${letsGo}`}`;
pub fn conversation_open_fix(lets_go: Option<&str>) -> String {
    match lets_go {
        None => "Continue in a copy with --copy, or close it there and try again.".to_string(),
        Some(lets_go) => format!("Continue in a copy with --copy, or close it there and try again. {lets_go}"),
    }
}

/// A turn the agent would not open because another process writes the conversation, in two halves.
// conversations.ts:164
// export const conversationWrittenElsewhereLine = (agent: string, letsGo: string | undefined): string =>
//   refusalLine(`${agent} is writing this conversation in another app, so this turn did not run`, `Close it there and send again.${letsGo === undefined ? "" : ` ${letsGo}`}`);
pub fn conversation_written_elsewhere_line(agent: &str, lets_go: Option<&str>) -> String {
    let fix = match lets_go {
        None => "Close it there and send again.".to_string(),
        Some(lets_go) => format!("Close it there and send again. {lets_go}"),
    };
    crate::words::thread::refusal_line(&format!("{agent} is writing this conversation in another app, so this turn did not run"), &fix)
}

// conversations.ts:166
// export const conversationGoneLine = (agent: string, id: string, computer: string): string => `${agent} holds no conversation ${id} on ${computer} any more`;
pub fn conversation_gone_line(agent: &str, id: &str, computer: &str) -> String {
    format!("{agent} holds no conversation {id} on {computer} any more")
}

// conversations.ts:167
// in wsp_wire::types::conversations: CONVERSATION_GONE_FIX

// conversations.ts:168
// export const conversationIsThreadLine = (id: string, thread: string): string => `conversation ${id} is thread ${thread.slice(0, 8)} already`;
pub fn conversation_is_thread_line(id: &str, thread: &str) -> String {
    format!("conversation {id} is thread {} already", crate::js::utf16::cut(thread, 8))
}

// conversations.ts:169
// export const conversationIsThreadFix = (thread: string): string => `Send into it with wsp send ${thread.slice(0, 8)}.`;
pub fn conversation_is_thread_fix(thread: &str) -> String {
    format!("Send into it with wsp send {}.", crate::js::utf16::cut(thread, 8))
}

// conversations.ts:170
// export const noConversationsLine = (agent: string): string => `wsp cannot pick up ${agent} conversations`;
pub fn no_conversations_line(agent: &str) -> String {
    format!("wsp cannot pick up {agent} conversations")
}

// conversations.ts:171
// in wsp_wire::types::conversations: NO_CONVERSATIONS_FIX

// conversations.ts:172
// in wsp_wire::types::conversations: RESUME_WHERE_LINE

// conversations.ts:173
// in wsp_wire::types::conversations: RESUME_WHERE_FIX

// conversations.ts:174
// in wsp_wire::types::conversations: RESUME_A_THREAD_LINE

// conversations.ts:175
// in wsp_wire::types::conversations: RESUME_A_THREAD_FIX

// conversations.ts:176
// in wsp_wire::types::conversations: RESUME_HERE_LINE

// conversations.ts:177
// in wsp_wire::types::conversations: RESUME_HERE_FIX

// conversations.ts:178
// in wsp_wire::types::conversations: COPY_ALONE_LINE

// conversations.ts:179
// in wsp_wire::types::conversations: COPY_ALONE_FIX

// conversations.ts:180
// export const conversationsUnreadLine = (agent: string, computer: string): string => `${agent} conversations on ${computer} were not read`;
pub fn conversations_unread_line(agent: &str, computer: &str) -> String {
    format!("{agent} conversations on {computer} were not read")
}

// conversations.ts:181
// export const conversationsFailedLine = (agent: string, why: string): string => `${agent}'s conversations could not be read: ${why}`;
pub fn conversations_failed_line(agent: &str, why: &str) -> String {
    format!("{agent}'s conversations could not be read: {why}")
}

// conversations.ts:182
// in wsp_wire::types::conversations: CONVERSATIONS_AGAIN_FIX

// conversations.ts:183
// export const conversationsUpdateFix = (computer: string): string => `Update ${computer} with wsp add ${computer} --update to list them.`;
pub fn conversations_update_fix(computer: &str) -> String {
    format!("Update {computer} with wsp add {computer} --update to list them.")
}

