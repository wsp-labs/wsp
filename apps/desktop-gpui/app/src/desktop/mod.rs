// SPDX-License-Identifier: AGPL-3.0-only
//! What the desktop shell does around the window (`apps/desktop/src/main.ts`): the switch between the two apps, links,
//! zoom, the Linux frame, and the host feed behind the badge, the tray, notifications, keep-awake and the Quit box; and
//! what Settings asks of this computer apart from the host (`service:login`, `service:login-set`, `logs:open`): whether
//! the host's service starts at login, and opening the app's log folder, one module per platform behind [`Desktop`].

pub mod frame;
pub mod links;
pub mod switch;
pub mod zoom;

pub mod awake;
pub mod feed;
pub mod mac;
pub mod notify;
pub mod quit;
pub mod tray;

pub mod app_procs;
mod linux;
mod macos;

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use gpui_kit::Global;
use sha2::{Digest, Sha256};
use wsp_host_client::Env;

/// The state file this launch serves and the environment it was started with, as `where()` in Electron's main reads
/// its launch.
pub struct Launched {
    pub state: PathBuf,
    pub env: Env,
}

impl Global for Launched {}

/// Which service this is (`serviceAddressHere`): one per state file, under this login's home.
pub struct Address {
    pub state: PathBuf,
    pub home: PathBuf,
}

impl Launched {
    pub fn address(&self) -> Address {
        Address { state: std::path::absolute(&self.state).unwrap_or_else(|_| self.state.clone()), home: self.env_path("HOME") }
    }

    /// `appLogsDir(homeOf(launch))`: the logs folder under the wsp home this launch means, which `wsp doctor` names and
    /// the Electron app writes its log and crash reports into.
    pub fn logs(&self) -> PathBuf {
        let home = self.env.get("WSP_HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
        let home = home.unwrap_or_else(|| self.env_path("HOME").join(".wsp"));
        std::path::absolute(&home).unwrap_or(home).join("logs")
    }

    fn env_path(&self, name: &str) -> PathBuf {
        PathBuf::from(self.env.get(name).cloned().unwrap_or_default())
    }
}

/// One platform's own roads.
pub trait Desktop: Sync {
    /// Whether the host's service for `at` starts at every login (`loginStart`); nothing where no service is
    /// registered for it or the manager's answer said neither.
    fn starts_at_login(&self, at: &Address, env: &Env) -> Option<bool>;
    /// Sets that, leaving the service running either way (`setLoginStart`); a refusal is the manager's own line.
    fn set_login_start(&self, at: &Address, on: bool, env: &Env) -> Result<(), String>;
    /// Opens a folder in the computer's file manager (`shell.openPath`); a folder that is not there is refused in the
    /// words `shell.openPath` answers with on this platform, and nothing is made.
    fn open(&self, folder: &Path, env: &Env) -> Result<(), String>;
}

/// This platform's, where wsp has one.
pub fn here() -> Option<&'static dyn Desktop> {
    if cfg!(target_os = "macos") {
        Some(&macos::Mac)
    } else if cfg!(target_os = "linux") {
        Some(&linux::Linux)
    } else {
        None
    }
}

/// `serviceTag`: the first eight hex of the state path's digest, which every manager's name for the service carries.
pub fn service_tag(state: &Path) -> String {
    Sha256::digest(state.to_string_lossy().as_bytes()).iter().take(4).map(|b| format!("{b:02x}")).collect()
}

/// What a command answered: its exit code and its output, stdout then stderr (`RunResult`).
struct Answer {
    code: i32,
    output: String,
}

/// Runs a program named by its full path on the environment's `names` alone.
fn run(argv: &[&str], names: &[&str], env: &Env) -> Result<Answer, String> {
    let (program, args) = argv.split_first().ok_or("nothing to run")?;
    let kept = names.iter().filter_map(|name| Some((*name, env.get(*name)?)));
    let out = Command::new(program).args(args).env_clear().envs(kept).output().map_err(|e| format!("{program}: {e}"))?;
    let output = format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
    Ok(Answer { code: out.status.code().unwrap_or(127), output })
}

/// Starts a program named by its full path on the environment's `names` alone and lets it run: its exit is reaped off
/// the caller's thread and read by nobody.
fn start(argv: &[&str], names: &[&str], env: &Env) -> Result<(), String> {
    let (program, args) = argv.split_first().ok_or("nothing to run")?;
    let kept = names.iter().filter_map(|name| Some((*name, env.get(*name)?)));
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .envs(kept)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("{program}: {e}"))?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Runs it and takes anything but 0 as a refusal, in `runFailureLine`'s words.
fn run_ok(argv: &[&str], names: &[&str], env: &Env) -> Result<(), String> {
    let answer = run(argv, names, env)?;
    if answer.code == 0 {
        return Ok(());
    }
    let words = answer.output.split_whitespace().collect::<Vec<_>>().join(" ");
    let said = if words.is_empty() { "and said nothing".to_owned() } else { format!("and said: {words}") };
    Err(format!("{} exited {} {said}", argv.join(" "), answer.code))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn launched(env: &[(&str, &str)]) -> Launched {
        Launched { state: PathBuf::from("/h/.wsp/state.json"), env: env.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect() }
    }

    #[test]
    fn the_tag_is_the_hosts() {
        // node -e 'console.log(require("crypto").createHash("sha256").update("/Users/a/.wsp/state.json").digest("hex").slice(0, 8))'
        assert_eq!(service_tag(Path::new("/Users/a/.wsp/state.json")), "2f71d6c9");
    }

    #[test]
    fn the_logs_sit_under_the_wsp_home() {
        assert_eq!(launched(&[("HOME", "/h")]).logs(), PathBuf::from("/h/.wsp/logs"));
        assert_eq!(launched(&[("HOME", "/h"), ("WSP_HOME", "/w")]).logs(), PathBuf::from("/w/logs"));
        assert_eq!(launched(&[("HOME", "/h"), ("WSP_HOME", "")]).logs(), PathBuf::from("/h/.wsp/logs"));
    }

    #[test]
    fn a_child_sees_only_the_names_it_is_given() {
        let env = launched(&[("HOME", "/h"), ("SECRET", "x")]).env;
        let answer = run(&["/usr/bin/env"], &["HOME", "MISSING"], &env).unwrap();
        // Names only, so a failure never prints a value of this process's own environment.
        let names: Vec<&str> = answer.output.lines().filter_map(|line| line.split('=').next()).collect();
        assert_eq!(names, ["HOME"]);
        let refused = run_ok(&["/bin/sh", "-c", "echo no  way >&2; exit 3"], &[], &env).unwrap_err();
        assert_eq!(refused, "/bin/sh -c echo no  way >&2; exit 3 exited 3 and said: no way");
    }

    #[test]
    fn a_started_program_is_not_waited_on() {
        let at = std::time::Instant::now();
        start(&["/bin/sleep", "3"], &[], &Env::default()).unwrap();
        assert!(at.elapsed() < std::time::Duration::from_secs(1), "{:?}", at.elapsed());
        assert!(start(&["/nowhere/xdg-open"], &[], &Env::default()).unwrap_err().starts_with("/nowhere/xdg-open: "));
    }

    #[test]
    fn a_missing_folder_is_refused_and_not_made() {
        let Some(road) = here() else { return };
        let home = std::env::temp_dir().join(format!("wsp-open-{}", std::process::id()));
        let folder = home.join(".wsp/logs");
        let said = road.open(&folder, &Env::default()).unwrap_err();
        assert!(["The item does not exist", "Failed to open path"].contains(&said.as_str()), "{said}");
        assert!(!home.exists());
    }
}
