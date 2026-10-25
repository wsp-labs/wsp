// SPDX-License-Identifier: AGPL-3.0-only
//! Commands with a Linux build of their own that the vendor road installs, each pinned with a sum per arch. Port of
//! packages/catalog/src/linux-casks.ts at a3b870a6e; the spec is spec/roads.md.
use std::sync::LazyLock;

use serde::{Deserialize, Serialize};

use wsp_wire::types::shell_quote::shell_quote;
use crate::roads::{ArchSums, VersionPin};

/// linux-casks.ts:387.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinuxCask {
    pub bin: String,
    pub from: String,
    pub detail: String,
    pub version: String,
    pub sha256: CaskSums,
    pub install: String,
    pub uninstall: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CaskSums {
    pub x86_64: String,
    pub aarch64: String,
}

fn sums(s: ArchSums) -> CaskSums {
    CaskSums { x86_64: s.x86_64.to_string(), aarch64: s.aarch64.to_string() }
}

const GCLOUD_PIN: VersionPin = VersionPin {
    version: "586.0.0",
    sha256: ArchSums {
        x86_64: "6c774c76793eedd501150b59da653610fbe3eaac169e822965b722de75a2f001",
        aarch64: "e50ea0141a027d5118d7dd011d2870e706466dc0fc437d95cae86a14e0d871bc",
    },
};

const KUBECTL_PIN: VersionPin = VersionPin {
    version: "v1.37.0",
    sha256: ArchSums {
        x86_64: "6129359f4e1f3848a5572ccb0b26cf28b8ca08cef38c95a765b2f64a2c961a2f",
        aarch64: "922df28df248cc00a9e025f947704f1d1482de64ece54cfe57e61f19eaf1eef3",
    },
};

const CURSOR_PIN: VersionPin = VersionPin {
    version: "2026.09.26-dd393fe",
    sha256: ArchSums {
        x86_64: "8085fd120f5c71f4eae7fea26a043718e5644e3071e4fab3220a0e58c51f9593",
        aarch64: "ab1178d0d8c10b254e7e427d1d673533a389338424e75034be9ab9da02845bde",
    },
};

const PRELUDE: [&str; 4] =
    ["set -euo pipefail", r#"arch="$(uname -m)""#, r#"tmp="$(mktemp -d /tmp/wsp-cask-XXXXXX)""#, r#"trap 'rm -rf "$tmp"' EXIT"#];

/// This vendor's own word for the arch, and the sum it pins for that arch's download, read together.
fn arch_case(x86: &str, arm: &str, sha256: ArchSums) -> String {
    format!(
        r#"case "$arch" in x86_64) a={x86} sha={} ;; aarch64) a={arm} sha={} ;; *) echo "Error: unsupported arch: $arch" >&2; exit 1 ;; esac"#,
        sha256.x86_64, sha256.aarch64
    )
}

fn script(body: &[&str]) -> String {
    PRELUDE.iter().copied().chain(body.iter().copied()).collect::<Vec<_>>().join("\n")
}

const GCLOUD_HOME: &str = "/opt/google-cloud-sdk";
const GCLOUD_BINS: [&str; 3] = ["gcloud", "gsutil", "bq"];

/// The x86_64 tarball bundles a Python; the arm one runs on the machine's python3.
pub static GCLOUD: LazyLock<LinuxCask> = LazyLock::new(|| {
    let arch = arch_case("x86_64", "arm", GCLOUD_PIN.sha256);
    let ver = format!("ver={}", shell_quote(GCLOUD_PIN.version));
    let rm = format!("rm -rf {GCLOUD_HOME}");
    let links: Vec<String> = GCLOUD_BINS.iter().map(|b| format!("ln -sf {GCLOUD_HOME}/bin/{b} /usr/local/bin/{b}")).collect();
    let mut body = vec![
        arch.as_str(),
        r#"[ "$a" != arm ] || command -v python3 >/dev/null || { echo "Error: gcloud on arm needs python3 on the machine; Google's arm tarball bundles none" >&2; exit 1; }"#,
        ver.as_str(),
        r#"pkg="google-cloud-cli-$ver-linux-$a.tar.gz""#,
        r#"curl -o "$tmp/$pkg" "https://dl.google.com/dl/cloudsdk/channels/rapid/downloads/$pkg""#,
        r#"echo "$sha  $tmp/$pkg" | sha256sum -c - >/dev/null"#,
        rm.as_str(),
        r#"tar -xzf "$tmp/$pkg" -C /opt"#,
    ];
    body.extend(links.iter().map(String::as_str));
    body.push(r#"echo "WSP_ROAD release $pkg $sha $ver""#);
    LinuxCask {
        bin: "gcloud".into(),
        from: "Google's Linux release".into(),
        detail: "from Google's Linux release, at the version and sum the catalog pins".into(),
        version: GCLOUD_PIN.version.into(),
        sha256: sums(GCLOUD_PIN.sha256),
        install: script(&body),
        uninstall: format!("rm -rf {GCLOUD_HOME} {}", GCLOUD_BINS.map(|b| format!("/usr/local/bin/{b}")).join(" ")),
    }
});

/// The static binary from the Kubernetes release, at the sums Kubernetes publishes beside it.
pub static KUBECTL: LazyLock<LinuxCask> = LazyLock::new(|| {
    let arch = arch_case("amd64", "arm64", KUBECTL_PIN.sha256);
    let ver = format!("ver={}", shell_quote(KUBECTL_PIN.version));
    let body = [
        arch.as_str(),
        ver.as_str(),
        r#"curl -o "$tmp/kubectl" "https://dl.k8s.io/release/$ver/bin/linux/$a/kubectl""#,
        r#"echo "$sha  $tmp/kubectl" | sha256sum -c - >/dev/null"#,
        r#"install -m 0755 "$tmp/kubectl" /usr/local/bin/kubectl"#,
        r#"echo "WSP_ROAD release kubectl-$ver-linux-$a $sha $ver""#,
    ];
    LinuxCask {
        bin: "kubectl".into(),
        from: "Kubernetes release".into(),
        detail: "kubectl only, from the Kubernetes release; Docker itself has no Linux build".into(),
        version: KUBECTL_PIN.version.into(),
        sha256: sums(KUBECTL_PIN.sha256),
        install: script(&body),
        uninstall: "rm -f /usr/local/bin/kubectl".into(),
    }
});

const CURSOR_HOME: &str = "/opt/cursor-agent";

/// The tarball is the CLI with its own node beside it; its launcher follows a link back to that folder.
pub static CURSOR: LazyLock<LinuxCask> = LazyLock::new(|| {
    let arch = arch_case("x64", "arm64", CURSOR_PIN.sha256);
    let ver = format!("ver={}", shell_quote(CURSOR_PIN.version));
    let rm = format!("rm -rf {CURSOR_HOME}");
    let mk = format!("mkdir -p {CURSOR_HOME}");
    let tar = format!("tar --strip-components=1 -xzf \"$tmp/$pkg\" -C {CURSOR_HOME}");
    let link = format!("ln -sf {CURSOR_HOME}/cursor-agent /usr/local/bin/cursor-agent");
    let body = [
        arch.as_str(),
        ver.as_str(),
        r#"pkg="agent-cli-package.tar.gz""#,
        r#"curl -o "$tmp/$pkg" "https://downloads.cursor.com/lab/$ver/linux/$a/$pkg""#,
        r#"echo "$sha  $tmp/$pkg" | sha256sum -c - >/dev/null"#,
        rm.as_str(),
        mk.as_str(),
        tar.as_str(),
        link.as_str(),
        r#"echo "WSP_ROAD release cursor-agent-$ver-linux-$a $sha $ver""#,
    ];
    LinuxCask {
        bin: "cursor-agent".into(),
        from: "Cursor's Linux release".into(),
        detail: "from Cursor's Linux release, at the version and sum the catalog pins".into(),
        version: CURSOR_PIN.version.into(),
        sha256: sums(CURSOR_PIN.sha256),
        install: script(&body),
        uninstall: format!("rm -rf {CURSOR_HOME} /usr/local/bin/cursor-agent"),
    }
});

/// gcloud, kubectl, cursor-agent, in that order (linux-casks.ts:507).
pub fn linux_casks() -> Vec<&'static LinuxCask> {
    vec![&*GCLOUD, &*KUBECTL, &*CURSOR]
}
