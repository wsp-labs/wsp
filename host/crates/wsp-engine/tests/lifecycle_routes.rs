// SPDX-License-Identifier: AGPL-3.0-only
//! preview.test.ts:61-144 at a4033dc34: the Workspace's routes per port, through preview's refresh rule (a reach is
//! reused while more than 10 minutes remain before it expires). Spec: spec/lifecycle.md, "Routes".
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use futures::FutureExt;
use wsp_engine::cancel::Cancel;
use wsp_engine::errors::Result;
use wsp_engine::lifecycle::*;
use wsp_engine::machine::*;

const FRESH_MS: f64 = 60.0 * 60_000.0;

/// preview.test.ts:51 `machineWithPreview`: mints `https://<id>-<port>.preview.getsolari.com?pt_token=t<n>`, each
/// expiring `expires_in_ms` from now, and counts every mint.
struct PreviewMachine {
    id: String,
    expires_in_ms: f64,
    minted: AtomicU32,
}

fn machine_with_preview(id: &str, expires_in_ms: f64) -> Arc<PreviewMachine> {
    Arc::new(PreviewMachine { id: id.into(), expires_in_ms, minted: AtomicU32::new(0) })
}

impl PreviewMachine {
    fn mints(&self) -> u32 {
        self.minted.load(Ordering::SeqCst)
    }
}

#[async_trait(?Send)]
impl Machine for PreviewMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { preview_url: true, ..Default::default() }
    }
    async fn exec(&self, _: &str, _: ExecOptions) -> Result<ExecResult> {
        Ok(ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() })
    }
    async fn run(&self, _: &str, _: RunOptions) -> Result<ExecResult> {
        Ok(ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() })
    }
    async fn snapshot(&self, _: &str, _: MachineLife, _: SnapshotOptions) -> Result<String> {
        Ok("snap".into())
    }
    async fn pause(&self, _: Option<u64>) -> Result<()> {
        Ok(())
    }
    async fn resume(&self, _: Option<&Cancel>) -> Result<()> {
        Ok(())
    }
    async fn kill(&self) -> Result<()> {
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _: &str) -> Result<String> {
        Ok(String::new())
    }
    async fn upload_url(&self, _: &str) -> Result<String> {
        Ok(String::new())
    }
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        let n = self.minted.fetch_add(1, Ordering::SeqCst) + 1;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64;
        Ok(PreviewReach {
            url: format!("https://{}-{port}.preview.getsolari.com?pt_token=t{n}", self.id),
            token: format!("t{n}"),
            expires_at: (now + self.expires_in_ms).into(),
        })
    }
}

fn hooks(resurrect: Option<Arc<PreviewMachine>>) -> WorkspaceHooks {
    WorkspaceHooks {
        golden_snapshot: "snap_g".into(),
        wake_attempts: 2,
        resurrect: resurrect.map(|b| -> ResurrectHook {
            Arc::new(move |_spec| {
                let m: MachineRef = b.clone();
                async move { Ok(m) }.boxed_local()
            })
        }),
        vault_export: None,
        vault_import: None,
        stash_vault: None,
        restore_vault: None,
        wake_check: None,
        move_hook: None,
        retire: None,
    }
}

/// preview.test.ts:61
#[tokio::test]
async fn mints_once_and_reuses_across_calls_on_the_same_running_machine() {
    let m = machine_with_preview("m1", FRESH_MS);
    let ws = Workspace::new(m.clone(), hooks(None), WorkspaceInitial::default());
    let first = ws.daemon_reach().await.unwrap();
    assert_eq!(ws.daemon_reach().await.unwrap(), first);
    assert_eq!(m.mints(), 1);
}

/// preview.test.ts:69
#[tokio::test]
async fn a_wake_drops_the_cached_routes_the_first_daemon_reach_after_it_mints_again_on_the_same_machine() {
    let m = machine_with_preview("m1", FRESH_MS);
    let ws = Workspace::new(m.clone(), hooks(None), WorkspaceInitial::default());
    let first = ws.daemon_reach().await.unwrap();
    ws.nap().await.unwrap();
    ws.wake(false).await.unwrap();
    let second = ws.daemon_reach().await.unwrap();
    assert_ne!(second.token, first.token, "the machine was asked again");
    assert_eq!(second.url.split('?').next(), first.url.split('?').next());
    assert_eq!(m.mints(), 2);
}

/// preview.test.ts:82
#[tokio::test]
async fn remints_once_the_cached_reach_goes_stale() {
    let m = machine_with_preview("m1", 5.0 * 60_000.0);
    let ws = Workspace::new(m.clone(), hooks(None), WorkspaceInitial::default());
    ws.daemon_reach().await.unwrap();
    ws.daemon_reach().await.unwrap();
    assert_eq!(m.mints(), 2);
}

/// preview.test.ts:90
#[tokio::test]
async fn remints_when_the_machine_was_replaced() {
    let a = machine_with_preview("m1", FRESH_MS);
    let b = machine_with_preview("m2", FRESH_MS);
    let ws = Workspace::new(a.clone(), hooks(Some(b.clone())), WorkspaceInitial::default());
    let first = ws.daemon_reach().await.unwrap();
    ws.upgrade(None).await.unwrap();
    let second = ws.daemon_reach().await.unwrap();
    assert!(first.url.contains("m1-7070"), "{}", first.url);
    assert!(second.url.contains("m2-7070"), "{}", second.url);
    assert_eq!(b.mints(), 1);
}

/// preview.test.ts:107
#[tokio::test]
async fn caches_per_port_two_ports_mint_twice_asking_again_reuses_each_and_7070_is_the_daemons_own_route() {
    let m = machine_with_preview("m1", FRESH_MS);
    let ws = Workspace::new(m.clone(), hooks(None), WorkspaceInitial::default());
    let a = ws.port_reach(3000).await.unwrap();
    let b = ws.port_reach(5173).await.unwrap();
    assert!(a.url.contains("m1-3000"));
    assert!(b.url.contains("m1-5173"));
    assert_eq!(ws.port_reach(3000).await.unwrap(), a);
    assert_eq!(ws.port_reach(5173).await.unwrap(), b);
    assert_eq!(m.mints(), 2);
    let daemon = ws.daemon_reach().await.unwrap();
    assert_eq!(ws.port_reach(7070).await.unwrap(), daemon);
    assert_eq!(m.mints(), 3);
}

/// preview.test.ts:122
#[tokio::test]
async fn a_replaced_machine_voids_every_ports_route() {
    let a = machine_with_preview("m1", FRESH_MS);
    let b = machine_with_preview("m2", FRESH_MS);
    let ws = Workspace::new(a.clone(), hooks(Some(b.clone())), WorkspaceInitial::default());
    assert!(ws.port_reach(3000).await.unwrap().url.contains("m1-3000"));
    ws.upgrade(None).await.unwrap();
    assert!(ws.port_reach(3000).await.unwrap().url.contains("m2-3000"));
    assert_eq!(b.mints(), 1);
}

/// preview.test.ts:132
#[tokio::test]
async fn remint_port_reach_drops_one_ports_fresh_route_and_mints_it_again_the_other_ports_keep_theirs() {
    let m = machine_with_preview("m1", FRESH_MS);
    let ws = Workspace::new(m.clone(), hooks(None), WorkspaceInitial::default());
    let a = ws.port_reach(3000).await.unwrap();
    let b = ws.port_reach(5173).await.unwrap();
    let fresh = ws.remint_port_reach(3000).await.unwrap();
    assert_ne!(fresh, a);
    assert!(fresh.url.contains("m1-3000"));
    assert_eq!(ws.port_reach(3000).await.unwrap(), fresh);
    assert_eq!(ws.port_reach(5173).await.unwrap(), b);
    assert_eq!(m.mints(), 3);
}
