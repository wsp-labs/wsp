// SPDX-License-Identifier: AGPL-3.0-only
//! box-backend.ts:1046-1430 at a4033dc34. Spec: spec/box_machine.md.
use std::sync::Mutex;

use crate::cancel::Cancel;
use async_trait::async_trait;
use base64::Engine;
use indexmap::IndexMap;
use serde_json::{json, Value};

use super::plan::{
    box_class_for, box_snapshot_name, class_of_type, state_map, state_of, BoxClassType, SizeAsk, BOX_INLINE_MAX_MS, ENV_LANDING_MS,
    FILE_GET_MAX, FILE_PUT_MAX, FIREWALL_HOLD_MS, FIREWALL_WATCH_MS,
};
use super::request::{
    encode_uri_component, still_restoring, stop_refusal_of, BoxBackend, BoxView, CommandView, NamedSnapshotView, RequestOpts,
    DISK_EXPIRED_CODE,
};
use crate::errors::{EngineError, ErrorKind, Result};
use crate::exec_detached::{exec_detached, DAEMON_ENV_FILE, DEADLINE_EXIT, INLINE_EXEC_MS};
use crate::golden_seam::EXEC_ENV;
use crate::labels::NAME_LABEL;
use crate::machine::{
    BytesLanded, DaemonSupervisor, ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineOffers, MachineShape, MachineState,
    PreviewReach, RunOptions, Seen, SnapshotOptions,
};
use crate::preview::DAEMON_PORT;
use crate::run_env::DROP_SUDO_MARKS;
use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::session_events::DAEMON_UNIT;
use wsp_words::js::{js_round, js_trim, number_string, to_number, utf16};
use wsp_words::place_word::provider_key_name;
use wsp_words::words::computer::{move_timed_out_line, Move};
use wsp_words::words::thread::no_work_folder_line;
use wsp_words::words::units::BOAT_DISK_GONE_LINE;

/// box-backend.ts:1054. The script that writes the daemon's environment file under /etc, mode 0600.
pub fn env_landing_script(envs: &IndexMap<String, String>) -> String {
    let dir = utf16::slice(DAEMON_ENV_FILE, 0, DAEMON_ENV_FILE.rfind('/').map(|i| i as i64));
    let mut lines = vec![format!("mkdir -p {}", shell_quote(&dir)), format!("cat > {} <<'WSP_ENV'", shell_quote(DAEMON_ENV_FILE))];
    for (name, value) in envs {
        let escaped: String = value.chars().flat_map(|c| if c == '\\' || c == '"' { vec!['\\', c] } else { vec![c] }).collect();
        lines.push(format!("{name}=\"{escaped}\""));
    }
    lines.push("WSP_ENV".to_string());
    lines.push(format!("chmod 0600 {}", shell_quote(DAEMON_ENV_FILE)));
    lines.join("\n")
}

/// box-backend.ts:1067. `sudo -n bash -c '<DROP_SUDO_MARKS>\n<EXEC_ENV>\n<cmd>'`, shell-quoted whole.
pub fn sudo_command(cmd: &str) -> String {
    format!("sudo -n bash -c {}", shell_quote(&format!("{DROP_SUDO_MARKS}\n{EXEC_ENV}\n{cmd}")))
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
    match result {
        "exit-code" => format!("exit status {status}"),
        "signal" | "core-dump" => format!("killed by signal {status}"),
        "oom-kill" => "killed for want of memory".to_string(),
        _ => result.to_string(),
    }
}

/// box-backend.ts:1098. `None` where the mark is not there.
pub fn daemon_exits_of(stdout: &str) -> Option<String> {
    let lines: Vec<&str> = stdout.split('\n').map(js_trim).collect();
    let at = lines.iter().position(|l| l.starts_with(DAEMON_EXITS_MARK))?;
    let rest: Vec<&str> = lines[at + 1..].iter().copied().filter(|l| !l.is_empty()).collect();
    let said: Vec<&str> = (0..rest.len()).filter(|&i| !rest[i + 1..].contains(&rest[i])).map(|i| rest[i]).collect();
    if !said.is_empty() {
        let cut: Vec<String> = said[said.len().saturating_sub(4)..]
            .iter()
            .map(|l| if utf16::len(l) > 120 { format!("{}…", utf16::cut(l, 120)) } else { l.to_string() })
            .collect();
        return Some(cut.join("; "));
    }
    let mut words = lines[at][DAEMON_EXITS_MARK.len()..].split_whitespace();
    let sub = words.next().unwrap_or("");
    let result = words.next().unwrap_or("");
    let status = words.next().unwrap_or("");
    let restarting = sub.starts_with("auto-restart") || sub == "failed";
    Some(if restarting && !result.is_empty() { exit_words(result, status) } else { "it printed nothing".to_string() })
}

/// The `box_backend` view of a state's word, as `moveTimedOutLine` reads a state.
fn state_word(state: MachineState) -> &'static str {
    match state {
        MachineState::Starting => "starting",
        MachineState::Running => "running",
        MachineState::Paused => "paused",
        MachineState::Gone => "gone",
    }
}

/// The stderr the loader's EMFILE line sits on: `/^(bash|sudo): error while loading shared libraries: .*: Error 24/m`.
fn loader_emfile(stderr: &str) -> bool {
    stderr.split(['\n', '\r', '\u{2028}', '\u{2029}']).any(|line| {
        ["bash", "sudo"].iter().any(|p| {
            line.strip_prefix(&format!("{p}: error while loading shared libraries: ")).is_some_and(|rest| rest.contains(": Error 24"))
        })
    })
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
        Self { backend, id: id.to_string(), kind, labels, seen, replayed, deletion: Mutex::new(None) }
    }

    /// box-backend.ts:1124. `/sandboxes/<id, URI-encoded><suffix>`.
    pub fn path(&self, suffix: &str) -> String {
        format!("/sandboxes/{}{suffix}", encode_uri_component(&self.id))
    }

    /// box-backend.ts:1147.
    pub async fn exec_inline(&self, cmd: &str, timeout_ms: u64, signal: Option<&Cancel>) -> Result<ExecResult> {
        let timeout_s = (timeout_ms as f64 / 1000.0).ceil().max(1.0) as u64;
        let body = json!({ "command": sudo_command(cmd), "timeoutSeconds": timeout_s });
        let opts =
            RequestOpts { body: Some(body), cap_ms: Some(timeout_s * 1000 + 30_000), signal: signal.cloned(), ..RequestOpts::default() };
        let path = self.path("/commands");
        let raw = self
            .backend
            .while_restoring(|| self.backend.request("POST", &path, opts.clone()), signal)
            .await
            .map_err(|e| self.command_failure(e))?;
        let res: CommandView = serde_json::from_value(raw).map_err(|e| EngineError::Other { message: e.to_string(), code: None })?;
        let stderr = res.stderr.clone().unwrap_or_default();
        if loader_emfile(&stderr) {
            return Err(EngineError::guest_unusable(&self.id, &provider_key_name("box"), js_trim(&stderr), 200));
        }
        let exit_code = res.exit_code.unwrap_or(if res.timed_out == Some(true) { DEADLINE_EXIT } else { -1 });
        Ok(ExecResult { exit_code: JsInt::from(exit_code), stdout: res.stdout.unwrap_or_default(), stderr })
    }

    /// The reading of a command's refusal: a work folder that is gone, a shell the agent could not spawn, and a box
    /// that is stopped each name the guest, anything else is the refusal as it came.
    fn command_failure(&self, e: EngineError) -> EngineError {
        let provider = provider_key_name("box");
        let message = e.to_string();
        if message.to_ascii_lowercase().contains("cwd must be an existing directory") {
            let detail = no_work_folder_line(&provider, super::plan::BOX_WORK_DIR, &message);
            return EngineError::guest_unusable(&self.id, &provider, &detail, e.status().unwrap_or(0));
        }
        if e.kind() == Some(ErrorKind::Unknown) && e.status() == Some(500) {
            return EngineError::guest_unusable(&self.id, &provider, &message, 500);
        }
        if e.code() == Some("machine_not_running") {
            return EngineError::machine_stopped(&self.id, &e);
        }
        e
    }

    /// box-backend.ts:1174. One command on the road every other command takes.
    async fn prove_road_on(&self, signal: Option<&Cancel>) -> Result<()> {
        self.exec_inline("true", INLINE_EXEC_MS, signal).await.map(|_| ()).map_err(|e| {
            if still_restoring(&e) {
                EngineError::restore_unfinished(&self.id, &e)
            } else {
                e
            }
        })
    }

    /// box-backend.ts:1188. Lands the environment; a landing that times out while the box streams its disk in is
    /// asked again inside the restore budget.
    pub async fn land_envs(&self, envs: &IndexMap<String, String>) -> Result<()> {
        let started = self.backend.clock().now();
        loop {
            let opts = ExecOptions { timeout_ms: Some(ENV_LANDING_MS), ..ExecOptions::default() };
            let res = self.exec(&env_landing_script(envs), opts).await?;
            let exit_code = res.exit_code.get();
            if exit_code == 0 {
                return Ok(());
            }
            let budget = self.backend.budgets();
            if exit_code != DEADLINE_EXIT || self.backend.clock().now() - started >= budget.restore_ms as i64 {
                let stderr = js_trim(&res.stderr);
                let detail = if stderr.is_empty() { js_trim(&res.stdout).to_string() } else { stderr.to_string() };
                return Err(EngineError::EnvNotLanded { machine_id: self.id.clone(), exit_code, detail });
            }
            self.backend.clock().sleep(budget.poll_ms).await;
        }
    }

    /// box-backend.ts:1260. `None` keeps the box's own class.
    pub async fn resume_as(&self, kind: Option<BoxClassType>, signal: Option<&Cancel>) -> Result<()> {
        self.backend.before_start().await?;
        let last = self.backend.last_timer(&self.id);
        let instant = last.map(|t| self.backend.clock().now() + t.span_ms);
        let ttl_seconds = self.backend.ttl_seconds_for(instant).await?;
        let mut body = json!({ "ttlSeconds": ttl_seconds });
        if let Some(kind) = kind {
            body["type"] = json!(kind.as_str());
        }
        let opts = RequestOpts { body: Some(body), signal: signal.cloned(), ..RequestOpts::default() };
        if let Err(e) = self.backend.request("POST", &self.path("/resume"), opts).await {
            if e.code() == Some(DISK_EXPIRED_CODE) {
                return Err(EngineError::DiskGone {
                    machine_id: self.id.clone(),
                    line: BOAT_DISK_GONE_LINE.to_string(),
                    said: e.to_string(),
                });
            }
            return Err(self.backend.start_refused(e, self.labels.as_ref().and_then(|l| l.get(NAME_LABEL)).map(String::as_str)).await);
        }
        self.backend.counted();
        let budget = self.backend.budgets().resume_ms;
        let running = |v: &BoxView| state_map(&v.state) == Some(MachineState::Running);
        self.backend.settle(&self.id, &running, budget, "resume", signal, None, None).await?;
        self.prove_road_on(signal).await
    }

    /// box-backend.ts:1256. The class `{cpu, mem}` names.
    async fn resume_at_size(&self, cpu: f64, mem_mb: u64, signal: Option<&Cancel>) -> Result<()> {
        let size = SizeAsk { cpu: Some(cpu), mem_mb: Some(mem_mb as f64) };
        self.resume_as(Some(box_class_for(size).kind), signal).await
    }

    /// The deletion id kill() kept.
    pub fn deletion(&self) -> Option<String> {
        self.deletion.lock().unwrap().clone()
    }

    /// box-backend.ts:1202: a named snapshot of the disk as it stands, answered with its name once the provider reads it ready.
    async fn named_snapshot(&self, name: &str) -> Result<String> {
        let saved = box_snapshot_name(name)?;
        let body = json!({ "sandboxId": self.id, "name": saved });
        self.backend.request("POST", "/named-snapshots", RequestOpts { body: Some(body), ..RequestOpts::default() }).await?;
        let budgets = self.backend.budgets();
        let started = self.backend.clock().now();
        loop {
            self.backend.clock().sleep(budgets.poll_ms).await;
            let raw =
                self.backend.request("GET", &format!("/named-snapshots/{}", encode_uri_component(&saved)), RequestOpts::default()).await?;
            let snapshot: NamedSnapshotView = serde_json::from_value(raw.get("snapshot").cloned().unwrap_or(Value::Null))
                .map_err(|e| EngineError::Other { message: e.to_string(), code: None })?;
            if snapshot.status == "ready" {
                return Ok(saved);
            }
            if snapshot.status == "failed" {
                let error = snapshot.error.as_deref().filter(|e| !e.is_empty()).map_or(String::new(), |e| format!(": {e}"));
                return Err(EngineError::Other { message: format!("named snapshot {saved} of {} failed{error}", self.id), code: None });
            }
            if self.backend.clock().now() - started >= budgets.snapshot_ms as i64 {
                let after = number_string(js_round(budgets.snapshot_ms as f64 / 1000.0));
                let message = format!("named snapshot {saved} of {} still reads saving after {after} s", self.id);
                return Err(EngineError::Other { message, code: None });
            }
        }
    }

    /// box-backend.ts:1221. Returns when the box reads archived; `wait_ms` stands in for the pause budget.
    async fn pause_within(&self, wait_ms: Option<u64>) -> Result<()> {
        let budgets = self.backend.budgets();
        let pause_ms = wait_ms.unwrap_or(budgets.pause_ms) as i64;
        let started = self.backend.clock().now();
        let mut view = self.backend.view(&self.id).await?;
        if view.state == "archived" {
            return Ok(());
        }
        if view.state != "archiving" {
            self.backend.request("POST", &self.path("/stop"), RequestOpts { body: Some(json!({})), ..RequestOpts::default() }).await?;
        }
        loop {
            self.backend.clock().sleep(budgets.poll_ms).await;
            view = self.backend.view(&self.id).await?;
            if view.state == "archived" {
                return Ok(());
            }
            if view.state != "archiving" {
                let said = stop_refusal_of(&view);
                self.backend
                    .warn(&format!("{}: pause did not take: the provider left it {} and refused the stop: {said}", self.id, view.state));
                return Err(EngineError::stop_refused(&self.id, &said));
            }
            let elapsed = self.backend.clock().now() - started;
            if elapsed >= pause_ms {
                let reads = state_map(&view.state).map(state_word);
                let words = move_timed_out_line(Move::Pause, elapsed as f64, reads);
                self.backend.warn(&format!("{}: {words}", self.id));
                return Err(EngineError::MoveUnanswered { message: words, cause: None });
            }
        }
    }

    /// box-backend.ts:1330. Opens the guest firewall for one port and holds it until a check has seen the rule stand
    /// for the hold window.
    pub async fn open_port(&self, port: u16) -> Result<()> {
        if self.port_standing(port).await {
            return Ok(());
        }
        let budgets = self.backend.budgets();
        let started = self.backend.clock().now();
        let adds_until = started + budgets.restore_ms as i64 + FIREWALL_HOLD_MS as i64;
        let mut held: Option<i64> = None;
        let mut said = String::new();
        loop {
            if let Some(since) = held {
                if self.port_standing(port).await {
                    if self.backend.clock().now() - since >= FIREWALL_HOLD_MS as i64 {
                        return Ok(());
                    }
                    self.backend.clock().sleep(FIREWALL_WATCH_MS).await;
                    continue;
                }
                held = None;
            }
            if self.backend.clock().now() >= adds_until {
                break;
            }
            let opts = ExecOptions::default();
            match self.exec(&format!("ufw allow {port}/tcp"), opts).await {
                Ok(res) if res.exit_code.get() == 0 => held = Some(self.backend.clock().now()),
                Ok(res) => said = refusal_words(&res.stderr, &res.stdout),
                Err(e) => said = utf16::tail(js_trim(&e.to_string()), 200),
            }
            let wait = if held.is_none() { budgets.poll_ms } else { FIREWALL_WATCH_MS };
            self.backend.clock().sleep(wait).await;
        }
        let detail = if said.is_empty() { String::new() } else { format!(" ({said})") };
        self.backend
            .warn(&format!("{}: the firewall did not hold a rule for port {port}{detail}; the route is minted without one", self.id));
        Ok(())
    }

    /// Whether the guest's firewall stands a rule for the port: one `ufw status` read, a failed read is not standing.
    async fn port_standing(&self, port: u16) -> bool {
        let cmd = format!("ufw status | grep -q '^{port}/tcp '");
        self.exec(&cmd, ExecOptions::default()).await.is_ok_and(|res| res.exit_code.get() == 0)
    }

    /// box-backend.ts:1393's straight road: one GET of the file, base64 as the endpoint answers it.
    async fn get_file(&self, at: &str, timeout_ms: Option<u64>) -> Result<Vec<u8>> {
        let path = self.path(&format!("/files?path={}&encoding=base64", encode_uri_component(at)));
        let opts = RequestOpts { cap_ms: timeout_ms, ..RequestOpts::default() };
        let raw = self.backend.while_restoring(|| self.backend.request("GET", &path, opts.clone()), None).await?;
        let read: CommandViewContent =
            serde_json::from_value(raw).map_err(|e| EngineError::Other { message: e.to_string(), code: None })?;
        base64::engine::general_purpose::STANDARD
            .decode(read.content.unwrap_or_default())
            .map_err(|e| EngineError::Other { message: e.to_string(), code: None })
    }

    /// box-backend.ts:1360's one PUT of a piece of bytes.
    async fn put_file(&self, at: &str, part: &[u8], timeout_ms: Option<u64>) -> Result<()> {
        let path = self.path("/files");
        let content = base64::engine::general_purpose::STANDARD.encode(part);
        let body = json!({ "path": at, "content": content, "encoding": "base64" });
        let opts = RequestOpts { body: Some(body), cap_ms: timeout_ms, ..RequestOpts::default() };
        self.backend.while_restoring(|| self.backend.request("PUT", &path, opts.clone()), None).await.map(|_| ())
    }
}

/// box-backend.ts:1393's cut-and-join road for a file too big for one GET: cut into 4 MiB pieces under /tmp as root, each
/// read back, the whole checked against the size. The caller removes the pieces either way.
impl BoxMachine {
    async fn read_pieces(
        &self,
        path: &str,
        size: f64,
        base: &str,
        names: &[String],
        quoted: &[String],
        timeout_ms: Option<u64>,
    ) -> Result<Vec<u8>> {
        let cut_cmd = format!(
            "split -b {FILE_GET_MAX} -d -a 4 {} {} && chmod 0644 {}",
            shell_quote(path),
            shell_quote(&format!("{base}.")),
            quoted.join(" ")
        );
        let cut = self.exec(&cut_cmd, ExecOptions::default()).await?;
        if cut.exit_code.get() != 0 {
            let detail = refusal_tail(&cut.stderr, &cut.stdout);
            let message = format!("{path} on {} was not cut for reading (exit {}): {detail}", self.id, cut.exit_code.get());
            return Err(EngineError::Other { message, code: None });
        }
        let mut whole = Vec::new();
        for name in names {
            whole.extend(self.get_file(name, timeout_ms).await?);
        }
        if whole.len() as f64 != size {
            let message = format!("{path} on {} read {} bytes of {}", self.id, whole.len(), number_string(size));
            return Err(EngineError::Other { message, code: None });
        }
        Ok(whole)
    }
}

/// The `content` a GET /files answers with.
#[derive(serde::Deserialize)]
struct CommandViewContent {
    #[serde(default)]
    content: Option<String>,
}

/// What a refused ufw add says: stderr when it is not empty, else stdout, trimmed, and its last 200 units.
fn refusal_words(stderr: &str, stdout: &str) -> String {
    let raw = if stderr.is_empty() { stdout } else { stderr };
    utf16::tail(js_trim(raw), 200)
}

#[async_trait(?Send)]
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
        MachineOffers {
            resume_at: true,
            preview_url: true,
            prove_road: true,
            start_daemon: true,
            daemon_exits: true,
            put_bytes: true,
            read_bytes: true,
            describe: true,
            ..MachineOffers::default()
        }
    }
    /// box-backend.ts:1128.
    async fn exec(&self, cmd: &str, opts: ExecOptions) -> Result<ExecResult> {
        let timeout_ms = opts.timeout_ms.unwrap_or(INLINE_EXEC_MS);
        if timeout_ms > BOX_INLINE_MAX_MS {
            let run = RunOptions { deadline_ms: timeout_ms, poll_ms: Some(self.backend.budgets().poll_ms), ..RunOptions::default() };
            return exec_detached(self, cmd, run).await;
        }
        self.exec_inline(cmd, timeout_ms, None).await
    }
    /// box-backend.ts:1180.
    async fn run(&self, script: &str, opts: RunOptions) -> Result<ExecResult> {
        let poll_ms = opts.poll_ms.or(Some(self.backend.budgets().poll_ms));
        exec_detached(self, script, RunOptions { poll_ms, ..opts }).await
    }
    /// box-backend.ts:1202.
    async fn snapshot(&self, name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        self.named_snapshot(name).await
    }
    /// box-backend.ts:1221.
    async fn pause(&self, wait_ms: Option<u64>) -> Result<()> {
        self.pause_within(wait_ms).await
    }
    /// box-backend.ts:1249.
    async fn resume(&self, signal: Option<&Cancel>) -> Result<()> {
        self.resume_as(None, signal).await
    }
    /// box-backend.ts:1256.
    async fn resume_at(&self, cpu: f64, mem_mb: u64, signal: Option<&Cancel>) -> Result<()> {
        self.resume_at_size(cpu, mem_mb, signal).await
    }
    /// box-backend.ts:1277.
    async fn kill(&self) -> Result<()> {
        let opts = RequestOpts { headers: vec![("X-Ascii-Confirm-Delete".to_string(), self.id.clone())], ..RequestOpts::default() };
        match self.backend.request("DELETE", &self.path(""), opts).await {
            Ok(res) => {
                let deletion = res.get("operation").and_then(|op| op.get("id")).and_then(Value::as_str).map(str::to_string);
                *self.deletion.lock().unwrap() = deletion;
            }
            Err(e) if crate::errors::is_missing(&e) => {}
            Err(e) => return Err(e),
        }
        self.backend.forget_backstop(&self.id);
        Ok(())
    }
    /// box-backend.ts:1287.
    async fn state(&self) -> Result<MachineState> {
        match self.backend.view(&self.id).await {
            Ok(view) => Ok(state_of(&view.state)),
            Err(e) if crate::errors::is_missing(&e) => Ok(MachineState::Gone),
            Err(e) => Err(e),
        }
    }
    /// box-backend.ts:1423.
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(EngineError::Other {
            message: "a box serves no signed download URL; its files come out through GET /files".to_string(),
            code: None,
        })
    }
    /// box-backend.ts:1427.
    async fn upload_url(&self, _path: &str) -> Result<String> {
        Err(EngineError::Other { message: "a box serves no signed upload URL; its files go in through PUT /files".to_string(), code: None })
    }
    /// box-backend.ts:1314.
    async fn preview_url(&self, port: u16) -> Result<PreviewReach> {
        let daemon = port == DAEMON_PORT;
        if !daemon {
            self.open_port(port).await?;
        }
        let body = json!({ "port": port, "public": daemon });
        let res = self.backend.request("POST", &self.path("/host"), RequestOpts { body: Some(body), ..RequestOpts::default() }).await?;
        let url = res.get("url").and_then(Value::as_str).unwrap_or_default().to_string();
        let parsed = url::Url::parse(&url).map_err(|_| EngineError::Other { message: "Invalid URL".to_string(), code: None })?;
        let token = parsed.query_pairs().find(|(k, _)| k == "_token").map(|(_, v)| v.into_owned()).unwrap_or_default();
        Ok(PreviewReach { url, token, expires_at: JsNumber(9_007_199_254_740_991.0) })
    }
    /// box-backend.ts:1174.
    async fn prove_road(&self, signal: Option<&Cancel>) -> Result<()> {
        self.prove_road_on(signal).await
    }
    /// box-backend.ts:1137.
    async fn start_daemon(&self) -> Result<ExecResult> {
        self.exec(&format!("systemctl start {DAEMON_UNIT}"), ExecOptions::default()).await
    }
    /// box-backend.ts:1141.
    async fn daemon_exits(&self) -> Result<Option<String>> {
        let res = self.exec(DAEMON_EXITS_CMD, ExecOptions::default()).await?;
        Ok(daemon_exits_of(&res.stdout))
    }
    /// box-backend.ts:1360. Answers `Ok(None)`: the TS's `void`.
    async fn put_bytes(&self, path: &str, bytes: &[u8], timeout_ms: Option<u64>) -> Result<Option<BytesLanded>> {
        let pieces = bytes.len().div_ceil(FILE_PUT_MAX as usize).max(1);
        if pieces == 1 && (path.starts_with("/tmp/") || path.starts_with("/home/user/")) {
            self.put_file(path, bytes, timeout_ms).await?;
            return Ok(None);
        }
        let base = format!("/tmp/wsp-put-{}", random_hex(6));
        let mut names: Vec<String> = Vec::with_capacity(pieces);
        for i in 0..pieces {
            let name = format!("{base}.{i}");
            let from = (i * FILE_PUT_MAX as usize).min(bytes.len());
            let to = ((i + 1) * FILE_PUT_MAX as usize).min(bytes.len());
            self.put_file(&name, &bytes[from..to], timeout_ms).await?;
            names.push(name);
        }
        let quoted: Vec<String> = names.iter().map(|n| shell_quote(n)).collect();
        let quoted = quoted.join(" ");
        let quoted_path = shell_quote(path);
        let last_slash = utf16::last_index_of(path, "/").map_or(-1, |i| i as i64);
        let folder = utf16::slice(path, 0, Some(last_slash));
        let folder = if folder.is_empty() { "/".to_string() } else { folder };
        let len = bytes.len();
        let script = [
            format!("[ ! -e {} ] && [ \"$(stat -c %s {quoted_path} 2>/dev/null)\" = {len} ] && exit 0", shell_quote(&names[0])),
            format!("mkdir -p {}", shell_quote(&folder)),
            format!("cat {quoted} > {quoted_path} && chmod 0644 {quoted_path} && rm -f {quoted}"),
            format!("[ \"$(stat -c %s {quoted_path})\" = {len} ] || {{ echo \"WSP_SHORT $(stat -c %s {quoted_path})\"; exit 1; }}"),
        ]
        .join("\n");
        let joined = self.exec(&script, ExecOptions::default()).await?;
        if joined.exit_code.get() != 0 {
            let said = refusal_tail(&joined.stderr, &joined.stdout);
            return Err(EngineError::Other {
                message: format!("{path} did not land on {} (exit {}): {said}", self.id, joined.exit_code.get()),
                code: None,
            });
        }
        Ok(None)
    }
    /// box-backend.ts:1393.
    async fn read_bytes(&self, path: &str, size: Option<u64>, timeout_ms: Option<u64>) -> Result<Vec<u8>> {
        let size = match size {
            Some(size) => size as f64,
            None => {
                let said = self.exec(&format!("stat -c %s {}", shell_quote(path)), ExecOptions::default()).await?;
                let size = to_number(js_trim(&said.stdout));
                if said.exit_code.get() != 0 || size.fract() != 0.0 || !size.is_finite() {
                    let detail = refusal_tail(&said.stderr, &said.stdout);
                    let message = format!("{path} on {} could not be read (exit {}): {detail}", self.id, said.exit_code.get());
                    return Err(EngineError::Other { message, code: None });
                }
                size
            }
        };
        if size <= FILE_GET_MAX as f64 && (path.starts_with("/tmp/") || path.starts_with("/home/user/")) {
            return self.get_file(path, timeout_ms).await;
        }
        let base = format!("/tmp/wsp-get-{}", random_hex(6));
        let pieces = ((size / FILE_GET_MAX as f64).ceil().max(1.0)) as usize;
        let names: Vec<String> = (0..pieces).map(|i| format!("{base}.{i:04}")).collect();
        let quoted: Vec<String> = names.iter().map(|n| shell_quote(n)).collect();
        let result = self.read_pieces(path, size, &base, &names, &quoted, timeout_ms).await;
        let _ = self.exec(&format!("rm -f {}", quoted.join(" ")), ExecOptions::default()).await;
        result
    }
    /// box-backend.ts:1297.
    async fn describe(&self) -> Result<MachineShape> {
        let view = self.backend.view(&self.id).await?;
        let cls = class_of_type(view.kind.as_deref());
        Ok(MachineShape {
            cpu: view.vcpu.map(JsNumber),
            mem_mb: view.memory_gb.map(|gb| JsNumber(gb * 1024.0)),
            disk_gb: cls.map(|c| JsNumber(c.disk_gb)),
            created_at: view.created_at.clone(),
            used_bytes: None,
        })
    }
}

/// Six random bytes as lowercase hex, the twelve characters `randomBytes(6).toString("hex")` makes.
fn random_hex(n: usize) -> String {
    let bytes: Vec<u8> = (0..n).map(|_| rand::random::<u8>()).collect();
    hex::encode(bytes)
}

/// `stderr.trim() || stdout.trim()`, the words a failed command names.
fn refusal_tail(stderr: &str, stdout: &str) -> String {
    let err = js_trim(stderr);
    if err.is_empty() {
        js_trim(stdout).to_string()
    } else {
        err.to_string()
    }
}
