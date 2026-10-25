// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/daemon-targets.ts. Spec: spec/machine_reads.md.
//!
//! Which wsp-daemon binary a machine runs: one row per target wsp builds, in the words node, uname and the Rust
//! toolchain each use for it.

/// node's `process.platform` word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Platform {
    Linux,
    Darwin,
}

/// node's `process.arch` word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Arch {
    X64,
    Arm64,
}

/// daemon-targets.ts:9.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaemonTarget {
    /// The Rust target triple.
    pub triple: &'static str,
    pub platform: Platform,
    pub arch: Arch,
    /// What `uname -s` prints there.
    pub system: &'static str,
    /// What `uname -m` prints there.
    pub uname: &'static str,
    /// What a sentence calls a computer of this system.
    pub computer: &'static str,
}

/// daemon-targets.ts:23. In this order; the first row a lookup matches wins.
pub const DAEMON_TARGETS: [DaemonTarget; 4] = [
    DaemonTarget { triple: "x86_64-unknown-linux-musl", platform: Platform::Linux, arch: Arch::X64, system: "Linux", uname: "x86_64", computer: "Linux computer" },
    DaemonTarget { triple: "aarch64-unknown-linux-musl", platform: Platform::Linux, arch: Arch::Arm64, system: "Linux", uname: "aarch64", computer: "Linux computer" },
    DaemonTarget { triple: "aarch64-apple-darwin", platform: Platform::Darwin, arch: Arch::Arm64, system: "Darwin", uname: "arm64", computer: "Mac" },
    DaemonTarget { triple: "x86_64-apple-darwin", platform: Platform::Darwin, arch: Arch::X64, system: "Darwin", uname: "x86_64", computer: "Mac" },
];

/// daemon-targets.ts:31. node's word for the system a machine's `uname -s` names; exact, case-sensitive match on
/// `system`, `None` for a system no row has (`linux`, `""`, `FreeBSD`).
pub fn platform_of_system(system: &str) -> Option<Platform> {
    todo!()
}
