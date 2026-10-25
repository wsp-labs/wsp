// SPDX-License-Identifier: AGPL-3.0-only
//! Unit runs_secrets: slate-runs.ts:132-138, 197-208, 499-611 at 65f6b89c7. Spec: spec/runs_secrets.md.
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use wsp_core::clock::Clock;

use super::parts::SecretHandle;

// slate-runs.ts:132
// /** Where "Always in this thread" is kept. The host backs it with the slate record's consents; the default is memory. */
// export interface RunApprovals {
//   has(threadId: string, key: string): boolean;
//   allow(threadId: string, key: string): void;
//   revoke(threadId: string, key: string): void;
//   list(threadId: string): string[];
// }
/// Where "Always in this thread" is kept. The slate area backs it with the record; the default is memory.
pub trait RunApprovals {
    fn has(&self, thread_id: &str, key: &str) -> bool;
    fn allow(&self, thread_id: &str, key: &str);
    fn revoke(&self, thread_id: &str, key: &str);
    fn list(&self, thread_id: &str) -> Vec<String>;
}

// slate-runs.ts:197
// export interface SlateSecrets {
//   /** The person typed it. An empty text clears it. */
//   set(threadId: string, name: string, plaintext: string, opts?: { keep?: boolean }): SecretHandle;
//   clear(threadId: string, name: string): SecretHandle;
//   handle(threadId: string, name: string): SecretHandle;
//   /** One secret, or every secret of the thread: a dropped declaration, "Forget secrets", a deleted thread. */
//   forget(threadId: string, name?: string): void;
//   /** Every filled secret of the thread replaced by `[secret:name]`, exact, base64 and URL-encoded. */
//   scrub(threadId: string, text: string): string;
//   /** The plaintext, for a tool run's arguments at the moment of the call and nowhere else. */
//   plaintext(threadId: string, name: string): string | undefined;
// }
pub trait SlateSecrets {
    /// The person typed it; an empty text clears it.
    fn set(&self, thread_id: &str, name: &str, plaintext: &str, keep: bool) -> SecretHandle;
    fn clear(&self, thread_id: &str, name: &str) -> SecretHandle;
    fn handle(&self, thread_id: &str, name: &str) -> SecretHandle;
    /// One secret, or with none every secret of the thread.
    fn forget(&self, thread_id: &str, name: Option<&str>);
    fn scrub(&self, thread_id: &str, text: &str) -> String;
    fn plaintext(&self, thread_id: &str, name: &str) -> Option<String>;
}

// slate-runs.ts:499
// /** Every text a command could print a secret as: itself, base64 (standard and URL-safe, padded or not, and the
//  * stable middle of the secret inside a longer base64 run such as a Basic header), and URL-encoded. */
// function secretForms(text: string): string[] {
//   const forms = new Set<string>([text]);
//   const bytes = Buffer.from(text, "utf8");
//   const b64 = (b: Buffer): string[] => {
//     const std = b.toString("base64");
//     const url = std.replace(/\+/g, "-").replace(/\//g, "_");
//     return [std, std.replace(/=+$/, ""), url, url.replace(/=+$/, "")];
//   };
//   for (const f of b64(bytes)) forms.add(f);
//   for (const shift of [0, 1, 2]) {
//     const all = Buffer.concat([Buffer.alloc(shift), bytes]).toString("base64");
//     const from = shift === 0 ? 0 : 4;
//     const to = Math.floor((shift + bytes.length) / 3) * 4;
//     const middle = all.slice(from, to);
//     if (middle.length < 8) continue;
//     forms.add(middle);
//     forms.add(middle.replace(/\+/g, "-").replace(/\//g, "_"));
//   }
//   const url = encodeURIComponent(text);
//   forms.add(url);
//   forms.add(url.replace(/%[0-9A-F]{2}/g, m => m.toLowerCase()));
//   forms.add(url.replace(/%20/g, "+"));
//   forms.delete("");
//   return [...forms];
// }
pub fn secret_forms(text: &str) -> Vec<String> {
    todo!()
}

/// JS `encodeURIComponent` over a valid string: everything but `A-Za-z0-9-_.!~*'()` as `%XX` of its UTF-8 bytes,
/// uppercase hex.
pub fn encode_uri_component(text: &str) -> String {
    todo!()
}

// slate-runs.ts:527
// class DefaultApprovals implements RunApprovals {
//   private readonly allowed = new Map<string, Set<string>>();
//   has(threadId: string, key: string): boolean {
//     return this.allowed.get(threadId)?.has(key) ?? false;
//   }
//   allow(threadId: string, key: string): void {
//     const keys = this.allowed.get(threadId) ?? new Set<string>();
//     keys.add(key);
//     this.allowed.set(threadId, keys);
//   }
//   revoke(threadId: string, key: string): void {
//     this.allowed.get(threadId)?.delete(key);
//   }
//   list(threadId: string): string[] {
//     return [...(this.allowed.get(threadId) ?? [])];
//   }
// }
#[derive(Default)]
pub struct DefaultApprovals {
    allowed: RefCell<IndexMap<String, IndexSet<String>>>,
}

impl RunApprovals for DefaultApprovals {
    fn has(&self, thread_id: &str, key: &str) -> bool {
        todo!()
    }
    fn allow(&self, thread_id: &str, key: &str) {
        todo!()
    }
    fn revoke(&self, thread_id: &str, key: &str) {
        todo!()
    }
    fn list(&self, thread_id: &str) -> Vec<String> {
        todo!()
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

// slate-runs.ts:545
// function createSecrets(file: string | undefined, now: () => number): SlateSecrets {
//   const held = new Map<string, Map<string, Kept>>();
//   if (file !== undefined && existsSync(file)) {
//     try {
//       const read = JSON.parse(readFileSync(file, "utf8")) as Record<string, Record<string, { text: string; at: number }>>;
//       for (const [threadId, names] of Object.entries(read)) {
//         held.set(threadId, new Map(Object.entries(names).map(([name, k]) => [name, { text: k.text, at: k.at, keep: true }])));
//       }
//     } catch {
//       // An unreadable file holds nothing this host can use; the slates ask for their secrets again.
//     }
//   }
//   const save = (): void => {
//     if (file === undefined) return;
//     const out: Record<string, Record<string, { text: string; at: number }>> = {};
//     for (const [threadId, names] of held) {
//       for (const [name, k] of names) if (k.keep) (out[threadId] ??= {})[name] = { text: k.text, at: k.at };
//     }
//     writeOwn(dirname(file), basename(file), JSON.stringify(out));
//   };
//   const handle = (threadId: string, name: string): SecretHandle => {
//     const k = held.get(threadId)?.get(name);
//     return k === undefined ? { secret: true, set: false, len: 0, at: null } : { secret: true, set: true, len: k.text.length, at: k.at };
//   };
//   const forget = (threadId: string, name?: string): void => {
//     const names = held.get(threadId);
//     if (names === undefined) return;
//     const kept = name === undefined ? [...names.values()].some(k => k.keep) : names.get(name)?.keep === true;
//     if (name === undefined) held.delete(threadId);
//     else names.delete(name);
//     if (kept) save();
//   };
/// Reads the kept ones back from `file` at once.
pub fn create_secrets(file: Option<PathBuf>, clock: Rc<dyn Clock>) -> Rc<Secrets> {
    todo!()
}

impl Secrets {
    /// Writes the kept secrets, compact, through `write_own`; nothing without a file.
    pub fn save(&self) {
        todo!()
    }
}

// slate-runs.ts:577
//   return {
//     set(threadId, name, plaintext, opts) {
//       if (plaintext === "") {
//         forget(threadId, name);
//         return handle(threadId, name);
//       }
//       const names = held.get(threadId) ?? new Map<string, Kept>();
//       const was = names.get(name)?.keep === true;
//       const keep = opts?.keep === true;
//       names.set(name, { text: plaintext, at: now(), keep });
//       held.set(threadId, names);
//       if (keep || was) save();
//       return handle(threadId, name);
//     },
//     clear(threadId, name) {
//       forget(threadId, name);
//       return handle(threadId, name);
//     },
//     handle,
//     forget,
//     scrub(threadId, text) {
//       const names = held.get(threadId);
//       if (names === undefined || text === "") return text;
//       const forms: [string, string][] = [];
//       for (const [name, k] of names) for (const f of secretForms(k.text)) forms.push([f, `[secret:${name}]`]);
//       forms.sort((a, b) => b[0].length - a[0].length);
//       let out = text;
//       for (const [form, mark] of forms) if (out.includes(form)) out = out.split(form).join(mark);
//       return out;
//     },
//     plaintext(threadId, name) {
//       return held.get(threadId)?.get(name)?.text;
//     },
//   };
impl SlateSecrets for Secrets {
    fn set(&self, thread_id: &str, name: &str, plaintext: &str, keep: bool) -> SecretHandle {
        todo!()
    }
    fn clear(&self, thread_id: &str, name: &str) -> SecretHandle {
        todo!()
    }
    fn handle(&self, thread_id: &str, name: &str) -> SecretHandle {
        todo!()
    }
    fn forget(&self, thread_id: &str, name: Option<&str>) {
        todo!()
    }
    fn scrub(&self, thread_id: &str, text: &str) -> String {
        todo!()
    }
    fn plaintext(&self, thread_id: &str, name: &str) -> Option<String> {
        todo!()
    }
}
