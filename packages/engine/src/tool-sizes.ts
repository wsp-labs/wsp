// SPDX-License-Identifier: AGPL-3.0-only
// What a recipe costs on the builder's disk, judged before anything boots: a
// row the catalog carries takes the catalog's measured size, which already
// holds its Linux runtime dependencies, so two catalog formulae that share one
// count it twice (the estimate overstates, never under); any other Homebrew
// formula is its dependency closure from this Mac's own Homebrew, shared
// members once; Homebrew's toolchain is one line; a row nothing measured counts
// at a stated default for its kind. Nothing here runs a command: the host reads
// the Mac's Homebrew and hands the table in.
import { BREW_TOOLCHAIN, CATALOG_PREFIX, MACOS_ONLY_FORMULAE, catalogToolOf, formulaOf, isTap, packageOf, type BrewFormula, type BrewTable, type RecipeEntry, type ToolSource } from "./golden-import.js";
import { MIB, catalogEntry, catalogToolByRoad, isRoad, sizeBytes } from "@wsp/catalog";
import { BREW_ID_PREFIX, toolRowPrefix } from "@wsp/protocol";
import { TOOLS_DISK_FLOOR } from "./golden-tools.js";

/** Root disk asked for every builder and fork, Solari's cap: a 4 GB root filled during the tools stage and
 * five agents failed to install on it (measured 2026-09-05). */
export const BUILDER_DISK_GB = 20;
/** What df -Pk said was free on a 20 GB builder after the base stage, before the upload: 17,992,136 KiB (measured
 * 2026-09-05, when the base was the daemon alone). The base image, the filesystem's reserved blocks and everything the
 * base stage installs are inside it, so it is read again on the first golden built with the base floor. */
export const BUILDER_FREE_BYTES = 17570 * MIB;
/** What the recipe may take: what the builder has free less the floor the tools stage keeps free for unpack peaks. */
export const DISK_ROOM_BYTES = BUILDER_FREE_BYTES - TOOLS_DISK_FLOOR;

/** Homebrew's checkout with its own glibc and gcc, pulled in by the first formula: df moved 1006 MB across the three installs. */
export const BREW_TOOLCHAIN_BYTES = 1024 * MIB;

/** What a row nothing measured counts as, by what installs it: two go installs moved df by 115 MB and 924 MB
 * (module and build caches), three uv tools by 12 to 222 MB, three npm globals by 0 to 25 MB, six agents by
 * 165 to 673 MB. */
const ASSUMED_MIB = { go: 500, uv: 100, npm: 50, agent: 350, other: 100 } as const;

/** The default a row without a size counts at, and the words for where it came from. */
export interface AssumedSize {
  bytes: number;
  kind: "a go install" | "a uv tool" | "an npm global" | "an agent" | "an install";
}

const NPM_MANAGERS = ["npm", "pnpm", "bun"] as const;

/** The stated default for a tools or agents row nothing measured. */
export function assumedSize(e: RecipeEntry): AssumedSize {
  if (e.rung === "agents") return { bytes: ASSUMED_MIB.agent * MIB, kind: "an agent" };
  if (e.id.startsWith(toolRowPrefix("go"))) return { bytes: ASSUMED_MIB.go * MIB, kind: "a go install" };
  if (e.id.startsWith(toolRowPrefix("uv"))) return { bytes: ASSUMED_MIB.uv * MIB, kind: "a uv tool" };
  if (NPM_MANAGERS.some(m => e.id.startsWith(toolRowPrefix(m)))) return { bytes: ASSUMED_MIB.npm * MIB, kind: "an npm global" };
  return { bytes: ASSUMED_MIB.other * MIB, kind: "an install" };
}

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === "object" && v !== null && !Array.isArray(v);
}

const str = (v: unknown): string | undefined => (typeof v === "string" ? v : undefined);
const strings = (v: unknown): string[] => (Array.isArray(v) ? v.filter((x): x is string => typeof x === "string") : []);

const GITHUB = /^https:\/\/github\.com\/([^/]+)\/([^/]+?)(?:\.git)?(?:\/(.*))?$/;

/** The GitHub repository and tag a formula builds from, when its url names them: a release asset,
 * a tag archive, or a git url with the tag beside it. */
export function sourceOf(url: string | undefined, tag: string | null | undefined): ToolSource | undefined {
  const m = url === undefined ? null : GITHUB.exec(url);
  if (m === null) return undefined;
  const repo = `${m[1]}/${m[2]}`;
  const rest = m[3] ?? "";
  const release = /^releases\/download\/([^/]+)\//.exec(rest)?.[1];
  const archive = /^archive\/(?:refs\/tags\/)?([^/]+?)\.(?:tar\.gz|zip)$/.exec(rest)?.[1];
  const found = release ?? archive ?? (rest === "" ? tag ?? undefined : undefined);
  return found === undefined ? undefined : { repo, tag: found };
}

/** The formulae in `brew info --json=v2 --installed`, one row each, without sizes. */
export function parseBrewInfo(json: unknown): BrewFormula[] {
  if (!isRecord(json) || !Array.isArray(json["formulae"])) return [];
  const out: BrewFormula[] = [];
  for (const f of json["formulae"]) {
    if (!isRecord(f)) continue;
    const name = str(f["name"]);
    const fullName = str(f["full_name"]) ?? name;
    if (name === undefined || fullName === undefined) continue;
    const installed = Array.isArray(f["installed"]) && isRecord(f["installed"][0]) ? f["installed"][0] : undefined;
    const runtime = installed !== undefined && Array.isArray(installed["runtime_dependencies"]) ? installed["runtime_dependencies"].flatMap(d => (isRecord(d) ? strings([d["full_name"]]) : [])) : [];
    const deps = [...new Set([...strings(f["dependencies"]), ...runtime])];
    const requirements = Array.isArray(f["requirements"]) ? f["requirements"] : [];
    const macosOnly = MACOS_ONLY_FORMULAE.has(fullName) || requirements.some(r => isRecord(r) && r["name"] === "macos");
    const urls = isRecord(f["urls"]) && isRecord(f["urls"]["stable"]) ? f["urls"]["stable"] : undefined;
    const source = sourceOf(str(urls?.["url"]), urls === undefined ? undefined : (str(urls["tag"]) ?? null));
    const version = str(installed?.["version"]);
    out.push({ name, fullName, deps, macosOnly, ...(version !== undefined ? { version } : {}), ...(source !== undefined ? { source } : {}) });
  }
  return out;
}

/** The name a du line is filed under: the last part of its path, unless the caller knows the paths it asked about. */
const duName = (path: string): string => path.slice(path.lastIndexOf("/") + 1);

/** `du -sk` lines over a directory's entries: bytes by directory name, or by whatever `keyOf` calls each path. */
export function parseDu(text: string, keyOf: (path: string) => string = duName): Map<string, number> {
  const out = new Map<string, number>();
  for (const line of text.split("\n")) {
    const m = /^(\d+)\s+(.+)$/.exec(line.trim());
    if (m === null) continue;
    out.set(keyOf(m[2]!.replace(/\/+$/, "")), Number(m[1]) * 1024);
  }
  return out;
}

/** The Mac's formulae by full name, each with its Cellar size when du read one. */
export function brewTable(json: unknown, du: ReadonlyMap<string, number>): BrewTable {
  const out = new Map<string, BrewFormula>();
  for (const f of parseBrewInfo(json)) {
    const bytes = du.get(f.name);
    out.set(f.fullName, bytes === undefined ? f : { ...f, bytes });
  }
  return out;
}

export interface ToolSize {
  bytes: number;
  /** Where the number came from: the catalog's measurement, or this Mac's Homebrew. */
  road: "measured" | "mac";
  /** Dependencies counted in besides the row's own formula. */
  deps: number;
}

const TOOLCHAIN = new Set<string>(BREW_TOOLCHAIN);

/** A formula's size: the catalog's measured closure when it carries the formula, else this Mac's Cellar. */
function formulaBytes(name: string, brew: BrewTable): number | undefined {
  const entry = catalogToolByRoad("brew", name);
  return entry !== undefined ? sizeBytes(entry.size) : brew.get(name)?.bytes;
}

/** The formula and every formula it depends on, by full name, Homebrew's own toolchain left out; a formula the
 * catalog carries is a leaf, since its measured size already holds its runtime dependencies. */
function closureOf(name: string, brew: BrewTable): Set<string> {
  const seen = new Set<string>([name]);
  const queue = [name];
  for (let n = queue.shift(); n !== undefined; n = queue.shift()) {
    if (catalogToolByRoad("brew", n) !== undefined) continue;
    for (const d of brew.get(n)?.deps ?? []) {
      if (TOOLCHAIN.has(d) || seen.has(d)) continue;
      seen.add(d);
      queue.push(d);
    }
  }
  return seen;
}

/** The catalog's measured size for a row: a catalog row's own, or the size of the catalog tool that installs the
 * same package by the same road as this Mac's row (a formula by brew, a global by npm). */
function catalogSize(e: RecipeEntry): ToolSize | undefined {
  const manager = e.id.split("/")[1];
  const entry = e.id.startsWith(CATALOG_PREFIX) ? catalogEntry(packageOf(e)) : (catalogToolOf(e) ?? (isRoad(manager) ? catalogToolByRoad(manager, packageOf(e)) : undefined));
  const size = entry === undefined ? undefined : sizeBytes(entry.size);
  return size === undefined ? undefined : { bytes: size, road: "measured", deps: 0 };
}

/** What one tools row puts on the machine: the catalog's measurement where it carries the tool, else a formula
 * with its closure from this Mac; nothing for a tap or a row nothing measured or read. */
export function toolSize(e: RecipeEntry, brew: BrewTable): ToolSize | undefined {
  if (isTap(e)) return undefined;
  const known = catalogSize(e);
  if (known !== undefined) return known;
  const formula = formulaOf(e);
  if (formula === undefined || formulaBytes(formula, brew) === undefined) return undefined;
  const members = closureOf(formula, brew);
  let bytes = 0;
  for (const m of members) bytes += formulaBytes(m, brew) ?? 0;
  return { bytes, road: "mac", deps: members.size - 1 };
}

/** An agent's measured install size, by the row's name. */
export function agentSize(e: RecipeEntry): number | undefined {
  const entry = catalogEntry(e.id.slice(e.id.indexOf("/") + 1));
  return entry === undefined ? undefined : sizeBytes(entry.size);
}
