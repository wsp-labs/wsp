// SPDX-License-Identifier: AGPL-3.0-only
//! The machine's two readings over a socket: sys.watch, sys.unwatch and sys.history for its load, and the proc ops
//! for its processes. A socket holds at most one watch of each, which its unwatch or its close ends.

use std::sync::{Arc, Mutex};

use wsp_frames::{numbers, DaemonErrorCode, DaemonOp, Empty, RequestId};

use super::{answer, not_built, ok, refuse, Conn};
use crate::paths::OpError;
use crate::proc::{kill_process, ProcSampler, ProtectedPids};
use crate::{readings, Ctx, Outbound};

/// Both proc ops refuse a pid above the Linux pid_max ceiling as a bad request, as the node daemon does.
fn pid_in_range(pid: std::num::NonZeroU32) -> Result<u32, OpError> {
    if pid.get() > numbers::PID_MAX {
        return Err(OpError::coded(DaemonErrorCode::BadRequest, format!("pid must be an integer between 1 and {}", numbers::PID_MAX)));
    }
    Ok(pid.get())
}

/// This socket's proc.watch, and every proc.unwatch it sent, so a watch still probing when one lands takes nothing on.
#[derive(Default)]
pub(super) struct ProcWatch(Mutex<ProcHeld>);

#[derive(Default)]
struct ProcHeld {
    held: Option<(u64, Arc<ProcSampler>)>,
    unwatches: u64,
}

impl ProcWatch {
    fn lock(&self) -> std::sync::MutexGuard<'_, ProcHeld> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Subscribes under the key, reading the cgroups it names, unless an unwatch came in since the watch arrived; a
    /// socket already watching that watches again names its cgroups afresh and missed a frame, so its next one is whole.
    fn watch(&self, sampler: Arc<ProcSampler>, key: u64, out: Outbound, since: u64, cgroups: Option<Vec<String>>) {
        let mut watch = self.lock();
        if watch.unwatches != since {
            return;
        }
        match watch.held.as_ref() {
            Some((held, sampler)) => sampler.rescope(*held, cgroups),
            None => {
                sampler.subscribe(key, out, cgroups);
                watch.held = Some((key, sampler));
            }
        }
    }

    /// Ends this socket's watch, if it holds one; the socket's close runs it too.
    pub(super) fn unwatch(&self) {
        let taken = {
            let mut watch = self.lock();
            watch.unwatches += 1;
            watch.held.take()
        };
        if let Some((key, sampler)) = taken {
            sampler.unsubscribe(key);
        }
    }
}

/// The ops `serve` answers.
pub(super) fn serves(op: &DaemonOp) -> bool {
    matches!(
        op,
        DaemonOp::SysWatch
            | DaemonOp::SysUnwatch
            | DaemonOp::SysHistory { .. }
            | DaemonOp::ProcWatch { .. }
            | DaemonOp::ProcUnwatch
            | DaemonOp::ProcInspect { .. }
            | DaemonOp::ProcList
            | DaemonOp::ProcKill { .. }
    )
}

pub(super) async fn serve(conn: &Arc<Conn>, ctx: &Arc<Ctx>, id: Option<RequestId>, name: &str, op: DaemonOp) -> String {
    match op {
        DaemonOp::SysWatch => {
            // Each frame runs in its own task, so an unwatch sent behind this watch lands while the probe reads; this
            // count is taken before anything awaits, in the order the frames came.
            let since = conn.sys_watch.unwatches();
            // One read before the watch is taken: a machine whose module cannot read it refuses here, where the pane
            // can say so, rather than accepting a stream it will never send and leaving the rows at pending.
            let watched = async {
                let sampler = ctx.sys_sampler()?;
                sampler.probe().await?;
                let key = ctx.next_key();
                conn.while_open(|| {
                    conn.sys_watch.watch(sampler, key, conn.out.clone(), since);
                    None
                });
                Ok(Empty {})
            };
            answer(id, watched.await)
        }
        DaemonOp::SysUnwatch => {
            conn.sys_watch.unwatch();
            ok(id)
        }
        DaemonOp::SysHistory { from, to, step_ms } => {
            let read = async {
                // A kind that reads no readings keeps none, and says so in the watch's own words.
                readings::readings_for(&ctx.options.kind, std::env::consts::OS)?;
                if step_ms == 0 || to <= from {
                    return Err(OpError::coded(
                        DaemonErrorCode::BadRequest,
                        "sys.history takes a from before its to and a step above zero",
                    ));
                }
                let history = Arc::clone(&ctx.history);
                tokio::task::spawn_blocking(move || history.read(from, to, step_ms, numbers::READINGS_POINTS_CAP))
                    .await
                    .map_err(|e| OpError::plain(e.to_string()))
            };
            answer(id, read.await)
        }
        DaemonOp::ProcWatch { cgroups } => {
            // Read before anything awaits, as sys.watch does.
            let since = conn.proc_watch.lock().unwatches;
            let watched = async {
                let sampler = ctx.proc_sampler()?;
                sampler.probe().await?;
                let key = ctx.next_key();
                conn.while_open(|| {
                    conn.proc_watch.watch(sampler, key, conn.out.clone(), since, cgroups);
                    None
                });
                Ok(Empty {})
            };
            answer(id, watched.await)
        }
        DaemonOp::ProcUnwatch => {
            conn.proc_watch.unwatch();
            ok(id)
        }
        DaemonOp::ProcInspect { pid } => {
            let inspected = async { ctx.proc_sampler()?.inspect(pid_in_range(pid)?).await };
            answer(id, inspected.await)
        }
        DaemonOp::ProcList => answer(id, async { ctx.proc_sampler()?.list().await }.await),
        DaemonOp::ProcKill { pid, signal } => {
            let protected = ProtectedPids { this: std::process::id(), parent: std::os::unix::process::parent_id() };
            answer(id, pid_in_range(pid).and_then(|pid| kill_process(pid, signal, protected)).map(|()| Empty {}))
        }
        _ => refuse(id, DaemonErrorCode::Unsupported, not_built(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::super::{handle, tests::conn};
    use crate::proc::fake_proc::{fake_passwd, fake_proc_tree, FakeProc};
    use crate::{Ctx, Options};
    use serde_json::{json, Value};
    use std::io::Write;
    use std::sync::Arc;

    #[tokio::test]
    async fn a_thousand_proc_watches_and_unwatches_on_one_socket_leave_its_close_list_as_it_opened() {
        let tree = fake_proc_tree(&[FakeProc { pid: 1, comm: Some("init".into()), ..FakeProc::default() }], 4096);
        let passwd = fake_passwd();
        let mut token = tempfile::NamedTempFile::new().unwrap();
        writeln!(token, "t").unwrap();
        let mut options = Options::new(token.path());
        options.proc_root = Some(tree.path().to_path_buf());
        options.passwd_path = Some(passwd.1.clone());
        options.root = Some(tree.path().to_path_buf());
        let ctx = Arc::new(Ctx::new(options, Box::new(|_| {}), 0).unwrap());
        let (c, _rx) = conn(None);
        let held = |c: &super::Conn| c.detaches.lock().unwrap().as_ref().map_or(0, Vec::len);
        let ask = |op: &'static str| {
            let (c, ctx) = (Arc::clone(&c), Arc::clone(&ctx));
            async move {
                let reply: Value = serde_json::from_str(handle(&c, &ctx, &json!({ "id": 1, "op": op }).to_string()).await.text()).unwrap();
                assert_eq!(reply["ok"], true, "{op}: {reply}");
            }
        };
        // One undo each for the sys and the proc watch, put in when the socket opened.
        assert_eq!(held(&c), 2);
        for _ in 0..1000 {
            ask("proc.watch").await;
            ask("proc.unwatch").await;
        }
        assert_eq!(held(&c), 2, "the close list grew over a thousand watches and unwatches");
        let sampler = ctx.proc_sampler().unwrap();
        assert!(!sampler.running());
        // The close still ends a watch the socket holds.
        ask("proc.watch").await;
        assert!(sampler.running());
        c.close();
        assert!(!sampler.running());
    }
}
