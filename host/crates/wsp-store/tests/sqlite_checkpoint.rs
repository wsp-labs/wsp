// SPDX-License-Identifier: AGPL-3.0-only
// The checkpointer of the state database against sqlite-checkpoint.test.ts (spec/sqlite_store.md, 12 and 13): a
// thread of its own copies a commit's WAL, a commit past the bound copies it itself, a thread that dies or cannot be
// made is made again, and the WAL file is given back past its kept size.
mod common;

use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use common::*;
use serde_json::json;
use wsp_store::sqlite_store::{
    checkpoints_stopped_line, sqlite_store_with, CheckpointJob, CheckpointerSpawn, RealCheckpointer, SqliteStoreOptions, WAL_BOUND_BYTES,
    WAL_KEPT_BYTES,
};
use wsp_store::store::Store;

/// The WAL's last frame and the last one copied into the database, off the wal-index header SQLite keeps in the shared
/// memory file: mxFrame at byte 16 of the header, nBackfill at byte 96 after its two copies (sqlite-checkpoint.test.ts:33-37).
fn wal(db: &Path) -> (u32, u32) {
    let shm = fs::read(db.with_file_name("state.db-shm")).unwrap();
    let at = |i: usize| u32::from_le_bytes(shm[i..i + 4].try_into().unwrap());
    (at(16), at(96))
}

fn wait_for(limit: Duration, mut ok: impl FnMut() -> bool) -> bool {
    let until = Instant::now() + limit;
    while Instant::now() < until {
        if ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    ok()
}

/// Past SQLite's own threshold of 1000 pages in one commit.
fn over_threshold() -> Vec<u8> {
    vec![1u8; 6 * 1024 * 1024]
}

/// What each next checkpointer thread is, in order; `Real` once the script runs out. Counts every spawn asked for.
#[derive(Clone, Copy, Debug)]
enum Next {
    Real,
    /// Alive and never checkpointing.
    Stalled,
    /// Ends at once with an error, as a worker whose first line throws.
    Dies,
    /// Cannot be made at all.
    Refused,
}

struct Scripted {
    script: Mutex<Vec<Next>>,
    spawned: AtomicUsize,
}

impl Scripted {
    fn new(script: Vec<Next>) -> Arc<Scripted> {
        Arc::new(Scripted { script: Mutex::new(script), spawned: AtomicUsize::new(0) })
    }
}

impl CheckpointerSpawn for Scripted {
    fn spawn(&self, job: CheckpointJob) -> std::io::Result<JoinHandle<Result<(), String>>> {
        self.spawned.fetch_add(1, Ordering::SeqCst);
        let next = {
            let mut s = self.script.lock().unwrap();
            if s.is_empty() {
                Next::Real
            } else {
                s.remove(0)
            }
        };
        match next {
            Next::Real => RealCheckpointer.spawn(job),
            Next::Stalled => Ok(std::thread::spawn(|| loop {
                std::thread::park();
            })),
            Next::Dies => Ok(std::thread::spawn(|| Err("a tick that throws".to_string()))),
            Next::Refused => Err(std::io::Error::other("no thread to be had")),
        }
    }
}

fn options(spawner: Arc<Scripted>, warned: Arc<Mutex<Vec<String>>>) -> SqliteStoreOptions {
    SqliteStoreOptions { checkpointer: spawner, warn: Arc::new(move |line: &str| warned.lock().unwrap().push(line.to_string())) }
}

#[test]
fn leaves_a_commits_wal_to_a_thread_of_its_own_which_copies_it_within_seconds() {
    let home = Home::new();
    let store = sqlite_store_with(&home.state(), writer(), options(Scripted::new(vec![]), Default::default()));
    run(store.put_blob("transcripts", "w1", over_threshold())).unwrap();
    let (frames, copied) = wal(&home.db());
    assert!(frames > 1000, "{frames} frames");
    assert!(copied < frames, "the commit copied its own WAL: {copied} of {frames}");
    assert!(wait_for(Duration::from_secs(4), || wal(&home.db()).1 == frames), "copied {:?} of {frames}", wal(&home.db()));
}

#[test]
fn checkpoints_on_a_commit_once_the_wal_passes_its_bound_where_the_thread_never_does() {
    let home = Home::new();
    let store = sqlite_store_with(&home.state(), writer(), options(Scripted::new(vec![Next::Stalled]), Default::default()));
    let blob = over_threshold();
    let (mut largest_wal, mut largest_copied) = (0u64, 0u32);
    for i in 0..((WAL_BOUND_BYTES as f64 * 1.5) / blob.len() as f64).ceil() as usize {
        run(store.put_blob("transcripts", &format!("w{i}"), blob.clone())).unwrap();
        largest_wal = largest_wal.max(fs::metadata(home.db().with_file_name("state.db-wal")).unwrap().len());
        largest_copied = largest_copied.max(wal(&home.db()).1);
    }
    assert!(largest_copied > 0, "no commit checkpointed");
    assert!(largest_wal < WAL_BOUND_BYTES + 2 * blob.len() as u64, "the WAL grew to {largest_wal}");
}

#[test]
fn starts_the_thread_again_after_one_that_dies_and_it_checkpoints() {
    let home = Home::new();
    let spawner = Scripted::new(vec![Next::Dies]);
    let store = sqlite_store_with(&home.state(), writer(), options(spawner.clone(), Default::default()));
    run(store.put_blob("transcripts", "w1", over_threshold())).unwrap();
    let (frames, copied) = wal(&home.db());
    assert!(copied < frames);
    assert!(wait_for(Duration::from_secs(8), || wal(&home.db()).1 == frames), "copied {:?} of {frames}", wal(&home.db()));
    assert_eq!(spawner.spawned.load(Ordering::SeqCst), 2);
}

#[test]
fn starts_the_thread_again_after_one_that_cannot_be_made_and_it_checkpoints() {
    let home = Home::new();
    let spawner = Scripted::new(vec![Next::Refused]);
    let store = sqlite_store_with(&home.state(), writer(), options(spawner.clone(), Default::default()));
    run(store.put_blob("transcripts", "w1", over_threshold())).unwrap();
    let (frames, copied) = wal(&home.db());
    assert!(copied < frames);
    assert!(wait_for(Duration::from_secs(8), || wal(&home.db()).1 == frames), "copied {:?} of {frames}", wal(&home.db()));
    assert_eq!(spawner.spawned.load(Ordering::SeqCst), 2);
}

#[test]
fn gives_up_after_three_threads_in_a_row_and_says_so_once() {
    let home = Home::new();
    let spawner = Scripted::new(vec![Next::Refused, Next::Dies, Next::Refused, Next::Refused, Next::Refused]);
    let warned: Arc<Mutex<Vec<String>>> = Default::default();
    let store = sqlite_store_with(&home.state(), writer(), options(spawner.clone(), warned.clone()));
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    assert!(wait_for(Duration::from_secs(8), || !warned.lock().unwrap().is_empty()), "no warning");
    std::thread::sleep(Duration::from_millis(2500));
    let db = home.db().display().to_string();
    assert_eq!(*warned.lock().unwrap(), vec![checkpoints_stopped_line(&db, "no thread to be had")]);
    assert_eq!(spawner.spawned.load(Ordering::SeqCst), 3);
}

#[test]
fn the_give_up_line_names_the_database_and_why() {
    assert_eq!(
        checkpoints_stopped_line("/h/state.db", "no thread to be had"),
        "the checkpoints of /h/state.db stopped (no thread to be had), so a commit takes one past 64 MB of WAL"
    );
}

#[test]
fn a_thread_ends_with_its_database_and_is_not_made_again() {
    let home = Home::new();
    let spawner = Scripted::new(vec![]);
    let store = sqlite_store_with(&home.state(), writer(), options(spawner.clone(), Default::default()));
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    drop(store);
    for f in ["state.db", "state.db-wal", "state.db-shm"] {
        let _ = fs::remove_file(home.dir.path().join(f));
    }
    std::thread::sleep(Duration::from_millis(5000));
    assert_eq!(spawner.spawned.load(Ordering::SeqCst), 1);
    assert!(!home.db().exists(), "the checkpointer made the database again");
}

#[test]
fn gives_the_wal_file_back_past_its_kept_size_once_a_checkpoint_copied_it() {
    let home = Home::new();
    let store = sqlite_store_with(&home.state(), writer(), options(Scripted::new(vec![]), Default::default()));
    run(store.put_blob("transcripts", "w1", vec![1u8; (WAL_KEPT_BYTES + 8 * 1024 * 1024) as usize])).unwrap();
    let wal_file = home.db().with_file_name("state.db-wal");
    assert!(fs::metadata(&wal_file).unwrap().len() > WAL_KEPT_BYTES);
    let (frames, _) = wal(&home.db());
    assert!(wait_for(Duration::from_secs(4), || wal(&home.db()).1 == frames));
    run(store.put("workspaces", "a", json!({"id": "a"}))).unwrap();
    assert!(fs::metadata(&wal_file).unwrap().len() <= WAL_KEPT_BYTES, "{}", fs::metadata(&wal_file).unwrap().len());
}
