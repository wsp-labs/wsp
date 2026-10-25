// SPDX-License-Identifier: AGPL-3.0-only
//! The toolchains rung: version managers and the pins that travel. Port of packages/collect/src/detect/toolchains.ts
//! at a3b870a6e; the spec is spec/detect_small.md.
use crate::detect::common::{entry, exists, first_line, found, EntrySpec, RowOptions};
use crate::host::{Host, RunOptions};
use crate::manifest::{ManifestEntry, Rung};

/// A manager is installed when its binary is on PATH or one of its markers exists; `pins` are the files that travel.
struct Manager {
    id: &'static str,
    bin: Option<&'static str>,
    markers: &'static [&'static str],
    pins: &'static [&'static str],
}

const MANAGERS: &[Manager] = &[
    Manager {
        id: "mise",
        bin: Some("mise"),
        markers: &["~/.config/mise/config.toml"],
        pins: &["~/.config/mise/config.toml", "~/.tool-versions"],
    },
    Manager { id: "asdf", bin: Some("asdf"), markers: &["~/.asdf"], pins: &["~/.tool-versions", "~/.asdfrc"] },
    Manager { id: "fnm", bin: Some("fnm"), markers: &[], pins: &["~/.node-version", "~/.nvmrc"] },
    Manager { id: "nvm", bin: None, markers: &["~/.nvm"], pins: &["~/.nvmrc"] },
    Manager { id: "pyenv", bin: Some("pyenv"), markers: &["~/.pyenv"], pins: &["~/.pyenv/version"] },
    Manager { id: "uv", bin: Some("uv"), markers: &[], pins: &["~/.config/uv/uv.toml", "~/.python-version"] },
    Manager { id: "rustup", bin: None, markers: &["~/.rustup/settings.toml"], pins: &["~/.rustup/settings.toml", "~/.cargo/config.toml"] },
    Manager { id: "go", bin: Some("go"), markers: &[], pins: &["~/.config/go/env"] },
];

async fn label_of(host: &Host, id: &str) -> String {
    match id {
        "mise" => "mise pins".to_string(),
        "asdf" => "asdf pins".to_string(),
        "fnm" => {
            let out = host.exec.run("fnm", &["current".to_string()], RunOptions::default()).await;
            first_line(out.as_deref()).map_or_else(|| "fnm".to_string(), |v| format!("fnm (node {v})"))
        }
        "nvm" => {
            let alias = host.fs.read_text(&format!("{}/.nvm/alias/default", host.home)).await;
            first_line(alias.as_deref()).map_or_else(|| "nvm".to_string(), |v| format!("nvm (default {v})"))
        }
        "pyenv" => "pyenv (global python)".to_string(),
        "uv" => "uv (python versions)".to_string(),
        "rustup" => "rustup default toolchain".to_string(),
        _ => {
            let out = host.exec.run("go", &["version".to_string()], RunOptions::default()).await;
            let line = first_line(out.as_deref()).unwrap_or_default();
            let version = regex::Regex::new(r"go([0-9]+(?:\.[0-9]+)*)").expect("a literal pattern compiles");
            match version.captures(&line) {
                Some(caps) => format!("go {}", &caps[1]),
                None => "go".to_string(),
            }
        }
    }
}

pub async fn detect_toolchains(host: &Host) -> Vec<ManifestEntry> {
    let mut rows = Vec::new();
    for m in MANAGERS {
        let mut hit = match m.bin {
            Some(bin) => host.exec.which(bin).await,
            None => false,
        };
        for marker in m.markers {
            hit = hit || exists(host, marker).await;
        }
        if !hit {
            continue;
        }
        let label = label_of(host, m.id).await;
        let pins: Vec<String> = m.pins.iter().map(|p| p.to_string()).collect();
        let f = found(host, &pins).await;
        rows.push(entry(EntrySpec {
            rung: Rung::Toolchains,
            id: format!("toolchains/{}", m.id),
            label,
            paths: f.paths,
            bytes: f.bytes,
            opt: RowOptions::default(),
        }));
    }
    rows
}
