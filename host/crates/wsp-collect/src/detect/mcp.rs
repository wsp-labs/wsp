// SPDX-License-Identifier: AGPL-3.0-only
//! MCP servers travel with the agent that runs them: one row per server under its agent, read through the catalog
//! entry's format module, saying what each server is, what it needs on Linux and which secret it carries, then the
//! mcp-remote sign-in store. Token files are only stat'ed, never read. Port of
//! packages/collect/src/detect/mcp.ts:292-464 at a3b870a6e; the spec is spec/mcp.md.
use indexmap::IndexMap;
use wsp_catalog::catalog::{mcp_agents, AgentEntry};
use wsp_catalog::mcp::{McpServer, McpServerScope, McpTransport};
use wsp_catalog::mcp_launch::TurnScope;
use wsp_wire::types::recipe_file::ServerSignIn;
use wsp_wire::views::golden_image::MCP_ID_PREFIX;
use wsp_words::words::units::fmt_bytes;

pub use crate::detect::mcp_args::*;
use crate::detect::turn_servers::{read_turn_servers, TurnRead};
use crate::host::{expand, Host, StatKind};
use crate::manifest::{ManifestEntry, RowDefault, Rung};

/// The row that carries mcp-remote's saved browser sign-ins for every agent: MCP_ID_PREFIX then `mcp-remote` (mcp.ts:17).
pub const MCP_REMOTE_ID: &str = "agents/mcp/mcp-remote";
pub const MCP_REMOTE_LABEL: &str = "mcp-remote sign-ins";

pub const MCP_AUTH: &str = "~/.mcp-auth";
/// mcp-remote's store since its versioned folders went away: MCP_REMOTE_CONFIG_DIR or ~/.mcp-auth, then mcp-remote-v1.
pub const MCP_REMOTE_STORE: &str = "~/.mcp-auth/mcp-remote-v1";

/// What a definition carries, as the row's count and its sign-in read it.
struct Carried {
    secrets: Vec<String>,
    /// What each of the secrets goes by, for the question that asks to copy them.
    keys: Vec<String>,
    notes: Vec<String>,
    paths: Vec<String>,
    bytes: u64,
    sign_in: ServerSignIn,
}

/// Whether each home path a definition runs against is here, and whether one is not.
struct HomeDeps {
    notes: Vec<String>,
    /// A path the definition runs against is not here: the row starts unticked, and the person decides.
    gone: bool,
}

/// The store as it is on disk, by listing: token sizes per hash, older bridge versions' folders and lock files set aside.
struct RemoteStore {
    /// Token file size by server hash.
    tokens: IndexMap<String, u64>,
    /// Every file that travels: tokens, client registrations and the verifiers beside them.
    bytes: u64,
    excludes: Vec<String>,
}

fn transport_line(server: &McpServer, home: &str) -> String {
    match &server.transport {
        McpTransport::Http { url, .. } => format!("http: {}", shown_url(url)),
        McpTransport::Stdio { command, args, .. } => format!("stdio: {}", stdio_line(command, args, home)),
    }
}

/// A secret the definition carries under a name that says key is a key; any other is a token.
fn key_or_token(names: &[String]) -> ServerSignIn {
    if names.iter().any(|n| n.to_ascii_lowercase().contains("key")) {
        ServerSignIn::Key
    } else {
        ServerSignIn::Token
    }
}

/// Home paths a definition runs against that are not among the carried ones: each one is here or is not.
async fn home_deps(host: &Host, server: &McpServer, carried_paths: &[String]) -> HomeDeps {
    let mut out = HomeDeps { notes: Vec::new(), gone: false };
    for p in home_paths(server, &host.home) {
        if carried_paths.contains(&p) {
            continue;
        }
        if host.fs.stat(&expand(&host.home, &p)).await.is_none() {
            out.gone = true;
            out.notes.push(format!("{p} is not on this computer; unticked, tick it if the server creates it on first start"));
        } else {
            out.notes.push(format!("depends on {p}, which comes along only if a row carries it"));
        }
    }
    out
}

/// What a definition carries: secret-named env values by size, the file such a value points at (which travels on
/// the row), header values, and where its mcp-remote sign-in or, as the agent's entry says, its http sign-in lives. Nothing is read.
async fn carried(host: &Host, server: &McpServer, agent: &AgentEntry, remote_tokens: &IndexMap<String, u64>) -> Carried {
    let mut out =
        Carried { secrets: Vec::new(), keys: Vec::new(), notes: Vec::new(), paths: Vec::new(), bytes: 0, sign_in: ServerSignIn::None };
    let http_auth = agent.mcp.as_ref().and_then(|m| m.http_auth.clone());
    let mut named: Vec<String> = server.env_refs.clone();
    match &server.transport {
        McpTransport::Http { headers, .. } => {
            for (k, v) in headers {
                out.secrets.push(format!("header {k} ({} B)", v.len()));
                out.keys.push(k.clone());
            }
            if let Some(note) = &http_auth {
                out.notes.push(note.clone());
            }
            let header_names: Vec<String> = headers.keys().cloned().collect();
            out.sign_in = if !header_names.is_empty() || !named.is_empty() {
                key_or_token(&[header_names, named].concat())
            } else if http_auth.is_some() {
                ServerSignIn::Oauth
            } else {
                ServerSignIn::None
            };
            out
        }
        McpTransport::Stdio { command, args, env, .. } => {
            for (k, v) in env {
                if !secret_named(k) {
                    continue;
                }
                named.push(k.clone());
                let abs = if v.starts_with("~/") { expand(&host.home, v) } else { v.clone() };
                let st = if abs.starts_with('/') { host.fs.stat(&abs).await } else { None };
                if let Some(st) = st.filter(|st| st.kind == StatKind::File) {
                    if !abs.starts_with(&format!("{}/", host.home)) {
                        out.notes.push(format!("the file {k} points at is outside your home and is not copied"));
                        continue;
                    }
                    out.secrets.push(format!("the file {k} points at ({})", fmt_bytes(st.bytes as f64)));
                    out.keys.push(k.clone());
                    out.paths.push(format!("~{}", &abs[host.home.len()..]));
                    out.bytes += st.bytes;
                    continue;
                }
                out.secrets.push(format!("env {k} ({} B)", v.len()));
                out.keys.push(k.clone());
            }
            let read: Vec<ArgRead> = read_args(command, args, &host.home).into_iter().filter(|r| r.secret.is_some()).collect();
            out.secrets.extend(read.iter().filter_map(|r| r.secret.clone()));
            out.keys.extend(read.iter().filter_map(|r| r.key.clone()));
            let hash = mcp_remote_hash(args);
            if let Some(hash) = &hash {
                out.notes.push(match remote_tokens.get(hash) {
                    None => "no saved sign-in; the browser sign-in runs again on the machine".to_string(),
                    Some(bytes) => format!("its saved sign-in ({}) travels on the {MCP_REMOTE_LABEL} row", fmt_bytes(*bytes as f64)),
                });
            }
            out.sign_in = if hash.is_some() {
                ServerSignIn::Oauth
            } else if !named.is_empty() {
                key_or_token(&named)
            } else if !read.is_empty() {
                ServerSignIn::Token
            } else {
                ServerSignIn::None
            };
            out
        }
    }
}

/// The group a server's row sits under.
fn mcp_group(agent: &AgentEntry) -> String {
    format!("{} MCP servers", agent.name)
}

/// One server's row: its id, what it needs and carries, and the notes that say so.
fn server_row(agent: &AgentEntry, server: &McpServer, fit: LinuxFit, deps: HomeDeps, c: Carried, home: &str) -> ManifestEntry {
    let id = format!("{MCP_ID_PREFIX}{}/{}{}", agent.id, if server.scope == McpServerScope::Home { "home/" } else { "" }, server.name);
    let reason = match &fit {
        LinuxFit::No { reason } => Some(reason.clone()),
        LinuxFit::Ok { .. } => None,
    };
    let mut words: Vec<String> = Vec::new();
    if server.scope == McpServerScope::Home {
        words.push("local to ~".to_string());
    }
    words.push(transport_line(server, home));
    if let LinuxFit::Ok { needs } = &fit {
        words.push(needs.clone());
        words.extend(deps.notes);
    }
    words.extend(server.env_refs.iter().map(|n| format!("reads {n} from the environment, set it on the machine")));
    if !c.secrets.is_empty() {
        let label = if c.secrets.len() == 1 { "a secret" } else { "secrets" };
        words.push(format!("carries {label}: {}", c.secrets.join(", ")));
    } else if c.notes.is_empty() {
        words.push("carries no secret".to_string());
    }
    words.extend(c.notes.iter().cloned());
    let mut keys: Vec<String> = Vec::new();
    for k in &c.keys {
        if !keys.contains(k) {
            keys.push(k.clone());
        }
    }
    let carries = !c.secrets.is_empty();
    let mut row = ManifestEntry::new(Rung::Agents, id, server.name.clone(), c.paths, c.bytes);
    row.group = Some(mcp_group(agent));
    row.default = if reason.is_none() && !deps.gone { RowDefault::Bring } else { RowDefault::Skip };
    row.reason = reason;
    if carries {
        row.consent = Some(true);
        row.keys = Some(keys);
    }
    row.detail = Some(words.join("; "));
    row.sign_in = Some(c.sign_in);
    row
}

/// The mcp-remote store as it is on disk, by listing. Undefined when there is no store.
async fn remote_store(host: &Host) -> Option<RemoteStore> {
    if host.fs.stat(&expand(&host.home, MCP_AUTH)).await?.kind != StatKind::Dir {
        return None;
    }
    let mut out = RemoteStore { tokens: IndexMap::new(), bytes: 0, excludes: Vec::new() };
    for name in host.fs.list(&expand(&host.home, MCP_AUTH)).await {
        if is_remote_folder(&name) {
            out.excludes.push(format!("{MCP_AUTH}/{name}"));
        }
    }
    for name in host.fs.list(&expand(&host.home, MCP_REMOTE_STORE)).await {
        if is_lock_file(&name) {
            out.excludes.push(format!("{MCP_REMOTE_STORE}/{name}"));
            continue;
        }
        let bytes = host.fs.stat(&expand(&host.home, &format!("{MCP_REMOTE_STORE}/{name}"))).await.map_or(0, |s| s.bytes);
        out.bytes += bytes;
        if let Some(hash) = token_hash(&name) {
            out.tokens.insert(hash, bytes);
        }
    }
    Some(out)
}

/// `^mcp-remote-\d`: an older bridge version's folder under ~/.mcp-auth.
fn is_remote_folder(name: &str) -> bool {
    name.strip_prefix("mcp-remote-").is_some_and(|rest| rest.starts_with(|c: char| c.is_ascii_digit()))
}

/// `^[0-9a-f]{32}_lock\.json$`.
fn is_lock_file(name: &str) -> bool {
    hash_before(name, "_lock.json")
}

/// The hash of a `^([0-9a-f]{32})_tokens\.json$` name.
fn token_hash(name: &str) -> Option<String> {
    hash_before(name, "_tokens.json").then(|| name[..32].to_string())
}

/// Whether a name is 32 lower-case hex digits and then the suffix.
fn hash_before(name: &str, suffix: &str) -> bool {
    name.strip_suffix(suffix).is_some_and(|hash| hash.len() == 32 && hash.chars().all(|c| c.is_ascii_digit() || matches!(c, 'a'..='f')))
}

/// The row that carries the mcp-remote sign-in store.
fn remote_row(store: &RemoteStore, matched: &[String]) -> ManifestEntry {
    let n = store.tokens.len();
    let token_bytes: u64 = store.tokens.values().sum();
    let older = store.excludes.iter().any(|e| !e.starts_with(&format!("{MCP_REMOTE_STORE}/")));
    let mut row = ManifestEntry::new(Rung::Agents, MCP_REMOTE_ID, MCP_REMOTE_LABEL, vec![MCP_AUTH.to_string()], store.bytes);
    row.group = Some("MCP sign-ins".to_string());
    if !store.excludes.is_empty() {
        row.excludes = Some(store.excludes.clone());
    }
    if n == 0 {
        row.default = RowDefault::Skip;
        row.reason = Some(format!(
            "no saved sign-in the current bridge reads{}",
            if older { "; older versions' folders are left here" } else { "" }
        ));
        return row;
    }
    let elsewhere = n as i64 - matched.len() as i64;
    let mut whom: Vec<String> = Vec::new();
    if !matched.is_empty() {
        whom.push(format!("for {}", matched.join(", ")));
    }
    if elsewhere > 0 {
        whom.push(format!("{elsewhere} for server{} configured elsewhere (a repo's .mcp.json)", if elsewhere == 1 { "" } else { "s" }));
    }
    row.default = RowDefault::Bring;
    row.consent = Some(true);
    row.detail = Some(format!(
        "browser sign-ins saved by mcp-remote for remote servers: {n} token{} ({}){}{}",
        if n == 1 { "" } else { "s" },
        fmt_bytes(token_bytes as f64),
        if whom.is_empty() { String::new() } else { format!(", {}", whom.join("; ")) },
        if older { "; older bridge versions' folders stay here" } else { "" }
    ));
    row
}

/// The file mcp-remote keeps one store key's sign-in in on a computer, as its getConfigDir names the folder: under
/// the MCP_REMOTE_CONFIG_DIR its environment sets, else ~/.mcp-auth.
/// An empty MCP_REMOTE_CONFIG_DIR counts as unset, as the TS truthiness check reads it.
pub fn mcp_remote_token_file(host: &Host, hash: &str, env: &IndexMap<String, String>) -> String {
    let dir = match env.get("MCP_REMOTE_CONFIG_DIR") {
        Some(dir) if !dir.is_empty() => format!("{dir}/mcp-remote-v1"),
        _ => expand(&host.home, MCP_REMOTE_STORE),
    };
    format!("{dir}/{hash}_tokens.json")
}

/// One server's row as the scan reads it: how it fits Linux, what it carries and the home paths it runs against.
/// `agent` is a catalog entry whose `mcp` is set.
pub async fn mcp_server_row(host: &Host, agent: &AgentEntry, server: &McpServer, tokens: &IndexMap<String, u64>) -> ManifestEntry {
    let c = carried(host, server, agent, tokens).await;
    let deps = home_deps(host, server, &c.paths).await;
    server_row(agent, server, linux_fit(server, &host.home), deps, c, &host.home)
}

/// Token file bytes by server hash, in the store's listing order; empty when there is no store.
pub async fn mcp_remote_tokens(host: &Host) -> IndexMap<String, u64> {
    remote_store(host).await.map(|store| store.tokens).unwrap_or_default()
}

/// One row per MCP server under its agent, then the mcp-remote sign-in store when there is one. Each agent's
/// config is read by its entry's format module, so an agent the catalog gains needs nothing here.
/// `agents` `None` is the catalog's MCP agents. Each agent's own file is read through `read_turn_servers` with
/// `own: true` and the home as the folder, its `TurnAgent` built from the entry's id and `mcp` format and files.
pub async fn detect_mcp(host: &Host, agents: Option<&[&AgentEntry]>) -> Vec<ManifestEntry> {
    let store = remote_store(host).await;
    let tokens: IndexMap<String, u64> = store.as_ref().map(|s| s.tokens.clone()).unwrap_or_default();
    let catalog: Vec<&AgentEntry> = match agents {
        Some(agents) => agents.to_vec(),
        None => mcp_agents(),
    };
    let mut rows: Vec<ManifestEntry> = Vec::new();
    let mut matched: Vec<String> = Vec::new();
    for agent in catalog {
        let turns = read_turn_servers(host, agent, &host.home, TurnRead { own: true, ..TurnRead::default() }).await;
        let user = turns.iter().filter(|t| t.scope == TurnScope::User);
        let local = turns.iter().filter(|t| t.scope == TurnScope::Local);
        for turn in user.chain(local) {
            let scope = if turn.scope == TurnScope::Local { McpServerScope::Home } else { McpServerScope::User };
            let server = McpServer { scope, ..turn.server.clone() };
            if let McpTransport::Stdio { args, .. } = &server.transport {
                if let Some(hash) = mcp_remote_hash(args) {
                    if tokens.contains_key(&hash) {
                        matched.push(server.name.clone());
                    }
                }
            }
            rows.push(mcp_server_row(host, agent, &server, &tokens).await);
        }
    }
    if let Some(store) = &store {
        rows.push(remote_row(store, &matched));
    }
    rows
}
