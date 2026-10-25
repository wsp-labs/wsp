// SPDX-License-Identifier: AGPL-3.0-only
//! The identity rung: git's name, email and signing, the ssh config, public and private keys, known hosts and GPG.
//! Port of packages/collect/src/detect/identity.ts at a3b870a6e; the spec is spec/detect_small.md.
use crate::detect::common::{entry, exists, first_line, found, present, row, EntrySpec, RowOptions, RowSpec};
use crate::host::{Host, RunOptions, StatKind};
use crate::manifest::{ManifestEntry, RowDefault, Rung};
use crate::ssh_config::{SSH_CONFIG_PATH, SSH_IGNORE_UNKNOWN_DETAIL};

pub const GIT_CONFIGS: [&str; 2] = ["~/.gitconfig", "~/.config/git/config"];
/// Names under ~/.ssh that are not keys (identity.ts:12).
pub const SSH_NOT_KEYS: [&str; 5] = ["config", "authorized_keys", "environment", "rc", "allowed_signers"];

async fn git_get(host: &Host, key: &str) -> Option<String> {
    let args = ["config", "--global", "--get", key].map(String::from);
    first_line(host.exec.run("git", &args, RunOptions::default()).await.as_deref())
}

pub async fn detect_identity(host: &Host) -> Vec<ManifestEntry> {
    let mut rows: Vec<Option<ManifestEntry>> = Vec::new();
    let has_git = host.exec.which("git").await;
    let configs = found(host, &GIT_CONFIGS.map(String::from)).await;

    let name = if has_git { git_get(host, "user.name").await } else { None };
    let email = if has_git { git_get(host, "user.email").await } else { None };
    let has_user = name.is_some() || email.is_some();
    if has_user || !configs.paths.is_empty() {
        rows.push(Some(entry(EntrySpec {
            rung: Rung::Identity,
            id: "identity/git-user".into(),
            label: "git name and email".into(),
            paths: configs.paths,
            bytes: configs.bytes,
            opt: RowOptions { required: has_user.then_some(true), ..Default::default() },
        })));
    }

    if has_git {
        let format = git_get(host, "gpg.format").await;
        let signs = match git_get(host, "commit.gpgsign").await {
            None => git_get(host, "user.signingkey").await,
            some => some,
        };
        if format.is_some() || signs.is_some() {
            let signers = if format.as_deref() == Some("ssh") { git_get(host, "gpg.ssh.allowedSignersFile").await } else { None };
            let f = found(host, &signers.into_iter().collect::<Vec<String>>()).await;
            rows.push(Some(entry(EntrySpec {
                rung: Rung::Identity,
                id: "identity/git-signing".into(),
                label: format!("git signing ({}); the signing key itself stays on this computer", format.as_deref().unwrap_or("gpg")),
                paths: f.paths,
                bytes: f.bytes,
                opt: RowOptions::default(),
            })));
        }
    }

    rows.push(
        row(
            host,
            RowSpec {
                rung: Rung::Identity,
                id: "identity/ssh-config".into(),
                label: SSH_CONFIG_PATH.into(),
                paths: vec![SSH_CONFIG_PATH.into()],
                always: false,
                opt: RowOptions { detail: Some(SSH_IGNORE_UNKNOWN_DETAIL.into()), ..Default::default() },
            },
        )
        .await,
    );

    let ssh_dir = format!("{}/.ssh", host.home);
    let ssh = host.fs.list(&ssh_dir).await;
    let pubs: Vec<String> = ssh.iter().filter(|n| n.ends_with(".pub")).map(|n| format!("~/.ssh/{n}")).collect();
    if !pubs.is_empty() {
        let f = found(host, &pubs).await;
        rows.push(Some(entry(EntrySpec {
            rung: Rung::Identity,
            id: "identity/ssh-public-keys".into(),
            label: format!("SSH public keys ({})", pubs.len()),
            paths: f.paths,
            bytes: f.bytes,
            opt: RowOptions { required: Some(true), ..Default::default() },
        })));
    }
    for n in &ssh {
        if n.ends_with(".pub") || SSH_NOT_KEYS.contains(&n.as_str()) || n.contains("known_hosts") || n.ends_with(".sock") {
            continue;
        }
        let Some(s) = host.fs.stat(&format!("{ssh_dir}/{n}")).await else {
            continue;
        };
        if s.kind != StatKind::File {
            continue;
        }
        rows.push(Some(entry(EntrySpec {
            rung: Rung::Identity,
            id: format!("identity/ssh-key/{n}"),
            label: format!("~/.ssh/{n}"),
            paths: vec![format!("~/.ssh/{n}")],
            bytes: s.bytes,
            opt: RowOptions {
                default: Some(RowDefault::Skip),
                reason: Some("private key, never copied; the machine gets its own key".into()),
                ..Default::default()
            },
        })));
    }
    let known: Vec<String> = ssh.iter().filter(|n| n.contains("known_hosts")).map(|n| format!("~/.ssh/{n}")).collect();
    rows.push(
        row(
            host,
            RowSpec {
                rung: Rung::Identity,
                id: "identity/ssh-known-hosts".into(),
                label: "~/.ssh/known_hosts".into(),
                paths: known,
                always: false,
                opt: RowOptions {
                    default: Some(RowDefault::Skip),
                    reason: Some("host entries are rebuilt on first connect".into()),
                    ..Default::default()
                },
            },
        )
        .await,
    );

    if exists(host, "~/.gnupg").await {
        let f = found(host, &["~/.gnupg".to_string()]).await;
        rows.push(Some(entry(EntrySpec {
            rung: Rung::Identity,
            id: "identity/gpg".into(),
            label: "GPG keyring".into(),
            paths: f.paths,
            bytes: f.bytes,
            opt: RowOptions {
                default: Some(RowDefault::Skip),
                reason: Some("GPG keys are never copied".into()),
                ..Default::default()
            },
        })));
    }
    present(rows)
}
