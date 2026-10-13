// SPDX-License-Identifier: AGPL-3.0-only
//! git and a git host's command line run on the computer this daemon is: the road every machine but a workspace on
//! a computer somebody owns takes, word for word what this daemon always did.

use std::path::{Path, PathBuf};
use std::process::Stdio;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::process::Command;
use wsp_frames::numbers;

use super::{GitResult, OnThisSide, Runs, GIT_ENV};
use crate::paths::OpError;
use crate::pipe::OpEnv;

/// This computer, and the PATH programs are looked up on: the daemon's own environment, or the one a caller names,
/// which is what a case hands in rather than writing the process's own environment from under every other case.
pub(crate) struct Here {
    path: String,
    op: Option<OpEnv>,
}

impl Here {
    pub(crate) fn new() -> Here {
        Here { path: std::env::var("PATH").unwrap_or_default(), op: None }
    }

    #[cfg(test)]
    pub(crate) fn on(path: &str) -> Here {
        Here { path: path.to_owned(), op: None }
    }

    /// The same computer, with every program this op runs reaching GitHub through the doors on the op's token.
    pub(crate) fn with_op(self, op: OpEnv) -> Here {
        Here { op: Some(op), ..self }
    }
}

impl Runs for Here {
    async fn run(
        &self,
        cwd: &Path,
        program: &str,
        args: &[&str],
        input: Option<&[u8]>,
        max_bytes: Option<usize>,
    ) -> Result<GitResult, OpError> {
        let mut command = command_of(&self.path, cwd, program, args, self.op.as_ref());
        let mut child = command.spawn()?;

        let stdin = child.stdin.take();
        let stdout = child.stdout.take();
        let stderr = child.stderr.take();
        let feed = async {
            if let (Some(mut stdin), Some(bytes)) = (stdin, input) {
                let _ = stdin.write_all(bytes).await;
            }
        };
        let read_out = async {
            let mut collected = Vec::new();
            let mut truncated = false;
            if let Some(mut pipe) = stdout {
                let mut buf = vec![0u8; 64 * 1024];
                while let Ok(n) = pipe.read(&mut buf).await {
                    if n == 0 {
                        break;
                    }
                    if truncated {
                        continue;
                    }
                    collected.extend_from_slice(&buf[..n]);
                    if max_bytes.is_some_and(|cap| collected.len() > cap) {
                        truncated = true;
                        let _ = child.start_kill();
                    }
                }
            }
            (collected, truncated)
        };
        let read_err = async {
            let mut collected = Vec::new();
            if let Some(mut pipe) = stderr {
                let _ = pipe.read_to_end(&mut collected).await;
            }
            String::from_utf8_lossy(&collected).into_owned()
        };
        let ((), (stdout, truncated), stderr) = tokio::join!(feed, read_out, read_err);
        let status = child.wait().await?;
        Ok(GitResult { code: status.code(), stdout, stderr, truncated })
    }

    async fn on_path(&self, program: &str) -> Result<bool, OpError> {
        Ok(on_path(&self.path, program).is_some())
    }

    /// The folder as named: on this computer it is one of the daemon's own roots, or a folder inside one, and a
    /// writer inside that folder cannot swap the folder itself.
    fn on_this_side(&self, folder: &Path) -> Option<OnThisSide> {
        Some(OnThisSide { open: folder.to_path_buf(), walk: PathBuf::new() })
    }
}

/// argv behind the work-score line, never interpolated into a shell: sh runs the line, then execs the program in
/// its own place.
/// An op's token rides the environment and never argv, which every process on the machine can read.
fn command_of(path: &str, cwd: &Path, program: &str, args: &[&str], op: Option<&OpEnv>) -> Command {
    let mut command = Command::new("/bin/sh");
    command
        .arg("-c")
        .arg(format!("{}; exec \"$0\" \"$@\"", numbers::work_score_line()))
        .arg(program)
        .args(args)
        .current_dir(cwd)
        .env("PATH", path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    for (name, value) in GIT_ENV {
        command.env(name, value);
    }
    if let Some(op) = op {
        command.envs(op.git.iter().map(|(k, v)| (k, v)));
        if program == "gh" {
            command.envs(op.gh.iter().map(|(k, v)| (k, v)));
        }
    }
    command
}

/// The program on that PATH, by the same reading a shell makes: the first entry holding a file that can be run.
pub(crate) fn on_path(path: &str, program: &str) -> Option<PathBuf> {
    path.split(':').filter(|dir| !dir.is_empty()).map(|dir| Path::new(dir).join(program)).find(|at| at.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_program_is_found_on_the_path_the_caller_named_and_nowhere_else() {
        let dir = tempfile::Builder::new().prefix("wsp-host-cli-").tempdir().unwrap();
        let bin = dir.path().join("gh");
        std::fs::write(&bin, "#!/bin/sh\n").unwrap();
        assert_eq!(on_path(&dir.path().to_string_lossy(), "gh"), Some(bin));
        assert_eq!(on_path("", "gh"), None);
        assert_eq!(on_path(&dir.path().to_string_lossy(), "glab"), None);
    }

    #[tokio::test]
    async fn the_program_runs_in_the_directory_with_the_git_environment_and_the_work_score() {
        let dir = tempfile::tempdir().unwrap();
        let res = Here::new()
            .run(dir.path(), "sh", &["-c", "pwd; echo \"$GIT_OPTIONAL_LOCKS|$LC_ALL|$GIT_TERMINAL_PROMPT\""], None, None)
            .await
            .unwrap();
        assert_eq!(res.code, Some(0), "{}", res.stderr);
        let out = String::from_utf8_lossy(&res.stdout).into_owned();
        let said: Vec<&str> = out.lines().collect();
        assert_eq!(said.first().map(|p| std::fs::canonicalize(p).unwrap()), Some(std::fs::canonicalize(dir.path()).unwrap()));
        assert_eq!(said.get(1), Some(&"0|C|0"));
        // And a program nothing on the PATH holds is not there, which is what a pull request with no gh reads.
        assert!(!Here::on("").on_path("gh").await.unwrap());
    }

    /// Stand-ins for git and gh that print every argument and then their whole environment.
    fn printing(dir: &Path) {
        use std::os::unix::fs::PermissionsExt as _;
        for name in ["git", "gh"] {
            let at = dir.join(name);
            std::fs::write(&at, "#!/bin/sh\nfor a in \"$@\"; do echo \"ARG $a\"; done\nenv\n").unwrap();
            std::fs::set_permissions(&at, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
    }

    /// The lines a stand-in printed that hold the token anywhere in them.
    async fn holding(here: &Here, cwd: &Path, program: &str, token: &str) -> (Vec<String>, Vec<String>) {
        let res = here.run(cwd, program, &["push", "-u", "--", "origin", "wsp/fix"], None, None).await.unwrap();
        assert_eq!(res.code, Some(0), "{}", res.stderr);
        let out = String::from_utf8_lossy(&res.stdout).into_owned();
        let args = out.lines().filter(|l| l.starts_with("ARG ")).map(str::to_owned).collect();
        (args, out.lines().filter(|l| l.contains(token)).map(str::to_owned).collect())
    }

    #[tokio::test]
    async fn an_op_token_is_gits_proxy_password_and_ghs_token_and_nowhere_else_and_without_one_nothing_changes() {
        let bin = tempfile::tempdir().unwrap();
        printing(bin.path());
        let path = format!("{}:/usr/bin:/bin", bin.path().display());
        let wsp = tempfile::tempdir().unwrap();
        let doors = crate::pipe::Doors {
            http: "127.0.0.1:7071".parse().unwrap(),
            gh: wsp.path().join("broker.sock"),
            hold_max: std::time::Duration::ZERO,
        };
        let token = "op-tok-9";
        let op = Here::on(&path).with_op(crate::pipe::op_env(token, &doors));
        let cwd = tempfile::tempdir().unwrap();

        let (args, seen) = holding(&op, cwd.path(), "git", token).await;
        assert_eq!(args, ["ARG push", "ARG -u", "ARG --", "ARG origin", "ARG wsp/fix"]);
        assert_eq!(seen, ["GIT_CONFIG_VALUE_4=http://wsp:op-tok-9@127.0.0.1:7071"], "git holds the token somewhere but its proxy password");

        let (_, seen) = holding(&op, cwd.path(), "gh", token).await;
        let mut seen = seen;
        seen.sort();
        assert_eq!(seen, ["GH_TOKEN=op-tok-9", "GIT_CONFIG_VALUE_4=http://wsp:op-tok-9@127.0.0.1:7071"]);
        // gh reads the configuration the gh door wrote beside itself as it opened, the one a turn's gh reads too.
        let gh = op.run(cwd.path(), "gh", &[], None, None).await.unwrap();
        let dir = format!("GH_CONFIG_DIR={}", wsp.path().join("gh").display());
        assert!(String::from_utf8_lossy(&gh.stdout).lines().any(|l| l == dir), "{}", String::from_utf8_lossy(&gh.stdout));

        // Without a token the same line runs as it always has: the same arguments, and none of the op's variables.
        let plain = Here::on(&path).run(cwd.path(), "git", &["push", "-u", "--", "origin", "wsp/fix"], None, None).await.unwrap();
        let out = String::from_utf8_lossy(&plain.stdout).into_owned();
        assert!(out.lines().all(|l| !l.starts_with("GIT_CONFIG_") && !l.contains(token)), "{out}");
        assert_eq!(out.lines().filter(|l| l.starts_with("ARG ")).collect::<Vec<_>>(), args);
    }
}
