// SPDX-License-Identifier: AGPL-3.0-only
//! The install roads as modules: the line that installs, the line that takes off, the names a row may carry, what it
//! runs on, how it reads to a person, and where each manager installs. Port of packages/catalog/src/road-modules.ts
//! at a3b870a6e (the release, vendor, apt and script roads' lines are in road_release); spec/road_modules.md.
use std::sync::LazyLock;

use indexmap::IndexMap;
use serde::Serialize;

use crate::js_regex::compiled;
use crate::road_release::{asset_install, release_install};
use crate::roads::{
    is_road, standing_pin, version_of, InstallRoad, PackageRoad, RoadName, APT_ENV, GUEST_HOME, HOME_BIN, LOCAL_BIN, ROADS,
};
use wsp_wire::types::daemon_contract::PNPM_HOME;
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_wire::types::shell_quote::shell_quote;

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
    has_at(road.name()) || road.version().is_some()
}

fn has_at(name: RoadName) -> bool {
    matches!(name, RoadName::Npm | RoadName::Pnpm | RoadName::Bun | RoadName::Uv | RoadName::Pipx | RoadName::Cargo | RoadName::Go)
}

pub const NET_READ_S: u64 = 60;
pub const NET_RETRIES: u64 = 1;
const NET_CONNECT_S: u64 = 15;
const NET_RETRY_WAIT_S: u64 = 10;

/// road-modules.ts:99.
pub static CURL_NET: LazyLock<String> = LazyLock::new(|| {
    format!("curl() {{ command curl --connect-timeout {NET_CONNECT_S} --speed-limit 1 --speed-time {NET_READ_S} --retry {NET_RETRIES} --fail --silent --show-error --location \"$@\"; }}")
});
static NPM_NET: LazyLock<String> = LazyLock::new(|| {
    format!(
        "export npm_config_fetch_timeout={} npm_config_fetch_retries={NET_RETRIES} npm_config_fetch_retry_maxtimeout={}",
        NET_READ_S * 1000,
        NET_RETRY_WAIT_S * 1000
    )
});
static PIP_NET: LazyLock<String> = LazyLock::new(|| format!("export PIP_TIMEOUT={NET_READ_S} PIP_RETRIES={NET_RETRIES}"));
static UV_NET: LazyLock<String> = LazyLock::new(|| format!("export UV_HTTP_TIMEOUT={NET_READ_S} UV_HTTP_RETRIES={NET_RETRIES}"));
static CARGO_NET: LazyLock<String> = LazyLock::new(|| format!("export CARGO_HTTP_TIMEOUT={NET_READ_S} CARGO_NET_RETRY={NET_RETRIES}"));

pub const CARGO_BIN: &str = "/root/.cargo/bin";
pub const APT_BIN: &str = "/usr/bin";
const CARGO_HOME: &str = "/root/.cargo";
const GO_BIN: &str = "/root/go/bin";

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
pub const INSTALL_HOMES: [InstallHomeName; 6] =
    [InstallHomeName::Pnpm, InstallHomeName::Bun, InstallHomeName::Uv, InstallHomeName::Pipx, InstallHomeName::Cargo, InstallHomeName::Go];

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
        match name {
            InstallHomeName::Pnpm => &self.pnpm,
            InstallHomeName::Bun => &self.bun,
            InstallHomeName::Uv => &self.uv,
            InstallHomeName::Pipx => &self.pipx,
            InstallHomeName::Cargo => &self.cargo,
            InstallHomeName::Go => &self.go,
        }
    }
}

fn env(pairs: Vec<(&str, String)>) -> IndexMap<String, String> {
    pairs.into_iter().map(|(k, v)| (k.to_string(), v)).collect()
}

/// Every manager's own home with no prefix, else every one under `prefix` with its commands in /usr/local/bin
/// (road-modules.ts:148).
pub fn install_homes(prefix: Option<&str>) -> InstallHomes {
    let Some(prefix) = prefix else {
        let own = |home: String, bin: &str| InstallHome { home, bin: bin.to_string(), links: None, env: IndexMap::new() };
        return InstallHomes {
            // pnpm alone has no folder of its own by default, so its knob rides every job, image included.
            pnpm: InstallHome {
                home: PNPM_HOME.into(),
                bin: PNPM_HOME.into(),
                links: None,
                env: env(vec![("PNPM_HOME", PNPM_HOME.into())]),
            },
            bun: own(format!("{GUEST_HOME}/.bun"), &format!("{GUEST_HOME}/.bun/bin")),
            uv: own(format!("{GUEST_HOME}/.local"), HOME_BIN),
            pipx: own(format!("{GUEST_HOME}/.local"), HOME_BIN),
            cargo: own(CARGO_HOME.into(), CARGO_BIN),
            go: own(format!("{GUEST_HOME}/go"), GO_BIN),
        };
    };
    let at = |name: &str| format!("{prefix}/{name}");
    InstallHomes {
        // pnpm links what it installs globally into its home's own bin folder and takes no knob for another, so that
        // folder is linked onto the fixed PATH; rustup's toolchain rides cargo's home and cargo install takes none either.
        pnpm: InstallHome {
            home: at("pnpm"),
            bin: LOCAL_BIN.into(),
            links: Some(format!("{}/bin", at("pnpm"))),
            env: env(vec![("PNPM_HOME", at("pnpm"))]),
        },
        bun: InstallHome {
            home: at("bun"),
            bin: LOCAL_BIN.into(),
            links: None,
            env: env(vec![("BUN_INSTALL", at("bun")), ("BUN_INSTALL_BIN", LOCAL_BIN.into())]),
        },
        uv: InstallHome {
            home: at("uv"),
            bin: LOCAL_BIN.into(),
            links: None,
            env: env(vec![
                ("UV_TOOL_DIR", format!("{}/tools", at("uv"))),
                ("UV_TOOL_BIN_DIR", LOCAL_BIN.into()),
                ("UV_PYTHON_INSTALL_DIR", format!("{}/python", at("uv"))),
            ]),
        },
        pipx: InstallHome {
            home: at("pipx"),
            bin: LOCAL_BIN.into(),
            links: None,
            env: env(vec![("PIPX_HOME", at("pipx")), ("PIPX_BIN_DIR", LOCAL_BIN.into())]),
        },
        cargo: InstallHome {
            home: at("cargo"),
            bin: LOCAL_BIN.into(),
            links: Some(format!("{}/bin", at("cargo"))),
            env: env(vec![("CARGO_HOME", at("cargo")), ("RUSTUP_HOME", at("rustup"))]),
        },
        go: InstallHome {
            home: at("go"),
            bin: LOCAL_BIN.into(),
            links: None,
            env: env(vec![("GOPATH", at("go")), ("GOBIN", LOCAL_BIN.into())]),
        },
    }
}

/// The homes a job with no prefix runs on, which is every manager's own (road-modules.ts:175).
static OWN_HOMES: LazyLock<InstallHomes> = LazyLock::new(|| install_homes(None));

/// The homes a job on a computer somebody owns runs on, since one prefix is wsp's (road-modules.ts:179).
static PREFIX_HOMES: LazyLock<InstallHomes> = LazyLock::new(|| install_homes(Some(TOOL_PREFIX)));

/// A manager's own command folder and the folder every command in it is linked into (road-modules.ts:183).
#[derive(Debug, Clone, PartialEq, Eq)]
struct LinkedCommands {
    links: String,
    bin: String,
}

fn linked_of(home: &InstallHome) -> Option<LinkedCommands> {
    home.links.as_ref().map(|links| LinkedCommands { links: links.clone(), bin: home.bin.clone() })
}

/// The pair for the manager that keeps its commands in this folder under the own homes (road-modules.ts:196).
fn moved_home(dir: &str, homes: &InstallHomes) -> Option<LinkedCommands> {
    INSTALL_HOMES
        .into_iter()
        .find(|&name| homes.get(name).links.is_some() && OWN_HOMES.get(name).bin == dir)
        .and_then(|name| linked_of(homes.get(name)))
}

/// The folders a row's own installer links into, under this job's homes (road-modules.ts:205).
pub fn home_bins(bins: &[String], homes: &InstallHomes) -> Vec<String> {
    bins.iter()
        .flat_map(|dir| match moved_home(dir, homes) {
            None => vec![dir.clone()],
            Some(moved) => vec![moved.links, moved.bin],
        })
        .collect()
}

/// Every command in that folder, linked into the folder the daemon's fixed PATH holds (road-modules.ts:214).
fn link_commands(at: Option<LinkedCommands>) -> Vec<String> {
    at.map(|at| vec![format!("find {} -maxdepth 1 -type f -perm -u+x -exec ln -sfn {{}} {}/ ';'", at.links, at.bin)]).unwrap_or_default()
}

/// The link one command answers by, taken off again where the install put one there (road-modules.ts:217).
fn unlink_command(home: &InstallHome, bin: &str) -> Vec<String> {
    if home.links.is_none() {
        Vec::new()
    } else {
        vec![format!("rm -f {}/{}", home.bin, shell_quote(bin))]
    }
}

/// What a step reads a row of this manager back by (road-modules.ts:221).
fn bins_of(home: &InstallHome) -> Vec<String> {
    match &home.links {
        None => vec![home.bin.clone()],
        Some(links) => vec![links.clone(), home.bin.clone()],
    }
}

/// One line of a manager's own, run with its command folder ahead of the job's PATH (road-modules.ts:225).
fn in_home(home: &InstallHome, cmd: &str) -> String {
    match &home.links {
        None => cmd.to_string(),
        Some(links) => format!("export PATH={links}:$PATH; {cmd}"),
    }
}

/// Lines joined by a newline, the first one given and the rest after it.
fn joined(first: String, rest: Vec<String>) -> String {
    std::iter::once(first).chain(rest).collect::<Vec<_>>().join("\n")
}

/// The pseudo step every apt row waits on (road-modules.ts:228).
pub const APT_INDEX: &str = "apt-index";
/// The pseudo step every formula waits on (road-modules.ts:230).
pub const HOMEBREW_STEP: &str = "homebrew";
/// The index read, as the stage runs it before the first apt row (road-modules.ts:232).
pub static APT_UPDATE: LazyLock<String> = LazyLock::new(|| format!("{APT_ENV}\napt-get update -qq"));

pub const HOMEBREW_TAG: &str = "6.0.21";
pub const HOMEBREW_COMMIT: &str = "560147012b9678b42ef5e83b690f0895552d1366";
pub const LINUXBREW_HOME: &str = wsp_wire::types::daemon_contract::HOMEBREW_HOME;
pub const BREW_PREFIX: &str = wsp_wire::types::daemon_contract::HOMEBREW_PREFIX;
pub static BREW_REPO: LazyLock<String> = LazyLock::new(|| format!("{BREW_PREFIX}/Homebrew"));
pub static BREW_ENV: LazyLock<String> = LazyLock::new(|| {
    format!(
        "HOMEBREW_NO_AUTO_UPDATE=1 HOMEBREW_NO_ANALYTICS=1 HOMEBREW_NO_ENV_HINTS=1 NONINTERACTIVE=1 HOMEBREW_CURL_RETRIES={NET_RETRIES}"
    )
});
pub static BREW: LazyLock<String> = LazyLock::new(|| format!("{BREW_PREFIX}/bin/brew"));
pub static BREW_REAL: LazyLock<String> = LazyLock::new(|| format!("{BREW_PREFIX}/libexec/brew"));

const CALLER_DIR_READABLE: &str = r#"[ -d "$PWD" ] && [ -r "$PWD" ]"#;
pub static FROM_A_READABLE_DIR: LazyLock<String> = LazyLock::new(|| format!("{CALLER_DIR_READABLE} || cd {LINUXBREW_HOME}"));

/// road-modules.ts:266.
pub fn as_linuxbrew_script(script: &str) -> String {
    format!("su -s /bin/bash linuxbrew -c {}", shell_quote(&format!("{}\nexport {}\n{script}", *FROM_A_READABLE_DIR, *BREW_ENV)))
}

/// road-modules.ts:270.
pub fn as_linuxbrew(cmd: &str) -> String {
    format!("su -s /bin/bash linuxbrew -c {}", shell_quote(&format!("{}\n{} {} {cmd}", *FROM_A_READABLE_DIR, *BREW_ENV, *BREW)))
}

pub static LINUXBREW_SHIM: LazyLock<String> = LazyLock::new(|| {
    let inner = format!(
        "{CALLER_DIR_READABLE} || {{ cd {LINUXBREW_HOME}; [ -e /dev/fd/9 ] && export HOMEBREW_BUNDLE_FILE=\"${{HOMEBREW_BUNDLE_FILE:-/dev/fd/9}}\"; }}\nexec \"$0\" \"$@\""
    );
    [
        "#!/bin/sh".to_string(),
        format!(r#"if [ "$(id -un)" = linuxbrew ]; then exec {} "$@"; fi"#, *BREW_REAL),
        "if [ -f Brewfile ]; then exec 9<Brewfile; else exec 9<&-; fi".to_string(),
        format!("exec su -s /bin/bash linuxbrew -c {} -- {} \"$@\"", shell_quote(&inner), *BREW_REAL),
    ]
    .join("\n")
});
pub static BREW_SHIM_PRESENT: LazyLock<String> = LazyLock::new(|| format!(r#"[ "$(cat {})" = {} ]"#, *BREW, shell_quote(&LINUXBREW_SHIM)));

/// The part after the last `/` (road-modules.ts:292).
pub fn formula_short_name(formula: &str) -> String {
    formula[formula.rfind('/').map_or(0, |i| i + 1)..].to_string()
}

/// Whether a formula is on a machine, as one shell test a workspace can answer (road-modules.ts:301).
fn formula_present(formula: &str) -> String {
    let short = formula_short_name(formula);
    let linked = format!("test -e {BREW_PREFIX}/opt/{short}");
    if formula.contains('/') {
        format!("{linked} || command -v {} >/dev/null 2>&1", shell_quote(&short))
    } else {
        linked
    }
}

/// road-modules.ts:310.
pub fn brew_has_check(formulae: &[&str]) -> String {
    as_linuxbrew(&format!("list --versions {}", formulae.join(" ")))
}

/// The file `go install` writes for a module (road-modules.ts:409).
pub fn go_binary(module: &str) -> String {
    let base = match module.rfind('@') {
        Some(at) if at > 0 => &module[..at],
        _ => module,
    };
    let parts: Vec<&str> = base.split('/').collect();
    let last = parts[parts.len() - 1];
    if parts.len() > 1 && is_major_suffix(last) {
        parts[parts.len() - 2].to_string()
    } else {
        last.to_string()
    }
}

/// `^v[1-9]\d*$`, the major-version suffix a Go module path ends in.
fn is_major_suffix(s: &str) -> bool {
    matches!(s.strip_prefix('v'), Some(rest) if rest.as_bytes().first().is_some_and(|b| (b'1'..=b'9').contains(b)) && rest.bytes().all(|b| b.is_ascii_digit()))
}

/// The module and version a go row names: its first path's `path@version`, else the label's `(...)` (road-modules.ts:401).
fn go_module(row: &ToolRow) -> Option<(String, String)> {
    let spec = match row.paths.first() {
        Some(path) => path.clone(),
        None => compiled(r"\(([^()\s]+)\)", "").re.captures(&row.label).ok()??.get(1)?.as_str().to_string(),
    };
    let at = spec.rfind('@')?;
    if at == 0 || at == spec.len() - 1 {
        return None;
    }
    Some((spec[..at].to_string(), spec[at + 1..].to_string()))
}

/// One road's module (road-modules.ts:26); every method answers for the road kind it was made for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoadModule {
    pub road: RoadName,
}

/// The module that walks a road (road-modules.ts:682).
pub fn road_module(road: &InstallRoad) -> RoadModule {
    RoadModule { road: road.name() }
}

/// The module of a road by name (`ROAD_MODULES[name]`).
pub fn road_module_named(name: RoadName) -> RoadModule {
    RoadModule { road: name }
}

const NO_RELEASE: &str = "no GitHub release to install from";
/// A release road with no tag installs nothing: the current release is a moving artifact and no road fetches one.
const NO_TAG: &str = "names no release tag; name the version";

/// The homes a manager's own road reads, by road name (road-modules.ts:209-216).
fn manager(name: RoadName) -> Option<InstallHomeName> {
    match name {
        RoadName::Pnpm => Some(InstallHomeName::Pnpm),
        RoadName::Bun => Some(InstallHomeName::Bun),
        RoadName::Uv => Some(InstallHomeName::Uv),
        RoadName::Pipx => Some(InstallHomeName::Pipx),
        RoadName::Cargo => Some(InstallHomeName::Cargo),
        RoadName::Go => Some(InstallHomeName::Go),
        _ => None,
    }
}

fn package_road(road: &InstallRoad) -> Option<&PackageRoad> {
    match road {
        InstallRoad::Npm { package, .. }
        | InstallRoad::Pnpm(package)
        | InstallRoad::Bun(package)
        | InstallRoad::Uv(package)
        | InstallRoad::Pipx(package)
        | InstallRoad::Cargo(package) => Some(package),
        _ => None,
    }
}

fn pinned(pkg: &str, version: Option<&str>, sep: &str) -> String {
    match version {
        None => pkg.to_string(),
        Some(v) => format!("{pkg}{sep}{v}"),
    }
}

/// The version field of a Node global's package.json under the manager's global root (road-modules.ts:104).
fn node_global_version(root_cmd: &str, pkg: &str) -> String {
    format!(r#"node -p 'require(process.argv[1] + "/package.json").version' "$({root_cmd})/"{}"#, shell_quote(pkg))
}

/// The second column of the line a listing prints for the package, with the leading v and a trailing colon off (road-modules.ts:106).
fn listed_version(list: &str, pkg: &str) -> String {
    format!(r#"{list} 2>/dev/null | awk -v p={} '$1==p{{sub(/^v/,"",$2); sub(/:$/,"",$2); print $2}}'"#, shell_quote(pkg))
}

fn node_global(road: &InstallRoad, p: &PackageRoad, home: &InstallHome, version: Option<&str>) -> String {
    joined(in_home(home, &format!("{} add -g {}", road.name().as_str(), pinned(&p.package, version, "@"))), link_commands(linked_of(home)))
}

impl RoadModule {
    /// "with Homebrew", "as an npm global", ...
    pub fn words(self) -> String {
        let words = match self.road {
            RoadName::Brew => "with Homebrew",
            RoadName::Npm => "as an npm global",
            RoadName::Pnpm => "with pnpm",
            RoadName::Bun => "with bun",
            RoadName::Uv => "with uv",
            RoadName::Pipx => "with pipx",
            RoadName::Cargo => "with cargo",
            RoadName::Go => "with go install",
            RoadName::Release => "from its release",
            RoadName::Vendor => "from its vendor's release",
            RoadName::Apt => "by apt",
            RoadName::Script => "by its own installer",
        };
        words.to_string()
    }

    /// Every directory this road writes an install into.
    pub fn roots(self) -> Vec<String> {
        let own_and_prefix = |name: InstallHomeName| vec![OWN_HOMES.get(name).home.clone(), PREFIX_HOMES.get(name).home.clone()];
        match self.road {
            RoadName::Brew => vec![BREW_PREFIX.into(), "/usr/local/bin".into()],
            RoadName::Npm => vec!["/usr/local/lib/node_modules".into(), "/usr/local/bin".into()],
            RoadName::Pnpm => own_and_prefix(InstallHomeName::Pnpm),
            RoadName::Bun => own_and_prefix(InstallHomeName::Bun),
            RoadName::Uv => own_and_prefix(InstallHomeName::Uv),
            RoadName::Pipx => own_and_prefix(InstallHomeName::Pipx),
            RoadName::Cargo => own_and_prefix(InstallHomeName::Cargo),
            RoadName::Go => own_and_prefix(InstallHomeName::Go),
            RoadName::Release => vec!["/usr/local/bin".into()],
            RoadName::Vendor => vec!["/opt".into(), "/usr/local/bin".into()],
            RoadName::Apt => vec!["/usr".into(), "/etc".into(), "/var".into()],
            RoadName::Script => vec!["/usr".into(), "/opt".into(), "/root".into()],
        }
    }

    /// The directories its commands answer from; `homes` defaults to the own homes.
    pub fn bins(self, road: &InstallRoad, homes: Option<&InstallHomes>) -> Vec<String> {
        let homes = homes.unwrap_or(&OWN_HOMES);
        match road {
            InstallRoad::Brew { .. } => vec![format!("{BREW_PREFIX}/bin"), format!("{BREW_PREFIX}/sbin"), LOCAL_BIN.into()],
            InstallRoad::Npm { .. } | InstallRoad::Release { .. } | InstallRoad::Vendor { .. } => vec![LOCAL_BIN.into()],
            InstallRoad::Apt { .. } => vec![APT_BIN.into(), "/usr/sbin".into(), "/bin".into(), "/sbin".into()],
            InstallRoad::Script { bins, .. } => home_bins(bins.as_deref().unwrap_or(&[]), homes),
            _ => manager(road.name()).map_or_else(Vec::new, |m| bins_of(homes.get(m))),
        }
    }

    /// Brew's presence test; `None` for every other road.
    pub fn present(self, road: &InstallRoad, _bin: &str) -> Option<String> {
        match road {
            InstallRoad::Brew { formula, .. } => Some(formula_present(formula)),
            _ => None,
        }
    }

    /// Brew's own list as the check; `None` for every other road.
    pub fn check(self, road: &InstallRoad, _bin: &str) -> Option<String> {
        match road {
            InstallRoad::Brew { formula, .. } => Some(brew_has_check(&[formula.as_str()])),
            _ => None,
        }
    }

    /// The line a person reads while it runs, where it is not the install line: brew, release, vendor, apt.
    pub fn shown(self, road: &InstallRoad, _bin: &str) -> Option<String> {
        match road {
            InstallRoad::Brew { formula, .. } => Some(format!("brew install {formula}")),
            InstallRoad::Release { repo, .. } => Some(match (repo, version_of(road)) {
                (None, _) => NO_RELEASE.to_string(),
                (Some(_), None) => NO_TAG.to_string(),
                (Some(repo), Some(tag)) => format!("the {tag} release of github.com/{repo}"),
            }),
            InstallRoad::Vendor { cask, .. } => Some(cask.from.clone()),
            InstallRoad::Apt { packages, .. } => Some(format!("apt-get install {}", packages.join(" "))),
            _ => None,
        }
    }

    /// False for release and vendor, whose shown words are not a line to paste.
    pub fn pastes(self) -> bool {
        !matches!(self.road, RoadName::Release | RoadName::Vendor)
    }

    /// `homebrew`, `node`, `apt-index`, or nothing.
    pub fn after(self) -> Option<&'static str> {
        match self.road {
            RoadName::Brew => Some(HOMEBREW_STEP),
            RoadName::Npm => Some("node"),
            RoadName::Apt => Some(APT_INDEX),
            _ => None,
        }
    }

    /// The road a manager's tools row takes; `None` for release, vendor, apt and script.
    pub fn from_row(self, row: &ToolRow) -> Option<InstallRoad> {
        let package = || PackageRoad { package: row.name.clone(), version: row.version.clone(), pin: None };
        Some(match self.road {
            RoadName::Brew => InstallRoad::Brew { formula: row.name.clone(), pin: None },
            RoadName::Npm => InstallRoad::Npm { package: package(), ignore_scripts: None },
            RoadName::Pnpm => InstallRoad::Pnpm(package()),
            RoadName::Bun => InstallRoad::Bun(package()),
            RoadName::Uv => InstallRoad::Uv(package()),
            RoadName::Pipx => InstallRoad::Pipx(package()),
            RoadName::Cargo => InstallRoad::Cargo(package()),
            RoadName::Go => match go_module(row) {
                None => InstallRoad::Go { module: None, version: None, pin: None },
                Some((module, version)) => {
                    InstallRoad::Go { module: Some(module), version: Some(row.version.clone().unwrap_or(version)), pin: None }
                }
            },
            _ => return None,
        })
    }

    pub fn install(self, road: &InstallRoad, bin: &str, homes: Option<&InstallHomes>) -> Result<String, RoadNote> {
        let homes = homes.unwrap_or(&OWN_HOMES);
        let version = version_of(road);
        let line = match road {
            InstallRoad::Brew { formula, .. } => as_linuxbrew(&format!("install {formula}")),
            InstallRoad::Npm { package, ignore_scripts } => {
                let flag = if *ignore_scripts == Some(true) { "--ignore-scripts " } else { "" };
                format!("npm install -g {flag}{}", pinned(&package.package, version.as_deref(), "@"))
            }
            InstallRoad::Pnpm(p) => node_global(road, p, &homes.pnpm, version.as_deref()),
            InstallRoad::Bun(p) => node_global(road, p, &homes.bun, version.as_deref()),
            InstallRoad::Uv(p) => format!("uv tool install {}", pinned(&p.package, version.as_deref(), "==")),
            InstallRoad::Pipx(p) => format!("pipx install {}", pinned(&p.package, version.as_deref(), "==")),
            InstallRoad::Cargo(p) => {
                let at = version.as_deref().map(|v| format!(" --version {v}")).unwrap_or_default();
                joined(in_home(&homes.cargo, &format!("cargo install {}{at} --locked", p.package)), link_commands(linked_of(&homes.cargo)))
            }
            InstallRoad::Go { module: None, .. } => return Err(RoadNote("no module to install from".into())),
            InstallRoad::Go { module: Some(module), .. } => format!("go install {}", pinned(module, version.as_deref(), "@")),
            InstallRoad::Release { repo, assets, go, .. } => {
                let Some(repo) = repo else { return Err(RoadNote(NO_RELEASE.into())) };
                let Some(tag) = version.as_deref() else { return Err(RoadNote(NO_TAG.into())) };
                match assets {
                    None => release_install(bin, repo, tag, standing_pin(road).as_ref(), go.as_deref()),
                    Some(assets) => asset_install(bin, repo, tag, assets, go.as_deref()),
                }
            }
            InstallRoad::Vendor { cask, .. } => cask.install.clone(),
            InstallRoad::Apt { packages, .. } => format!("{APT_ENV}\napt-get install -y -qq {}", packages.join(" ")),
            InstallRoad::Script { script, bins, .. } => {
                let mut lines = vec![script.clone()];
                for dir in bins.iter().flatten() {
                    lines.extend(link_commands(moved_home(dir, homes)));
                }
                lines.join("\n")
            }
        };
        Ok(line)
    }

    pub fn uninstall(self, road: &InstallRoad, bin: &str, homes: Option<&InstallHomes>) -> Result<String, RoadNote> {
        let homes = homes.unwrap_or(&OWN_HOMES);
        let line = match road {
            InstallRoad::Brew { formula, .. } => {
                if !formula.contains('/') {
                    as_linuxbrew(&format!("uninstall {formula}"))
                } else {
                    // A tap formula with no Linux bottle took the road to /usr/local/bin under the formula's name.
                    format!(
                        "if [ -x {} ] && {} >/dev/null 2>&1; then {}; else rm -f /usr/local/bin/{}; fi",
                        *BREW,
                        as_linuxbrew(&format!("list --formula {formula}")),
                        as_linuxbrew(&format!("uninstall {formula}")),
                        shell_quote(&formula_short_name(formula))
                    )
                }
            }
            InstallRoad::Npm { package, .. } => format!("npm uninstall -g {}", package.package),
            InstallRoad::Pnpm(p) => {
                joined(in_home(&homes.pnpm, &format!("pnpm remove -g {}", p.package)), unlink_command(&homes.pnpm, bin))
            }
            InstallRoad::Bun(p) => joined(in_home(&homes.bun, &format!("bun remove -g {}", p.package)), unlink_command(&homes.bun, bin)),
            InstallRoad::Uv(p) => format!("uv tool uninstall {}", p.package),
            InstallRoad::Pipx(p) => format!("pipx uninstall {}", p.package),
            InstallRoad::Cargo(p) => {
                joined(in_home(&homes.cargo, &format!("cargo uninstall {}", p.package)), unlink_command(&homes.cargo, bin))
            }
            InstallRoad::Go { .. } => return Err(RoadNote(format!("go has no uninstall; the binary stays in {}", homes.go.bin))),
            InstallRoad::Release { .. } => format!("rm -f /usr/local/bin/{}", shell_quote(bin)),
            InstallRoad::Vendor { cask, .. } => cask.uninstall.clone(),
            InstallRoad::Apt { packages, .. } => {
                format!("{APT_ENV}\napt-get purge -y -qq {} && apt-get autoremove -y -qq --purge", packages.join(" "))
            }
            InstallRoad::Script { .. } => return Err(RoadNote(format!("{bin} has no uninstaller; left on the machine"))),
        };
        Ok(line)
    }

    /// The knobs pnpm, bun, uv, pipx, cargo and go read for these homes; `None` for the rest.
    pub fn env(self, homes: &InstallHomes) -> Option<IndexMap<String, String>> {
        manager(self.road).map(|m| homes.get(m).env.clone())
    }

    pub fn names(self, road: &InstallRoad) -> Vec<String> {
        match road {
            InstallRoad::Brew { formula, .. } => vec![formula.clone()],
            InstallRoad::Apt { packages, .. } => packages.clone(),
            _ => package_road(road).map_or_else(Vec::new, |p| vec![p.package.clone()]),
        }
    }

    /// The command go and vendor roads know; `None` elsewhere.
    pub fn bin(self, road: &InstallRoad) -> Option<String> {
        match road {
            InstallRoad::Go { module, .. } => module.as_deref().map(go_binary),
            InstallRoad::Vendor { cask, .. } => Some(cask.bin.clone()),
            _ => None,
        }
    }

    /// The road fixed to a version, for npm, pnpm, bun, uv, pipx, cargo and go; `None` for the rest.
    pub fn at(self, road: &InstallRoad, version: &str) -> Option<InstallRoad> {
        let mut out = road.clone();
        match &mut out {
            InstallRoad::Npm { package, .. }
            | InstallRoad::Pnpm(package)
            | InstallRoad::Bun(package)
            | InstallRoad::Uv(package)
            | InstallRoad::Pipx(package)
            | InstallRoad::Cargo(package) => package.version = Some(version.to_string()),
            InstallRoad::Go { version: v, .. } => *v = Some(version.to_string()),
            _ => return None,
        }
        Some(out)
    }

    /// The line printing the installed version; `None` for release and vendor.
    pub fn installed(self, road: &InstallRoad, bin: &str, homes: Option<&InstallHomes>) -> Option<String> {
        let homes = homes.unwrap_or(&OWN_HOMES);
        Some(match road {
            InstallRoad::Brew { formula, .. } => {
                format!("{} 2>/dev/null | awk '{{print $2}}'", as_linuxbrew(&format!("list --versions {formula}")))
            }
            InstallRoad::Npm { package, .. } => node_global_version("npm root -g", &package.package),
            InstallRoad::Pnpm(p) => in_home(&homes.pnpm, &node_global_version("pnpm root -g", &p.package)),
            InstallRoad::Bun(p) => in_home(
                &homes.bun,
                &format!(r#"bun pm ls -g 2>/dev/null | grep -oE "(^| ){}@[^[:space:]]+" | head -n 1 | sed 's/.*@//'"#, p.package),
            ),
            InstallRoad::Uv(p) => listed_version("uv tool list", &p.package),
            InstallRoad::Pipx(p) => listed_version("pipx list --short", &p.package),
            InstallRoad::Cargo(p) => in_home(&homes.cargo, &listed_version("cargo install --list", &p.package)),
            InstallRoad::Go { .. } => {
                format!(r#"go version -m {}/{} 2>/dev/null | awk '$1=="mod"{{print $3}}'"#, homes.go.bin, shell_quote(bin))
            }
            InstallRoad::Apt { packages, .. } => {
                format!(r#"dpkg-query -W -f='${{Version}}\n' {} 2>/dev/null"#, shell_quote(packages.first().map_or("", String::as_str)))
            }
            InstallRoad::Script { .. } => {
                format!(r#"{} --version 2>/dev/null | head -n 1 | grep -oE '[0-9][^ ,()]*\.[0-9][^ ,()]*' | head -n 1"#, shell_quote(bin))
            }
            InstallRoad::Release { .. } | InstallRoad::Vendor { .. } => return None,
        })
    }
}

/// Every knob of every module, in ROADS order (road-modules.ts:627).
pub fn install_env(homes: &InstallHomes) -> IndexMap<String, String> {
    let mut out = IndexMap::new();
    for road in ROADS {
        if let Some(knobs) = road_module_named(road).env(homes) {
            out.extend(knobs);
        }
    }
    out
}

/// The road a manager's own module reads off a row filed under it, or nothing (road-modules.ts:636).
pub fn row_road(manager: &str, row: &ToolRow) -> Option<InstallRoad> {
    road_module_named(is_road(Some(manager))?).from_row(row)
}

/// Whether a manager's module reads a row's road (road-modules.ts:644).
pub fn reads_row_road(manager: &str) -> bool {
    is_road(Some(manager)).is_some_and(|name| {
        matches!(
            name,
            RoadName::Brew
                | RoadName::Npm
                | RoadName::Pnpm
                | RoadName::Bun
                | RoadName::Uv
                | RoadName::Pipx
                | RoadName::Cargo
                | RoadName::Go
        )
    })
}

/// What a road's step gets from the guard that runs it (road-modules.ts:647).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoadStep {
    pub limit_s: u64,
    pub retry: bool,
    pub env: Vec<String>,
}

/// A script road's step is a whole script whose last line may be a cleanup, so the road runs it under set -e
/// (road-modules.ts:658).
const SHELL_STRICT: &str = "set -euo pipefail";

/// A package manager or a download finishes in a couple of minutes or is stuck; Homebrew, go, apt and a script may
/// build or configure for longer; cargo compiles every crate from source (road-modules.ts:660).
const DOWNLOAD_S: u64 = 300;
const MIXED_S: u64 = 600;
const COMPILE_S: u64 = 1200;

/// road-modules.ts:666.
pub fn road_step(road: RoadName) -> RoadStep {
    let (limit_s, retry, env): (u64, bool, Vec<String>) = match road {
        RoadName::Brew => (MIXED_S, false, vec![]),
        RoadName::Npm => (DOWNLOAD_S, true, vec![NPM_NET.clone()]),
        RoadName::Pnpm => (DOWNLOAD_S, true, vec![NPM_NET.clone()]),
        RoadName::Bun => (DOWNLOAD_S, true, vec![]),
        RoadName::Uv => (DOWNLOAD_S, true, vec![UV_NET.clone()]),
        RoadName::Pipx => (DOWNLOAD_S, true, vec![PIP_NET.clone()]),
        RoadName::Cargo => (COMPILE_S, false, vec![CARGO_NET.clone()]),
        RoadName::Go => (MIXED_S, false, vec![]),
        RoadName::Release => (DOWNLOAD_S, true, vec![CURL_NET.clone()]),
        RoadName::Vendor => (DOWNLOAD_S, true, vec![CURL_NET.clone()]),
        RoadName::Apt => (MIXED_S, false, vec![]),
        RoadName::Script => (
            MIXED_S,
            false,
            vec![SHELL_STRICT.into(), NPM_NET.clone(), PIP_NET.clone(), UV_NET.clone(), CARGO_NET.clone(), CURL_NET.clone()],
        ),
    };
    RoadStep { limit_s, retry, env }
}
