// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.ts:1046-1430 at a4033dc34. Spec: spec/box_machine.md.
use std::sync::Mutex;

use async_trait::async_trait;
use indexmap::IndexMap;
use tokio_util::sync::CancellationToken;

use super::plan::BoxClassType;
use super::request::BoxBackend;
use crate::errors::Result;
use crate::machine::{
    BytesLanded, DaemonSupervisor, ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineShape, MachineState,
    PreviewReach, RunOptions, Seen, SnapshotOptions,
};

/// box-backend.ts:1054. The script that writes the daemon's environment file under /etc, mode 0600.
pub fn env_landing_script(envs: &IndexMap<String, String>) -> String {
    todo!()
}

/// box-backend.ts:1067. `sudo -n bash -c '<DROP_SUDO_MARKS>\n<EXEC_ENV>\n<cmd>'`, shell-quoted whole.
pub fn sudo_command(cmd: &str) -> String {
    todo!()
}

/// box-backend.ts:1071.
pub const DAEMON_EXITS_MARK: &str = "WSP_DAEMON_EXITS";

/// box-backend.ts:1079.
pub const DAEMON_EXITS_CMD: &str = r#"u=wsp-daemon.service
a=$(systemctl show -p NRestarts --value "$u")
sleep 5
b=$(systemctl show -p NRestarts --value "$u")
if [ "$(systemctl show -p ActiveState --value "$u")" = failed ] || [ $(( ${b:-0} - ${a:-0} )) -ge 2 ] || { [ "${b:-0}" -ge 5 ] && [ "${b:-0}" -gt "${a:-0}" ]; }; then
  echo "WSP_DAEMON_EXITS $(systemctl show -p SubState --value "$u") $(systemctl show -p Result --value "$u") $(systemctl show -p ExecMainStatus --value "$u")"
  journalctl _SYSTEMD_UNIT="$u" -n 20 --no-pager -o cat
fi"#;

/// box-backend.ts:1092.
pub fn exit_words(result: &str, status: &str) -> String {
    todo!()
}

/// box-backend.ts:1098. `None` where the mark is not there.
pub fn daemon_exits_of(stdout: &str) -> Option<String> {
    todo!()
}

/// box-backend.ts:1108.
pub struct BoxMachine {
    pub backend: BoxBackend,
    pub id: String,
    pub kind: MachineKind,
    pub labels: Option<IndexMap<String, String>>,
    pub seen: Option<Seen>,
    pub replayed: bool,
    /// The id of the deletion the provider accepted, once kill() has been answered.
    pub deletion: Mutex<Option<String>>,
}

impl BoxMachine {
    /// box-backend.ts:1115.
    pub fn new(
        backend: BoxBackend,
        id: &str,
        kind: MachineKind,
        labels: Option<IndexMap<String, String>>,
        seen: Option<Seen>,
        replayed: bool,
    ) -> Self {
        todo!()
    }

    /// box-backend.ts:1124. `/sandboxes/<id, URI-encoded><suffix>`.
    pub fn path(&self, suffix: &str) -> String {
        todo!()
    }

    /// box-backend.ts:1147.
    pub async fn exec_inline(&self, cmd: &str, timeout_ms: u64, signal: Option<&CancellationToken>) -> Result<ExecResult> {
        todo!()
    }

    /// box-backend.ts:1188.
    pub async fn land_envs(&self, envs: &IndexMap<String, String>) -> Result<()> {
        todo!()
    }

    /// box-backend.ts:1260. `None` keeps the box's own class.
    pub async fn resume_as(&self, kind: Option<BoxClassType>, signal: Option<&CancellationToken>) -> Result<()> {
        todo!()
    }

    /// box-backend.ts:1330.
    pub async fn open_port(&self, port: u16) -> Result<()> {
        todo!()
    }

    /// The deletion id kill() kept.
    pub fn deletion(&self) -> Option<String> {
        todo!()
    }
}

#[async_trait]
impl Machine for BoxMachine {
    fn id(&self) -> &str {
        &self.id
    }
    fn kind(&self) -> MachineKind {
        self.kind
    }
    /// Always `None`: the desktop stream is minted per open.
    fn stream_url(&self) -> Option<&str> {
        None
    }
    fn labels(&self) -> Option<&IndexMap<String, String>> {
        self.labels.as_ref()
    }
    fn seen(&self) -> Option<&Seen> {
        self.seen.as_ref()
    }
    fn replayed(&self) -> bool {
        self.replayed
    }
    fn daemon_supervisor(&self) -> Option<DaemonSupervisor> {
        None
    }
    /// resume_at, preview_url, prove_road, start_daemon, daemon_exits, put_bytes, read_bytes, describe.
    fn offers(&self) -> MachineOffers {
        todo!()
    }
    /// box-backend.ts:1128.
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        todo!()
    }
    /// box-backend.ts:1180.
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        todo!()
    }
    /// box-backend.ts:1202.
    async fn snapshot(&self, name: &str, life: MachineLife, opts: SnapshotOptions) -> Result<String> {
        todo!()
    }
    /// box-backend.ts:1221.
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        todo!()
    }
    /// box-backend.ts:1249.
    async fn resume(&self, signal: Option<CancellationToken>) -> Result<()> {
        todo!()
    }
    /// box-backend.ts:1256.
    async fn resume_at(&self, cpu: f64, mem_mb: u64, signal: Option<CancellationToken>) -> Result<()> {
        todo!()
    }
    /// box-backend.ts:1277.
    async fn kill(&self) -> Result<()> {
        todo!()
    }
    /// box-backend.ts:1287.
    async fn state(&self) -> Result<MachineState> {
        todo!()
    }
    /// box-backend.ts:1423.
    async fn download_url(&self, path: &str) -> Result<String> {
        todo!()
    }
    /// box-backend.ts:1427.
    async fn upload_url(&self, path: &str) -> Result<String> {
        todo!()
    }
    /// box-backend.ts:1314.
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        todo!()
    }
    /// box-backend.ts:1174.
    async fn prove_road(&self, signal: Option<CancellationToken>) -> Result<()> {
        todo!()
    }
    /// box-backend.ts:1137.
    async fn start_daemon(&self) -> Result<ExecResult> {
        todo!()
    }
    /// box-backend.ts:1141.
    async fn daemon_exits(&self) -> Result<Option<String>> {
        todo!()
    }
    /// box-backend.ts:1360. Answers `Ok(None)`: the TS's `void`.
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        todo!()
    }
    /// box-backend.ts:1393.
    async fn read_bytes(&self, path: &str, size: Option<u64>, timeout_ms: Option<u64>) -> Result<Vec<u8>> {
        todo!()
    }
    /// box-backend.ts:1297.
    async fn describe(&self) -> Result<MachineShape> {
        todo!()
    }
}
