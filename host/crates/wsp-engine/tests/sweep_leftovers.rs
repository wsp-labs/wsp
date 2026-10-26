// SPDX-License-Identifier: AGPL-3.0-only
//! `sweepLeftovers` and `SWEEP_STAGED_CMD` (golden-tools.ts:181), ported from sweep-leftovers.test.ts at 65f6b89c7.
use std::fs;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use async_trait::async_trait;
use wsp_engine::errors::Result;
use wsp_engine::machine::{ExecOptions, ExecResult, Machine, MachineKind, MachineLife, MachineState, RunOptions, SnapshotOptions};
use wsp_engine::sweep_leftovers::{sweep_leftovers, SWEEP_STAGED_CMD};

// node printed this from golden-tools.ts's STAGED_PREFIXES and template: JSON.stringify(SWEEP_STAGED_CMD).
const NODE_SWEEP_STAGED_CMD: &str = "find /tmp -maxdepth 1 \\( -name 'wsp-put-*' -o -name 'wsp-get-*' -o -name 'wsp-seed-*' -o -name 'wsp-session-*' -o -name 'wsp-vault-*' -o -name 'wsp-land-*' -o -name 'wsp-road-*' -o -name 'wsp-cask-*' -o -name 'wsp-draft-*' \\) -mmin +60 -exec rm -rf -- {} +";

#[test]
fn the_staging_command_is_the_ts_byte_for_byte() {
    assert_eq!(SWEEP_STAGED_CMD, NODE_SWEEP_STAGED_CMD);
}

// sweep-leftovers.test.ts:19
#[test]
fn it_takes_the_staging_a_road_left_an_hour_ago_and_more_and_never_a_road_in_flight_the_live_slates_or_anyones_file() {
    let dir = tempfile::tempdir().unwrap();
    let two_hours_ago = SystemTime::now() - Duration::from_secs(2 * 3600);
    let old = |name: &str, folder: bool| {
        let path = dir.path().join(name);
        if folder {
            fs::create_dir(&path).unwrap();
        } else {
            fs::write(&path, "x").unwrap();
        }
        let f = fs::File::open(&path).unwrap();
        f.set_modified(two_hours_ago).unwrap();
        path
    };
    let gone = [
        old("wsp-put-a1b2c3.0", false),
        old("wsp-get-d4e5f6.0001", false),
        old("wsp-seed-ws_1.tgz", false),
        old("wsp-session-t1.gz", false),
        old("wsp-vault-1.tgz", false),
        old("wsp-land-a1b2c3", true),
        old("wsp-road-Xy12ab", true),
    ];
    let mut kept = vec![old("wsp-slates", true), old("other-file", false), old("tmp.Abc123", true)];
    let in_flight = dir.path().join("wsp-put-ffffff.0");
    fs::write(&in_flight, "x").unwrap();
    kept.push(in_flight);
    let cmd = NODE_SWEEP_STAGED_CMD.replace("find /tmp ", &format!("find {} ", dir.path().display()));
    assert!(std::process::Command::new("bash").args(["-c", &cmd]).status().unwrap().success());
    assert_eq!(gone.iter().filter(|p| p.exists()).count(), 0);
    assert_eq!(kept.iter().filter(|p| !p.exists()).count(), 0);
}

struct Recorder {
    said: Mutex<Vec<String>>,
}

impl Recorder {
    fn answer(&self, cmd: &str) -> Result<ExecResult> {
        self.said.lock().unwrap().push(cmd.to_string());
        Ok(ExecResult { exit_code: 0.into(), stdout: String::new(), stderr: String::new() })
    }
}

#[async_trait(?Send)]
impl Machine for Recorder {
    fn id(&self) -> &str {
        "bx_1"
    }
    fn kind(&self) -> MachineKind {
        MachineKind::Sandbox
    }
    async fn exec(&self, cmd: &str, _opts: ExecOptions) -> Result<ExecResult> {
        self.answer(cmd)
    }
    async fn run(&self, script: &str, _opts: RunOptions) -> Result<ExecResult> {
        self.answer(script)
    }
    async fn snapshot(&self, _: &str, _: MachineLife, _: SnapshotOptions) -> Result<String> {
        unreachable!()
    }
    async fn pause(&self, _: Option<u64>) -> Result<()> {
        unreachable!()
    }
    async fn resume(&self, _: Option<&wsp_engine::Cancel>) -> Result<()> {
        unreachable!()
    }
    async fn kill(&self) -> Result<()> {
        unreachable!()
    }
    async fn state(&self) -> Result<MachineState> {
        unreachable!()
    }
    async fn download_url(&self, _: &str) -> Result<String> {
        unreachable!()
    }
    async fn upload_url(&self, _: &str) -> Result<String> {
        unreachable!()
    }
}

// sweep-leftovers.test.ts:37
#[tokio::test]
async fn it_sweeps_the_staging_then_the_build_caches_on_the_machines_own_roads() {
    let machine = Recorder { said: Mutex::new(Vec::new()) };
    sweep_leftovers(&machine).await.unwrap();
    let said = machine.said.lock().unwrap().clone();
    assert_eq!(said[0], NODE_SWEEP_STAGED_CMD);
    assert!(said.iter().any(|c| c.contains("rm -rf /root/.npm /root/.cache/uv /root/.cache/go-build /root/.cache/node-gyp")));
}

// golden-tools.ts:181: a refused exec ends the sweep before the caches, and the caches' own failure is the sweep's.
#[tokio::test]
async fn a_failure_of_the_staging_exec_is_the_sweeps_and_no_cache_command_follows() {
    struct Refuses(Recorder);
    #[async_trait(?Send)]
    impl Machine for Refuses {
        fn id(&self) -> &str {
            "bx_1"
        }
        fn kind(&self) -> MachineKind {
            MachineKind::Sandbox
        }
        async fn exec(&self, cmd: &str, _: ExecOptions) -> Result<ExecResult> {
            self.0.said.lock().unwrap().push(cmd.to_string());
            Err(wsp_engine::errors::EngineError::Other { message: "link dropped".into(), code: None })
        }
        async fn run(&self, script: &str, _: RunOptions) -> Result<ExecResult> {
            self.0.said.lock().unwrap().push(script.to_string());
            unreachable!()
        }
        async fn snapshot(&self, _: &str, _: MachineLife, _: SnapshotOptions) -> Result<String> {
            unreachable!()
        }
        async fn pause(&self, _: Option<u64>) -> Result<()> {
            unreachable!()
        }
        async fn resume(&self, _: Option<&wsp_engine::Cancel>) -> Result<()> {
            unreachable!()
        }
        async fn kill(&self) -> Result<()> {
            unreachable!()
        }
        async fn state(&self) -> Result<MachineState> {
            unreachable!()
        }
        async fn download_url(&self, _: &str) -> Result<String> {
            unreachable!()
        }
        async fn upload_url(&self, _: &str) -> Result<String> {
            unreachable!()
        }
    }
    let machine = Refuses(Recorder { said: Mutex::new(Vec::new()) });
    let e = sweep_leftovers(&machine).await.expect_err("the exec's failure");
    assert_eq!(e.to_string(), "link dropped");
    assert_eq!(machine.0.said.lock().unwrap().len(), 1);
}
