// SPDX-License-Identifier: AGPL-3.0-only
//! stub-backend.ts:511 `copyingFake`: the copy road as a test wires it, the engine's fake (which records every ask) over
//! a folder copied for real, so a thread or a command on the copy has a folder to start in and a delete takes it away.
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use wsp_engine::copier::{
    fake_copier, Copier, CopyAsk, CopyReport, CopyRoad, FakeCopier, FakeCopierLog, WorktreeAsk, WorktreeAt, WorktreeProject,
    WorktreeRemoval, WorktreeRemove, WorktreeReport,
};
use wsp_engine::errors::Result;
use wsp_engine::local_backend::MemoryHere;
use wsp_wire::views::workspace_more::Carried;

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

pub struct CopyingFake {
    inner: FakeCopier,
}

/// `copyingFake()`.
pub fn copying_fake() -> Rc<CopyingFake> {
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
    Rc::new(CopyingFake { inner: fake_copier(Some(script), None) })
}

/// `copyingFake()` with its log beside it, the shape part 2c's tests take (`let (copier, log) = copying_fake()`); `common`
/// hands it out as `copying_fake`.
pub fn copying_fake_logged() -> (Rc<CopyingFake>, Arc<std::sync::Mutex<FakeCopierLog>>) {
    let copier = copying_fake();
    let log = copier.log();
    (copier, log)
}

impl CopyingFake {
    /// What the fake was asked, as `fakeCopier()` keeps it.
    pub fn log(&self) -> Arc<std::sync::Mutex<FakeCopierLog>> {
        self.inner.log.clone()
    }
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
