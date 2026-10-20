// SPDX-License-Identifier: AGPL-3.0-only
//! The rows the transcript draws and what each row carries.

use super::{Listed, StartFailed, ThreadAt, Wait};

/// Which lucide glyph a tool line draws (TOOL_GROUP_GLYPHS and WORK_TONES in the web app's timeline/workEntry.tsx).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Glyph {
    Eye,
    SquarePen,
    FilePlus,
    Terminal,
    Globe,
    Search,
    Wrench,
    Hammer,
    Bot,
    Zap,
    Brain,
    Info,
    CircleAlert,
    Minimize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tone {
    Thinking,
    Tool,
    Notice,
    Error,
    Compaction,
}

/// One call inside an expanded tool group (ExpandedWorkGroupEntries in the web app's timeline/workGroup.tsx).
#[derive(Clone, Debug, PartialEq)]
pub struct Call {
    pub key: String,
    pub glyph: Glyph,
    pub tone: Tone,
    /// A verb drawn in the label ink before `label`, which then draws muted ("Edited" + "src/a.ts"); None draws `label` alone.
    pub verb: Option<String>,
    pub label: String,
    /// The label is a command or a path, drawn in the mono face.
    pub mono: bool,
    /// Some for a shell call: drawn as a command block (CommandCallRow in timeline/callBlock.tsx).
    pub command: Option<String>,
    pub running: bool,
    pub failed: bool,
    pub exit_code: Option<i64>,
    pub duration_ms: Option<f64>,
    /// What opening the row shows: a command's output, a tool's detail, a patch.
    pub body: Option<String>,
    /// A unified patch for an edit, drawn as a diff (EditCallRow in timeline/callBlock.tsx).
    pub patch: Option<String>,
    /// The edit's files, each drawn under its own head when there are several.
    pub files: Vec<FilePatch>,
    /// The host kept only part of the edit's patch (`patchCut`).
    pub cut: bool,
    /// The whole output's size in bytes when the host kept only its first part.
    pub bytes: Option<u64>,
    /// When a running command started, for its timer.
    pub since: Option<f64>,
    pub open: bool,
}

/// One file of an edit: its path from the thread's folder, its own unified patch and counts, and the line Open diff opens at.
#[derive(Clone, Debug, PartialEq)]
pub struct FilePatch {
    pub path: String,
    pub text: String,
    pub added: usize,
    pub removed: usize,
    /// Written whole, so its head counts its lines (`isNewFile`).
    pub created: bool,
    pub line: u64,
}

/// One choice on a prompt: a consent's option, or one answer to a question with the sentence under it.
#[derive(Clone, Debug, PartialEq)]
pub struct PromptOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
    /// allow, deny, mode or answer.
    pub effect: String,
    /// The access mode a mode option moves the turn to.
    pub mode: Option<String>,
}

/// One question a call put to the person (AskedQuestion in packages/protocol/src/words/tools.ts).
#[derive(Clone, Debug, PartialEq)]
pub struct Asked {
    pub header: Option<String>,
    pub question: String,
    pub options: Vec<PromptOption>,
    pub multiple: bool,
}

/// One line in a subagent's fold: prose, or a mono call with its answer in `detail`; `text` is empty for an answer with no call.
#[derive(Clone, Debug, PartialEq)]
pub struct SubLine {
    pub text: String,
    pub mono: bool,
    pub detail: Option<String>,
}

/// A child a call started: a thread by its id, or one of the agent's subagents.
#[derive(Clone, Debug, PartialEq)]
pub struct Child {
    pub thread: Option<String>,
    pub title: String,
    pub note: Option<String>,
    /// A subagent's state: running, done, failed or stopped; None for a thread, whose state the listing holds.
    pub status: Option<String>,
    pub subagent: bool,
    pub settled: bool,
    /// A subagent's launching call, which its page opens by.
    pub call: Option<String>,
    /// A thread's row in the listing, once it holds one: its agent, its state and where it runs.
    pub listed: Option<Listed>,
    /// The lead's workspace, which a subagent's page opens in.
    pub workspace: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChangedFile {
    pub path: String,
    pub added: u64,
    pub removed: u64,
}

/// A reply's shell block run (`session.run`): the block's index among the reply's fenced blocks.
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    pub run_id: String,
    pub block: usize,
    pub command: String,
    /// running, exited, moved or lost.
    pub state: String,
    pub output: Option<String>,
    pub exit_code: Option<i64>,
    pub signal: Option<String>,
    /// The pty it runs or ran in, on the workspace's daemon.
    pub pty_id: Option<String>,
}

impl Run {
    /// runEndedWords in packages/protocol/src/run-block.ts.
    pub fn words(&self) -> String {
        match (self.state.as_str(), &self.signal) {
            ("moved", _) => "Moved to a terminal tab".to_owned(),
            ("lost", _) => "Ended unseen: its terminal went away first".to_owned(),
            ("running", _) => "Running".to_owned(),
            (_, Some(signal)) => format!("Stopped by signal {signal}"),
            _ => format!("Exited {}", self.exit_code.unwrap_or(0)),
        }
    }
}

/// What the thread's agent can do, off its HarnessCatalog: Fork from here and the conversation half of Rewind to here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Can {
    pub forks: bool,
    pub forks_by_count: bool,
    pub rewinds_conversation: bool,
    pub rewinds_by_count: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FooterWord {
    Interrupted,
    Failed,
    LimitReached,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RowKind {
    /// What the person sent (row.text, newlines as breaks); `fork` is the turn Fork from here forks at, empty on the first message;
    /// `full`: the person asked for the whole of a long message, toggled by "full:" + key; `steered`: it was sent
    /// while the agent worked, which its bubble's hover says.
    Person { at: Option<f64>, files: Vec<FileTile>, fork: Option<String>, full: bool, steered: bool },
    /// An agent message; `meta`: the settled turn's final reply, with a 16 px foot and a hover-only meta line.
    Agent { at: Option<f64>, meta: bool, streaming: bool, rewind: Option<String>, fork: Option<String>, runs: Vec<Run> },
    /// "Worked for 12s" (row.text) over a settled turn's hidden work; toggles `turn`.
    Fold { open: bool },
    /// "You stopped after 4.2s" (row.text) after an interrupted turn's last row.
    Stopped,
    /// A collapsed or expanded tool group's summary line (row.text); toggles `group`.
    Group { group: String, glyph: Glyph, open: bool },
    /// The running turn's trailing calls as one line (LiveWorkEntryTimelineRow in timeline/workGroup.tsx); `active` shimmers.
    Live { group: String, glyph: Glyph, verb: Option<String>, mono: bool, since: Option<f64>, open: bool, active: bool },
    /// The calls of the open group right above it.
    Calls { group: String, calls: Vec<Call> },
    /// A standalone work row (notice, compaction, a lone non-tool entry): row.text with its tone's glyph; `body` is what
    /// opening it shows, toggled by the row's key.
    Work { tone: Tone, glyph: Glyph, body: Option<String>, open: bool },
    /// A failed turn's error (row.text), in the destructive ink, never folded.
    Error,
    /// "Working for 12s" over the running turn; `waiting`: a prompt waits on the person, so it reads "Waiting for you";
    /// `retry`: the agent is making its model call again, which the line says in its place (`retryLine`); `wait`: the
    /// workspace cannot run the turn, which the line says in its place in the rule grammar (`machineWait`).
    Working { since: Option<f64>, waiting: bool, retry: Option<String>, wait: Option<Wait> },
    /// The running turn's last row while no call is live.
    Thinking,
    /// The latest turn ended without a reply: one mono word under the transcript; `why`: a stop that never reached the
    /// computer, which says so under the word.
    Footer { word: FooterWord, why: Option<String> },
    /// A permission prompt (row.text is `lead` and `code` on one line); sessions.answer takes `session` and `ask`.
    /// `fields` are the inputs the lead leaves out; `file` is a write's body, folded away; `outcome` is a closed prompt's line.
    Permission {
        ask: String,
        session: String,
        asker: Option<String>,
        glyph: Glyph,
        lead: String,
        code: Option<String>,
        fields: Vec<(String, String)>,
        file: Option<String>,
        options: Vec<PromptOption>,
        outcome: Option<String>,
        open: bool,
    },
    /// A call that asks the person something (row.text is its first question); `outcome` once answered.
    Question { ask: String, session: String, asker: Option<String>, questions: Vec<Asked>, outcome: Option<String> },
    /// A proposed plan: row.text is the plan's markdown, whole (`plan_folds`, `plan_shown`); toggle id "plan:" + key.
    Plan { title: String, open: bool },
    /// A subagent's fold, toggled by `id`; its prompts follow it as Permission and Question rows while it is open.
    Subagent { id: String, title: String, state: Option<String>, open: bool, lines: Vec<SubLine> },
    /// The children a run of calls started, as their rows.
    Spawn { children: Vec<Child> },
    /// What a settled turn changed, under its final reply.
    /// `range` is the turn's snapshots at its launch and its end, which Open diff opens the Changes pane on.
    ChangedFiles { label: String, files: Vec<ChangedFile>, others: Vec<ChangedFile>, range: (String, String) },
    /// "Forked from {source}" (row.text) where a fork's own history starts; `open` while the listing holds that thread.
    Fork { source: String, open: Option<ThreadAt> },
    /// A sub-thread's machine start that failed, under the message it was for, with Try again.
    StartFailed { failed: StartFailed },
    /// The Threads section under the transcript: the threads this one's agent opened.
    Threads { open: bool, children: Vec<Listed> },
    /// One quiet line about the workspace in the rule grammar (row.text): the machine line.
    Rule,
    /// "Updated the slate" (row.text) under the last reply of a turn the agent wrote the slate in; a press opens it.
    Slate,
}

impl RowKind {
    /// The variant's name, as a trace says it.
    pub fn name(&self) -> &'static str {
        match self {
            RowKind::Person { .. } => "Person",
            RowKind::Agent { .. } => "Agent",
            RowKind::Fold { .. } => "Fold",
            RowKind::Stopped => "Stopped",
            RowKind::Group { .. } => "Group",
            RowKind::Live { .. } => "Live",
            RowKind::Calls { .. } => "Calls",
            RowKind::Work { .. } => "Work",
            RowKind::Error => "Error",
            RowKind::Working { .. } => "Working",
            RowKind::Thinking => "Thinking",
            RowKind::Footer { .. } => "Footer",
            RowKind::Permission { .. } => "Permission",
            RowKind::Question { .. } => "Question",
            RowKind::Plan { .. } => "Plan",
            RowKind::Subagent { .. } => "Subagent",
            RowKind::Spawn { .. } => "Spawn",
            RowKind::ChangedFiles { .. } => "ChangedFiles",
            RowKind::Fork { .. } => "Fork",
            RowKind::StartFailed { .. } => "StartFailed",
            RowKind::Threads { .. } => "Threads",
            RowKind::Rule => "Rule",
            RowKind::Slate => "Slate",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileTile {
    pub name: String,
    pub bytes: Option<u64>,
    pub image: bool,
    /// What sessions.attachment fetches: the send's request id, and the file's place among its attachments.
    pub id: Option<String>,
    pub index: usize,
    pub mime: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// Stays the same for the same row across folds and live events, so the view keeps what it built for it.
    pub key: String,
    pub kind: RowKind,
    pub text: String,
    pub turn: Option<String>,
}

/// The rows changed by one fold step: rows[start..start+removed] were replaced by rows[start..start+inserted].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Splice {
    pub start: usize,
    pub removed: usize,
    pub inserted: usize,
}
