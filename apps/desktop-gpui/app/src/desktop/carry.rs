// SPDX-License-Identifier: AGPL-3.0-only
//! What today's app kept in its page's local storage, carried once into a store that has never been written: the
//! composer's drafts and queues, the prompt stash, favourites and fast mode, and the creations whose first message
//! lives nowhere else (keptCreations.ts). Chromium keeps local storage as LevelDB in the folder Electron's userData
//! names beside the state file (`userDataIn`). That folder is only read: its files are copied into an in-memory
//! LevelDB and read there, so nothing, not even a lock or a recovered log, is written beside today's app.

use std::collections::BTreeSet;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use chrono::{DateTime, SecondsFormat};
use gpui_kit::{App, BorrowAppContext as _};
use rusty_leveldb::{LdbIterator as _, Options, DB};
use serde_json::{json, Map, Value};

use crate::local_store::LocalStore;
use crate::screens::composer::model::STASH_MAX;
use crate::screens::composer::store::{self, DRAFTS_KEY, FAVOURITES_KEY, MODES_KEY, STASH_KEY};

/// Today's creations kept until their workspace is up, by the state file whose host they were asked on.
const CREATIONS_KEY: &str = "wsp:creations:v1";
const KEYS: [&str; 5] = [DRAFTS_KEY, STASH_KEY, FAVOURITES_KEY, MODES_KEY, CREATIONS_KEY];

/// How long a launch waits on today's app to quit before it reads what that app had written so far.
const HAND_OVER: Duration = Duration::from_secs(5);

/// `Local Storage/leveldb` in `APP_DATA_FOLDER` beside `state`, where today's app on that state keeps its pages' storage.
pub fn folder_of(state: &Path) -> PathBuf {
    state.parent().unwrap_or(Path::new("")).join("desktop/Local Storage/leveldb")
}

/// Whether today's app on `state` runs: Chromium's `SingletonLock` in its profile names `<host>-<pid>` until it quits.
fn todays_app_runs(state: &Path) -> bool {
    let lock = fs::read_link(state.parent().unwrap_or(Path::new("")).join("desktop/SingletonLock"));
    lock.ok().and_then(|to| to.to_str()?.rsplit('-').next()?.parse().ok()).is_some_and(crate::update::swap::running)
}

/// Carries today's storage for `state` into the app's store where the store's file was never written.
pub fn on_launch(state: &Path, cx: &mut App) {
    let folder = folder_of(state);
    if cx.global::<LocalStore>().path().exists() || !folder.join("CURRENT").exists() {
        return;
    }
    // Today's app hands over by opening this one and then quitting, and Chromium writes its newest storage as it quits.
    let until = Instant::now() + HAND_OVER;
    while todays_app_runs(state) && Instant::now() < until {
        std::thread::sleep(Duration::from_millis(50));
    }
    let state = std::path::absolute(state).unwrap_or_else(|_| state.to_owned());
    match cx.update_global(|store: &mut LocalStore, _| carry(store, &folder, &state)) {
        Ok(()) => super::log::info(&format!("read today's local storage at {} into a new store", folder.display())),
        Err(e) => super::log::error(&format!("today's local storage at {} could not be carried: {e}", folder.display())),
    }
}

/// Reads `folder` and writes what it held for the composer into `store`, every origin's values joined, the first
/// origin's winning where two hold one thread. A kept creation of `state`'s becomes its workspace's draft, or the
/// first stash entry where that workspace already has a draft, or the create had no workspace yet or was refused.
pub fn carry(store: &mut LocalStore, folder: &Path, state: &Path) -> io::Result<()> {
    // The reader indexes into files as Chromium left them, and one torn by a crash must not stop every launch after it.
    let values = std::panic::catch_unwind(|| read(folder)).map_err(|_| io::Error::other("the LevelDB reader panicked"))??;
    let parsed =
        |key: &'static str| values.iter().filter(move |(k, _)| k == key).filter_map(|(_, v)| serde_json::from_str::<Value>(v).ok());
    let states = |key: &'static str| parsed(key).filter_map(|v| v.get("state").and_then(Value::as_object).cloned());
    let object = |state: &Map<String, Value>, field: &str| state.get(field).and_then(Value::as_object).cloned().unwrap_or_default();

    let (mut drafts, mut queues) = (Map::new(), Map::new());
    for state in states(DRAFTS_KEY) {
        // A draft an earlier build kept under its workspace alone goes to that workspace's composer.
        for (key, draft) in object(&state, "drafts").into_iter().chain(object(&state, "unclaimed")) {
            let Some(prompt) = draft.get("prompt").and_then(Value::as_str).filter(|p| !p.is_empty()) else { continue };
            // Today's caret counts a chip as one character where these words spell it out, so it goes to their end.
            drafts.entry(key).or_insert_with(|| json!({ "prompt": prompt, "cursor": prompt.len() }));
        }
        // Today's rows have string ids and this app's numbers, which only order one queue's rows.
        for (key, rows) in object(&state, "queues") {
            let prompts = rows.as_array().into_iter().flatten().filter_map(|r| r.get("prompt")?.as_str());
            let rows: Vec<Value> = prompts.enumerate().map(|(i, prompt)| json!({ "id": (i + 1).to_string(), "prompt": prompt })).collect();
            if !rows.is_empty() {
                queues.entry(key).or_insert(Value::Array(rows));
            }
        }
    }
    if !drafts.is_empty() || !queues.is_empty() {
        store.set(DRAFTS_KEY, json!({ "state": { "drafts": drafts, "queues": queues }, "version": 2 }))?;
    }

    let mut favourites: Vec<Value> = Vec::new();
    for key in states(FAVOURITES_KEY).filter_map(|s| s.get("keys").and_then(Value::as_array).cloned()).flatten() {
        if !favourites.contains(&key) {
            favourites.push(key);
        }
    }
    if !favourites.is_empty() {
        store.set(FAVOURITES_KEY, json!({ "state": { "keys": favourites }, "version": 1 }))?;
    }

    let mut fast = Map::new();
    for state in states(MODES_KEY) {
        for (key, on) in object(&state, "fast").into_iter().filter(|(_, on)| on.is_boolean()) {
            fast.entry(key).or_insert(on);
        }
    }
    if !fast.is_empty() {
        store.set(MODES_KEY, json!({ "state": { "fast": fast }, "version": 0 }))?;
    }

    // A creation's message lives nowhere else, so it goes ahead of every stash entry, and is the last the quota drops.
    let (mut seen, mut kept) = (BTreeSet::new(), Vec::new());
    for creations in parsed(CREATIONS_KEY) {
        // A page whose host names no state file to it, one bound beyond this computer, keeps its rows under "".
        let mine = creations.as_object().into_iter().flatten().filter(|(file, _)| file.is_empty() || Path::new(file) == state);
        for row in mine.filter_map(|(_, rows)| rows.as_array()).flatten() {
            let text = |field: &str| row.get(field).and_then(Value::as_str).filter(|t| !t.is_empty());
            let (Some(key), Some(asked)) = (text("key"), text("asked")) else { continue };
            if !seen.insert(key.to_owned()) {
                continue;
            }
            let refused = row.get("failed").is_some_and(|f| !f.is_null());
            let workspace = text("workspaceId").filter(|workspace| !refused && store::draft(store, workspace).is_none());
            if workspace.is_some_and(|workspace| store::set_draft(store, workspace, asked, asked.len()).is_ok()) {
                continue;
            }
            let at = row.get("askedAt").and_then(Value::as_i64).and_then(DateTime::from_timestamp_millis).unwrap_or_default();
            let at = at.to_rfc3339_opts(SecondsFormat::Millis, true);
            kept.push(json!({ "id": key, "createdAt": at, "prompt": asked, "files": [], "dropped": [] }));
        }
    }
    let mut entries: Vec<Value> = parsed(STASH_KEY).filter_map(|v| v.as_array().cloned()).flatten().collect();
    entries.sort_by(|a, b| b.get("createdAt").and_then(Value::as_str).cmp(&a.get("createdAt").and_then(Value::as_str)));
    entries.truncate(STASH_MAX.saturating_sub(kept.len()));
    entries.splice(0..0, kept);
    while !entries.is_empty() {
        match store.set(STASH_KEY, Value::Array(entries.clone())) {
            Err(e) if e.kind() == io::ErrorKind::StorageFull => _ = entries.pop(),
            done => return done,
        }
    }
    Ok(())
}

/// Each value under one of `KEYS` in Chromium's local storage at `folder`, by key, every origin's in LevelDB's order.
fn read(folder: &Path) -> io::Result<Vec<(String, String)>> {
    let options = Options { create_if_missing: false, ..Options::default() };
    let memory = Path::new("leveldb");
    for entry in fs::read_dir(folder)? {
        let entry = entry?;
        let name = entry.file_name();
        // LOCK is held by a running Chromium and LOG is its text log; neither holds data.
        if name == "LOCK" || name.to_string_lossy().starts_with("LOG") || !entry.file_type()?.is_file() {
            continue;
        }
        options.env.open_writable_file(&memory.join(&name)).map_err(io::Error::other)?.write_all(&fs::read(entry.path())?)?;
    }
    let mut db = DB::open(memory, options).map_err(io::Error::other)?;
    let mut entries = db.new_iter().map_err(io::Error::other)?;
    let mut out = Vec::new();
    while let Some((key, value)) = entries.next() {
        // `_`, the page's origin, a 0 byte, then the script's key.
        let Some(key) = key.strip_prefix(b"_").and_then(|k| Some(&k[k.iter().position(|&b| b == 0)? + 1..])) else { continue };
        if let (Some(key), Some(value)) = (decoded(key), decoded(&value)) {
            if KEYS.contains(&key.as_str()) {
                out.push((key, value));
            }
        }
    }
    Ok(out)
}

/// A string as Chromium's local storage writes one: a 0 byte then UTF-16LE, or a 1 byte then Latin-1.
fn decoded(bytes: &[u8]) -> Option<String> {
    match bytes.split_first()? {
        (0, rest) => Some(String::from_utf16_lossy(&rest.as_chunks::<2>().0.iter().map(|&c| u16::from_le_bytes(c)).collect::<Vec<_>>())),
        (1, rest) => Some(rest.iter().copied().map(char::from).collect()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_store::scratch;
    use crate::screens::composer::store::{draft, fast, favourites, queue, stash};

    /// Written by real Chromium through today-local-storage.mjs, as today's app's page writes it.
    fn recorded() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("record/today-local-storage")
    }

    fn files(folder: &Path) -> Vec<(PathBuf, Vec<u8>)> {
        let mut all: Vec<_> =
            fs::read_dir(folder).unwrap().map(|e| e.unwrap().path()).map(|p| (p.clone(), fs::read(&p).unwrap())).collect();
        all.sort();
        all
    }

    #[test]
    fn todays_drafts_queues_stash_and_kept_creations_reach_an_empty_store() {
        let path = scratch("carry");
        let before = files(&recorded());
        let mut store = LocalStore::open(&path);
        carry(&mut store, &recorded(), Path::new("/home/person/.wsp/state.json")).unwrap();
        assert_eq!(files(&recorded()), before, "today's folder is only read");

        let store = LocalStore::open(&path);
        assert_eq!(draft(&store, "thr_ascii"), Some(("half a thought".to_owned(), "half a thought".len())));
        assert_eq!(draft(&store, "thr_wide"), Some(("naïve 日本 🙂 end".to_owned(), "naïve 日本 🙂 end".len())));
        assert_eq!(draft(&store, "ws_old"), Some(("from an earlier build".to_owned(), "from an earlier build".len())));
        assert_eq!(draft(&store, "thr_other_host").unwrap().0, "typed on another host");
        let queued = queue(&store, "thr_ascii");
        assert!(queued.held, "a carried queue waits for the person, as a reloaded page's does");
        assert_eq!(queued.rows.iter().map(|r| r.prompt.as_str()).collect::<Vec<_>>(), ["queued first", "queued second"]);
        assert_eq!(favourites(&store), ["claude:opus", "codex:gpt-5"], "the value written last, over the one in the table");
        assert_eq!(fast(&store, "thr_ascii"), Some(true));

        assert_eq!(draft(&store, "ws_new"), Some(("first message for ws_new".to_owned(), "first message for ws_new".len())));
        assert_eq!(draft(&store, "ws_remote").unwrap().0, "asked on a host somewhere else");
        assert_eq!(draft(&store, "ws_other"), None, "another state file's creations stay where they are");
        assert_eq!(draft(&store, "ws_refused"), None, "a refused create's workspace never came up");
        let stash = stash(&store);
        let prompts: Vec<&str> = stash.entries.iter().map(|e| e.prompt.as_str()).collect();
        assert_eq!(
            prompts,
            [
                "asked before the workspace had an id",
                "asked where a draft waits",
                "asked of a create that was refused",
                "the long notes",
                "saved for later"
            ]
        );
        let notes = &stash.entries[3].files[0];
        assert_eq!((notes.name.as_str(), notes.bytes.len()), ("notes.txt", 1_692_000));
        assert!(notes.bytes.starts_with(b"a page of notes, the same line again and again\n"));
        assert_eq!(stash.entries[4].files[0].bytes.as_slice(), b"hello");
    }

    #[test]
    fn a_torn_table_is_an_error_and_writes_nothing() {
        let folder = scratch("carry-torn").with_file_name("leveldb");
        fs::create_dir_all(&folder).unwrap();
        for (file, mut bytes) in files(&recorded()) {
            if file.extension().is_some_and(|e| e == "ldb") {
                bytes.truncate(bytes.len() / 2);
            }
            fs::write(folder.join(file.file_name().unwrap()), bytes).unwrap();
        }
        let path = scratch("carry-torn-store");
        let mut store = LocalStore::open(&path);
        assert!(carry(&mut store, &folder, Path::new("/home/person/.wsp/state.json")).is_err());
        assert!(!path.exists());
    }

    #[test]
    fn todays_app_runs_while_its_singleton_lock_names_a_live_pid() {
        let state = scratch("carry-singleton").with_file_name("state.json");
        fs::create_dir_all(state.with_file_name("desktop")).unwrap();
        assert!(!todays_app_runs(&state), "no lock");
        let lock = state.with_file_name("desktop").join("SingletonLock");
        std::os::unix::fs::symlink(format!("some-host-{}", std::process::id()), &lock).unwrap();
        assert!(todays_app_runs(&state));
        let mut gone = std::process::Command::new("true").spawn().unwrap();
        gone.wait().unwrap();
        fs::remove_file(&lock).unwrap();
        std::os::unix::fs::symlink(format!("some-host-{}", gone.id()), &lock).unwrap();
        assert!(!todays_app_runs(&state), "a lock a crash left behind");
    }

    #[gpui_kit::test]
    fn a_launch_carries_only_into_a_store_never_written(cx: &mut gpui_kit::TestAppContext) {
        let home = scratch("carry-launch").with_file_name("home");
        let folder = folder_of(&home.join("state.json"));
        fs::create_dir_all(&folder).unwrap();
        for (file, bytes) in files(&recorded()) {
            fs::write(folder.join(file.file_name().unwrap()), bytes).unwrap();
        }
        let (used, fresh) = (home.join("used.json"), home.join("fresh.json"));
        fs::write(&used, b"{\"width\":320}").unwrap();

        cx.update(|cx| {
            crate::local_store::init(&used, cx);
            on_launch(&home.join("state.json"), cx);
            assert_eq!(draft(cx.global::<LocalStore>(), "thr_ascii"), None);
            crate::local_store::init(&fresh, cx);
            on_launch(&home.join("state.json"), cx);
        });
        cx.run_until_parked();
        assert_eq!(draft(&LocalStore::open(&fresh), "thr_ascii").unwrap().0, "half a thought");
        assert_eq!(LocalStore::open(&used).get("width"), Some(&json!(320)));
    }
}
