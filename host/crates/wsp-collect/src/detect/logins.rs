// SPDX-License-Identifier: AGPL-3.0-only
//! The logins rung, by presence only: a path is stat'ed, a Keychain item looked up by service without -w, and no login
//! file is read but Claude Code's credentials file, for whether it holds a login at all. Port of
//! packages/collect/src/detect/logins.ts at a3b870a6e; the spec is spec/logins.md.
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use crate::host::Host;
use crate::manifest::{ManifestEntry, RowDefault};

/// How Claude Code bills on this computer (TS `"key" | "login"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Bills {
    Key,
    Login,
}

// logins.ts:35
// /** The detail on a login that is signed in once on the computer that runs the workspaces: it is found here, and
//  * none of it is offered to copy, since no machine ever holds a copy of it. */
// export const LIVES_ON_COMPUTER_DETAIL = "it signs in once on the computer that runs your workspaces; nothing of it travels";
pub const LIVES_ON_COMPUTER_DETAIL: &str = "it signs in once on the computer that runs your workspaces; nothing of it travels";

// logins.ts:77
// export const CLAUDE_KEY_ENV = "ANTHROPIC_API_KEY";
pub const CLAUDE_KEY_ENV: &str = "ANTHROPIC_API_KEY";

// Private table and helpers the port keeps (names in snake case):
// logins.ts:15
// interface Login {
//   id: string;
//   label: string;
//   group: "CLI logins" | "Agent logins";
//   paths: Partial<Record<Platform, string[]>> & { all?: string[] };
//   /** Set where the catalog's sign-in kind is not the row's answer; the detail says why. */
//   default?: Default;
//   detail?: string;
// }
// logins.ts:37
// const LOGINS: readonly Login[] = [
//   { id: "gcloud", label: "Google Cloud login", group: "CLI logins", paths: { all: ["~/.config/gcloud/credentials.db", "~/.config/gcloud/access_tokens.db", "~/.config/gcloud/application_default_credentials.json", "~/.config/gcloud/configurations", "~/.config/gcloud/active_config", "~/.config/gcloud/legacy_credentials"] } },
//   { id: "wrangler", label: "Cloudflare Wrangler login", group: "CLI logins", paths: { darwin: ["~/Library/Preferences/.wrangler/config/default.toml"], linux: ["~/.config/.wrangler/config/default.toml"] } },
//   { id: "cloudflared", label: "cloudflared login", group: "CLI logins", paths: { all: ["~/.cloudflared/cert.pem"] } },
//   { id: "vercel", label: "Vercel login", group: "CLI logins", paths: { darwin: ["~/Library/Application Support/com.vercel.cli/auth.json"], linux: ["~/.config/com.vercel.cli/auth.json"], all: ["~/.vercel/auth.json"] } },
//   { id: "aws", label: "AWS keys and profiles", group: "CLI logins", paths: { all: ["~/.aws/credentials", "~/.aws/config"] } },
//   { id: "kube", label: "kubectl config", group: "CLI logins", paths: { all: ["~/.kube/config"] } },
//   { id: "codex", label: "Codex login", group: "Agent logins", paths: { all: ["~/.codex/auth.json"] } },
//   { id: "gemini", label: "Gemini CLI login", group: "Agent logins", paths: { all: ["~/.gemini/oauth_creds.json"] } },
//   { id: "opencode", label: "OpenCode login", group: "Agent logins", paths: { all: ["~/.local/share/opencode/auth.json"] } },
//   { id: "pi", label: "Pi login", group: "Agent logins", paths: { all: ["~/.pi/agent/auth.json"] } },
//   { id: "hermes", label: "Hermes Agent login", group: "Agent logins", paths: { all: ["~/.hermes/auth.json"] } },
// ];
// logins.ts:52
// /** The rows for key files the catalog names beside a login: one per entry whose keys are here, a copy by default. */
// async function keysRows(host: Host): Promise<ManifestEntry[]> {
//   const rows: ManifestEntry[] = [];
//   for (const r of LOGIN_ROWS) {
//     if (r.keys === undefined) continue;
//     const f = await found(host, r.keys.paths);
//     if (f.paths.length === 0) continue;
//     rows.push(entry({ rung: "logins", id: `logins/${r.id}`, label: `${r.entry.name} API keys`, group: r.entry.kind === "agent" ? "Agent logins" : "CLI logins", ...f, default: loginDefault(r.id), detail: r.keys.note }));
//   }
//   return rows;
// }
// logins.ts:63
// async function keychainHas(host: Host, service: string): Promise<boolean> {
//   if (host.platform !== "darwin") return false;
//   return (await host.exec.run("security", ["find-generic-password", "-s", service])) !== undefined;
// }
// logins.ts:70
// // hosts.yml names the account; on macOS the token sits in the Keychain, so that
// // item rides along as a second path and the copy step reads it there.
// async function ghRow(host: Host): Promise<ManifestEntry | undefined> {
//   const f = await found(host, ["~/.config/gh/hosts.yml"]);
//   if (f.paths.length === 0) return undefined;
//   const paths = (await keychainHas(host, "gh:github.com")) ? [...f.paths, "Keychain: gh:github.com"] : f.paths;
//   return entry({ rung: "logins", id: "logins/gh", label: GITHUB_LOGIN_NAME, group: "CLI logins", paths, bytes: f.bytes, default: loginDefault("gh") });
// }
// logins.ts:97
// /** The first rc file that exports the name; the pack cuts that line and the secrets step sets it on the machine. */
// async function exportedIn(host: Host, name: string): Promise<string | undefined> {
//   for (const rel of RC_PATHS) {
//     const text = await host.fs.readText(`${host.home}/${rel}`);
//     if (text !== undefined && stripExports(text).names.includes(name)) return `~/${rel}`;
//   }
//   return undefined;
// }
// logins.ts:141
// // The workspace reads the long-lived token the row's own command mints, held in the wsp home and set on every turn.
// // The row is still here, because it is where the person answers how Claude Code signs in.
// async function claudeRow(host: Host): Promise<ManifestEntry | undefined> {
//   if ((await claudeBills(host)) === undefined) return undefined;
//   return entry({
//     rung: "logins",
//     id: "logins/claude",
//     label: "Claude Code login",
//     group: "Agent logins",
//     paths: [],
//     bytes: 0,
//     default: "skip",
//     detail: CLAUDE_TOKEN_DETAIL,
//   });
// }

// logins.ts:28
// /** What a login row starts as: a copy (bring) for a key, a keys row or a tool with no sign-in, and nothing to copy
//  * (skip) for a browser or device flow the catalog names and for a tool that mints its token on this computer; a
//  * tool the catalog does not know starts as a copy. */
// export function loginDefault(id: string): Default {
//   const row = loginRow(id);
//   return row !== undefined && (signsInByDefault(row.signIn) || mintsToken(row.signIn)) ? "skip" : "bring";
// }
pub fn login_default(id: &str) -> RowDefault {
    todo!()
}

// logins.ts:81
// /** The Keychain item Claude Code keeps its login under: with CLAUDE_CONFIG_DIR set, the name carries the first eight
//  * hex of that folder's sha256 (read off 2.1.296's own code), so each config folder signs in on its own. */
// export const claudeKeychainService = (configDir?: string): string =>
//   `Claude Code-credentials${configDir === undefined ? "" : `-${createHash("sha256").update(configDir.normalize("NFC")).digest("hex").slice(0, 8)}`}`;
/// The sha256 is of the folder's text in Unicode NFC form, its first eight hex digits.
pub fn claude_keychain_service(config_dir: Option<&str>) -> String {
    todo!()
}

// logins.ts:85
// /** The apiKeyHelper command a settings.json names, when it parses and has one. */
// export function apiKeyHelperOf(text: string | undefined): string | undefined {
//   if (text === undefined) return undefined;
//   try {
//     const parsed: unknown = JSON.parse(text);
//     const helper = typeof parsed === "object" && parsed !== null ? (parsed as Record<string, unknown>)["apiKeyHelper"] : undefined;
//     return typeof helper === "string" && helper.trim() !== "" ? helper : undefined;
//   } catch {
//     return undefined;
//   }
// }
pub fn api_key_helper_of(text: Option<&str>) -> Option<String> {
    todo!()
}

// logins.ts:107
// /** What the row says instead of a path: the login on the workspace is the token the tool mints here, so whatever
//  * signed Claude Code in on this computer stays on it. */
// export const CLAUDE_TOKEN_DETAIL = `Claude Code signs in with the token ${SIGN_IN_ROWS.claude.mint} prints on this computer; nothing of its login here travels`;
/// Built from the catalog's claude sign-in row's `mint` command.
pub fn claude_token_detail() -> String {
    todo!()
}

// logins.ts:114
// // Claude Code takes its key from ANTHROPIC_API_KEY first, then the apiKeyHelper, then the OAuth credentials
// // (measured on 2.1.257), so a key found here is what it bills, ahead of a login beside it. None of them travels.
// /** How Claude Code signs in on this computer, by presence alone: `key` where the login shell exports its key or its
//  * settings name a helper for one, `login` where only its subscription login stands (the Keychain item on a Mac, the
//  * credentials file on Linux), nothing where neither does. */
// export async function claudeBills(host: Host): Promise<"key" | "login" | undefined> {
//   const helper = apiKeyHelperOf(await host.fs.readText(expand(host, CLAUDE_SETTINGS_FILE)));
//   if (helper !== undefined || (await exportedIn(host, CLAUDE_KEY_ENV)) !== undefined) return "key";
//   const oauth = host.platform === "darwin" ? await keychainHas(host, claudeKeychainService(host.claudeConfigDir)) : holdsClaudeLogin(await host.fs.readText(expand(host, `${host.claudeConfigDir ?? "~/.claude"}/.credentials.json`)));
//   return oauth ? "login" : undefined;
// }
pub async fn claude_bills(host: &Host) -> Option<Bills> {
    todo!()
}

// logins.ts:123
// /** Whether Claude Code's credentials file holds its own login: a non-empty claudeAiOauth access token. The same file
//  * keeps MCP servers' OAuth tokens, and one holding only those is no login. The value is never kept. */
// export function holdsClaudeLogin(text: string | undefined): boolean {
//   if (text === undefined) return false;
//   try {
//     const token: unknown = (JSON.parse(text) as { claudeAiOauth?: { accessToken?: unknown } } | null)?.claudeAiOauth?.accessToken;
//     return typeof token === "string" && token !== "";
//   } catch {
//     return false;
//   }
// }
pub fn holds_claude_login(text: Option<&str>) -> bool {
    todo!()
}

// logins.ts:134
// /** How each agent that reads this signs in on this computer, by agent id: the fact a box's sign-in row starts from. */
// export async function billedHere(host: Host): Promise<Record<string, "key" | "login">> {
//   const claude = await claudeBills(host);
//   return claude === undefined ? {} : { claude };
// }
pub async fn billed_here(host: &Host) -> IndexMap<String, Bills> {
    todo!()
}

// logins.ts:155
// export async function detectLogins(host: Host): Promise<ManifestEntry[]> {
//   const rows: (ManifestEntry | undefined)[] = [await ghRow(host)];
//   for (const l of LOGINS) {
//     // Codex keeps its login under CODEX_HOME where that is set.
//     const f = await found(host, l.id === "codex" && host.codexHome !== undefined ? [`${host.codexHome}/auth.json`] : [...(l.paths[host.platform] ?? []), ...(l.paths.all ?? [])]);
//     if (f.paths.length === 0) continue;
//     // A login that lives on the computer that runs the workspaces is found here and offered nowhere: it is shared
//     // into each workspace from that computer, so no path of it travels and no row can answer copy.
//     const signIn = loginRow(l.id)?.signIn;
//     const here = signIn !== undefined && livesOnComputer(signIn);
//     rows.push(
//       entry({
//         rung: "logins",
//         id: `logins/${l.id}`,
//         label: l.label,
//         group: l.group,
//         ...(here ? { paths: [], bytes: 0 } : f),
//         default: here ? "skip" : (l.default ?? loginDefault(l.id)),
//         ...(here ? { detail: LIVES_ON_COMPUTER_DETAIL } : l.detail !== undefined ? { detail: l.detail } : {}),
//       }),
//     );
//   }
//   rows.push(...(await keysRows(host)));
//   if (await host.exec.which("op")) {
//     rows.push(item({ rung: "logins", id: "logins/op", label: "1Password CLI", group: "CLI logins", default: "skip", reason: "needs the 1Password desktop app; the machine uses a service account token" }));
//   }
//   rows.push(await claudeRow(host));
//   return present(rows);
// }
pub async fn detect_logins(host: &Host) -> Vec<ManifestEntry> {
    todo!()
}
