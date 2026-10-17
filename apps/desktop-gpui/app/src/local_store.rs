// SPDX-License-Identifier: AGPL-3.0-only
//! The app's localStorage: string keys to JSON values in one file in the app's data folder. The values in memory are
//! the truth; each keeps its JSON beside it, so a change encodes only its own value, and the file is joined from those
//! and written atomically off the window's thread once the app runs, one write at a time with the newest winning. A missing file is an empty store, and an unreadable one is
//! kept beside it as `.corrupt` before it is replaced. Two copies of the app sharing one file each write their whole
//! store, so the last writer's keys win.

use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use gpui_kit::{App, BackgroundExecutor, Global};
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

const FOLDER: &str = "wsp-desktop-gpui";
const FILE: &str = "local-store.json";
/// The bytes the file may hold, as the web's localStorage gives the same keys about 5 MB and refuses a write past it.
pub const QUOTA: usize = 5 * 1024 * 1024;

/// Each key's value as the file writes it.
type Encoded = BTreeMap<String, Arc<str>>;

pub struct LocalStore {
    path: PathBuf,
    items: Map<String, Value>,
    encoded: Encoded,
    /// Where writes run once the app is up; none writes in place, as a test or a tool does.
    executor: Option<BackgroundExecutor>,
    writer: Arc<Writer>,
}

/// The snapshot waiting to be written, and the lock that keeps one write at a time.
#[derive(Default)]
struct Writer {
    pending: Mutex<Option<Encoded>>,
    writing: Mutex<()>,
}

impl Global for LocalStore {}

/// Reads the store at `path` into the app, where a view reaches it with `cx.global::<LocalStore>()`, its writes on the
/// app's background executor.
pub fn init(path: impl Into<PathBuf>, cx: &mut App) {
    let mut store = LocalStore::open(path);
    store.executor = Some(cx.background_executor().clone());
    cx.set_global(store);
}

/// `local-store.json` in the app's data folder: `~/Library/Application Support/wsp-desktop-gpui` on macOS,
/// `$XDG_DATA_HOME/wsp-desktop-gpui` or `~/.local/share/wsp-desktop-gpui` elsewhere.
pub fn default_path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default();
    let data = if cfg!(target_os = "macos") {
        home.join("Library/Application Support")
    } else {
        std::env::var_os("XDG_DATA_HOME").map(PathBuf::from).filter(|p| p.is_absolute()).unwrap_or_else(|| home.join(".local/share"))
    };
    data.join(FOLDER).join(FILE)
}

impl LocalStore {
    pub fn open(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        let read = fs::read(&path).ok();
        let items = read.as_deref().and_then(|bytes| match serde_json::from_slice::<Value>(bytes) {
            Ok(Value::Object(items)) => Some(items),
            _ => None,
        });
        if read.is_some() && items.is_none() {
            let corrupt = path.with_file_name(format!("{}.corrupt", path.file_name().and_then(|n| n.to_str()).unwrap_or(FILE)));
            if let Err(e) = fs::copy(&path, &corrupt) {
                eprintln!("desktop-gpui: store: keeping {}: {e}", corrupt.display());
            }
        }
        let items = items.unwrap_or_default();
        let encoded = items.iter().map(|(k, v)| (k.clone(), Arc::from(v.to_string()))).collect();
        LocalStore { path, items, encoded, executor: None, writer: Arc::default() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.items.get(key)
    }

    /// The value under `key` read as `T`; one of another shape reads as none.
    pub fn get_as<T: DeserializeOwned>(&self, key: &str) -> Option<T> {
        self.get(key).and_then(|v| T::deserialize(v).ok())
    }

    /// Keeps `value` under `key` and writes the file. A store past its quota refuses and is left as it was; a write
    /// that fails on disk leaves the value kept in memory.
    pub fn set(&mut self, key: &str, value: Value) -> io::Result<()> {
        let old = self.items.insert(key.to_owned(), value);
        self.commit(key, old)
    }

    pub fn remove(&mut self, key: &str) -> io::Result<()> {
        match self.items.remove(key) {
            Some(old) => self.commit(key, Some(old)),
            None => Ok(()),
        }
    }

    fn commit(&mut self, key: &str, old: Option<Value>) -> io::Result<()> {
        let mut encoded = self.encoded.clone();
        match self.items.get(key) {
            Some(value) => encoded.insert(key.to_owned(), Arc::from(value.to_string())),
            None => encoded.remove(key),
        };
        let size = file_size(&encoded);
        if size > QUOTA {
            match old {
                Some(old) => self.items.insert(key.to_owned(), old),
                None => self.items.remove(key),
            };
            return Err(io::Error::new(io::ErrorKind::StorageFull, format!("the store would hold {size} bytes, past its {QUOTA}")));
        }
        self.encoded = encoded;
        *self.writer.pending.lock().unwrap_or_else(|e| e.into_inner()) = Some(self.encoded.clone());
        let (writer, path) = (self.writer.clone(), self.path.clone());
        match &self.executor {
            Some(executor) => {
                executor
                    .spawn(async move {
                        if let Err(e) = writer.write_newest(&path) {
                            eprintln!("desktop-gpui: store: {}: {e}", path.display());
                        }
                    })
                    .detach();
                Ok(())
            }
            None => writer.write_newest(&path),
        }
    }
}

impl Writer {
    /// Writes the newest snapshot waiting, if one still is, once any write before it has finished.
    fn write_newest(&self, path: &Path) -> io::Result<()> {
        let _one = self.writing.lock().unwrap_or_else(|e| e.into_inner());
        let Some(encoded) = self.pending.lock().unwrap_or_else(|e| e.into_inner()).take() else { return Ok(()) };
        write_atomic(path, join(&encoded).as_bytes())
    }
}

/// The file's JSON object, joined from each key's encoded value.
fn join(encoded: &Encoded) -> String {
    let mut out = String::with_capacity(file_size(encoded));
    out.push('{');
    for (i, (key, value)) in encoded.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&Value::from(key.as_str()).to_string());
        out.push(':');
        out.push_str(value);
    }
    out.push('}');
    out
}

/// The bytes `join` writes for `encoded`, counted without joining them.
fn file_size(encoded: &Encoded) -> usize {
    let entries: usize = encoded.iter().map(|(k, v)| Value::from(k.as_str()).to_string().len() + 1 + v.len()).sum();
    2 + entries + encoded.len().saturating_sub(1)
}

/// A temp file beside `path`, synced, renamed over it, and the folder synced so the rename survives a crash.
fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(dir)?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or(FILE);
    // The pid keeps two copies of the app from writing one temp file.
    let temp = path.with_file_name(format!(".{name}.{}.tmp", std::process::id()));
    let written = (|| {
        let mut file = fs::File::create(&temp)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        fs::rename(&temp, path)?;
        fs::File::open(dir)?.sync_all()
    })();
    if written.is_err() {
        let _ = fs::remove_file(&temp);
    }
    written
}

/// A fresh path under the system's temp folder for one test's store.
#[cfg(test)]
pub(crate) fn scratch(name: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!("wsp-local-store-{}-{}-{name}", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
    let _ = fs::remove_dir_all(&dir);
    dir.join(FILE)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_kit::BorrowAppContext as _;
    use serde_json::json;

    fn leftovers(path: &Path) -> Vec<String> {
        fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .filter(|n| n.ends_with(".tmp"))
            .collect()
    }

    #[test]
    fn a_value_set_reads_back_after_a_reopen() {
        let path = scratch("round-trip");
        let mut store = LocalStore::open(&path);
        assert!(store.get("width").is_none());
        store.set("width", json!(320)).unwrap();
        store.set("names", json!(["a", "b"])).unwrap();
        let store = LocalStore::open(&path);
        assert_eq!(store.get("width"), Some(&json!(320)));
        assert_eq!(store.get_as::<u32>("width"), Some(320));
        assert_eq!(store.get_as::<Vec<String>>("names"), Some(vec!["a".to_owned(), "b".to_owned()]));
        assert_eq!(store.get_as::<String>("width"), None);
    }

    #[test]
    fn a_missing_or_corrupt_file_reads_empty() {
        let path = scratch("corrupt");
        assert!(LocalStore::open(&path).get("x").is_none());
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"{\"x\": 1").unwrap();
        assert!(LocalStore::open(&path).get("x").is_none());
        fs::write(&path, b"[1, 2]").unwrap();
        assert!(LocalStore::open(&path).get("x").is_none());
        let mut store = LocalStore::open(&path);
        store.set("x", json!(2)).unwrap();
        assert_eq!(LocalStore::open(&path).get("x"), Some(&json!(2)));
        let corrupt = path.with_file_name("local-store.json.corrupt");
        assert_eq!(fs::read(&corrupt).unwrap(), b"[1, 2]", "the unreadable file is kept before it is replaced");
    }

    #[test]
    fn a_write_leaves_no_temp_file_and_a_failed_one_keeps_the_value_in_memory() {
        let path = scratch("atomic");
        let mut store = LocalStore::open(&path);
        store.set("a", json!("one")).unwrap();
        assert!(leftovers(&path).is_empty());

        // A folder where the file goes makes the rename fail.
        fs::remove_file(&path).unwrap();
        fs::create_dir_all(path.join("blocker")).unwrap();
        assert!(store.set("a", json!("two")).is_err());
        assert_eq!(store.get("a"), Some(&json!("two")), "memory is the truth");
        assert!(leftovers(&path).is_empty());
        assert!(path.is_dir(), "the failed write left what was there");
    }

    #[gpui_kit::test]
    fn writes_run_off_the_windows_thread_and_the_newest_wins(cx: &mut gpui_kit::TestAppContext) {
        let path = scratch("background");
        cx.update(|cx| init(path.clone(), cx));
        cx.update(|cx| {
            cx.update_global(|s: &mut LocalStore, _| {
                s.set("draft", json!("one")).unwrap();
                s.set("draft", json!("two")).unwrap();
            })
        });
        assert!(!path.exists(), "nothing is written on the window's thread");
        cx.run_until_parked();
        assert_eq!(LocalStore::open(&path).get("draft"), Some(&json!("two")));
    }

    #[test]
    fn a_removed_key_stays_removed_after_a_reopen() {
        let path = scratch("remove");
        let mut store = LocalStore::open(&path);
        store.set("a", json!(1)).unwrap();
        store.set("b", json!(2)).unwrap();
        store.remove("a").unwrap();
        store.remove("never").unwrap();
        let store = LocalStore::open(&path);
        assert!(store.get("a").is_none());
        assert_eq!(store.get("b"), Some(&json!(2)));
    }

    #[test]
    fn a_write_past_the_quota_is_refused_and_changes_nothing() {
        let path = scratch("quota");
        let mut store = LocalStore::open(&path);
        store.set("a", json!("kept")).unwrap();
        assert!(store.set("big", json!("x".repeat(5 * 1024 * 1024))).is_err());
        assert!(store.get("big").is_none());
        let reopened = LocalStore::open(&path);
        assert_eq!(reopened.get("a"), Some(&json!("kept")));
        assert!(reopened.get("big").is_none());
    }

    #[test]
    fn the_size_counted_is_the_files_size() {
        let path = scratch("size");
        let mut store = LocalStore::open(&path);
        for (key, value) in [("a", json!(1)), ("b \"quoted\"", json!(["x", "y"])), ("a", json!("longer")), ("c", json!({"k": 2}))] {
            store.set(key, value).unwrap();
            assert_eq!(file_size(&store.encoded), fs::read(&path).unwrap().len(), "after {key}");
        }
        store.remove("b \"quoted\"").unwrap();
        assert_eq!(file_size(&store.encoded), fs::read(&path).unwrap().len());
        assert_eq!(LocalStore::open(&path).get("a"), Some(&json!("longer")));
    }

    #[test]
    fn the_default_path_ends_in_the_apps_folder() {
        let path = default_path();
        assert!(path.ends_with("wsp-desktop-gpui/local-store.json"), "{path:?}");
    }
}
