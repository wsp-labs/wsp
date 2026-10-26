// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/github-cache.ts at 65f6b89c7: what the host keeps of its reads off a git host, by a
//! key the reader names. Spec: spec/host_files.md.
use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use serde_json::Value;

/// One read kept: the body it answered, the tag the next read compares (an ETag, or the reader's own words), and when
/// it was read, epoch ms. github-cache.ts:6.
#[derive(Clone, Debug, PartialEq)]
pub struct GitHubCacheEntry {
    pub body: Value,
    pub tag: Option<String>,
    pub fetched_at: i64,
}

/// github-cache.ts:12. A store that outlives the host implements the same three calls.
pub trait GitHubCache {
    fn get(&self, key: &str) -> Option<GitHubCacheEntry>;
    fn set(&self, key: &str, entry: GitHubCacheEntry);
    fn delete(&self, key: &str);
}

/// The in-memory cache: held for this host's life, nothing written anywhere. github-cache.ts:18.
#[derive(Default)]
pub struct MemoryGitHubCache {
    held: RefCell<IndexMap<String, GitHubCacheEntry>>,
}

impl GitHubCache for MemoryGitHubCache {
    fn get(&self, key: &str) -> Option<GitHubCacheEntry> {
        self.held.borrow().get(key).cloned()
    }

    fn set(&self, key: &str, entry: GitHubCacheEntry) {
        self.held.borrow_mut().insert(key.to_string(), entry);
    }

    fn delete(&self, key: &str) {
        self.held.borrow_mut().shift_remove(key);
    }
}

/// github-cache.ts:18.
pub fn memory_github_cache() -> Rc<dyn GitHubCache> {
    Rc::new(MemoryGitHubCache::default())
}
