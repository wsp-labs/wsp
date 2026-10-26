// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/slate-images.ts at 65f6b89c7. Spec: spec/slate_images.md (rules 1-6).
//!
//! A slate's image piece, read for the window: a path from any folder on the thread's computer, this one's disk or
//! another computer's through its daemon, or an address the host fetches once the person allows its domain. Only a
//! regular file whose bytes are an image type and that weighs at most the cap shows; each answer carries its version;
//! a thread reads at most four images at once.

use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::io::Read;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::rc::Rc;

use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use futures::future::LocalBoxFuture;
use serde_json::{json, Value};
use tokio::sync::oneshot;

pub use wsp_wire::wire::daemon::FsImageReply;

use wsp_core::node_path::join;
use wsp_wire::wire::daemon_version::place_update_line;
use wsp_words::attachments::image_type_of;
use wsp_words::daemon_contract::unknown_op_line;
use wsp_words::js::{js_round, number_string};

use crate::pending::slate::{
    slate_image_missing, slate_image_not_a_file, slate_image_source, slate_image_too_big, slate_looks_svg, slate_not_an_image,
    slate_problem, SlateImageSource, SLATE_IMAGE_MAX_BYTES,
};
use crate::pending::wire::SlatesImageAnswer;
use crate::slate_image_fetch::{fetch_slate_image, problem_answer, FetchRoad};

/// How many images one thread reads or fetches at once (slate-images.ts:189).
pub const IMAGES_AT_ONCE: usize = 4;

/// A daemon's refusal of an `fs.image` read: its code where it sent one, and its words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageRefusal {
    pub code: Option<String>,
    pub message: String,
}

/// The thread's computer where it is not this one (slate-images.ts:192).
#[derive(Clone)]
pub struct ImageOn {
    pub name: String,
    pub read: Rc<dyn Fn(String) -> LocalBoxFuture<'static, Result<FsImageReply, ImageRefusal>>>,
}

/// slate-images.ts:197 SlateImageRoads.
#[derive(Clone)]
pub struct SlateImageRoads {
    pub thread: String,
    /// The version the window holds.
    pub have: Option<String>,
    /// The thread's folder on its own computer, which a relative path is read under.
    pub folder: Option<String>,
    /// Absent where the thread runs on this computer.
    pub on: Option<ImageOn>,
    /// Whether the person allowed this domain for the thread.
    pub allowed: Rc<dyn Fn(&str) -> bool>,
    pub fetch: Option<Rc<dyn FetchRoad>>,
}

/// The reads of one thread under way and the ones waiting for a turn (slate-images.ts:211).
#[derive(Default)]
struct Turns {
    running: usize,
    waiting: VecDeque<oneshot::Sender<()>>,
}

thread_local! {
    static TURNS: RefCell<HashMap<String, Turns>> = RefCell::new(HashMap::new());
}

/// A read's place in its thread's turns: the turn it runs in, or the place it waits for one. Dropped at any point, it
/// gives the turn back, or passes it to the next waiter, so no way out of a read leaves the count high.
struct Turn {
    thread: String,
    queued: Option<oneshot::Receiver<()>>,
}

impl Turn {
    fn enter(thread: &str) -> Turn {
        let queued = TURNS.with(|turns| {
            let mut turns = turns.borrow_mut();
            let mine = turns.entry(thread.to_string()).or_default();
            if mine.running >= IMAGES_AT_ONCE {
                let (tx, rx) = oneshot::channel();
                mine.waiting.push_back(tx);
                Some(rx)
            } else {
                mine.running += 1;
                None
            }
        });
        Turn { thread: thread.to_string(), queued }
    }
}

impl Drop for Turn {
    fn drop(&mut self) {
        if let Some(queued) = self.queued.as_mut() {
            if queued.try_recv().is_err() {
                return;
            }
        }
        TURNS.with(|turns| {
            let mut turns = turns.borrow_mut();
            let Some(mine) = turns.get_mut(&self.thread) else {
                return;
            };
            while let Some(next) = mine.waiting.pop_front() {
                if next.send(()).is_ok() {
                    return;
                }
            }
            mine.running -= 1;
            let idle = mine.running == 0;
            if idle {
                turns.remove(&self.thread);
            }
        });
    }
}

async fn in_turn<T>(thread: &str, work: impl Future<Output = T>) -> T {
    let mut turn = Turn::enter(thread);
    if let Some(queued) = turn.queued.as_mut() {
        let _ = queued.await;
        turn.queued = None;
    }
    work.await
}

/// An answer the window already holds, said as unchanged rather than sent again (slate-images.ts:240).
fn kept(have: Option<&str>, answer: Value) -> Value {
    match (have, answer.get("version").and_then(Value::as_str), answer.get("bytes")) {
        (Some(have), Some(version), Some(_)) if version == have => json!({ "unchanged": true, "version": have }),
        _ => answer,
    }
}

/// `round(modified):size[:inode][:changed]` (rule 4).
pub fn version_of(modified_ms: f64, size: u64, inode: Option<u64>, changed_ns: Option<u128>) -> String {
    let mut parts = vec![number_string(js_round(modified_ms)), size.to_string()];
    if let Some(inode) = inode {
        parts.push(inode.to_string());
    }
    if let Some(changed) = changed_ns {
        parts.push(changed.to_string());
    }
    parts.join(":")
}

/// The errno names Node puts in its error codes, for the codes a read of a file can fail with.
fn node_code(e: &std::io::Error) -> Option<&'static str> {
    let code = match e.raw_os_error()? {
        libc::ENOENT => "ENOENT",
        libc::ENOTDIR => "ENOTDIR",
        libc::EACCES => "EACCES",
        libc::EPERM => "EPERM",
        libc::EISDIR => "EISDIR",
        libc::ELOOP => "ELOOP",
        libc::ENXIO => "ENXIO",
        libc::ENODEV => "ENODEV",
        libc::EMFILE => "EMFILE",
        libc::ENFILE => "ENFILE",
        libc::ENAMETOOLONG => "ENAMETOOLONG",
        libc::EIO => "EIO",
        libc::EBUSY => "EBUSY",
        libc::EROFS => "EROFS",
        libc::EINVAL => "EINVAL",
        libc::EAGAIN => "EAGAIN",
        libc::EOVERFLOW => "EOVERFLOW",
        _ => return None,
    };
    Some(code)
}

fn cap() -> u64 {
    SLATE_IMAGE_MAX_BYTES
}

/// A file on this computer, opened without blocking so a pipe in its place cannot hold the read, and judged on the
/// handle it was opened as, so nothing swapped in after a stat is read (slate-images.ts:254).
fn read_here(shown: &str, path: &str, have: Option<&str>) -> SlatesImageAnswer {
    let mut file = match std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(path) {
        Ok(file) => file,
        Err(e) => {
            let code = node_code(&e);
            if matches!(code, Some("ENOENT") | Some("ENOTDIR")) {
                return problem_answer(slate_image_missing(shown));
            }
            if std::fs::metadata(path).is_ok_and(|found| !found.is_file()) {
                return problem_answer(slate_image_not_a_file(shown));
            }
            let why = code.map_or_else(|| e.to_string(), String::from);
            return problem_answer(slate_problem("R903", &format!("{shown} could not be opened: {why}"), None));
        }
    };
    let meta = match file.metadata() {
        Ok(meta) => meta,
        Err(e) => {
            let why = node_code(&e).map_or_else(|| e.to_string(), String::from);
            return problem_answer(slate_problem("R903", &format!("{shown} could not be opened: {why}"), None));
        }
    };
    if !meta.is_file() {
        return problem_answer(slate_image_not_a_file(shown));
    }
    let size = meta.len();
    if size > cap() {
        return problem_answer(slate_image_too_big(shown, Some(size)));
    }
    let modified_ms = meta.mtime() as f64 * 1000.0 + meta.mtime_nsec() as f64 / 1e6;
    let changed_ns = (meta.ctime() as i128 * 1_000_000_000 + meta.ctime_nsec() as i128) as u128;
    let version = version_of(modified_ms, size, Some(meta.ino()), Some(changed_ns));
    if have == Some(version.as_str()) {
        return json!({ "unchanged": true, "version": version });
    }
    // One byte past what the stat said tells a file still being written; it grows to the cap and no further.
    let mut bytes = vec![0u8; size as usize + 1];
    let mut got = 0usize;
    loop {
        if got == bytes.len() {
            if bytes.len() as u64 > cap() {
                return problem_answer(slate_image_too_big(shown, None));
            }
            let grown = (bytes.len() * 2).min(cap() as usize + 1);
            bytes.resize(grown, 0);
        }
        match file.read(&mut bytes[got..]) {
            Ok(0) => break,
            Ok(n) => got += n,
            Err(e) => return problem_answer(slate_problem("R903", &format!("{shown} could not be read: {e}"), None)),
        }
    }
    if got as u64 > cap() {
        return problem_answer(slate_image_too_big(shown, None));
    }
    bytes.truncate(got);
    match image_type_of(&bytes) {
        None => problem_answer(slate_not_an_image(shown, slate_looks_svg(shown, &bytes))),
        Some(media_type) => json!({ "mediaType": media_type, "bytes": STANDARD.encode(&bytes), "version": version }),
    }
}

/// A file on the thread's computer, which that computer's daemon reads and judges by the same rules (slate-images.ts:296).
async fn on_daemon(shown: &str, path: &str, there: &ImageOn, have: Option<&str>) -> SlatesImageAnswer {
    let said = match (there.read)(path.to_string()).await {
        Ok(said) => said,
        Err(refusal) => {
            return match refusal.code.as_deref() {
                Some("not-found") => problem_answer(slate_image_missing(shown)),
                Some("not-a-file") => problem_answer(slate_image_not_a_file(shown)),
                _ if refusal.message == unknown_op_line("fs.image") => problem_answer(slate_problem(
                    "R903",
                    &format!("{} runs a daemon too old to read images; {} updates it", there.name, place_update_line(&there.name)),
                    None,
                )),
                _ => problem_answer(slate_problem("R903", &format!("{} could not read {shown}: {}", there.name, refusal.message), None)),
            };
        }
    };
    let size = said.size.0 as u64;
    if size > cap() {
        return problem_answer(slate_image_too_big(shown, Some(size)));
    }
    let (Some(media_type), Some(content)) = (said.media_type.clone(), said.content.clone()) else {
        let svg = said.svg == Some(true) || slate_looks_svg(shown, &[]);
        return problem_answer(slate_not_an_image(shown, svg));
    };
    let mut answer = json!({ "mediaType": media_type, "bytes": content });
    if let Some(modified) = said.modified {
        let inode = said.inode.map(|i| i.0 as u64);
        let changed = said.changed.map(|c| c.0 as u128);
        answer["version"] = json!(version_of(modified.0, size, inode, changed));
    }
    kept(have, answer)
}

pub async fn slate_image(src: &str, roads: SlateImageRoads) -> SlatesImageAnswer {
    let path = match slate_image_source(src) {
        SlateImageSource::Problem(problem) => return problem_answer(problem),
        SlateImageSource::Url { url, domain } => {
            if !(roads.allowed)(&domain) {
                return json!({ "ask": { "domain": domain } });
            }
            let fetched = in_turn(&roads.thread, fetch_slate_image(&url, &domain, roads.fetch.clone())).await;
            return kept(roads.have.as_deref(), fetched);
        }
        SlateImageSource::Path(path) => path,
    };
    let whole = path.starts_with('/');
    let thread = roads.thread.clone();
    let folder = roads.folder.clone();
    if !whole && folder.is_none() {
        return problem_answer(slate_problem(
            "R903",
            &format!("{path} is not a whole path, and this host knows no folder for the thread to read it under"),
            None,
        ));
    }
    let under = |base: &str| if whole { path.clone() } else { join(&[base, path.as_str()]) };
    match &roads.on {
        Some(there) => {
            let read_at = under(folder.as_deref().unwrap_or(""));
            in_turn(&thread, on_daemon(&path, &read_at, there, roads.have.as_deref())).await
        }
        None => {
            let base = match folder.as_deref() {
                Some(folder) if !folder.starts_with('/') => {
                    let cwd = std::env::current_dir().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|_| "/".into());
                    join(&[cwd.as_str(), folder])
                }
                Some(folder) => folder.to_string(),
                None => String::new(),
            };
            let read_at = under(&base);
            let have = roads.have.clone();
            in_turn(&thread, async move { read_here(&path, &read_at, have.as_deref()) }).await
        }
    }
}
