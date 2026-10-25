// SPDX-License-Identifier: AGPL-3.0-only
//! The install roads as modules: the line that installs, the line that takes off, the names a row may carry, what it
//! runs on, how it reads to a person, and where each manager installs. Port of packages/catalog/src/road-modules.ts
//! at a3b870a6e (the release, vendor, apt and script roads' lines are in road_release); spec/road_modules.md.
use std::sync::LazyLock;

use indexmap::IndexMap;
use serde::Serialize;

use crate::roads::{InstallRoad, RoadName};

/// A recipe's tools row as a manager road reads it (road-modules.ts:19).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ToolRow {
    pub name: String,
    pub version: Option<String>,
    pub paths: Vec<String>,
    pub label: String,
}

/// Why a road installs or takes off nothing; the TS `{ note }`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoadNote(pub String);

/// Whether a copy built from this road's pin gets the version the seal read (road-modules.ts:79).
pub fn fixes_version(road: &InstallRoad) -> bool {
    todo!()
}

pub const NET_READ_S: u64 = 60;
pub const NET_RETRIES: u64 = 1;
/// road-modules.ts:99.
pub static CURL_NET: LazyLock<String> = LazyLock::new(|| todo!());
pub const CARGO_BIN: &str = "/root/.cargo/bin";
pub const APT_BIN: &str = "/usr/bin";

/// Where one manager keeps what it installs (road-modules.ts:124).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallHome {
    pub home: String,
    pub bin: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub links: Option<String>,
    pub env: IndexMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallHomeName {
    Pnpm,
    Bun,
    Uv,
    Pipx,
    Cargo,
    Go,
}

/// road-modules.ts:140, in the order a job exports their knobs.
pub const INSTALL_HOMES: [InstallHomeName; 6] = [InstallHomeName::Pnpm, InstallHomeName::Bun, InstallHomeName::Uv, InstallHomeName::Pipx, InstallHomeName::Cargo, InstallHomeName::Go];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InstallHomes {
    pub pnpm: InstallHome,
    pub bun: InstallHome,
    pub uv: InstallHome,
    pub pipx: InstallHome,
    pub cargo: InstallHome,
    pub go: InstallHome,
}

impl InstallHomes {
    pub fn get(&self, name: InstallHomeName) -> &InstallHome {
        todo!()
    }
}

/// Every manager's own home with no prefix, else every one under `prefix` with its commands in /usr/local/bin
/// (road-modules.ts:148).
pub fn install_homes(prefix: Option<&str>) -> InstallHomes {
    todo!()
}

/// The folders a row's own installer links into under this job's homes: a manager's own command folder moves with it
/// and answers as both folders (road-modules.ts:205).
pub fn home_bins(bins: &[String], homes: &InstallHomes) -> Vec<String> {
    todo!()
}

pub const APT_INDEX: &str = "apt-index";
pub const HOMEBREW_STEP: &str = "homebrew";
pub static APT_UPDATE: LazyLock<String> = LazyLock::new(|| todo!());

pub const HOMEBREW_TAG: &str = "6.0.21";
pub const HOMEBREW_COMMIT: &str = "560147012b9678b42ef5e83b690f0895552d1366";
pub const LINUXBREW_HOME: &str = wsp_wire::types::daemon_contract::HOMEBREW_HOME;
pub const BREW_PREFIX: &str = wsp_wire::types::daemon_contract::HOMEBREW_PREFIX;
pub static BREW_REPO: LazyLock<String> = LazyLock::new(|| todo!());
pub static BREW_ENV: LazyLock<String> = LazyLock::new(|| todo!());
pub static BREW: LazyLock<String> = LazyLock::new(|| todo!());
pub static BREW_REAL: LazyLock<String> = LazyLock::new(|| todo!());
pub static FROM_A_READABLE_DIR: LazyLock<String> = LazyLock::new(|| todo!());

/// road-modules.ts:266.
pub fn as_linuxbrew_script(script: &str) -> String {
    todo!()
}

/// road-modules.ts:270.
pub fn as_linuxbrew(cmd: &str) -> String {
    todo!()
}

pub static LINUXBREW_SHIM: LazyLock<String> = LazyLock::new(|| todo!());
pub static BREW_SHIM_PRESENT: LazyLock<String> = LazyLock::new(|| todo!());

/// The part after the last `/` (road-modules.ts:292).
pub fn formula_short_name(formula: &str) -> String {
    todo!()
}

/// road-modules.ts:310.
pub fn brew_has_check(formulae: &[&str]) -> String {
    todo!()
}

/// The file `go install` writes for a module (road-modules.ts:409).
pub fn go_binary(module: &str) -> String {
    todo!()
}

/// One road's module (road-modules.ts:26); every method answers for the road kind it was made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoadModule {
    pub road: RoadName,
}

/// The module that walks a road (road-modules.ts:682).
pub fn road_module(road: &InstallRoad) -> RoadModule {
    todo!()
}

/// The module of a road by name (`ROAD_MODULES[name]`).
pub fn road_module_named(name: RoadName) -> RoadModule {
    todo!()
}

impl RoadModule {
    /// "with Homebrew", "as an npm global", ...
    pub fn words(self) -> String {
        todo!()
    }

    /// Every directory this road writes an install into.
    pub fn roots(self) -> Vec<String> {
        todo!()
    }

    /// The directories its commands answer from; `homes` defaults to the own homes.
    pub fn bins(self, road: &InstallRoad, homes: Option<&InstallHomes>) -> Vec<String> {
        todo!()
    }

    /// Brew's presence test; `None` for every other road.
    pub fn present(self, road: &InstallRoad, bin: &str) -> Option<String> {
        todo!()
    }

    /// Brew's own list as the check; `None` for every other road.
    pub fn check(self, road: &InstallRoad, bin: &str) -> Option<String> {
        todo!()
    }

    /// The line a person reads while it runs, where it is not the install line: brew, release, vendor, apt.
    pub fn shown(self, road: &InstallRoad, bin: &str) -> Option<String> {
        todo!()
    }

    /// False for release and vendor, whose shown words are not a line to paste.
    pub fn pastes(self) -> bool {
        todo!()
    }

    /// `homebrew`, `node`, `apt-index`, or nothing.
    pub fn after(self) -> Option<&'static str> {
        todo!()
    }

    /// The road a manager's tools row takes; `None` for release, vendor, apt and script.
    pub fn from_row(self, row: &ToolRow) -> Option<InstallRoad> {
        todo!()
    }

    pub fn install(self, road: &InstallRoad, bin: &str, homes: Option<&InstallHomes>) -> Result<String, RoadNote> {
        todo!()
    }

    pub fn uninstall(self, road: &InstallRoad, bin: &str, homes: Option<&InstallHomes>) -> Result<String, RoadNote> {
        todo!()
    }

    /// The knobs pnpm, bun, uv, pipx, cargo and go read for these homes; `None` for the rest.
    pub fn env(self, homes: &InstallHomes) -> Option<IndexMap<String, String>> {
        todo!()
    }

    pub fn names(self, road: &InstallRoad) -> Vec<String> {
        todo!()
    }

    /// The command go and vendor roads know; `None` elsewhere.
    pub fn bin(self, road: &InstallRoad) -> Option<String> {
        todo!()
    }

    /// The road fixed to a version, for npm, pnpm, bun, uv, pipx, cargo and go; `None` for the rest.
    pub fn at(self, road: &InstallRoad, version: &str) -> Option<InstallRoad> {
        todo!()
    }

    /// The line printing the installed version; `None` for release and vendor.
    pub fn installed(self, road: &InstallRoad, bin: &str, homes: Option<&InstallHomes>) -> Option<String> {
        todo!()
    }
}

/// Every knob of every module, in ROADS order (road-modules.ts:627).
pub fn install_env(homes: &InstallHomes) -> IndexMap<String, String> {
    todo!()
}

/// The road a manager's own module reads off a row filed under it, or nothing (road-modules.ts:636).
pub fn row_road(manager: &str, row: &ToolRow) -> Option<InstallRoad> {
    todo!()
}

/// Whether a manager's module reads a row's road (road-modules.ts:644).
pub fn reads_row_road(manager: &str) -> bool {
    todo!()
}

/// What a road's step gets from its guard (road-modules.ts:647).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadStep {
    pub limit_s: u64,
    pub retry: bool,
    pub env: Vec<String>,
}

/// road-modules.ts:666.
pub fn road_step(road: RoadName) -> RoadStep {
    todo!()
}
