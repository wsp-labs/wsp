// SPDX-License-Identifier: AGPL-3.0-only
//! The machine half of "BoxMachine against a fake Box API" in packages/engine/test/box-backend.test.ts: the
//! machine's commands (exec, the sudo wrapper, the detached road, the restore and gateway refusals, the readings of
//! a guest that cannot run anything), start_daemon, a resume that fails on the road, and the environment landing
//! script. Bodies are the ones the live spike recorded on 2026-09-11; nothing here dials the real API.
mod common;

use std::sync::Arc;

use base64::{engine::general_purpose::STANDARD, Engine as _};
use indexmap::IndexMap;
use serde_json::{json, Value};
use wsp_engine::box_backend::{env_landing_script, sudo_command, BoxBackend, BoxBudgetsAsk, BoxMachine, BOX_INLINE_MAX_MS};
use wsp_engine::errors::{EngineError, ErrorKind};
use wsp_engine::golden_seam::EXEC_ENV;
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineKind};
use wsp_wire::views::session_events::DAEMON_UNIT;
use wsp_wire::wire::daemon::EXEC_DEADLINE_EXIT;

use common::fake_box::{backend_on, command, error, inner, machine_on, with, Answer, FakeBox, Reply};

// exec-detached.ts:21 DEADLINE_EXIT is protocol's EXEC_DEADLINE_EXIT.
const DEADLINE_EXIT: i64 = EXEC_DEADLINE_EXIT;
// exec-detached.ts:18
const INLINE_EXEC_MS: u64 = 20_000;
// exec-detached.ts:50
const DAEMON_ENV_FILE: &str = "/etc/wsp/daemon.env";
// box-backend.ts:250
const BOX_WORK_DIR: &str = "/home/user";
// run-env.ts:44
const DROP_SUDO_MARKS: &str = "unset SUDO_USER SUDO_UID SUDO_GID SUDO_COMMAND";
const BX: &str = "bx_tumrjngm";
const COMMANDS: &str = "/sandboxes/bx_tumrjngm/commands";

fn map(pairs: &[(&str, &str)]) -> IndexMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn reply(status: u16, body: Value) -> Answer {
    Answer::Reply(Reply { status, body })
}

fn refuse(status: u16, code: &str, message: &str) -> Answer {
    error(status, code, message).into()
}

fn cmd(stdout: &str) -> Answer {
    command(stdout, Some(0), json!({}))
}

fn done(stdout: &str) -> ExecResult {
    ExecResult { exit_code: 0, stdout: stdout.to_string(), stderr: String::new() }
}

/// box-backend.test.ts:139 `launchedText`.
fn launched_text(command_line: &str) -> String {
    let script = inner(command_line);
    let start = script.find("printf %s '").expect("a launch carries its file as printf %s '<base64>'") + "printf %s '".len();
    let end = start + script[start..].find('\'').expect("the base64 ends at a quote");
    String::from_utf8(STANDARD.decode(&script[start..end]).expect("base64")).expect("UTF-8")
}

const LOADER_ERROR: &str = "bash: error while loading shared libraries: libtinfo.so.6: cannot open shared object file: Error 24\n";

fn failed<T>(r: Result<T, EngineError>) -> EngineError {
    match r {
        Ok(_) => panic!("expected the call to fail"),
        Err(e) => e,
    }
}

fn calls_ending(api: &FakeBox, suffix: &str) -> usize {
    api.calls().iter().filter(|c| c.ends_with(suffix)).count()
}

fn machine_for(api: &Arc<FakeBox>) -> BoxMachine {
    machine_on(api, BX).1
}

fn machine_with(api: &Arc<FakeBox>, budgets: BoxBudgetsAsk) -> (BoxBackend, BoxMachine, Arc<common::fake_clock::FakeClock>) {
    let on = backend_on(api, "trial", budgets);
    let machine = BoxMachine::new(on.backend.clone(), BX, MachineKind::Sandbox, None, None, false);
    (on.backend, machine, on.clock)
}

fn exec_opts(timeout_ms: u64) -> ExecOptions {
    ExecOptions { timeout_ms: Some(timeout_ms), ..Default::default() }
}

/// box-backend.test.ts:441
#[tokio::test]
async fn a_box_that_reads_ready_but_refuses_commands_while_its_disk_streams_in_is_asked_again_inside_the_restore_budget() {
    let restoring = || refuse(409, "sandbox_restoring", "Box is restoring.");
    let api = FakeBox::new()
        .on("POST", COMMANDS, vec![restoring(), refuse(409, "sandbox_starting", "Box is starting."), cmd("up\n")])
        .on("PUT", "/sandboxes/bx_tumrjngm/files", vec![restoring(), reply(200, json!({ "ok": true }))]);
    let (on, machine) = machine_on(&api, BX);
    let before = on.clock.now_ms();
    assert_eq!(machine.exec("echo up", ExecOptions::default()).await.unwrap(), done("up\n"));
    assert_eq!(calls_ending(&api, "/commands"), 3);
    assert!(on.clock.now_ms() - before >= 20);
    machine.put_bytes("/tmp/x", &[1], None).await.unwrap();
    assert_eq!(calls_ending(&api, "/files"), 2);
    // Past the budget the refusal is the caller's, in the provider's words.
    let stuck = FakeBox::new().on("POST", COMMANDS, vec![restoring()]);
    let e = failed(machine_for(&stuck).exec("true", ExecOptions::default()).await);
    assert_eq!(e.code(), Some("sandbox_restoring"), "{e:?}");
    assert!(stuck.calls().len() > 5);
}

/// box-backend.test.ts:458
#[tokio::test]
async fn a_proof_still_refused_as_restoring_at_the_deadline_ends_typed_with_the_providers_request_id() {
    let stuck = FakeBox::new().on("POST", COMMANDS, vec![refuse(409, "sandbox_restoring", "Box is restoring.")]);
    let e = failed(machine_for(&stuck).prove_road(None).await);
    let EngineError::RestoreUnfinished { message, carried, .. } = &e else { panic!("expected RestoreUnfinished, got {e:?}") };
    assert_eq!(carried.kind, Some(ErrorKind::Conflict));
    assert_eq!(carried.status, Some(409));
    assert_eq!(carried.code.as_deref(), Some("sandbox_restoring"));
    assert_eq!(message, "Box is restoring.");
    assert_eq!(carried.request_id.as_deref(), Some("req_b603629401be48d082e623575a1b9111"));
}

/// box-backend.test.ts:465
#[tokio::test]
async fn a_box_answering_502_to_its_first_commands_is_asked_again_at_the_poll_pace_and_past_the_budget_the_502_is_the_callers() {
    let gateway = || refuse(502, "bad_gateway", "Bad Gateway");
    let api = FakeBox::new()
        .on("POST", COMMANDS, vec![gateway(), gateway(), gateway(), gateway(), gateway(), cmd("up\n")])
        .on("PUT", "/sandboxes/bx_tumrjngm/files", vec![gateway(), gateway(), gateway(), gateway(), gateway(), reply(200, json!({ "ok": true }))]);
    let (_, machine, clock) = machine_with(&api, BoxBudgetsAsk { restore_ms: Some(10_000), ..Default::default() });
    let before = clock.now_ms();
    assert_eq!(machine.exec("echo up", ExecOptions::default()).await.unwrap(), done("up\n"));
    assert_eq!(calls_ending(&api, "/commands"), 6);
    assert!(clock.now_ms() - before >= 10);
    machine.put_bytes("/tmp/x", &[1], None).await.unwrap();
    assert_eq!(calls_ending(&api, "/files"), 6);
    // Past the budget the gateway answer is the caller's, in the kind the engine reads it as.
    let stuck = FakeBox::new().on("POST", COMMANDS, vec![gateway()]);
    let (_, stuck_machine, _) = machine_with(&stuck, BoxBudgetsAsk { restore_ms: Some(10_000), ..Default::default() });
    let e = failed(stuck_machine.exec("true", ExecOptions::default()).await);
    assert_eq!(e.kind(), Some(ErrorKind::Transient), "{e:?}");
    assert_eq!(e.status(), Some(502));
    assert!(calls_ending(&stuck, "/commands") > 5);
    // A refusal that is neither the restore conflicts nor a gateway status is the caller's after one round.
    let refused = FakeBox::new().on("POST", COMMANDS, vec![refuse(400, "invalid_timeout", "timeoutSeconds must be between 1 and 600.")]);
    let e = failed(machine_for(&refused).exec("true", ExecOptions::default()).await);
    assert_eq!(e.kind(), Some(ErrorKind::Unknown), "{e:?}");
    assert_eq!(e.code(), Some("invalid_timeout"));
    assert_eq!(calls_ending(&refused, "/commands"), 1);
}

/// box-backend.test.ts:523
#[tokio::test]
async fn starts_the_daemons_unit_through_the_boxs_own_commands_endpoint_as_root() {
    let api = FakeBox::new().on("POST", COMMANDS, vec![cmd("")]);
    let machine = machine_for(&api);
    assert_eq!(machine.start_daemon().await.unwrap(), done(""));
    assert_eq!(api.commands_sent(), vec![sudo_command(&format!("systemctl start {DAEMON_UNIT}"))]);
}

/// box-backend.test.ts:530
#[tokio::test]
async fn runs_every_command_as_root_through_sudo_under_the_exec_environment_with_the_timeout_in_seconds() {
    let api = FakeBox::new().on(
        "POST",
        COMMANDS,
        vec![cmd("hi\n"), command("", None, json!({ "timedOut": true, "signal": "SIGTERM" })), refuse(409, "machine_not_running", "Box machine is not running.")],
    );
    let machine = machine_for(&api);
    assert_eq!(machine.exec("echo hi", ExecOptions::default()).await.unwrap(), done("hi\n"));
    let sent = api.took("POST", COMMANDS).unwrap();
    let sent_command = sent.command();
    assert_eq!(sent_command, sudo_command("echo hi"));
    assert!(sent_command.starts_with("sudo -n bash -c '"), "{sent_command}");
    assert_eq!(inner(&sent_command).split('\n').collect::<Vec<_>>(), vec![DROP_SUDO_MARKS, EXEC_ENV, "echo hi"]);
    assert!(sent_command.contains(EXEC_ENV));
    assert!(!sent_command.contains("-lc"));
    assert_eq!(sent.body["timeoutSeconds"].as_u64(), Some(INLINE_EXEC_MS / 1000));
    // A command the provider cut off at its timeout reads as a run past its deadline does.
    assert_eq!(machine.exec("true", exec_opts(BOX_INLINE_MAX_MS)).await.unwrap().exit_code, DEADLINE_EXIT);
    assert_eq!(api.seen().last().unwrap().body["timeoutSeconds"].as_u64(), Some(BOX_INLINE_MAX_MS / 1000));
    // A box that is not running refuses the command, thrown as the stopped machine with the refusal's kind and code.
    let stopped = failed(machine.exec("true", ExecOptions::default()).await);
    let EngineError::MachineStopped { machine_id, message, carried } = &stopped else { panic!("expected MachineStopped, got {stopped:?}") };
    assert_eq!(machine_id, BX);
    assert_eq!(carried.kind, Some(ErrorKind::Conflict));
    assert_eq!(carried.status, Some(409));
    assert_eq!(carried.code.as_deref(), Some("machine_not_running"));
    assert_eq!(message, "Box machine is not running.");
}

/// box-backend.test.ts:550
#[tokio::test]
async fn an_exec_asked_for_longer_than_the_edge_allows_goes_the_detached_road_and_no_command_carries_more_than_the_inline_cap() {
    let api = FakeBox::new().on(
        "POST",
        COMMANDS,
        vec![with(|seen| {
            let timeout_seconds = seen.body["timeoutSeconds"].as_f64().unwrap_or(0.0);
            let command_line = seen.command();
            // The edge cuts a long request itself, whatever the command asked for.
            if timeout_seconds > 30.0 {
                return refuse(504, "request_timeout", "Request timeout after 69998ms");
            }
            if command_line.contains("WSP_LAUNCHED") {
                return cmd("WSP_LAUNCHED\n");
            }
            if command_line.contains("WSP_POLL") {
                return cmd(&format!("WSP_POLL\n0\n{}\n\ndown\nWSP_POLL_END\n", STANDARD.encode("late\n")));
            }
            cmd("")
        })],
    );
    let machine = machine_for(&api);
    assert_eq!(machine.exec("sleep 100; echo late", exec_opts(300_000)).await.unwrap(), done("late\n"));
    let asked: Vec<f64> = api.seen().iter().filter(|s| s.path.ends_with("/commands")).map(|s| s.body["timeoutSeconds"].as_f64().unwrap()).collect();
    assert!(asked.len() > 1);
    assert!(asked.iter().cloned().fold(f64::MIN, f64::max) <= (BOX_INLINE_MAX_MS / 1000) as f64);
    // The script the caller asked for travelled as the launched file's text, not as one long request.
    let launch = api.seen().into_iter().find(|s| s.body["command"].as_str().is_some_and(|c| c.contains("WSP_LAUNCHED"))).unwrap();
    assert_eq!(launched_text(&launch.command()), "sleep 100; echo late");
    // Nothing is left on the guest: the run cleans up after itself.
    assert!(api.seen().iter().any(|s| s.body["command"].as_str().is_some_and(|c| c.contains("rm -rf"))));
}

/// box-backend.test.ts:571
#[tokio::test]
async fn a_command_road_whose_shell_dies_on_the_open_file_limit_is_the_providers_fault_not_a_command_that_failed() {
    let api = FakeBox::new().on("POST", COMMANDS, vec![command("", Some(127), json!({ "stderr": LOADER_ERROR }))]);
    let e = failed(machine_for(&api).exec("true", ExecOptions::default()).await);
    let EngineError::GuestUnusable { status, .. } = &e else { panic!("expected GuestUnusable, got {e:?}") };
    assert!(e.to_string().contains("Boat left bx_tumrjngm running but nothing on it can run"), "{e}");
    assert!(e.to_string().contains("libtinfo.so.6"), "{e}");
    assert_eq!(*status, 200);
    // A command of its own that fails, even with a loader error of its own kind, is still a command's exit code.
    let ordinary = FakeBox::new().on("POST", COMMANDS, vec![command("", Some(127), json!({ "stderr": "bash: nosuchthing: command not found\n" }))]);
    assert_eq!(machine_for(&ordinary).exec("nosuchthing", ExecOptions::default()).await.unwrap().exit_code, 127);
    // So is a child of the command that hits the same limit: the line is the box's only when what wsp starts died.
    let child = FakeBox::new().on(
        "POST",
        COMMANDS,
        vec![command("", Some(127), json!({ "stderr": "gh: error while loading shared libraries: libssl.so.3: cannot open shared object file: Error 24\n" }))],
    );
    assert_eq!(machine_for(&child).exec("gh auth status", ExecOptions::default()).await.unwrap().exit_code, 127);
    // The sudo that wraps every command is wsp's own and runs under the same limit, so it is the box's fault too.
    let wrapper = FakeBox::new().on(
        "POST",
        COMMANDS,
        vec![command("", Some(127), json!({ "stderr": "sudo: error while loading shared libraries: libaudit.so.1: cannot open shared object file: Error 24\n" }))],
    );
    let e = failed(machine_for(&wrapper).exec("true", ExecOptions::default()).await);
    assert!(matches!(e, EngineError::GuestUnusable { .. }), "{e:?}");
}

/// box-backend.test.ts:590
#[tokio::test]
async fn a_commands_endpoint_answering_500_because_its_agent_could_not_spawn_the_shell_is_the_same_fault() {
    let api = FakeBox::new().on("POST", COMMANDS, vec![refuse(500, "internal_error", "undefined is not an object (evaluating 'D.stdout.on')")]);
    let e = failed(machine_for(&api).exec("true", ExecOptions::default()).await);
    let EngineError::GuestUnusable { status, .. } = &e else { panic!("expected GuestUnusable, got {e:?}") };
    assert_eq!(e.to_string(), "Boat left bx_tumrjngm running but nothing on it can run: undefined is not an object (evaluating 'D.stdout.on')");
    // The status the provider answered with rides along: a caller that keys its creates spends the key on it.
    assert_eq!(*status, 500);
    // Every other refusal of the endpoint keeps its own kind.
    let busy = FakeBox::new().on("POST", COMMANDS, vec![refuse(409, "machine_not_running", "Box machine is not running.")]);
    let e = failed(machine_for(&busy).exec("true", ExecOptions::default()).await);
    assert_eq!(e.kind(), Some(ErrorKind::Conflict), "{e:?}");
    assert_eq!(e.code(), Some("machine_not_running"));
}

/// box-backend.test.ts:603
#[tokio::test]
async fn a_box_whose_work_folder_is_gone_names_the_folder_and_the_machine_and_says_a_rebuild_is_the_way_out() {
    for status in [400u16, 500] {
        let api = FakeBox::new().on("POST", COMMANDS, vec![refuse(status, "invalid_cwd", "cwd must be an existing directory.")]);
        let e = failed(machine_for(&api).exec("true", ExecOptions::default()).await);
        let EngineError::GuestUnusable { status: carried_status, .. } = &e else { panic!("expected GuestUnusable, got {e:?}") };
        assert_eq!(
            e.to_string(),
            format!(
                "Boat left bx_tumrjngm running but nothing on it can run: {BOX_WORK_DIR} is missing, and Boat runs every command there (it said: cwd must be an existing directory.); a wake cannot make it again, so a rebuild is the way out, and work not pushed is lost with the old disk"
            )
        );
        assert_eq!(*carried_status, status);
    }
}

/// box-backend.test.ts:649
#[tokio::test]
async fn a_resume_whose_box_reads_ready_and_then_cannot_run_a_command_ends_the_same_way_so_the_wake_reads_it_as_a_fault() {
    let api = FakeBox::new()
        .on("POST", "/sandboxes/bx_tumrjngm/resume", vec![reply(202, json!({ "ok": true, "status": "resuming", "sandbox": common::fake_box::box_view(BX, "provisioned", json!({})) }))])
        .on("GET", "/sandboxes/bx_tumrjngm", vec![common::fake_box::info(BX, "ready", json!({}))])
        .on("POST", COMMANDS, vec![command("", Some(127), json!({ "stderr": LOADER_ERROR }))]);
    let e = failed(machine_for(&api).resume(None).await);
    assert!(matches!(e, EngineError::GuestUnusable { .. }), "{e:?}");
}

/// box-backend.test.ts:657
#[test]
fn the_environment_script_writes_the_daemons_own_file_under_etc_one_quoted_line_per_variable_and_nothing_manager_wide() {
    let script = env_landing_script(&map(&[("IS_SANDBOX", "1"), ("PATH", "/usr/local/bin:/usr/bin"), ("QUOTED", "a\"b\\c")]));
    assert_eq!(
        script.split('\n').collect::<Vec<_>>(),
        vec![
            "mkdir -p '/etc/wsp'".to_string(),
            format!("cat > '{DAEMON_ENV_FILE}' <<'WSP_ENV'"),
            "IS_SANDBOX=\"1\"".to_string(),
            "PATH=\"/usr/local/bin:/usr/bin\"".to_string(),
            "QUOTED=\"a\\\"b\\\\c\"".to_string(),
            "WSP_ENV".to_string(),
            format!("chmod 0600 '{DAEMON_ENV_FILE}'"),
        ]
    );
    assert_eq!(DAEMON_ENV_FILE, "/etc/wsp/daemon.env");
    for banned in ["DefaultEnvironment", "system.conf", "set-environment"] {
        assert!(!script.contains(banned), "{banned} in {script}");
    }
}
