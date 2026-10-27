// SPDX-License-Identifier: AGPL-3.0-only
//! stub-backend.ts:460-490 `testPlatform`, `fakeLocal` and `copyingFake`: this computer as a whole-runtime test wires
//! it. A `LocalBackend` rooted in a folder of the test's own, so a project here is a real folder and the exec that reads
//! whether it is a repo runs where the test can see it; a copy road that copies the folder for real and takes it away.
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use indexmap::IndexMap;
use wsp_core::local_exec::{local_exec_stream, LocalExecOptions};
use wsp_core::types::wiring::{LocalWiring, Platform};
use wsp_engine::copier::{
    fake_copier, Copier, CopyAsk, CopyReport, CopyRoad, FakeCopier, FakeCopierLog, WorktreeAsk, WorktreeAt, WorktreeProject,
    WorktreeRemoval, WorktreeRemove, WorktreeReport,
};
use wsp_engine::errors::Result;
use wsp_engine::local_backend::{LocalBackend, LocalBackendOptions, MemoryHere};
use wsp_wire::views::workspace_more::Carried;

pub fn test_platform() -> Platform {
    if cfg!(target_os = "macos") {
        Platform::Darwin
    } else {
        Platform::Linux
    }
}

/// `cp -r from to`: a folder copied whole, a file or a link as it stands.
pub fn copy_tree(from: &Path, to: &Path) {
    let meta = std::fs::symlink_metadata(from).unwrap_or_else(|e| panic!("copy from {}: {e}", from.display()));
    if meta.is_dir() {
        std::fs::create_dir_all(to).expect("a folder");
        for entry in std::fs::read_dir(from).expect("a listing").flatten() {
            copy_tree(&entry.path(), &to.join(entry.file_name()));
        }
    } else if meta.file_type().is_symlink() {
        let target = std::fs::read_link(from).expect("a link");
        let _ = std::os::unix::fs::symlink(target, to);
    } else {
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).expect("a folder");
        }
        std::fs::copy(from, to).expect("a copied file");
    }
}

/// `copyingFake()`: the engine's fake, which records every ask, over a folder copied for real.
pub struct CopyingFake {
    inner: FakeCopier,
}

pub fn copying_fake() -> (Rc<CopyingFake>, Arc<std::sync::Mutex<FakeCopierLog>>) {
    let script: wsp_engine::copier::CopyScript = Arc::new(|ask: &CopyAsk| {
        copy_tree(Path::new(&ask.from), Path::new(&ask.to));
        CopyReport {
            road: ask.road.unwrap_or(CopyRoad::Clonefile),
            path: ask.to.clone(),
            base: "0".repeat(40),
            branch: "main".into(),
            fetched: true,
            carried: Carried::DepsAndConfig,
            excluded: ask.exclude.clone(),
            skipped: None,
            bytes: 1024.into(),
            ms: 1.into(),
            fell_back: None,
        }
    });
    let inner = fake_copier(Some(script), None);
    let log = inner.log.clone();
    (Rc::new(CopyingFake { inner }), log)
}

#[async_trait(?Send)]
impl Copier for CopyingFake {
    async fn make(&self, ask: &CopyAsk) -> Result<CopyReport> {
        self.inner.make(ask).await
    }
    async fn remove(&self, from: &str, to: &str, road: CopyRoad) -> Result<()> {
        self.inner.remove(from, to, road).await?;
        let _ = std::fs::remove_dir_all(to);
        Ok(())
    }
    async fn worktree(&self, ask: &WorktreeAsk) -> Result<WorktreeReport> {
        self.inner.worktree(ask).await
    }
    async fn worktree_mount(&self, o: &WorktreeAt) -> Result<()> {
        self.inner.worktree_mount(o).await
    }
    async fn worktree_forget(&self, o: &WorktreeProject) -> Result<()> {
        self.inner.worktree_forget(o).await
    }
    async fn worktree_remove(&self, o: &WorktreeRemove) -> Result<WorktreeRemoval> {
        self.inner.worktree_remove(o).await
    }
    fn has_room(&self) -> bool {
        self.inner.has_room()
    }
    async fn room(&self) -> Option<MemoryHere> {
        self.inner.room().await
    }
}

/// `fakeLocal(root)`, with `exec` changing the options of every local exec stream the runtime opens (the TS spreads
/// `...o` over `{ root, runDir }`, then a test adds `pollMs`).
pub fn fake_local_with(root: &Path, env: Option<IndexMap<String, String>>, exec: impl Fn(&mut LocalExecOptions) + 'static) -> LocalWiring {
    let folder = root.display().to_string();
    let runs = format!("{folder}/runs");
    let (exec_root, home) = (folder.clone(), folder.clone());
    let (copier, _log) = copying_fake();
    LocalWiring {
        backend: Arc::new(LocalBackend::new(LocalBackendOptions { root: root.to_path_buf(), env: None })),
        folder: folder.clone(),
        exec_stream: Rc::new(move |opts, waiting| {
            let mut local = LocalExecOptions::new(exec_root.clone(), runs.clone());
            if let Some(opts) = opts {
                local.idle_ms = opts.idle_ms;
                local.deadline_ms = opts.deadline_ms;
                local.now = opts.now;
                local.poll_ms = opts.poll_ms;
                local.reading = opts.reading;
            }
            exec(&mut local);
            local_exec_stream(local, waiting)
        }),
        home: Rc::new(move |_| format!("{home}/.claude")),
        home_dir: folder.clone(),
        roots_path: format!("{folder}/roots"),
        env: Rc::new(move || {
            env.clone()
                .unwrap_or_else(|| IndexMap::from([("PATH".to_string(), std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into()))]))
        }),
        daemon_road: None,
        here_daemon: None,
        restart_daemon: None,
        sys_samples: None,
        copier: Some(copier),
        platform: test_platform(),
        close: None,
    }
}

/// `fakeLocal(root)`.
pub fn fake_local(root: &Path) -> LocalWiring {
    fake_local_with(root, None, |_| {})
}
