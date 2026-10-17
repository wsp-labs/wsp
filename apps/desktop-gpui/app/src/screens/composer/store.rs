// SPDX-License-Identifier: AGPL-3.0-only
//! The composer's drafts, queues, stash, favourites and fast mode on the local store, under the web app's keys and
//! with its lifetimes: composerDraftStore.ts, promptStashStore.ts, composerFavouritesStore.ts and
//! composerModesStore.ts. A draft's and a queued row's files stay in memory, as composerFiles.ts keeps them; the stash
//! alone stores bytes. Prompt history is read off the transcript and stores nothing (composerPromptHistory.ts).

use std::io;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde_json::{json, Map, Value};

use super::model::{File, Queued, Stash, Stashed, STASH_MAX};
use crate::local_store::LocalStore;

pub const DRAFTS_KEY: &str = "wsp:composer-drafts:v1";
pub const STASH_KEY: &str = "wsp:prompt-stash";
pub const FAVOURITES_KEY: &str = "wsp:composer-favourites:v1";
pub const MODES_KEY: &str = "wsp:composer-modes";
/// The base64 characters one stash entry's files may take; the files past it are dropped by name.
pub const STASH_ENTRY_FILE_CHARS: usize = 2_700_000;

/// The zustand persist versions each key was written at.
const DRAFTS_VERSION: u32 = 2;
const FAVOURITES_VERSION: u32 = 1;
const MODES_VERSION: u32 = 0;

/// A thread's queue as storage gave it back. Rows read back are held: none goes by itself until the person's next
/// Enter or send-now on the thread. Their files were not kept.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Restored {
    pub rows: Vec<Queued>,
    pub held: bool,
}

fn state(store: &LocalStore, key: &str) -> Map<String, Value> {
    store.get(key).and_then(|v| v.get("state")).and_then(Value::as_object).cloned().unwrap_or_default()
}

fn set_state(store: &mut LocalStore, key: &str, state: Map<String, Value>, version: u32) -> io::Result<()> {
    store.set(key, json!({ "state": state, "version": version }))
}

/// The record under `field` of a persisted state, made an object where it was missing or of another shape.
fn record<'a>(state: &'a mut Map<String, Value>, field: &str) -> &'a mut Map<String, Value> {
    let slot = state.entry(field).or_insert_with(|| Value::Object(Map::new()));
    if !slot.is_object() {
        *slot = Value::Object(Map::new());
    }
    slot.as_object_mut().expect("made an object above")
}

fn draft_of(value: &Value) -> Option<(String, usize)> {
    let prompt = value.get("prompt")?.as_str().filter(|p| !p.is_empty())?;
    let cursor =
        value.get("cursor").and_then(Value::as_u64).map(|c| c as usize).filter(|&c| prompt.is_char_boundary(c)).unwrap_or(prompt.len());
    Some((prompt.to_owned(), cursor))
}

/// The words a thread's composer held and the caret's byte offset in them; an empty draft reads as none.
pub fn draft(store: &LocalStore, thread_key: &str) -> Option<(String, usize)> {
    state(store, DRAFTS_KEY).get("drafts")?.get(thread_key).and_then(draft_of)
}

/// Keeps a thread's draft until it is cleared; an empty prompt clears it.
pub fn set_draft(store: &mut LocalStore, thread_key: &str, prompt: &str, cursor: usize) -> io::Result<()> {
    if draft(store, thread_key) == (!prompt.is_empty()).then(|| (prompt.to_owned(), cursor)) {
        return Ok(());
    }
    let mut state = state(store, DRAFTS_KEY);
    let drafts = record(&mut state, "drafts");
    if prompt.is_empty() {
        drafts.remove(thread_key);
    } else {
        drafts.insert(thread_key.to_owned(), json!({ "prompt": prompt, "cursor": cursor }));
    }
    set_state(store, DRAFTS_KEY, state, DRAFTS_VERSION)
}

fn rows_of(value: Option<&Value>) -> Vec<Queued> {
    let rows = value.and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default();
    rows.iter()
        .filter_map(|row| {
            let id = row.get("id")?.as_str()?.parse().ok()?;
            let prompt = row.get("prompt")?.as_str()?.to_owned();
            Some(Queued { id, prompt, files: Vec::new() })
        })
        .collect()
}

/// A thread's waiting messages, oldest first, without their files.
pub fn queue(store: &LocalStore, thread_key: &str) -> Restored {
    let rows = rows_of(state(store, DRAFTS_KEY).get("queues").and_then(|q| q.get(thread_key)));
    Restored { held: !rows.is_empty(), rows }
}

/// Keeps a thread's waiting messages, their ids and words only; an empty queue drops the key.
pub fn set_queue(store: &mut LocalStore, thread_key: &str, rows: &[Queued]) -> io::Result<()> {
    let mut state = state(store, DRAFTS_KEY);
    let queues = record(&mut state, "queues");
    if rows.is_empty() {
        if queues.remove(thread_key).is_none() {
            return Ok(());
        }
    } else {
        let rows: Vec<Value> = rows.iter().map(|r| json!({ "id": r.id.to_string(), "prompt": r.prompt })).collect();
        queues.insert(thread_key.to_owned(), Value::Array(rows));
    }
    set_state(store, DRAFTS_KEY, state, DRAFTS_VERSION)
}

/// A thread's first start hands it what waited under the workspace's key: the queue goes behind the rows already
/// at `to`, the draft goes where `to` has none, and fast mode moves over whatever `to` held.
pub fn rekey(store: &mut LocalStore, from: &str, to: &str) -> io::Result<()> {
    if from == to {
        return Ok(());
    }
    let mut drafts_state = state(store, DRAFTS_KEY);
    let mut changed = false;
    let moving = record(&mut drafts_state, "queues").remove(from);
    if let Some(moving) = moving {
        let queues = record(&mut drafts_state, "queues");
        let mut rows = queues.get(to).and_then(Value::as_array).cloned().unwrap_or_default();
        rows.extend(moving.as_array().cloned().unwrap_or_default());
        queues.insert(to.to_owned(), Value::Array(rows));
        changed = true;
    }
    let drafts = record(&mut drafts_state, "drafts");
    if drafts.get(to).and_then(draft_of).is_none() {
        if let Some(draft) = drafts.remove(from) {
            drafts.insert(to.to_owned(), draft);
            changed = true;
        }
    }
    if changed {
        set_state(store, DRAFTS_KEY, drafts_state, DRAFTS_VERSION)?;
    }
    let mut modes = state(store, MODES_KEY);
    let fast = record(&mut modes, "fast");
    if let Some(on) = fast.remove(from) {
        fast.insert(to.to_owned(), on);
        set_state(store, MODES_KEY, modes, MODES_VERSION)?;
    }
    Ok(())
}

/// Splits `files` by the per-entry budget in the order they were added: keeps the ones that fit and answers with the
/// names of the rest.
pub fn fit(files: &mut Vec<File>) -> Vec<String> {
    let mut used = 0;
    let mut dropped = Vec::new();
    files.retain(|f| {
        let chars = f.bytes.len().div_ceil(3) * 4;
        if used + chars > STASH_ENTRY_FILE_CHARS {
            dropped.push(f.name.clone());
            return false;
        }
        used += chars;
        true
    });
    dropped
}

fn stashed_of(value: &Value) -> Option<Stashed> {
    let text = |field: &str| value.get(field).and_then(Value::as_str);
    let (_id, at, prompt) = (text("id")?, text("createdAt")?, text("prompt")?);
    value.get("dropped")?.as_array()?;
    let files = value
        .get("files")?
        .as_array()?
        .iter()
        .filter_map(|f| {
            let text = |field: &str| f.get(field).and_then(Value::as_str).map(str::to_owned);
            f.get("size")?.as_u64()?;
            let bytes = STANDARD.decode(text("bytes")?).ok()?;
            Some(File { name: text("name")?, media_type: text("mediaType")?, bytes: Arc::new(bytes) })
        })
        .collect();
    Some(Stashed { prompt: prompt.to_owned(), files, at: time_of(at)? })
}

/// The stash, newest first; an entry of another shape is skipped.
pub fn stash(store: &LocalStore) -> Stash {
    let entries = store.get(STASH_KEY).and_then(Value::as_array).map(|e| e.iter().filter_map(stashed_of).collect());
    Stash { entries: entries.unwrap_or_default() }
}

/// Writes the stash's newest `STASH_MAX` entries at once, each one's files within the budget; on an error nothing was
/// kept and the composer should not clear.
pub fn set_stash(store: &mut LocalStore, stash: &Stash) -> io::Result<()> {
    let entries: Vec<Value> = stash
        .entries
        .iter()
        .take(STASH_MAX)
        .enumerate()
        .map(|(ix, entry)| {
            let mut files = entry.files.clone();
            let dropped = fit(&mut files);
            let files: Vec<Value> = files
                .iter()
                .map(|f| json!({ "mediaType": f.media_type, "name": f.name, "bytes": STANDARD.encode(f.bytes.as_slice()), "size": f.bytes.len() }))
                .collect();
            let millis = entry.at.duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
            json!({ "id": format!("{millis:x}-{ix}"), "createdAt": iso_of(entry.at), "prompt": entry.prompt, "files": files, "dropped": dropped })
        })
        .collect();
    store.set(STASH_KEY, Value::Array(entries))
}

/// The starred models across every workspace, each `harness:model`, in the order they were starred.
pub fn favourites(store: &LocalStore) -> Vec<String> {
    let keys = state(store, FAVOURITES_KEY).get("keys").and_then(Value::as_array).cloned().unwrap_or_default();
    keys.into_iter().filter_map(|k| k.as_str().filter(|k| k.contains(':')).map(str::to_owned)).collect()
}

pub fn set_favourites(store: &mut LocalStore, keys: &[String]) -> io::Result<()> {
    let mut state = Map::new();
    state.insert("keys".to_owned(), json!(keys));
    set_state(store, FAVOURITES_KEY, state, FAVOURITES_VERSION)
}

/// Fast mode where the person turned it on or off by hand on this thread; none means the thread's own rows decide.
pub fn fast(store: &LocalStore, thread_key: &str) -> Option<bool> {
    state(store, MODES_KEY).get("fast")?.get(thread_key)?.as_bool()
}

/// Kept for as long as the store is; nothing clears it but a rekey.
pub fn set_fast(store: &mut LocalStore, thread_key: &str, on: bool) -> io::Result<()> {
    let mut state = state(store, MODES_KEY);
    record(&mut state, "fast").insert(thread_key.to_owned(), Value::Bool(on));
    set_state(store, MODES_KEY, state, MODES_VERSION)
}

/// `at` as `Date.toISOString` writes it, `2026-10-17T09:05:03.042Z`.
fn iso_of(at: SystemTime) -> String {
    let since = at.duration_since(UNIX_EPOCH).unwrap_or_default();
    let secs = since.as_secs();
    let (y, m, d) = civil_of((secs / 86_400) as i64);
    let s = secs % 86_400;
    format!("{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z", s / 3600, s / 60 % 60, s % 60, since.subsec_millis())
}

/// The time an ISO string in `Date.toISOString`'s form names; any other form reads as none.
fn time_of(iso: &str) -> Option<SystemTime> {
    let num = |range: std::ops::Range<usize>| iso.get(range)?.parse::<u64>().ok();
    let b = iso.as_bytes();
    if b.len() != 24 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' || b[19] != b'.' || b[23] != b'Z' {
        return None;
    }
    let (y, m, d) = (num(0..4)? as i64, num(5..7)?, num(8..10)?);
    let (h, min, s, ms) = (num(11..13)?, num(14..16)?, num(17..19)?, num(20..23)?);
    if !(1..=12).contains(&m) || !(1..=31).contains(&d) || h > 23 || min > 59 || s > 59 {
        return None;
    }
    let days = u64::try_from(days_of(y, m as i64, d as i64)).ok()?;
    Some(UNIX_EPOCH + Duration::from_millis(((days * 86_400 + h * 3600 + min * 60 + s) * 1000) + ms))
}

/// Howard Hinnant's days-from-civil and its inverse, on the proleptic Gregorian calendar.
fn days_of(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

fn civil_of(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_store::scratch;

    fn file(name: &str, len: usize) -> File {
        File { name: name.to_owned(), media_type: "image/png".to_owned(), bytes: Arc::new(vec![7; len]) }
    }

    fn row(id: u64, prompt: &str) -> Queued {
        Queued { id, prompt: prompt.to_owned(), files: vec![file("held.png", 3)] }
    }

    fn at(millis: u64) -> SystemTime {
        UNIX_EPOCH + Duration::from_millis(millis)
    }

    #[test]
    fn a_draft_reads_back_after_a_reopen_and_an_empty_one_clears() {
        let path = scratch("drafts");
        let mut store = LocalStore::open(&path);
        set_draft(&mut store, "thr_1", "half a", 6).unwrap();
        set_draft(&mut store, "ws_1", "héllo", 99).unwrap();
        let mut store = LocalStore::open(&path);
        assert_eq!(draft(&store, "thr_1"), Some(("half a".to_owned(), 6)));
        assert_eq!(draft(&store, "ws_1"), Some(("héllo".to_owned(), "héllo".len())), "a cursor past the words lands at their end");
        assert_eq!(draft(&store, "thr_2"), None);
        assert_eq!(store.get(DRAFTS_KEY).unwrap()["version"], json!(2));
        set_draft(&mut store, "thr_1", "", 0).unwrap();
        assert_eq!(draft(&LocalStore::open(&path), "thr_1"), None);
        assert_eq!(draft(&LocalStore::open(&path), "ws_1").unwrap().0, "héllo");
    }

    #[test]
    fn a_blank_draft_or_a_malformed_row_written_by_another_build_reads_as_none() {
        let path = scratch("malformed");
        let mut store = LocalStore::open(&path);
        let doctored = json!({ "state": {
            "drafts": { "blank": { "prompt": "", "cursor": 0 }, "odd": 3, "nocursor": { "prompt": "abc" } },
            "queues": { "thr": [{ "id": "1", "prompt": "kept" }, { "id": 7 }, "junk", { "id": "x", "prompt": "no number" }], "empty": [] },
        }, "version": 2 });
        store.set(DRAFTS_KEY, doctored).unwrap();
        assert_eq!(draft(&store, "blank"), None);
        assert_eq!(draft(&store, "odd"), None);
        assert_eq!(draft(&store, "nocursor"), Some(("abc".to_owned(), 3)));
        assert_eq!(queue(&store, "thr").rows.iter().map(|r| r.prompt.as_str()).collect::<Vec<_>>(), ["kept"]);
        assert_eq!(queue(&store, "empty"), Restored::default());
    }

    #[test]
    fn a_queue_reads_back_held_without_its_files_and_an_empty_one_goes() {
        let path = scratch("queue");
        let mut store = LocalStore::open(&path);
        assert_eq!(queue(&store, "thr_1"), Restored { rows: vec![], held: false });
        set_queue(&mut store, "thr_1", &[row(3, "first"), row(9, "second")]).unwrap();
        set_draft(&mut store, "thr_1", "still here", 0).unwrap();
        let mut store = LocalStore::open(&path);
        let back = queue(&store, "thr_1");
        assert!(back.held);
        assert_eq!(
            back.rows,
            vec![Queued { id: 3, prompt: "first".to_owned(), files: vec![] }, Queued { id: 9, prompt: "second".to_owned(), files: vec![] }]
        );
        assert!(store.get(DRAFTS_KEY).unwrap()["state"].get("held").is_none(), "the hold is not written, so every reopen holds again");
        set_queue(&mut store, "thr_1", &[]).unwrap();
        let store = LocalStore::open(&path);
        assert_eq!(queue(&store, "thr_1"), Restored::default());
        assert!(store.get(DRAFTS_KEY).unwrap()["state"]["queues"].get("thr_1").is_none());
        assert_eq!(draft(&store, "thr_1").unwrap().0, "still here");
    }

    #[test]
    fn a_first_start_moves_the_workspaces_queue_draft_and_fast_mode_onto_the_thread() {
        let path = scratch("rekey");
        let mut store = LocalStore::open(&path);
        set_queue(&mut store, "ws", &[row(1, "from the new thread page")]).unwrap();
        set_queue(&mut store, "thr", &[row(2, "already here")]).unwrap();
        set_draft(&mut store, "ws", "typed before the start", 3).unwrap();
        set_fast(&mut store, "ws", true).unwrap();
        set_fast(&mut store, "thr", false).unwrap();
        rekey(&mut store, "ws", "thr").unwrap();
        let mut store = LocalStore::open(&path);
        assert_eq!(queue(&store, "thr").rows.iter().map(|r| r.id).collect::<Vec<_>>(), [2, 1]);
        assert_eq!(queue(&store, "ws"), Restored::default());
        assert_eq!(draft(&store, "thr"), Some(("typed before the start".to_owned(), 3)));
        assert_eq!(draft(&store, "ws"), None);
        assert_eq!(fast(&store, "thr"), Some(true));
        assert_eq!(fast(&store, "ws"), None);

        set_draft(&mut store, "ws", "second", 0).unwrap();
        rekey(&mut store, "ws", "thr").unwrap();
        assert_eq!(draft(&store, "thr").unwrap().0, "typed before the start", "a thread's own draft is not overwritten");
        assert_eq!(draft(&store, "ws").unwrap().0, "second", "and the workspace's stays where it was");
        rekey(&mut store, "thr", "thr").unwrap();
        assert_eq!(queue(&store, "thr").rows.len(), 2);
    }

    #[test]
    fn the_stash_keeps_files_and_order_across_a_reopen() {
        let path = scratch("stash");
        let mut store = LocalStore::open(&path);
        assert_eq!(stash(&store), Stash::default());
        let mut s = Stash::default();
        s.push(Stashed { prompt: "older".to_owned(), files: vec![], at: at(1_700_000_000_123) });
        s.push(Stashed { prompt: "newer".to_owned(), files: vec![file("a.png", 5), file("b.png", 1)], at: at(1_760_000_000_042) });
        set_stash(&mut store, &s).unwrap();
        assert_eq!(stash(&LocalStore::open(&path)), s);
        let reopened = LocalStore::open(&path);
        let raw = &reopened.get(STASH_KEY).unwrap()[0];
        assert_eq!(raw["createdAt"], json!("2025-10-09T08:53:20.042Z"));
        assert_eq!(raw["files"][0], json!({ "mediaType": "image/png", "name": "a.png", "bytes": "BwcHBwc=", "size": 5 }));
        assert_eq!(raw["dropped"], json!([]));
    }

    #[test]
    fn the_stash_keeps_twenty_entries_newest_first() {
        let path = scratch("stash-cap");
        let mut store = LocalStore::open(&path);
        let entries = (0..25).rev().map(|i| Stashed { prompt: format!("draft {i}"), files: vec![], at: at(i * 1000) }).collect();
        set_stash(&mut store, &Stash { entries }).unwrap();
        let back = stash(&LocalStore::open(&path));
        assert_eq!(back.entries.len(), STASH_MAX);
        assert_eq!(back.entries[0].prompt, "draft 24");
        assert_eq!(back.entries[19].prompt, "draft 5");
    }

    #[test]
    fn a_stash_entrys_files_past_the_budget_are_dropped_by_name() {
        let third = STASH_ENTRY_FILE_CHARS / 4 * 3 / 3;
        let mut files = vec![file("one", third), file("big", third * 3), file("two", third), file("three", third + 1)];
        assert_eq!(fit(&mut files), ["big", "three"]);
        assert_eq!(files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["one", "two"]);

        let path = scratch("stash-budget");
        let mut store = LocalStore::open(&path);
        let entry = Stashed { prompt: "p".to_owned(), files: vec![file("small", 10), file("huge", STASH_ENTRY_FILE_CHARS)], at: at(0) };
        set_stash(&mut store, &Stash { entries: vec![entry] }).unwrap();
        let raw = &store.get(STASH_KEY).unwrap()[0];
        assert_eq!(raw["dropped"], json!(["huge"]));
        assert_eq!(stash(&LocalStore::open(&path)).entries[0].files.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["small"]);
    }

    #[test]
    fn a_stash_past_the_stores_quota_is_refused() {
        let mut store = LocalStore::open(scratch("stash-quota"));
        let full = STASH_ENTRY_FILE_CHARS / 4 * 3;
        let mut s = Stash::default();
        s.push(Stashed { prompt: "one".to_owned(), files: vec![file("a.png", full)], at: at(0) });
        set_stash(&mut store, &s).unwrap();
        s.push(Stashed { prompt: "two".to_owned(), files: vec![file("b.png", full)], at: at(1) });
        assert!(set_stash(&mut store, &s).is_err(), "two full entries pass the 5 MB the web's localStorage holds");
        assert_eq!(stash(&store).entries.iter().map(|e| e.prompt.as_str()).collect::<Vec<_>>(), ["one"]);
    }

    #[test]
    fn a_stash_entry_of_another_shape_is_skipped() {
        let mut store = LocalStore::open(scratch("stash-shape"));
        let good =
            json!({ "id": "a", "createdAt": "2025-10-17T09:05:03.042Z", "prompt": "kept", "files": [{ "name": "x" }], "dropped": [] });
        let no_dropped = json!({ "id": "b", "createdAt": "2025-10-17T09:05:03.042Z", "prompt": "gone", "files": [] });
        let bad_time = json!({ "id": "c", "createdAt": "yesterday", "prompt": "gone", "files": [], "dropped": [] });
        store.set(STASH_KEY, json!([good, no_dropped, bad_time, 4])).unwrap();
        let back = stash(&store);
        assert_eq!(back.entries.len(), 1);
        assert_eq!(back.entries[0].prompt, "kept");
        assert!(back.entries[0].files.is_empty());
        assert_eq!(back.entries[0].at, UNIX_EPOCH + Duration::from_millis(1_760_691_903_042));
    }

    #[test]
    fn favourites_read_back_in_order_and_keys_without_a_harness_are_skipped() {
        let path = scratch("favourites");
        let mut store = LocalStore::open(&path);
        assert!(favourites(&store).is_empty());
        set_favourites(&mut store, &["claude:opus".to_owned(), "codex:gpt-5".to_owned()]).unwrap();
        assert_eq!(favourites(&LocalStore::open(&path)), ["claude:opus", "codex:gpt-5"]);
        assert_eq!(store.get(FAVOURITES_KEY).unwrap()["version"], json!(1));
        store.set(FAVOURITES_KEY, json!({ "state": { "keys": ["claude:opus", "nocolon", 3] }, "version": 1 })).unwrap();
        assert_eq!(favourites(&store), ["claude:opus"]);
    }

    #[test]
    fn fast_mode_is_kept_per_thread_and_unset_reads_as_none() {
        let path = scratch("fast");
        let mut store = LocalStore::open(&path);
        assert_eq!(fast(&store, "thr_1"), None);
        set_fast(&mut store, "thr_1", true).unwrap();
        set_fast(&mut store, "thr_2", false).unwrap();
        let store = LocalStore::open(&path);
        assert_eq!(fast(&store, "thr_1"), Some(true));
        assert_eq!(fast(&store, "thr_2"), Some(false));
        assert_eq!(store.get(MODES_KEY).unwrap(), &json!({ "state": { "fast": { "thr_1": true, "thr_2": false } }, "version": 0 }));
    }

    #[test]
    fn an_iso_time_round_trips() {
        for millis in [0, 951_782_400_000, 1_760_691_903_042, 4_102_444_799_999] {
            assert_eq!(time_of(&iso_of(at(millis))), Some(at(millis)));
        }
        assert_eq!(iso_of(at(951_782_400_000)), "2000-02-29T00:00:00.000Z");
        assert_eq!(time_of("2026-13-01T00:00:00.000Z"), None);
    }
}
