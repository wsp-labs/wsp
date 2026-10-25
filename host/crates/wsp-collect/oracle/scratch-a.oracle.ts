// scratch for audit group a
import { execFileSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, symlinkSync, writeFileSync, utimesSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { it } from "vitest";
import { nodeExec, nodeFs, nodeHost, platformOf, spawnRun } from "../../../../packages/collect/src/live-host.js";

const P = (label: string, v: unknown): void => console.log(`ORACLE ${label} ${JSON.stringify(v)}`);
const mk = (): string => mkdtempSync(join(tmpdir(), "wsp-a-"));
const all = async (it: AsyncIterable<string>): Promise<string[]> => {
  const out: string[] = [];
  for await (const l of it) out.push(l);
  return out;
};

it("fs", async () => {
  const d = mk();
  // tree with a FILE named .git and node_modules, nested
  mkdirSync(join(d, "t/sub"), { recursive: true });
  writeFileSync(join(d, "t/.git"), "0123456789");
  writeFileSync(join(d, "t/sub/node_modules"), "abc");
  writeFileSync(join(d, "t/a"), "12");
  mkdirSync(join(d, "t/.git2"));
  P("tree with .git file", { stat: await nodeFs.stat(join(d, "t")), walk: (await nodeFs.walk(join(d, "t"))).map(p => p.slice(d.length)) });
  // root named node_modules is entered
  mkdirSync(join(d, "node_modules/x"), { recursive: true });
  writeFileSync(join(d, "node_modules/x/f"), "12345");
  P("root is node_modules", { statBytes: (await nodeFs.stat(join(d, "node_modules")))!.bytes, walk: (await nodeFs.walk(join(d, "node_modules"))).map(p => p.slice(d.length)) });
  // symlinks
  mkdirSync(join(d, "real"));
  writeFileSync(join(d, "real/f"), "1234567");
  symlinkSync(join(d, "real"), join(d, "linkdir"));
  symlinkSync(join(d, "nope"), join(d, "broken"));
  symlinkSync(join(d, "real/f"), join(d, "linkfile"));
  const sl = await nodeFs.stat(join(d, "linkdir"));
  P("stat symlink to dir", { kind: sl!.kind, bytes: sl!.bytes });
  P("stat symlink to file", { kind: (await nodeFs.stat(join(d, "linkfile")))!.kind, bytes: (await nodeFs.stat(join(d, "linkfile")))!.bytes });
  P("stat broken link", await nodeFs.stat(join(d, "broken")));
  P("walk via symlink root", (await nodeFs.walk(join(d, "linkdir"))).map(p => p.slice(d.length)));
  P("list via symlink root", await nodeFs.list(join(d, "linkdir")));
  P("list of a file", await nodeFs.list(join(d, "real/f")));
  P("walk of a file", await nodeFs.walk(join(d, "real/f")));
  // empty dir
  mkdirSync(join(d, "empty"));
  P("empty dir", { ...(await nodeFs.stat(join(d, "empty")))!, mtimeMs: "x" });
  // hard link counted twice
  execFileSync("ln", [join(d, "real/f"), join(d, "real/hard")]);
  P("hardlink", (await nodeFs.stat(join(d, "real")))!.bytes);
  // fifo skipped in walk and in bytes
  execFileSync("mkfifo", [join(d, "real/ff")]);
  P("fifo in walk", (await nodeFs.walk(join(d, "real"))).map(p => p.slice(d.length)));
  // walk sort: whole path strings
  mkdirSync(join(d, "w/a"), { recursive: true });
  mkdirSync(join(d, "w/a-b"), { recursive: true });
  mkdirSync(join(d, "w/a.b"), { recursive: true });
  for (const f of ["a/x", "a-b/x", "a.b/x", "a0", "B", "b", "é", "Ａ", "😀", "a/B", "a/a"]) { try { writeFileSync(join(d, "w", f), ""); } catch (e) { P("err " + f, String(e)); } }
  P("walk sort", (await nodeFs.walk(join(d, "w"))).map(p => p.slice(join(d, "w").length)));
  P("list sort", await nodeFs.list(join(d, "w")));
  // path normalisation
  P("walk trailing slash", (await nodeFs.walk(join(d, "w") + "/")).slice(0, 3).map(p => p.slice(d.length)));
  P("walk double slash", (await nodeFs.walk(join(d, "w") + "//")).slice(0, 2).map(p => p.slice(d.length)));
  P("walk dotdot", (await nodeFs.walk(join(d, "w/a/../a-b"))).map(p => p.slice(d.length)));
  P("walk dot", (await nodeFs.walk(join(d, "w/./a-b"))).map(p => p.slice(d.length)));
  // read_text
  writeFileSync(join(d, "bom"), Buffer.from([0xef, 0xbb, 0xbf, 0x61]));
  P("bom", await nodeFs.readText(join(d, "bom")));
  writeFileSync(join(d, "bad"), Buffer.from([0x61, 0xe2, 0x82, 0x62, 0xf0, 0x9f, 0x98, 0x63, 0xed, 0xa0, 0x80, 0x64, 0xc0, 0xaf, 0x65, 0xff]));
  P("bad bytes readText", await nodeFs.readText(join(d, "bad")));
  P("readText of dir", await nodeFs.readText(d));
  P("readText missing", await nodeFs.readText(join(d, "zzz")));
  P("readText empty", await (async () => { writeFileSync(join(d, "e"), ""); return nodeFs.readText(join(d, "e")); })());
  P("readText nul", await (async () => { writeFileSync(join(d, "nul"), Buffer.from([0x61, 0, 0x62])); return nodeFs.readText(join(d, "nul")); })());
  // stat of file mtime is float
});

it("lines", async () => {
  const d = mk();
  const f = (name: string, data: Buffer | string): string => { writeFileSync(join(d, name), data); return join(d, name); };
  P("empty file", await all(nodeFs.lines(f("e", ""))));
  P("only LF", await all(nodeFs.lines(f("lf", "\n"))));
  P("only CR", await all(nodeFs.lines(f("cr", "\r"))));
  P("two LF", await all(nodeFs.lines(f("lf2", "\n\n"))));
  P("ends with LF", await all(nodeFs.lines(f("el", "a\nb\n"))));
  P("bom", await all(nodeFs.lines(f("bom", Buffer.from([0xef, 0xbb, 0xbf, 0x61, 0x0a, 0x62])))));
  P("nul", await all(nodeFs.lines(f("nul", Buffer.from([0x61, 0, 0x62, 0x0a])))));
  P("dir", await all(nodeFs.lines(d)));
  P("holding empty string", await all(nodeFs.lines(f("h", "a\n\nb"), "")));
  P("holding with CR", await all(nodeFs.lines(f("h2", "ab\r\ncd\n"), "b\r")));
  P("holding with CR, other", await all(nodeFs.lines(f("h3", "ab\r\ncd\n"), "b")));
  P("holding LF", await all(nodeFs.lines(f("h4", "ab\ncd\n"), "\n")));
  P("holding is case sensitive", await all(nodeFs.lines(f("h5", "Ab\nab\n"), "ab")));
  P("holding multibyte", await all(nodeFs.lines(f("h6", "caf\u00e9\ncafe\n"), "\u00e9")));
  P("holding on the last line without LF and CR", await all(nodeFs.lines(f("h7", "x\ny\r"), "y")));
  // CR/LF straddling the 1 MiB boundary
  const M = 1 << 20;
  const straddle = await all(nodeFs.lines(f("s1", "x".repeat(M - 1) + "\r\n" + "tail\n")));
  P("CR then LF across boundary", straddle.map(l => l.length));
  const straddle2 = await all(nodeFs.lines(f("s2", "x".repeat(M - 1) + "\r" + "\nnext\n")));
  P("CR at last byte of chunk, LF first of next", straddle2.map(l => [l.length, l.endsWith("\r")]));
  const straddle3 = await all(nodeFs.lines(f("s3", "x".repeat(M) + "\n" + "y\n")));
  P("line exactly one chunk long then LF at start of the next", straddle3.map(l => l.length));
  const straddle4 = await all(nodeFs.lines(f("s4", "a\n" + "y".repeat(M - 2 - 4) + "TOOL" + "z".repeat(10) + "\nq\n"), "TOOL"));
  P("needle across the boundary", straddle4.map(l => l.length));
  const exact = await all(nodeFs.lines(f("s5", "x".repeat(M - 1) + "\n" + "w".repeat(M - 1) + "\n")));
  P("lines ending exactly at chunk end", exact.map(l => l.length));
  const crlast = await all(nodeFs.lines(f("s6", "x".repeat(M - 1) + "\r")));
  P("last line CR is last byte of file at the boundary", crlast.map(l => l.length));
  const multi = await all(nodeFs.lines(f("s7", "a".repeat(M - 1) + "\u00e9" + "\nz")));
  P("multibyte char across boundary", multi.map(l => [l.length, l.slice(-1)]));
  // after a partial consume the file is closed: break out early
  for await (const l of nodeFs.lines(f("s8", "1\n2\n3\n"))) { P("first line then break", l); break; }
});
