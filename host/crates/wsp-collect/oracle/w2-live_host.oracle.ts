// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/live_host.md: live-host.ts's pure parts and NodeFs/on_path over a real temp tree (wave-2 audit, group a).
// Process cases (setsid, grace, cancel, caps, NUL) are not recorded: they depend on timing or on a running child.
//
// A tree is {"<rel path>": entry}; entries are applied in key order, parent folders are made as needed:
//   "text"                      a file holding that text
//   {"text": s, "mode": "0755"} a file with a mode
//   {"bytes": N}                a file of N bytes ("x")
//   {"hex": "ef bb bf 61"}      a file of those raw bytes
//   {"cat": [[s, n], ...]}      a file of each s repeated n times, joined
//   {"dir": true}               an empty folder
//   {"link": "<target>"}        a symlink; "<dir>" in the target is the temp folder
//   {"hardlink": "<rel>"}       a hard link to another entry of the tree
//   {"fifo": true}              a named pipe
// "path" is joined to the temp folder as `<dir>/<path>` (trailing slashes and dots kept as written; "" is the folder itself).
// Every absolute path in "out" is written "<dir>/<rel>". "mtimeMs" is left out of stat answers.
import { chmodSync, linkSync, mkdirSync, mkdtempSync, symlinkSync, writeFileSync } from "node:fs";
import { execFileSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { it } from "vitest";
import { nodeExec, nodeFs, platformOf } from "../../../../packages/collect/src/live-host.js";
import { write } from "./w2-host.js";

type Entry = string | { text?: string; mode?: string; bytes?: number; hex?: string; cat?: [string, number][]; dir?: true; link?: string; hardlink?: string; fifo?: true };
type Tree = Record<string, Entry>;
type Case = Record<string, unknown>;

function build(tree: Tree): string {
  const d = mkdtempSync(join(tmpdir(), "wsp-w2-"));
  for (const [rel, e] of Object.entries(tree)) {
    const p = join(d, rel);
    mkdirSync(dirname(p), { recursive: true });
    if (typeof e === "string") writeFileSync(p, e);
    else if (e.dir) mkdirSync(p, { recursive: true });
    else if (e.link !== undefined) symlinkSync(e.link.split("<dir>").join(d), p);
    else if (e.hardlink !== undefined) linkSync(join(d, e.hardlink), p);
    else if (e.fifo) execFileSync("mkfifo", [p]);
    else {
      if (e.hex !== undefined) writeFileSync(p, Buffer.from(e.hex.replace(/\s+/g, ""), "hex"));
      else if (e.cat !== undefined) writeFileSync(p, e.cat.map(([s, n]) => s.repeat(n)).join(""));
      else if (e.bytes !== undefined) writeFileSync(p, "x".repeat(e.bytes));
      else writeFileSync(p, e.text ?? "");
      if (e.mode !== undefined) chmodSync(p, parseInt(e.mode, 8));
    }
  }
  return d;
}

const sub = (v: unknown, d: string): unknown => JSON.parse(JSON.stringify(v ?? null).split(d).join("<dir>"));
const all = async (it: AsyncIterable<string>): Promise<string[]> => {
  const out: string[] = [];
  for await (const l of it) out.push(l);
  return out;
};

async function fsCase(name: string, tree: Tree, op: "stat" | "list" | "walk" | "read_text" | "lines", path: string, extra: { holding?: string; lens?: boolean } = {}): Promise<Case> {
  const d = build(tree);
  const p = path === "" ? d : `${d}/${path}`;
  let out: unknown;
  if (op === "stat") {
    const s = await nodeFs.stat(p);
    out = s === undefined ? null : { kind: s.kind, bytes: s.bytes };
  } else if (op === "list") out = await nodeFs.list(p);
  else if (op === "walk") out = await nodeFs.walk(p);
  else if (op === "read_text") out = (await nodeFs.readText(p)) ?? null;
  else {
    const lines = await all(nodeFs.lines(p, extra.holding));
    out = extra.lens ? lines.map(l => l.length) : lines;
  }
  const c: Case = { name, tree, op, path, ...(extra.holding !== undefined ? { holding: extra.holding } : {}), ...(extra.lens ? { lens: true } : {}) };
  c[extra.lens ? "out_lens" : "out"] = sub(out, d);
  return c;
}

const M = 1 << 20;

it("live_host fixtures", async () => {
  const fixture: Record<string, Case[]> = {};

  const sizes: Tree = { "t/.git": "0123456789", "t/sub/node_modules": "abc", "t/a": "12", "t/.git2/": { dir: true } };
  const skipped: Tree = { "tree/a.txt": "abc", "tree/sub/b.txt": "defg", "tree/.git/c": { bytes: 100 }, "tree/node_modules/d": { bytes: 100 }, "tree/deep/x/.git/y": { bytes: 50 }, "tree/link": { link: "<dir>/tree/a.txt" } };
  const links: Tree = { "real/f": { bytes: 7 }, linkdir: { link: "<dir>/real" }, linkfile: { link: "<dir>/real/f" }, broken: { link: "<dir>/nope" } };
  const fs: Case[] = [];
  fs.push(await fsCase("a file", { f: "hello" }, "stat", "f"));
  fs.push(await fsCase("a missing path", {}, "stat", "nope"));
  fs.push(await fsCase("git and node_modules folders and links are not sized", skipped, "stat", "tree"));
  fs.push(await fsCase("a file named .git or node_modules is sized; an empty .git2 folder is not a problem", sizes, "stat", "t"));
  fs.push(await fsCase("the root may be named node_modules", { "node_modules/x/f": { bytes: 5 } }, "stat", "node_modules"));
  fs.push(await fsCase("an empty folder", { empty: { dir: true } }, "stat", "empty"));
  fs.push(await fsCase("a hard link is counted twice", { "real/f": { bytes: 7 }, "real/hard": { hardlink: "real/f" } }, "stat", "real"));
  fs.push(await fsCase("a fifo has no stat", { ff: { fifo: true } }, "stat", "ff"));
  fs.push(await fsCase("a fifo in a folder is not counted", { "d/ff": { fifo: true }, "d/f": { bytes: 3 } }, "stat", "d"));
  fs.push(await fsCase("a link to a folder is followed", links, "stat", "linkdir"));
  fs.push(await fsCase("a link to a file is followed", links, "stat", "linkfile"));
  fs.push(await fsCase("a broken link has no stat", links, "stat", "broken"));
  fs.push(await fsCase("a path under a file has no stat", { f: "x" }, "stat", "f/g"));
  fixture["node_fs_stat"] = fs;

  const names = ["a/x", "a-b/x", "a.b/x", "a0", "B", "b", "é", "Ａ", "😀", "a/B", "a/a"];
  const sortTree: Tree = Object.fromEntries(names.map(n => [`w/${n}`, ""]));
  const list: Case[] = [];
  list.push(await fsCase("names sort by UTF-16 units, dots and links included", { ...sortTree, "w/.hidden": "", "w/link": { link: "<dir>/w/a0" } }, "list", "w"));
  list.push(await fsCase("a link to a folder as the root", links, "list", "linkdir"));
  list.push(await fsCase("a file has no names", links, "list", "real/f"));
  list.push(await fsCase("a missing folder", {}, "list", "nope"));
  list.push(await fsCase("an empty folder", { e: { dir: true } }, "list", "e"));
  list.push(await fsCase("git and node_modules are listed by name", skipped, "list", "tree"));
  fixture["node_fs_list"] = list;

  const walk: Case[] = [];
  walk.push(await fsCase("whole paths sort by UTF-16 units: a-b/x before a/B, an astral name before U+FF21", sortTree, "walk", "w"));
  walk.push(await fsCase("a trailing slash is normalised away", sortTree, "walk", "w/"));
  walk.push(await fsCase("a double trailing slash is normalised away", sortTree, "walk", "w//"));
  walk.push(await fsCase("dot-dot segments are normalised", sortTree, "walk", "w/a/../a-b"));
  walk.push(await fsCase("dot segments are normalised", sortTree, "walk", "w/./a-b"));
  walk.push(await fsCase("git and node_modules folders and links are not entered", skipped, "walk", "tree"));
  walk.push(await fsCase("a file named .git or node_modules is listed", sizes, "walk", "t"));
  walk.push(await fsCase("the root may be named node_modules", { "node_modules/x/f": { bytes: 5 } }, "walk", "node_modules"));
  walk.push(await fsCase("a hard link is a file; a fifo is not listed", { "real/f": { bytes: 7 }, "real/hard": { hardlink: "real/f" }, "real/ff": { fifo: true } }, "walk", "real"));
  walk.push(await fsCase("a link to a folder as the root keeps the link's name", links, "walk", "linkdir"));
  walk.push(await fsCase("a file has no walk", links, "walk", "real/f"));
  walk.push(await fsCase("a missing folder", {}, "walk", "nope"));
  fixture["node_fs_walk"] = walk;

  const rt: Case[] = [];
  rt.push(await fsCase("a BOM is kept", { f: { hex: "ef bb bf 61" } }, "read_text", "f"));
  rt.push(await fsCase("invalid byte runs become one replacement per maximal subpart", { f: { hex: "61 e2 82 62 f0 9f 98 63 ed a0 80 64 c0 af 65 ff" } }, "read_text", "f"));
  rt.push(await fsCase("a folder has no text", { d: { dir: true } }, "read_text", "d"));
  rt.push(await fsCase("a missing file", {}, "read_text", "nope"));
  rt.push(await fsCase("an empty file is empty text", { f: "" }, "read_text", "f"));
  rt.push(await fsCase("a NUL is kept", { f: { hex: "61 00 62" } }, "read_text", "f"));
  fixture["node_fs_read_text"] = rt;

  const ln: Case[] = [];
  const L = (name: string, text: Entry, extra: { holding?: string; lens?: boolean } = {}) => fsCase(name, { f: text }, "lines", "f", extra).then(c => ln.push(c));
  await L("an empty file has no lines", "");
  await L("a lone LF is one empty line", "\n");
  await L("a lone CR is one empty line", "\r");
  await L("two LF are two empty lines", "\n\n");
  await L("a trailing LF gives no last empty line", "a\nb\n");
  await L("a BOM is kept", { hex: "ef bb bf 61 0a 62" });
  await L("a NUL is kept", { hex: "61 00 62 0a" });
  await L("an empty needle holds every line", "a\n\nb", { holding: "" });
  await L("a needle holding a CR matches before the CR is dropped", "ab\r\ncd\n", { holding: "b\r" });
  await L("a needle without the CR matches the same line", "ab\r\ncd\n", { holding: "b" });
  await L("a needle of LF matches nothing", "ab\ncd\n", { holding: "\n" });
  await L("the needle is case sensitive", "Ab\nab\n", { holding: "ab" });
  await L("the needle is matched as UTF-8 bytes", "café\ncafe\n", { holding: "é" });
  await L("a last line without LF is matched and loses its CR", "x\ny\r", { holding: "y" });
  await L("CR at the last byte of a read, LF first of the next", { cat: [["x", M - 1], ["\r\n", 1], ["tail\n", 1]] }, { lens: true });
  await L("a line exactly one read long, then its LF", { cat: [["x", M], ["\n", 1], ["y\n", 1]] }, { lens: true });
  await L("a needle straddling the read boundary", { cat: [["a\n", 1], ["y", M - 6], ["TOOL", 1], ["z", 10], ["\nq\n", 1]] }, { holding: "TOOL", lens: true });
  await L("lines ending exactly at the read end", { cat: [["x", M - 1], ["\n", 1], ["w", M - 1], ["\n", 1]] }, { lens: true });
  await L("a CR as the last byte of the file at the read end", { cat: [["x", M - 1], ["\r", 1]] }, { lens: true });
  await L("a multi-byte character across the read boundary", { cat: [["a", M - 1], ["é", 1], ["\nz", 1]] }, { lens: true });
  ln.push(await fsCase("a folder gives no lines", { d: { dir: true } }, "lines", "d"));
  ln.push(await fsCase("a missing file gives no lines", {}, "lines", "nope"));
  fixture["node_fs_lines"] = ln;

  // on_path: the TS reads process.env.PATH at call time; "path" is the PATH text ("<dir>" is the temp folder), null is unset.
  const bins: Tree = {
    "bin1/adir/": { dir: true }, "bin1/exe": { text: "x", mode: "0644" }, "bin1/linkexe": { link: "<dir>/bin2/exe" }, "bin1/dead": { link: "<dir>/nowhere" },
    "bin2/tool": { text: "x", mode: "0644" }, "bin2/exe": { text: "x", mode: "0755" }, "bin2/sub/exe2": { text: "x", mode: "0755" },
  };
  const onPath: Case[] = [];
  const real = process.env["PATH"];
  const op = async (name: string, bin: string, path: string | null) => {
    const d = build(bins);
    if (path === null) delete process.env["PATH"];
    else process.env["PATH"] = path.split("<dir>").join(d);
    const out = await nodeExec.which(bin.split("<dir>").join(d));
    onPath.push({ name, tree: bins, bin, path, out });
  };
  await op("a folder named like the bin counts", "adir", "<dir>/bin1");
  await op("an empty bin counts when a PATH folder exists", "", "<dir>/bin1");
  await op("only empty entries", "", "::");
  await op("a file without an executable bit", "tool", "<dir>/bin2");
  await op("an executable file", "exe", "<dir>/bin2");
  await op("a later folder wins when the earlier one has a non-executable file of the name", "exe", "<dir>/bin1:<dir>/bin2");
  await op("a bin with a slash is joined under the folder", "sub/exe2", "<dir>/bin2");
  await op("an absolute bin is joined under the folder, not used as it is", "<dir>/bin2/exe", "<dir>/bin2");
  await op("a bin with dot-dot", "../bin2/exe", "<dir>/bin1");
  await op("a link to an executable", "linkexe", "<dir>/bin1");
  await op("a broken link", "dead", "<dir>/bin1");
  await op("PATH unset", "exe", null);
  await op("PATH empty", "exe", "");
  await op("a PATH entry with a leading space is not trimmed", "exe", " <dir>/bin2");
  await op("a PATH entry with a trailing slash", "exe", "<dir>/bin2/");
  await op("a missing folder first, then the real one", "exe", "<dir>/nope:<dir>/bin2");
  if (real === undefined) delete process.env["PATH"];
  else process.env["PATH"] = real;
  fixture["on_path"] = onPath;

  // children_of: ps listing -> {parent: [children]} in listing order. Parse copied from live-host.ts:165-169 (treeOf is not exported).
  const parse = (listing: string): Record<string, number[]> => {
    const children = new Map<number, number[]>();
    for (const line of listing.split("\n")) {
      const [p, parent] = line.trim().split(/\s+/).map(Number);
      if (p === undefined || parent === undefined || Number.isNaN(p) || Number.isNaN(parent)) continue;
      children.set(parent, [...(children.get(parent) ?? []), p]);
    }
    return Object.fromEntries([...children.entries()].map(([k, v]) => [String(k), v]));
  };
  fixture["children_of"] = [
    ["the listing of the ported test", "  1     0\n 20 1\nx y\n 21 1\n\n 30 20\n"],
    ["a third column is ignored", "5 1 extra\n6 1\n"],
    ["a duplicate line is kept twice", "5 1\n5 1\n"],
    ["a one column line is skipped", "5\n6 1\n"],
    ["tabs, CR and padding", "\t5\t1\r\n 6   1 \n"],
    ["a plus sign parses", "+5 1\n"],
    ["a hex pid parses as its value", "0x10 5\n"],
    ["an exponent pid parses as its value", "1e2 7\n"],
    ["a pid that is its own parent", "1 1\n"],
    ["an empty listing", ""],
  ].map(([name, listing]) => ({ name, args: [listing], out: parse(listing as string) }));

  // platform_of takes node's platform names in the TS; the Rust takes std::env::consts::OS, where "macos" is node's "darwin".
  fixture["platform_of"] = ["darwin", "linux", "macos", "win32", "windows", "freebsd", ""].map(os => ({ name: `node platform ${JSON.stringify(os)}`, args: [os], out: platformOf(os as never) ?? null }));

  write("live_host", fixture);
}, 60000);
