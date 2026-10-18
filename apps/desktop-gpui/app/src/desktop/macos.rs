// SPDX-License-Identifier: AGPL-3.0-only
//! macOS: the host's service is a launchd agent in the login's own domain (`launchd` in packages/host's service.ts),
//! and a folder opens through open.

use std::path::Path;

use wsp_host_client::Env;

use super::{run, run_ok, service_tag, Address, Desktop};

pub struct Mac;

const LAUNCHCTL: &str = "/bin/launchctl";
const OPEN: &str = "/usr/bin/open";
/// What Electron's `shell.openPath` answers on macOS for a path NSWorkspace does not open.
const MISSING: &str = "Failed to open path";
const ENV: &[&str] = &["HOME"];

/// `launchdName` for the host's role.
fn label(at: &Address) -> String {
    format!("com.wsp.host.{}", service_tag(&at.state))
}

fn domain() -> String {
    // SAFETY: getuid has no preconditions and cannot fail.
    format!("gui/{}", unsafe { libc::getuid() })
}

/// `startsAtLogin`: launchd keeps a label's switch in its overrides, which print-disabled lists as `"label" =>
/// disabled`, or `=> true` before macOS 13; a label never switched is absent, and starts.
fn starts(label: &str, code: i32, output: &str) -> Option<bool> {
    (code == 0).then(|| !["disabled", "true"].iter().any(|said| output.contains(&format!("\"{label}\" => {said}"))))
}

impl Desktop for Mac {
    fn starts_at_login(&self, at: &Address, env: &Env) -> Option<bool> {
        let label = label(at);
        // `registeredIn`: the plist standing is what makes the service this state file's.
        if !at.home.join("Library/LaunchAgents").join(format!("{label}.plist")).exists() {
            return None;
        }
        let answer = run(&[LAUNCHCTL, "print-disabled", &domain()], ENV, env).ok()?;
        starts(&label, answer.code, &answer.output)
    }

    fn set_login_start(&self, at: &Address, on: bool, env: &Env) -> Result<(), String> {
        run_ok(&[LAUNCHCTL, if on { "enable" } else { "disable" }, &format!("{}/{}", domain(), label(at))], ENV, env)
    }

    fn open(&self, folder: &Path, env: &Env) -> Result<(), String> {
        if !folder.exists() {
            return Err(MISSING.to_owned());
        }
        run_ok(&[OPEN, &folder.to_string_lossy()], ENV, env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn reads_print_disabled_as_the_host_does() {
        let at = Address { state: PathBuf::from("/Users/a/.wsp/state.json"), home: PathBuf::from("/h") };
        let label = label(&at);
        assert_eq!(label, "com.wsp.host.2f71d6c9");
        let listed = |said: &str| format!("disabled services = {{\n\t\"com.apple.x\" => enabled\n\t\"{label}\" => {said}\n}}\n");
        assert_eq!(starts(&label, 0, &listed("disabled")), Some(false));
        assert_eq!(starts(&label, 0, &listed("true")), Some(false));
        assert_eq!(starts(&label, 0, &listed("enabled")), Some(true));
        assert_eq!(starts(&label, 0, "disabled services = {\n}\n"), Some(true));
        assert_eq!(starts(&label, 5, ""), None);
    }
}
