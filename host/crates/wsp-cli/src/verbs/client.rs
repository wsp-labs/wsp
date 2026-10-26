// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/client.ts` at 65f6b89c7: the socket to the host, the verb table's types, and the
//! helpers every verb module shares. Spec: `spec/client-dial.md` (lines 1-452), `spec/client-rows.md` (453-735),
//! `spec/client-table.md` (736-1102). The socket's functions live in the `socket` module and are re-exported here.

mod rows;
mod socket;

pub use rows::*;

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use wsp_store::keys::PlaceKeyPair;
use wsp_wire::types::thread_defaults::AccessChoice;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::session::ThreadView;
use wsp_wire::wire::machine_link::no_such_place_refusal;
use wsp_words::agents_report::without_control_chars;
use wsp_words::exit::{json_line, usage_refusal, verb_failure, HostError};
use wsp_words::place_name::table_name;
use wsp_words::place_word::{names_place, PlaceRef};
use wsp_words::semver::compare_versions;
use wsp_words::words::thread::{say_once, validator_refusal};

pub use socket::{dial_host, host_address, host_again, host_back, stopped_under, until_settled, PushedFrames, SocketClient};

use crate::daemon_fix::release_update_line;
use crate::host_lock::HostStarter;
use crate::hosts::{HostAim, HostPick};
use crate::signin_relay::RelayTerminal;
use crate::watch::WatchSignals;

/// A future a verb's run or a tool's call hands back; boxed so the verb table can hold plain fn pointers.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// ---------------------------------------------------------------------------------------------------------------
// The socket (client.ts:95-452). Spec: spec/client-dial.md.
// ---------------------------------------------------------------------------------------------------------------

/// `Frame` (client.ts:95): one JSON object off the socket, keys in the order the host wrote them.
pub type Frame = serde_json::Map<String, Value>;

/// What `onFrame` is handed: called with every frame that is not a reply, in arrival order.
pub type FrameSink = Arc<dyn Fn(&Frame) + Send + Sync>;

/// The handle `onFrame` returns (client.ts:102): calling `off` removes that one sink, and only that one.
pub struct FrameOff(pub Box<dyn FnOnce() + Send>);

impl FrameOff {
    pub fn off(self) {
        (self.0)()
    }
}

/// The device token a host on the account answered this computer's key with (client.ts:109).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Paired {
    pub device_id: String,
    pub device_token: String,
}

/// `HostClient` (client.ts:97-116): one socket to the host, request and reply by id.
#[async_trait]
pub trait HostClient: Send + Sync {
    /// Sends `{...params, id, op}` (params' keys first, then `id`, then `op`, an `id` or `op` among the params
    /// overwritten in place) and answers the reply frame whole when its `ok` is `true`. A reply with `ok !== true`
    /// fails with its `error` string (else `<op> failed`), its string `kind`, and its array `errors` and
    /// `warnings`; never `left` (client.ts:259-268). A socket that closes while the request waits fails it with
    /// `close_words()` and no kind (client.ts:254-257).
    async fn request(&self, op: &str, params: Frame) -> Result<Frame, HostError>;
    /// `events.subscribe` at most once per socket: a second call awaits the first one's answer and sends nothing
    /// (client.ts:378).
    async fn events(&self) -> Result<(), HostError>;
    /// Every frame that is not a reply to a waiting request (client.ts:251).
    fn on_frame(&self, sink: FrameSink) -> FrameOff;
    /// Resolves once the socket is gone, however it went, with the close code where one came (client.ts:104).
    async fn closed(&self) -> Option<u16>;
    /// The non-waiting read of `closed`: None while the socket is open, Some(code) once it is gone. What
    /// `stoppedUnder` reads (`Promise.race([client.closed, Promise.resolve()])`, client.ts:394).
    fn gone(&self) -> Option<Option<u16>>;
    /// `HOST_STOPPING_LINE` when the close code was 4001, else `HOST_CLOSED_LINE` (client.ts:253).
    fn close_words(&self) -> String;
    /// The device token a dial that proved this computer's key was answered with (client.ts:109, 385).
    fn paired(&self) -> Option<Paired>;
    /// A graceful close: no close code of its own, so the host sees 1005 (rule 10.19).
    fn close(&self);
    /// Drops the socket without waiting for the close handshake (client.ts:115).
    fn terminate(&self);
}

/// `UNAUTHORIZED_CLOSE` (client.ts:119).
pub const UNAUTHORIZED_CLOSE: u16 = 4401;
/// `CLOSE_GRACE_MS` (client.ts:121).
pub const CLOSE_GRACE_MS: u64 = 500;

/// `admit` of `DialOpts` (client.ts:133): this computer's device key, proved inside the seal.
#[derive(Clone, Debug)]
pub struct Admit {
    /// What the host's listing calls this computer: `os.hostname()`.
    pub name: String,
    /// The fingerprint the record pinned for that host.
    pub host_key: String,
    pub key: PlaceKeyPair,
}

/// `DialOpts` (client.ts:125-143).
#[derive(Clone, Default)]
pub struct DialOpts {
    pub pick: HostPick,
    /// One deadline over the open and the first answer; `dial_window_ms(aim)` when None.
    pub deadline_ms: Option<u64>,
    /// Resolved already by the caller; `aimed_host(state, pick)` when None.
    pub aim: Option<HostAim>,
    pub admit: Option<Admit>,
    pub any_release: bool,
    pub start: Option<Arc<dyn HostStarter>>,
    /// Where the starter's one line goes; stderr when None.
    pub say: Option<Arc<dyn Fn(&str) + Send + Sync>>,
}

/// `noHostServingLine` (client.ts:149).
pub fn no_host_serving_line(state_path: &str) -> String {
    format!("no wsp host is serving {state_path}; start one with wsp up --state {state_path}")
}

/// `hostTokenMissingLine` (client.ts:152).
pub fn host_token_missing_line(path: &str) -> String {
    format!("the host's token file is missing: {path}")
}

/// Where a dial goes and what it presents (`hostAddress`, client.ts:158-167).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAddress {
    pub url: String,
    pub token: String,
}

/// `HOST_RESTARTING_LINE` (client.ts:398).
pub const HOST_RESTARTING_LINE: &str = "the host is restarting; waiting for it to come back";

/// What `host_again` is handed: one dial with what is left of the window as its deadline.
pub type DialWithin<'a> = &'a (dyn Fn(u64) -> BoxFuture<'a, Result<Arc<dyn HostClient>, HostError>> + Send + Sync);

// ---------------------------------------------------------------------------------------------------------------
// The verb table's types and the shared helpers (client.ts:736-1102). Spec: spec/client-table.md.
// ---------------------------------------------------------------------------------------------------------------

/// One parsed flag's value (`Flags`, client.ts:736).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FlagValue {
    Str(String),
    Bool(bool),
    List(Vec<String>),
}

/// `Flags` (client.ts:736): name to value, in the order the parser met them.
pub type Flags = IndexMap<String, FlagValue>;

/// `again` of `VerbDeps.client` (client.ts:752): the dial after the host let the last socket go as it stopped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Again {
    pub within_ms: u64,
}

/// What opens the socket for a verb: `VerbDeps.client` (client.ts:752).
#[async_trait]
pub trait Dialer: Send + Sync {
    async fn client(&self, again: Option<Again>) -> Result<Arc<dyn HostClient>, HostError>;
}

/// `dial?: typeof dialHost` (client.ts:764): the dial a caller hands in to count dials.
#[async_trait]
pub trait DialFn: Send + Sync {
    async fn dial(&self, state_path: &str, opts: DialOpts) -> Result<Arc<dyn HostClient>, HostError>;
}

/// `open?(url)` (client.ts:773): what opens a page here.
#[async_trait]
pub trait Opener: Send + Sync {
    async fn open(&self, url: &str) -> bool;
}

/// `VerbDeps` (client.ts:739-774).
#[derive(Clone)]
pub struct VerbDeps {
    pub state_path: String,
    pub cwd: Option<String>,
    pub env: IndexMap<String, String>,
    pub dialer: Arc<dyn Dialer>,
    pub host_wait_ms: Option<u64>,
    pub start: Option<Arc<dyn HostStarter>>,
    pub signals: Option<Arc<dyn WatchSignals>>,
    pub dial: Option<Arc<dyn DialFn>>,
    pub elsewhere: bool,
    pub terminal: Option<Arc<tokio::sync::Mutex<RelayTerminal>>>,
    pub open: Option<Arc<dyn Opener>>,
}

impl VerbDeps {
    /// `deps.client(again)`.
    pub async fn client(&self, again: Option<Again>) -> Result<Arc<dyn HostClient>, HostError> {
        self.dialer.client(again).await
    }
}

/// `SshAsked` (client.ts:778-782).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SshAsked {
    pub name: Option<String>,
    pub port: Option<u16>,
    pub key_path: Option<String>,
}

/// `VerbContext` (client.ts:784-795).
pub struct VerbContext {
    pub deps: VerbDeps,
    pub args: Vec<String>,
    pub flags: Flags,
    pub io: Arc<dyn CliIo>,
    pub out: Out,
    pub aim: HostAim,
    pub usage: String,
}

impl std::ops::Deref for VerbContext {
    type Target = VerbDeps;
    fn deref(&self) -> &VerbDeps {
        &self.deps
    }
}

/// `usageIs(ctx)` (client.ts:798): `usage: <ctx.usage>`.
pub fn usage_is(ctx: &VerbContext) -> String {
    format!("usage: {}", ctx.usage)
}

/// `CallToolResult` of the MCP SDK as the TS tools build it: keys `content`, `structuredContent`, `isError`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallToolResult {
    pub content: Vec<TextContent>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub structured_content: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub is_error: Option<bool>,
}

/// One `{type: "text", text}` of a tool's content.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextContent {
    #[serde(rename = "type")]
    pub kind: String,
    pub text: String,
}

/// What a zod input field is, as far as the command line and the parity rows read it.
#[derive(Clone, Debug, PartialEq)]
pub enum InputKind {
    String,
    Boolean,
    Number,
    /// `z.number().int()`, with its `.min()` where it has one (`.positive()` is min 1).
    Integer {
        min: Option<i64>,
    },
    /// `z.array(z.string())`, with its `.min()` where it has one.
    StringList {
        min: Option<usize>,
    },
    /// `z.enum([...])` or a zod enum schema's options, in order.
    Enum(Vec<&'static str>),
    /// `z.literal(v)`.
    Literal(Value),
    /// Any other shape (an object, an array of objects): read as JSON as given.
    Json,
}

/// One field of a tool's input shape, in the shape's key order.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolInput {
    pub name: &'static str,
    pub kind: InputKind,
    pub optional: bool,
    /// The zod `.describe()` text, cloud spans not yet cut.
    pub describe: Option<String>,
}

/// A tool's call: the arguments as the server parsed them, and the deps.
pub type ToolCallFn = for<'a> fn(Frame, &'a VerbDeps) -> BoxFuture<'a, Result<CallToolResult, HostError>>;

/// `Tool` (client.ts:802-810).
#[derive(Clone)]
pub struct Tool {
    pub description: String,
    pub input: Vec<ToolInput>,
    /// The output shape's field names, in order.
    pub output: Vec<&'static str>,
    /// The output fields printed as frames under --json (client.ts:808).
    pub stream: Vec<&'static str>,
    pub call: ToolCallFn,
}

/// `Page` (client.ts:827).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Page {
    Front,
    Agent,
    Host,
    Dev,
    App,
}

/// A flag's parser type.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlagKind {
    String,
    Boolean,
}

/// `FlagRow` (client.ts:833): node's `parseArgs` option row plus `valueWith`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlagRow {
    pub kind: FlagKind,
    pub multiple: bool,
    pub short: Option<char>,
    pub value_with: Option<&'static str>,
}

impl FlagRow {
    pub const STRING: FlagRow = FlagRow { kind: FlagKind::String, multiple: false, short: None, value_with: None };
    pub const BOOLEAN: FlagRow = FlagRow { kind: FlagKind::Boolean, multiple: false, short: None, value_with: None };
    pub const STRINGS: FlagRow = FlagRow { kind: FlagKind::String, multiple: true, short: None, value_with: None };
}

/// `FlagTable` (client.ts:834): rows in declaration order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FlagTable(pub Vec<(&'static str, FlagRow)>);

impl FlagTable {
    pub fn get(&self, name: &str) -> Option<&FlagRow> {
        self.0.iter().find(|(n, _)| *n == name).map(|(_, row)| row)
    }

    pub fn has(&self, name: &str) -> bool {
        self.get(name).is_some()
    }
}

/// `optionalValues(argv, table)` (client.ts:838-851).
pub fn optional_values(argv: &[String], table: &FlagTable) -> Vec<String> {
    let cut = argv.iter().position(|w| w == "--").unwrap_or(argv.len());
    let words = &argv[..cut];
    let has = |name: &str| words.iter().any(|w| w == &format!("--{name}") || w.starts_with(&format!("--{name}=")));
    let mut bare = std::collections::HashSet::new();
    for (name, row) in &table.0 {
        let Some(value_with) = row.value_with else { continue };
        let alone = !has(value_with);
        for (at, word) in words.iter().enumerate() {
            if word == &format!("--{name}") && (alone || words.get(at + 1).is_none_or(|next| next.starts_with('-'))) {
                bare.insert(at);
            }
        }
    }
    argv.iter().enumerate().map(|(at, word)| if bare.contains(&at) { format!("{word}=") } else { word.clone() }).collect()
}

/// A verb's run: the parsed line to an exit code.
pub type RunFn = for<'a> fn(&'a VerbContext) -> BoxFuture<'a, Result<i32, HostError>>;

/// `CliVerb` (client.ts:853-872).
#[derive(Clone)]
pub struct CliVerb {
    pub name: &'static str,
    pub usage: String,
    pub about: String,
    pub page: Page,
    pub options: FlagTable,
    pub run: RunFn,
    pub tool: Tool,
    pub reads_here: Option<&'static str>,
    pub starts_no_host: Option<&'static str>,
    pub any_release: Option<&'static str>,
    pub cloud: bool,
    pub cloud_flags: Vec<&'static str>,
}

/// `ToolOnlyVerb` (client.ts:875-880).
#[derive(Clone)]
pub struct ToolOnlyVerb {
    pub name: &'static str,
    pub tool: Tool,
    pub tool_only: &'static str,
    pub reads_here: Option<&'static str>,
}

/// `CliOnlyVerb` (client.ts:884-889): a `CliVerb` without its tool.
#[derive(Clone)]
pub struct CliOnlyVerb {
    pub name: &'static str,
    pub usage: String,
    pub about: String,
    pub page: Page,
    pub options: FlagTable,
    pub run: RunFn,
    pub reads_here: Option<&'static str>,
    pub starts_no_host: Option<&'static str>,
    pub any_release: Option<&'static str>,
    pub cloud: bool,
    pub cloud_flags: Vec<&'static str>,
    pub cli_only: &'static str,
    pub host_side: Option<&'static str>,
}

/// `CommandToolVerb` (client.ts:893-897).
#[derive(Clone)]
pub struct CommandToolVerb {
    pub name: &'static str,
    pub tool: Tool,
}

/// `Verb` (client.ts:899).
#[derive(Clone)]
pub enum Verb {
    Cli(CliVerb),
    ToolOnly(ToolOnlyVerb),
    CliOnly(CliOnlyVerb),
    CommandTool(CommandToolVerb),
}

/// A verb the command line answers: what `findVerb`, the pages and `runVerb` take.
#[derive(Clone, Copy)]
pub enum LineVerb<'a> {
    Cli(&'a CliVerb),
    CliOnly(&'a CliOnlyVerb),
}

impl Verb {
    pub fn name(&self) -> &'static str {
        match self {
            Verb::Cli(verb) => verb.name,
            Verb::ToolOnly(verb) => verb.name,
            Verb::CliOnly(verb) => verb.name,
            Verb::CommandTool(verb) => verb.name,
        }
    }

    /// The line form of a `Cli` or `CliOnly` entry; None for the other two.
    pub fn line(&self) -> Option<LineVerb<'_>> {
        match self {
            Verb::Cli(verb) => Some(LineVerb::Cli(verb)),
            Verb::CliOnly(verb) => Some(LineVerb::CliOnly(verb)),
            Verb::ToolOnly(_) | Verb::CommandTool(_) => None,
        }
    }

    /// `cloud === true` on a `Cli` or `CliOnly` entry.
    pub fn is_cloud(&self) -> bool {
        match self {
            Verb::Cli(verb) => verb.cloud,
            Verb::CliOnly(verb) => verb.cloud,
            Verb::ToolOnly(_) | Verb::CommandTool(_) => false,
        }
    }
}

impl<'a> LineVerb<'a> {
    pub fn name(&self) -> &'static str {
        match *self {
            LineVerb::Cli(verb) => verb.name,
            LineVerb::CliOnly(verb) => verb.name,
        }
    }
    pub fn usage(&self) -> &'a str {
        match *self {
            LineVerb::Cli(verb) => verb.usage.as_str(),
            LineVerb::CliOnly(verb) => verb.usage.as_str(),
        }
    }
    pub fn about(&self) -> &'a str {
        match *self {
            LineVerb::Cli(verb) => verb.about.as_str(),
            LineVerb::CliOnly(verb) => verb.about.as_str(),
        }
    }
    pub fn page(&self) -> Page {
        match *self {
            LineVerb::Cli(verb) => verb.page,
            LineVerb::CliOnly(verb) => verb.page,
        }
    }
    pub fn options(&self) -> &'a FlagTable {
        match *self {
            LineVerb::Cli(verb) => &verb.options,
            LineVerb::CliOnly(verb) => &verb.options,
        }
    }
    pub fn run(&self) -> RunFn {
        match *self {
            LineVerb::Cli(verb) => verb.run,
            LineVerb::CliOnly(verb) => verb.run,
        }
    }
    pub fn starts_no_host(&self) -> Option<&'static str> {
        match *self {
            LineVerb::Cli(verb) => verb.starts_no_host,
            LineVerb::CliOnly(verb) => verb.starts_no_host,
        }
    }
    pub fn any_release(&self) -> Option<&'static str> {
        match *self {
            LineVerb::Cli(verb) => verb.any_release,
            LineVerb::CliOnly(verb) => verb.any_release,
        }
    }
    /// Only a `CliOnly` entry carries one.
    pub fn host_side(&self) -> Option<&'static str> {
        match *self {
            LineVerb::Cli(_) => None,
            LineVerb::CliOnly(verb) => verb.host_side,
        }
    }
}

/// `readsHere(verb)` (client.ts:904-906).
pub fn reads_here(verb: &Verb) -> Option<&'static str> {
    match verb {
        Verb::Cli(verb) => verb.reads_here,
        Verb::ToolOnly(verb) => verb.reads_here,
        Verb::CliOnly(verb) => verb.reads_here,
        Verb::CommandTool(_) => None,
    }
}

/// `hasTool(verb)` (client.ts:909-911): the tool of a `Cli`, `ToolOnly` or `CommandTool` entry.
pub fn has_tool(verb: &Verb) -> Option<&Tool> {
    match verb {
        Verb::Cli(verb) => Some(&verb.tool),
        Verb::ToolOnly(verb) => Some(&verb.tool),
        Verb::CommandTool(verb) => Some(&verb.tool),
        Verb::CliOnly(_) => None,
    }
}

/// `toolName(words)` (client.ts:914-916): every space to `_`.
pub fn tool_name(words: &str) -> String {
    words.replace(' ', "_")
}

/// `COMMON` (client.ts:919-924): host, state (strings), json, help (booleans, help also `-h`), in that order.
pub fn common() -> FlagTable {
    FlagTable(vec![
        ("host", FlagRow::STRING),
        ("state", FlagRow::STRING),
        ("json", FlagRow::BOOLEAN),
        ("help", FlagRow { kind: FlagKind::Boolean, multiple: false, short: Some('h'), value_with: None }),
    ])
}

/// What `takeCommon` splits a line into (client.ts:933).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakenCommon {
    pub common: Vec<String>,
    pub rest: Vec<String>,
}

/// `takeCommon(argv)` (client.ts:933-959).
pub fn take_common(argv: &[String]) -> TakenCommon {
    let table = common();
    let mut taken = TakenCommon { common: Vec::new(), rest: Vec::new() };
    let mut at = 0;
    while at < argv.len() {
        let word = &argv[at];
        if word == "--" {
            taken.rest.extend_from_slice(&argv[at..]);
            break;
        }
        let name = match word.strip_prefix("--") {
            Some(long) => Some(long.split('=').next().unwrap_or_default().to_owned()),
            None => word.strip_prefix('-').and_then(|letters| {
                table
                    .0
                    .iter()
                    .find(|(_, row)| row.short.map(|c| c.to_string()).as_deref() == Some(letters))
                    .map(|(name, _)| (*name).to_owned())
            }),
        };
        let Some(row) = name.as_deref().and_then(|name| table.get(name)).cloned() else {
            taken.rest.push(word.clone());
            at += 1;
            continue;
        };
        let takes_value = row.kind == FlagKind::String && !word.contains('=');
        let value = if takes_value { argv.get(at + 1).cloned() } else { None };
        // A flag needing a value and given none by the end of the line stays where it was typed.
        if takes_value && value.is_none() {
            taken.rest.push(word.clone());
            at += 1;
            continue;
        }
        taken.common.push(word.clone());
        if let Some(value) = value {
            taken.common.push(value);
            at += 1;
        }
        at += 1;
    }
    taken
}

/// `PICK_FLAGS` (client.ts:962).
pub const PICK_FLAGS: [&str; 3] = ["model", "effort", "access"];
/// `SEND_FLAGS` (client.ts:966).
pub const SEND_FLAGS: [&str; 2] = ["model", "effort"];

/// `PICK_OPTIONS` (client.ts:968): each pick flag a string.
pub fn pick_options() -> FlagTable {
    FlagTable(PICK_FLAGS.iter().map(|name| (*name, FlagRow::STRING)).collect())
}

/// `SEND_OPTIONS` (client.ts:969).
pub fn send_options() -> FlagTable {
    FlagTable(SEND_FLAGS.iter().map(|name| (*name, FlagRow::STRING)).collect())
}

/// `flag(flags, name)` (client.ts:971): the value only where it is a string.
pub fn flag<'a>(flags: &'a Flags, name: &str) -> Option<&'a str> {
    match flags.get(name) {
        Some(FlagValue::Str(text)) => Some(text.as_str()),
        _ => None,
    }
}

/// `flagList(flags, name)` (client.ts:972): the value only where it is a list, else empty.
pub fn flag_list(flags: &Flags, name: &str) -> Vec<String> {
    match flags.get(name) {
        Some(FlagValue::List(items)) => items.clone(),
        _ => Vec::new(),
    }
}

/// `flags[name] === true`.
pub fn flag_set(flags: &Flags, name: &str) -> bool {
    matches!(flags.get(name), Some(FlagValue::Bool(true)))
}

/// `PLACES_FIX` (client.ts:974).
pub const PLACES_FIX: &str = "Run wsp computers.";

/// `placeNamed(client, word)` (client.ts:978-983): one `places.list`, the first row `names_place` takes.
pub async fn place_named(client: &dyn HostClient, word: &str) -> Result<PlaceView, HostError> {
    let places = places_of(&client.request("places.list", Frame::new()).await?, "places.list")?;
    if let Some(found) = places.iter().find(|p| names_place(&place_ref(p), word)) {
        return Ok(found.clone());
    }
    let names: Vec<String> = places.iter().map(|p| table_name(&place_ref(p))).collect();
    Err(usage_refusal(&no_such_place_refusal(word, &names), PLACES_FIX))
}

/// `absolutePath(named, path)` (client.ts:987-990): refused unless it opens with `/`.
pub fn absolute_path(named: &str, path: &str) -> Result<String, HostError> {
    if !path.starts_with('/') {
        let said = format!("{named}, absolute, and got {}.", json_string(path));
        return Err(usage_refusal(&said, "Give a path that opens with /, since whoever reads it works in a folder this line cannot see."));
    }
    Ok(path.to_owned())
}

/// `absoluteFolder(cwd)` (client.ts:994-996).
pub fn absolute_folder(cwd: Option<&str>) -> Result<Option<String>, HostError> {
    cwd.map(|cwd| absolute_path("--cwd is a path on the machine", cwd)).transpose()
}

/// `accessWordOf(given)` (client.ts:1000-1004): one of ask, auto-edit, full, plan.
pub fn access_word_of(given: &str) -> Result<AccessChoice, HostError> {
    match given {
        "ask" => Ok(AccessChoice::Ask),
        "auto-edit" => Ok(AccessChoice::AutoEdit),
        "full" => Ok(AccessChoice::Full),
        "plan" => Ok(AccessChoice::Plan),
        _ => Err(usage_refusal(
            &format!("--access takes ask, auto-edit, full or plan, and got {given}."),
            "Name one of those; which of its own modes each one is, is the agent's row's to say.",
        )),
    }
}

/// `under(path, folder)` (client.ts:1007).
pub fn under(path: &str, folder: &str) -> bool {
    let inside = if folder.ends_with('/') { folder.to_owned() } else { format!("{folder}/") };
    path == folder || path.starts_with(&inside)
}

/// `RELEASE_WORDS` (client.ts:1011-1020).
pub mod release_words {
    use wsp_wire::types::release::UP_RESTART_LINE;

    pub fn said(mine: &str, host: &str, theirs: &str) -> String {
        format!("this line runs wsp {mine} and {host} runs wsp {theirs}:")
    }
    pub fn host_here(state: &str) -> String {
        format!("the host serving {state}")
    }
    pub fn host_at(r#where: &str) -> String {
        format!("the host at {where}")
    }
    pub const RESTART: &str = "restart that host with wsp restart.";
    pub fn reopen_app(mine: &str) -> String {
        format!("quit and reopen the app that holds it once the app is on wsp {mine}.")
    }
    /// `UP_RESTART_LINE`.
    pub fn restart_up() -> &'static str {
        UP_RESTART_LINE
    }
    pub fn update_there(mine: &str) -> String {
        format!("update wsp to {mine} on that computer and restart its host there.")
    }
    pub fn update_here(line: &str, theirs: &str) -> String {
        format!("{line} on this computer brings this line to {theirs}.")
    }
}

/// `ReleaseHost` (client.ts:1024).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReleaseHost {
    State(String),
    Where { r#where: String, here: bool },
}

/// `releaseHereFix(mine, road)` (client.ts:1029-1032): "app" reopens the app, "up" is `UP_RESTART_LINE`, anything
/// else (None included) restarts.
pub fn release_here_fix(mine: &str, road: Option<&str>) -> String {
    match road {
        Some("app") => release_words::reopen_app(mine),
        Some("up") => release_words::restart_up().to_owned(),
        _ => release_words::RESTART.to_owned(),
    }
}

/// `releaseRefusal(mine, theirs, host, road)` (client.ts:1035-1041). `argv` is this process's, read by
/// `release_update_line` when this line is the older end.
pub fn release_refusal(mine: &str, theirs: &str, host: &ReleaseHost, road: Option<&str>, argv: &[String]) -> HostError {
    let host_words = match host {
        ReleaseHost::State(state) => release_words::host_here(state),
        ReleaseHost::Where { r#where, .. } => release_words::host_at(r#where),
    };
    let said = release_words::said(mine, &host_words, theirs);
    if compare_versions(mine, theirs) == std::cmp::Ordering::Less {
        let fix = release_words::update_here(&release_update_line(argv, None, theirs), theirs);
        return usage_refusal(&said, &fix);
    }
    let fix = match host {
        ReleaseHost::State(_) | ReleaseHost::Where { here: true, .. } => release_here_fix(mine, road),
        ReleaseHost::Where { here: false, .. } => release_words::update_there(mine),
    };
    usage_refusal(&said, &fix)
}

/// `otherVersion(op)` (client.ts:1044).
pub fn other_version(op: &str) -> String {
    format!("the host answered {op} in a shape this wsp does not read; it runs another version of wsp, restart it with wsp restart")
}

/// `threadIdOf(t)` (client.ts:1047): `threadId` where set, else `id`.
pub fn thread_id_of(thread: &ThreadView) -> String {
    thread.thread_id.clone().unwrap_or_else(|| thread.id.clone())
}

/// `THREAD_PREFIX_WORD` (client.ts:1053).
pub const THREAD_PREFIX_WORD: &str = "wsp thread read, wsp send and wsp stop take its first characters";

/// The line a caller's opening names a thread with (`opened` of `openedThreadLine`, client.ts:1056).
pub type OpenedFn<'a> = &'a (dyn Fn(&str, Option<&str>) -> String + Send + Sync);

/// `openedThreadLine(threadId, opened, folder)` (client.ts:1056).
pub fn opened_thread_line(thread_id: &str, opened: Option<OpenedFn<'_>>, folder: Option<&str>) -> String {
    match opened {
        None => format!("thread {thread_id}"),
        Some(opened) => opened(thread_id, folder),
    }
}

/// `openedThreadSaid(threadId, opened, folder)` (client.ts:1060): the line, two spaces, `THREAD_PREFIX_WORD`.
pub fn opened_thread_said(thread_id: &str, opened: Option<OpenedFn<'_>>, folder: Option<&str>) -> String {
    format!("{}  {THREAD_PREFIX_WORD}", opened_thread_line(thread_id, opened, folder))
}

/// `printable(s)` (client.ts:1063): drops U+0000-U+0008, U+000B-U+001F and U+007F-U+009F; keeps tab and newline.
pub fn printable(s: &str) -> String {
    s.chars().filter(|c| !matches!(*c as u32, 0x00..=0x08 | 0x0b..=0x1f | 0x7f..=0x9f)).collect()
}

/// `cell(s)` (client.ts:1066): newline and tab to a space, then `without_control_chars`.
pub fn cell(s: &str) -> String {
    without_control_chars(&s.replace(['\n', '\t'], " "))
}

/// `pick(o)` (client.ts:1069-1071): the given fields in order, the absent ones dropped. A `Some(Value::Null)` is
/// kept: only `undefined` is left out.
pub fn pick(fields: Vec<(&str, Option<Value>)>) -> Frame {
    fields.into_iter().filter_map(|(key, value)| value.map(|value| (key.to_owned(), value))).collect()
}

/// `failed(io, json, e, prefix)` (client.ts:1076-1080): one line on stderr, `json_line(verb_failure(e))` under
/// --json, else `say_once(prefix, message)`; answers the class's exit code.
pub fn failed(io: &dyn CliIo, json: bool, e: &HostError, prefix: &str) -> i32 {
    let failure = verb_failure(e);
    let line = if json {
        json_line(&serde_json::to_value(&failure).expect("a failure writes as JSON"), None)
    } else {
        say_once(prefix, &failure.error)
    };
    io.error(&line);
    failure.exit as i32
}

/// `toolFailure(e, usage)` (client.ts:1085-1088).
pub fn tool_failure(e: &HostError, usage: Option<&str>) -> CallToolResult {
    let refused = usage.and_then(|usage| host_schema_refusal(e, usage));
    let failure = verb_failure(refused.as_ref().unwrap_or(e));
    CallToolResult {
        content: vec![TextContent { kind: "text".to_owned(), text: failure.error.clone() }],
        structured_content: Some(serde_json::to_value(&failure).expect("a failure writes as JSON")),
        is_error: Some(true),
    }
}

/// `jsonAsked(argv)` (client.ts:1091-1094): `--json` among the words before the first `--`.
pub fn json_asked(argv: &[String]) -> bool {
    let cut = argv.iter().position(|w| w == "--").unwrap_or(argv.len());
    argv[..cut].iter().any(|w| w == "--json")
}

/// `hostSchemaRefusal(e, usage)` (client.ts:1099-1102).
pub fn host_schema_refusal(e: &HostError, usage: &str) -> Option<HostError> {
    let said = validator_refusal(&e.message)?;
    Some(usage_refusal(&said, &format!("usage: {usage}")))
}

// ---------------------------------------------------------------------------------------------------------------
// Helpers private to this file.
// ---------------------------------------------------------------------------------------------------------------

/// `cell.padEnd(width)` where `width` is None for the JS `undefined` a column past the widths gives: no padding.
fn json_string(text: &str) -> String {
    serde_json::to_string(text).expect("a string writes as JSON")
}

/// A usage word from the table of `USAGE_WORDS` by its key.
fn is_provider(p: &PlaceView) -> bool {
    matches!(p.kind, PlaceKind::Provider)
}

fn place_ref(p: &PlaceView) -> PlaceRef {
    PlaceRef { id: p.id.clone(), name: p.name.clone(), kind: Some(kind_word(p).to_owned()) }
}

/// The THREADS and MACHINES cells: what the row's cap counts against the cap, under the column named for what it counts.
/// The SPEND cell: what the row spent today against its spend per day, empty where its kind has no spend limit or
/// no spend was read, since a figure is never guessed.
/// A reply's field read into the type a verb takes; a field the reply lacks or shapes otherwise is the other-version
/// refusal the host's answer earns.
fn reply_field<T: DeserializeOwned>(frame: &Frame, op: &str, key: &str) -> Result<T, HostError> {
    frame.get(key).cloned().and_then(|value| serde_json::from_value(value).ok()).ok_or_else(|| HostError::new(other_version(op)))
}

/// A `places.list` row without a kind reads as a computer, as the TS's table prints it.
pub fn places_of(frame: &Frame, op: &str) -> Result<Vec<PlaceView>, HostError> {
    let rows: Vec<Value> = reply_field(frame, op, "places")?;
    rows.into_iter()
        .map(|mut row| {
            if let Some(object) = row.as_object_mut() {
                object.entry("kind").or_insert_with(|| Value::from("computer"));
            }
            serde_json::from_value(row).map_err(|_| HostError::new(other_version(op)))
        })
        .collect()
}

/// A reply's field where the host may leave it out: None where absent or null.
fn optional_field<T: DeserializeOwned>(frame: &Frame, op: &str, key: &str) -> Result<Option<T>, HostError> {
    match frame.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => serde_json::from_value(value.clone()).map(Some).map_err(|_| HostError::new(other_version(op))),
    }
}

fn frame_of_value(value: Value) -> Frame {
    match value {
        Value::Object(frame) => frame,
        _ => Frame::new(),
    }
}

fn kind_word(p: &PlaceView) -> &'static str {
    if is_provider(p) {
        "provider"
    } else {
        "computer"
    }
}
