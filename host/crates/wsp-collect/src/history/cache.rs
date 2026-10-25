// SPDX-License-Identifier: AGPL-3.0-only
//! What each session file came to last time, kept in one JSON so a second run reads only the files that changed. A
//! file is trusted while its stamp and the parse version behind its counts both match. Port of
//! packages/collect/src/history/cache.ts at a3b870a6e; the spec is spec/history_store.md.
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use indexmap::IndexMap;
use serde::Serialize;
use serde_json::Value;
use wsp_catalog::json_parse::parse_json;
use wsp_words::exit::HostError;

use crate::history::reader::PARSE_VERSION;
use crate::history::tally::{js_key_order, HistoryBucket};

/// cache.ts:15. `&self` throughout: a cache is shared by the reads one store runs at once, on one thread.
pub trait HistoryCache {
    /// The buckets kept for this file when its stamp still matches; a hit keeps the entry for the next save.
    fn get(&self, path: &str, stamp: &str) -> Option<Vec<HistoryBucket>>;
    fn set(&self, path: &str, stamp: &str, buckets: &[HistoryBucket]);
    /// Writes what this run asked about, when anything changed; the error is the write's.
    fn save(&self) -> Result<(), HostError>;
}

/// One file's entry as the cache file keeps it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CacheEntry {
    pub stamp: String,
    pub buckets: Vec<HistoryBucket>,
}

/// The cache over one JSON file (cache.ts:52). The file is read once, when this is made.
pub struct FileHistoryCache {
    pub path: PathBuf,
    pub stored: IndexMap<String, CacheEntry>,
    pub kept: RefCell<IndexMap<String, CacheEntry>>,
    pub changed: RefCell<bool>,
}

#[derive(Serialize)]
struct CacheFile<'a> {
    version: u32,
    parse: u32,
    files: &'a IndexMap<String, CacheEntry>,
}

/// A count as the cache file writes it: a whole number, not negative. zod's `int()` takes any such double, `1e20` too.
fn whole(v: &Value) -> Option<f64> {
    let n = v.as_f64()?;
    (n >= 0.0 && n.fract() == 0.0).then_some(n)
}

fn count(v: &Value) -> Option<u64> {
    whole(v).map(|n| n as u64)
}

/// A name-to-count record as zod's `z.record` rebuilds it: a `__proto__` key is assigned to the new object's prototype
/// slot, where a number does nothing, so it is left out; the keys then come in a JavaScript object's order.
fn counts(v: Option<&Value>) -> Option<IndexMap<String, u64>> {
    let kept: IndexMap<String, u64> = v?
        .as_object()?
        .iter()
        .map(|(name, c)| Some((name.clone(), count(c)?)))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .filter(|(name, _)| name != "__proto__")
        .collect();
    Some(js_key_order(kept))
}

/// One bucket as the cache file shape holds it; a `null` where the shape has an optional field is refused.
fn bucket_of(v: &Value) -> Option<HistoryBucket> {
    let folder = match v.get("folder") {
        None => None,
        Some(f) => Some(f.as_str()?.to_string()),
    };
    let at = match v.get("at") {
        None => None,
        Some(a) => Some(a.as_f64()?),
    };
    Some(HistoryBucket {
        session: v.get("session")?.as_str()?.to_string(),
        folder,
        calls: whole(v.get("calls")?)? as u128,
        commands: counts(v.get("commands"))?,
        installs: counts(v.get("installs"))?,
        at,
    })
}

/// The entries of a parsed cache file, or none when its version, parse version or shape is not this code's.
fn cache_entries(top: &Value) -> Option<IndexMap<String, CacheEntry>> {
    if top.get("version")?.as_f64()? != 1.0 || top.get("parse")?.as_f64()? != f64::from(PARSE_VERSION) {
        return None;
    }
    top.get("files")?
        .as_object()?
        .iter()
        .map(|(path, entry)| {
            let stamp = entry.get("stamp")?.as_str()?.to_string();
            let buckets = entry.get("buckets")?.as_array()?.iter().map(bucket_of).collect::<Option<Vec<_>>>()?;
            Some((path.clone(), CacheEntry { stamp, buckets }))
        })
        .collect()
}

/// The file's entries, or none when there is no file, it cannot be read, or its shape is not version 1 with this
/// PARSE_VERSION.
pub fn read_cache_file(path: &std::path::Path) -> IndexMap<String, CacheEntry> {
    std::fs::read_to_string(path).ok().and_then(|text| parse_json(&text).ok()).and_then(|top| cache_entries(&top)).unwrap_or_default()
}

pub fn file_history_cache(path: impl Into<PathBuf>) -> FileHistoryCache {
    let path = path.into();
    let stored = read_cache_file(&path);
    FileHistoryCache { path, stored, kept: RefCell::new(IndexMap::new()), changed: RefCell::new(false) }
}

/// A short random-looking name for the temp file beside the cache: six base-36 characters from the clock and the pid.
fn temp_suffix() -> String {
    let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    let mut n = u64::from(now.subsec_nanos()) ^ (u64::from(std::process::id()) << 20);
    (0..6)
        .map(|_| {
            let digit = char::from_digit((n % 36) as u32, 36).unwrap_or('0');
            n /= 36;
            digit
        })
        .collect()
}

impl HistoryCache for FileHistoryCache {
    fn get(&self, path: &str, stamp: &str) -> Option<Vec<HistoryBucket>> {
        let entry = self.stored.get(path).filter(|e| e.stamp == stamp)?;
        self.kept.borrow_mut().insert(path.to_string(), entry.clone());
        Some(entry.buckets.clone())
    }

    fn set(&self, path: &str, stamp: &str, buckets: &[HistoryBucket]) {
        self.kept.borrow_mut().insert(path.to_string(), CacheEntry { stamp: stamp.to_string(), buckets: buckets.to_vec() });
        *self.changed.borrow_mut() = true;
    }

    fn save(&self) -> Result<(), HostError> {
        let kept = self.kept.borrow();
        if !*self.changed.borrow() && kept.len() == self.stored.len() {
            return Ok(());
        }
        let out = CacheFile { version: 1, parse: PARSE_VERSION, files: &kept };
        let text = serde_json::to_string(&out).map_err(|e| HostError::new(e.to_string()))?;
        let dir = self.path.parent().unwrap_or(Path::new(""));
        std::fs::create_dir_all(dir).map_err(|e| node_error(&e, "mkdir", dir, None))?;
        let now_ms = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let tmp = dir.join(format!(".{now_ms}-{}.tmp", temp_suffix()));
        std::fs::write(&tmp, format!("{text}\n")).map_err(|e| node_error(&e, "open", &tmp, None))?;
        std::fs::rename(&tmp, &self.path).map_err(|e| node_error(&e, "rename", &tmp, Some(&self.path)))
    }
}

/// A failed system call as node words it: `EEXIST: file already exists, mkdir '/a/b'`, a rename naming both paths.
fn node_error(e: &std::io::Error, syscall: &str, path: &Path, dest: Option<&Path>) -> HostError {
    let (code, what) = match e.raw_os_error() {
        Some(libc::EEXIST) => ("EEXIST", "file already exists"),
        Some(libc::ENOENT) => ("ENOENT", "no such file or directory"),
        Some(libc::EACCES) => ("EACCES", "permission denied"),
        Some(libc::EPERM) => ("EPERM", "operation not permitted"),
        Some(libc::ENOTDIR) => ("ENOTDIR", "not a directory"),
        Some(libc::EISDIR) => ("EISDIR", "illegal operation on a directory"),
        Some(libc::ENOSPC) => ("ENOSPC", "no space left on device"),
        Some(libc::EROFS) => ("EROFS", "read-only file system"),
        Some(libc::EMFILE) => ("EMFILE", "too many open files"),
        Some(libc::ENAMETOOLONG) => ("ENAMETOOLONG", "name too long"),
        Some(libc::ENOTEMPTY) => ("ENOTEMPTY", "directory not empty"),
        Some(libc::EBUSY) => ("EBUSY", "resource busy or locked"),
        Some(libc::EXDEV) => ("EXDEV", "cross-device link not permitted"),
        Some(libc::ELOOP) => ("ELOOP", "too many symbolic links encountered"),
        Some(libc::EDQUOT) => ("EDQUOT", "disk quota exceeded"),
        Some(libc::EINVAL) => ("EINVAL", "invalid argument"),
        Some(libc::EIO) => ("EIO", "i/o error"),
        _ => return HostError::new(e.to_string()),
    };
    let to = dest.map_or(String::new(), |d| format!(" -> '{}'", d.display()));
    HostError::new(format!("{code}: {what}, {syscall} '{}'{to}", path.display()))
}
