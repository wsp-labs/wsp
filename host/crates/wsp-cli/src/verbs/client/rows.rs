// SPDX-License-Identifier: AGPL-3.0-only
//! The rows the listings print and the output every verb makes (`packages/host/src/verbs/client.ts:453-735`): `Out`,
//! the C1-escaping writer, the tables of places, computers, recipes and usage, and the reads and the write behind
//! them. Spec: `spec/client-rows.md`. The socket and the verb table are in the parent module.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};

use wsp_wire::types::place_state::{DELETE_AFTER_MAX_MS, NAP_AFTER_MAX_MS, TURN_LIMIT_MAX_MS};
use wsp_wire::types::recipe_file::{RecipeView, NO_RECIPE, RECIPE_KINDS};
use wsp_wire::types::usage::{AccountRow, LimitKind, UsageRange, UsageSplit, UsedAnswer, USAGE_WORDS};
use wsp_wire::views::place::{PendingComputer, PlaceSetAlso};
use wsp_wire::views::place_more::{PlaceSettingWord, PlaceSettingsAsk, PlaceSpend, PlaceView};
use wsp_wire::wire::daemon_version::place_daemon_behind;
use wsp_words::exit::{escape_c1, json_line, usage_refusal, HostError, Kind};
use wsp_words::js::{js_trim, js_trim_end, number_string, to_number};
use wsp_words::place_name::table_name;
use wsp_words::place_state::{place_room, place_settings_line, place_spend_limit};
use wsp_words::usage::{account_state, credits_word, fresh_in, list_words, no_such_account_line, used_ask, used_price, window_cell};
use wsp_words::views::place::computer_kind_word;
use wsp_words::views::place_setup_words::{pending_step_words, pending_word, place_state_of, setup_word};
use wsp_words::words::computer::Platform;
use wsp_words::words::places::{agents_cell, spend_meter_word};
use wsp_words::words::units::{fmt_bytes, fmt_price, fmt_tokens};
use wsp_words::workspace_state::absent_computer;

use crate::watch::Redraw;

use super::{
    frame_of_value, is_provider, json_string, kind_word, optional_field, place_named, place_ref, places_of, reply_field, BoxFuture, Frame,
    HostClient,
};

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
        let Some(io) = &self.io else { return };
        if self.json {
            io.log(&json_line(value, None));
        } else if let Some(line) = line {
            io.log(line);
        }
    }

    pub fn stream(&self, text: &str) {
        if let (Some(io), false) = (&self.io, self.json) {
            io.stream(text);
        }
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

impl<W: std::io::Write> C1Escaped<W> {
    /// An incomplete character waits for its next bytes unless `end` says none will come.
    fn decode(&mut self, end: bool) -> String {
        let bytes = std::mem::take(&mut self.pending);
        let mut out = String::new();
        let mut at = 0;
        loop {
            match std::str::from_utf8(&bytes[at..]) {
                Ok(text) => {
                    out.push_str(text);
                    at = bytes.len();
                    break;
                }
                Err(e) => {
                    let valid = e.valid_up_to();
                    out.push_str(std::str::from_utf8(&bytes[at..at + valid]).expect("the valid prefix decodes"));
                    at += valid;
                    match e.error_len() {
                        Some(len) => {
                            out.push('\u{FFFD}');
                            at += len;
                        }
                        None => {
                            if end {
                                out.push('\u{FFFD}');
                                at = bytes.len();
                            }
                            break;
                        }
                    }
                }
            }
        }
        self.pending = bytes[at..].to_vec();
        out
    }
}

impl<W: std::io::Write> std::io::Write for C1Escaped<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.pending.extend_from_slice(buf);
        let text = self.decode(false);
        self.inner.write_all(escape_c1(&text).as_bytes())?;
        Ok(buf.len())
    }

    /// Writes out what the decoder still holds, as `flush` of the Transform does (client.ts:476).
    fn flush(&mut self) -> std::io::Result<()> {
        let text = self.decode(true);
        self.inner.write_all(escape_c1(&text).as_bytes())?;
        self.inner.flush()
    }
}

/// `placeLines(places)` (client.ts:483-504).
pub fn place_lines(places: &[PlaceView]) -> Vec<String> {
    if places.is_empty() {
        return vec!["This host holds no place. wsp add prints the join line for a computer you are sitting at.".to_owned()];
    }
    let mut rows = vec![strings(&[
        "PLACE",
        "KIND",
        "CORES",
        "MEMORY",
        "DISK FREE",
        "COPIES",
        "PRESENT",
        "FORKS",
        "LAST SEEN",
        "DEFAULT",
        "BEHIND",
        "IMAGE",
    ])];
    for p in places {
        rows.push(vec![
            p.name.clone(),
            kind_word(p).to_owned(),
            cores_cell(p),
            memory_cell(p),
            p.disk_free_bytes.map_or_else(String::new, |bytes| fmt_bytes(bytes.as_f64())),
            p.copies.map_or_else(String::new, |copies| copies.word().to_owned()),
            if is_provider(p) {
                fmt_price(rate_of(p))
            } else if p.present == Some(true) {
                "yes".to_owned()
            } else {
                "no".to_owned()
            },
            forks_cell(p),
            if is_provider(p) { String::new() } else { p.last_seen_at.clone().unwrap_or_default() },
            if p.default { "default".to_owned() } else { String::new() },
            place_daemon_behind(p.daemon_version.map(|version| version.get())).unwrap_or_default(),
            p.build.clone().unwrap_or_default(),
        ]);
    }
    table(&rows)
}

/// This computer's platform word (`hostPlatform`, client.ts:508): "darwin" on macOS, else "linux".
pub fn host_platform() -> &'static str {
    if cfg!(target_os = "macos") {
        "darwin"
    } else {
        "linux"
    }
}

/// `STARTS_AT` (client.ts:516): the column index of STARTS, dropped where no row reads its starts.
pub const STARTS_AT: usize = 11;

/// `computerLines(places, platform, spend, pending)` (client.ts:518-554).
pub fn computer_lines(places: &[PlaceView], platform: &str, spend: &[PlaceSpend], pending: &[PendingComputer]) -> Vec<String> {
    if places.is_empty() {
        return vec!["This host holds no computer. wsp add prints the join line for a computer you are sitting at.".to_owned()];
    }
    let platform = if platform == "darwin" { Platform::Darwin } else { Platform::Linux };
    let mut rows = vec![strings(&[
        "COMPUTER",
        "KIND",
        "CORES",
        "MEMORY",
        "DISK FREE",
        "COPIES",
        "PRESENT",
        "WORKSPACES",
        "THREADS",
        "MACHINES",
        "SPEND",
        "STARTS",
        "STATE",
        "LAST SEEN",
        "BEHIND",
        "IMAGE",
        "TOOLS",
        "AGENTS",
    ])];
    for p in places {
        let today = spend.iter().find(|s| s.place == p.id).map(|s| s.today_usd.0);
        let add = pending.iter().find(|a| a.place_id.as_deref() == Some(p.id.as_str()));
        let absent = (p.present == Some(false)).then(|| absent_computer(&p.name, None));
        let state = place_state_of(p, absent.as_ref(), today, add).word;
        let [threads, machines] = cap_cells(p);
        rows.push(vec![
            table_name(&place_ref(p)),
            computer_kind_word(p, &platform),
            cores_cell(p),
            memory_cell(p),
            p.disk_free_bytes.map_or_else(String::new, |bytes| fmt_bytes(bytes.as_f64())),
            p.copies.map_or_else(String::new, |copies| copies.word().to_owned()),
            if is_provider(p) {
                fmt_price(rate_of(p))
            } else if p.present == Some(true) {
                "yes".to_owned()
            } else {
                "no".to_owned()
            },
            forks_cell(p),
            threads,
            machines,
            spend_cell(p, today),
            p.starts.as_ref().map_or_else(String::new, |starts| {
                format!("{}/{} today", number_string(starts.day.used.as_f64()), number_string(starts.day.limit.as_f64()))
            }),
            state,
            if is_provider(p) { String::new() } else { p.last_seen_at.clone().unwrap_or_default() },
            place_daemon_behind(p.daemon_version.map(|version| version.get())).unwrap_or_default(),
            p.build.clone().unwrap_or_default(),
            match add {
                None => setup_word(p.setup.as_ref(), p.applied.as_ref()),
                Some(add) => pending_step_words(add.step).to_owned(),
            },
            agents_cell(p),
        ]);
    }
    // An add that has not reached Set up is a row of its own until it does: its word and how far it got.
    for add in pending.iter().filter(|p| p.place_id.as_ref().is_none_or(|id| !places.iter().any(|r| r.id == *id))) {
        let word = pending_word(add);
        let mut row = vec![add.name.clone().unwrap_or_else(|| add.address.clone()), "pending".to_owned()];
        row.extend(std::iter::repeat_n(String::new(), 10));
        row.push(word.word);
        row.extend(std::iter::repeat_n(String::new(), 3));
        row.push(word.sentence.unwrap_or_default());
        row.push(String::new());
        rows.push(row);
    }
    let starts = places.iter().any(|p| p.starts.is_some());
    let shown: Vec<Vec<String>> = rows
        .into_iter()
        .map(|row| row.into_iter().enumerate().filter(|(at, _)| starts || *at != STARTS_AT).map(|(_, cell)| cell).collect())
        .collect();
    table(&shown)
}

/// `followLine(computer)` (client.ts:557-560).
pub fn follow_line(name: &str, recipe: Option<&str>) -> String {
    match recipe {
        None => format!("{name} follows no recipe; it keeps what it has"),
        Some(recipe) if recipe == NO_RECIPE => format!("{name} follows no recipe; it keeps what it has"),
        Some(recipe) => format!("{name} follows {recipe}; a change to it reaches {name} on its own"),
    }
}

/// `recipeLines(recipes)` (client.ts:563-566).
pub fn recipe_lines(recipes: &[RecipeView]) -> Vec<String> {
    if recipes.is_empty() {
        return vec![
            "No recipe is saved yet. The app's Add a computer saves one, and wsp recipes save <name> --from <computer> saves what a computer was set up with.".to_owned(),
        ];
    }
    let mut rows = vec![strings(&["RECIPE", "HOLDS", "SAVED", "COMPUTERS"])];
    for recipe in recipes {
        let saved = recipe
            .saved_at
            .as_deref()
            .map_or_else(String::new, |at| String::from_utf16_lossy(&at.encode_utf16().take(10).collect::<Vec<u16>>()));
        rows.push(vec![recipe.name.clone(), recipe.summary.clone(), saved, recipe.machines.join(", ")]);
    }
    table(&rows)
}

/// `recipeRowWords(kind, file)` (client.ts:569-575): a recipe row's name and what it says beyond the name.
fn recipe_row_words(kind: &str, file: &Value) -> Vec<String> {
    let Some(rows) = file.get(kind).and_then(Value::as_object) else { return Vec::new() };
    rows.iter()
        .filter(|(_, row)| !(kind == "configs" && row.is_null()))
        .map(|(name, row)| {
            let said: Vec<String> =
                row.as_object().map_or_else(Vec::new, |fields| fields.iter().filter_map(|(key, value)| field_said(key, value)).collect());
            if said.is_empty() {
                name.clone()
            } else {
                format!("{name} ({})", said.join("; "))
            }
        })
        .collect()
}

fn field_said(key: &str, value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::Array(items) if items.is_empty() => None,
        Value::Array(items) => Some(format!("{key} {}", items.iter().map(js_string).collect::<Vec<_>>().join(", "))),
        other => Some(format!("{key} {}", js_string(other))),
    }
}

/// `String(v)` of a JSON value, as the recipe rows print it.
fn js_string(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::String(text) => text.clone(),
        Value::Array(items) => {
            items.iter().map(|item| if item.is_null() { String::new() } else { js_string(item) }).collect::<Vec<_>>().join(",")
        }
        Value::Object(_) => "[object Object]".to_owned(),
    }
}

/// `recipeShownLines(recipe, hash)` (client.ts:579-590).
pub fn recipe_shown_lines(recipe: &RecipeView, hash: &str) -> Vec<String> {
    let file = serde_json::to_value(&recipe.file).expect("a recipe file writes as JSON");
    let mut lines = vec![format!("{}: {}", recipe.name, recipe.summary)];
    for kind in RECIPE_KINDS {
        let rows = recipe_row_words(kind, &file);
        if !rows.is_empty() {
            lines.push(format!("  {kind:<8} {}", rows.join(", ")));
        }
    }
    if file.get("copyKeys") == Some(&Value::Bool(true)) {
        lines.push("  keys     copied with the servers that carry them".to_owned());
    }
    let followed = if recipe.machines.is_empty() { "no computer".to_owned() } else { recipe.machines.join(", ") };
    lines.push(format!("  followed by {followed}"));
    lines.push(format!("  hash     {hash}"));
    lines
}

/// `recipeSavedLine` (client.ts:593).
pub fn recipe_saved_line(recipe: &RecipeView) -> String {
    let followed = if recipe.machines.is_empty() { String::new() } else { format!(", followed by {}", recipe.machines.join(", ")) };
    format!("saved {} ({}){followed}", recipe.name, recipe.summary)
}

/// `recipeRemovedLine` (client.ts:596-597).
pub fn recipe_removed_line(recipe: &RecipeView) -> String {
    if recipe.machines.is_empty() {
        format!("removed {}", recipe.name)
    } else {
        format!("removed {}; {} keep what it put there and follow no recipe", recipe.name, recipe.machines.join(", "))
    }
}

/// `oneOf(name, words, value, on)` (client.ts:600-604): None for None; a word of the set as given; anything else
/// a usage refusal `<flag> takes one of a, b, and got "x".` with the fix `Name one of those.`
pub fn one_of(name: &str, words: &[&str], value: Option<&str>, on: Option<&str>) -> Result<Option<String>, HostError> {
    let Some(value) = value else { return Ok(None) };
    if !words.contains(&value) {
        let said = format!("{} takes one of {}, and got {}.", flag_for(&format!("--{name}"), on), words.join(", "), json_string(value));
        return Err(usage_refusal(&said, "Name one of those."));
    }
    Ok(Some(value.to_owned()))
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
    let (accounts, used) = futures::join!(
        client.request("usage.accounts", frame_of_value(json!({ "fresh": true }))),
        client.request("usage.used", frame_of_value(serde_json::to_value(used_ask(&range, &by)).expect("a usage ask writes as JSON"))),
    );
    let (accounts, used) = (accounts?, used?);
    Ok(UsageRead {
        accounts: accounts.get("accounts").cloned().unwrap_or(Value::Null),
        used: used.get("used").cloned().unwrap_or(Value::Null),
    })
}

/// `accountNamed(rows, word)` (client.ts:614-622).
pub fn account_named<'a>(rows: &'a [AccountRow], word: &str) -> Result<&'a AccountRow, HostError> {
    if let Some(exact) = rows.iter().find(|row| row.key == word) {
        return Ok(exact);
    }
    let said = word.to_lowercase();
    let named: Vec<&'a AccountRow> = rows
        .iter()
        .filter(|row| row.label.to_lowercase() == said || row.address.as_ref().is_some_and(|address| address.to_lowercase() == said))
        .collect();
    match named.as_slice() {
        [one] => Ok(*one),
        [] => Err(HostError::with_kind(no_such_account_line(word), Kind::NotFound)),
        many => {
            let keys: Vec<&str> = many.iter().map(|row| row.key.as_str()).collect();
            Err(usage_refusal(&format!("{word} names {} accounts: {}.", many.len(), list_words(&keys)), "Name one by its key."))
        }
    }
}

/// `usageTableLines(read, now)` (client.ts:625-650). `now` is epoch milliseconds.
pub fn usage_table_lines(accounts: &[AccountRow], used: &UsedAnswer, now: f64) -> Vec<String> {
    let window = |row: &AccountRow, kind: LimitKind| -> String {
        row.windows
            .as_ref()
            .and_then(|windows| windows.iter().find(|w| w.kind == kind))
            .map_or_else(String::new, |w| window_cell(w, now, None))
    };
    let mut lines: Vec<String> = if accounts.is_empty() {
        vec!["No agent is signed in on any computer.".to_owned()]
    } else {
        let mut rows = vec![strings(&["AGENT", "ACCOUNT", "COMPUTERS", "SESSION", "WEEK", "RESETS", "PLAN", "STATE"])];
        for row in accounts {
            rows.push(vec![
                wsp_catalog::catalog::agent_name(&row.agent),
                row.label.clone(),
                row.computers.join(", "),
                window(row, LimitKind::Session),
                window(row, LimitKind::Week),
                row.credits.as_ref().map_or_else(String::new, |credits| {
                    credits_word(credits.count.as_f64(), credits.next_expires_at.as_ref().map(|at| at.0), now)
                }),
                row.plan.clone().unwrap_or_default(),
                account_state(row),
            ]);
        }
        table(&rows)
    };
    lines.push(String::new());
    // Two agents running one model are two rows of one name, so a model's row names its agent first.
    let by_model = used.split == UsageSplit::Model;
    if used.rows.is_empty() {
        lines.push(format!("{}.", usage_word("noUse")));
        return lines;
    }
    let split_word = serde_json::to_value(used.split).ok().and_then(|word| word.as_str().map(str::to_uppercase)).unwrap_or_default();
    let mut head: Vec<String> = Vec::new();
    if by_model {
        head.push("AGENT".to_owned());
    }
    head.push(split_word);
    head.extend(strings(&["FRESH IN", "WRITTEN", "CACHED", "OUT", "PRICE"]));
    let mut rows = vec![head];
    for row in &used.rows {
        let price = used_price(row);
        let mut cells: Vec<String> = Vec::new();
        if by_model {
            cells.push(row.agent.as_deref().map(wsp_catalog::catalog::agent_name).unwrap_or_default());
        }
        cells.push(row.label.clone());
        let cache_write = row.tokens.cache_write.as_ref().map(|n| n.0);
        cells.push(fmt_tokens(fresh_in(row.tokens.input.0, row.tokens.cached.0, cache_write)));
        cells.push(fmt_tokens(cache_write.unwrap_or(0.0)));
        cells.push(fmt_tokens(row.tokens.cached.0));
        cells.push(fmt_tokens(row.tokens.output.0));
        cells.push([price.figure, price.word].into_iter().flatten().filter(|word| !word.is_empty()).collect::<Vec<_>>().join(" "));
        rows.push(cells);
    }
    lines.extend(table(&rows));
    lines
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
    let (listed, spent) = futures::join!(client.request("places.list", Frame::new()), client.request("cost.spend", Frame::new()));
    let (listed, spent) = (listed?, spent?);
    Ok(ComputersRead {
        computers: places_of(&listed, "places.list")?,
        spend: reply_field(&spent, "cost.spend", "places")?,
        pending: optional_field(&listed, "places.list", "pending")?.unwrap_or_default(),
    })
}

/// `SETTING_RESETS` (client.ts:659): `PlaceSettingWord.options` in its order.
pub fn setting_resets() -> Vec<&'static str> {
    vec!["threads", "machines", "spend", "size", "delete-after", "nap", "turn-limit", "max-machines", "max-depth"]
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
    let place = match computer {
        ComputerRef::Word(word) => place_named(client, word).await?,
        ComputerRef::Row(row) => row.clone(),
    };
    let mut params = Frame::new();
    params.insert("placeId".to_owned(), json!(place.id));
    params.extend(frame_of_value(serde_json::to_value(set).expect("settings write as JSON")));
    if !reset.is_empty() {
        params.insert("reset".to_owned(), json!(reset));
    }
    params.extend(frame_of_value(serde_json::to_value(also).expect("the settings written beside the row write as JSON")));
    let reply = client.request("places.set", params).await?;
    let computer: PlaceView = reply_field(&reply, "places.set", "place")?;
    Ok((computer, place.name))
}

/// `computerSetLines(computer, was, also, settled)` (client.ts:673-680).
pub fn computer_set_lines(computer: &PlaceView, was: &str, also: &PlaceSetAlso, settled: bool) -> Vec<String> {
    let mut lines = Vec::new();
    if also.name.is_some() && was != computer.name {
        lines.push(format!("{was} is {} now", computer.name));
    }
    if let Some(ssh) = &also.ssh {
        lines.push(format!("{} is reached over ssh as {} from now", computer.name, js_trim(ssh)));
    }
    if also.recipe.is_some() {
        lines.push(follow_line(&computer.name, computer.recipe.as_deref()));
    }
    let nothing_named = also.name.is_none() && also.ssh.is_none() && also.recipe.is_none();
    if settled || nothing_named {
        lines.push(place_settings_line(computer));
    }
    lines
}

/// `flagFor(flagName, on)` (client.ts:683-685).
pub fn flag_for(flag_name: &str, on: Option<&str>) -> String {
    match on {
        None => flag_name.to_owned(),
        Some(on) => format!("{flag_name} for {on}"),
    }
}

/// `napAsked(word, on)` (client.ts:688-693): "off" is 0; whole minutes 1 to `NAP_AFTER_MAX_MS / 60000`.
pub fn nap_asked(word: &str, on: &str) -> Result<u64, HostError> {
    if word == "off" {
        return Ok(0);
    }
    let minutes = to_number(word);
    if !is_digits(word) || minutes < 1.0 || minutes * 60_000.0 > NAP_AFTER_MAX_MS as f64 {
        let said = format!(
            "{} takes whole minutes from 1 to {}, or off, and got {}.",
            flag_for("--nap", Some(on)),
            NAP_AFTER_MAX_MS / 60_000,
            json_string(word)
        );
        return Err(usage_refusal(&said, "Write it as --nap 20 or --nap off."));
    }
    Ok(minutes as u64)
}

/// `turnLimitAsked(word, on)` (client.ts:696-701): "off" is 0; whole hours 1 to `TURN_LIMIT_MAX_MS / 3600000`.
pub fn turn_limit_asked(word: &str, on: &str) -> Result<u64, HostError> {
    if word == "off" {
        return Ok(0);
    }
    let hours = to_number(word);
    if !is_digits(word) || hours < 1.0 || hours * 3_600_000.0 > TURN_LIMIT_MAX_MS as f64 {
        let said = format!(
            "{} takes whole hours from 1 to {}, or off, and got {}.",
            flag_for("--turn-limit", Some(on)),
            TURN_LIMIT_MAX_MS / 3_600_000,
            json_string(word)
        );
        return Err(usage_refusal(&said, "Write it as --turn-limit 6 or --turn-limit off."));
    }
    Ok(hours as u64)
}

/// `deleteAfterAsked(word, on)` (client.ts:704-708): whole minutes from none to a day.
pub fn delete_after_asked(word: &str, on: &str) -> Result<u64, HostError> {
    let minutes = to_number(word);
    if !is_digits(word) || minutes * 60_000.0 > DELETE_AFTER_MAX_MS as f64 {
        let said = format!(
            "{} takes whole minutes from 0 to {}, and got {}.",
            flag_for("--delete-after", Some(on)),
            DELETE_AFTER_MAX_MS / 60_000,
            json_string(word)
        );
        return Err(usage_refusal(&said, "Write it as --delete-after 30, or 0 to delete once it is done."));
    }
    Ok(minutes as u64)
}

/// `dollarsAsked(word, on)` (client.ts:711-715): JS `Number(word)`, finite and not below zero, a blank word refused.
pub fn dollars_asked(word: &str, on: &str) -> Result<f64, HostError> {
    let usd = to_number(word);
    if js_trim(word).is_empty() || !usd.is_finite() || usd < 0.0 {
        let said = format!("{} takes dollars a day, zero or more, and got {}.", flag_for("--spend", Some(on)), json_string(word));
        return Err(usage_refusal(&said, "Write it as --spend 10."));
    }
    Ok(usd)
}

/// `table(rows)` (client.ts:731-734): columns padded to their widest cell in UTF-16 units, two spaces apart, the
/// last cell of each row never padded, every line's end trimmed of white space as `trimEnd` does.
pub fn table(rows: &[Vec<String>]) -> Vec<String> {
    // Widths carry over only through the rows that reach a column, as client.ts's reduce takes them.
    let mut widths: Vec<usize> = Vec::new();
    for row in rows {
        widths = row.iter().enumerate().map(|(at, cell)| widths.get(at).copied().unwrap_or(0).max(utf16_len(cell))).collect();
    }
    rows.iter()
        .map(|row| {
            let cells: Vec<String> = row
                .iter()
                .enumerate()
                .map(|(at, cell)| if at + 1 == row.len() { cell.clone() } else { pad_end(cell, widths.get(at).copied()) })
                .collect();
            js_trim_end(&cells.join("  ")).to_owned()
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------------------------
// Helpers private to this file.
// ---------------------------------------------------------------------------------------------------------------

fn strings(cells: &[&str]) -> Vec<String> {
    cells.iter().map(|cell| (*cell).to_owned()).collect()
}

fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

fn pad_end(text: &str, width: Option<usize>) -> String {
    match width {
        Some(width) if utf16_len(text) < width => format!("{text}{}", " ".repeat(width - utf16_len(text))),
        _ => text.to_owned(),
    }
}

fn is_digits(word: &str) -> bool {
    !word.is_empty() && word.bytes().all(|byte| byte.is_ascii_digit())
}

fn usage_word(key: &str) -> &'static str {
    USAGE_WORDS.iter().find(|(k, _)| *k == key).map_or("", |(_, word)| *word)
}

fn rate_of(p: &PlaceView) -> f64 {
    p.rate_usd_per_hour.as_ref().map_or(0.0, |rate| rate.0)
}

fn cores_cell(p: &PlaceView) -> String {
    p.shape.as_ref().map_or_else(String::new, |shape| number_string(shape.cpu.0))
}

fn memory_cell(p: &PlaceView) -> String {
    p.shape.as_ref().map_or_else(String::new, |shape| fmt_bytes(shape.mem_mb.0 * 1024.0 * 1024.0))
}

fn forks_cell(p: &PlaceView) -> String {
    p.forks.as_ref().map_or_else(String::new, |forks| {
        format!("{} of {}", number_string(forks.running.as_f64()), number_string(forks.running.as_f64() + forks.room.as_f64()))
    })
}

fn cap_cells(p: &PlaceView) -> [String; 2] {
    let room = place_room(p);
    let cell = |noun: &str| match &room {
        Some(room) if room.noun == noun => format!("{}/{}", number_string(room.running), number_string(room.at_once)),
        _ => String::new(),
    };
    [cell("thread"), cell("machine")]
}

fn spend_cell(p: &PlaceView, today_usd: Option<f64>) -> String {
    match (place_spend_limit(p), today_usd) {
        (Some(limit), Some(today)) => spend_meter_word(today, limit),
        _ => String::new(),
    }
}
