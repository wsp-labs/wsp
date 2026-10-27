// SPDX-License-Identifier: AGPL-3.0-only
//! join A (#2476): a launch that panics ends the stream with a failure, as a throw in the TS's upload is a rejected
//! promise (machine-exec.ts `launched`), and never leaves `opened` waiting for good.
mod support;

use std::sync::Arc;
use std::time::Duration;

use futures::future::LocalBoxFuture;
use support::guest::{Hook, Inner, ScriptGuest};
use wsp_adapters::port::{ExecOptions as Launch, ExecStreamFactory};
use wsp_core::machine_exec::{machine_exec_stream, MachineExecOptions};
use wsp_engine::errors::Result as EngineResult;
use wsp_engine::machine::ExecResult;

#[test]
fn a_launch_that_panics_ends_the_stream_with_a_failure_and_never_hangs() {
    support::run_local(async {
        let (guest, machine) = ScriptGuest::new(vec![]);
        let hook: Hook =
            Arc::new(|cmd: String, _stdin: Option<Vec<u8>>, _inner: Inner| -> LocalBoxFuture<'static, EngineResult<ExecResult>> {
                Box::pin(async move {
                    if cmd.contains("WSP_LAUNCHED") {
                        panic!("the launch broke");
                    }
                    Ok(ExecResult { exit_code: support::code(0), stdout: String::new(), stderr: String::new() })
                })
            });
        guest.set_hook(hook);
        let factory = machine_exec_stream(machine, MachineExecOptions { poll_ms: Some(5), ..Default::default() }, None);
        let stream = factory.launch("true", Launch::default());
        let ended = tokio::time::timeout(Duration::from_secs(5), support::collect(&*stream)).await;
        let (_, failure) = ended.expect("the stream ends, not waits for good");
        let failure = failure.expect("a failure");
        assert!(failure.message.contains("the launch broke"), "{}", failure.message);
    });
}
