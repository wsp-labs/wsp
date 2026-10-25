// SPDX-License-Identifier: AGPL-3.0-only
//! The one contract an agent reads wsp by: with --json stdout carries JSON alone, every refusal or failure is one
//! line on stderr, and the exit code is the class the error belongs to. The class is read off the kind stamped on
//! the error where it was born, never off its words. Port of packages/protocol/src/exit.ts at a3b870a6e, with the
//! one error type of spec chapter 05 (Proposal 4) standing where the TS stamps fields on an `Error`. See spec/exit.md.

use serde::{Deserialize, Serialize};

/// exit.ts:12. Serialized as "ok", "provider", "auth", "usage".
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExitClass {
    Ok,
    Provider,
    Auth,
    Usage,
}

impl ExitClass {
    /// Every class in the TS enum's order: ok, provider, auth, usage (exit.ts:12).
    pub const ALL: [ExitClass; 4] = [ExitClass::Ok, ExitClass::Provider, ExitClass::Auth, ExitClass::Usage];

    /// `EXIT_CODES[class]`: ok 0, provider 1, auth 2, usage 3 (exit.ts:16).
    pub fn code(self) -> i32 {
        todo!()
    }

    /// `EXIT_WORDS[class]`, the words the skill, the help and the instructions quote (exit.ts:19-24), word for word.
    pub fn words(self) -> &'static str {
        todo!()
    }

    /// The class's wire word: "ok", "provider", "auth", "usage".
    pub fn as_str(self) -> &'static str {
        todo!()
    }
}

/// A settle or a delete refused while a thread still runs processes, carrying them for a client to ask about
/// (exit.ts:31).
pub const PROCESSES_LEFT_KIND: &str = "processes-left";

/// How much of one line a refusal keeps where it outlives its answer, in UTF-16 units (exit.ts:92, rule 8.4).
pub const SAID_LINE_CHARS: usize = 400;

/// The lines a kept refusal keeps: its own sentence and the ten a computer's log adds under it (exit.ts:94).
pub const SAID_LINES: usize = 11;

/// The kind stamped on an error where it was born. Every kind spec chapter 05's table lists is a variant, written
/// as the TS string; any other string the TS stamps or a reply carries (`unavailable`, `quota`, `provider` among
/// them) is `Other` with that string, kept as it came. Serialized as the bare string.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Kind {
    Usage,
    Invalid,
    NotFound,
    ProcessesLeft,
    Auth,
    Ticket,
    Conflict,
    Concurrency,
    StartLimit,
    Plan,
    Missing,
    SnapshotUnavailable,
    Transient,
    Unknown,
    NotFirstLife,
    MachineAlive,
    SnapshotFailed,
    CredentialOnBuilder,
    NoProvider,
    VaultTooLarge,
    Exists,
    SshInclude,
    HostKey,
    SshBehind,
    Unreachable,
    Refused,
    Reauth,
    Other(String),
}

impl Kind {
    /// The TS string: "usage", "invalid", "not-found", "processes-left", "auth", "ticket", "conflict",
    /// "concurrency", "startLimit", "plan", "missing", "snapshotUnavailable", "transient", "unknown",
    /// "notFirstLife", "machineAlive", "snapshotFailed", "credentialOnBuilder", "noProvider", "vaultTooLarge",
    /// "exists", "sshInclude", "host-key", "sshBehind", "unreachable", "refused", "reauth"; `Other` its own.
    pub fn as_str(&self) -> &str {
        todo!()
    }

    /// The variant a string names; a string no variant names is `Other` holding it, so `parse(k).as_str() == k`
    /// for every string.
    pub fn parse(s: &str) -> Kind {
        todo!()
    }

    /// `KIND_CLASS[kind] ?? "provider"` (exit.ts:34,39): usage, invalid, not-found and processes-left are usage,
    /// auth is auth, every other kind (an `Other` included, even `Other("usage")` cannot exist since `parse` maps it)
    /// is the provider's. Never `Ok`.
    pub fn class(&self) -> ExitClass {
        todo!()
    }
}

impl Serialize for Kind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        todo!()
    }
}

impl<'de> Deserialize<'de> for Kind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        todo!()
    }
}

/// An error as every door reads it: the wire reply, the failure object, the MCP tool error and the log line. Fields
/// are what the TS stamps on its `Error` (exit.ts:27-67, spec chapter 05 Proposal 4).
#[derive(Clone, Debug, PartialEq)]
pub struct HostError {
    /// The sentence: `refusal_line(happened, fix)` for a refusal, else the message.
    pub message: String,
    /// What to do, where the error was built as a refusal.
    pub fix: Option<String>,
    /// The kind stamped where the error was born; None classes as provider.
    pub kind: Option<Kind>,
    /// A slate write's validator errors.
    pub errors: Option<Vec<serde_json::Value>>,
    /// A slate write's validator warnings.
    pub warnings: Option<Vec<serde_json::Value>>,
    /// The threads and processes a settle or delete was refused over; read only when the kind is ProcessesLeft.
    pub left: Option<Vec<serde_json::Value>>,
    /// The names of the boolean marks the TS stamps on an `Error` as `true` (`wspMachineLacks`,
    /// `wspMachineUnanswered`, words/computer.ts:594-603), read by `marked`. Never on the wire, never in a reply.
    pub marks: Vec<String>,
}

impl HostError {
    /// `new Error(message)`: no fix, no kind, no lists.
    pub fn new(message: impl Into<String>) -> HostError {
        todo!()
    }

    /// `Object.assign(new Error(message), { kind })`: the message as given, no fix.
    pub fn with_kind(message: impl Into<String>, kind: Kind) -> HostError {
        todo!()
    }

    /// `exitClassOf(e)` (exit.ts:37-40): the kind's class, provider where there is no kind.
    pub fn class(&self) -> ExitClass {
        todo!()
    }

    /// `EXIT_CODES[exitClassOf(e)]`: 1, 2 or 3, never 0.
    pub fn exit_code(&self) -> i32 {
        todo!()
    }
}

impl std::fmt::Display for HostError {
    /// The message alone, as `e.message`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl std::error::Error for HostError {}

/// A failure as --json prints it on stderr and as a tool error carries it beside its text (exit.ts:27). Keys in this
/// order: error, class, exit, then errors and warnings only where the error carried them as lists.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerbFailure {
    pub error: String,
    /// Never `Ok`: a failure parsing with class "ok" is refused (exit.test.ts:34).
    #[serde(deserialize_with = "failure_class")]
    pub class: ExitClass,
    pub exit: i64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub errors: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub warnings: Option<Vec<serde_json::Value>>,
}

/// `ExitClass.exclude(["ok"])`: the class of a failure, which refuses "ok".
fn failure_class<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<ExitClass, D::Error> {
    todo!()
}

/// What a refused reply carries beside its sentence for a client to act on (exit.ts:44-47). Keys in this order;
/// each absent where the TS leaves it out.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct RefusalFields {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub errors: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub warnings: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub left: Option<Vec<serde_json::Value>>,
}

/// The problem lists of a slate write's refusal (exit.ts:63-67).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProblemLists {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub errors: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub warnings: Option<Vec<serde_json::Value>>,
}

/// A rejection read into the halves a refusal was stamped with (exit.ts:84).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefusalParts {
    pub said: String,
    pub fix: Option<String>,
    pub kind: Option<String>,
}

/// `exitClassOf(e)` (exit.ts:37).
pub fn exit_class_of(e: &HostError) -> ExitClass {
    todo!()
}

/// `refusalFields(e)` (exit.ts:44): the problem lists, and `left` only where the kind is processes-left.
pub fn refusal_fields(e: &HostError) -> RefusalFields {
    todo!()
}

/// `escapeC1(json)` (exit.ts:51): DEL and U+0080 to U+009F written as `\u007f` ... `\u009f`, four lower-case hex
/// digits; nothing else changes.
pub fn escape_c1(json: &str) -> String {
    todo!()
}

/// `jsonLine(value, space)` (exit.ts:54): `JSON.stringify(value, null, space)` with `escape_c1` over it. `space`
/// None is one line with no spaces; Some(n) indents by n spaces with `": "` after a key, as JSON.stringify does
/// (an empty array `[]`, an empty object `{}`). Numbers are written by `number_string` (rule 4.25), keys in the
/// order the value holds them (rule 4.26).
pub fn json_line(value: &serde_json::Value, space: Option<usize>) -> String {
    todo!()
}

/// `verbFailure(e)` (exit.ts:56).
pub fn verb_failure(e: &HostError) -> VerbFailure {
    todo!()
}

/// `problemListsOf(e)` (exit.ts:63).
pub fn problem_lists_of(e: &HostError) -> ProblemLists {
    todo!()
}

/// `refusal(happened, fix, kind)` (exit.ts:72): the message is `refusal_line(happened, fix)`, the fix rides beside
/// it, and the kind is stamped only when given.
pub fn refusal(happened: &str, fix: &str, kind: Option<Kind>) -> HostError {
    todo!()
}

/// `refusalSaid(message, fix)` (exit.ts:76): the message with `" " + fix` taken off its end where it ends so, else
/// the message as it is.
pub fn refusal_said(message: &str, fix: Option<&str>) -> String {
    todo!()
}

/// `refusalParts(e)` (exit.ts:84).
pub fn refusal_parts(e: &HostError) -> RefusalParts {
    todo!()
}

/// `markedCut(line, max)` (exit.ts:96): the line as it is up to `max` units, else its first `max` units and
/// ` (cut N characters)` with N the units taken off. Counts UTF-16 units (rule 8.4).
pub fn marked_cut(line: &str, max: usize) -> String {
    todo!()
}

/// `keptSaid(said)` (exit.ts:99): split on "\n", the first 11 lines each through `marked_cut(_, 400)`, then
/// `(cut N lines)` where more than 11 were there.
pub fn kept_said(said: &str) -> String {
    todo!()
}

/// `usageRefusal(happened, fix)` (exit.ts:106).
pub fn usage_refusal(happened: &str, fix: &str) -> HostError {
    todo!()
}

/// `cloudOffRefusal(line)` (exit.ts:109): `<line> needs the cloud, which is off on this computer:` and
/// `start the host with WSP_CLOUD=1 to turn it on.`, kind usage.
pub fn cloud_off_refusal(line: &str) -> HostError {
    todo!()
}

/// `authRefusal(message)` (exit.ts:112): the message as given, kind auth, no fix.
pub fn auth_refusal(message: &str) -> HostError {
    todo!()
}

/// `notFoundRefusal(message)` (exit.ts:116): the message as given, kind not-found, no fix.
pub fn not_found_refusal(message: &str) -> HostError {
    todo!()
}
