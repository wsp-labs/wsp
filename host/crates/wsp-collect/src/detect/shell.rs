// SPDX-License-Identifier: AGPL-3.0-only
//! The shell rung's rows: rc files, prompts, multiplexers, plugin managers and history. Port of
//! packages/collect/src/detect/shell.ts at a3b870a6e; the spec is spec/detect_small.md.
use wsp_words::js::js_trim;

use crate::detect::common::{exists, item, present, row, RowOptions, RowSpec};
use crate::host::Host;
use crate::manifest::{ManifestEntry, RowDefault, Rung};

/// shell.ts:6, in this order.
pub const RC_FILES: [&str; 10] =
    ["zshrc", "zshenv", "zprofile", "zlogin", "bashrc", "bash_profile", "profile", "inputrc", "aliases", "zsh_aliases"];

/// The name of the login shell, when SHELL names one /etc/shells lists.
pub async fn login_shell(host: &Host) -> Option<String> {
    let shell = host.shell.as_deref()?;
    let listed = host.fs.read_text("/etc/shells").await.unwrap_or_default();
    if !listed.split('\n').map(js_trim).any(|line| line == shell) {
        return None;
    }
    let start = shell.rfind('/').map_or(0, |i| i + 1);
    Some(shell[start..].to_string())
}

const ROWS: &[(&str, &str, &[&str])] = &[
    ("shell/fish", "fish config", &["~/.config/fish"]),
    ("shell/sheldon", "sheldon plugins", &["~/.config/sheldon"]),
    ("shell/starship", "starship prompt", &["~/.config/starship.toml"]),
    ("shell/p10k", "powerlevel10k prompt", &["~/.p10k.zsh"]),
    ("shell/oh-my-posh", "oh-my-posh prompt", &["~/.config/oh-my-posh"]),
    ("shell/tmux", "tmux config", &["~/.tmux.conf", "~/.config/tmux"]),
    ("shell/zellij", "zellij config", &["~/.config/zellij"]),
    ("shell/direnv", "direnv config", &["~/.config/direnv", "~/.direnvrc"]),
];

/// Plugin managers whose install is a line in the rc file; only a separate plugin list travels.
struct Manager {
    id: &'static str,
    label: &'static str,
    markers: &'static [&'static str],
    list: Option<&'static str>,
}

const MANAGERS: &[Manager] = &[
    Manager { id: "zinit", label: "zinit (reinstalls from ~/.zshrc)", markers: &["~/.local/share/zinit", "~/.zinit"], list: None },
    Manager { id: "zplug", label: "zplug (reinstalls from ~/.zshrc)", markers: &["~/.zplug"], list: None },
    Manager { id: "antidote", label: "antidote plugin list", markers: &["~/.antidote"], list: Some("~/.zsh_plugins.txt") },
    Manager {
        id: "fisher",
        label: "fisher plugin list",
        markers: &["~/.config/fish/functions/fisher.fish"],
        list: Some("~/.config/fish/fish_plugins"),
    },
];

const HISTORY: &[&str] = &["~/.zsh_history", "~/.bash_history", "~/.local/share/fish/fish_history", "~/.local/share/atuin"];

fn spec_of(id: &str, label: &str, paths: &[&str]) -> RowSpec {
    RowSpec {
        rung: Rung::Shell,
        id: id.to_string(),
        label: label.to_string(),
        paths: paths.iter().map(|p| p.to_string()).collect(),
        always: false,
        opt: RowOptions::default(),
    }
}

pub async fn detect_shell(host: &Host) -> Vec<ManifestEntry> {
    let mut rows: Vec<Option<ManifestEntry>> = Vec::new();
    for name in RC_FILES {
        let path = format!("~/.{name}");
        rows.push(row(host, spec_of(&format!("shell/{name}"), &path, &[path.as_str()])).await);
    }
    for (id, label, paths) in ROWS {
        rows.push(row(host, spec_of(id, label, paths)).await);
    }

    if exists(host, "~/.oh-my-zsh").await {
        let mut spec = spec_of("shell/oh-my-zsh", "oh-my-zsh custom dir (the framework reinstalls)", &["~/.oh-my-zsh/custom"]);
        spec.always = true;
        rows.push(row(host, spec).await);
    }
    for m in MANAGERS {
        let mut hit = false;
        for marker in m.markers {
            hit = hit || exists(host, marker).await;
        }
        if !hit {
            continue;
        }
        let id = format!("shell/{}", m.id);
        rows.push(match m.list {
            None => Some(item(Rung::Shell, &id, m.label, RowOptions::default())),
            Some(list) => {
                let mut spec = spec_of(&id, m.label, &[list]);
                spec.always = true;
                row(host, spec).await
            }
        });
    }

    let mut history = spec_of("shell/history", "shell history", HISTORY);
    history.opt.default = Some(RowDefault::Skip);
    rows.push(row(host, history).await);
    let login = login_shell(host).await;
    let mut out = present(rows);
    if let Some(login) = login {
        for r in &mut out {
            r.login = Some(login.clone());
        }
    }
    out
}
