// SPDX-License-Identifier: AGPL-3.0-only
//! `daemon-binary.ts`: where a wsp-daemon binary sits, and which row of the engine's target table this machine takes.
/// `DAEMON_TARGETS` and `DaemonTarget` (`daemon-binary.ts:7`), re-exported from the engine.
pub use wsp_engine::daemon_targets::{DaemonTarget, DAEMON_TARGETS};

/// `GUEST_DAEMON_TARGETS` (`daemon-binary.ts:13`).
pub fn guest_daemon_targets() -> Vec<DaemonTarget> {
    todo!()
}

/// `DAEMON_BIN` (`daemon-binary.ts:16`).
pub const DAEMON_BIN: &str = "wsp-daemon";

/// `daemonTargetFor` (`daemon-binary.ts:20`).
pub fn daemon_target_for(_platform: &str, _arch: &str) -> Option<DaemonTarget> {
    todo!()
}

/// `daemonTargetHere` (`daemon-binary.ts:25`).
pub fn daemon_target_here() -> Option<DaemonTarget> {
    todo!()
}

/// `daemonBinaryIn` (`daemon-binary.ts:30`).
pub fn daemon_binary_in(_dir: &str, _triple: &str) -> String {
    todo!()
}

/// `daemonArtifactName` (`daemon-binary.ts:35`).
pub fn daemon_artifact_name(_triple: &str) -> String {
    todo!()
}

/// `noDaemonBuildLine` (`daemon-binary.ts:37`).
pub fn no_daemon_build_line(_platform: &str, _arch: &str) -> String {
    todo!()
}

/// `guestDaemonTarget` (`daemon-binary.ts:43`).
pub fn guest_daemon_target(_system: Option<&str>, _uname: &str) -> Option<DaemonTarget> {
    todo!()
}

/// `guestSystem` (`daemon-binary.ts:49`).
pub fn guest_system(_system: Option<&str>) -> bool {
    todo!()
}

/// `noPlaceSystemLine` (`daemon-binary.ts:54`), where `who` is the computer's name in the sentence and `joined` says it is already in the wsp.
pub fn no_place_system_line(_system: &str, _who: &str, _joined: bool) -> String {
    todo!()
}

/// `noGuestDaemonLine` (`daemon-binary.ts:62`).
pub fn no_guest_daemon_line(_uname: &str) -> String {
    todo!()
}
