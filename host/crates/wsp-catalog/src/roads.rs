// SPDX-License-Identifier: AGPL-3.0-only
//! The roads a catalog entry or a recipe's tools row takes onto a Linux machine, and the pinned installers the script
//! road carries. Port of packages/catalog/src/roads.ts at a3b870a6e; the spec is spec/roads.md. Literal constants are
//! copied; every text the TS builds is a `LazyLock` built the same way.
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use crate::linux_casks::LinuxCask;
use wsp_words::words::image::pin_mismatch_line;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_words::words::image::short_sum;
use wsp_wire::views::golden_image::ToolPin;
use wsp_words::words::image::SUM_SHOWN;

pub const MIB: u64 = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PinState {
    None,
    Same,
    Moved,
}

/// roads.ts:19.
pub fn pin_state_of(version: Option<&str>, pin: Option<&ToolPin>) -> PinState {
    match pin {
        None => PinState::None,
        Some(p) if version.is_none_or(|v| v == p.tag) => PinState::Same,
        Some(_) => PinState::Moved,
    }
}

/// The recorded pin while it stands (roads.ts:26).
pub fn standing_pin(road: &InstallRoad) -> Option<ToolPin> {
    if pin_state_of(road.version(), road.pin()) == PinState::Same {
        road.pin().cloned()
    } else {
        None
    }
}

/// The road without its recorded pin (roads.ts:31).
pub fn unpinned(road: &InstallRoad) -> InstallRoad {
    let mut out = road.clone();
    match &mut out {
        InstallRoad::Brew { pin, .. }
        | InstallRoad::Go { pin, .. }
        | InstallRoad::Release { pin, .. }
        | InstallRoad::Vendor { pin, .. }
        | InstallRoad::Apt { pin, .. }
        | InstallRoad::Script { pin, .. } => *pin = None,
        InstallRoad::Npm { package, .. }
        | InstallRoad::Pnpm(package)
        | InstallRoad::Bun(package)
        | InstallRoad::Uv(package)
        | InstallRoad::Pipx(package)
        | InstallRoad::Cargo(package) => package.pin = None,
    }
    out
}

/// roads.ts:39.
pub fn pin_check_line(what: &str, tag: &str, sha256: &str) -> String {
    format!(
        "[ \"$sum\" = {} ] || {{ echo \"Error: {}\" >&2; exit 1; }}",
        shell_quote(sha256),
        pin_mismatch_line(what, tag, &short_sum(sha256), &format!("${{sum:0:{SUM_SHOWN}}}"))
    )
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
        match self {
            InstallRoad::Brew { .. } => RoadName::Brew,
            InstallRoad::Npm { .. } => RoadName::Npm,
            InstallRoad::Pnpm(_) => RoadName::Pnpm,
            InstallRoad::Bun(_) => RoadName::Bun,
            InstallRoad::Uv(_) => RoadName::Uv,
            InstallRoad::Pipx(_) => RoadName::Pipx,
            InstallRoad::Cargo(_) => RoadName::Cargo,
            InstallRoad::Go { .. } => RoadName::Go,
            InstallRoad::Release { .. } => RoadName::Release,
            InstallRoad::Vendor { .. } => RoadName::Vendor,
            InstallRoad::Apt { .. } => RoadName::Apt,
            InstallRoad::Script { .. } => RoadName::Script,
        }
    }

    /// The road's own `version` field, where its kind has one.
    pub fn version(&self) -> Option<&str> {
        match self {
            InstallRoad::Brew { .. } | InstallRoad::Apt { .. } => None,
            InstallRoad::Npm { package, .. }
            | InstallRoad::Pnpm(package)
            | InstallRoad::Bun(package)
            | InstallRoad::Uv(package)
            | InstallRoad::Pipx(package)
            | InstallRoad::Cargo(package) => package.version.as_deref(),
            InstallRoad::Go { version, .. }
            | InstallRoad::Release { version, .. }
            | InstallRoad::Vendor { version, .. }
            | InstallRoad::Script { version, .. } => version.as_deref(),
        }
    }

    pub fn pin(&self) -> Option<&ToolPin> {
        match self {
            InstallRoad::Brew { pin, .. }
            | InstallRoad::Go { pin, .. }
            | InstallRoad::Release { pin, .. }
            | InstallRoad::Vendor { pin, .. }
            | InstallRoad::Apt { pin, .. }
            | InstallRoad::Script { pin, .. } => pin.as_ref(),
            InstallRoad::Npm { package, .. }
            | InstallRoad::Pnpm(package)
            | InstallRoad::Bun(package)
            | InstallRoad::Uv(package)
            | InstallRoad::Pipx(package)
            | InstallRoad::Cargo(package) => package.pin.as_ref(),
        }
    }

    /// The package a package road names (`"package" in road`).
    pub fn package(&self) -> Option<&str> {
        match self {
            InstallRoad::Npm { package, .. }
            | InstallRoad::Pnpm(package)
            | InstallRoad::Bun(package)
            | InstallRoad::Uv(package)
            | InstallRoad::Pipx(package)
            | InstallRoad::Cargo(package) => Some(package.package.as_str()),
            _ => None,
        }
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
        match self {
            RoadName::Brew => "brew",
            RoadName::Npm => "npm",
            RoadName::Pnpm => "pnpm",
            RoadName::Bun => "bun",
            RoadName::Uv => "uv",
            RoadName::Pipx => "pipx",
            RoadName::Cargo => "cargo",
            RoadName::Go => "go",
            RoadName::Release => "release",
            RoadName::Vendor => "vendor",
            RoadName::Apt => "apt",
            RoadName::Script => "script",
        }
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
    ROADS.into_iter().find(|r| Some(r.as_str()) == s)
}

/// The row's own version, else the standing pin's tag (roads.ts:97).
pub fn version_of(road: &InstallRoad) -> Option<String> {
    road.version().map(str::to_string).or_else(|| standing_pin(road).map(|p| p.tag))
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
    for &(from, to) in rewrites.unwrap_or(&GUEST_REWRITES) {
        if let Some(rest) = rel.strip_prefix(from) {
            return format!("{to}{rest}");
        }
        if from.ends_with('/') && rel == &from[..from.len() - 1] {
            return to.strip_suffix('/').unwrap_or(to).to_string();
        }
    }
    rel.to_string()
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

const CLAUDE_DOWNLOADS: &str = "https://downloads.claude.ai/claude-code-releases";

/// The address whose whole answer is the newest version the vendor publishes, which its own installer reads.
pub static CLAUDE_LATEST: LazyLock<String> = LazyLock::new(|| format!("{CLAUDE_DOWNLOADS}/latest"));

/// The binary alone, since the vendor's installer reads the current version off the network, in the folder every
/// pinned command lands in: the home a box shares with its workspaces is off its job's PATH and each can write it.
pub static CLAUDE_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [
        r#"arch="$(uname -m)""#,
        r#"case "$arch" in"#,
        &*format!("  x86_64) plat=linux-x64 sha={} ;;", CLAUDE_CODE.sha256.x86_64),
        &*format!("  aarch64) plat=linux-arm64 sha={} ;;", CLAUDE_CODE.sha256.aarch64),
        r#"  *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#,
        "esac",
        "trap 'rm -f /tmp/claude' EXIT",
        &*format!(r#"curl -o /tmp/claude "{CLAUDE_DOWNLOADS}/{}/$plat/claude""#, CLAUDE_CODE.version),
        r#"echo "$sha  /tmp/claude" | sha256sum -c - >/dev/null"#,
        &*format!("install -D -m 0755 /tmp/claude {LOCAL_BIN}/claude"),
    ]
    .join("\n")
});

/// What the image build runs to put the harness on a first-life builder, recorded in the manifest as setupSha; the
/// smoke is what proves the result. An update runs it too, on an image that may hold the harness under the home,
/// first on TOOLS_PATH, so that file goes first: a builder's home is root's alone.
pub static GOLDEN_SETUP: LazyLock<String> = LazyLock::new(|| format!("rm -f {HOME_BIN}/claude\n{}", *CLAUDE_INSTALL));
pub const GOLDEN_SMOKE: &str = "claude --version";

pub const UV: VersionPin = VersionPin {
    version: "0.12.9",
    sha256: ArchSums {
        x86_64: "ec7a99cd05e0cd7f80243f135ce1361c76835cb0ee60055d14d20eba8eba1460",
        aarch64: "c36fe17937ff6bd16dc42fc13854b5465999fcab2efe0af559381e945e3c6001",
    },
};

pub static UV_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [
        "if ! command -v uv >/dev/null 2>&1; then",
        r#"  arch="$(uname -m)""#,
        r#"  case "$arch" in"#,
        &*format!("    x86_64) sha={} ;;", UV.sha256.x86_64),
        &*format!("    aarch64) sha={} ;;", UV.sha256.aarch64),
        r#"    *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#,
        "  esac",
        r#"  pkg="uv-$arch-unknown-linux-gnu.tar.gz""#,
        &*format!(r#"  curl -o "/tmp/$pkg" "https://github.com/astral-sh/uv/releases/download/{}/$pkg""#, UV.version),
        r#"  echo "$sha  /tmp/$pkg" | sha256sum -c - >/dev/null"#,
        r#"  tar -xzf "/tmp/$pkg" -C /tmp"#,
        r#"  install -m 0755 "/tmp/uv-$arch-unknown-linux-gnu/uv" /usr/local/bin/uv"#,
        r#"  install -m 0755 "/tmp/uv-$arch-unknown-linux-gnu/uvx" /usr/local/bin/uvx"#,
        r#"  rm -rf "/tmp/$pkg" "/tmp/uv-$arch-unknown-linux-gnu""#,
        "fi",
    ]
    .join("\n")
});

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
    NODE_RELEASES.iter().find(|(m, _)| *m == major).map(|(_, r)| *r)
}

pub const NODE_PATH_LINE: &str = "export PATH=\"/usr/local/bin:$PATH\"";
pub const NODE_BIN: &str = "node";

/// roads.ts:236.
pub fn node_install_script(floor: u32, release: &NodeRelease) -> String {
    let v = release.version;
    let major: u32 = v.split('.').next().and_then(|m| m.parse().ok()).unwrap_or(0);
    [
        r#"node_have="$(node --version 2>/dev/null || echo v0)""#,
        r#"node_major="$(printf '%s' "$node_have" | sed 's/^v//; s/\..*//')""#,
        r#"echo "NODE_HAVE $node_have""#,
        &*format!(r#"if [ "${{node_major:-0}}" -ge {floor} ] && [ "${{node_major:-0}}" -le {major} ]; then echo "NODE_KEPT $node_have"; exit 0; fi"#),
        r#"arch="$(uname -m)""#,
        r#"case "$arch" in"#,
        &*format!("  x86_64) pkg=node-v{v}-linux-x64.tar.gz sha={} ;;", release.sha256.x86_64),
        &*format!("  aarch64) pkg=node-v{v}-linux-arm64.tar.gz sha={} ;;", release.sha256.aarch64),
        r#"  *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#,
        "esac",
        &*format!(r#"curl -o "/tmp/$pkg" "https://nodejs.org/dist/v{v}/$pkg""#),
        r#"echo "$sha  /tmp/$pkg" | sha256sum -c - >/dev/null"#,
        r#"tar -xzf "/tmp/$pkg" -C /usr/local --strip-components=1"#,
        r#"rm -f "/tmp/$pkg""#,
        NODE_PATH_LINE,
        &*format!(r#"test "$(node --version)" = "v{v}""#),
        &*format!(r#"echo "NODE_INSTALLED v{v}""#),
    ]
    .join("\n")
}

/// Docker Compose by its release binary, checksummed against the sums Docker
/// publishes next to it (https://github.com/docker/compose/releases).
pub const COMPOSE: VersionPin = VersionPin {
    version: "v5.5.1",
    sha256: ArchSums {
        x86_64: "db1889184726840f75c4f9c001048430d4f25b3be3cb084d3ddd762bc0aed576",
        aarch64: "732e3a84c1a0f67256ce80bc2598a24546b10ca05f9faa97efceb1171ece2ef7",
    },
};
pub const COMPOSE_PLUGIN: &str = "/usr/libexec/docker/cli-plugins/docker-compose";

/// The engine from the distro, compose from its release: Debian bookworm, the machines' base, packages
/// docker.io but no compose v2, so the plugin is fetched pinned and put where the cli reads it.
pub static DOCKER_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [
        APT_ENV,
        "apt-get install -y -qq docker.io",
        r#"arch="$(uname -m)""#,
        r#"case "$arch" in"#,
        &*format!("  x86_64) sha={} ;;", COMPOSE.sha256.x86_64),
        &*format!("  aarch64) sha={} ;;", COMPOSE.sha256.aarch64),
        r#"  *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#,
        "esac",
        &*format!(
            r#"curl -o /tmp/docker-compose "https://github.com/docker/compose/releases/download/{}/docker-compose-linux-$arch""#,
            COMPOSE.version
        ),
        r#"echo "$sha  /tmp/docker-compose" | sha256sum -c - >/dev/null"#,
        &*format!("install -D -m 0755 /tmp/docker-compose {COMPOSE_PLUGIN}"),
        "rm -f /tmp/docker-compose",
    ]
    .join("\n")
});

/// Python 3.12 as uv's managed interpreter: uv pins the python-build-standalone release and checks its sha256,
/// so the pin is uv's own; python3 on PATH is a link to that interpreter, ahead of whatever the image ships.
pub static PYTHON_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [UV_INSTALL.as_str(), "uv python install 3.12", r#"ln -sfn "$(uv python find --managed-python 3.12)" /usr/local/bin/python3"#]
        .join("\n")
});

pub const RUSTUP: VersionPin = VersionPin {
    version: "1.29.1",
    sha256: ArchSums {
        x86_64: "dda7234360b7f578ca8b0ddcb80145646fa61a67c1720a5abc7051b35c9fcb71",
        aarch64: "15f6e4ce9f583b929c996c91562bad6d4454f3281de858b02cdfdef615fac433",
    },
};

/// The stable toolchain in rustup's default profile, which is cargo, rustc, the standard library, clippy, rustfmt and
/// the docs. Homebrew's rust bottle would link against llvm@22 instead, 2.5 GB of its 3.1 GB closure on Linux. The
/// installer leaves the shell files alone: the guest's PATH already carries the cargo bin directory.
pub static RUSTUP_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [
        r#"arch="$(uname -m)""#,
        r#"case "$arch" in"#,
        &*format!("  x86_64) sha={} ;;", RUSTUP.sha256.x86_64),
        &*format!("  aarch64) sha={} ;;", RUSTUP.sha256.aarch64),
        r#"  *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#,
        "esac",
        &*format!(
            r#"curl -o /tmp/rustup-init "https://static.rust-lang.org/rustup/archive/{}/$arch-unknown-linux-gnu/rustup-init""#,
            RUSTUP.version
        ),
        r#"echo "$sha  /tmp/rustup-init" | sha256sum -c - >/dev/null"#,
        "chmod +x /tmp/rustup-init",
        "/tmp/rustup-init -y --no-modify-path --profile default --default-toolchain stable",
        "rm -f /tmp/rustup-init",
    ]
    .join("\n")
});

/// Debian ships fd as fdfind to dodge a name clash; agents type fd, so the row links it onto PATH under that name.
pub static FD_INSTALL: LazyLock<String> =
    LazyLock::new(|| [APT_ENV, "apt-get install -y -qq fd-find", "ln -sfn /usr/bin/fdfind /usr/local/bin/fd"].join("\n"));

/// Yarn through the corepack Node 22 ships, at the exact version corepack's own `stable` alias named on 2026-09-22
/// (https://repo.yarnpkg.com/tags, aliases.stable), with the download prompt off: the alias moves under an image
/// build, a version does not.
pub const YARN_VERSION: &str = "4.18.0";
pub static YARN_INSTALL: LazyLock<String> = LazyLock::new(|| {
    ["corepack enable yarn".to_string(), format!("COREPACK_ENABLE_DOWNLOAD_PROMPT=0 corepack install -g yarn@{YARN_VERSION}")].join("\n")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct GitPin {
    pub tag: &'static str,
    pub commit: &'static str,
}

pub const HERMES: GitPin = GitPin { tag: "v2026.8.31", commit: "29112bef099274229cadff79cdff7bf7b99c4b77" };

/// https://hermes-agent.nousresearch.com/docs/developer-guide/contributing#manual-clone-fallback
pub static HERMES_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [
        UV_INSTALL.as_str(),
        "if [ ! -d /root/.hermes/hermes-agent/.git ]; then",
        &*format!(
            "  git clone -q --depth 1 --branch {} https://github.com/NousResearch/hermes-agent.git /root/.hermes/hermes-agent",
            HERMES.tag
        ),
        "fi",
        &*format!(r#"test "$(git -C /root/.hermes/hermes-agent rev-parse HEAD)" = "{}""#, HERMES.commit),
        "uv venv --python 3.11 /root/.hermes/venvs/hermes",
        "uv pip install --python /root/.hermes/venvs/hermes/bin/python -e /root/.hermes/hermes-agent",
        "ln -sfn /root/.hermes/venvs/hermes/bin/hermes /usr/local/bin/hermes",
    ]
    .join("\n")
});

/// Playwright's own Chromium build for the Playwright the web render tests import: the browser lives under
/// ~/.cache/ms-playwright keyed by that version's revision, so a project on another Playwright looks for another one.
pub const PLAYWRIGHT_VERSION: &str = "1.62.1";

/// The global first, then Playwright's installer for its Chromium and the Debian packages the browser needs; that
/// installer reads the apt index itself. Where npm's own folder is on no PATH yet (a builder whose Node is nvm's, which
/// the seal then puts on the image's PATH), the installer is found there, never under the home a computer somebody
/// owns shares with every workspace on it.
pub static PLAYWRIGHT_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [
        APT_ENV,
        &*format!("npm install -g playwright@{PLAYWRIGHT_VERSION}"),
        &*format!(r#"command -v playwright >/dev/null || {{ b="$(npm prefix -g)/bin"; case "$b" in {GUEST_HOME}/*) ;; *) PATH="$b:$PATH" ;; esac; }}"#),
        "playwright install --with-deps chromium",
    ]
    .join("\n")
});

/// Swift by swift.org's Debian 12 toolchain tarball; swift.org signs it and publishes no sum, so the sums are the
/// tarballs' own as read on 2026-09-07 (https://www.swift.org/install/linux/debian/12/).
pub const SWIFT: VersionPin = VersionPin {
    version: "6.3.3",
    sha256: ArchSums {
        x86_64: "19e0c78cad5418ad48bfa87aa20c53ac9ac9996d1695d04dd94f7c7ea4eb133f",
        aarch64: "ecba8ef87b54a5048d466af500f3169c939a6b8a2cb7c600f76b5184457f293a",
    },
};

const SWIFT_HOME: &str = "/opt/swift";

/// The packages swift.org's own Debian 12 image installs ahead of the toolchain
/// (https://github.com/swiftlang/swift-docker/blob/main/6.3/debian/12/Dockerfile). The toolchain unpacks under its
/// own prefix, since it ships a clang and an lld of its own that would shadow the LLVM row's under /usr/bin.
pub static SWIFT_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [
        APT_ENV,
        "apt-get install -y -qq binutils libicu-dev libcurl4-openssl-dev libedit-dev libsqlite3-dev libncurses-dev libpython3-dev libxml2-dev pkg-config uuid-dev tzdata git gcc libstdc++-12-dev",
        r#"arch="$(uname -m)""#,
        r#"case "$arch" in"#,
        &*format!("  x86_64) dir=debian12 sha={} ;;", SWIFT.sha256.x86_64),
        &*format!("  aarch64) dir=debian12-aarch64 sha={} ;;", SWIFT.sha256.aarch64),
        r#"  *) echo "unsupported arch: $arch" >&2; exit 1 ;;"#,
        "esac",
        &*format!(r#"pkg="swift-{}-RELEASE-$dir.tar.gz""#, SWIFT.version),
        &*format!(r#"curl -o "/tmp/$pkg" "https://download.swift.org/swift-{v}-release/$dir/swift-{v}-RELEASE/$pkg""#, v = SWIFT.version),
        r#"echo "$sha  /tmp/$pkg" | sha256sum -c - >/dev/null"#,
        &*format!("rm -rf {SWIFT_HOME} && mkdir -p {SWIFT_HOME}"),
        &*format!(r#"tar -xzf "/tmp/$pkg" -C {SWIFT_HOME} --strip-components=1"#),
        r#"rm -f "/tmp/$pkg""#,
        &*format!("chmod -R o+r {SWIFT_HOME}/usr/lib/swift"),
        &*format!("ln -sfn {SWIFT_HOME}/usr/bin/swift /usr/local/bin/swift"),
        &*format!("ln -sfn {SWIFT_HOME}/usr/bin/swiftc /usr/local/bin/swiftc"),
    ]
    .join("\n")
});

/// 1Password's apt signing key as published on 2026-09-07 (https://downloads.1password.com/linux/keys/1password.asc);
/// a rotated key fails the install instead of being trusted unread.
const ONE_PASSWORD_KEY_SHA256: &str = "f39e7dd9dedc581ced85732832f217e0de5860a3b80279b5af4bc7c6d8157bae";

const ONE_PASSWORD_KEYRING: &str = "/usr/share/keyrings/1password-archive-keyring.asc";
const ONE_PASSWORD_LIST: &str = "/etc/apt/sources.list.d/1password.list";

/// 1Password publishes the CLI only through its own apt repository (https://developer.1password.com/docs/cli/get-started/),
/// so the road adds the repository under its key and reads that one index before the install.
pub static OP_INSTALL: LazyLock<String> = LazyLock::new(|| {
    [
        APT_ENV.to_string(),
        format!("curl -o {ONE_PASSWORD_KEYRING} https://downloads.1password.com/linux/keys/1password.asc"),
        format!(r#"echo "{ONE_PASSWORD_KEY_SHA256}  {ONE_PASSWORD_KEYRING}" | sha256sum -c - >/dev/null"#),
        r#"arch="$(dpkg --print-architecture)""#.to_string(),
        format!(r#"echo "deb [arch=$arch signed-by={ONE_PASSWORD_KEYRING}] https://downloads.1password.com/linux/debian/$arch stable main" > {ONE_PASSWORD_LIST}"#),
        format!("apt-get update -qq -o Dir::Etc::sourcelist={ONE_PASSWORD_LIST} -o Dir::Etc::sourceparts=- -o APT::Get::List-Cleanup=0"),
        "apt-get install -y -qq 1password-cli".to_string(),
    ]
    .join("\n")
});
