// SPDX-License-Identifier: AGPL-3.0-only
//! The one contract an agent reads wsp by: with --json stdout carries JSON alone, every refusal or failure is one
//! line on stderr, and the exit code is the class the error belongs to. The class is read off the kind stamped on
//! the error where it was born, never off its words. Port of packages/protocol/src/exit.ts at a3b870a6e, with the
//! one error type of spec chapter 05 (Proposal 4) standing where the TS stamps fields on an `Error`. See spec/exit.md.

use serde::{Deserialize, Serialize};

use crate::js::{number_string, utf16};
use crate::words::thread::refusal_line;
use wsp_wire::types::env::CLOUD_ENV;

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
        match self {
            ExitClass::Ok => 0,
            ExitClass::Provider => 1,
            ExitClass::Auth => 2,
            ExitClass::Usage => 3,
        }
    }

    /// `EXIT_WORDS[class]`, the words the skill, the help and the instructions quote (exit.ts:19-24), word for word.
    pub fn words(self) -> &'static str {
        match self {
            ExitClass::Ok => "it did what its line says; with --json stdout holds the answer",
            ExitClass::Provider => "the host, the runtime, a provider or the machine refused or failed",
            ExitClass::Auth => "no key, no sign-in, or the host refused the token",
            ExitClass::Usage => "the line was refused before anything ran: a missing argument, an unknown flag or a value nothing takes",
        }
    }

    /// The class's wire word: "ok", "provider", "auth", "usage".
    pub fn as_str(self) -> &'static str {
        match self {
            ExitClass::Ok => "ok",
            ExitClass::Provider => "provider",
            ExitClass::Auth => "auth",
            ExitClass::Usage => "usage",
        }
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
        match self {
            Kind::Usage => "usage",
            Kind::Invalid => "invalid",
            Kind::NotFound => "not-found",
            Kind::ProcessesLeft => PROCESSES_LEFT_KIND,
            Kind::Auth => "auth",
            Kind::Ticket => "ticket",
            Kind::Conflict => "conflict",
            Kind::Concurrency => "concurrency",
            Kind::StartLimit => "startLimit",
            Kind::Plan => "plan",
            Kind::Missing => "missing",
            Kind::SnapshotUnavailable => "snapshotUnavailable",
            Kind::Transient => "transient",
            Kind::Unknown => "unknown",
            Kind::NotFirstLife => "notFirstLife",
            Kind::MachineAlive => "machineAlive",
            Kind::SnapshotFailed => "snapshotFailed",
            Kind::CredentialOnBuilder => "credentialOnBuilder",
            Kind::NoProvider => "noProvider",
            Kind::VaultTooLarge => "vaultTooLarge",
            Kind::Exists => "exists",
            Kind::SshInclude => "sshInclude",
            Kind::HostKey => "host-key",
            Kind::SshBehind => "sshBehind",
            Kind::Unreachable => "unreachable",
            Kind::Refused => "refused",
            Kind::Reauth => "reauth",
            Kind::Other(s) => s.as_str(),
        }
    }

    /// The variant a string names; a string no variant names is `Other` holding it, so `parse(k).as_str() == k`
    /// for every string.
    pub fn parse(s: &str) -> Kind {
        match s {
            "usage" => Kind::Usage,
            "invalid" => Kind::Invalid,
            "not-found" => Kind::NotFound,
            PROCESSES_LEFT_KIND => Kind::ProcessesLeft,
            "auth" => Kind::Auth,
            "ticket" => Kind::Ticket,
            "conflict" => Kind::Conflict,
            "concurrency" => Kind::Concurrency,
            "startLimit" => Kind::StartLimit,
            "plan" => Kind::Plan,
            "missing" => Kind::Missing,
            "snapshotUnavailable" => Kind::SnapshotUnavailable,
            "transient" => Kind::Transient,
            "unknown" => Kind::Unknown,
            "notFirstLife" => Kind::NotFirstLife,
            "machineAlive" => Kind::MachineAlive,
            "snapshotFailed" => Kind::SnapshotFailed,
            "credentialOnBuilder" => Kind::CredentialOnBuilder,
            "noProvider" => Kind::NoProvider,
            "vaultTooLarge" => Kind::VaultTooLarge,
            "exists" => Kind::Exists,
            "sshInclude" => Kind::SshInclude,
            "host-key" => Kind::HostKey,
            "sshBehind" => Kind::SshBehind,
            "unreachable" => Kind::Unreachable,
            "refused" => Kind::Refused,
            "reauth" => Kind::Reauth,
            other => Kind::Other(other.to_string()),
        }
    }

    /// `KIND_CLASS[kind] ?? "provider"` (exit.ts:34,39): usage, invalid, not-found and processes-left are usage,
    /// auth is auth, every other kind (an `Other` included, even `Other("usage")` cannot exist since `parse` maps it)
    /// is the provider's. Never `Ok`.
    pub fn class(&self) -> ExitClass {
        match self {
            Kind::Usage | Kind::Invalid | Kind::NotFound | Kind::ProcessesLeft => ExitClass::Usage,
            Kind::Auth => ExitClass::Auth,
            _ => ExitClass::Provider,
        }
    }
}

impl Serialize for Kind {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Kind {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Kind::parse(&s))
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
    /// The key a computer never dialled answered with, stamped by an add refused under `Kind::HostKey`
    /// (`Object.assign(new Error(..), { kind: PLACE_HOST_KEY_KIND, hostKey })`, places/install.ts:69).
    pub host_key: Option<String>,
    /// The names of the boolean marks the TS stamps on an `Error` as `true` (`wspMachineLacks`,
    /// `wspMachineUnanswered`, words/computer.ts:594-603), read by `marked`. Never on the wire, never in a reply.
    pub marks: Vec<String>,
}

impl HostError {
    /// `new Error(message)`: no fix, no kind, no lists.
    pub fn new(message: impl Into<String>) -> HostError {
        HostError {
            message: message.into(),
            fix: None,
            kind: None,
            errors: None,
            warnings: None,
            left: None,
            host_key: None,
            marks: Vec::new(),
        }
    }

    /// `Object.assign(new Error(message), { kind })`: the message as given, no fix.
    pub fn with_kind(message: impl Into<String>, kind: Kind) -> HostError {
        HostError { kind: Some(kind), ..HostError::new(message) }
    }

    /// An add refused over a key the computer offered and nobody has confirmed: the kind `host-key` and the key, as
    /// places/install.ts:69 stamps them.
    pub fn with_host_key(message: impl Into<String>, host_key: impl Into<String>) -> HostError {
        HostError { host_key: Some(host_key.into()), ..HostError::with_kind(message, Kind::HostKey) }
    }

    /// The key an add offers back (places/manage.ts:321): only an error of kind `host-key` carrying a string key.
    pub fn offered_host_key(&self) -> Option<&str> {
        if self.kind == Some(Kind::HostKey) {
            self.host_key.as_deref()
        } else {
            None
        }
    }

    /// `exitClassOf(e)` (exit.ts:37-40): the kind's class, provider where there is no kind.
    pub fn class(&self) -> ExitClass {
        self.kind.as_ref().map_or(ExitClass::Provider, Kind::class)
    }

    /// `EXIT_CODES[exitClassOf(e)]`: 1, 2 or 3, never 0.
    pub fn exit_code(&self) -> i32 {
        self.class().code()
    }
}

impl std::fmt::Display for HostError {
    /// The message alone, as `e.message`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
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
    let class = ExitClass::deserialize(deserializer)?;
    if class == ExitClass::Ok {
        return Err(<D::Error as serde::de::Error>::custom("a failure is never of class ok"));
    }
    Ok(class)
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
    e.class()
}

/// `refusalFields(e)` (exit.ts:44): the problem lists, and `left` only where the kind is processes-left.
pub fn refusal_fields(e: &HostError) -> RefusalFields {
    let lists = problem_lists_of(e);
    let left = if e.kind == Some(Kind::ProcessesLeft) { e.left.clone() } else { None };
    RefusalFields { errors: lists.errors, warnings: lists.warnings, left }
}

/// `escapeC1(json)` (exit.ts:51): DEL and U+0080 to U+009F written as `\u007f` ... `\u009f`, four lower-case hex
/// digits; nothing else changes.
pub fn escape_c1(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    for c in json.chars() {
        if ('\u{7f}'..='\u{9f}').contains(&c) {
            out.push_str(&format!("\\u{:04x}", c as u32));
        } else {
            out.push(c);
        }
    }
    out
}

/// `jsonLine(value, space)` (exit.ts:54): `JSON.stringify(value, null, space)` with `escape_c1` over it. `space`
/// None is one line with no spaces; Some(n) indents by n spaces with `": "` after a key, as JSON.stringify does
/// (an empty array `[]`, an empty object `{}`). Numbers are written by `number_string` (rule 4.25), keys in the
/// order the value holds them (rule 4.26).
pub fn json_line(value: &serde_json::Value, space: Option<usize>) -> String {
    // JSON.stringify with a gap of 0 writes no white space at all, the same as no gap.
    let space = space.filter(|n| *n > 0);
    let mut out = String::new();
    write_json(value, space, 0, &mut out);
    escape_c1(&out)
}

fn write_json(value: &serde_json::Value, space: Option<usize>, depth: usize, out: &mut String) {
    use serde_json::Value;
    let newline = |depth: usize, out: &mut String| {
        if let Some(n) = space {
            out.push('\n');
            out.push_str(&" ".repeat(n * depth));
        }
    };
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => out.push_str(&number_string(n.as_f64().expect("a JSON number reads as a double"))),
        Value::String(s) => out.push_str(&serde_json::to_string(s).expect("a string writes as JSON")),
        Value::Array(items) if items.is_empty() => out.push_str("[]"),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(depth + 1, out);
                write_json(item, space, depth + 1, out);
            }
            newline(depth, out);
            out.push(']');
        }
        Value::Object(map) if map.is_empty() => out.push_str("{}"),
        Value::Object(map) => {
            out.push('{');
            for (i, (key, item)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(depth + 1, out);
                out.push_str(&serde_json::to_string(key).expect("a key writes as JSON"));
                out.push_str(if space.is_some() { ": " } else { ":" });
                write_json(item, space, depth + 1, out);
            }
            newline(depth, out);
            out.push('}');
        }
    }
}

/// `verbFailure(e)` (exit.ts:56).
pub fn verb_failure(e: &HostError) -> VerbFailure {
    let lists = problem_lists_of(e);
    VerbFailure {
        error: e.message.clone(),
        class: e.class(),
        exit: i64::from(e.exit_code()),
        errors: lists.errors,
        warnings: lists.warnings,
    }
}

/// `problemListsOf(e)` (exit.ts:63).
pub fn problem_lists_of(e: &HostError) -> ProblemLists {
    ProblemLists { errors: e.errors.clone(), warnings: e.warnings.clone() }
}

/// `refusal(happened, fix, kind)` (exit.ts:72): the message is `refusal_line(happened, fix)`, the fix rides beside
/// it, and the kind is stamped only when given.
pub fn refusal(happened: &str, fix: &str, kind: Option<Kind>) -> HostError {
    HostError { fix: Some(fix.to_string()), kind, ..HostError::new(refusal_line(happened, fix)) }
}

/// `refusalSaid(message, fix)` (exit.ts:76): the message with `" " + fix` taken off its end where it ends so, else
/// the message as it is.
pub fn refusal_said(message: &str, fix: Option<&str>) -> String {
    match fix.and_then(|fix| message.strip_suffix(&format!(" {fix}"))) {
        Some(said) => said.to_string(),
        None => message.to_string(),
    }
}

/// `refusalParts(e)` (exit.ts:84).
pub fn refusal_parts(e: &HostError) -> RefusalParts {
    RefusalParts {
        said: refusal_said(&e.message, e.fix.as_deref()),
        fix: e.fix.clone(),
        kind: e.kind.as_ref().map(|k| k.as_str().to_string()),
    }
}

/// `markedCut(line, max)` (exit.ts:96): the line as it is up to `max` units, else its first `max` units and
/// ` (cut N characters)` with N the units taken off. Counts UTF-16 units (rule 8.4).
pub fn marked_cut(line: &str, max: usize) -> String {
    let len = utf16::len(line);
    if len > max {
        format!("{} (cut {} characters)", utf16::cut(line, max), len - max)
    } else {
        line.to_string()
    }
}

/// `keptSaid(said)` (exit.ts:99): split on "\n", the first 11 lines each through `marked_cut(_, 400)`, then
/// `(cut N lines)` where more than 11 were there.
pub fn kept_said(said: &str) -> String {
    let lines: Vec<&str> = said.split('\n').collect();
    let mut kept: Vec<String> = lines.iter().take(SAID_LINES).map(|line| marked_cut(line, SAID_LINE_CHARS)).collect();
    if lines.len() > SAID_LINES {
        kept.push(format!("(cut {} lines)", lines.len() - SAID_LINES));
    }
    kept.join("\n")
}

/// `usageRefusal(happened, fix)` (exit.ts:106).
pub fn usage_refusal(happened: &str, fix: &str) -> HostError {
    refusal(happened, fix, Some(Kind::Usage))
}

/// `cloudOffRefusal(line)` (exit.ts:109): `<line> needs the cloud, which is off on this computer:` and
/// `start the host with WSP_CLOUD=1 to turn it on.`, kind usage.
pub fn cloud_off_refusal(line: &str) -> HostError {
    refusal(
        &format!("{line} needs the cloud, which is off on this computer:"),
        &format!("start the host with {CLOUD_ENV}=1 to turn it on."),
        Some(Kind::Usage),
    )
}

/// `authRefusal(message)` (exit.ts:112): the message as given, kind auth, no fix.
pub fn auth_refusal(message: &str) -> HostError {
    HostError::with_kind(message, Kind::Auth)
}

/// `notFoundRefusal(message)` (exit.ts:116): the message as given, kind not-found, no fix.
pub fn not_found_refusal(message: &str) -> HostError {
    HostError::with_kind(message, Kind::NotFound)
}
