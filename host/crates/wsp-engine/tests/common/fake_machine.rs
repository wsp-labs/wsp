// SPDX-License-Identifier: AGPL-3.0-only
//! lifecycle.test.ts:6 `stubMachine` and :18 `counting`: a machine that answers every call and records each one in
//! order, so a test asserts the exact sequence the Workspace drove it through. Entries are `pause`, `resume`, `kill`,
//! `snapshot:<name>:<first_life>` and `preview_url:<port>`; a call that failed is its name with `:failed` after it.
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use tokio_util::sync::CancellationToken;
use wsp_engine::errors::{EngineError, Result};
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineState, PreviewReach, RunOptions, SnapshotOptions};

pub struct FakeMachine {
    id: String,
    /// Every call, in order. Shared with a test that records its own hooks in the same list.
    pub log: Arc<Mutex<Vec<String>>>,
    /// How many routes `preview_url` has minted: the token of the n-th is `t<n>`.
    minted: AtomicU32,
    resume_error: Mutex<Option<EngineError>>,
    refuse_resumed_snapshots: Mutex<bool>,
}

impl FakeMachine {
    pub fn new(id: &str) -> Arc<Self> {
        Self::with_log(id, Arc::new(Mutex::new(Vec::new())))
    }

    pub fn with_log(id: &str, log: Arc<Mutex<Vec<String>>>) -> Arc<Self> {
        Arc::new(FakeMachine { id: id.to_string(), log, minted: AtomicU32::new(0), resume_error: Mutex::new(None), refuse_resumed_snapshots: Mutex::new(false) })
    }

    /// Every resume from now on throws this.
    pub fn fail_resume_with(&self, e: EngineError) {
        *self.resume_error.lock().unwrap() = Some(e);
    }

    /// A snapshot of a resumed machine throws `snapshot <name> refused: resumed`, as a backend's own refusal.
    pub fn refuse_resumed_snapshots(&self) {
        *self.refuse_resumed_snapshots.lock().unwrap() = true;
    }

    pub fn calls(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }

    /// How many entries are exactly `call`.
    pub fn count(&self, call: &str) -> usize {
        self.log.lock().unwrap().iter().filter(|c| c.as_str() == call).count()
    }

    fn record(&self, call: String) {
        self.log.lock().unwrap().push(call);
    }
}

fn ok_exec() -> ExecResult {
    ExecResult { exit_code: 0, stdout: String::new(), stderr: String::new() }
}

#[async_trait]
impl Machine for FakeMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    fn offers(&self) -> MachineOffers {
        MachineOffers { preview_url: true, ..Default::default() }
    }

    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        Ok(ok_exec())
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        Ok(ok_exec())
    }
    async fn snapshot(&self, name: &str, life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        self.record(format!("snapshot:{name}:{}", life.first_life));
        if *self.refuse_resumed_snapshots.lock().unwrap() && !life.first_life {
            return Err(EngineError::Other { message: format!("snapshot {name} refused: resumed"), code: None });
        }
        Ok("snap_x".to_string())
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        self.record("pause".to_string());
        Ok(())
    }
    async fn resume(&self, _signal: Option<CancellationToken>) -> Result<()> {
        let failure = self.resume_error.lock().unwrap().clone();
        match failure {
            Some(e) => {
                self.record("resume:failed".to_string());
                Err(e)
            }
            None => {
                self.record("resume".to_string());
                Ok(())
            }
        }
    }
    async fn kill(&self) -> Result<()> {
        self.record("kill".to_string());
        Ok(())
    }
    async fn state(&self) -> Result<MachineState> {
        Ok(MachineState::Running)
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Ok("https://x".to_string())
    }
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Ok("https://x".to_string())
    }
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        self.record(format!("preview_url:{port}"));
        let n = self.minted.fetch_add(1, Ordering::SeqCst) + 1;
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as f64;
        Ok(PreviewReach { url: format!("https://{}-{port}.example", self.id), token: format!("t{n}"), expires_at: (now + 3_600_000.0).into() })
    }
}
