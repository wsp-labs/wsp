// SPDX-License-Identifier: AGPL-3.0-only
//! Linux: the host's service is a systemd unit in the login's own systemd (`systemd` in packages/host's service.ts),
//! and a folder opens through xdg-open.

use std::path::Path;

use wsp_host_client::Env;

use super::{run, run_ok, service_tag, start, Address, Desktop};

pub struct Linux;

const SYSTEMCTL: &str = "/usr/bin/systemctl";
const XDG_OPEN: &str = "/usr/bin/xdg-open";
/// What Electron's `shell.openPath` answers on Linux for a path that is not there: xdg-open's exit 2, in its words.
const MISSING: &str = "The item does not exist";
/// What `systemctl --user` reaches the login's systemd by.
const SYSTEMD_ENV: &[&str] = &["HOME", "XDG_RUNTIME_DIR", "DBUS_SESSION_BUS_ADDRESS"];
/// What xdg-open and the file manager it picks read: the session, its desktop and the folders it keeps its own in.
const OPEN_ENV: &[&str] = &[
    "HOME",
    "PATH",
    "LANG",
    "DISPLAY",
    "WAYLAND_DISPLAY",
    "XDG_RUNTIME_DIR",
    "DBUS_SESSION_BUS_ADDRESS",
    "XDG_CURRENT_DESKTOP",
    "XDG_SESSION_DESKTOP",
    "DESKTOP_SESSION",
    "XDG_CONFIG_HOME",
    "XDG_CONFIG_DIRS",
    "XDG_DATA_HOME",
    "XDG_DATA_DIRS",
];

/// `systemdName` for the host's role.
fn unit(at: &Address) -> String {
    format!("wsp-host-{}.service", service_tag(&at.state))
}

/// `startsAtLogin`: the last line of `is-enabled`, which answers on stdout whatever its exit code.
fn starts(output: &str) -> Option<bool> {
    match output.trim().lines().last().map(str::trim) {
        Some("enabled") => Some(true),
        Some("disabled") => Some(false),
        _ => None,
    }
}

impl Desktop for Linux {
    fn starts_at_login(&self, at: &Address, env: &Env) -> Option<bool> {
        let unit = unit(at);
        // `registeredIn`: the unit file standing is what makes the service this state file's.
        if !at.home.join(".config/systemd/user").join(&unit).exists() {
            return None;
        }
        starts(&run(&[SYSTEMCTL, "--user", "is-enabled", &unit], SYSTEMD_ENV, env).ok()?.output)
    }

    fn set_login_start(&self, at: &Address, on: bool, env: &Env) -> Result<(), String> {
        run_ok(&[SYSTEMCTL, "--user", if on { "enable" } else { "disable" }, &unit(at)], SYSTEMD_ENV, env)
    }

    fn open(&self, folder: &Path, env: &Env) -> Result<(), String> {
        if !folder.exists() {
            return Err(MISSING.to_owned());
        }
        // On a desktop xdg-open does not know, it runs the file manager itself and returns only once that closes.
        start(&[XDG_OPEN, &folder.to_string_lossy()], OPEN_ENV, env)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn reads_is_enabled_as_the_host_does() {
        assert_eq!(starts("enabled\n"), Some(true));
        assert_eq!(starts("disabled\n"), Some(false));
        assert_eq!(starts("static\n"), None);
        assert_eq!(starts("Failed to connect to bus: No medium found\n"), None);
        let at = Address { state: PathBuf::from("/Users/a/.wsp/state.json"), home: PathBuf::from("/h") };
        assert_eq!(unit(&at), "wsp-host-2f71d6c9.service");
    }
}
