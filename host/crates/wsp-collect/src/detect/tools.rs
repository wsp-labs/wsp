// SPDX-License-Identifier: AGPL-3.0-only
//! The tools rung: Homebrew's taps and formulae, Nix home-manager, every global package manager's packages, and Go
//! binaries. Port of packages/collect/src/detect/tools.ts at a3b870a6e; the unit is apt_tools and its spec is
//! spec/apt_tools.md.
use wsp_words::exit::HostError;

use crate::host::Host;
use crate::manifest::ManifestEntry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrewKind {
    Tap,
    Brew,
}

// tools.ts:10
// export interface BrewLine {
//   kind: "tap" | "brew";
//   name: string;
// }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrewLine {
    pub kind: BrewKind,
    pub name: String,
}

// tools.ts:36
// export interface Pkg {
//   name: string;
//   version?: string;
//   /** The folder the package sits in, where the manager's listing names one. */
//   path?: string;
// }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pkg {
    pub name: String,
    pub version: Option<String>,
    /// The folder the package sits in, where the manager's listing names one.
    pub path: Option<String>,
}

// tools.ts:124
// export interface GoModule {
//   path: string;
//   version: string;
// }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GoModule {
    pub path: String,
    pub version: String,
}

/// How npm and pnpm both list their globals (tools.ts:156).
pub const NPM_LS: [&str; 4] = ["ls", "-g", "--depth=0", "--json"];

/// Where bun keeps its global packages (tools.ts:176).
pub const BUN_GLOBALS: &str = "~/.bun/install/global/node_modules";

/// npm's own packages, never a row (tools.ts:55).
pub const NPM_OWN: [&str; 2] = ["npm", "corepack"];

// tools.ts:16
// /** The Brewfile lines that name a tap or a formula; a cask or a Mac App Store app has no Linux build to bring. */
// export function parseBrewfile(text: string): BrewLine[] {
//   const out: BrewLine[] = [];
//   for (const line of text.split("\n")) {
//     const m = /^(tap|brew)\s+"([^"]+)"/.exec(line.trim());
//     if (m === null) continue;
//     const kind = m[1];
//     const name = m[2];
//     if ((kind === "tap" || kind === "brew") && name !== undefined) out.push({ kind, name });
//   }
//   return out;
// }
pub fn parse_brewfile(text: &str) -> Vec<BrewLine> {
    todo!()
}

// tools.ts:32
// /** The taps and formulae this computer's Homebrew has, as `brew bundle dump` writes them. A plain dump also runs a
//  * lister for casks, App Store apps, editor extensions and every other manager's packages, none of which a reader here
//  * keeps; and `bundle` is a command Homebrew runs `brew update` before once its last one is a day old. Measured on a
//  * Mac with 103 formulae: 3.3 s plain, 0.7 s like this. */
// export async function brewfileHere(host: Host): Promise<BrewLine[]> {
//   return parseBrewfile((await host.exec.run("brew", ["bundle", "dump", "--file=-", "--formula", "--taps", "--no-restart"], { env: { HOMEBREW_NO_AUTO_UPDATE: "1" } })) ?? "");
// }
pub async fn brewfile_here(host: &Host) -> Vec<BrewLine> {
    todo!()
}

// tools.ts:57
// export function parseNpmGlobals(text: string): Pkg[] {
//   const data = tryJson(text);
//   if (!isRecord(data) || !isRecord(data["dependencies"])) return [];
//   const out: Pkg[] = [];
//   for (const [name, info] of Object.entries(data["dependencies"])) {
//     if (NPM_OWN.has(name)) continue;
//     const version = isRecord(info) && typeof info["version"] === "string" ? info["version"] : undefined;
//     out.push(version === undefined ? { name } : { name, version });
//   }
//   return out;
// }
pub fn parse_npm_globals(text: &str) -> Vec<Pkg> {
    todo!()
}

// tools.ts:69
// export function parsePipxList(text: string): Pkg[] {
//   const data = tryJson(text);
//   if (!isRecord(data) || !isRecord(data["venvs"])) return [];
//   const out: Pkg[] = [];
//   for (const [name, venv] of Object.entries(data["venvs"])) {
//     const meta = isRecord(venv) && isRecord(venv["metadata"]) ? venv["metadata"] : undefined;
//     const main = meta !== undefined && isRecord(meta["main_package"]) ? meta["main_package"] : undefined;
//     const version = main !== undefined && typeof main["package_version"] === "string" ? main["package_version"] : undefined;
//     out.push(version === undefined ? { name } : { name, version });
//   }
//   return out;
// }
pub fn parse_pipx_list(text: &str) -> Vec<Pkg> {
    todo!()
}

// tools.ts:83
// /** Top-level lines of `uv tool list` and `cargo install --list`: `name v1.2.3` with optional trailing decoration. */
// function parseNameVersionLines(text: string): Pkg[] {
//   const out: Pkg[] = [];
//   for (const line of text.split("\n")) {
//     if (line === "" || /^[\s-]/.test(line)) continue;
//     const m = /^(\S+)\s+v(\S+?)(?::|\s|$)/.exec(line);
//     if (m?.[1] === undefined || m[2] === undefined) continue;
//     out.push({ name: m[1], version: m[2] });
//   }
//   return out;
// }
/// `uv tool list`'s top-level lines (TS `parseUvToolList = parseNameVersionLines`).
pub fn parse_uv_tool_list(text: &str) -> Vec<Pkg> {
    todo!()
}

/// `cargo install --list`'s top-level lines (TS `parseCargoInstalls = parseNameVersionLines`, tools.ts:95).
pub fn parse_cargo_installs(text: &str) -> Vec<Pkg> {
    todo!()
}

// tools.ts:98
// /** `pnpm ls -g --json`: one object per global dir, each with a dependencies map. */
// export function parsePnpmGlobals(text: string): Pkg[] {
//   const data = tryJson(text);
//   if (!Array.isArray(data)) return [];
//   const out: Pkg[] = [];
//   for (const dir of data) {
//     if (!isRecord(dir) || !isRecord(dir["dependencies"])) continue;
//     for (const [name, info] of Object.entries(dir["dependencies"])) {
//       const version = isRecord(info) && typeof info["version"] === "string" ? info["version"] : undefined;
//       const path = isRecord(info) && typeof info["path"] === "string" ? info["path"] : undefined;
//       out.push({ name, ...(version === undefined ? {} : { version }), ...(path === undefined ? {} : { path }) });
//     }
//   }
//   return out;
// }
pub fn parse_pnpm_globals(text: &str) -> Vec<Pkg> {
    todo!()
}

// tools.ts:114
// /** `bun pm ls -g`: a header naming the global dir, then tree lines of `name@version`. */
// export function parseBunGlobals(text: string): Pkg[] {
//   const out: Pkg[] = [];
//   for (const line of text.split("\n")) {
//     const m = /^[\u2502\u251c\u2514\u2500\s]+(@?[^@\s]+)@(\S+)$/.exec(line);
//     if (m?.[1] === undefined || m[2] === undefined) continue;
//     out.push({ name: m[1], version: m[2] });
//   }
//   return out;
// }
pub fn parse_bun_globals(text: &str) -> Vec<Pkg> {
    todo!()
}

// tools.ts:130
// /** `go version -m <binary>`: the `path` line is the install target, the `mod` line carries the version. */
// export function parseGoVersionM(text: string): GoModule | undefined {
//   let path: string | undefined;
//   let version: string | undefined;
//   for (const line of text.split("\n")) {
//     const cols = line.split("\t");
//     if (cols[1] === "path" && cols[2] !== undefined) path = cols[2];
//     if (cols[1] === "mod" && cols[3] !== undefined) version = cols[3];
//   }
//   return path !== undefined && version !== undefined ? { path, version } : undefined;
// }
pub fn parse_go_version_m(text: &str) -> Option<GoModule> {
    todo!()
}

// Private helpers and tables the port keeps (names in snake case):
// tools.ts:43
// function isRecord(v: unknown): v is Record<string, unknown> {
//   return typeof v === "object" && v !== null && !Array.isArray(v);
// }
// tools.ts:47
// function tryJson(text: string): unknown {
//   try {
//     return JSON.parse(text);
//   } catch {
//     return undefined;
//   }
// }
// tools.ts:141
// const versioned = (p: Pkg, sep: string | undefined) => (p.version === undefined || sep === undefined ? p.name : `${p.name}${sep}${p.version}`);
// tools.ts:145
// /** How a manager says what it has: one listing this reader parses, or a reader of its own where one listing is not
//  * the whole story (npm's other prefixes, apt's rule over what a person chose). */
// type Listing = { args: readonly string[]; parse: (out: string) => Pkg[] } | { list: (host: Host) => Promise<Pkg[]> };
// tools.ts:147
// type GlobalManager = {
//   id: string;
//   bin: string;
//   group: string;
//   /** Between a package and its version in the row's label; a manager whose packages carry no version needs none. */
//   sep?: string;
// } & Listing;
// tools.ts:158
// /** Where another node once kept its globals; a laptop that moved to a new node still runs them from PATH. */
// const NPM_PREFIXES = [MAC_BREW, "/usr/local"];
// tools.ts:161
// /** npm's own prefix, then every other prefix on this laptop that holds globals; a name in both is the own prefix's. */
// async function npmGlobals(host: Host): Promise<Pkg[]> {
//   const own = firstLine(await host.exec.run("npm", ["prefix", "-g"]));
//   const out = new Map<string, Pkg>();
//   const add = (pkgs: Pkg[]): void => {
//     for (const p of pkgs) if (!out.has(p.name)) out.set(p.name, p);
//   };
//   add(parseNpmGlobals((await host.exec.run("npm", NPM_LS)) ?? ""));
//   for (const prefix of NPM_PREFIXES) {
//     if (prefix === own || !(await exists(host, `${prefix}/lib/node_modules`))) continue;
//     add(parseNpmGlobals((await host.exec.run("npm", [...NPM_LS, "--prefix", prefix])) ?? ""));
//   }
//   return [...out.values()];
// }
// tools.ts:204
// const GLOBALS: readonly GlobalManager[] = [
//   { id: "npm", bin: "npm", group: "npm globals", list: npmGlobals, sep: "@" },
//   { id: "pnpm", bin: "pnpm", group: "pnpm globals", args: NPM_LS, parse: parsePnpmGlobals, sep: "@" },
//   { id: "bun", bin: "bun", group: "bun globals", args: ["pm", "ls", "-g"], parse: parseBunGlobals, sep: "@" },
//   { id: "pipx", bin: "pipx", group: "pipx", args: ["list", "--json"], parse: parsePipxList, sep: " " },
//   { id: "uv", bin: "uv", group: "uv tools", args: ["tool", "list"], parse: parseUvToolList, sep: " " },
//   { id: "cargo", bin: "cargo", group: "cargo installs", args: ["install", "--list"], parse: parseCargoInstalls, sep: " " },
//   // apt-mark is what says which packages a person asked for; dpkg alone cannot tell those from the base image's own.
//   { id: "apt", bin: "apt-mark", group: "apt packages", list: aptPackages },
// ];
// tools.ts:217
// // A formula the laptop already runs on Linux needs no snapshot to vouch for it. A formula nothing vouches
// // for starts unticked without a reason, so the row stays open to a tick that tries it.
// function formulaRow(host: Host, name: string): ManifestEntry {
//   const linux = host.platform === "linux" ? "yes" : linuxSupport(name);
//   const base = { rung: "tools" as const, id: toolRowId("brew", name), label: name, group: "Homebrew", linux };
//   if (linux === "no") return item({ ...base, default: "skip", reason: "no Linux bottle" });
//   return linux === "unknown" ? item({ ...base, default: "skip" }) : item(base);
// }
// tools.ts:224
// async function brewRows(host: Host): Promise<ManifestEntry[]> {
//   if (!(await host.exec.which("brew"))) return [];
//   const lines = await brewfileHere(host);
//   return lines.map(l => (l.kind === "tap" ? item({ rung: "tools", id: `tools/brew-tap/${l.name}`, label: l.name, group: "Homebrew taps", linux: "yes" }) : formulaRow(host, l.name)));
// }
// tools.ts:230
// async function goRows(host: Host): Promise<ManifestEntry[]> {
//   if (!(await host.exec.which("go"))) return [];
//   const bin = `${host.home}/go/bin`;
//   const rows: ManifestEntry[] = [];
//   for (const name of await host.fs.list(bin)) {
//     const mod = parseGoVersionM((await host.exec.run("go", ["version", "-m", `${bin}/${name}`])) ?? "");
//     rows.push(mod === undefined
//       ? item({ rung: "tools", id: toolRowId("go", name), label: `${name} (no module info)`, group: "Go binaries", default: "skip", linux: "yes" })
//       // The module path rides in paths so the detail pane shows it first; bytes 0 says there is nothing to upload.
//       : entry({ rung: "tools", id: toolRowId("go", name), label: name, group: "Go binaries", paths: [`${mod.path}@${mod.version}`], bytes: 0, linux: "yes", version: mod.version }));
//   }
//   return rows;
// }
// tools.ts:244
// async function nixRows(host: Host): Promise<(ManifestEntry | undefined)[]> {
//   if (!(await exists(host, "~/.config/home-manager")) && !(await exists(host, "~/.config/nixpkgs/home.nix"))) return [];
//   return [entry({ rung: "tools", id: "tools/nix-home-manager", label: "Nix home-manager config", linux: "yes", ...(await found(host, ["~/.config/home-manager", "~/.config/nixpkgs/home.nix"])) })];
// }
// tools.ts:252
// // Each row installs on the image: a language manager fetches that package's own Linux build, and apt is the
// // image's own package manager. A manager whose rows the build reads no road off (apt) files none for a tool the
// // catalog carries: that row would stand in for the catalog's, whose own road is what installs it there.
// async function globalRows(host: Host, g: GlobalManager): Promise<ManifestEntry[]> {
//   if (!(await host.exec.which(g.bin))) return [];
//   const all = "list" in g ? await g.list(host) : g.parse((await host.exec.run(g.bin, g.args)) ?? "");
//   const pkgs = readsRowRoad(g.id) ? all : all.filter(p => catalogToolFor(p.name) === undefined);
//   return pkgs.map(p => item({ rung: "tools", id: toolRowId(g.id, p.name), label: versioned(p, g.sep), group: g.group, linux: "yes", ...(p.version !== undefined ? { version: p.version } : {}) }));
// }

// tools.ts:181
// /** The folders a global package of npm, pnpm or bun may sit in on this computer, by its name: one level under npm's and
//  * bun's global folders, and wherever pnpm's own listing says, since pnpm 11 keeps each global in a hashed folder of its
//  * own under `pnpm root -g` and pnpm 10 names a linked folder where it really is. */
// export async function jsGlobalFolders(host: Host): Promise<(name: string) => string[]> {
//   const [npm, pnpm] = await Promise.all([host.exec.run("npm", ["root", "-g"]), host.exec.run("pnpm", NPM_LS)]);
//   const roots = [...new Set([firstLine(npm), ...NPM_PREFIXES.map(p => `${p}/lib/node_modules`), expand(host, BUN_GLOBALS)].filter((r): r is string => r !== undefined))];
//   const listed = new Map<string, string[]>();
//   for (const p of parsePnpmGlobals(pnpm ?? "")) if (p.path !== undefined) listed.set(p.name, [...(listed.get(p.name) ?? []), p.path]);
//   return name => [...roots.map(root => `${root}/${name}`), ...(listed.get(name) ?? [])];
// }
/// The returned function gives a package's candidate folders: each root's `<root>/<name>`, then pnpm's listed paths.
pub async fn js_global_folders(host: &Host) -> Box<dyn Fn(&str) -> Vec<String>> {
    todo!()
}

// tools.ts:191
// /** The commands a global package puts on PATH, off its package.json in any of its folders: each key of an object
//  * `bin`, or for a single one the name past its scope, as npm links it. */
// export async function packageCommands(host: Host, name: string, folders: readonly string[]): Promise<string[]> {
//   const texts = await Promise.all(folders.map(folder => host.fs.readText(`${folder}/package.json`)));
//   return [
//     ...new Set(
//       texts.flatMap(text => {
//         const data = tryJson(text ?? "");
//         const bin = isRecord(data) ? data["bin"] : undefined;
//         return typeof bin === "string" ? [name.split("/").at(-1) ?? name] : isRecord(bin) ? Object.keys(bin) : [];
//       }),
//     ),
//   ];
// }
pub async fn package_commands(host: &Host, name: &str, folders: &[String]) -> Vec<String> {
    todo!()
}

// tools.ts:259
// export async function detectTools(host: Host): Promise<ManifestEntry[]> {
//   // Every manager is a program of its own, so they are all asked at once; the rows keep this order whichever answers first.
//   const [brew, nix, globals, go] = await Promise.all([brewRows(host), nixRows(host), Promise.all(GLOBALS.map(g => globalRows(host, g))), goRows(host)]);
//   return present([...brew, ...nix, ...globals.flat(), ...go]);
// }
/// Every manager is asked at once; the rows keep brew, nix, the GLOBALS in table order, then go.
/// apt's list read failing part way fails the whole read, as the TS `Promise.all` rejects (coordinator's ruling).
pub async fn detect_tools(host: &Host) -> Result<Vec<ManifestEntry>, HostError> {
    todo!()
}
