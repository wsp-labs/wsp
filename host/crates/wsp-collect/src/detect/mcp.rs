// SPDX-License-Identifier: AGPL-3.0-only
//! MCP servers travel with the agent that runs them: one row per server under its agent, read through the catalog
//! entry's format module, saying what each server is, what it needs on Linux and which secret it carries, then the
//! mcp-remote sign-in store. Token files are only stat'ed, never read. Port of
//! packages/collect/src/detect/mcp.ts:292-464 at a3b870a6e; the spec is spec/mcp.md.
use indexmap::IndexMap;
use wsp_catalog::catalog::AgentEntry;
use wsp_catalog::mcp::McpServer;

pub use crate::detect::mcp_args::*;
use crate::host::Host;
use crate::manifest::ManifestEntry;

/// The row that carries mcp-remote's saved browser sign-ins for every agent: MCP_ID_PREFIX then `mcp-remote` (mcp.ts:17).
pub const MCP_REMOTE_ID: &str = "agents/mcp/mcp-remote";
// mcp.ts:18
// export const MCP_REMOTE_LABEL = "mcp-remote sign-ins";
pub const MCP_REMOTE_LABEL: &str = "mcp-remote sign-ins";

// mcp.ts:22
// const MCP_AUTH = "~/.mcp-auth";
pub const MCP_AUTH: &str = "~/.mcp-auth";
// mcp.ts:24
// /** mcp-remote's store since its versioned folders went away: MCP_REMOTE_CONFIG_DIR or ~/.mcp-auth, then mcp-remote-v1. */
// const MCP_REMOTE_STORE = `${MCP_AUTH}/mcp-remote-v1`;
pub const MCP_REMOTE_STORE: &str = "~/.mcp-auth/mcp-remote-v1";

// Private helpers the port keeps (names in snake case):
// mcp.ts:254
// /** The definition in a few words: the command and its arguments as the reading above leaves them, urls shown as
//  * host and path, and every value hidden behind a placeholder. */
// function transportLine(t: McpTransport, home: string): string {
//   return t.kind === "http" ? `http: ${shownUrl(t.url)}` : `stdio: ${stdioLine(t, home)}`;
// }
// mcp.ts:265
// interface Carried {
//   secrets: string[];
//   /** What each of the secrets goes by, for the question that asks to copy them. */
//   keys: string[];
//   notes: string[];
//   paths: string[];
//   bytes: number;
//   signIn: ServerSignIn;
// }
// mcp.ts:276
// /** A secret the definition carries under a name that says key is a key; any other is a token. */
// const keyOrToken = (names: readonly string[]): ServerSignIn => (names.some(n => /key/i.test(n)) ? "key" : "token");
// mcp.ts:292
// interface HomeDeps {
//   notes: string[];
//   /** A path the definition runs against is not here: the row starts unticked, and the person decides. */
//   gone: boolean;
// }
// mcp.ts:301
// /** Whether each home path a definition runs against is here: one that is here and travels on no row of this
//  * server is named as a dependency; one that is not here unticks the row but never locks it, since a server that
//  * makes its own file (a memory store, a sqlite db) runs fine without it. */
// async function homeDeps(host: Host, server: McpServer, carriedPaths: readonly string[]): Promise<HomeDeps> {
//   const out: HomeDeps = { notes: [], gone: false };
//   for (const p of homePaths(server, host.home)) {
//     if (carriedPaths.includes(p)) continue;
//     if ((await host.fs.stat(expand(host, p))) === undefined) {
//       out.gone = true;
//       out.notes.push(`${p} is not on this computer; unticked, tick it if the server creates it on first start`);
//     } else out.notes.push(`depends on ${p}, which comes along only if a row carries it`);
//   }
//   return out;
// }
// mcp.ts:315
// /** What a definition carries: secret-named env values by size, the file such a value points at (which travels on
//  * the row), header values, and where its mcp-remote sign-in or, as the agent's entry says, its http sign-in lives. Nothing is read. */
// async function carried(host: Host, server: McpServer, agent: McpAgent, remoteTokens: ReadonlyMap<string, number>): Promise<Carried> {
//   const out: Carried = { secrets: [], keys: [], notes: [], paths: [], bytes: 0, signIn: "none" };
//   const t = server.transport;
//   // The names the server reads its secret under, whether the definition carries the value or the machine sets it.
//   const named = [...server.envRefs];
//   if (t.kind === "http") {
//     for (const [k, v] of Object.entries(t.headers)) {
//       out.secrets.push(`header ${k} (${Buffer.byteLength(v)} B)`);
//       out.keys.push(k);
//     }
//     if (agent.mcp.httpAuth !== undefined) out.notes.push(agent.mcp.httpAuth);
//     const headers = Object.keys(t.headers);
//     out.signIn = headers.length > 0 || named.length > 0 ? keyOrToken([...headers, ...named]) : agent.mcp.httpAuth !== undefined ? "oauth" : "none";
//     return out;
//   }
//   for (const [k, v] of Object.entries(t.env)) {
//     if (!secretNamed(k)) continue;
//     named.push(k);
//     const abs = v.startsWith("~/") ? expand(host, v) : v;
//     const st = abs.startsWith("/") ? await host.fs.stat(abs) : undefined;
//     if (st?.kind === "file") {
//       if (!abs.startsWith(`${host.home}/`)) {
//         out.notes.push(`the file ${k} points at is outside your home and is not copied`);
//         continue;
//       }
//       out.secrets.push(`the file ${k} points at (${fmtBytes(st.bytes)})`);
//       out.keys.push(k);
//       out.paths.push(`~${abs.slice(host.home.length)}`);
//       out.bytes += st.bytes;
//       continue;
//     }
//     out.secrets.push(`env ${k} (${Buffer.byteLength(v)} B)`);
//     out.keys.push(k);
//   }
//   const read = readArgs(t, host.home).filter(r => r.secret !== undefined);
//   const args = read.map(r => r.secret!);
//   out.secrets.push(...args);
//   out.keys.push(...read.flatMap(r => (r.key !== undefined ? [r.key] : [])));
//   const hash = mcpRemoteHash(t.args);
//   if (hash !== undefined) {
//     const bytes = remoteTokens.get(hash);
//     out.notes.push(bytes === undefined ? "no saved sign-in; the browser sign-in runs again on the machine" : `its saved sign-in (${fmtBytes(bytes)}) travels on the ${MCP_REMOTE_LABEL} row`);
//   }
//   // mcp-remote's own browser sign-in is the server's, whatever its url reads as among the arguments.
//   out.signIn = hash !== undefined ? "oauth" : named.length > 0 ? keyOrToken(named) : args.length > 0 ? "token" : "none";
//   return out;
// }
// mcp.ts:363
// const mcpGroup = (agent: McpAgent): string => `${agent.name} MCP servers`;
// mcp.ts:365
// function serverRow(agent: McpAgent, server: McpServer, fit: LinuxFit, deps: HomeDeps, c: Carried, home: string): ManifestEntry {
//   const id = `${MCP_ID_PREFIX}${agent.id}/${server.scope === "home" ? "home/" : ""}${server.name}`;
//   const reason = fit.ok ? undefined : fit.reason;
//   const words = [
//     ...(server.scope === "home" ? ["local to ~"] : []),
//     transportLine(server.transport, home),
//     ...(fit.ok ? [fit.needs, ...deps.notes] : []),
//     ...server.envRefs.map(n => `reads ${n} from the environment, set it on the machine`),
//     ...(c.secrets.length > 0 ? [`carries ${c.secrets.length === 1 ? "a secret" : "secrets"}: ${c.secrets.join(", ")}`] : c.notes.length === 0 ? ["carries no secret"] : []),
//     ...c.notes,
//   ];
//   return {
//     rung: "agents",
//     id,
//     label: server.name,
//     group: mcpGroup(agent),
//     paths: c.paths,
//     bytes: c.bytes,
//     default: reason === undefined && !deps.gone ? "bring" : "skip",
//     ...(reason !== undefined ? { reason } : {}),
//     // A server that carries a secret travels only on a copy answer, never on a bare tick.
//     ...(c.secrets.length > 0 ? { consent: true, keys: [...new Set(c.keys)] } : {}),
//     detail: words.join("; "),
//     signIn: c.signIn,
//   };
// }
// mcp.ts:392
// interface RemoteStore {
//   /** Token file size by server hash. */
//   tokens: Map<string, number>;
//   /** Every file that travels: tokens, client registrations and the verifiers beside them. */
//   bytes: number;
//   excludes: string[];
// }
// mcp.ts:402
// /** The store as it is on disk, by listing: token sizes per hash, older bridge versions' folders and lock files
//  * set aside. Undefined when there is no store. */
// async function remoteStore(host: Host): Promise<RemoteStore | undefined> {
//   if ((await host.fs.stat(expand(host, MCP_AUTH)))?.kind !== "dir") return undefined;
//   const out: RemoteStore = { tokens: new Map(), bytes: 0, excludes: [] };
//   for (const name of await host.fs.list(expand(host, MCP_AUTH))) if (/^mcp-remote-\d/.test(name)) out.excludes.push(`${MCP_AUTH}/${name}`);
//   for (const name of await host.fs.list(expand(host, MCP_REMOTE_STORE))) {
//     if (/^[0-9a-f]{32}_lock\.json$/.test(name)) {
//       out.excludes.push(`${MCP_REMOTE_STORE}/${name}`);
//       continue;
//     }
//     const bytes = (await host.fs.stat(expand(host, `${MCP_REMOTE_STORE}/${name}`)))?.bytes ?? 0;
//     out.bytes += bytes;
//     const token = /^([0-9a-f]{32})_tokens\.json$/.exec(name);
//     if (token !== null) out.tokens.set(token[1]!, bytes);
//   }
//   return out;
// }
// mcp.ts:419
// function remoteRow(store: RemoteStore, matched: readonly string[]): ManifestEntry {
//   const n = store.tokens.size;
//   const tokenBytes = [...store.tokens.values()].reduce((a, b) => a + b, 0);
//   const older = store.excludes.some(e => !e.startsWith(`${MCP_REMOTE_STORE}/`));
//   const base = { rung: "agents" as const, id: MCP_REMOTE_ID, label: MCP_REMOTE_LABEL, group: "MCP sign-ins", paths: [MCP_AUTH], ...(store.excludes.length > 0 ? { excludes: store.excludes } : {}), bytes: store.bytes };
//   if (n === 0) return { ...base, default: "skip", reason: `no saved sign-in the current bridge reads${older ? "; older versions' folders are left here" : ""}` };
//   const elsewhere = n - matched.length;
//   const whom = [...(matched.length > 0 ? [`for ${matched.join(", ")}`] : []), ...(elsewhere > 0 ? [`${elsewhere} for server${elsewhere === 1 ? "" : "s"} configured elsewhere (a repo's .mcp.json)`] : [])];
//   return {
//     ...base,
//     default: "bring",
//     consent: true,
//     detail: `browser sign-ins saved by mcp-remote for remote servers: ${n} token${n === 1 ? "" : "s"} (${fmtBytes(tokenBytes)})${whom.length > 0 ? `, ${whom.join("; ")}` : ""}${older ? "; older bridge versions' folders stay here" : ""}`,
//   };
// }

// mcp.ts:116
// /** The file mcp-remote keeps one store key's sign-in in on a computer, as its getConfigDir names the folder: under
//  * the MCP_REMOTE_CONFIG_DIR its environment sets, else ~/.mcp-auth. */
// export const mcpRemoteTokenFile = (host: Host, hash: string, env: Readonly<Record<string, string>>): string =>
//   `${env["MCP_REMOTE_CONFIG_DIR"] ? `${env["MCP_REMOTE_CONFIG_DIR"]}/mcp-remote-v1` : expand(host, MCP_REMOTE_STORE)}/${hash}_tokens.json`;
/// An empty MCP_REMOTE_CONFIG_DIR counts as unset, as the TS truthiness check reads it.
pub fn mcp_remote_token_file(host: &Host, hash: &str, env: &IndexMap<String, String>) -> String {
    todo!()
}

// mcp.ts:436
// /** One server's row as the scan reads it: how it fits Linux, what it carries and the home paths it runs against. */
// export async function mcpServerRow(host: Host, agent: McpAgent, server: McpServer, tokens: ReadonlyMap<string, number>): Promise<ManifestEntry> {
//   const c = await carried(host, server, agent, tokens);
//   return serverRow(agent, server, linuxFit(server, host.home), await homeDeps(host, server, c.paths), c, host.home);
// }
/// `agent` is a catalog entry whose `mcp` is set.
pub async fn mcp_server_row(host: &Host, agent: &AgentEntry, server: &McpServer, tokens: &IndexMap<String, u64>) -> ManifestEntry {
    todo!()
}

// mcp.ts:442
// /** The token sizes mcp-remote keeps by server hash, for the rows of servers read apart from the scan. */
// export const mcpRemoteTokens = async (host: Host): Promise<ReadonlyMap<string, number>> => (await remoteStore(host))?.tokens ?? new Map();
/// Token file bytes by server hash, in the store's listing order; empty when there is no store.
pub async fn mcp_remote_tokens(host: &Host) -> IndexMap<String, u64> {
    todo!()
}

// mcp.ts:446
// /** One row per MCP server under its agent, then the mcp-remote sign-in store when there is one. Each agent's
//  * config is read by its entry's format module, so an agent the catalog gains needs nothing here. */
// export async function detectMcp(host: Host, agents: readonly McpAgent[] = MCP_AGENTS): Promise<ManifestEntry[]> {
//   const store = await remoteStore(host);
//   const tokens = store?.tokens ?? new Map<string, number>();
//   const rows: ManifestEntry[] = [];
//   const matched: string[] = [];
//   for (const agent of agents) {
//     // What a turn at home gets from the agent's own file: its user scope, then the home folder's own entry.
//     const turns = await readTurnServers(host, agent, host.home, { own: true });
//     for (const turn of [...turns.filter(t => t.scope === "user"), ...turns.filter(t => t.scope === "local")]) {
//       const server: McpServer = { ...turn.server, scope: turn.scope === "local" ? "home" : "user" };
//       const hash = server.transport.kind === "stdio" ? mcpRemoteHash(server.transport.args) : undefined;
//       if (hash !== undefined && tokens.has(hash)) matched.push(server.name);
//       rows.push(await mcpServerRow(host, agent, server, tokens));
//     }
//   }
//   if (store !== undefined) rows.push(remoteRow(store, matched));
//   return rows;
// }
/// `agents` `None` is the catalog's MCP agents. Each agent's own file is read through `read_turn_servers` with
/// `own: true` and the home as the folder, its `TurnAgent` built from the entry's id and `mcp` format and files.
pub async fn detect_mcp(host: &Host, agents: Option<&[&AgentEntry]>) -> Vec<ManifestEntry> {
    todo!()
}
