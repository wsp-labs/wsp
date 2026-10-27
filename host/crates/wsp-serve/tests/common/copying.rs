// SPDX-License-Identifier: AGPL-3.0-only
//! stub-backend.ts:511 `copyingFake`: the copy road as a test wires it, the engine's fake (which records every ask) over
//! a folder copied for real, so a thread or a command on the copy has a folder to start in and a delete takes it away.
use std::process::Command;
use std::rc::Rc;
use std::sync::Arc;

use async_trait::async_trait;
use wsp_engine::copier::{
    fake_copier, Copier, CopyAsk, CopyReport, CopyRoad, FakeCopier, WorktreeAsk, WorktreeAt, WorktreeProject, WorktreeRemoval,
    WorktreeRemove, WorktreeReport,
};
use wsp_engine::errors::Result;
use wsp_engine::local_backend::MemoryHere;
use wsp_wire::views::workspace_more::Carried;

pub struct CopyingFake {
    inner: FakeCopier,
}

/// `copyingFake()`.
pub fn copying_fake() -> Rc<CopyingFake> {
    let script: wsp_engine::copier::CopyScript = Arc::new(|ask: &CopyAsk| {
        let ok = Command::new("cp").arg("-R").arg(&ask.from).arg(&ask.to).status().expect("cp").success();
        assert!(ok, "cp -R {} {}", ask.from, ask.to);
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

impl CopyingFake {
    /// What the fake was asked, as `fakeCopier()` keeps it.
    pub fn log(&self) -> Arc<std::sync::Mutex<wsp_engine::copier::FakeCopierLog>> {
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
