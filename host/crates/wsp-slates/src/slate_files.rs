// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-files.ts at 65f6b89c7. Spec: spec/slate_files.md.
//!
//! The files a slate run reads: the slate's own files its command names, and the scripts in the thread's folder an
//! approval binds by hash, read on this disk or, for a command on the thread's own computer, by that computer's daemon.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::io::{ErrorKind, Write};
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use futures::future::LocalBoxFuture;
use indexmap::IndexMap;
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};
use wsp_words::js::is_js_whitespace;

pub use wsp_wire::types::daemon_contract::{FS_HASH_CAP_BYTES, FS_HASH_FILES_MAX, FS_HASH_PATHS_MAX};
pub use wsp_wire::wire::daemon::FsHashReply;

use crate::pending::slate::{SlateCmdDecl, SlateDoc, SlateRunDecl};

/// What a read of the files a run names found, on this computer or by the daemon of the thread's own (slate-files.ts:51).
pub type HashRead = FsHashReply;

/// The thread's own computer as its daemon reads the files a run names there, under `root`; fails where that daemon
/// cannot say (slate-files.ts:55).
pub type HashOn = std::rc::Rc<dyn Fn(String, Vec<String>) -> LocalBoxFuture<'static, Result<HashRead, String>>>;
pub type PinHashOn = Box<dyn Fn(&str) -> Option<HashOn>>;

/// Why an Always cannot hold to a file a run names (slate-files.ts:61).
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UnpinnedWhy {
    Unread,
    Outside,
    Link,
    Past,
    Special,
    Built,
    Paths,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Unpinned {
    pub path: String,
    pub why: UnpinnedWhy,
}

/// The hashes an approval of a run binds, and the files it names that no hash can bind (slate-files.ts:64). `scripts`
/// is absent when no file was hashed; `unpinned` keeps first-keyed order.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub struct Pin {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scripts: Option<IndexMap<String, String>>,
    pub unpinned: Vec<Unpinned>,
}

const SCRIPT_EXTS: [&str; 24] = [
    "sh", "bash", "zsh", "fish", "ksh", "py", "js", "mjs", "cjs", "ts", "mts", "cts", "jsx", "tsx", "rb", "pl", "pm", "php", "lua", "r",
    "jl", "ps1", "awk", "tcl",
];

const RUNNERS: [&str; 29] = [
    "sh", "bash", "zsh", "dash", "ksh", "fish", "source", ".", "python", "python2", "python3", "node", "nodejs", "deno", "bun", "tsx",
    "ts-node", "ruby", "perl", "php", "lua", "Rscript", "env", "exec", "nohup", "sudo", "time", "nice", "command",
];

const INLINE: [&str; 4] = ["-c", "-e", "--eval", "-m"];

/// How long a read of the hashes waits on the thread's computer (slate-files.ts:223).
pub const HASH_WAIT_MS: u64 = 10_000;

fn joined_cmd(cmd: &SlateCmdDecl) -> String {
    format!("{}\n{}", cmd.cmd, cmd.then.as_deref().unwrap_or(""))
}

fn said_of(decl: &SlateRunDecl) -> String {
    match decl {
        SlateRunDecl::Cmd(c) => joined_cmd(c),
        SlateRunDecl::Tool(t) => format!("\n{}", t.then.as_deref().unwrap_or("")),
        SlateRunDecl::Resource(r) => format!("\n{}", r.then.as_deref().unwrap_or("")),
    }
}

fn slate_dir_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\$\{?SLATE_DIR(?-u:\b)").unwrap())
}

fn slate_dir_out_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\$\{?SLATE_DIR(?-u:\b)\}?").unwrap())
}

/// A file name read where it appears in the text, bounded by the start, the end or a character outside `[A-Za-z0-9._-]`.
/// Only the dots of the name are escaped, as the TS does; a name the regex engine cannot read is matched as text.
fn name_re(name: &str) -> Regex {
    let pattern = format!(r"(^|[^A-Za-z0-9._-]){}($|[^A-Za-z0-9._-])", name.replace('.', r"\."));
    Regex::new(&pattern).unwrap_or_else(|_| Regex::new(&format!(r"(^|[^A-Za-z0-9._-]){}($|[^A-Za-z0-9._-])", regex::escape(name))).unwrap())
}

/// The slate's files a run's cmd or then reads, in `doc.files` order; `None` when it reads none (spec rule 1).
pub fn files_read(doc: Option<&SlateDoc>, decl: &SlateRunDecl) -> Option<IndexMap<String, String>> {
    let doc = doc?;
    let said = said_of(decl);
    let every = slate_dir_re().is_match(&said);
    let used: IndexMap<String, String> = doc
        .files
        .as_ref()?
        .iter()
        .filter(|(name, _)| every || name_re(name).is_match(&said))
        .map(|(name, text)| (name.clone(), text.clone()))
        .collect();
    if used.is_empty() {
        None
    } else {
        Some(used)
    }
}

fn remove_at(path: &Path) -> std::io::Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(path),
        Ok(_) => std::fs::remove_file(path),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// The slate's folder made to hold its document's files and nothing else (spec rule 3). Answers `dir`.
pub fn write_slate_files(dir: &Path, files: &IndexMap<String, String>) -> std::io::Result<PathBuf> {
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let kept = entry.file_name().to_str().is_some_and(|name| files.contains_key(name));
        if !kept {
            remove_at(&entry.path())?;
        }
    }
    for (name, text) in files {
        let path = dir.join(name);
        remove_at(&path)?;
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path)?;
        file.write_all(text.as_bytes())?;
    }
    Ok(dir.to_path_buf())
}

/// A declaration with the files it reads beside it, as its approval key and its sheet take it (slate-files.ts:32).
#[derive(Debug, Clone, PartialEq)]
pub struct WithFiles {
    pub decl: SlateRunDecl,
    pub files: Option<IndexMap<String, String>>,
}

impl WithFiles {
    /// The declaration as JSON with `files` last when there are any: what the key and the sheet read.
    pub fn to_json(&self) -> serde_json::Value {
        let mut value = serde_json::to_value(&self.decl).unwrap();
        if let Some(files) = &self.files {
            let map: serde_json::Map<String, Value> = files.iter().map(|(k, v)| (k.clone(), Value::String(v.clone()))).collect();
            value.as_object_mut().unwrap().insert("files".into(), Value::Object(map));
        }
        value
    }
}

pub fn with_files(doc: Option<&SlateDoc>, decl: &SlateRunDecl) -> WithFiles {
    WithFiles { decl: decl.clone(), files: files_read(doc, decl) }
}

/// The characters a command's words are split on: JS whitespace and the shell's joining marks (slate-files.ts:38).
fn is_word_split(c: char) -> bool {
    is_js_whitespace(c) || "'\"`;|&()<>=,".contains(c)
}

/// The words of a cmd run's cmd or then that read as a path (spec rule 4). A non-cmd declaration names none.
pub fn paths_named(decl: &SlateRunDecl) -> Vec<String> {
    let SlateRunDecl::Cmd(cmd) = decl else {
        return Vec::new();
    };
    joined_cmd(cmd)
        .split(is_word_split)
        .filter(|w| !w.is_empty() && !w.starts_with('-') && !w.contains('$') && !w.contains("://") && w.contains(['.', '/']))
        .map(String::from)
        .collect()
}

/// Node's path.normalize for a posix path: `.` and empty segments dropped, `..` folded, a trailing slash kept.
fn posix_normalize(p: &str) -> String {
    if p.is_empty() {
        return ".".into();
    }
    let abs = p.starts_with('/');
    let mut out: Vec<&str> = Vec::new();
    for seg in p.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                if matches!(out.last(), Some(last) if *last != "..") {
                    out.pop();
                } else if !abs {
                    out.push("..");
                }
            }
            s => out.push(s),
        }
    }
    let mut r = out.join("/");
    if abs {
        r = format!("/{r}");
    }
    if r.is_empty() {
        return if abs { "/".into() } else { ".".into() };
    }
    if p.ends_with('/') && !r.ends_with('/') {
        r.push('/');
    }
    r
}

/// Node's path.resolve(base, rel) with the process folder for a relative base: an absolute `rel` wins, and the result
/// has no trailing slash unless it is the root.
fn node_resolve(base: &str, rel: &str) -> String {
    let abs_base = if base.starts_with('/') {
        base.to_string()
    } else {
        let cwd = std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".into());
        format!("{cwd}/{base}")
    };
    let joined = if rel.starts_with('/') { rel.to_string() } else { format!("{abs_base}/{rel}") };
    let mut n = posix_normalize(&joined);
    if n.len() > 1 && n.ends_with('/') {
        n.pop();
    }
    n
}

/// Each word `paths_named` gives, a relative one joined to the folder the command runs in (spec rule 5).
pub fn words_at(folder: &str, decl: &SlateRunDecl, _there: bool) -> Vec<String> {
    let SlateRunDecl::Cmd(cmd) = decl else {
        return Vec::new();
    };
    let ran = match &cmd.cwd {
        None => folder.to_string(),
        Some(cwd) => node_resolve(folder, cwd),
    };
    paths_named(decl)
        .into_iter()
        .map(|word| {
            if word.starts_with('/') {
                word
            } else if ran == "/" {
                format!("/{word}")
            } else {
                format!("{ran}/{word}")
            }
        })
        .collect()
}

/// A word of a run's command as bash reads it (slate-files.ts:101-141).
#[derive(Debug, Clone)]
struct Word {
    text: String,
    said: String,
    loose: bool,
    run: bool,
}

#[derive(Default)]
struct PartialWord {
    text: String,
    said: String,
    escaped: bool,
}

fn end_word(current: &mut Option<PartialWord>, out: &mut Vec<Word>) {
    let Some(w) = current.take() else {
        return;
    };
    let loose = w.escaped || w.text.chars().any(is_js_whitespace) || slate_dir_out_re().replace_all(&w.said, "").contains('$');
    out.push(Word { text: w.text, said: w.said, loose, run: false });
}

fn is_assignment(text: &str) -> bool {
    let mut chars = text.chars();
    match chars.next() {
        Some(c) if c.is_ascii_alphabetic() || c == '_' => {}
        _ => return false,
    }
    for c in chars {
        if c == '=' {
            return true;
        }
        if !(c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
    }
    false
}

fn runner(word: &str) -> bool {
    let name = word.rsplit('/').next().unwrap_or("");
    RUNNERS.contains(&name) || RUNNERS.contains(&name.trim_end_matches(|c: char| c.is_ascii_digit() || c == '.'))
}

fn is_script(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    match lower.rfind('.') {
        Some(at) => SCRIPT_EXTS.contains(&&lower[at + 1..]),
        None => false,
    }
}

/// The words of a run's cmd and then, each simple command split on ; | & ( ) and new lines outside quotes (spec rule 6).
fn words_of(cmd: &SlateCmdDecl) -> Vec<Word> {
    let text: Vec<char> = joined_cmd(cmd).chars().collect();
    let mut commands: Vec<Vec<Word>> = vec![Vec::new()];
    let mut current: Option<PartialWord> = None;
    let mut quote: Option<char> = None;
    let mut i = 0;
    while i < text.len() {
        let c = text[i];
        if quote.is_none() && matches!(c, ';' | '|' | '&' | '(' | ')' | '\n') {
            end_word(&mut current, commands.last_mut().unwrap());
            commands.push(Vec::new());
        } else if quote.is_none() && is_js_whitespace(c) {
            end_word(&mut current, commands.last_mut().unwrap());
        } else {
            let w = current.get_or_insert_with(PartialWord::default);
            w.said.push(c);
            if quote.is_none() && (c == '\'' || c == '"') {
                quote = Some(c);
            } else if quote == Some(c) {
                quote = None;
            } else if c == '\\' && quote != Some('\'') && i + 1 < text.len() {
                i += 1;
                w.said.push(text[i]);
                w.text.push(text[i]);
                w.escaped = true;
            } else {
                w.text.push(c);
            }
        }
        i += 1;
    }
    end_word(&mut current, commands.last_mut().unwrap());

    for command in commands.iter_mut() {
        let kept: Vec<usize> = (0..command.len()).filter(|&k| !is_assignment(&command[k].text)).collect();
        let mut at = 0;
        let mut inline = false;
        while at < kept.len() && runner(&command[kept[at]].text) {
            at += 1;
            while at < kept.len() && command[kept[at]].text.starts_with('-') {
                inline |= INLINE.contains(&command[kept[at]].text.as_str());
                at += 1;
            }
        }
        if let Some(&k) = kept.get(at) {
            if !inline && (at > 0 || !command[k].text.starts_with('/')) {
                command[k].run = true;
            }
        }
    }
    commands.into_iter().flatten().collect()
}

/// Whether a path is not under the folder (folder with its slash). A link on the way is the read's to name, as outside.
fn beyond(folder: &str, path: &str) -> bool {
    let prefix = if folder.ends_with('/') { folder.to_string() } else { format!("{folder}/") };
    !path.starts_with(&prefix)
}

fn listed(list: &Option<Vec<String>>, full: &str) -> bool {
    list.as_ref().is_some_and(|l| l.iter().any(|p| p == full))
}

/// What an approval of a run binds, given what a read of its files found (slate-files.ts:174-191). `read` is `None` when
/// no read was made at all, which leaves every path it names unpinned.
fn pin_of(folder: &str, decl: &SlateRunDecl, read: Option<&HashRead>, there: bool) -> Pin {
    let SlateRunDecl::Cmd(cmd) = decl else {
        return Pin::default();
    };
    let named = words_of(cmd);
    let run: HashSet<&str> = named.iter().filter(|w| w.run).map(|w| w.text.as_str()).collect();
    let mut unpinned: IndexMap<String, UnpinnedWhy> =
        named.iter().filter(|w| w.loose && (w.run || is_script(&w.text))).map(|w| (w.said.clone(), UnpinnedWhy::Built)).collect();
    let words = paths_named(decl);
    if read.is_none() {
        for path in &words {
            unpinned.insert(path.clone(), UnpinnedWhy::Unread);
        }
    }
    let at = words_at(folder, decl, there);
    let folded = |p: &str| if there { posix_normalize(p) } else { node_resolve(p, "") };
    let folded_folder = folded(folder);
    for (i, path) in words.iter().enumerate() {
        let full = &at[i];
        let script = is_script(path) || run.contains(path.as_str());
        let why = match read {
            None => None,
            Some(r) => {
                if there && i as u64 >= FS_HASH_PATHS_MAX {
                    Some(UnpinnedWhy::Paths)
                } else if !script {
                    None
                } else if listed(&r.past, full) {
                    Some(UnpinnedWhy::Past)
                } else if listed(&r.special, full) {
                    Some(UnpinnedWhy::Special)
                } else if path.starts_with('~') || beyond(&folded_folder, &folded(full)) {
                    Some(UnpinnedWhy::Outside)
                } else if listed(&r.outside, full) {
                    Some(UnpinnedWhy::Link)
                } else {
                    None
                }
            }
        };
        if let Some(why) = why {
            if !unpinned.contains_key(path) {
                unpinned.insert(path.clone(), why);
            }
        }
    }
    let scripts = read.map(|r| r.files.clone()).filter(|files| !files.is_empty());
    Pin { scripts, unpinned: unpinned.into_iter().map(|(path, why)| Unpinned { path, why }).collect() }
}

/// Node's path.relative between two absolute posix paths.
fn relative(from: &Path, to: &Path) -> String {
    let from: Vec<String> = from.to_string_lossy().split('/').filter(|s| !s.is_empty()).map(String::from).collect();
    let to: Vec<String> = to.to_string_lossy().split('/').filter(|s| !s.is_empty()).map(String::from).collect();
    let common = from.iter().zip(to.iter()).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = vec!["..".to_string(); from.len() - common];
    parts.extend(to[common..].iter().cloned());
    parts.join("/")
}

fn empty_read() -> HashRead {
    HashRead { files: IndexMap::new(), outside: None, past: None, special: None }
}

/// Every file a run's cmd or then names, read on this disk (slate-files.ts:197-214). `None` when the folder is not there.
fn read_here(folder: &str, decl: &SlateRunDecl) -> Option<HashRead> {
    let root = std::fs::canonicalize(node_resolve(folder, "")).ok()?;
    let mut read = HashRead { files: IndexMap::new(), outside: Some(Vec::new()), past: Some(Vec::new()), special: Some(Vec::new()) };
    for word in words_at(folder, decl, false) {
        let Ok(at) = std::fs::canonicalize(node_resolve(&word, "")) else {
            continue;
        };
        let Ok(meta) = std::fs::metadata(&at) else {
            continue;
        };
        let rel = relative(&root, &at);
        if meta.is_dir() || read.files.contains_key(&rel) {
            continue;
        }
        if rel.split('/').next() == Some("..") {
            read.outside.get_or_insert_with(Vec::new).push(word);
        } else if !meta.is_file() {
            read.special.get_or_insert_with(Vec::new).push(word);
        } else if meta.len() > FS_HASH_CAP_BYTES || read.files.len() as u64 >= FS_HASH_FILES_MAX {
            read.past.get_or_insert_with(Vec::new).push(word);
        } else if let Ok(bytes) = std::fs::read(&at) {
            let digest = hex::encode(Sha256::digest(&bytes));
            read.files.insert(rel, digest[..16].to_string());
        }
    }
    Some(read)
}

/// What an approval of a run on this computer binds; no folder, a missing folder or a non-cmd run pins nothing.
pub fn pin_here(folder: Option<&str>, decl: &SlateRunDecl) -> Pin {
    let Some(folder) = folder else {
        return Pin::default();
    };
    if !matches!(decl, SlateRunDecl::Cmd(_)) {
        return Pin::default();
    }
    let read = read_here(folder, decl).unwrap_or_else(empty_read);
    pin_of(folder, decl, Some(&read), false)
}

/// One cmd a reading covers.
#[derive(Debug, Clone)]
pub struct PinAsk {
    pub folder: Option<String>,
    pub decl: SlateRunDecl,
}

struct Reading {
    at: u64,
    pins: HashMap<String, HashRead>,
}

/// slate-files.ts:231 boxPins(hashOn, waitMs): the last reading per thread of what each command on a thread's own
/// computer names there (spec rule 15). State is borrowed only between awaits (rule 3.3).
pub struct BoxPins {
    hash_on: PinHashOn,
    wait_ms: u64,
    read: RefCell<HashMap<String, Reading>>,
    reads: Cell<u64>,
}

pub fn box_pins(hash_on: PinHashOn, wait_ms: Option<u64>) -> BoxPins {
    BoxPins { hash_on, wait_ms: wait_ms.unwrap_or(HASH_WAIT_MS), read: RefCell::new(HashMap::new()), reads: Cell::new(0) }
}

fn pin_key(folder: &str, decl: &SlateRunDecl) -> String {
    serde_json::to_string(&serde_json::json!([folder, words_at(folder, decl, true)])).unwrap()
}

/// The hashes cut to their first 16 units, as the held reading keeps them (slate-files.ts:262).
fn cut_hashes(mut r: HashRead) -> HashRead {
    for sum in r.files.values_mut() {
        *sum = String::from_utf16_lossy(&sum.encode_utf16().take(16).collect::<Vec<u16>>());
    }
    r
}

impl BoxPins {
    /// Reads the hashes again for these commands; `asleep` asks nothing, so a napping computer is not woken.
    pub async fn read(&self, thread_id: &str, cmds: &[PinAsk], asleep: bool) {
        let at = self.reads.get() + 1;
        self.reads.set(at);
        let on: Option<HashOn> = if asleep { None } else { (self.hash_on)(thread_id) };
        let mut asks: IndexMap<String, (String, Vec<String>)> = IndexMap::new();
        for ask in cmds {
            let (Some(folder), SlateRunDecl::Cmd(_)) = (&ask.folder, &ask.decl) else {
                continue;
            };
            if paths_named(&ask.decl).is_empty() {
                continue;
            }
            let paths: Vec<String> = words_at(folder, &ask.decl, true).into_iter().take(FS_HASH_PATHS_MAX as usize).collect();
            asks.insert(pin_key(folder, &ask.decl), (folder.clone(), paths));
        }
        if asks.is_empty() {
            self.read.borrow_mut().remove(thread_id);
            return;
        }
        let wait = Duration::from_millis(self.wait_ms);
        let answers = asks.into_iter().map(|(key, (folder, paths))| {
            let on = on.clone();
            async move {
                let on = on?;
                match tokio::time::timeout(wait, on(folder, paths)).await {
                    Ok(Ok(reply)) => Some((key, cut_hashes(reply))),
                    _ => None,
                }
            }
        });
        let pins: HashMap<String, HashRead> = futures::future::join_all(answers).await.into_iter().flatten().collect();
        let mut held = self.read.borrow_mut();
        if held.get(thread_id).map_or(0, |r| r.at) < at {
            held.insert(thread_id.to_string(), Reading { at, pins });
        }
    }

    /// What an approval of this command binds, from the last reading; one that names no path needs none.
    pub fn pin(&self, thread_id: &str, folder: Option<&str>, decl: &SlateRunDecl) -> Pin {
        let (Some(folder), SlateRunDecl::Cmd(_)) = (folder, decl) else {
            return Pin::default();
        };
        let read = if paths_named(decl).is_empty() {
            Some(empty_read())
        } else {
            let key = pin_key(folder, decl);
            let found = self.read.borrow().get(thread_id).and_then(|r| r.pins.get(&key).cloned());
            found
        };
        pin_of(folder, decl, read.as_ref(), true)
    }

    pub fn drop_thread(&self, thread_id: &str) {
        self.read.borrow_mut().remove(thread_id);
    }
}

/// The phrase a sheet line uses for a file no hash binds; `None` for an unread file, which the sheet does not name.
fn why_phrase(why: UnpinnedWhy) -> Option<&'static str> {
    match why {
        UnpinnedWhy::Unread => None,
        UnpinnedWhy::Outside => Some("is outside the thread's folder"),
        UnpinnedWhy::Link => Some("is a link out of the thread's folder"),
        UnpinnedWhy::Past => Some("is past what one command can pin (32 files, each up to 4 MiB)"),
        UnpinnedWhy::Special => Some("is not a regular file"),
        UnpinnedWhy::Built => Some("is a script wsp cannot read by name (a space, an escape or a $ in it)"),
        UnpinnedWhy::Paths => Some("is past the 256 paths one command can name"),
    }
}

fn why_key(why: UnpinnedWhy) -> &'static str {
    match why {
        UnpinnedWhy::Unread => "unread",
        UnpinnedWhy::Outside => "outside",
        UnpinnedWhy::Link => "link",
        UnpinnedWhy::Past => "past",
        UnpinnedWhy::Special => "special",
        UnpinnedWhy::Built => "built",
        UnpinnedWhy::Paths => "paths",
    }
}

/// Each script a pin cannot bind and why, joined as the sheet and a refused Always say (slate-files.ts:79).
fn why_said(pin: &Pin) -> Option<String> {
    let said: Vec<String> = pin.unpinned.iter().filter_map(|u| why_phrase(u.why).map(|p| format!("{} {}", u.path, p))).collect();
    if said.is_empty() {
        None
    } else {
        Some(said.join(" and "))
    }
}

/// The hashes a pin puts in an approval key, each unpinned file standing there by its why (spec rule 10).
pub fn keyed(pin: &Pin) -> Option<IndexMap<String, String>> {
    if pin.unpinned.is_empty() {
        return pin.scripts.clone();
    }
    let mut map = pin.scripts.clone().unwrap_or_default();
    for u in &pin.unpinned {
        map.insert(u.path.clone(), why_key(u.why).to_string());
    }
    Some(map)
}

pub fn cannot_pin(pin: &Pin) -> Option<String> {
    let why = why_said(pin)?;
    let it = if pin.unpinned.len() == 1 { "it" } else { "them" };
    Some(format!("{why}, so wsp cannot pin {it} and it asks every time"))
}

pub fn always_refused(run: &str, pin: &Pin) -> Option<String> {
    if pin.unpinned.is_empty() {
        return None;
    }
    match why_said(pin) {
        Some(why) => Some(format!("${run} names a script wsp cannot pin to hold an Always to: {why}.")),
        None => {
            let paths: Vec<&str> = pin.unpinned.iter().map(|u| u.path.as_str()).collect();
            Some(format!(
                "${run} runs on the thread's machine and names {} there, which its computer could not hash to hold an Always to.",
                paths.join(", ")
            ))
        }
    }
}
