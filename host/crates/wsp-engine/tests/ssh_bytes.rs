// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/test/ssh-bytes.test.ts: the one road bytes take onto a machine. A provider that mints a
//! signed URL gets a PUT; a machine that mints none carries the bytes over the connection that runs its commands.
//! Spec: spec/ssh_machine.md "Bytes over the connection".
mod roads_common;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use roads_common::*;
use tokio_util::sync::CancellationToken;
use wsp_engine::errors::{Result, RoadError};
use wsp_engine::fetch::{Fetch, FetchRequest, FetchResponse};
use wsp_engine::land_bytes::{land_bytes, LandBytesOptions};
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::ssh_backend::{put_bytes_script, ssh_args, SshArgsOptions, SshMachine, SshReach, SshTransport, SSH_BYTES_OK};

fn reach() -> SshReach {
    SshReach { user: "maya".to_string(), host: "box".to_string(), port: 2222, key_path: None }
}

/// An ssh client that never leaves this computer: it keeps every script it was asked to carry and the bytes that
/// rode stdin with it, and answers the way the machine's own script would.
fn fake_transport() -> (SshTransport, Arc<Mutex<Vec<Carried>>>) {
    transport(|_, _, _| res(0, &format!("{SSH_BYTES_OK}\n"), ""))
}

#[tokio::test]
async fn ride_the_connections_stdin_under_a_script_that_names_the_path() {
    let (transport, carried) = fake_transport();
    let machine = SshMachine::new(reach(), transport).unwrap();
    let bytes = vec![1u8, 2, 3, 4, 5];
    machine.put_bytes("/home/maya/.wsp/daemon.tgz", &bytes, None).await.unwrap();
    let carried = carried.lock().unwrap().clone();
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].opts.stdin, Some(bytes));
    let script = &carried[0].script;
    assert!(script.contains("cat > '/home/maya/.wsp/daemon.tgz.wsp-in-"), "{script}");
    assert!(script.contains("mkdir -p '/home/maya/.wsp'"), "{script}");
    // Into place only once the count matches: a connection cut halfway must leave the target as it was rather
    // than a file that looks whole.
    assert!(script.contains("= 5 ]"), "{script}");
    let tmp = script.split("cat > '").nth(1).unwrap().split('\'').next().unwrap();
    let suffix = tmp.strip_prefix("/home/maya/.wsp/daemon.tgz.wsp-in-").unwrap();
    assert!(!suffix.is_empty() && suffix.chars().all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c)), "{tmp}");
    assert!(script.contains(&format!("mv -f '{tmp}' '/home/maya/.wsp/daemon.tgz'")), "{script}");
}

#[test]
fn keeps_the_connections_stdin_for_the_file() {
    let with_stdin = SshArgsOptions { stdin: true, ..Default::default() };
    assert!(!ssh_args(&reach(), "cat > x", &with_stdin).iter().any(|a| a == "-n"));
    assert!(ssh_args(&reach(), "echo hi", &SshArgsOptions::default()).iter().any(|a| a == "-n"));
}

#[tokio::test]
async fn says_what_did_not_land_rather_than_leaving_a_half_file_behind() {
    let (transport, _) = transport(|_, _, _| res(1, "WSP_BYTES_SHORT\n", ""));
    let machine = SshMachine::new(reach(), transport).unwrap();
    let refused = machine.put_bytes("/home/maya/x", &[0u8; 9], None).await.unwrap_err();
    assert!(refused.to_string().contains("9 bytes did not land at /home/maya/x over ssh (exit 1)"), "{refused}");
}

#[test]
fn names_the_bytes_the_path_and_the_temporary_name_it_lands_under() {
    let s = put_bytes_script("/home/maya/f", 12, "/home/maya/f.tmp");
    assert_eq!(s.split('\n').next().unwrap(), "set -e");
    assert!(s.contains(&format!("echo {SSH_BYTES_OK}")), "{s}");
}

#[tokio::test]
async fn takes_the_machines_own_road_when_it_has_one() {
    let (transport, carried) = fake_transport();
    let machine = SshMachine::new(reach(), transport).unwrap();
    land_bytes(&machine, "/home/maya/f", &[7, 7], LandBytesOptions::default()).await.unwrap();
    let carried = carried.lock().unwrap().clone();
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].opts.stdin, Some(vec![7u8, 7]));
}

/// A machine that mints a signed URL and carries no bytes of its own, which is what every provider backend is.
struct UrlMachine;

#[async_trait]
impl Machine for UrlMachine {
    fn id(&self) -> &str {
        "m1"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, _cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        Err(plain("not in this test"))
    }
    async fn run(&self, _script: &str, _opts: RunOptions) -> Result<ExecResult> {
        Err(plain("not in this test"))
    }
    async fn snapshot(&self, _name: &str, _life: MachineLife, _opts: SnapshotOptions) -> Result<String> {
        Err(plain("not in this test"))
    }
    async fn pause(&self, _wait_ms: Option<u64>) -> Result<()> {
        Err(plain("not in this test"))
    }
    async fn resume(&self, _signal: Option<CancellationToken>) -> Result<()> {
        Err(plain("not in this test"))
    }
    async fn kill(&self) -> Result<()> {
        Err(plain("not in this test"))
    }
    async fn state(&self) -> Result<MachineState> {
        Err(plain("not in this test"))
    }
    async fn download_url(&self, _path: &str) -> Result<String> {
        Err(plain("not in this test"))
    }
    async fn upload_url(&self, path: &str) -> Result<String> {
        Ok(format!("https://signed.example/{path}"))
    }
}

struct RecordingFetch {
    puts: Mutex<Vec<FetchRequest>>,
}

#[async_trait]
impl Fetch for RecordingFetch {
    async fn fetch(&self, req: FetchRequest) -> std::result::Result<FetchResponse, RoadError> {
        self.puts.lock().unwrap().push(req);
        Ok(FetchResponse { status: 200, headers: vec![], body: vec![] })
    }
}

#[tokio::test]
async fn mints_a_signed_url_and_puts_where_the_machine_carries_no_bytes_itself() {
    let fetch = Arc::new(RecordingFetch { puts: Mutex::new(Vec::new()) });
    let opts = LandBytesOptions { fetch: Some(fetch.clone()), timeout_ms: None };
    land_bytes(&UrlMachine, "/root/wsp-daemon.tgz", &[9], opts).await.unwrap();
    let puts = fetch.puts.lock().unwrap();
    let seen: Vec<(String, Option<Vec<u8>>)> = puts.iter().map(|p| (p.url.clone(), p.body.clone())).collect();
    assert_eq!(seen, vec![("https://signed.example//root/wsp-daemon.tgz".to_string(), Some(vec![9u8]))]);
    assert_eq!(puts[0].method, "PUT");
    assert!(puts[0].headers.is_empty());
}
