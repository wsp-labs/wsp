// SPDX-License-Identifier: AGPL-3.0-only
//! The logins rung, by presence only: a path is stat'ed, a Keychain item looked up by service without -w, and no login
//! file is read but Claude Code's credentials file, for whether it holds a login at all. Port of
//! packages/collect/src/detect/logins.ts at a3b870a6e; the spec is spec/logins.md.
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use unicode_normalization::UnicodeNormalization;
use wsp_catalog::catalog::{login_row, login_rows, EntryKind};
use wsp_catalog::hooks::CLAUDE_SETTINGS_FILE;
use wsp_catalog::json_parse::parse_json;
use wsp_catalog::signin::{lives_on_computer, mints_token};
use wsp_catalog::signin_rows::SIGN_IN_ROWS;
use wsp_words::js::js_trim;
use wsp_words::words::base::GITHUB_LOGIN_NAME;

use crate::detect::common::{entry, found, item, present, EntrySpec, RowOptions};
use crate::detect::shell_rc::RC_PATHS;
use crate::detect::shell_rc_strip::strip_exports;
use crate::detect::tools::argv;
use crate::host::{expand, Host, Platform, RunOptions};
use crate::manifest::{ManifestEntry, RowDefault, Rung};

/// How Claude Code bills on this computer (TS `"key" | "login"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bills {
    Key,
    Login,
}

pub const LIVES_ON_COMPUTER_DETAIL: &str = "it signs in once on the computer that runs your workspaces; nothing of it travels";

pub const CLAUDE_KEY_ENV: &str = "ANTHROPIC_API_KEY";

/// One CLI or agent login the table names: its files on each platform, then the files every platform has (logins.ts:37).
struct Login {
    id: &'static str,
    label: &'static str,
    group: &'static str,
    darwin: &'static [&'static str],
    linux: &'static [&'static str],
    all: &'static [&'static str],
}

const LOGINS: [Login; 11] = [
    Login {
        id: "gcloud",
        label: "Google Cloud login",
        group: "CLI logins",
        darwin: &[],
        linux: &[],
        all: &[
            "~/.config/gcloud/credentials.db",
            "~/.config/gcloud/access_tokens.db",
            "~/.config/gcloud/application_default_credentials.json",
            "~/.config/gcloud/configurations",
            "~/.config/gcloud/active_config",
            "~/.config/gcloud/legacy_credentials",
        ],
    },
    Login {
        id: "wrangler",
        label: "Cloudflare Wrangler login",
        group: "CLI logins",
        darwin: &["~/Library/Preferences/.wrangler/config/default.toml"],
        linux: &["~/.config/.wrangler/config/default.toml"],
        all: &[],
    },
    Login {
        id: "cloudflared",
        label: "cloudflared login",
        group: "CLI logins",
        darwin: &[],
        linux: &[],
        all: &["~/.cloudflared/cert.pem"],
    },
    Login {
        id: "vercel",
        label: "Vercel login",
        group: "CLI logins",
        darwin: &["~/Library/Application Support/com.vercel.cli/auth.json"],
        linux: &["~/.config/com.vercel.cli/auth.json"],
        all: &["~/.vercel/auth.json"],
    },
    Login {
        id: "aws",
        label: "AWS keys and profiles",
        group: "CLI logins",
        darwin: &[],
        linux: &[],
        all: &["~/.aws/credentials", "~/.aws/config"],
    },
    Login { id: "kube", label: "kubectl config", group: "CLI logins", darwin: &[], linux: &[], all: &["~/.kube/config"] },
    Login { id: "codex", label: "Codex login", group: "Agent logins", darwin: &[], linux: &[], all: &["~/.codex/auth.json"] },
    Login { id: "gemini", label: "Gemini CLI login", group: "Agent logins", darwin: &[], linux: &[], all: &["~/.gemini/oauth_creds.json"] },
    Login {
        id: "opencode",
        label: "OpenCode login",
        group: "Agent logins",
        darwin: &[],
        linux: &[],
        all: &["~/.local/share/opencode/auth.json"],
    },
    Login { id: "pi", label: "Pi login", group: "Agent logins", darwin: &[], linux: &[], all: &["~/.pi/agent/auth.json"] },
    Login { id: "hermes", label: "Hermes Agent login", group: "Agent logins", darwin: &[], linux: &[], all: &["~/.hermes/auth.json"] },
];

/// What a login row starts as: a copy (bring) for a key, a keys row or a tool with no sign-in, and nothing to copy
/// (skip) for a browser or device flow the catalog names and for a tool that mints its token on this computer; a
/// tool the catalog does not know starts as a copy.
pub fn login_default(id: &str) -> RowDefault {
    match login_row(id) {
        Some(row) if wsp_catalog::signin::signs_in_by_default(&row.sign_in) || mints_token(&row.sign_in).is_some() => RowDefault::Skip,
        _ => RowDefault::Bring,
    }
}

/// The sha256 is of the folder's text in Unicode NFC form, its first eight hex digits.
pub fn claude_keychain_service(config_dir: Option<&str>) -> String {
    match config_dir {
        None => "Claude Code-credentials".to_string(),
        Some(dir) => {
            let nfc: String = dir.nfc().collect();
            let hex: String = Sha256::digest(nfc.as_bytes()).iter().take(4).map(|b| format!("{b:02x}")).collect();
            format!("Claude Code-credentials-{hex}")
        }
    }
}

/// The apiKeyHelper command a settings.json names, when it parses and has one.
pub fn api_key_helper_of(text: Option<&str>) -> Option<String> {
    let parsed: Value = parse_json(text?).ok()?;
    let helper = parsed.get("apiKeyHelper")?.as_str()?;
    (!js_trim(helper).is_empty()).then(|| helper.to_string())
}

/// Built from the catalog's claude sign-in row's `mint` command.
pub fn claude_token_detail() -> String {
    let mint = SIGN_IN_ROWS.get("claude").and_then(mints_token).map_or_else(String::new, |t| t.mint.clone());
    format!("Claude Code signs in with the token {mint} prints on this computer; nothing of its login here travels")
}

/// The first rc file that exports the name; the pack cuts that line and the secrets step sets it on the machine.
async fn exported_in(host: &Host, name: &str) -> Option<String> {
    for rel in RC_PATHS {
        if let Some(text) = host.fs.read_text(&format!("{}/{rel}", host.home)).await {
            if strip_exports(&text, None, &mut Vec::new(), false).names.iter().any(|n| n == name) {
                return Some(format!("~/{rel}"));
            }
        }
    }
    None
}

async fn keychain_has(host: &Host, service: &str) -> bool {
    if !matches!(host.platform, Platform::Darwin) {
        return false;
    }
    host.exec.run("security", &argv(&["find-generic-password", "-s", service]), RunOptions::default()).await.is_some()
}

/// Whether Claude Code's credentials file holds its own login: a non-empty claudeAiOauth access token. The same file
/// keeps MCP servers' OAuth tokens, and one holding only those is no login. The value is never kept.
pub fn holds_claude_login(text: Option<&str>) -> bool {
    let Some(parsed) = text.and_then(|t| parse_json(t).ok()) else { return false };
    parsed.get("claudeAiOauth").and_then(|o| o.get("accessToken")).and_then(Value::as_str).is_some_and(|t| !t.is_empty())
}

/// How Claude Code signs in on this computer, by presence alone: `key` where the login shell exports its key or its
/// settings name a helper for one, `login` where only its subscription login stands (the Keychain item on a Mac, the
/// credentials file on Linux), nothing where neither does.
pub async fn claude_bills(host: &Host) -> Option<Bills> {
    let settings = host.fs.read_text(&expand(&host.home, CLAUDE_SETTINGS_FILE)).await;
    if api_key_helper_of(settings.as_deref()).is_some() || exported_in(host, CLAUDE_KEY_ENV).await.is_some() {
        return Some(Bills::Key);
    }
    let oauth = if matches!(host.platform, Platform::Darwin) {
        keychain_has(host, &claude_keychain_service(host.claude_config_dir.as_deref())).await
    } else {
        let dir = host.claude_config_dir.as_deref().unwrap_or("~/.claude");
        let credentials = host.fs.read_text(&expand(&host.home, &format!("{dir}/.credentials.json"))).await;
        holds_claude_login(credentials.as_deref())
    };
    oauth.then_some(Bills::Login)
}

/// How each agent that reads this signs in on this computer, by agent id: the fact a box's sign-in row starts from.
pub async fn billed_here(host: &Host) -> IndexMap<String, Bills> {
    let mut out = IndexMap::new();
    if let Some(claude) = claude_bills(host).await {
        out.insert("claude".to_string(), claude);
    }
    out
}

async fn gh_row(host: &Host) -> Option<ManifestEntry> {
    let f = found(host, &["~/.config/gh/hosts.yml".to_string()]).await;
    if f.paths.is_empty() {
        return None;
    }
    let mut paths = f.paths;
    if keychain_has(host, "gh:github.com").await {
        paths.push("Keychain: gh:github.com".to_string());
    }
    Some(entry(EntrySpec {
        rung: Rung::Logins,
        id: "logins/gh".to_string(),
        label: GITHUB_LOGIN_NAME.to_string(),
        paths,
        bytes: f.bytes,
        opt: RowOptions { group: Some("CLI logins".to_string()), default: Some(login_default("gh")), ..RowOptions::default() },
    }))
}

async fn keys_rows(host: &Host) -> Vec<ManifestEntry> {
    let mut rows = Vec::new();
    for r in login_rows() {
        let Some(keys) = &r.keys else { continue };
        let f = found(host, &keys.paths).await;
        if f.paths.is_empty() {
            continue;
        }
        let group = if r.entry.kind() == EntryKind::Agent { "Agent logins" } else { "CLI logins" };
        rows.push(entry(EntrySpec {
            rung: Rung::Logins,
            id: format!("logins/{}", r.id),
            label: format!("{} API keys", r.entry.name()),
            paths: f.paths,
            bytes: f.bytes,
            opt: RowOptions {
                group: Some(group.to_string()),
                default: Some(login_default(&r.id)),
                detail: Some(keys.note.clone()),
                ..RowOptions::default()
            },
        }));
    }
    rows
}

async fn claude_row(host: &Host) -> Option<ManifestEntry> {
    claude_bills(host).await?;
    Some(entry(EntrySpec {
        rung: Rung::Logins,
        id: "logins/claude".to_string(),
        label: "Claude Code login".to_string(),
        paths: Vec::new(),
        bytes: 0,
        opt: RowOptions {
            group: Some("Agent logins".to_string()),
            default: Some(RowDefault::Skip),
            detail: Some(claude_token_detail()),
            ..RowOptions::default()
        },
    }))
}

pub async fn detect_logins(host: &Host) -> Vec<ManifestEntry> {
    let mut rows = vec![gh_row(host).await];
    for l in &LOGINS {
        let candidates: Vec<String> = match (l.id, host.codex_home.as_deref()) {
            ("codex", Some(codex_home)) => vec![format!("{codex_home}/auth.json")],
            _ => {
                let own = match host.platform {
                    Platform::Darwin => l.darwin,
                    Platform::Linux => l.linux,
                };
                own.iter().chain(l.all.iter()).map(|p| p.to_string()).collect()
            }
        };
        let f = found(host, &candidates).await;
        if f.paths.is_empty() {
            continue;
        }
        let here = login_row(l.id).is_some_and(|r| lives_on_computer(&r.sign_in));
        let (paths, bytes) = if here { (Vec::new(), 0) } else { (f.paths, f.bytes) };
        rows.push(Some(entry(EntrySpec {
            rung: Rung::Logins,
            id: format!("logins/{}", l.id),
            label: l.label.to_string(),
            paths,
            bytes,
            opt: RowOptions {
                group: Some(l.group.to_string()),
                default: Some(if here { RowDefault::Skip } else { login_default(l.id) }),
                detail: here.then(|| LIVES_ON_COMPUTER_DETAIL.to_string()),
                ..RowOptions::default()
            },
        })));
    }
    rows.extend(keys_rows(host).await.into_iter().map(Some));
    if host.exec.which("op").await {
        rows.push(Some(item(
            Rung::Logins,
            "logins/op",
            "1Password CLI",
            RowOptions {
                group: Some("CLI logins".to_string()),
                default: Some(RowDefault::Skip),
                reason: Some("needs the 1Password desktop app; the machine uses a service account token".to_string()),
                ..RowOptions::default()
            },
        )));
    }
    rows.push(claude_row(host).await);
    present(rows)
}
