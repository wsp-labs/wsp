// SPDX-License-Identifier: AGPL-3.0-only
//! The roads a catalog entry or a recipe's tools row takes onto a Linux machine, and the pinned installers the script
//! road carries. Port of packages/catalog/src/roads.ts at a3b870a6e; the spec is spec/roads.md. Literal constants are
//! copied; every text the TS builds is a `LazyLock` built the same way.
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::linux_casks::LinuxCask;
use wsp_wire::views::golden_image::ToolPin;

pub const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinState {
    None,
    Same,
    Moved,
}

/// roads.ts:19.
pub fn pin_state_of(version: Option<&str>, pin: Option<&ToolPin>) -> PinState {
    todo!()
}

/// The recorded pin while it stands (roads.ts:26).
pub fn standing_pin(road: &InstallRoad) -> Option<ToolPin> {
    todo!()
}

/// The road without its recorded pin (roads.ts:31).
pub fn unpinned(road: &InstallRoad) -> InstallRoad {
    todo!()
}

/// roads.ts:39.
pub fn pin_check_line(what: &str, tag: &str, sha256: &str) -> String {
    todo!()
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReleaseAssets {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x86_64: Option<ReleaseAsset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aarch64: Option<ReleaseAsset>,
}

/// A package manager's global (roads.ts:58).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageRoad {
    pub package: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pin: Option<ToolPin>,
}

/// roads.ts:65.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "road", rename_all = "lowercase")]
pub enum InstallRoad {
    Brew {
        formula: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        pin: Option<ToolPin>,
    },
    Npm {
        #[serde(flatten)]
        package: PackageRoad,
        #[serde(rename = "ignoreScripts", skip_serializing_if = "Option::is_none")]
        ignore_scripts: Option<bool>,
    },
    Pnpm(PackageRoad),
    Bun(PackageRoad),
    Uv(PackageRoad),
    Pipx(PackageRoad),
    Cargo(PackageRoad),
    Go {
        #[serde(skip_serializing_if = "Option::is_none")]
        module: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pin: Option<ToolPin>,
    },
    Release {
        #[serde(skip_serializing_if = "Option::is_none")]
        repo: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        assets: Option<ReleaseAssets>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pin: Option<ToolPin>,
        #[serde(skip_serializing_if = "Option::is_none")]
        go: Option<String>,
    },
    Vendor {
        cask: LinuxCask,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pin: Option<ToolPin>,
    },
    Apt {
        packages: Vec<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pin: Option<ToolPin>,
    },
    Script {
        script: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        version: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pin: Option<ToolPin>,
        #[serde(skip_serializing_if = "Option::is_none")]
        bins: Option<Vec<String>>,
    },
}

impl InstallRoad {
    pub fn name(&self) -> RoadName {
        todo!()
    }

    /// The road's own `version` field, where its kind has one.
    pub fn version(&self) -> Option<&str> {
        todo!()
    }

    pub fn pin(&self) -> Option<&ToolPin> {
        todo!()
    }

    /// The package a package road names (`"package" in road`).
    pub fn package(&self) -> Option<&str> {
        todo!()
    }
}

/// roads.ts:90.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RoadName {
    Brew,
    Npm,
    Pnpm,
    Bun,
    Uv,
    Pipx,
    Cargo,
    Go,
    Release,
    Vendor,
    Apt,
    Script,
}

impl RoadName {
    pub fn as_str(self) -> &'static str {
        todo!()
    }
}

/// roads.ts:91, in this order.
pub const ROADS: [RoadName; 12] = [
    RoadName::Brew,
    RoadName::Npm,
    RoadName::Pnpm,
    RoadName::Bun,
    RoadName::Uv,
    RoadName::Pipx,
    RoadName::Cargo,
    RoadName::Go,
    RoadName::Release,
    RoadName::Vendor,
    RoadName::Apt,
    RoadName::Script,
];

/// The road a word names; nothing for any other word, the empty word and none included (roads.ts:93).
pub fn is_road(s: Option<&str>) -> Option<RoadName> {
    todo!()
}

/// The row's own version, else the standing pin's tag (roads.ts:97).
pub fn version_of(road: &InstallRoad) -> Option<String> {
    todo!()
}

pub const GUEST_HOME: &str = "/root";
pub const HOME_BIN: &str = "/root/.local/bin";
pub const LOCAL_BIN: &str = wsp_wire::types::daemon_contract::TOOL_LINKS_DIR;
pub const APT_ENV: &str = "export DEBIAN_FRONTEND=noninteractive";
pub const CLAUDE_CONFIG_REL: &str = ".claude-cfg";
pub const GUEST_REWRITES: [(&str, &str); 2] = [(".claude/", ".claude-cfg/"), (".claude.json", ".claude-cfg/.claude.json")];

/// Where a home-relative path lands under prefix rewrites; a `/`-ended prefix also moves the folder named bare
/// (roads.ts:120). `GUEST_REWRITES` where none are handed.
pub fn rewritten_rel(rel: &str, rewrites: Option<&[(&str, &str)]>) -> String {
    todo!()
}

pub const CLAUDE_CONFIG_DIR: &str = "/root/.claude-cfg";
pub const CLAUDE_PLUGIN_SEED: &str = "/root/.claude-plugin-seed";
pub const CLAUDE_KEY_FILE: &str = "anthropic-api-key";

/// A pinned download per arch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ArchSums {
    pub x86_64: &'static str,
    pub aarch64: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct VersionPin {
    pub version: &'static str,
    pub sha256: ArchSums,
}

pub const CLAUDE_CODE: VersionPin = VersionPin {
    version: "2.1.296",
    sha256: ArchSums {
        x86_64: "24972e3bc859fab2b46ed4c1e51f7d6130f06d3bd550811a114640de3370d0de",
        aarch64: "f1f6e96e0d8342b9dbf41d7e88255397a6a52ce3d8736ad6a4c6b59c9b62fefa",
    },
};

pub static CLAUDE_LATEST: LazyLock<String> = LazyLock::new(|| todo!());
pub static CLAUDE_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());
pub static GOLDEN_SETUP: LazyLock<String> = LazyLock::new(|| todo!());
pub const GOLDEN_SMOKE: &str = "claude --version";

pub const UV: VersionPin = VersionPin {
    version: "0.12.9",
    sha256: ArchSums {
        x86_64: "ec7a99cd05e0cd7f80243f135ce1361c76835cb0ee60055d14d20eba8eba1460",
        aarch64: "c36fe17937ff6bd16dc42fc13854b5465999fcab2efe0af559381e945e3c6001",
    },
};
pub static UV_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());

/// One Node release (roads.ts:219).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct NodeRelease {
    pub version: &'static str,
    pub sha256: ArchSums,
    pub eol: &'static str,
}

/// roads.ts:204, keyed by major: 20 then 22.
pub const NODE_RELEASES: [(u32, NodeRelease); 2] = [
    (
        20,
        NodeRelease {
            version: "20.20.2",
            sha256: ArchSums {
                x86_64: "19e56f0825510207dd904f087fe52faa0a4eb6b2aab5f0ea7a33830d04888b8b",
                aarch64: "47ef73d543ecf6eb19435f6c03a0ac4809b3bf0dd6b26c7c571efc2a6572a74d",
            },
            eol: "2026-04-30",
        },
    ),
    (
        22,
        NodeRelease {
            version: "22.23.2",
            sha256: ArchSums {
                x86_64: "b294a556e639d64338823920e5866c21c02741742d2e1529ee1a225c1ec9252a",
                aarch64: "013b59cfd2819703a6f4a14ab891fc46fc2a4e3f5bcd92de3fb4929b43e35b30",
            },
            eol: "2027-04-30",
        },
    ),
];

/// The release of a major, where the table holds it.
pub fn node_release(major: u32) -> Option<NodeRelease> {
    todo!()
}

pub const NODE_PATH_LINE: &str = "export PATH=\"/usr/local/bin:$PATH\"";
pub const NODE_BIN: &str = "node";

/// roads.ts:236.
pub fn node_install_script(floor: u32, release: &NodeRelease) -> String {
    todo!()
}

pub const COMPOSE: VersionPin = VersionPin {
    version: "v5.5.1",
    sha256: ArchSums {
        x86_64: "db1889184726840f75c4f9c001048430d4f25b3be3cb084d3ddd762bc0aed576",
        aarch64: "732e3a84c1a0f67256ce80bc2598a24546b10ca05f9faa97efceb1171ece2ef7",
    },
};
pub const COMPOSE_PLUGIN: &str = "/usr/libexec/docker/cli-plugins/docker-compose";
pub static DOCKER_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());
pub static PYTHON_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());

pub const RUSTUP: VersionPin = VersionPin {
    version: "1.29.1",
    sha256: ArchSums {
        x86_64: "dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71",
        aarch64: "15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433",
    },
};
pub static RUSTUP_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());
pub static FD_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());
pub const YARN_VERSION: &str = "4.18.0";
pub static YARN_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GitPin {
    pub tag: &'static str,
    pub commit: &'static str,
}

pub const HERMES: GitPin = GitPin { tag: "v2026.8.31", commit: "29112bef099274229cadff79cdff7bf7b99c4b77" };
pub static HERMES_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());

pub const PLAYWRIGHT_VERSION: &str = "1.62.1";
pub static PLAYWRIGHT_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());

pub const SWIFT: VersionPin = VersionPin {
    version: "6.3.3",
    sha256: ArchSums {
        x86_64: "19e0c78cad5418ad48bfa87aa20c53ac9ac9996d1695d04dd94f7c7ea4eb133f",
        aarch64: "ecba8ef87b54a5048d466af500f3169c939a6b8a2cb7c600f76b5184457f293a",
    },
};
pub static SWIFT_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());
pub static OP_INSTALL: LazyLock<String> = LazyLock::new(|| todo!());
