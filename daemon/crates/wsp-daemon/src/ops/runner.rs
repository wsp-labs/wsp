// SPDX-License-Identifier: AGPL-3.0-only

use std::io::Read;
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

/// A returned branch is a file in this checkout, never a path on a different workspace or computer.
pub(super) async fn returned_bundle(under: &Path, named: &str) -> Result<Vec<u8>, OpError> {
    let rel = named.strip_prefix("./").ok_or_else(|| crate::paths::outside_root(named))?;
    let parts: Vec<&str> = rel.split('/').collect();
    if parts.len() != 2
        || parts[1] != "result.bundle"
        || !parts[0].strip_prefix(".wsp-return-").is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric()))
    {
        return Err(crate::paths::outside_root(named));
    }
    let root = under.to_path_buf();
    let relative = rel.to_owned();
    let said = named.to_owned();
    crate::fs::blocking(move || {
        let folder = nix::fcntl::open(&root, crate::beneath::dir_flags(), nix::sys::stat::Mode::empty())
            .map_err(|e| OpError::plain(e.to_string()))?;
        let file = crate::beneath::file(&folder, &relative).map_err(OpError::plain)?.ok_or_else(|| crate::paths::not_found(&said))?;
        const CAP: u64 = 64 * 1024 * 1024;
        if file.metadata()?.len() > CAP {
            return Err(OpError::plain("The returned branch exceeds 64 MB. Export its changes in smaller commits before retrying."));
        }
        let mut bytes = Vec::new();
        file.take(CAP + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > CAP {
            return Err(OpError::plain("The returned branch grew beyond 64 MB. Retry the return."));
        }
        Ok(bytes)
    })
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[tokio::test]
    async fn returned_bundles_are_read_by_descriptor_and_survive_a_path_swap() {
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let under = root.path().join("rootfs/root/project");
        std::fs::create_dir_all(under.join(".wsp-return-ABC123")).unwrap();
        std::fs::write(under.join(".wsp-return-ABC123/result.bundle"), "bundle").unwrap();
        assert_eq!(returned_bundle(&under, "./.wsp-return-ABC123/result.bundle").await.unwrap(), b"bundle");
        for path in [
            "./.wsp-return-ABC123/../result.bundle",
            "./.wsp-return-/result.bundle",
            "./.wsp-return-ABC123/result.bundle/extra",
            "/root/.wsp-return-ABC123/result.bundle",
        ] {
            assert!(returned_bundle(&under, path).await.is_err(), "{path}");
        }
        std::fs::write(outside.path().join("result.bundle"), "private").unwrap();
        symlink(outside.path(), under.join(".wsp-return-escape")).unwrap();
        assert!(returned_bundle(&under, "./.wsp-return-escape/result.bundle").await.is_err());
        let kept = returned_bundle(&under, "./.wsp-return-ABC123/result.bundle").await.unwrap();
        std::fs::remove_dir_all(under.join(".wsp-return-ABC123")).unwrap();
        symlink(outside.path(), under.join(".wsp-return-ABC123")).unwrap();
        assert_eq!(kept, b"bundle");
        assert!(returned_bundle(&under, "./.wsp-return-ABC123/result.bundle").await.is_err());
        std::fs::remove_file(under.join(".wsp-return-ABC123")).unwrap();
        std::fs::create_dir(under.join(".wsp-return-ABC123")).unwrap();
        std::fs::create_dir(under.join(".wsp-return-ABC123/result.bundle")).unwrap();
        assert!(returned_bundle(&under, "./.wsp-return-ABC123/result.bundle").await.is_err());
    }
}
