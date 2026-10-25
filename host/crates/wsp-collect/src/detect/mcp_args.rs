// SPDX-License-Identifier: AGPL-3.0-only
//! The pure half of the MCP reader: whether a server definition runs on Linux, what mcp-remote it runs and the store
//! key it keeps a sign-in under, and the one reading of a definition's arguments that both its detail line and its
//! count of secrets come from. Port of packages/collect/src/detect/mcp.ts:1-290 at a3b870a6e; the spec is
//! spec/mcp_args.md. `detect::mcp` re-exports every item here, as the TS module exports them from one file.
use indexmap::IndexMap;
use wsp_catalog::mcp::McpServer;

/// Directories whose binaries the machine finds on its own PATH by the same name: the catalog's MAC_BIN_DIRS, then
/// these three (mcp.ts:29).
pub const MCP_BIN_DIRS: [&str; 5] = ["/opt/homebrew/bin/", "/opt/homebrew/sbin/", "/usr/local/bin/", "/usr/bin/", "/bin/"];

/// A path segment long and token-like enough to be a key, as a JavaScript regex source (mcp.ts:169).
pub const KEYLIKE_SEGMENT: &str = r"^(?=.*\d)[A-Za-z0-9_=-]{24,}$";

/// Flags whose next argument is a secret whatever its name says (mcp.ts:127).
pub const HIDDEN_FLAGS: [&str; 3] = ["--header", "-H", "--static-oauth-client-info"];

/// The runners a definition names its own program after (mcp.ts:148).
pub const RUNNERS: [&str; 3] = ["npx", "uvx", "uv"];

// mcp.ts:20
// export type LinuxFit = { ok: true; needs: string } | { ok: false; reason: string };
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxFit {
    Ok { needs: String },
    No { reason: String },
}

/// What `mcp_remote_of` reads off a definition that runs mcp-remote (mcp.ts:82).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpRemote {
    pub url: String,
    /// `--header` and `-H` pairs, later ones of a name replacing the value in the first one's place.
    pub headers: IndexMap<String, String>,
    pub hash: String,
    pub client_credentials: bool,
    pub v1: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgKind {
    Flag,
    Assign,
    Positional,
}

// mcp.ts:152
// /** What one argument of a stdio definition comes to: the words the detail line shows for it, and how the count of
//  * what the row carries names it. */
// export interface ArgRead {
//   /** Nothing where the line drops the argument. */
//   show?: string;
//   /** Nothing where the argument carries no value of the person's. */
//   secret?: string;
//   /** What the value goes by where it is one: its variable, its flag or its place among the arguments. */
//   key?: string;
//   /** Where the value is: a flag's (`--token x`, `--token=x`), an assignment's (`NAME=x`), or the argument itself. */
//   kind?: "flag" | "assign" | "positional";
// }
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ArgRead {
    /// Nothing where the line drops the argument.
    pub show: Option<String>,
    /// Nothing where the argument carries no value of the person's.
    pub secret: Option<String>,
    /// What the value goes by where it is one: its variable, its flag or its place among the arguments.
    pub key: Option<String>,
    pub kind: Option<ArgKind>,
}

// Private helpers the port keeps (names in snake case):
// mcp.ts:31
// function isMacOnly(p: string, home: string): boolean {
//   const abs = p.startsWith("~/") ? `${home}${p.slice(1)}` : p;
//   return abs.startsWith(`${home}/Library/`) || MAC_ONLY.some(prefix => abs.startsWith(prefix));
// }
// mcp.ts:37
// /** `--flag=/path` carries its path after the equals sign. */
// const pathOf = (s: string): string => (/^--?[\w-]+=/.test(s) ? s.slice(s.indexOf("=") + 1) : s);
// mcp.ts:40
// /** The binary a command names: a path under a known bin directory or under ~/.local/bin is found by name on the machine. */
// function binaryOf(command: string, home: string): string {
//   const abs = command.startsWith("~/") ? `${home}${command.slice(1)}` : command;
//   if (!abs.startsWith("/")) return abs;
//   if (MCP_BIN_DIRS.some(d => abs.startsWith(d)) || abs.startsWith(`${home}/.local/bin/`)) return abs.slice(abs.lastIndexOf("/") + 1);
//   return tilde(home, abs);
// }
// mcp.ts:64
// // --- mcp-remote ------------------------------------------------------------------
//
// const isUrl = (s: string): boolean => /^https?:\/\//.test(s);
// mcp.ts:66
// const sortedJson = (o: Record<string, string>): string[] => (Object.keys(o).length > 0 ? [JSON.stringify(o, Object.keys(o).sort())] : []);
// mcp.ts:70
// /** Whether the mcp-remote a definition names keeps its sign-ins under mcp-remote-v1: unpinned, latest, and every version
//  * from 0.3.2 on. An earlier one names that folder by its own version, and a range or another tag may land either side. */
// function keepsV1(spec: string): boolean {
//   const pin = spec.split("@")[1];
//   if (pin === undefined || pin === "latest") return true;
//   const v = /^(\d+)\.(\d+)\.(\d+)$/.exec(pin)?.slice(1).map(Number);
//   return v !== undefined && (v[0]! > 0 || v[1]! > 3 || (v[1] === 3 && v[2]! >= 2));
// }
// mcp.ts:129
// /** A flag takes the next argument as its value unless that argument is itself a flag. */
// const takesValue = (next: string | undefined): next is string => next !== undefined && !next.startsWith("-");
// mcp.ts:132
// /** The flag an argument names: `--api-key`, `--token=`, `-H`; nothing where the argument is no flag. */
// const flagName = (arg: string): string | undefined => /^(--?[A-Za-z][\w-]*)(=|$)/.exec(arg)?.[1];
// mcp.ts:134
// /** The variable an `API_KEY=...` argument sets. */
// const assignName = (arg: string): string | undefined => /^([A-Za-z_]\w*)=/.exec(arg)?.[1];
// mcp.ts:138
// const shownUrl = (url: string): string => {
//   try {
//     const u = new URL(url);
//     return `${u.host}${u.pathname === "/" ? "" : u.pathname}`;
//   } catch {
//     return url;
//   }
// };
// mcp.ts:163
// const size = (v: string): string => `(${Buffer.byteLength(v)} B)`;
// mcp.ts:166
// /** An address under any scheme: `https://`, `postgres://`. */
// const ADDRESS = /^[A-Za-z][A-Za-z0-9+.-]*:\/\//;
// mcp.ts:173
// /** An address that carries nothing of the person's: no login, no query and no path segment shaped like a key. One that
//  * carries any of them is a value, since asking costs a question and not asking can carry a key. */
// const plainAddress = (s: string): boolean => {
//   try {
//     const u = new URL(s);
//     return u.username === "" && u.password === "" && u.search === "" && !u.pathname.split("/").some(seg => KEYLIKE_SEGMENT.test(seg));
//   } catch {
//     return false;
//   }
// };
// mcp.ts:185
// /** An `API_KEY=value` argument wherever it stands, a bare one or a flag's value: under a secret's name, or holding an
//  * address that is not plain, the name is shown and the value hidden and counted, so a variable set behind `-e` reads
//  * as the variable it is; any other is shown whole. Nothing for any other argument. */
// const assignRead = (arg: string): ArgRead | undefined => {
//   const name = assignName(arg);
//   if (name === undefined) return undefined;
//   const value = arg.slice(arg.indexOf("=") + 1);
//   return secretNamed(name) || (ADDRESS.test(value) && !plainAddress(value)) ? { show: `${name}=…`, secret: `arg ${name} ${size(value)}`, key: name, kind: "assign" } : { show: arg };
// };

// mcp.ts:49
// /** Whether a definition can run on the machine and what it needs there. A path under ~/Library or a macOS
//  * install location has no Linux equivalent; home paths and Homebrew's prefix are rewritten on the machine. */
// export function linuxFit(server: McpServer, home: string): LinuxFit {
//   const t = server.transport;
//   if (t.kind === "http") return { ok: true, needs: "nothing to install" };
//   if (isMacOnly(t.command, home)) return { ok: false, reason: `command ${tilde(home, t.command)} is macOS-only, will not run` };
//   for (const s of [...t.args.map(pathOf), ...(t.cwd !== undefined ? [t.cwd] : []), ...Object.values(t.env)]) {
//     if (isMacOnly(s, home)) return { ok: false, reason: `path ${tilde(home, s)} is macOS-only, will not run` };
//   }
//   const bin = binaryOf(t.command, home);
//   if (bin === "npx") return { ok: true, needs: "runs via npx" };
//   if (bin === "uvx" || bin === "uv") return { ok: true, needs: "needs uv, installed on the machine when missing" };
//   return { ok: true, needs: `needs ${bin} on the machine` };
// }
pub fn linux_fit(server: &McpServer, home: &str) -> LinuxFit {
    todo!()
}

// mcp.ts:82
// /** What a definition that runs mcp-remote hands it: the server's url, its `--header` pairs, the store key mcp-remote
//  * derives as its getServerUrlHash does (md5 of the url, the `--resource`, the sorted `--authorize-param` pairs, the
//  * sorted `--header` pairs and the `--client-metadata-url`, joined with `|`), whether `--client-credentials` has it
//  * get its own token, where no person signs in, and whether its version keeps sign-ins under mcp-remote-v1. Undefined
//  * when the definition does not run mcp-remote. */
// export function mcpRemoteOf(args: readonly string[]): { url: string; headers: Record<string, string>; hash: string; clientCredentials: boolean; v1: boolean } | undefined {
//   const at = args.findIndex(a => /^mcp-remote(@[^/]*)?$/.test(a));
//   if (at < 0) return undefined;
//   const rest = args.slice(at + 1);
//   const url = rest.find(isUrl);
//   if (url === undefined) return undefined;
//   const headers: Record<string, string> = {};
//   const params: Record<string, string> = {};
//   const after = (flag: string): string | undefined => {
//     const i = rest.indexOf(flag);
//     return i >= 0 ? rest[i + 1]?.trim() : undefined;
//   };
//   for (let i = 0; i < rest.length; i++) {
//     const value = rest[i + 1];
//     if (value === undefined) continue;
//     if (rest[i] === "--header" || rest[i] === "-H") {
//       const colon = value.indexOf(":");
//       if (colon > 0) headers[value.slice(0, colon).trim()] = value.slice(colon + 1).trim();
//     } else if (rest[i] === "--authorize-param") {
//       const eq = value.indexOf("=");
//       if (eq > 0) params[value.slice(0, eq).trim()] = value.slice(eq + 1).trim();
//     }
//   }
//   const resource = after("--resource");
//   const metadata = after("--client-metadata-url");
//   const parts = [url, ...(resource !== undefined ? [resource] : []), ...sortedJson(params), ...sortedJson(headers), ...(metadata !== undefined ? [metadata] : [])];
//   return { url, headers, hash: createHash("md5").update(parts.join("|")).digest("hex"), clientCredentials: rest.includes("--client-credentials"), v1: keepsV1(args[at]!) };
// }
pub fn mcp_remote_of(args: &[String]) -> Option<McpRemote> {
    todo!()
}

// mcp.ts:112
// /** The store key mcp-remote derives for a definition that runs it; undefined when the definition does not run it. */
// export const mcpRemoteHash = (args: readonly string[]): string | undefined => mcpRemoteOf(args)?.hash;
pub fn mcp_remote_hash(args: &[String]) -> Option<String> {
    todo!()
}

// mcp.ts:122
// // --- rows ----------------------------------------------------------------------------
//
// /** isSecretName's words plus the ones a file-valued variable tends to carry. */
// export function secretNamed(name: string): boolean {
//   return isSecretName(name) || name.toUpperCase().split("_").some(w => ["CREDENTIAL", "CREDENTIALS", "AUTH", "CERT"].includes(w));
// }
pub fn secret_named(name: &str) -> bool {
    todo!()
}

// mcp.ts:136
// /** A flag whose value is never shown: one that hides it whatever its name says, or a secret-shaped name. */
// export const hidesValue = (flag: string): boolean => HIDDEN_FLAGS.has(flag) || secretNamed(flag.replace(/-/g, "_"));
pub fn hides_value(flag: &str) -> bool {
    todo!()
}

// mcp.ts:193
// /** A positional argument that is the definition's own word and no value of the person's: a path, or a plain address. */
// export const plainArg = (arg: string): boolean => arg.startsWith("/") || arg.startsWith("~/") || (ADDRESS.test(arg) && plainAddress(arg));
pub fn plain_arg(arg: &str) -> bool {
    todo!()
}

// mcp.ts:200
// /** The one reading of a definition's arguments, for the line a person reads and for what the row carries alike.
//  * npx's own yes switch is dropped and takes no value. A dashed flag's next argument is that flag's value, read
//  * like any other argument and shown unless the flag is secret-named or hides its value whatever its name says.
//  * `--` ends the flags. A path or a plain address past the program is shown; every other argument there is a value
//  * this definition does not name, so it is hidden and counted, and the row takes the copy answer. */
// export function readArgs(t: { command: string; args: readonly string[] }, home: string): ArgRead[] {
//   const out: ArgRead[] = [];
//   const shown = (a: string): string => (isUrl(a) ? `(${shownUrl(a)})` : tilde(home, a));
//   let program = RUNNERS.has(binaryOf(t.command, home));
//   let flags = true;
//   for (let i = 0; i < t.args.length; i++) {
//     const a = t.args[i]!;
//     if (flags && (a === "-y" || a === "--yes")) {
//       out.push({});
//       continue;
//     }
//     if (flags && a === "--") {
//       flags = false;
//       out.push({ show: a });
//       continue;
//     }
//     if (flags && a.startsWith("-")) {
//       const flag = flagName(a);
//       const hides = flag !== undefined && hidesValue(flag);
//       if (a.includes("=")) {
//         const value = a.slice(a.indexOf("=") + 1);
//         if (hides) {
//           out.push({ show: `${flag!}=…`, secret: `flag ${flag!} ${size(value)}`, key: flag!, kind: "flag" });
//           continue;
//         }
//         // `--env=NAME=value` is the spaced spelling written short: its value is read the same way.
//         const assigned = assignRead(value);
//         out.push(assigned === undefined ? { show: shown(a) } : { ...assigned, show: `${a.slice(0, a.indexOf("=") + 1)}${assigned.show!}` });
//         continue;
//       }
//       out.push({ show: a });
//       const value = t.args[i + 1];
//       if (!takesValue(value)) continue;
//       i++;
//       out.push(hides ? { show: "…", secret: `flag ${a} ${size(value)}`, key: a, kind: "flag" } : (assignRead(value) ?? { show: shown(value) }));
//       continue;
//     }
//     const assigned = assignRead(a);
//     if (assigned !== undefined) {
//       out.push(assigned);
//       continue;
//     }
//     if (program || plainArg(a)) {
//       program = false;
//       out.push({ show: shown(a) });
//       continue;
//     }
//     out.push({ show: "…", secret: `arg ${i + 1} ${size(a)}`, key: `argument ${i + 1}`, kind: "positional" });
//   }
//   return out;
// }
/// One read per argument the loop visits, in order: a flag's value read with its flag adds a second entry.
pub fn read_args(command: &str, args: &[String], home: &str) -> Vec<ArgRead> {
    todo!()
}

// mcp.ts:259
// /** A stdio definition's command and its first arguments as a person reads them, every value of theirs hidden. */
// export function stdioLine(t: Extract<McpTransport, { kind: "stdio" }>, home: string): string {
//   const words = readArgs(t, home).flatMap(r => (r.show === undefined ? [] : [r.show]));
//   const cut = words.length > 5 ? [...words.slice(0, 5), "…"] : words;
//   return [tilde(home, t.command), ...cut].join(" ");
// }
pub fn stdio_line(command: &str, args: &[String], home: &str) -> String {
    todo!()
}

// mcp.ts:280
// /** Home paths a stdio definition runs against (arguments, cwd, env values), `~`-relative, the command itself and
//  * anything under ~/.local/bin left out: what the machine needs beside the definition. */
// export function homePaths(server: McpServer, home: string): string[] {
//   const t = server.transport;
//   if (t.kind === "http") return [];
//   const out = new Set<string>();
//   for (const s of [...t.args.map(pathOf), ...(t.cwd !== undefined ? [t.cwd] : []), ...Object.values(t.env)]) {
//     const abs = s.startsWith("~/") ? `${home}${s.slice(1)}` : s;
//     if (!abs.startsWith(`${home}/`) || abs.startsWith(`${home}/.local/bin/`)) continue;
//     out.add(tilde(home, abs));
//   }
//   return [...out];
// }
/// `~`-relative, once each, in first-seen order: arguments, then cwd, then env values in their order.
pub fn home_paths(server: &McpServer, home: &str) -> Vec<String> {
    todo!()
}
