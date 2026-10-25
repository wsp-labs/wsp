// SPDX-License-Identifier: AGPL-3.0-only
import { it } from "vitest";
import { aptPackages, aptSizes, parseAptManual, parseDpkgFields } from "../../../../packages/collect/src/detect/apt.js";
import {
  detectTools, jsGlobalFolders, packageCommands, parseBrewfile, parseBunGlobals, parseCargoInstalls, parseGoVersionM, parseNpmGlobals, parsePipxList, parsePnpmGlobals, parseUvToolList,
} from "../../../../packages/collect/src/detect/tools.js";
import { fakeHost } from "../../../../packages/collect/test/fake-host.js";
import type { Host } from "../../../../packages/collect/src/host.js";

const show = (label: string, v: unknown): void => {
  console.log(`@@ ${label} :: ${JSON.stringify(v, (_k, x) => (x instanceof Map ? { __map: [...x.entries()] } : x === undefined ? "__undefined" : typeof x === "number" && !Number.isFinite(x) ? String(x) : x))}`);
};

const MANUAL = "direnv\nfd-find\nfish\n";
it("apt parse", () => {
  show("dpkg NEL", [...parseDpkgFields("a\u0085b\nc d\ne﻿ f\ng h\n")]);
  show("dpkg tab+multi space+trailing ws", [...parseDpkgFields("a \t b  \nx  y z\n  lead v\nq w \r\n")]);
  show("dpkg empty", [...parseDpkgFields("")]);
  show("manual trim FEFF/nbsp", parseAptManual("﻿a \n \t\n b \r\n\nc\nc\n"));
  show("manual empty", parseAptManual(""));
});

async function aptRun(files: Record<string, string>, manual: string, prio: string, extra: Record<string, string> = {}) {
  const host = fakeHost({ platform: "linux", which: ["apt-mark", "dpkg-query"], files, exec: { "apt-mark showmanual": manual, "dpkg-query -Wf ${Package} ${Priority}\n": prio, ...extra } });
  const r = await aptPackages(host);
  return { r, calls: host.calls };
}

it("apt packages", async () => {
  show("command paths", await aptRun({
    "/var/lib/dpkg/info/a.list": "/usr/bin/\n/usr/bin/a/b\n/usr/sbin/a\n",
    "/var/lib/dpkg/info/b.list": "/bin/b c\n",
    "/var/lib/dpkg/info/c.list": "  /usr/bin/c\r\n",
    "/var/lib/dpkg/info/d.list": "",
    "/var/lib/dpkg/info/e.list": "/usr/local/bin/e\n/sbin/e\n/usr/bin\n",
    "/var/lib/dpkg/info/f.list": "x\n/usr/bin/f",
    "/var/lib/dpkg/info/g.list": "/usr/bin/g\n",
    "/var/lib/dpkg/info/h.list": "/usr/bin/h\n",
    "/var/lib/dpkg/info/i.list": "/usr/bin/i\n",
  }, "a\nb\nc\nd\ne\nf\ng\nh\ni\n", "a optional\nb extra\nc optional\nd optional\ne optional\nf optional\ng Optional\nh standard\ni optional extra\n"));
  show("colon only list name and weird names", await aptRun({
    "/var/lib/dpkg/info/:amd64.list": "/usr/bin/nameless\n",
    "/var/lib/dpkg/info/x:a:b.list": "/usr/bin/x\n",
    "/var/lib/dpkg/info/x.list.list": "/usr/bin/xx\n",
    "/var/lib/dpkg/info/y.md5sums": "/usr/bin/y\n",
    "/var/lib/dpkg/info/z.LIST": "/usr/bin/z\n",
    "/var/lib/dpkg/info/w.list/inner": "/usr/bin/w\n",
  }, "x\ny\nz\nw\nx.list\n", "x optional\ny optional\nz optional\nw optional\nx.list optional\n"));
  show("manual NOT_A_CHOICE and order and trailing cr", await aptRun({
    "/var/lib/dpkg/info/tmux.list": "/usr/bin/tmux\n", "/var/lib/dpkg/info/unminimize.list": "/usr/bin/unminimize\n", "/var/lib/dpkg/info/apt-transport-https.list": "/usr/bin/q\n", "/var/lib/dpkg/info/Sudo.list": "/usr/bin/q\n",
  }, "unminimize\r\ntmux\r\nSudo\napt-transport-https\n", "tmux optional\nSudo optional\n"));
  show("priority dup last wins", await aptRun({ "/var/lib/dpkg/info/t.list": "/usr/bin/t\n" }, "t\n", "t required\nt optional\n"));
  show("priority dup last wins reverse", await aptRun({ "/var/lib/dpkg/info/t.list": "/usr/bin/t\n" }, "t\n", "t optional\nt required\n"));
  show("dpkg-query answers nothing", await aptRun({ "/var/lib/dpkg/info/t.list": "/usr/bin/t\n" }, "t\n", "").then(x => x));
  show("apt-mark nothing", await aptRun({ "/var/lib/dpkg/info/t.list": "/usr/bin/t\n" }, "", "t optional\n"));
  show("apt-mark blank lines only", await aptRun({}, "\n \n", ""));
  show("no list dir", await aptRun({}, "t\n", "t optional\n"));
  show("stops reading at first command line", await aptRun({ "/var/lib/dpkg/info/t.list": "/usr/bin/t\n/usr/bin/t2\n" }, "t\n", "t optional\n"));
  show("first manual dup read once each", await aptRun({ "/var/lib/dpkg/info/t.list": "/usr/share/x\n" }, "t\nt\n", "t optional\n"));
  const noApt = fakeHost({ platform: "linux", files: { "/var/lib/dpkg/info/t.list": "/usr/bin/t\n" } });
  show("exec all undefined", await aptPackages(noApt));
  show("exec all undefined calls", noApt.calls);
});

it("apt failing lines", async () => {
  const base = fakeHost({ platform: "linux", which: ["apt-mark"], files: { "/var/lib/dpkg/info/a.list": "/usr/bin/a\n", "/var/lib/dpkg/info/b.list": "/usr/bin/b\n" }, exec: { "apt-mark showmanual": "a\nb\n", "dpkg-query -Wf ${Package} ${Priority}\n": "a optional\nb optional\n" } });
  const reads: string[] = [];
  const host: Host = { ...base, fs: { ...base.fs, async *lines(path: string) { reads.push(path); if (path.endsWith("a.list")) { yield "x"; throw new Error("EIO: boom"); } yield "/usr/bin/b"; } } };
  try { show("failing lines result", await aptPackages(host)); } catch (e) { show("failing lines threw", String(e)); }
  show("failing lines reads", reads);
  const host2: Host = { ...base, fs: { ...base.fs, async *lines(path: string) { reads.push(path); if (path.endsWith("a.list")) { yield "/usr/bin/a"; throw new Error("EIO: late"); } yield "/usr/bin/b"; } } };
  try { show("late failure after match", await aptPackages(host2)); } catch (e) { show("late failure threw", String(e)); }
  const host3: Host = { ...base, fs: { ...base.fs, async *lines(path: string) { if (path.endsWith("b.list")) throw new Error("EIO: second"); yield "/usr/bin/a"; } } };
  try { show("second file fails", await aptPackages(host3)); } catch (e) { show("second file fails threw", String(e)); }
});

it("apt sizes", async () => {
  const sizes = async (out: string | undefined, names: string[]) => {
    const host = fakeHost({ exec: out === undefined ? {} : { "dpkg-query -Wf ${Package} ${Installed-Size}\n": out } });
    const r = await aptSizes(host, names);
    return { r: [...r.entries()], calls: host.calls };
  };
  const fields = ["+5", ".5", "5.", "0b11", "0B11", "0o17", "0XFF", "1e400", "1_0", "-0", "1e-1", "0x", "1,5", "00012", "1e3", "9007199254740993", "NaN", "-Infinity", "0.0001", "٣"];
  show("sizes odd numbers", await sizes(fields.map((f, i) => `p${i} ${f}`).join("\n") + "\n", fields.map((_, i) => `p${i}`)));
  show("sizes order is names order not dpkg order", await sizes("a 1\nb 2\nc 3\n", ["c", "a", "zz", "b", "a"]));
  show("sizes dup package last wins", await sizes("a 1\na 5\n", ["a"]));
  show("sizes dpkg fails", await sizes(undefined, ["a"]));
  show("sizes empty names", await sizes("a 1\n", []));
  show("sizes name with empty string", await sizes("a 1\n", [""]));
});

it("tools parsers", () => {
  show("brewfile", parseBrewfile("  brew \"a\"  \n\tbrew\t\"b\"\r\nbrew 'c'\nbrew \"\"\nbrewx \"d\"\ntap \"e/f\", \"https://x\"\n# brew \"g\"\ntap  \"h i\"\nbrew \"a\"\nbrew \"j\" # c\nbrew\n\"k\"\nBrew \"l\"\ncask \"m\"\nbrew \"n \"\n﻿brew \"o\"\nbrew \"p"));
  show("brewfile empty", parseBrewfile(""));
  show("npm odd", parseNpmGlobals(JSON.stringify({ dependencies: { a: { version: "" }, b: { version: 1 }, c: "x", d: null, e: [], f: { version: "1", extra: 1 }, npm: {}, corepack: {}, "npm2": {} } })));
  show("npm top-level wrong type", [parseNpmGlobals("{}"), parseNpmGlobals(""), parseNpmGlobals('{"dependencies":null}'), parseNpmGlobals('{"dependencies":"x"}'), parseNpmGlobals('{"dependencies":{}}'), parseNpmGlobals('"x"'), parseNpmGlobals("5")]);
  show("npm dup key", parseNpmGlobals('{"dependencies":{"a":{"version":"1"},"b":{"version":"2"},"a":{"version":"3"}}}'));
  show("pnpm odd", parsePnpmGlobals(JSON.stringify([{ devDependencies: { d: { version: "1" } } }, { dependencies: { a: { version: "", path: "" } } }, null, "x", [], { dependencies: { b: { path: 5, version: "2" } } }, { dependencies: { a: { version: "9", path: "/p" } } }, { dependencies: null }])));
  show("pnpm not array", [parsePnpmGlobals("{}"), parsePnpmGlobals(""), parsePnpmGlobals("[]"), parsePnpmGlobals("null")]);
  show("pipx odd", parsePipxList(JSON.stringify({ venvs: { a: { metadata: { main_package: { package_version: "" } } }, b: { metadata: { main_package: { package_version: 3 } } }, c: null, d: { metadata: null }, e: { metadata: { main_package: [] } }, f: { metadata: { main_package: { package_version: "1", x: 1 } } } } })));
  show("pipx not object", [parsePipxList("[]"), parsePipxList('{"venvs":[]}'), parsePipxList('{"venvs":null}'), parsePipxList("{}")]);
  const nv = "ruff v0.6.3\r\n- ruff\n\nhttpx  v0.27.0 extra\n  indented v1\n-dash v1\nbat v0.24.0 (/src/bat):\n    bat\nx v1:2:3\ny v\nz vX\nvfoo vbar\nnov 1.0\n\tt\tv1\nname\tv2\nNo tools installed\nfoo v1.0 \nbar v1\n lead v1\nlast v9";
  show("name version lines", parseUvToolList(nv));
  show("cargo same", JSON.stringify(parseCargoInstalls(nv)) === JSON.stringify(parseUvToolList(nv)));
  show("bun odd", parseBunGlobals("/home/x/.bun/install/global node_modules (4)\r\n├── a@1.0.0\r\n├── b@1.0.0\n│   ├── c@2\n└── @s/d@3\n  e@4\n├── f@1@2\n├── g@\n├── @h\n├── @@i@1\n├── j@1 x\n├── k@1.0.0 \n├──  l@5\nm@6\n├── @n@7\n└── @/o@8\n"));
  show("go parse", [
    parseGoVersionM("\tpath\ta\n\tmod\ta\tv1\th1:x\n"),
    parseGoVersionM("path\ta\nmod\ta\tv1\n"),
    parseGoVersionM("x\tpath\t\nx\tmod\tm\t\n"),
    parseGoVersionM("x\tpath\ta\nx\tmod\tm\n"),
    parseGoVersionM("x\tpath\ta\nx\tmod\tm\tv1\nx\tmod\tm2\tv2\nx\tpath\tb\n"),
    parseGoVersionM("x\tpath\ta\nx\tdep\tm\tv1\n"),
    parseGoVersionM("x\tpath\ta\tb\nx\tmod\tm\tv1\tz\r\n"),
    parseGoVersionM(""),
    parseGoVersionM("x\tpath\ta\r\nx\tmod\tm\tv1\r\n"),
    parseGoVersionM("x\tmod\tm\tv1\nx\tpath\ta\n"),
  ]);
});

it("tools folders and commands", async () => {
  const ex = async (exec: Record<string, string>, name: string, extra: Partial<{ which: string[] }> = {}) => {
    const host = fakeHost({ exec, ...extra });
    const f = await jsGlobalFolders(host);
    return { f: f(name), calls: host.calls };
  };
  show("folders: npm root dup of fixed", await ex({ "npm root -g": "/opt/homebrew/lib/node_modules\n" }, "x"));
  show("folders: pnpm path equals root path (no dedupe)", await ex({ "npm root -g": "/r\n", "pnpm ls -g --depth=0 --json": JSON.stringify([{ dependencies: { x: { path: "/r/x" } } }, { dependencies: { x: { path: "/r/x" } } }]) }, "x"));
  show("folders: nothing answers", await ex({}, "x"));
  show("folders: npm root blank / padded", [await ex({ "npm root -g": "\n/zz\n" }, "x"), await ex({ "npm root -g": "  /zz  \n/yy\n" }, "x"), await ex({ "npm root -g": "/zz\r\n" }, "x")]);
  show("folders: pnpm listing wrong shape", await ex({ "pnpm ls -g --depth=0 --json": "{}" }, "x"));
  show("folders: scoped name", await ex({ "npm root -g": "/r" }, "@s/p"));
  show("folders: pnpm path empty string", await ex({ "pnpm ls -g --depth=0 --json": JSON.stringify([{ dependencies: { x: { path: "" } } }]) }, "x"));
  show("folders: pnpm path non-string", await ex({ "pnpm ls -g --depth=0 --json": JSON.stringify([{ dependencies: { x: { path: 5 } } }]) }, "x"));
  show("folders: runs without which", (await ex({}, "x")).calls);
  const pc = async (files: Record<string, string>, name: string, folders: string[]) => {
    const host = fakeHost({ files });
    return { r: await packageCommands(host, name, folders), calls: host.calls };
  };
  show("pc bin empty string", await pc({ "/a/package.json": '{"bin":""}' }, "@s/tool", ["/a"]));
  show("pc bin array/null/number/empty obj", await pc({ "/a/package.json": '{"bin":["x"]}', "/b/package.json": '{"bin":null}', "/c/package.json": '{"bin":5}', "/d/package.json": '{"bin":{}}', "/e/package.json": "[]", "/f/package.json": "null", "/g/package.json": "" }, "t", ["/a", "/b", "/c", "/d", "/e", "/f", "/g"]));
  show("pc int keys and dup across folders", await pc({ "/a/package.json": '{"bin":{"b":"1","2":"x","a":"3"}}', "/b/package.json": '{"bin":{"a":"1","c":"2"}}', "/c/package.json": '{"bin":"x"}' }, "scope/deep/name", ["/a", "/b", "/c"]));
  show("pc name edge", [await pc({ "/a/package.json": '{"bin":"x"}' }, "", ["/a"]), await pc({ "/a/package.json": '{"bin":"x"}' }, "@s/", ["/a"]), await pc({ "/a/package.json": '{"bin":"x"}' }, "plain", ["/a"])]);
  show("pc no folders", await pc({}, "t", []));
  show("pc bin key empty and name unaffected by package.json name", await pc({ "/a/package.json": '{"name":"other","bin":{"":"x","cli":"y"}}' }, "t", ["/a"]));
  show("pc a folder given as a file", await pc({ "/a/package.json": 7 as unknown as string }, "t", ["/a"]));
});
