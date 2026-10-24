// SPDX-License-Identifier: AGPL-3.0-only
//! Today's app replacing itself with this one at the same path: its own swap (`SWAP_SCRIPT` in `self-update.ts`), read
//! out of today's source and run as today's app runs it, with the Mac tools it names by path stood in for by fakes in a
//! temp folder, so no launchctl, codesign or open of the system's runs.

use std::os::unix::fs::PermissionsExt as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use super::swap::{self, Settled};
use super::tests::{quit_app, tmp, until};
use super::*;

const TODAYS: &str = include_str!("../../../../desktop/src/self-update.ts");
const PATIENCE_S: u32 = 6;

/// Today's `SWAP_SCRIPT` as the shell gets it: the template literal's text with its `\${` read as `${`.
fn todays_script(fakes: &Path) -> String {
    let from = TODAYS.find("export const SWAP_SCRIPT = `").expect("today's app still has its SWAP_SCRIPT") + 28;
    let to = from + TODAYS[from..].find("`;").expect("the script ends");
    let mut script = TODAYS[from..to].replace("\\${", "${");
    for tool in ["/usr/bin/plutil", "/usr/bin/codesign", "/bin/launchctl", "/usr/bin/open"] {
        let name = tool.rsplit('/').next().unwrap();
        assert!(script.contains(tool), "today's swap names {tool}");
        script = script.replace(tool, &fakes.join(name).to_string_lossy());
    }
    script
}

fn write_exe(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

/// A bundle at `app` whose Info.plist names `exe` at `version`, its executable running `then`.
fn bundle(app: &Path, exe: &str, version: &str, then: &str) {
    let plist = format!(
        "<plist version=\"1.0\">\n<dict>\n  <key>CFBundleExecutable</key>\n  <string>{exe}</string>\n  <key>CFBundleShortVersionString</key>\n  <string>{version}</string>\n</dict>\n</plist>\n"
    );
    std::fs::create_dir_all(app.join("Contents")).unwrap();
    std::fs::write(app.join("Contents/Info.plist"), plist).unwrap();
    write_exe(&app.join("Contents/MacOS").join(exe), &format!("#!/bin/sh\n{then}\n"));
}

struct Scene {
    _dir: tempfile::TempDir,
    root: PathBuf,
    installed: PathBuf,
    stage: PathBuf,
    log: PathBuf,
    script: Child,
}

/// A run that failed midway leaves no swap waiting and no stand-in app behind.
impl Drop for Scene {
    fn drop(&mut self) {
        _ = self.script.kill();
        _ = self.script.wait();
        if let Some(pid) = self.opened() {
            // SAFETY: the pid is the stand-in app this scene's swap started.
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
}

impl Scene {
    fn log(&self) -> String {
        std::fs::read_to_string(&self.log).unwrap_or_default()
    }

    fn calls(&self, tool: &str) -> String {
        std::fs::read_to_string(self.root.join(format!("{tool}.calls"))).unwrap_or_default()
    }

    /// The pid of the app the swap opened, as its log names it.
    fn opened(&self) -> Option<i32> {
        self.log().lines().find_map(|l| l.split(" as pid ").nth(1)).and_then(|pid| pid.trim().parse().ok())
    }
}

/// Today's wsp.app at 0.3.8 in Applications, this app at 0.3.9 staged and checked beside it as today's `stageUpdate`
/// leaves it, and today's swap started on them with its tools faked, as today's app starts it and then quits.
fn todays_swap() -> Scene {
    let dir = tmp();
    let root = std::fs::canonicalize(dir.path()).unwrap();
    let fakes = root.join("fakes");
    let calls = |tool: &str| root.join(format!("{tool}.calls")).to_string_lossy().into_owned();
    write_exe(&fakes.join("plutil"), "#!/bin/sh\nsed -n \"/<key>$2<\\/key>/{n;s/.*<string>\\(.*\\)<\\/string>.*/\\1/p;}\" \"$6\"\n");
    for tool in ["codesign", "launchctl", "open"] {
        write_exe(&fakes.join(tool), &format!("#!/bin/sh\necho \"$*\" >> '{}'\n", calls(tool)));
    }
    let apps = root.join("Applications");
    let installed = apps.join("wsp.app");
    bundle(&installed, "wsp", "0.3.8", "exit 0");
    let stage_root = apps.join(swap::TODAYS_STAGE);
    let stage = stage_root.join("stage-AbCd12");
    let staged = stage.join("new/wsp.app");
    bundle(&staged, stage::EXE, "0.3.9", "exec sleep 30");
    std::fs::set_permissions(&stage_root, std::fs::Permissions::from_mode(0o700)).unwrap();
    let log = root.join("update.log");
    let path = |p: &Path| p.to_string_lossy().into_owned();
    let args = [
        quit_app().to_string(),
        path(&installed),
        path(&stage_root),
        path(&stage),
        path(&staged),
        stage::EXE.to_owned(),
        "0.3.9".to_owned(),
        "gui/501/com.wsp.host.2f71d6c9".to_owned(),
        PATIENCE_S.to_string(),
        "1".to_owned(),
        path(&log),
    ];
    let script = Command::new("/bin/sh")
        .arg("-c")
        .arg(todays_script(&fakes))
        .arg("wsp-update")
        .args(args)
        .env_clear()
        .env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin")
        .env("HOME", &root)
        .spawn()
        .unwrap();
    Scene { _dir: dir, root, installed, stage: stage_root, log, script }
}

/// Catches: this app, opened by today's swap in today's place, never writing the `up` today's swap waits on in its own
/// stage, so the swap times out, stops this app, puts today's app back and restarts the service onto it: no one moves.
#[test]
fn todays_swap_into_this_app_hears_up_and_keeps_it() {
    let mut s = todays_swap();
    until("today's swap opened this app", Duration::from_secs(10), || s.stage.join("opening").exists());
    assert_eq!(s.calls("launchctl").trim(), "kickstart -k gui/501/com.wsp.host.2f71d6c9", "the service restarted before the open");
    let settled = swap::settle(&s.installed);
    assert!(matches!(settled, Settled::Waiting(_)), "this app's launch reads today's swap as waiting on it: {settled:?}");
    let waiting = after_swap(Some(settled), || Ok(crate::service::Followed::Restarted), || panic!("nothing to move back"))
        .unwrap()
        .expect("the swap is handed back to the launch");
    waiting.up("0.3.9").unwrap();
    let done = s.script.wait().unwrap();
    assert!(done.success(), "{}", s.log());
    assert!(s.log().contains("wsp 0.3.9 is up") && s.log().contains("deleted the old app"), "{}", s.log());
    assert!(!s.stage.exists(), "today's app and its stage are gone");
    assert!(s.installed.join("Contents/MacOS").join(stage::EXE).is_file(), "this app stands in today's place");
    assert!(s.opened().is_some() && s.calls("open").is_empty(), "this app was opened by the swap and today's never again");
}

/// Catches: this app answering today's swap with another version than the one today's update checked in its Info.plist,
/// which today's swap never takes as up: the swap's `up` is this app's release (`HostClient::app_release`), and the
/// package stamps CFBundleShortVersionString from apps/desktop/package.json.
#[test]
fn the_release_this_app_says_up_with_is_the_one_its_package_stamps() {
    let desktop: serde_json::Value =
        serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../desktop/package.json"))).unwrap();
    assert_eq!(wsp_host_client::HostClient::app_release(), desktop["version"].as_str().unwrap());
    let package = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../scripts/package.sh")).unwrap();
    assert!(package.contains("../desktop/package.json"), "package.sh stamps the bundle's version from apps/desktop");
}

/// Catches: this app sweeping today's stage at a launch no swap of today's waits on, which takes a download today's app
/// standing beside it checked and holds for its own Restart.
#[test]
fn todays_stage_is_left_where_no_swap_waits_on_this_app() {
    let dir = tmp();
    let installed = dir.path().join("Applications/wsp.app");
    std::fs::create_dir_all(&installed).unwrap();
    let staged = dir.path().join("Applications").join(swap::TODAYS_STAGE).join("stage-AbCd12/new/wsp.app");
    std::fs::create_dir_all(&staged).unwrap();
    std::fs::write(staged.parent().unwrap().parent().unwrap().parent().unwrap().join("opening"), quit_app().to_string()).unwrap();
    assert_eq!(swap::settle(&installed), Settled::Nothing);
    assert!(staged.exists(), "today's checked download stays for today's app");
}
