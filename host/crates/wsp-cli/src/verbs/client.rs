// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/host/src/verbs/client.ts` at 65f6b89c7: the socket to the host, the verb table's types, and the
//! helpers every verb module shares. Spec: `spec/client-dial.md` (lines 1-452), `spec/client-rows.md` (453-735),
//! `spec/client-table.md` (736-1102).

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use wsp_store::keys::PlaceKeyPair;
use wsp_wire::types::recipe_file::RecipeView;
use wsp_wire::types::usage::{AccountRow, UsageRange, UsageSplit, UsedAnswer};
use wsp_wire::views::place::{PendingComputer, PlaceSetAlso};
use wsp_wire::views::place_more::{PlaceSettingWord, PlaceSettingsAsk, PlaceSpend, PlaceView};
use wsp_wire::views::session::ThreadView;
use wsp_words::exit::HostError;

use crate::host_lock::HostStarter;
use crate::hosts::{HostAim, HostPick};
use crate::signin_relay::RelayTerminal;
use crate::watch::{Redraw, WatchSignals};

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
    let _ = state_path;
    todo!("client.ts:149")
}

/// `hostTokenMissingLine` (client.ts:152).
pub fn host_token_missing_line(path: &str) -> String {
    let _ = path;
    todo!("client.ts:152")
}

/// Where a dial goes and what it presents (`hostAddress`, client.ts:158-167).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostAddress {
    pub url: String,
    pub token: String,
}

/// `hostAddress(statePath, pick)` (client.ts:158-167). A url aim: its ws url and its token or "". An alias: the
/// record's url and device token. Here: the lock's dial address and the token beside the state; no lock is a plain
/// error `no_host_serving_line` (provider), no token an auth refusal `host_token_missing_line(token path)`.
pub fn host_address(state_path: &str, pick: &HostPick, aim: Option<&HostAim>) -> Result<HostAddress, HostError> {
    let _ = (state_path, pick, aim);
    todo!("client.ts:158-167")
}

/// `dialHost(statePath, opts)` (client.ts:172-198): one socket, with the one re-admission a host on the account may
/// need. An alias record with an empty device token and a `host_key` dials admitting this computer's key at once;
/// a token the host refused (kind auth, on the token's frame) dials once more admitting, unless this dial already
/// admitted. A dial that proved the key rewrites `hosts/<alias>.json` with the answered device id and token, the
/// rest of the record as it was.
pub async fn dial_host(state_path: &str, opts: DialOpts) -> Result<Arc<dyn HostClient>, HostError> {
    let _ = (state_path, opts);
    todo!("client.ts:172-198")
}

/// The concrete socket `dial_host` answers with (client.ts:207-389). Fields are the implementer's.
pub struct SocketClient {
    _private: (),
}

/// `stoppedUnder(client)` (client.ts:393-395): whether the socket is gone with close code 4001. Never waits.
pub fn stopped_under(client: &dyn HostClient) -> bool {
    let _ = client;
    todo!("client.ts:393-395")
}

/// `HOST_RESTARTING_LINE` (client.ts:398).
pub const HOST_RESTARTING_LINE: &str = "the host is restarting; waiting for it to come back";

/// What `host_again` is handed: one dial with what is left of the window as its deadline.
pub type DialWithin<'a> = &'a (dyn Fn(u64) -> BoxFuture<'a, Result<Arc<dyn HostClient>, HostError>> + Send + Sync);

/// `hostAgain(dial, windowMs)` (client.ts:404-414): dial with `max(1, left)` as each try's deadline; on failure,
/// if the window has passed, the last failure stands; else sleep `min(POLL_MS, max(0, left))` and try again.
pub async fn host_again(dial: DialWithin<'_>, window_ms: u64) -> Result<Arc<dyn HostClient>, HostError> {
    let _ = (dial, window_ms);
    todo!("client.ts:404-414")
}

/// `hostBack(deps, withinMs)` (client.ts:418-420): `host_again` over `deps.client(Some(Again { within_ms }))`,
/// within `within_ms`, else `deps.host_wait_ms`, else `SERVICE_WAIT_MS`.
pub async fn host_back(deps: &VerbDeps, within_ms: Option<u64>) -> Result<Arc<dyn HostClient>, HostError> {
    let _ = (deps, within_ms);
    todo!("client.ts:418-420")
}

/// `untilSettled(client, work)` (client.ts:423-430): the work's answer, or a plain error `close_words()` when the
/// socket goes first.
pub async fn until_settled<T: Send>(client: &dyn HostClient, work: BoxFuture<'_, Result<T, HostError>>) -> Result<T, HostError> {
    let _ = (client, work);
    todo!("client.ts:423-430")
}

/// `pushedFrames(client)` (client.ts:435-451): frames held from the moment it is made, replayed into `follow`'s sink
/// in arrival order, then passed straight through; `stop` removes the sink and drops whatever is still held.
pub struct PushedFrames {
    _private: (),
}

impl PushedFrames {
    pub fn new(client: &dyn HostClient) -> PushedFrames {
        let _ = client;
        todo!("client.ts:435-438")
    }

    /// Replays every held frame that `pick` takes into `on`, then sends each later one there too. A `stop` from
    /// inside `on` drops the held frames not yet replayed.
    pub fn follow(&self, pick: Arc<dyn Fn(&Frame) -> bool + Send + Sync>, on: FrameSink) {
        let _ = (pick, on);
        todo!("client.ts:440-445")
    }

    pub fn stop(&self) {
        todo!("client.ts:446-449")
    }
}

// ---------------------------------------------------------------------------------------------------------------
// Output and the rows the listings print (client.ts:453-735). Spec: spec/client-rows.md.
// ---------------------------------------------------------------------------------------------------------------

/// `CliIO` (`packages/host/src/cli/io.ts:9-38`): the terminal a line runs at. A method the TS marks optional has a
/// default here that behaves as absent.
#[async_trait]
pub trait CliIo: Send + Sync {
    fn log(&self, line: &str);
    fn error(&self, line: &str);
    /// Raw text on stderr, no newline added. Absent: nothing.
    fn stream(&self, text: &str) {
        let _ = text;
    }
    /// Where a list that refreshes in place redraws; None off a terminal.
    fn redraw(&self) -> Option<Arc<dyn Redraw>> {
        None
    }
    /// The text muted for the stream's colours; None leaves it plain.
    fn muted(&self, text: &str) -> Option<String> {
        let _ = text;
        None
    }
    /// This process's own stdin and stdout as bytes, taken once; None on a line carried from a machine.
    fn bytes(&self) -> Option<IoBytes> {
        None
    }
    /// A yes-or-no question; answers "yes" or "no".
    async fn ask(&self, question: &str) -> Result<String, HostError>;
    fn is_tty(&self) -> bool {
        false
    }
    fn same_screen(&self) -> bool {
        false
    }
    async fn ask_secret(&self, question: &str, variable: Option<&str>) -> Result<String, HostError>;
    /// Whether `answer_key` is offered (only at a terminal).
    fn answers_keys(&self) -> bool {
        false
    }
    /// One of `accept`, or None when `until` settles first.
    async fn answer_key(&self, accept: &[String], until: BoxFuture<'_, ()>) -> Option<String> {
        let _ = (accept, until);
        None
    }
}

/// `bytes` of `CliIO`: stdin and stdout as raw streams.
pub struct IoBytes {
    pub input: Box<dyn tokio::io::AsyncRead + Send + Unpin>,
    pub output: Box<dyn tokio::io::AsyncWrite + Send + Unpin>,
}

/// `Out` and `formatter(io, json)` (client.ts:455-470): with `json` every `emit` is `json_line(value)` through
/// `io.log` and nothing else prints; without it `emit` logs its line when there is one and `stream` writes to
/// `io.stream`. `Out::quiet()` is io.ts's `QUIET`: both do nothing.
#[derive(Clone)]
pub struct Out {
    io: Option<Arc<dyn CliIo>>,
    json: bool,
}

impl Out {
    /// `formatter(io, json)` (client.ts:460).
    pub fn formatter(io: Arc<dyn CliIo>, json: bool) -> Out {
        Out { io: Some(io), json }
    }

    /// `QUIET` (io.ts:226).
    pub fn quiet() -> Out {
        Out { io: None, json: false }
    }

    pub fn emit(&self, value: &Value, line: Option<&str>) {
        let _ = (value, line, &self.io, self.json);
        todo!("client.ts:462-465")
    }

    pub fn stream(&self, text: &str) {
        let _ = text;
        todo!("client.ts:466-468")
    }
}

/// `c1Escaped(out)` (client.ts:474-479): a writer that passes every chunk through `escape_c1`, decoding UTF-8 across
/// chunk boundaries so a character split over two writes is escaped whole.
pub struct C1Escaped<W> {
    pub inner: W,
    pending: Vec<u8>,
}

impl<W: std::io::Write> C1Escaped<W> {
    pub fn new(inner: W) -> C1Escaped<W> {
        C1Escaped { inner, pending: Vec::new() }
    }
}

impl<W: std::io::Write> std::io::Write for C1Escaped<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = (buf, &self.pending);
        todo!("client.ts:476")
    }

    /// Writes out what the decoder still holds, as `flush` of the Transform does (client.ts:476).
    fn flush(&mut self) -> std::io::Result<()> {
        todo!("client.ts:476")
    }
}

/// `placeLines(places)` (client.ts:483-504).
pub fn place_lines(places: &[PlaceView]) -> Vec<String> {
    let _ = places;
    todo!("client.ts:483-504")
}

/// This computer's platform word (`hostPlatform`, client.ts:508): "darwin" on macOS, else "linux".
pub fn host_platform() -> &'static str {
    todo!("client.ts:508")
}

/// `STARTS_AT` (client.ts:516): the column index of STARTS, dropped where no row reads its starts.
pub const STARTS_AT: usize = 11;

/// `computerLines(places, platform, spend, pending)` (client.ts:518-554).
pub fn computer_lines(places: &[PlaceView], platform: &str, spend: &[PlaceSpend], pending: &[PendingComputer]) -> Vec<String> {
    let _ = (places, platform, spend, pending);
    todo!("client.ts:518-554")
}

/// `followLine(computer)` (client.ts:557-560).
pub fn follow_line(name: &str, recipe: Option<&str>) -> String {
    let _ = (name, recipe);
    todo!("client.ts:557-560")
}

/// `recipeLines(recipes)` (client.ts:563-566).
pub fn recipe_lines(recipes: &[RecipeView]) -> Vec<String> {
    let _ = recipes;
    todo!("client.ts:563-566")
}

/// `recipeShownLines(recipe, hash)` (client.ts:579-590).
pub fn recipe_shown_lines(recipe: &RecipeView, hash: &str) -> Vec<String> {
    let _ = (recipe, hash);
    todo!("client.ts:569-590")
}

/// `recipeSavedLine` (client.ts:593).
pub fn recipe_saved_line(recipe: &RecipeView) -> String {
    let _ = recipe;
    todo!("client.ts:593")
}

/// `recipeRemovedLine` (client.ts:596-597).
pub fn recipe_removed_line(recipe: &RecipeView) -> String {
    let _ = recipe;
    todo!("client.ts:596-597")
}

/// `oneOf(name, words, value, on)` (client.ts:600-604): None for None; a word of the set as given; anything else
/// a usage refusal `<flag> takes one of a, b, and got "x".` with the fix `Name one of those.`
pub fn one_of(name: &str, words: &[&str], value: Option<&str>, on: Option<&str>) -> Result<Option<String>, HostError> {
    let _ = (name, words, value, on);
    todo!("client.ts:600-604")
}

/// What `readUsage` answers (client.ts:608-611): the two reads as the host wrote them.
#[derive(Clone, Debug, PartialEq)]
pub struct UsageRead {
    pub accounts: Value,
    pub used: Value,
}

/// `readUsage(client, range, by)` (client.ts:608-611): `usage.accounts {fresh: true}` and `usage.used` sent
/// together, before either answers.
pub async fn read_usage(client: &dyn HostClient, range: UsageRange, by: UsageSplit) -> Result<UsageRead, HostError> {
    let _ = (client, range, by);
    todo!("client.ts:608-611")
}

/// `accountNamed(rows, word)` (client.ts:614-622).
pub fn account_named<'a>(rows: &'a [AccountRow], word: &str) -> Result<&'a AccountRow, HostError> {
    let _ = (rows, word);
    todo!("client.ts:614-622")
}

/// `usageTableLines(read, now)` (client.ts:625-650). `now` is epoch milliseconds.
pub fn usage_table_lines(accounts: &[AccountRow], used: &UsedAnswer, now: f64) -> Vec<String> {
    let _ = (accounts, used, now);
    todo!("client.ts:625-650")
}

/// What `readComputers` answers (client.ts:653-656).
#[derive(Clone, Debug, PartialEq)]
pub struct ComputersRead {
    pub computers: Vec<PlaceView>,
    pub spend: Vec<PlaceSpend>,
    pub pending: Vec<PendingComputer>,
}

/// `readComputers(client)` (client.ts:653-656): `places.list` and `cost.spend` sent together; `pending` empty where
/// the listing carries none.
pub async fn read_computers(client: &dyn HostClient) -> Result<ComputersRead, HostError> {
    let _ = client;
    todo!("client.ts:653-656")
}

/// `SETTING_RESETS` (client.ts:659): `PlaceSettingWord.options` in its order.
pub fn setting_resets() -> Vec<&'static str> {
    todo!("client.ts:659")
}

/// What a computer is named by on `set_computer`: a word to look up, or the row already read.
pub enum ComputerRef<'a> {
    Word(&'a str),
    Row(&'a PlaceView),
}

/// `setComputer(client, ref, set, reset, also)` (client.ts:664-669): one `places.set` with `placeId`, then the
/// settings' keys, then `reset` only when non-empty, then the `also` fields that are given.
pub async fn set_computer(
    client: &dyn HostClient,
    computer: ComputerRef<'_>,
    set: &PlaceSettingsAsk,
    reset: &[PlaceSettingWord],
    also: &PlaceSetAlso,
) -> Result<(PlaceView, String), HostError> {
    let _ = (client, computer, set, reset, also);
    todo!("client.ts:664-669")
}

/// `computerSetLines(computer, was, also, settled)` (client.ts:673-680).
pub fn computer_set_lines(computer: &PlaceView, was: &str, also: &PlaceSetAlso, settled: bool) -> Vec<String> {
    let _ = (computer, was, also, settled);
    todo!("client.ts:673-680")
}

/// `flagFor(flagName, on)` (client.ts:683-685).
pub fn flag_for(flag_name: &str, on: Option<&str>) -> String {
    let _ = (flag_name, on);
    todo!("client.ts:683-685")
}

/// `napAsked(word, on)` (client.ts:688-693): "off" is 0; whole minutes 1 to `NAP_AFTER_MAX_MS / 60000`.
pub fn nap_asked(word: &str, on: &str) -> Result<u64, HostError> {
    let _ = (word, on);
    todo!("client.ts:688-693")
}

/// `turnLimitAsked(word, on)` (client.ts:696-701): "off" is 0; whole hours 1 to `TURN_LIMIT_MAX_MS / 3600000`.
pub fn turn_limit_asked(word: &str, on: &str) -> Result<u64, HostError> {
    let _ = (word, on);
    todo!("client.ts:696-701")
}

/// `deleteAfterAsked(word, on)` (client.ts:704-708): whole minutes 0 to `DELETE_AFTER_MAX_MS / 60000`.
pub fn delete_after_asked(word: &str, on: &str) -> Result<u64, HostError> {
    let _ = (word, on);
    todo!("client.ts:704-708")
}

/// `dollarsAsked(word, on)` (client.ts:711-715): JS `Number(word)`, finite and not below zero, a blank word refused.
pub fn dollars_asked(word: &str, on: &str) -> Result<f64, HostError> {
    let _ = (word, on);
    todo!("client.ts:711-715")
}

/// `table(rows)` (client.ts:731-734): columns padded to their widest cell in UTF-16 units, two spaces apart, the
/// last cell of each row never padded, every line's end trimmed of white space as `trimEnd` does.
pub fn table(rows: &[Vec<String>]) -> Vec<String> {
    let _ = rows;
    todo!("client.ts:731-734")
}

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
    pub terminal: Option<Arc<dyn RelayTerminal>>,
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
    let _ = ctx;
    todo!("client.ts:798")
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
    Integer { min: Option<i64> },
    /// `z.array(z.string())`, with its `.min()` where it has one.
    StringList { min: Option<usize> },
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
    let _ = (argv, table);
    todo!("client.ts:838-851")
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
        todo!("the entry's name")
    }

    /// The line form of a `Cli` or `CliOnly` entry; None for the other two.
    pub fn line(&self) -> Option<LineVerb<'_>> {
        todo!("\"run\" in v")
    }

    /// `cloud === true` on a `Cli` or `CliOnly` entry.
    pub fn is_cloud(&self) -> bool {
        todo!("verbs.ts:376")
    }
}

impl<'a> LineVerb<'a> {
    pub fn name(&self) -> &'static str {
        todo!()
    }
    pub fn usage(&self) -> &'a str {
        todo!()
    }
    pub fn about(&self) -> &'a str {
        todo!()
    }
    pub fn page(&self) -> Page {
        todo!()
    }
    pub fn options(&self) -> &'a FlagTable {
        todo!()
    }
    pub fn run(&self) -> RunFn {
        todo!()
    }
    pub fn starts_no_host(&self) -> Option<&'static str> {
        todo!()
    }
    pub fn any_release(&self) -> Option<&'static str> {
        todo!()
    }
    /// Only a `CliOnly` entry carries one.
    pub fn host_side(&self) -> Option<&'static str> {
        todo!()
    }
}

/// `readsHere(verb)` (client.ts:904-906).
pub fn reads_here(verb: &Verb) -> Option<&'static str> {
    let _ = verb;
    todo!("client.ts:904-906")
}

/// `hasTool(verb)` (client.ts:909-911): the tool of a `Cli`, `ToolOnly` or `CommandTool` entry.
pub fn has_tool(verb: &Verb) -> Option<&Tool> {
    let _ = verb;
    todo!("client.ts:909-911")
}

/// `toolName(words)` (client.ts:914-916): every space to `_`.
pub fn tool_name(words: &str) -> String {
    let _ = words;
    todo!("client.ts:914-916")
}

/// `COMMON` (client.ts:919-924): host, state (strings), json, help (booleans, help also `-h`), in that order.
pub fn common() -> FlagTable {
    todo!("client.ts:919-924")
}

/// What `takeCommon` splits a line into (client.ts:933).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TakenCommon {
    pub common: Vec<String>,
    pub rest: Vec<String>,
}

/// `takeCommon(argv)` (client.ts:933-959).
pub fn take_common(argv: &[String]) -> TakenCommon {
    let _ = argv;
    todo!("client.ts:933-959")
}

/// `PICK_FLAGS` (client.ts:962).
pub const PICK_FLAGS: [&str; 3] = ["model", "effort", "access"];
/// `SEND_FLAGS` (client.ts:966).
pub const SEND_FLAGS: [&str; 2] = ["model", "effort"];

/// `PICK_OPTIONS` (client.ts:968): each pick flag a string.
pub fn pick_options() -> FlagTable {
    todo!("client.ts:968")
}

/// `SEND_OPTIONS` (client.ts:969).
pub fn send_options() -> FlagTable {
    todo!("client.ts:969")
}

/// `flag(flags, name)` (client.ts:971): the value only where it is a string.
pub fn flag<'a>(flags: &'a Flags, name: &str) -> Option<&'a str> {
    let _ = (flags, name);
    todo!("client.ts:971")
}

/// `flagList(flags, name)` (client.ts:972): the value only where it is a list, else empty.
pub fn flag_list(flags: &Flags, name: &str) -> Vec<String> {
    let _ = (flags, name);
    todo!("client.ts:972")
}

/// `flags[name] === true`.
pub fn flag_set(flags: &Flags, name: &str) -> bool {
    let _ = (flags, name);
    todo!("flags[name] === true")
}

/// `PLACES_FIX` (client.ts:974).
pub const PLACES_FIX: &str = "Run wsp computers.";

/// `placeNamed(client, word)` (client.ts:978-983): one `places.list`, the first row `names_place` takes.
pub async fn place_named(client: &dyn HostClient, word: &str) -> Result<PlaceView, HostError> {
    let _ = (client, word);
    todo!("client.ts:978-983")
}

/// `absolutePath(named, path)` (client.ts:987-990): refused unless it opens with `/`.
pub fn absolute_path(named: &str, path: &str) -> Result<String, HostError> {
    let _ = (named, path);
    todo!("client.ts:987-990")
}

/// `absoluteFolder(cwd)` (client.ts:994-996).
pub fn absolute_folder(cwd: Option<&str>) -> Result<Option<String>, HostError> {
    let _ = cwd;
    todo!("client.ts:994-996")
}

/// `accessWordOf(given)` (client.ts:1000-1004): one of ask, auto-edit, full, plan.
pub fn access_word_of(given: &str) -> Result<wsp_wire::types::thread_defaults::AccessChoice, HostError> {
    let _ = given;
    todo!("client.ts:1000-1004")
}

/// `under(path, folder)` (client.ts:1007).
pub fn under(path: &str, folder: &str) -> bool {
    let _ = (path, folder);
    todo!("client.ts:1007")
}

/// `RELEASE_WORDS` (client.ts:1011-1020).
pub mod release_words {
    pub fn said(mine: &str, host: &str, theirs: &str) -> String {
        let _ = (mine, host, theirs);
        todo!("client.ts:1012")
    }
    pub fn host_here(state: &str) -> String {
        let _ = state;
        todo!("client.ts:1013")
    }
    pub fn host_at(r#where: &str) -> String {
        let _ = r#where;
        todo!("client.ts:1014")
    }
    pub const RESTART: &str = "restart that host with wsp restart.";
    pub fn reopen_app(mine: &str) -> String {
        let _ = mine;
        todo!("client.ts:1016")
    }
    /// `UP_RESTART_LINE`.
    pub fn restart_up() -> &'static str {
        todo!("client.ts:1017")
    }
    pub fn update_there(mine: &str) -> String {
        let _ = mine;
        todo!("client.ts:1018")
    }
    pub fn update_here(line: &str, theirs: &str) -> String {
        let _ = (line, theirs);
        todo!("client.ts:1019")
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
    let _ = (mine, road);
    todo!("client.ts:1029-1032")
}

/// `releaseRefusal(mine, theirs, host, road)` (client.ts:1035-1041). `argv` is this process's, read by
/// `release_update_line` when this line is the older end.
pub fn release_refusal(mine: &str, theirs: &str, host: &ReleaseHost, road: Option<&str>, argv: &[String]) -> HostError {
    let _ = (mine, theirs, host, road, argv);
    todo!("client.ts:1035-1041")
}

/// `otherVersion(op)` (client.ts:1044).
pub fn other_version(op: &str) -> String {
    let _ = op;
    todo!("client.ts:1044")
}

/// `threadIdOf(t)` (client.ts:1047): `threadId` where set, else `id`.
pub fn thread_id_of(thread: &ThreadView) -> String {
    let _ = thread;
    todo!("client.ts:1047")
}

/// `THREAD_PREFIX_WORD` (client.ts:1053).
pub const THREAD_PREFIX_WORD: &str = "wsp thread read, wsp send and wsp stop take its first characters";

/// The line a caller's opening names a thread with (`opened` of `openedThreadLine`, client.ts:1056).
pub type OpenedFn<'a> = &'a (dyn Fn(&str, Option<&str>) -> String + Send + Sync);

/// `openedThreadLine(threadId, opened, folder)` (client.ts:1056).
pub fn opened_thread_line(thread_id: &str, opened: Option<OpenedFn<'_>>, folder: Option<&str>) -> String {
    let _ = (thread_id, opened, folder);
    todo!("client.ts:1056")
}

/// `openedThreadSaid(threadId, opened, folder)` (client.ts:1060): the line, two spaces, `THREAD_PREFIX_WORD`.
pub fn opened_thread_said(thread_id: &str, opened: Option<OpenedFn<'_>>, folder: Option<&str>) -> String {
    let _ = (thread_id, opened, folder);
    todo!("client.ts:1060")
}

/// `printable(s)` (client.ts:1063): drops U+0000-U+0008, U+000B-U+001F and U+007F-U+009F; keeps tab and newline.
pub fn printable(s: &str) -> String {
    let _ = s;
    todo!("client.ts:1063")
}

/// `cell(s)` (client.ts:1066): newline and tab to a space, then `without_control_chars`.
pub fn cell(s: &str) -> String {
    let _ = s;
    todo!("client.ts:1066")
}

/// `pick(o)` (client.ts:1069-1071): the given fields in order, the absent ones dropped. A `Some(Value::Null)` is
/// kept: only `undefined` is left out.
pub fn pick(fields: Vec<(&str, Option<Value>)>) -> Frame {
    let _ = fields;
    todo!("client.ts:1069-1071")
}

/// `failed(io, json, e, prefix)` (client.ts:1076-1080): one line on stderr, `json_line(verb_failure(e))` under
/// --json, else `say_once(prefix, message)`; answers the class's exit code.
pub fn failed(io: &dyn CliIo, json: bool, e: &HostError, prefix: &str) -> i32 {
    let _ = (io, json, e, prefix);
    todo!("client.ts:1076-1080")
}

/// `toolFailure(e, usage)` (client.ts:1085-1088).
pub fn tool_failure(e: &HostError, usage: Option<&str>) -> CallToolResult {
    let _ = (e, usage);
    todo!("client.ts:1085-1088")
}

/// `jsonAsked(argv)` (client.ts:1091-1094): `--json` among the words before the first `--`.
pub fn json_asked(argv: &[String]) -> bool {
    let _ = argv;
    todo!("client.ts:1091-1094")
}

/// `hostSchemaRefusal(e, usage)` (client.ts:1099-1102).
pub fn host_schema_refusal(e: &HostError, usage: &str) -> Option<HostError> {
    let _ = (e, usage);
    todo!("client.ts:1099-1102")
}
