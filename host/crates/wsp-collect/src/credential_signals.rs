// SPDX-License-Identifier: AGPL-3.0-only
//! What makes a file look like a credential: its name, its mode, the key names in a JSON, YAML, TOML or KEY=value
//! file, a PEM header, a password in a URL. A value that is a template or environment reference, a placeholder or
//! nothing points at a secret held elsewhere and raises nothing. No value read here leaves these functions. Port of
//! packages/collect/src/credential-signals.ts at a3b870a6e; the spec is spec/credential_signals.md.
use std::future::Future;
use std::sync::LazyLock;

use indexmap::IndexMap;
use regex::{Captures, Regex};
use serde_json::{Map, Value};
use wsp_wire::wire::helpers::js_key_order;
use wsp_words::js::{js_trim, js_trim_start, JS_WS};

pub use wsp_wire::views::project_bundle::CredentialSignal as Signal;

/// The largest file mode and content are believed for, in bytes (credential-signals.ts:13).
pub const MAX_BYTES: u64 = 100 * 1024;
/// How deep key names are read into a JSON object (credential-signals.ts:15).
pub const KEY_DEPTH: usize = 3;

/// A file's size and permission bits, as the caller stat'ed it (TS `{ bytes, mode }`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileShape {
    pub bytes: u64,
    pub mode: u32,
}

const NOT_LINE_END: &str = r"\n\r\x{2028}\x{2029}";

static NAME_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let env = format!(r"^\.env(\.[^{NOT_LINE_END}]+)?$");
    [
        r"^\.?(?i-u:credentials)",
        r"^auth\.json$",
        r"^hosts\.yml$",
        r"(?i-u:access[-_]token)",
        r"\.pem$",
        r"\.key$",
        r"^key$",
        r"^id_[a-z0-9]+$",
        r"^\.?netrc$",
        env.as_str(),
        r"^\.npmrc$",
        r"^\.pypirc$",
        r"^\.git-credentials$",
        r"(^|[._-])(?i-u:tokens?)([._-]|$)",
        r"(^|[._-])(?i-u:secrets?)([._-]|$)",
    ]
    .iter()
    .map(|p| Regex::new(p).expect("a name pattern compiles"))
    .collect()
});
const SKIP_SUFFIXES: &[&str] = &[".example", ".sample", ".template", ".dist", ".pub", ".lock"];
/// Owner-only by habit, never a secret.
const SKIP_NAMES: &[&str] = &[".viminfo", ".CFUserTextEncoding", ".z", "known_hosts", ".envrc"];
const SOURCE_EXTENSIONS: &[&str] = &[
    "go", "ts", "tsx", "js", "mjs", "cjs", "py", "rs", "c", "h", "cc", "cpp", "java", "rb", "md", "html", "css", "svg", "png", "jpg",
    "gif", "ico", "woff", "woff2", "ttf", "zip", "gz", "tar", "wasm", "node", "so", "dylib",
];
/// Shapes worth opening for key names: the structured formats, plus files without an extension (config, token, .boto).
const READABLE_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml", "pem", "key", "env", ""];
/// Mode on its own is only believed for a structured file.
const STRUCTURED_EXTENSIONS: &[&str] = &["json", "yaml", "yml", "toml"];
/// Key names that hold a secret, after `accessToken` and `client-secret` are read as snake case. Bare `key` is left out (a
/// keybinding or a public key far more often) and so is a `_password` suffix (`generate_password` is a command).
const KEY_NAMES: &[&str] = &[
    "token",
    "password",
    "passwd",
    "secret",
    "bearer",
    "auths",
    "credentials",
    "_authtoken",
    "apikey",
    "api_key",
    "private_key",
    "secret_key",
    "access_key",
    "secret_access_key",
];
const KEY_SUFFIXES: &[&str] = &["_token", "_secret", "_api_key", "_apikey", "_private_key", "_secret_access_key"];

/// A value that names where the secret lives instead of holding one: a workflow's `${{ secrets.NAME }}`, a compose or
/// toml `${NAME}` or `$NAME`, an angle-bracket placeholder, or nothing at all.
static REFERENCE_VALUE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(?:\$\{\{[^}]*\}\}|\$\{[^}]*\}|\$[A-Za-z_][A-Za-z0-9_]*|<[^<>]*>)?$").expect("REFERENCE_VALUE compiles")
});
static PEM_HEADER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^-----BEGIN[ A-Z0-9_-]*PRIVATE KEY").expect("PEM_HEADER compiles"));
static BLANK_OR_EMPTY_STRUCTURE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"^[{JS_WS}]*(?:\{{[{JS_WS}]*\}}|\[[{JS_WS}]*\])?[{JS_WS}]*$")).expect("BLANK_OR_EMPTY_STRUCTURE compiles")
});
/// A URL whose userinfo carries a password, `scheme://user:secret@host` or `scheme://:secret@host`; a username alone
/// is a name, not a secret, and a password that is a reference names one rather than holding it.
static URL_CREDENTIAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"(?-u:\b)([a-zA-Z][a-zA-Z0-9+.-]*://)[^{JS_WS}/@:]*:([^{JS_WS}/@]+)@([^{JS_WS}"']+)"#))
        .expect("URL_CREDENTIAL compiles")
});
/// The `key:` or `key =` head of a YAML, TOML, JSONC or KEY=value line, with the rest of the line as group 2.
static KEY_LINE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r#"^[{JS_WS}]*(?://[^{JS_WS}:]+/:)?"?([A-Za-z_][A-Za-z0-9_.-]*)"?[{JS_WS}]*[:=]([^\n]*)$"#))
        .expect("KEY_LINE compiles")
});
static VALUE_COMMENT: LazyLock<Regex> = LazyLock::new(|| Regex::new(&format!(r"[{JS_WS}]+#[^\n]*$")).expect("VALUE_COMMENT compiles"));
static CAMEL_BOUNDARY: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-z0-9])([A-Z])").expect("CAMEL_BOUNDARY compiles"));

/// The lower-cased text after the last dot, or empty when the dot is the first character or there is none.
pub fn extension(name: &str) -> String {
    match name.rfind('.') {
        Some(dot) if dot > 0 => name[dot + 1..].to_lowercase(),
        _ => String::new(),
    }
}

fn skipped_name(name: &str) -> bool {
    SKIP_NAMES.contains(&name)
        || name.ends_with("_history")
        || SKIP_SUFFIXES.iter().any(|s| name.ends_with(s))
        || SOURCE_EXTENSIONS.contains(&extension(name).as_str())
}

/// Whether a file name is secret-shaped by its name alone (credential-signals.ts:54).
pub fn name_signal(name: &str) -> bool {
    !skipped_name(name) && NAME_PATTERNS.iter().any(|p| p.is_match(name))
}

/// Owner-only (no group or other bit), not empty, and at most MAX_BYTES.
pub fn mode_signal(e: FileShape) -> bool {
    e.mode & 0o077 == 0 && e.bytes > 0 && e.bytes <= MAX_BYTES
}

fn is_reference(value: Option<&str>) -> bool {
    value.is_some_and(|v| REFERENCE_VALUE.is_match(v))
}

fn skip_ws(b: &[u8], mut at: usize) -> usize {
    while matches!(b.get(at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
        at += 1;
    }
    at
}

fn json_string(text: &str, at: usize) -> Option<(String, usize)> {
    if text.as_bytes().get(at) != Some(&b'"') {
        return None;
    }
    let mut units: Vec<u16> = Vec::new();
    let mut chars = text[at + 1..].char_indices();
    loop {
        let (i, c) = chars.next()?;
        match c {
            '"' => return Some((String::from_utf16_lossy(&units), at + 1 + i + 1)),
            '\\' => {
                let (_, e) = chars.next()?;
                let unit = match e {
                    '"' | '\\' | '/' => e as u16,
                    'b' => 8,
                    'f' => 12,
                    'n' => 10,
                    'r' => 13,
                    't' => 9,
                    'u' => {
                        let mut v = 0u32;
                        for _ in 0..4 {
                            let (_, h) = chars.next()?;
                            v = v * 16 + h.to_digit(16)?;
                        }
                        v as u16
                    }
                    _ => return None,
                };
                units.push(unit);
            }
            c if (c as u32) < 0x20 => return None,
            c => {
                let mut buf = [0u16; 2];
                units.extend_from_slice(c.encode_utf16(&mut buf));
            }
        }
    }
}

fn json_scalar(b: &[u8], at: usize) -> Option<usize> {
    for word in ["true", "false", "null"] {
        if b[at..].starts_with(word.as_bytes()) {
            return Some(at + word.len());
        }
    }
    let digits = |mut i: usize| {
        let start = i;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        (i > start).then_some(i)
    };
    let mut i = at;
    if b.get(i) == Some(&b'-') {
        i += 1;
    }
    match b.get(i)? {
        b'0' => i += 1,
        b'1'..=b'9' => i = digits(i)?,
        _ => return None,
    }
    if b.get(i) == Some(&b'.') {
        i = digits(i + 1)?;
    }
    if matches!(b.get(i), Some(b'e' | b'E')) {
        i += 1;
        if matches!(b.get(i), Some(b'+' | b'-')) {
            i += 1;
        }
        i = digits(i)?;
    }
    Some(i)
}

fn json_key(text: &str, at: usize) -> Option<(String, usize)> {
    let (key, next) = json_string(text, at)?;
    let colon = skip_ws(text.as_bytes(), next);
    (text.as_bytes().get(colon) == Some(&b':')).then(|| (key, skip_ws(text.as_bytes(), colon + 1)))
}

/// A container being read: an object's pending key, and what it keeps. Only an object at KEY_DEPTH or less keeps its
/// entries; every other container is read to its end and kept as `Null`.
struct Open {
    object: bool,
    keep: bool,
    map: Map<String, Value>,
    key: Option<String>,
}

/// The text as V8's JSON.parse reads it, kept to the levels the key walk reads. Lone surrogate escapes and numbers out
/// of f64 range are fine and nesting has no limit, so a text serde_json refuses still takes the JSON road in the TS.
/// The container stack is explicit, so deep nesting cannot overflow the stack.
fn json_parse(text: &str) -> Option<Value> {
    let b = text.as_bytes();
    let mut stack: Vec<Open> = Vec::new();
    let mut at = skip_ws(b, 0);
    'value: loop {
        let mut value = match *b.get(at)? {
            open @ (b'{' | b'[') => {
                let object = open == b'{';
                let keep = object && stack.len() < KEY_DEPTH;
                let close = if object { b'}' } else { b']' };
                at = skip_ws(b, at + 1);
                if b.get(at) == Some(&close) {
                    at += 1;
                    Value::Null
                } else {
                    let mut frame = Open { object, keep, map: Map::new(), key: None };
                    if object {
                        let (key, next) = json_key(text, at)?;
                        frame.key = Some(key);
                        at = next;
                    }
                    stack.push(frame);
                    continue 'value;
                }
            }
            b'"' => {
                let (s, next) = json_string(text, at)?;
                at = next;
                Value::String(s)
            }
            _ => {
                at = json_scalar(b, at)?;
                Value::Null
            }
        };
        loop {
            let Some(frame) = stack.last_mut() else {
                at = skip_ws(b, at);
                return (at == b.len()).then_some(value);
            };
            let key = frame.key.take();
            if frame.keep {
                frame.map.insert(key?, value);
            }
            let object = frame.object;
            let close = if object { b'}' } else { b']' };
            at = skip_ws(b, at);
            match b.get(at).copied() {
                Some(b',') => {
                    at = skip_ws(b, at + 1);
                    if object {
                        let (key, next) = json_key(text, at)?;
                        frame.key = Some(key);
                        at = next;
                    }
                    continue 'value;
                }
                Some(c) if c == close => {
                    at += 1;
                    let done = stack.pop()?;
                    value = if done.keep { Value::Object(done.map) } else { Value::Null };
                }
                _ => return None,
            }
        }
    }
}

fn json_keys(data: &Value, depth: usize, out: &mut Vec<(String, Option<String>)>) {
    let Value::Object(map) = data else { return };
    if depth > KEY_DEPTH {
        return;
    }
    let mut entries: IndexMap<String, Value> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    js_key_order(&mut entries);
    for (key, v) in entries {
        out.push((key, v.as_str().map(str::to_string)));
        json_keys(&v, depth + 1, out);
    }
}

/// What a line holds after its key: the comment a config file trails the value with dropped, since it sits outside the
/// quotes and only whitespace-then-`#` starts one, so a literal `abc#def` keeps its hash; then the comma a JSONC line
/// ends with, then one layer of quotes.
fn line_value(raw: &str) -> String {
    let cut = VALUE_COMMENT.replace(raw, "");
    let t = js_trim(&cut);
    let t = js_trim(t.strip_suffix(',').unwrap_or(t));
    let inner = unquote(t, '"').or_else(|| unquote(t, '\''));
    js_trim(inner.unwrap_or(t)).to_string()
}

fn unquote(t: &str, quote: char) -> Option<&str> {
    let inner = t.strip_prefix(quote)?.strip_suffix(quote)?;
    (!inner.contains(quote)).then_some(inner)
}

/// Key names with their values, at any depth of a JSON object or from the `key:` / `key =` heads of a YAML, TOML, JSONC or
/// KEY=value file. The value of a line is its text; of a JSON entry, the string it holds (`None` for any other value).
fn key_values(text: &str) -> Vec<(String, Option<String>)> {
    match json_parse(text) {
        Some(data) => {
            let mut out = Vec::new();
            json_keys(&data, 1, &mut out);
            out
        }
        None => text
            .split('\n')
            .filter_map(|line| {
                let m = KEY_LINE.captures(line)?;
                Some((m[1].to_string(), Some(line_value(&m[2]))))
            })
            .collect(),
    }
}

/// The key names those pairs carry, in the order they were read.
pub fn top_level_keys(text: &str) -> Vec<String> {
    key_values(text).into_iter().map(|(key, _)| key).collect()
}

/// `accessToken` reads as `access_token`.
fn normalize_key(k: &str) -> String {
    CAMEL_BOUNDARY.replace_all(k, "${1}_${2}").replace('-', "_").to_lowercase()
}

/// Whether some key names a secret and holds a value that is not a reference to one.
pub fn keys_signal(text: &str) -> bool {
    key_values(text).iter().any(|(key, value)| {
        let n = normalize_key(key);
        (KEY_NAMES.contains(&n.as_str()) || KEY_SUFFIXES.iter().any(|sfx| n.ends_with(sfx))) && !is_reference(value.as_deref())
    })
}

/// Whether the text starts with a PEM private key header, past any leading whitespace.
pub fn pem_signal(text: &str) -> bool {
    PEM_HEADER.is_match(js_trim_start(text))
}

/// The text with every credential URL's userinfo removed, and those URLs as they now read (TS `{ text, urls }`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BareUrls {
    pub text: String,
    pub urls: Vec<String>,
}

/// Whether the text carries a URL with a password in its userinfo.
pub fn url_signal(text: &str) -> bool {
    !bare_urls(text).urls.is_empty()
}

/// The text with every such URL's userinfo removed, and those URLs as they now read.
pub fn bare_urls(text: &str) -> BareUrls {
    let mut urls = Vec::new();
    let bare = URL_CREDENTIAL
        .replace_all(text, |c: &Captures| {
            let (scheme, password, rest) = (&c[1], &c[2], &c[3]);
            if is_reference(Some(password)) {
                return c[0].to_string();
            }
            let kept = format!("{scheme}{rest}");
            urls.push(kept.clone());
            kept
        })
        .into_owned();
    BareUrls { text: bare, urls }
}

/// The signals one file carries, in the order name, mode, keys, pem, url, or `None` when it is not secret-shaped.
/// `read` is awaited at most once, and only for a non-empty file of at most MAX_BYTES with a readable extension.
pub async fn file_signals<F, Fut>(name: &str, e: FileShape, read: F, shallow: bool) -> Option<Vec<Signal>>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Option<String>>,
{
    let ext = extension(name);
    if e.bytes == 0 || skipped_name(name) {
        return None;
    }
    let mut signals = Vec::new();
    if name_signal(name) {
        signals.push(Signal::Name);
    }
    if mode_signal(e) {
        signals.push(Signal::Mode);
    }
    let mut parsed = false;
    if e.bytes <= MAX_BYTES && READABLE_EXTENSIONS.contains(&ext.as_str()) {
        if let Some(text) = read().await {
            if BLANK_OR_EMPTY_STRUCTURE.is_match(&text) {
                return None;
            }
            parsed = !top_level_keys(&text).is_empty();
            if keys_signal(&text) {
                signals.push(Signal::Keys);
            }
            if pem_signal(&text) {
                signals.push(Signal::Pem);
            }
            if url_signal(&text) {
                signals.push(Signal::Url);
            }
        }
    }
    if signals.is_empty() {
        return None;
    }
    if signals == [Signal::Mode] && !(shallow && STRUCTURED_EXTENSIONS.contains(&ext.as_str()) && !parsed) {
        return None;
    }
    Some(signals)
}
