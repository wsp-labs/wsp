// SPDX-License-Identifier: AGPL-3.0-only
//! A config's server values written by name for a copy on another machine, and the guards that refuse a copy still
//! holding one: the helpers both reference writers (mcp_json_refer, mcp_toml_refer) share, and `servers_by_name`.
//! Port of packages/catalog/src/mcp.ts:159-456 at a3b870a6e; the spec is spec/mcp_refs.md.
use indexmap::{IndexMap, IndexSet};
use serde_json::Value;

use crate::mcp::{KnownValue, McpFormat, McpReferred, ReferredServer, WholeRef};
use wsp_words::exit::HostError;

/// `WSP_MCP_<server>_<header>` upper-cased, every character outside A-Z 0-9 _ an underscore (mcp.ts:181).
pub fn mcp_header_variable(server: &str, header: &str) -> String {
    todo!()
}

/// Whether the value holds a `\n` or a `\r` (mcp.ts:184).
pub fn crosses_lines(value: &str) -> bool {
    todo!()
}

/// The value whole, then without a leading `bearer`, `basic` or `token` word and its whitespace, any case
/// (mcp.ts:190).
pub fn value_forms(value: &str) -> Vec<String> {
    todo!()
}

/// Why a value typed for a catalog row's own variable does not travel as a server's (mcp.ts:374).
pub fn row_variable_line(name: &str, row: &str) -> String {
    todo!()
}

/// Names every machine's environment defines (mcp.ts:203).
pub const EVERY_MACHINE: [&str; 8] = ["HOME", "USER", "LOGNAME", "PATH", "PWD", "SHELL", "TMPDIR", "LANG"];

/// A part of a string a value can stand as whole: where, and its offset where the part is the string's own
/// characters rather than a decoding (mcp.ts:253). Offsets are UTF-16 units (rule 8.3) only if they leave the unit;
/// here they only slice the same string, so bytes serve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    pub text: String,
    pub at: String,
    pub start: Option<usize>,
}

/// Every part of `s` a value can stand as whole, in the TS generator's order (mcp.ts:262): the string; a `--flag=v`'s
/// value; a header line's value; what follows a scheme word; each query value raw, then decoded where decoding changes
/// it. The empty string has none.
pub fn places(s: &str, at: &str, start: Option<usize>) -> Vec<Place> {
    todo!()
}

/// `decodeURIComponent` of a query value with `+` as a space; the raw text where it is not valid percent-encoding
/// (mcp.ts:243).
pub fn query_value(raw: &str) -> String {
    todo!()
}

/// Refuses a copy whose definitions still hold a value taken out of them, whole in any place, or as its base64 with
/// padding, without, or url-safe (mcp.ts:288). The message names the server and where.
pub fn still_stands(entries: &[Value], servers: &[ReferredServer]) -> Result<(), HostError> {
    todo!()
}

/// Where a known value stands whole as the string's own characters, the outer place winning where two overlap
/// (mcp.ts:317); `(name, value, at, start, end)`.
pub fn known_in(s: &str, at: &str, known: &[KnownValue]) -> Vec<KnownHit> {
    todo!()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownHit {
    pub name: String,
    pub value: String,
    pub at: String,
    pub start: usize,
    pub end: usize,
}

/// The string with each hit replaced by `reference(name)` (mcp.ts:329).
pub fn spliced(s: &str, hits: &[KnownHit], reference: &dyn Fn(&str) -> String) -> String {
    todo!()
}

/// Known values in one of `server`'s strings, its own names taken first, refused where another server of the file
/// sets that name to another value (mcp.ts:341).
pub fn known_hits(s: &str, at: &str, known: &[KnownValue], servers: &[ReferredServer], server: &str, own: &IndexMap<String, String>) -> Result<Vec<KnownHit>, HostError> {
    todo!()
}

/// Refuses a server whose written strings still hold a known non-empty value (mcp.ts:353).
pub fn known_stands(strings: &[String], known: &[KnownValue], server: &str) -> Result<(), HostError> {
    todo!()
}

/// Each argument, command and address of a definition: `args[i]` and `command[i]` of a list, `command` as a string,
/// `url`, `httpUrl`; with where each stands ("an argument", "the command", "the address"), mcp.ts:359.
pub fn arg_strings(def: &serde_json::Map<String, Value>) -> Vec<(crate::jsonc_tree::JsonPath, String, String)> {
    todo!()
}

/// The minted variable of each header, refused where two would travel under one (mcp.ts:377).
pub fn header_variables(server: &str, headers: &[String], variable: &dyn Fn(&str) -> String) -> Result<IndexMap<String, String>, HostError> {
    todo!()
}

/// `Authorization: Bearer <token>` gives the token alone; nothing for any other header (mcp.ts:391).
pub fn bearer_token(header: &str, value: &str) -> Option<String> {
    todo!()
}

/// Each Codex header: the variable env_http_headers reads it by, the one the vault keeps it under, and that value
/// (mcp.ts:396).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodexHeader {
    pub header: String,
    pub reads: String,
    pub keeps: String,
    pub value: String,
}

pub fn codex_headers(server: &str, headers: &IndexMap<String, String>) -> Result<Vec<CodexHeader>, HostError> {
    todo!()
}

/// The keys of an `oauth` table that hold a secret, spelled any way (mcp.ts:222).
pub fn oauth_secret_keys(oauth: &serde_json::Map<String, Value>) -> Vec<String> {
    todo!()
}

/// Where a definition still holds an OAuth secret, as dotted paths (mcp.ts:226).
pub fn oauth_secrets_in(v: &Value, reference: &dyn Fn(&str) -> bool) -> Vec<String> {
    todo!()
}

/// Whether a reference may read `name`: held outside, written by this file, its bearer's token held either way, or
/// every machine defines it outside an address (mcp.ts:207).
pub fn readable(outside: &IndexSet<String>, writes: &IndexSet<String>, name: &str, in_address: bool) -> bool {
    todo!()
}

/// One whole reference with no default to a readable name (mcp.ts:197).
pub fn refers(v: &str, named: &dyn Fn(&str) -> Option<WholeRef>, held: &dyn Fn(&str) -> bool) -> bool {
    todo!()
}

/// Why a server reading a variable nobody holds travels nowhere (mcp.ts:214).
pub fn unread_line(path: &crate::jsonc_tree::JsonPath) -> String {
    todo!()
}

/// The refusal for a definition no reader maps (mcp.ts:217).
pub fn shape_refusal(path: &crate::jsonc_tree::JsonPath) -> HostError {
    todo!()
}

/// A value servers.env holds and the server it was recorded for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldValue {
    pub value: String,
    pub by: String,
}

/// A value servers.env holds and every server recorded for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownEntry {
    pub value: String,
    pub by: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dropped {
    pub name: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByName {
    pub text: String,
    pub dropped: Vec<Dropped>,
}

/// A file's servers written by name for a copy (mcp.ts:414). `held` takes every value this file kept, on success
/// only; a server that clashes, crosses lines, sets a row's variable or reads what nobody holds is dropped with why.
pub fn servers_by_name(format: McpFormat, text: &str, held: &mut IndexMap<String, HeldValue>, row_of: &dyn Fn(&str) -> Option<String>, known: &IndexMap<String, KnownEntry>) -> Result<ByName, HostError> {
    todo!()
}

/// A reference writer as `servers_by_name` calls it: `(text, only, known, held)`.
pub type Refer<'a> = &'a dyn Fn(&str, Option<&str>, &[KnownValue], &IndexSet<String>) -> Result<McpReferred, HostError>;

/// `servers_by_name` with the format's `refer` replaced, every other method the format's own: the TS tests' stand-in
/// format `{ ...FORMAT, refer }` (mcp-refer.test.ts:277, 405, 647, 688). `servers_by_name` is this with
/// `format.refer`.
pub fn servers_by_name_with(format: McpFormat, refer: Refer<'_>, text: &str, held: &mut IndexMap<String, HeldValue>, row_of: &dyn Fn(&str) -> Option<String>, known: &IndexMap<String, KnownEntry>) -> Result<ByName, HostError> {
    todo!()
}
