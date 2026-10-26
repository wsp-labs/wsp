// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_secrets: slate-runs.ts:132-138, 197-208, 499-611 at 65f6b89c7. Spec: spec/runs_secrets.md.
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use std::io;
use std::path::Path;

use base64::engine::general_purpose::{STANDARD, URL_SAFE};
use base64::Engine;
use indexmap::{IndexMap, IndexSet};
use serde_json::{Map, Value};
use wsp_core::clock::Clock;
use wsp_store::own_file::write_own;
use wsp_words::exit::HostError;
use wsp_words::js::utf16;

use super::parts::SecretHandle;

/// Where "Always in this thread" is kept. The slate area backs it with the record; the default is memory.
pub trait RunApprovals {
    fn has(&self, thread_id: &str, key: &str) -> bool;
    fn allow(&self, thread_id: &str, key: &str);
    fn revoke(&self, thread_id: &str, key: &str);
    fn list(&self, thread_id: &str) -> Vec<String>;
}

/// The person typed it; an empty text clears it.
pub trait SlateSecrets {
    fn set(&self, thread_id: &str, name: &str, plaintext: &str, keep: bool) -> SecretHandle;
    /// As `set`, but a kept secret the host cannot save is an error, which refuses the typing.
    fn typed(&self, thread_id: &str, name: &str, plaintext: &str, keep: bool) -> Result<SecretHandle, HostError>;
    fn clear(&self, thread_id: &str, name: &str) -> SecretHandle;
    fn handle(&self, thread_id: &str, name: &str) -> SecretHandle;
    /// One secret, or with none every secret of the thread.
    fn forget(&self, thread_id: &str, name: Option<&str>);
    fn scrub(&self, thread_id: &str, text: &str) -> String;
    fn plaintext(&self, thread_id: &str, name: &str) -> Option<String>;
}

pub fn secret_forms(text: &str) -> Vec<String> {
    let mut forms: IndexSet<String> = IndexSet::new();
    forms.insert(text.to_string());
    let bytes = text.as_bytes();
    let std = STANDARD.encode(bytes);
    let url = URL_SAFE.encode(bytes);
    for f in [std.clone(), std.trim_end_matches('=').to_string(), url.clone(), url.trim_end_matches('=').to_string()] {
        forms.insert(f);
    }
    for shift in [0usize, 1, 2] {
        let mut padded = vec![0u8; shift];
        padded.extend_from_slice(bytes);
        let all = STANDARD.encode(&padded);
        let from = if shift == 0 { 0 } else { 4 };
        let to = ((shift + bytes.len()) / 3 * 4).min(all.len());
        if from >= to {
            continue;
        }
        let middle = &all[from..to];
        if middle.len() < 8 {
            continue;
        }
        forms.insert(middle.to_string());
        forms.insert(middle.replace('+', "-").replace('/', "_"));
    }
    let encoded = encode_uri_component(text);
    forms.insert(encoded.clone());
    forms.insert(lower_escapes(&encoded));
    forms.insert(encoded.replace("%20", "+"));
    forms.shift_remove("");
    forms.into_iter().collect()
}

fn lower_escapes(encoded: &str) -> String {
    let mut out = String::with_capacity(encoded.len());
    let mut chars = encoded.chars();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '%' {
            for _ in 0..2 {
                if let Some(h) = chars.next() {
                    out.push(h.to_ascii_lowercase());
                }
            }
        }
    }
    out
}

/// JS `encodeURIComponent` over a valid string: everything but `A-Za-z0-9-_.!~*'()` as `%XX` of its UTF-8 bytes,
/// uppercase hex.
pub fn encode_uri_component(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for &b in text.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[derive(Default)]
pub struct DefaultApprovals {
    allowed: RefCell<IndexMap<String, IndexSet<String>>>,
}

impl RunApprovals for DefaultApprovals {
    fn has(&self, thread_id: &str, key: &str) -> bool {
        self.allowed.borrow().get(thread_id).is_some_and(|keys| keys.contains(key))
    }
    fn allow(&self, thread_id: &str, key: &str) {
        self.allowed.borrow_mut().entry(thread_id.to_string()).or_default().insert(key.to_string());
    }
    fn revoke(&self, thread_id: &str, key: &str) {
        if let Some(keys) = self.allowed.borrow_mut().get_mut(thread_id) {
            keys.shift_remove(key);
        }
    }
    fn list(&self, thread_id: &str) -> Vec<String> {
        self.allowed.borrow().get(thread_id).map_or_else(Vec::new, |keys| keys.iter().cloned().collect())
    }
}

/// One held secret.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Kept {
    pub text: String,
    pub at: i64,
    pub keep: bool,
}

/// The secrets of every thread, and the file the kept ones live in.
pub struct Secrets {
    pub(crate) file: Option<PathBuf>,
    pub(crate) clock: Rc<dyn Clock>,
    pub(crate) held: RefCell<IndexMap<String, IndexMap<String, Kept>>>,
}

/// Reads the kept ones back from `file` at once.
pub fn create_secrets(file: Option<PathBuf>, clock: Rc<dyn Clock>) -> Rc<Secrets> {
    let mut held: IndexMap<String, IndexMap<String, Kept>> = IndexMap::new();
    if let Some(path) = file.as_ref().filter(|p| p.exists()) {
        if let Some(Value::Object(threads)) = std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok())
        {
            for (thread_id, names) in threads {
                let mut kept = IndexMap::new();
                if let Value::Object(names) = names {
                    for (name, k) in names {
                        let (Some(text), Some(at)) = (k.get("text").and_then(Value::as_str), k.get("at").and_then(Value::as_i64)) else {
                            break;
                        };
                        kept.insert(name, Kept { text: text.to_string(), at, keep: true });
                    }
                }
                held.insert(thread_id, kept);
            }
        }
    }
    Rc::new(Secrets { file, clock, held: RefCell::new(held) })
}

impl Secrets {
    /// Writes the kept secrets, compact, through `write_own`; nothing without a file.
    pub fn save(&self) -> Result<(), HostError> {
        let Some(file) = self.file.as_ref() else {
            return Ok(());
        };
        let mut out = Map::new();
        for (thread_id, names) in self.held.borrow().iter() {
            let mut kept = Map::new();
            for (name, k) in names.iter().filter(|(_, k)| k.keep) {
                kept.insert(name.clone(), serde_json::json!({ "text": k.text, "at": k.at }));
            }
            if !kept.is_empty() {
                out.insert(thread_id.clone(), Value::Object(kept));
            }
        }
        let (Some(dir), Some(name)) = (file.parent(), file.file_name().and_then(|n| n.to_str())) else {
            return Ok(());
        };
        write_own(dir, name, serde_json::to_string(&Value::Object(out)).unwrap_or_default()).map_err(|e| save_words(&e, file))
    }

    fn remove(&self, thread_id: &str, name: Option<&str>) -> Result<(), HostError> {
        let kept = {
            let mut held = self.held.borrow_mut();
            let Some(names) = held.get_mut(thread_id) else {
                return Ok(());
            };
            match name {
                None => {
                    let kept = names.values().any(|k| k.keep);
                    held.shift_remove(thread_id);
                    kept
                }
                Some(name) => {
                    let kept = names.get(name).is_some_and(|k| k.keep);
                    names.shift_remove(name);
                    kept
                }
            }
        };
        if kept {
            self.save()?;
        }
        Ok(())
    }

    fn held_kept(&self, thread_id: &str, name: &str) -> bool {
        self.held.borrow().get(thread_id).and_then(|names| names.get(name)).is_some_and(|k| k.keep)
    }
}

/// node's words for a write that failed, as writeOwn throws them; the rename names the file it writes over.
fn save_words(e: &io::Error, file: &Path) -> HostError {
    let (code, what) = match e.raw_os_error() {
        Some(libc::ENOENT) => ("ENOENT", "no such file or directory"),
        Some(libc::EACCES) => ("EACCES", "permission denied"),
        Some(libc::EPERM) => ("EPERM", "operation not permitted"),
        Some(libc::ENOTDIR) => ("ENOTDIR", "not a directory"),
        Some(libc::EISDIR) => ("EISDIR", "illegal operation on a directory"),
        Some(libc::ENOSPC) => ("ENOSPC", "no space left on device"),
        Some(libc::EROFS) => ("EROFS", "read-only file system"),
        _ => return HostError::new(e.to_string()),
    };
    HostError::new(format!("{code}: {what}, rename '{}'", file.display()))
}

impl SlateSecrets for Secrets {
    fn set(&self, thread_id: &str, name: &str, plaintext: &str, keep: bool) -> SecretHandle {
        self.typed(thread_id, name, plaintext, keep).unwrap_or_else(|_| self.handle(thread_id, name))
    }
    fn typed(&self, thread_id: &str, name: &str, plaintext: &str, keep: bool) -> Result<SecretHandle, HostError> {
        if plaintext.is_empty() {
            self.remove(thread_id, Some(name))?;
            return Ok(self.handle(thread_id, name));
        }
        let was = self.held_kept(thread_id, name);
        let at = self.clock.now();
        self.held
            .borrow_mut()
            .entry(thread_id.to_string())
            .or_default()
            .insert(name.to_string(), Kept { text: plaintext.to_string(), at, keep });
        if keep || was {
            self.save()?;
        }
        Ok(self.handle(thread_id, name))
    }
    fn clear(&self, thread_id: &str, name: &str) -> SecretHandle {
        self.forget(thread_id, Some(name));
        self.handle(thread_id, name)
    }
    fn handle(&self, thread_id: &str, name: &str) -> SecretHandle {
        match self.held.borrow().get(thread_id).and_then(|names| names.get(name)) {
            None => SecretHandle { secret: true, set: false, len: 0, at: None },
            Some(k) => SecretHandle { secret: true, set: true, len: utf16::len(&k.text) as i64, at: Some(k.at) },
        }
    }
    fn forget(&self, thread_id: &str, name: Option<&str>) {
        let _ = self.remove(thread_id, name);
    }
    fn scrub(&self, thread_id: &str, text: &str) -> String {
        let held = self.held.borrow();
        let Some(names) = held.get(thread_id) else {
            return text.to_string();
        };
        if text.is_empty() {
            return text.to_string();
        }
        let mut forms: Vec<(String, String)> = Vec::new();
        for (name, k) in names {
            for f in secret_forms(&k.text) {
                forms.push((f, format!("[secret:{name}]")));
            }
        }
        forms.sort_by_key(|f| std::cmp::Reverse(utf16::len(&f.0)));
        let mut out = text.to_string();
        for (form, mark) in &forms {
            if out.contains(form.as_str()) {
                out = out.replace(form.as_str(), mark);
            }
        }
        out
    }
    fn plaintext(&self, thread_id: &str, name: &str) -> Option<String> {
        self.held.borrow().get(thread_id).and_then(|names| names.get(name)).map(|k| k.text.clone())
    }
}
