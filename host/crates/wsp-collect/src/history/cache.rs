// SPDX-License-Identifier: AGPL-3.0-only
//! What each session file came to last time, kept in one JSON so a second run reads only the files that changed. A
//! file is trusted while its stamp and the parse version behind its counts both match. Port of
//! packages/collect/src/history/cache.ts at a3b870a6e; the spec is spec/history_store.md.
use std::cell::RefCell;
use std::path::PathBuf;

use indexmap::IndexMap;
use wsp_words::exit::HostError;

use crate::history::tally::HistoryBucket;

/// cache.ts:15. `&self` throughout: a cache is shared by the reads one store runs at once, on one thread.
pub trait HistoryCache {
    /// The buckets kept for this file when its stamp still matches; a hit keeps the entry for the next save.
    fn get(&self, path: &str, stamp: &str) -> Option<Vec<HistoryBucket>>;
    fn set(&self, path: &str, stamp: &str, buckets: &[HistoryBucket]);
    /// Writes what this run asked about, when anything changed; the error is the write's.
    fn save(&self) -> Result<(), HostError>;
}

/// One file's entry as the cache file keeps it.
#[derive(Debug, Clone, PartialEq)]
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

/// The file's entries, or none when there is no file, it cannot be read, or its shape is not version 1 with this
/// PARSE_VERSION.
// history/cache.ts:24
// const Bucket = z.object({
//   session: z.string(),
//   folder: z.string().optional(),
//   at: z.number().optional(),
//   calls: z.number().int().nonnegative(),
//   commands: z.record(z.number().int().nonnegative()),
//   installs: z.record(z.number().int().nonnegative()),
// });
//
// /** Version 1 and nothing else is read: a file this version does not know is one whole re-read, never half-trusted counts.
//  * `parse` is that same rule for the code that filled the buckets, which a file's stamp cannot see. */
// const CacheFile = z.object({ version: z.literal(1), parse: z.literal(PARSE_VERSION), files: z.record(z.object({ stamp: z.string(), buckets: z.array(Bucket) })) });
// type CacheFile = z.infer<typeof CacheFile>;
//
// type Entry = CacheFile["files"][string];
//
// /** The file's entries, or none when there is no file, it cannot be read, or its shape is not this version's. */
// function read(path: string): Map<string, Entry> {
//   let parsed: unknown;
//   try {
//     parsed = JSON.parse(readFileSync(path, "utf8"));
//   } catch {
//     return new Map();
//   }
//   const r = CacheFile.safeParse(parsed);
//   return r.success ? new Map(Object.entries(r.data.files)) : new Map();
// }
//
pub fn read_cache_file(path: &std::path::Path) -> IndexMap<String, CacheEntry> {
    todo!()
}

// history/cache.ts:52
// export function fileHistoryCache(path: string): HistoryCache {
//   const stored = read(path);
//   const kept = new Map<string, Entry>();
//   let changed = false;
//   return {
pub fn file_history_cache(path: impl Into<PathBuf>) -> FileHistoryCache {
    todo!()
}

impl HistoryCache for FileHistoryCache {
    // history/cache.ts:57
    //     get(file, stamp) {
    //       const e = stored.get(file);
    //       if (e === undefined || e.stamp !== stamp) return undefined;
    //       kept.set(file, e);
    //       return e.buckets;
    //     },
    fn get(&self, path: &str, stamp: &str) -> Option<Vec<HistoryBucket>> {
        todo!()
    }

    // history/cache.ts:63
    //     set(file, stamp, buckets) {
    //       kept.set(file, { stamp, buckets: [...buckets] });
    //       changed = true;
    //     },
    fn set(&self, path: &str, stamp: &str, buckets: &[HistoryBucket]) {
        todo!()
    }

    // history/cache.ts:67
    //     save() {
    //       if (!changed && kept.size === stored.size) return;
    //       const out: CacheFile = { version: 1, parse: PARSE_VERSION, files: Object.fromEntries(kept) };
    //       mkdirSync(dirname(path), { recursive: true });
    //       // Write-through with rename, as the state file beside it does: a wsp init and an MCP scan over one state folder
    //       // otherwise leave a torn file, which reads as no cache and costs the whole read this one exists to remove.
    //       const tmp = join(dirname(path), `.${Date.now()}-${Math.random().toString(36).slice(2, 8)}.tmp`);
    //       writeFileSync(tmp, `${JSON.stringify(out)}\n`);
    //       renameSync(tmp, path);
    //     },
    fn save(&self) -> Result<(), HostError> {
        todo!()
    }
}
