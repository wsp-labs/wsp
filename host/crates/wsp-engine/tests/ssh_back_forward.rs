// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/ssh-back-forward.test.ts: the person's config carried onto the dial-back forward's
//! child, and the held forward itself. Specs: spec/ssh_dial.md.
mod roads_common;

use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures::FutureExt;
use roads_common::*;
use tempfile::TempDir;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::oneshot;
use wsp_engine::child_exec::LineFeed;
use wsp_engine::errors::EngineError;
use wsp_engine::ssh_backend::{
    carried_ssh_values, hold_back_forward, missing_known_hosts_line, ssh_back_args, ChildEnd, HeldChild, SshCarried, SshLocalRun, SshReach,
    SshSpawn,
};

const DEFAULTS: [&str; 13] = [
    "user root",
    "hostname 203.0.113.9",
    "port 22",
    "checkhostip no",
    "identitiesonly no",
    "hostkeyalgorithms ssh-ed25519,ecdsa-sha2-nistp256,rsa-sha2-512",
    "pubkeyacceptedalgorithms ssh-ed25519,ecdsa-sha2-nistp256,rsa-sha2-512",
    "identityfile ~/.ssh/id_rsa",
    "identityfile ~/.ssh/id_ed25519",
    "globalknownhostsfile /etc/ssh/ssh_known_hosts /etc/ssh/ssh_known_hosts2",
    "userknownhostsfile /Users/dev/.ssh/known_hosts /Users/dev/.ssh/known_hosts2",
    "addkeystoagent false",
    "clearallforwardings no",
];

fn defaults() -> Vec<String> {
    DEFAULTS.iter().map(|l| l.to_string()).collect()
}

/// Known hosts files that are on this computer, since a child is never handed one that is not.
struct Hosts {
    dir: TempDir,
}

impl Hosts {
    fn new() -> Self {
        Hosts { dir: tempfile::Builder::new().prefix("wsp-known-").tempdir().unwrap() }
    }

    fn file(&self, name: &str) -> String {
        let path = self.dir.path().join(name);
        std::fs::write(&path, "").unwrap();
        path.to_string_lossy().into_owned()
    }

    fn path(&self, name: &str) -> String {
        self.dir.path().join(name).to_string_lossy().into_owned()
    }
}

const JUMPBOX_PROXY: &str =
    "/opt/homebrew/bin/python3 -S /opt/sdk/gcloud.py compute start-iap-tunnel jumpbox %p --listen-on-stdin --project=p --zone=z";

/// The owner's jumpbox block as `ssh -G` answers it: a tunnel command with a token the child expands, the name
/// its key is filed under and the file it is filed in, and the LocalForwards a block like spoo's carries too.
fn jumpbox(google_hosts: &str) -> Vec<String> {
    vec![
        "user dev_example".to_string(),
        "hostname compute.1234567890".to_string(),
        "port 22".to_string(),
        "checkhostip no".to_string(),
        "identitiesonly yes".to_string(),
        "hostkeyalias compute.1234567890".to_string(),
        "hostkeyalgorithms ssh-ed25519,ecdsa-sha2-nistp256,rsa-sha2-512".to_string(),
        "pubkeyacceptedalgorithms ssh-ed25519,ecdsa-sha2-nistp256,rsa-sha2-512".to_string(),
        "identityfile ~/.ssh/google_compute_engine".to_string(),
        "globalknownhostsfile /etc/ssh/ssh_known_hosts /etc/ssh/ssh_known_hosts2".to_string(),
        format!("userknownhostsfile {google_hosts}"),
        "addkeystoagent yes".to_string(),
        "localforward 8080 [127.0.0.1]:8080".to_string(),
        format!("proxycommand {JUMPBOX_PROXY}"),
    ]
}

fn replace_line(line: &str, key: &str, value: &str) -> String {
    if line.starts_with(&format!("{key} ")) {
        format!("{key} {value}")
    } else {
        line.to_string()
    }
}

/// `ssh -G` without the person's config for the same dial, and with it: the fake tells them apart by `-F /dev/null`
/// and records each ask, and never starts a client.
fn fake_config(with_config: Vec<String>, hostname: &str, user: &str) -> (SshLocalRun, Arc<Mutex<Vec<Vec<String>>>>) {
    let (hostname, user) = (hostname.to_string(), user.to_string());
    local_run(move |_file, args| {
        let bare = args.iter().any(|a| a == "/dev/null");
        let dialed = args.iter().position(|a| a == "-p").map(|i| args[i + 1].clone()).unwrap_or_else(|| "22".to_string());
        let lines: Vec<String> = if bare {
            DEFAULTS.iter().map(|l| replace_line(&replace_line(l, "user", &user), "hostname", &hostname)).collect()
        } else {
            with_config.clone()
        };
        let lines: Vec<String> = lines.iter().map(|l| replace_line(l, "port", &dialed)).collect();
        res(0, &(lines.join("\n") + "\n"), "")
    })
}

/// A child that never leaves this process: the test writes its output and ends it, and reads what the holder did.
struct Fake {
    stdout: Option<DuplexStream>,
    stderr: Option<DuplexStream>,
    stdin: DuplexStream,
    killed: Arc<Mutex<Vec<String>>>,
    end: Option<oneshot::Sender<ChildEnd>>,
}

fn fake_child() -> (HeldChild, Fake) {
    let (stdout_w, stdout_r) = tokio::io::duplex(65536);
    let (stderr_w, stderr_r) = tokio::io::duplex(65536);
    let (stdin_w, stdin_r) = tokio::io::duplex(65536);
    let killed = Arc::new(Mutex::new(Vec::new()));
    let (end_tx, end_rx) = oneshot::channel();
    let log = killed.clone();
    let child = HeldChild {
        stdin: Box::new(stdin_w),
        stdout: Box::new(stdout_r),
        stderr: Box::new(stderr_r),
        kill: Box::new(move |signal: &str| {
            log.lock().unwrap().push(signal.to_string());
            true
        }),
        exit: async move { end_rx.await.unwrap_or_else(|_| ChildEnd::Failed("the test dropped the child".to_string())) }.boxed_local(),
    };
    (child, Fake { stdout: Some(stdout_w), stderr: Some(stderr_w), stdin: stdin_r, killed, end: Some(end_tx) })
}

impl Fake {
    async fn out(&mut self, text: &str) {
        self.stdout.as_mut().unwrap().write_all(text.as_bytes()).await.unwrap();
    }

    async fn err(&mut self, text: &str) {
        self.stderr.as_mut().unwrap().write_all(text.as_bytes()).await.unwrap();
    }

    /// Reads stdin's far end: a read of 0 bytes is the holder having closed it.
    async fn stdin_ended(&mut self) -> bool {
        let mut buf = [0u8; 8];
        matches!(tokio::time::timeout(Duration::from_millis(50), self.stdin.read(&mut buf)).await, Ok(Ok(0)))
    }

    fn killed(&self) -> Vec<String> {
        self.killed.lock().unwrap().clone()
    }

    /// A real child's pipes close as it exits, so they close before the end is sent.
    fn exit(&mut self, end: ChildEnd) {
        self.stdout.take();
        self.stderr.take();
        self.end.take().expect("exited once").send(end).unwrap();
    }
}

fn spawn_of(child: HeldChild, spawned: Arc<Mutex<Vec<Vec<String>>>>) -> SshSpawn {
    let slot = Mutex::new(Some(child));
    Arc::new(move |file: &str, args: &[String]| {
        let mut line = vec![file.to_string()];
        line.extend(args.iter().cloned());
        spawned.lock().unwrap().push(line);
        slot.lock().unwrap().take().expect("the holder spawns one child")
    })
}

fn spawn_quiet(child: HeldChild) -> SshSpawn {
    spawn_of(child, Arc::new(Mutex::new(Vec::new())))
}

fn carried() -> SshCarried {
    SshCarried { reach: reach("root", "box", 22), options: vec![] }
}

fn reach(user: &str, host: &str, port: u16) -> SshReach {
    SshReach { user: user.to_string(), host: host.to_string(), port, key_path: None }
}

async fn settle() {
    tokio::time::sleep(Duration::from_millis(20)).await;
}

/// A holder that hangs would otherwise hang the suite.
async fn within<T>(fut: impl Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), fut).await.expect("timed out waiting on the holder")
}

fn options_of(args: &[String]) -> Vec<String> {
    args.iter().enumerate().filter(|(i, _)| *i > 0 && args[*i - 1] == "-o").map(|(_, a)| a.clone()).collect()
}

fn after<'a>(args: &'a [String], flag: &str) -> &'a str {
    &args[args.iter().position(|a| a == flag).unwrap() + 1]
}

fn has(list: &[String], word: &str) -> bool {
    list.iter().any(|a| a == word)
}

#[tokio::test]
async fn a_plain_host_dials_with_no_config() {
    let r = reach("root", "203.0.113.9", 22);
    let (run, asked) = fake_config(defaults(), "203.0.113.9", "root");
    let values = carried_ssh_values(&r, &run).await.unwrap();
    let asked = asked.lock().unwrap().clone();
    assert!(asked.iter().all(|a| a[0] == "ssh" && has(a, "-G") && a.last().unwrap() == "root@203.0.113.9"));
    assert!(asked.iter().any(|a| has(a, "-F") && has(a, "/dev/null")));
    let args = ssh_back_args(&values, 4640, 4640);
    assert_eq!(args[..2].to_vec(), vec!["-F", "/dev/null"]);
    for word in [
        "BatchMode=yes",
        "StrictHostKeyChecking=accept-new",
        "ExitOnForwardFailure=yes",
        "ServerAliveInterval=15",
        "ServerAliveCountMax=3",
        "ControlPath=none",
        "-T",
    ] {
        assert!(has(&args, word), "{word} missing from {args:?}");
    }
    assert!(!has(&args, "-N"));
    assert_eq!(after(&args, "-p"), "22");
    assert_eq!(args.iter().filter(|a| *a == "-R").count(), 1);
    assert_eq!(after(&args, "-R"), "127.0.0.1:4640:127.0.0.1:4640");
    assert_eq!(args[args.len() - 2..].to_vec(), vec!["root@203.0.113.9", "echo WSP_BACK_UP; exec cat"]);
    // The client's defaults are the child's defaults under -F /dev/null; spelling them out would only pin the
    // host key algorithms and lose the client's preference for the type known_hosts already holds.
    let joined = args.join(" ");
    for word in
        ["HostKeyAlgorithms", "IdentityFile", "UserKnownHostsFile", "ProxyCommand", "HostKeyAlias", "LocalForward", "ClearAllForwardings"]
    {
        assert!(!joined.contains(word), "{word} in {joined}");
    }
}

#[tokio::test]
async fn the_jumpbox_blocks_tunnel_key_and_identity_ride_along_but_its_local_forward_does_not() {
    let hosts = Hosts::new();
    let google_hosts = hosts.file("google_compute_known_hosts");
    let r = reach("dev_example", "jumpbox", 22);
    let (run, _) = fake_config(jumpbox(&google_hosts), "jumpbox", "dev_example");
    let args = ssh_back_args(&carried_ssh_values(&r, &run).await.unwrap(), 4640, 4640);
    assert_eq!(args[..2].to_vec(), vec!["-F", "/dev/null"]);
    let options = options_of(&args);
    for want in [
        format!("ProxyCommand={JUMPBOX_PROXY}"),
        "HostKeyAlias=compute.1234567890".to_string(),
        format!("UserKnownHostsFile={google_hosts}"),
        "IdentityFile=~/.ssh/google_compute_engine".to_string(),
        "IdentitiesOnly=yes".to_string(),
        "AddKeysToAgent=yes".to_string(),
    ] {
        assert!(has(&options, &want), "{want} missing from {options:?}");
    }
    let joined = options.join(" ");
    for word in ["LocalForward", "CheckHostIP", "HostKeyAlgorithms"] {
        assert!(!joined.contains(word), "{word} in {joined}");
    }
    assert_eq!(after(&args, "-R"), "127.0.0.1:4640:127.0.0.1:4640");
    assert_eq!(args[args.len() - 2..].to_vec(), vec!["dev_example@compute.1234567890", "echo WSP_BACK_UP; exec cat"]);
}

#[tokio::test]
async fn a_config_that_turns_check_host_ip_on_or_names_several_identities_carries_each() {
    let r = reach("root", "box", 2222);
    let mut block: Vec<String> =
        DEFAULTS.iter().map(|l| replace_line(l, "checkhostip", "yes")).filter(|l| !l.starts_with("identityfile ")).collect();
    block.push("identityfile ~/.ssh/a".to_string());
    block.push("identityfile ~/.ssh/b".to_string());
    let (run, _) = fake_config(block, "box", "root");
    let args = ssh_back_args(&carried_ssh_values(&r, &run).await.unwrap(), 4640, 4640);
    assert!(has(&args, "CheckHostIP=yes"));
    let identities: Vec<&String> = args.iter().filter(|a| a.starts_with("IdentityFile=")).collect();
    assert_eq!(identities, vec!["IdentityFile=~/.ssh/a", "IdentityFile=~/.ssh/b"]);
    assert_eq!(after(&args, "-p"), "2222");
}

#[tokio::test]
async fn a_carried_path_with_a_space_is_quoted_so_ssh_reads_it_as_one_value() {
    let r = reach("root", "box", 22);
    let hosts = Hosts::new();
    let (known, work) = (hosts.file("known_hosts"), hosts.file("work_hosts"));
    let agent = "/Users/dev/Library/Group Containers/2BUA8C4S2C.com.1password/t/agent.sock";
    let mut block: Vec<String> = DEFAULTS
        .iter()
        .filter(|l| !l.starts_with("identityfile ") && !l.starts_with("userknownhostsfile "))
        .map(|l| l.to_string())
        .collect();
    block.push(format!("identityagent {agent}"));
    block.push("identityfile ~/keys/my key".to_string());
    block.push("identityfile ~/.ssh/id_ed25519".to_string());
    block.push(r#"identityfile ~/odd/a"b\c"#.to_string());
    block.push(format!("userknownhostsfile {known} {work}"));
    block.push("proxycommand nc -X 5 -x proxy:1080 %h %p".to_string());
    let (run, _) = fake_config(block, "box", "root");
    let options = options_of(&ssh_back_args(&carried_ssh_values(&r, &run).await.unwrap(), 4640, 4640));
    for want in [
        format!(r#"IdentityAgent="{agent}""#),
        r#"IdentityFile="~/keys/my key""#.to_string(),
        "IdentityFile=~/.ssh/id_ed25519".to_string(),
        r#"IdentityFile="~/odd/a\"b\\c""#.to_string(),
        format!("UserKnownHostsFile={known} {work}"),
        "ProxyCommand=nc -X 5 -x proxy:1080 %h %p".to_string(),
    ] {
        assert!(has(&options, &want), "{want} missing from {options:?}");
    }
}

#[tokio::test]
async fn a_proxy_jump_rides_as_the_command_ssh_would_build_for_it() {
    let r = reach("root", "inner", 22);
    let mut block: Vec<String> = DEFAULTS.iter().map(|l| replace_line(l, "hostname", "10.0.0.7")).collect();
    block.push("proxyjump bastion,ops@edge:2222".to_string());
    let (run, _) = fake_config(block, "inner", "root");
    let args = ssh_back_args(&carried_ssh_values(&r, &run).await.unwrap(), 4640, 4640);
    assert!(has(&args, "ProxyCommand=ssh -o BatchMode=yes -J 'bastion' -W '[%h]:%p' 'ssh://ops@edge:2222'"));
    assert!(!args.join(" ").contains("ProxyJump"));
    assert_eq!(args[args.len() - 2], "root@10.0.0.7");
}

#[tokio::test]
async fn a_known_hosts_file_the_config_names_that_is_not_on_this_computer_refuses_the_forward() {
    let r = reach("root", "box", 22);
    let hosts = Hosts::new();
    let kept = hosts.file("kept_hosts");
    // How ssh -G prints `UserKnownHostsFile "~/my hosts"`: expanded, and joined by spaces with the quotes gone.
    let spaced = format!("{} hosts", hosts.path("my"));
    for (list, missing) in [(format!("{kept} {}", hosts.path("gone")), hosts.path("gone")), (spaced, hosts.path("my"))] {
        let mut block: Vec<String> = DEFAULTS.iter().filter(|l| !l.starts_with("userknownhostsfile ")).map(|l| l.to_string()).collect();
        block.push(format!("userknownhostsfile {list}"));
        let (run, _) = fake_config(block, "box", "root");
        let refused = carried_ssh_values(&r, &run).await.unwrap_err();
        assert!(matches!(refused, EngineError::MissingKnownHosts { .. }), "{refused:?}");
        assert_eq!(refused.to_string(), missing_known_hosts_line("root@box", &missing));
    }
    let mut block: Vec<String> = DEFAULTS.iter().filter(|l| !l.starts_with("globalknownhostsfile ")).map(|l| l.to_string()).collect();
    block.push("globalknownhostsfile none".to_string());
    let (run, _) = fake_config(block, "box", "root");
    assert!(has(&carried_ssh_values(&r, &run).await.unwrap().options, "GlobalKnownHostsFile=none"));
    assert!(missing_known_hosts_line("root@box", &"/x".repeat(400)).chars().count() <= 300);
    for long in [
        missing_known_hosts_line("root@box", &"/x".repeat(400)),
        missing_known_hosts_line(&format!("{}@box", "u".repeat(255)), "/etc/ssh/hosts"),
    ] {
        assert!(long.chars().count() <= 300);
        assert!(long.contains("create it, or rename a path that has a space in it"), "{long}");
    }
}

#[tokio::test]
async fn a_literal_percent_in_the_agents_path_is_doubled() {
    let r = reach("root", "box", 22);
    let mut block = defaults();
    block.push("identityagent /Users/dev/%h".to_string());
    block.push("certificatefile ~/.ssh/%h-cert.pub".to_string());
    let (run, _) = fake_config(block, "box", "root");
    let options = ssh_back_args(&carried_ssh_values(&r, &run).await.unwrap(), 4640, 4640);
    assert!(has(&options, "IdentityAgent=/Users/dev/%%h"));
    assert!(has(&options, "CertificateFile=~/.ssh/%h-cert.pub"));
}

#[tokio::test]
async fn a_config_ssh_cannot_read_is_refused_with_sshs_own_line() {
    let (run, _) = local_run(|_, _| res(255, "", "/Users/dev/.ssh/config line 3: Bad configuration option: frob\n"));
    let refused = carried_ssh_values(&reach("root", "box", 22), &run).await.unwrap_err();
    assert!(refused.to_string().contains("Bad configuration option: frob"), "{refused}");
}

/// ssh-backend.ts:329-336: the TS holder drains both pipes from the moment it spawns; the Rust one does it on a local
/// task, so a holder is started inside a `LocalSet`.
async fn on_local_set<T>(body: impl Future<Output = T>) -> T {
    tokio::task::LocalSet::new().run_until(body).await
}

#[tokio::test]
async fn the_forward_is_up_when_the_box_echoes_wsp_back_up() {
    on_local_set(async {
        let (child, mut fake) = fake_child();
        let spawned = Arc::new(Mutex::new(Vec::new()));
        let held = hold_back_forward(&carried(), 4640, 4640, spawn_of(child, spawned.clone()));
        let mut expected = vec!["ssh".to_string()];
        expected.extend(ssh_back_args(&carried(), 4640, 4640));
        assert_eq!(*spawned.lock().unwrap(), vec![expected]);
        fake.err("Allocated port 41234 for remote forward to 127.0.0.1:4640\n").await;
        settle().await;
        assert!(tokio::time::timeout(Duration::from_millis(20), held.up()).await.is_err(), "up settled on a stderr line");
        fake.out("WSP_BACK_UP\n").await;
        assert_eq!(within(held.up()).await, Ok(()));
        assert!(!fake.stdin_ended().await);
    })
    .await;
}

#[tokio::test]
async fn a_line_the_box_wrote_on_stderr_reaches_the_host_with_no_control_characters_in_it() {
    on_local_set(async {
        let (child, mut fake) = fake_child();
        let held = hold_back_forward(&carried(), 4640, 4640, spawn_quiet(child));
        fake.err("\x1b]0;pwned\x07motd from the box\x1b[2J\n").await;
        settle().await;
        fake.exit(ChildEnd::Exited(255));
        assert_eq!(within(held.ended()).await, "]0;pwnedmotd from the box[2J");
    })
    .await;
}

#[tokio::test]
async fn a_child_that_ends_before_it_is_up_rejects_with_sshs_last_line() {
    on_local_set(async {
        let (child, mut fake) = fake_child();
        let held = hold_back_forward(&carried(), 4640, 4640, spawn_quiet(child));
        fake.err("Warning: Permanently added 'box' (ED25519) to the list of known hosts.\n").await;
        fake.err("Warning: remote port forwarding failed for listen port 4640\n").await;
        fake.err("Error: remote port forwarding failed for listen port 4640\n").await;
        settle().await;
        fake.exit(ChildEnd::Exited(255));
        assert_eq!(within(held.up()).await, Err("Error: remote port forwarding failed for listen port 4640".to_string()));
        assert_eq!(within(held.ended()).await, "Error: remote port forwarding failed for listen port 4640");
    })
    .await;
}

#[tokio::test]
async fn a_child_that_says_nothing_but_sshs_own_known_hosts_note_says_its_exit_code() {
    on_local_set(async {
        let (child, mut fake) = fake_child();
        let held = hold_back_forward(&carried(), 4640, 4640, spawn_quiet(child));
        fake.err("Warning: Permanently added 'box' (ED25519) to the list of known hosts.\n").await;
        settle().await;
        fake.exit(ChildEnd::Exited(255));
        assert_eq!(within(held.up()).await, Err("ssh exited with 255".to_string()));
    })
    .await;
}

#[tokio::test]
async fn releasing_the_holder_closes_the_childs_stdin_and_stops_the_child() {
    on_local_set(async {
        let (child, mut fake) = fake_child();
        let held = hold_back_forward(&carried(), 4640, 4640, spawn_quiet(child));
        fake.out("WSP_BACK_UP\n").await;
        within(held.up()).await.unwrap();
        held.release();
        settle().await;
        assert!(fake.stdin_ended().await);
        assert_eq!(fake.killed(), vec!["SIGTERM"]);
        fake.exit(ChildEnd::Signalled("SIGTERM".to_string()));
        assert_eq!(within(held.ended()).await, "ssh ended on SIGTERM");
    })
    .await;
}

#[test]
fn a_box_that_writes_without_a_newline_leaves_only_a_bounded_tail_held() {
    let mut lines: Vec<String> = Vec::new();
    let mut feed = LineFeed::new(Some(4096));
    let chunk = "x".repeat(65536);
    for _ in 0..64 {
        lines.extend(feed.feed(&chunk));
    }
    lines.extend(feed.flush());
    assert_eq!(lines.len(), 1);
    assert!(lines[0].chars().count() <= 4096);
}

#[tokio::test]
async fn a_newline_free_flood_on_stderr_still_lets_the_up_line_and_sshs_last_line_through() {
    on_local_set(async {
        let (child, mut fake) = fake_child();
        let held = hold_back_forward(&carried(), 4640, 4640, spawn_quiet(child));
        let chunk = "x".repeat(65536);
        within(async {
            for _ in 0..64 {
                fake.err(&chunk).await;
            }
        })
        .await;
        fake.out("WSP_BACK_UP\n").await;
        assert_eq!(within(held.up()).await, Ok(()));
        within(async {
            for _ in 0..64 {
                fake.err(&chunk).await;
            }
        })
        .await;
        settle().await;
        fake.exit(ChildEnd::Exited(255));
        assert!(within(held.ended()).await.chars().count() <= 300);
    })
    .await;
}

#[tokio::test]
async fn a_child_that_ends_after_it_was_up_resolves_ended_with_its_last_line() {
    on_local_set(async {
        let (child, mut fake) = fake_child();
        let held = hold_back_forward(&carried(), 4640, 4640, spawn_quiet(child));
        fake.out("WSP_BACK_UP\n").await;
        within(held.up()).await.unwrap();
        fake.err("Timeout, server box not responding.\n").await;
        settle().await;
        fake.exit(ChildEnd::Exited(255));
        assert_eq!(within(held.ended()).await, "Timeout, server box not responding.");
    })
    .await;
}
