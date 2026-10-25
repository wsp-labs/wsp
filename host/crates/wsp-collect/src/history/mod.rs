// SPDX-License-Identifier: AGPL-3.0-only
//! The registry: one reader per session store format the catalog names, and the reads over every agent's store. Port
//! of packages/collect/src/history/index.ts at a3b870a6e; the spec is spec/history_store.md.
pub mod cache;
pub mod claude;
pub mod codex;
pub mod commands;
pub mod hermes;
pub mod reader;
pub mod tally;

use std::cell::{Cell, RefCell};

use wsp_catalog::catalog::{AgentEntry, HistoryFormat};
use wsp_wire::views::golden_image::RecipeHistoryState;
use wsp_words::exit::HostError;

use crate::history::cache::HistoryCache;
use crate::history::claude::ClaudeReader;
use crate::history::codex::CodexReader;
use crate::history::hermes::HermesReader;
use crate::history::reader::{HistoryFile, HistoryReader};
use crate::history::tally::{one_holder, reduce_calls, usage_of, Held, Usage};
use crate::host::{expand, Host};

/// How many of a store's files are read at once (index.ts:39).
pub const FILES_AT_ONCE: usize = 4;

/// `HISTORY_READERS[format]` (index.ts:16).
pub fn history_reader(format: HistoryFormat) -> &'static dyn HistoryReader {
    match format {
        HistoryFormat::ClaudeJsonl => &ClaudeReader,
        HistoryFormat::CodexRollout => &CodexReader,
        HistoryFormat::HermesSqlite => &HermesReader,
    }
}

/// index.ts:22: the recipe's history line for one agent, with the counts it kept.
#[derive(Debug, Clone, PartialEq)]
pub struct AgentHistory {
    pub agent: String,
    pub state: RecipeHistoryState,
    pub sessions: u64,
    pub calls: u64,
    pub usage: Usage,
}

/// index.ts:28.
#[derive(Default)]
pub struct StoreOptions<'a> {
    /// Absolute folders the counts are weighed against.
    pub folders: Option<&'a [String]>,
    pub cache: Option<&'a dyn HistoryCache>,
    /// Told `(read, files)` as each of the store's files lands.
    pub on_file: Option<&'a dyn Fn(usize, usize)>,
}

/// An `await` in JavaScript always lets the other workers run before it resumes, even on a settled promise.
async fn yield_once() {
    let mut yielded = false;
    futures::future::poll_fn(|cx| {
        if yielded {
            return std::task::Poll::Ready(());
        }
        yielded = true;
        cx.waker().wake_by_ref();
        std::task::Poll::Pending
    })
    .await
}

/// One store's read, shared by its workers: the next file to take, how many have landed, and what each came to.
struct StoreRead<'a> {
    host: &'a Host,
    reader: &'a dyn HistoryReader,
    root: &'a str,
    files: &'a [HistoryFile],
    opts: &'a StoreOptions<'a>,
    next: Cell<usize>,
    read: Cell<usize>,
    held: RefCell<Vec<Option<Held>>>,
}

impl StoreRead<'_> {
    /// One of the store's workers: takes the next file until none is left, or until one of them fails.
    async fn work(&self) -> Result<(), HostError> {
        let (host, reader, root, files, opts) = (self.host, self.reader, self.root, self.files, self.opts);
        loop {
            let i = self.next.get();
            self.next.set(i + 1);
            if i >= files.len() {
                return Ok(());
            }
            let f = &files[i];
            let buckets = match opts.cache.and_then(|c| c.get(&f.path, &f.stamp)) {
                Some(buckets) => buckets,
                None => {
                    let said = reduce_calls(reader.read(host, root, &f.path)).await;
                    yield_once().await;
                    let buckets = said?;
                    if let Some(cache) = opts.cache {
                        cache.set(&f.path, &f.stamp, &buckets);
                    }
                    buckets
                }
            };
            self.held.borrow_mut()[i] = Some(Held { holder: reader.holder(root, &f.path), buckets });
            self.read.set(self.read.get() + 1);
            if let Some(on_file) = opts.on_file {
                on_file(self.read.get(), files.len());
            }
        }
    }
}

/// What one agent's store came to: each file from the cache when its stamp matches, else read, FILES_AT_ONCE at a
/// time; the first file that cannot be read ends the store with its error once the reads already open settle.
pub async fn read_store(host: &Host, reader: &dyn HistoryReader, root: &str, opts: StoreOptions<'_>) -> Result<Usage, HostError> {
    let files = reader.files(host, root).await;
    let n = files.len();
    let store = StoreRead {
        host,
        reader,
        root,
        files: &files,
        opts: &opts,
        next: Cell::new(0),
        read: Cell::new(0),
        held: RefCell::new(vec![None; n]),
    };
    let store = &store;
    let ran = futures::future::join_all((0..n.min(FILES_AT_ONCE)).map(|_| async move {
        let result = store.work().await;
        if result.is_err() {
            store.next.set(n);
        }
        result
    }))
    .await;
    if let Some(error) = ran.into_iter().find_map(Result::err) {
        return Err(error);
    }
    let held: Vec<Held> = store.held.take().into_iter().flatten().collect();
    Ok(usage_of(&one_holder(&held), opts.folders))
}

/// index.ts:72.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryProgress {
    pub agent: String,
    pub read: usize,
    pub files: usize,
}

/// index.ts:80.
#[derive(Default)]
pub struct HistoryOptions<'a> {
    pub on_agent: Option<&'a dyn Fn(&AgentHistory)>,
    pub folders: Option<&'a [String]>,
    pub cache: Option<&'a dyn HistoryCache>,
    pub on_progress: Option<&'a dyn Fn(HistoryProgress)>,
}

/// What each agent's session store says it used, in the agents' order: read, empty, unreadable, or no reader. The
/// cache is saved once after the last agent; its write error is the only error.
pub async fn read_histories(host: &Host, agents: &[&AgentEntry], opts: HistoryOptions<'_>) -> Result<Vec<AgentHistory>, HostError> {
    let mut out = Vec::new();
    for a in agents {
        let history = match &a.history {
            None => {
                AgentHistory { agent: a.id.clone(), state: RecipeHistoryState::NoReader, sessions: 0, calls: 0, usage: Usage::default() }
            }
            Some(store) => {
                let progress = |read: usize, files: usize| {
                    if let Some(on_progress) = opts.on_progress {
                        on_progress(HistoryProgress { agent: a.id.clone(), read, files });
                    }
                };
                let store_opts = StoreOptions { folders: opts.folders, cache: opts.cache, on_file: Some(&progress) };
                match read_store(host, history_reader(store.format), &expand(&host.home, &store.root), store_opts).await {
                    Ok(usage) => AgentHistory {
                        agent: a.id.clone(),
                        state: if usage.sessions == 0 { RecipeHistoryState::Empty } else { RecipeHistoryState::Read },
                        sessions: usage.sessions,
                        calls: usage.calls,
                        usage,
                    },
                    Err(_) => AgentHistory {
                        agent: a.id.clone(),
                        state: RecipeHistoryState::Unreadable,
                        sessions: 0,
                        calls: 0,
                        usage: Usage::default(),
                    },
                }
            }
        };
        if let Some(on_agent) = opts.on_agent {
            on_agent(&history);
        }
        out.push(history);
    }
    if let Some(cache) = opts.cache {
        cache.save()?;
    }
    Ok(out)
}
