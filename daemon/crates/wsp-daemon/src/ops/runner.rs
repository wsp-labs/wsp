// SPDX-License-Identifier: AGPL-3.0-only

use std::path::{Path, PathBuf};

use wsp_frames::{words, DaemonErrorCode};

use crate::git;
use crate::paths::OpError;
use crate::{pipe, Ctx};

/// Which way an op runs a program, and so which machine it is answered for: this computer, or one workspace this
/// computer holds. One enum rather than a generic on every arm, and one module behind each way.
pub(super) enum Runner {
    Here(git::here::Here),
    #[cfg(target_os = "linux")]
    Inside(git::inside::Inside),
}

/// The road of the six ops that may reach GitHub, git.push, git.pr, git.prCheckout, git.update, git.startOn and
/// git.mergeIn: the road every op takes, with the op's token where the frame carries one.
pub(super) async fn op_road(
    ctx: &Ctx,
    machine: Option<&str>,
    requested: &str,
    token: Option<&str>,
) -> Result<(Runner, PathBuf, PathBuf), OpError> {
    let (runner, under, at) = super::road(ctx, machine, requested, git::Asked::Work).await?;
    Ok((runner.with_op(ctx, token)?, under, at))
}

impl Runner {
    /// The runner an op runs its git and gh through: as it is where the frame carries no token, and with the token
    /// as git's proxy password and gh's GH_TOKEN where it does. A token reaches only a daemon serving the doors,
    /// which is a provider's machine's and so always runs its git here.
    fn with_op(self, ctx: &Ctx, token: Option<&str>) -> Result<Runner, OpError> {
        let Some(token) = token else { return Ok(self) };
        let Some(doors) = ctx.doors.get() else {
            return Err(OpError::coded(DaemonErrorCode::BadRequest, words::OP_TOKEN_NO_DOOR.to_owned()));
        };
        match self {
            Runner::Here(here) => Ok(Runner::Here(here.with_op(pipe::op_env(token, doors)))),
            #[cfg(target_os = "linux")]
            Runner::Inside(_) => Err(OpError::coded(DaemonErrorCode::BadRequest, words::OP_TOKEN_ON_A_COMPUTER.to_owned())),
        }
    }
}

impl git::Runs for Runner {
    async fn run(
        &self,
        cwd: &Path,
        program: &str,
        args: &[&str],
        input: Option<&[u8]>,
        max_bytes: Option<usize>,
    ) -> Result<git::GitResult, OpError> {
        match self {
            Runner::Here(here) => here.run(cwd, program, args, input, max_bytes).await,
            #[cfg(target_os = "linux")]
            Runner::Inside(inside) => inside.run(cwd, program, args, input, max_bytes).await,
        }
    }

    async fn on_path(&self, program: &str) -> Result<bool, OpError> {
        match self {
            Runner::Here(here) => here.on_path(program).await,
            #[cfg(target_os = "linux")]
            Runner::Inside(inside) => inside.on_path(program).await,
        }
    }

    fn on_this_side(&self, folder: &Path) -> Option<git::OnThisSide> {
        match self {
            Runner::Here(here) => here.on_this_side(folder),
            #[cfg(target_os = "linux")]
            Runner::Inside(inside) => inside.on_this_side(folder),
        }
    }
}
