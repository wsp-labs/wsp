// SPDX-License-Identifier: AGPL-3.0-only
//! One row per thing the collector found on the computer that could be brought to an image. wsp init reads it, the
//! person ticks rows, and the same shape with `bring` and `choice` filled in is saved as the recipe file. Port of
//! packages/collect/src/manifest.ts at a3b870a6e; the spec is spec/manifest.md.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use wsp_wire::types::init_job::LoginChoice;
use wsp_wire::types::recipe_file::ServerSignIn;
use wsp_wire::views::golden_image::ToolPin;
use wsp_words::exit::HostError;
use wsp_words::words::thread::{issues_line, Issue, PathPart};

pub use wsp_wire::types::init_job::LOGIN_CHOICES;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Rung {
    Identity,
    Shell,
    Toolchains,
    Tools,
    Agents,
    Logins,
}

/// The ladder, in the order collect() runs it and a manifest lists it.
pub const RUNGS: [Rung; 6] = [Rung::Identity, Rung::Shell, Rung::Toolchains, Rung::Tools, Rung::Agents, Rung::Logins];

impl Rung {
    /// The word the schema and an id prefix use: `identity`, `shell`, ...
    pub fn as_str(self) -> &'static str {
        match self {
            Rung::Identity => "identity",
            Rung::Shell => "shell",
            Rung::Toolchains => "toolchains",
            Rung::Tools => "tools",
            Rung::Agents => "agents",
            Rung::Logins => "logins",
        }
    }
}

/// TS `Default` (manifest.ts:15), renamed: the TS name shadows the prelude's `Default` trait.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowDefault {
    Bring,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Linux {
    Yes,
    No,
    Unknown,
}

pub const LINUX: [Linux; 3] = [Linux::Yes, Linux::No, Linux::Unknown];

/// manifest.ts:23-69. Fields in the schema's key order, which is the order zod rebuilds a parsed row in, so the order
/// collect() answers and the recipe file saves; `arch` is the one key the schema lacks (detect/common.ts:77 writes it,
/// parse_manifest drops it as zod's strip does).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEntry {
    pub rung: Rung,
    /// `<rung>/<name>`; the last segment looks up install and sign-in commands.
    pub id: String,
    pub label: String,
    /// Source paths on the computer, `~`-relative; empty for a list item.
    pub paths: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excludes: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub volatile: Option<Vec<String>>,
    pub bytes: u64,
    pub default: RowDefault,
    /// Why the default is skip; with a reason the row renders locked off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bring: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub choice: Option<LoginChoice>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub linux: Option<Linux>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    /// Not in the schema: a detector's row may carry it, a parsed row never does.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin: Option<ToolPin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consent: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secrets: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub font: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sources: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_in: Option<ServerSignIn>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keys: Option<Vec<String>>,
}

impl ManifestEntry {
    /// A row with only the required fields set and every optional absent, `default` bring.
    pub fn new(rung: Rung, id: impl Into<String>, label: impl Into<String>, paths: Vec<String>, bytes: u64) -> ManifestEntry {
        ManifestEntry {
            rung,
            id: id.into(),
            label: label.into(),
            paths,
            excludes: None,
            volatile: None,
            bytes,
            default: RowDefault::Bring,
            reason: None,
            group: None,
            required: None,
            bring: None,
            choice: None,
            linux: None,
            version: None,
            arch: None,
            pin: None,
            consent: None,
            detail: None,
            secrets: None,
            login: None,
            font: None,
            sources: None,
            sign_in: None,
            keys: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Manifest {
    pub entries: Vec<ManifestEntry>,
}

/// One zod issue as manifest.ts's schemas raise it: the path (keys as strings, indexes as numbers) and zod's message.
#[derive(Debug, Clone, PartialEq)]
pub struct ManifestIssue {
    pub path: Vec<Value>,
    pub message: String,
}

/// How far a value got before zod stopped with it: a type error aborts its object, a short string or a bad number only
/// dirties it, and the refine runs on anything not aborted.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Status {
    Valid,
    Dirty,
    Aborted,
}

type Field = (&'static str, bool, Shape);

#[derive(Clone, Copy)]
enum Shape {
    Str { min1: bool },
    StrList { min1: bool },
    Bool,
    Bytes,
    Enum(&'static [&'static str]),
    Literal,
    Object(&'static [Field]),
}

const RUNG_WORDS: &[&str] = &["identity", "shell", "toolchains", "tools", "agents", "logins"];

const PIN: &[Field] = &[
    ("tag", true, Shape::Str { min1: true }),
    ("sha256", false, Shape::Str { min1: true }),
    ("arch", false, Shape::Enum(&["x86_64", "aarch64"])),
    ("latest", false, Shape::Literal),
];

/// manifest.ts:23-69 in zod's key order; `arch` is not here, so zod strips it.
const ROW: &[Field] = &[
    ("rung", true, Shape::Enum(RUNG_WORDS)),
    ("id", true, Shape::Str { min1: true }),
    ("label", true, Shape::Str { min1: false }),
    ("paths", true, Shape::StrList { min1: false }),
    ("excludes", false, Shape::StrList { min1: false }),
    ("volatile", false, Shape::StrList { min1: false }),
    ("bytes", true, Shape::Bytes),
    ("default", true, Shape::Enum(&["bring", "skip"])),
    ("reason", false, Shape::Str { min1: false }),
    ("group", false, Shape::Str { min1: false }),
    ("required", false, Shape::Bool),
    ("bring", false, Shape::Bool),
    ("choice", false, Shape::Enum(&["copy", "machine", "later", "key", "skip", "token"])),
    ("linux", false, Shape::Enum(&["yes", "no", "unknown"])),
    ("version", false, Shape::Str { min1: true }),
    ("pin", false, Shape::Object(PIN)),
    ("consent", false, Shape::Bool),
    ("detail", false, Shape::Str { min1: false }),
    ("secrets", false, Shape::StrList { min1: false }),
    ("login", false, Shape::Str { min1: true }),
    ("font", false, Shape::Str { min1: true }),
    ("sources", false, Shape::StrList { min1: true }),
    ("signIn", false, Shape::Enum(&["none", "key", "token", "oauth"])),
    ("keys", false, Shape::StrList { min1: true }),
];

fn at(path: &[Value], part: Value) -> Vec<Value> {
    let mut next = path.to_vec();
    next.push(part);
    next
}

fn issue(out: &mut Vec<ManifestIssue>, path: &[Value], message: impl Into<String>) {
    out.push(ManifestIssue { path: path.to_vec(), message: message.into() });
}

fn received(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

fn expected(out: &mut Vec<ManifestIssue>, path: &[Value], want: &str, v: &Value) -> Status {
    issue(out, path, format!("Expected {want}, received {}", received(v)));
    Status::Aborted
}

fn quoted(words: &[&str]) -> String {
    words.iter().map(|w| format!("'{w}'")).collect::<Vec<_>>().join(" | ")
}

fn text(v: &Value, path: &[Value], min1: bool, out: &mut Vec<ManifestIssue>) -> Status {
    match v.as_str() {
        None => expected(out, path, "string", v),
        Some(s) if min1 && s.is_empty() => {
            issue(out, path, "String must contain at least 1 character(s)");
            Status::Dirty
        }
        Some(_) => Status::Valid,
    }
}

fn number(v: &Value, path: &[Value], out: &mut Vec<ManifestIssue>) -> Status {
    let Some(n) = v.as_number() else {
        return expected(out, path, "number", v);
    };
    let x = n.as_f64().unwrap_or(f64::NAN);
    let mut st = Status::Valid;
    if n.is_f64() && x.fract() != 0.0 {
        issue(out, path, "Expected integer, received float");
        st = Status::Dirty;
    }
    if x < 0.0 {
        issue(out, path, "Number must be greater than or equal to 0");
        st = Status::Dirty;
    }
    st
}

fn check(shape: &Shape, v: &Value, path: &[Value], out: &mut Vec<ManifestIssue>) -> Status {
    match shape {
        Shape::Str { min1 } => text(v, path, *min1, out),
        Shape::StrList { min1 } => match v.as_array() {
            None => expected(out, path, "array", v),
            Some(items) => {
                items.iter().enumerate().fold(Status::Valid, |st, (i, item)| st.max(text(item, &at(path, json!(i)), *min1, out)))
            }
        },
        Shape::Bool if v.is_boolean() => Status::Valid,
        Shape::Bool => expected(out, path, "boolean", v),
        Shape::Bytes => number(v, path, out),
        Shape::Enum(words) => match v.as_str() {
            Some(s) if words.contains(&s) => Status::Valid,
            Some(s) => {
                issue(out, path, format!("Invalid enum value. Expected {}, received '{s}'", quoted(words)));
                Status::Aborted
            }
            None => expected(out, path, &quoted(words), v),
        },
        Shape::Literal if v == &Value::Bool(true) => Status::Valid,
        Shape::Literal => {
            issue(out, path, "Invalid literal value, expected true");
            Status::Aborted
        }
        Shape::Object(fields) => object(v, path, fields, out),
    }
}

fn object(v: &Value, path: &[Value], fields: &[Field], out: &mut Vec<ManifestIssue>) -> Status {
    let Some(obj) = v.as_object() else {
        return expected(out, path, "object", v);
    };
    fields.iter().fold(Status::Valid, |st, (key, required, shape)| {
        let p = at(path, json!(key));
        let got = match obj.get(*key) {
            Some(value) => check(shape, value, &p, out),
            None if *required => {
                issue(out, &p, "Required");
                Status::Aborted
            }
            None => Status::Valid,
        };
        st.max(got)
    })
}

/// manifest.ts:71-96: the refine that runs once the keys are checked, not aborted.
fn refine(data: &Value, path: &[Value], out: &mut Vec<ManifestIssue>) -> Status {
    let Some(obj) = data.as_object() else {
        return Status::Valid;
    };
    let rung = obj.get("rung").and_then(Value::as_str).unwrap_or_default();
    let id = obj.get("id").and_then(Value::as_str).unwrap_or_default();
    let consent_true = obj.get("consent") == Some(&Value::Bool(true));
    let mut rules: Vec<(&str, String)> = Vec::new();
    if !id.starts_with(&format!("{rung}/")) {
        rules.push(("id", format!("id must start with {rung}/")));
    }
    if obj.contains_key("choice") && rung != "logins" && !consent_true {
        rules.push(("choice", "only a logins row or a consent row carries a choice".into()));
    }
    if obj.contains_key("linux") && rung != "tools" {
        rules.push(("linux", "only a tools row carries a linux marker".into()));
    }
    if obj.contains_key("version") && rung != "tools" {
        rules.push(("version", "only a tools row carries a version".into()));
    }
    if obj.contains_key("login") && rung != "shell" {
        rules.push(("login", "only a shell row carries a login shell".into()));
    }
    if obj.contains_key("font") && rung != "shell" {
        rules.push(("font", "only a shell row carries a terminal font".into()));
    }
    if obj.contains_key("sources") && rung != "shell" {
        rules.push(("sources", "only a shell row carries sourced files".into()));
    }
    if obj.get("required") == Some(&Value::Bool(true)) && obj.get("default").and_then(Value::as_str) == Some("skip") {
        rules.push(("required", "a required row cannot default to skip".into()));
    }
    let st = if rules.is_empty() { Status::Valid } else { Status::Dirty };
    for (key, message) in rules {
        issue(out, &at(path, json!(key)), message);
    }
    st
}

fn check_row(data: &Value, path: &[Value], out: &mut Vec<ManifestIssue>) -> Status {
    let st = object(data, path, ROW, out);
    if st == Status::Aborted {
        return st;
    }
    st.max(refine(data, path, out))
}

/// The parsed row for a value that passed every check: zod's output, with `arch` stripped and `bytes` as an integer.
fn build_row(data: &Value) -> ManifestEntry {
    let mut row = data.as_object().cloned().unwrap_or_default();
    row.remove("arch");
    let bytes = data.get("bytes").map_or(0, |v| v.as_u64().unwrap_or_else(|| v.as_f64().unwrap_or(0.0) as u64));
    row.insert("bytes".into(), json!(bytes));
    serde_json::from_value(Value::Object(row)).expect("a row that passed every check deserializes")
}

/// `ManifestEntry.safeParse(data)` (manifest.ts:71-96): the row, or every issue zod raises for it, in zod's order
/// (spec/manifest.md, rules 3 to 6).
pub fn parse_entry(data: &Value) -> Result<ManifestEntry, Vec<ManifestIssue>> {
    let mut out = Vec::new();
    check_row(data, &[], &mut out);
    if out.is_empty() {
        Ok(build_row(data))
    } else {
        Err(out)
    }
}

/// `Manifest.safeParse(data)` (manifest.ts:99-107): the envelope, every row and the duplicate-id check.
pub fn safe_parse_manifest(data: &Value) -> Result<Manifest, Vec<ManifestIssue>> {
    let mut out = Vec::new();
    let Some(obj) = data.as_object() else {
        issue(&mut out, &[], format!("Expected object, received {}", received(data)));
        return Err(out);
    };
    let entries_path = [json!("entries")];
    let Some(list) = obj.get("entries") else {
        issue(&mut out, &entries_path, "Required");
        return Err(out);
    };
    let Some(rows) = list.as_array() else {
        issue(&mut out, &entries_path, format!("Expected array, received {}", received(list)));
        return Err(out);
    };
    let mut st = Status::Valid;
    for (i, row) in rows.iter().enumerate() {
        st = st.max(check_row(row, &at(&entries_path, json!(i)), &mut out));
    }
    if st != Status::Aborted {
        let mut seen = HashSet::new();
        for (i, row) in rows.iter().enumerate() {
            let id = row.get("id").and_then(Value::as_str).unwrap_or_default();
            if !seen.insert(id) {
                issue(&mut out, &at(&at(&entries_path, json!(i)), json!("id")), format!("duplicate id {id}"));
            }
        }
    }
    if out.is_empty() {
        Ok(Manifest { entries: rows.iter().map(build_row).collect() })
    } else {
        Err(out)
    }
}

/// Parses unknown data into a manifest; the error is `invalid manifest: ` and every issue as `issuesLine` writes it,
/// `entries.3.rung: <message>` joined by `; ` (manifest.ts:111).
pub fn parse_manifest(data: &Value) -> Result<Manifest, HostError> {
    safe_parse_manifest(data).map_err(|issues| {
        let lines: Vec<Issue> = issues
            .into_iter()
            .map(|i| Issue {
                path: i
                    .path
                    .iter()
                    .map(|p| match p {
                        Value::String(key) => PathPart::Key(key.clone()),
                        other => PathPart::Index(other.as_f64().unwrap_or_default()),
                    })
                    .collect(),
                message: i.message,
            })
            .collect();
        HostError::new(format!("invalid manifest: {}", issues_line(&lines)))
    })
}
