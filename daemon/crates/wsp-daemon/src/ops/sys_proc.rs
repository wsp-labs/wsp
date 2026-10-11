// SPDX-License-Identifier: AGPL-3.0-only
//! The machine's two readings over a socket: sys.watch, sys.unwatch and sys.history for its load, and the proc ops
//! for its processes. A socket holds at most one watch of each, which its unwatch or its close ends.

use std::sync::Arc;

use wsp_frames::{numbers, DaemonErrorCode, DaemonOp, Empty, RequestId};

use super::{answer, not_built, ok, refuse, Conn, Detach};
use crate::paths::OpError;
use crate::proc::{kill_process, ProtectedPids};
use crate::{readings, Ctx};

/// Both proc ops refuse a pid above the Linux pid_max ceiling as a bad request, as the node daemon does.
fn pid_in_range(pid: std::num::NonZeroU32) -> Result<u32, OpError> {
    if pid.get() > numbers::PID_MAX {
        return Err(OpError::coded(DaemonErrorCode::BadRequest, format!("pid must be an integer between 1 and {}", numbers::PID_MAX)));
    }
    Ok(pid.get())
}

pub(super) async fn serve(conn: &Arc<Conn>, ctx: &Arc<Ctx>, id: Option<RequestId>, name: &str, op: DaemonOp) -> String {
    match op {
        DaemonOp::SysWatch => {
            // One read before the watch is taken: a machine whose module cannot read it refuses here, where the pane
            // can say so, rather than accepting a stream it will never send and leaving the rows at pending.
            let watched = async {
                let sampler = ctx.sys_sampler()?;
                sampler.probe().await?;
                let key = ctx.next_key();
                conn.while_open(|| {
                    conn.sys_watch.watch(sampler, key, conn.out.clone());
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
        DaemonOp::ProcWatch => {
            let watched = async {
                let sampler = ctx.proc_sampler()?;
                sampler.probe().await?;
                let key = ctx.next_key();
                conn.while_open(|| {
                    let mut watch = conn.proc_watch.lock().unwrap_or_else(|e| e.into_inner());
                    // A socket already watching that watches again missed a frame: its next one is whole.
                    if let Some((held, sampler)) = watch.as_ref() {
                        sampler.resend(*held);
                        return None;
                    }
                    sampler.subscribe(key, conn.out.clone());
                    *watch = Some((key, Arc::clone(&sampler)));
                    Some(Box::new(move || sampler.unsubscribe(key)) as Detach)
                });
                Ok(Empty {})
            };
            answer(id, watched.await)
        }
        DaemonOp::ProcUnwatch => {
            let watch = conn.proc_watch.lock().unwrap_or_else(|e| e.into_inner()).take();
            if let Some((key, sampler)) = watch {
                sampler.unsubscribe(key);
            }
            ok(id)
        }
        DaemonOp::ProcInspect { pid } => {
            let inspected = async { ctx.proc_sampler()?.inspect(pid_in_range(pid)?).await };
            answer(id, inspected.await)
        }
        DaemonOp::ProcKill { pid, signal } => {
            let protected = ProtectedPids { this: std::process::id(), parent: std::os::unix::process::parent_id() };
            answer(id, pid_in_range(pid).and_then(|pid| kill_process(pid, signal, protected)).map(|()| Empty {}))
        }
        _ => refuse(id, DaemonErrorCode::Unsupported, not_built(name)),
    }
}
