// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/apt_tools.md: apt.ts and tools.ts on the edge inputs of the wave-2 audit (group f).
import { it } from "vitest";
import { aptPackages, aptSizes, parseAptManual, parseDpkgFields } from "../../../../packages/collect/src/detect/apt.js";
import {
  detectTools, jsGlobalFolders, packageCommands, parseBrewfile, parseBunGlobals, parseCargoInstalls, parseGoVersionM, parseNpmGlobals, parsePipxList, parsePnpmGlobals, parseUvToolList,
} from "../../../../packages/collect/src/detect/tools.js";
import { type Laptop, host, plain, settle, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const pure = (name: string, f: (...a: never[]) => unknown, ...args: unknown[]): Case => ({ name, args, out: plain((f as (...a: unknown[]) => unknown)(...args)) });
// `calls` is left out where managers run side by side, since their order depends on interleaving.
const onHost = async (name: string, laptop: Laptop, run: (h: ReturnType<typeof host>) => unknown, args: unknown[] = [], calls = true): Promise<Case> => {
  const h = host(laptop);
  const r = await settle(() => run(h));
  return { name, laptop, args, ...r, ...(calls ? { calls: h.calls } : {}) };
};

const PRIORITY = "dpkg-query -Wf ${Package} ${Priority}\n";
const SIZE = "dpkg-query -Wf ${Package} ${Installed-Size}\n";
const info = (name: string, text: string): [string, string] => [`/var/lib/dpkg/info/${name}`, text];
const aptLaptop = (files: [string, string][], manual: string, prio: string, extra: Laptop = {}): Laptop => ({
  platform: "linux", which: ["apt-mark", "dpkg-query"], files: Object.fromEntries(files), exec: { "apt-mark showmanual": manual, [PRIORITY]: prio }, ...extra,
});
const NPM_LS = "npm ls -g --depth=0 --json";
const BREW_DUMP = "HOMEBREW_NO_AUTO_UPDATE=1 brew bundle dump --file=- --formula --taps --no-restart";

const NAME_VERSION = "ruff v0.6.3\r\n- ruff\n\nhttpx  v0.27.0 extra\n  indented v1\n-dash v1\nbat v0.24.0 (/src/bat):\n    bat\nx v1:2:3\ny v\nz vX\nvfoo vbar\nnov 1.0\n\tt\tv1\nname\tv2\nNo tools installed\nfoo v1.0 \nbar v1\n lead v1\nlast v9";
const SIZE_TEXT = "p0 +5\np1 .5\np2 5.\np3 0b11\np4 0B11\np5 0o17\np6 0XFF\np7 1e400\np8 1_0\np9 -0\np10 1e-1\np11 0x\np12 1,5\np13 00012\np14 1e3\np15 NaN\np16 -Infinity\np17 0.0001\n";
const SIZE_NAMES = Array.from({ length: 18 }, (_, i) => `p${i}`);

it("apt_tools fixtures", async () => {
  const fixture: Record<string, Case[]> = {};

  fixture["parse_dpkg_fields"] = [
    pure("JavaScript whitespace: U+0085 is not a separator, U+00A0 U+FEFF U+2028 are", parseDpkgFields, "a\u0085b\nc d\ne﻿ f\ng h\n"),
    pure("tab and runs of spaces split, a leading space or a third field drops the line, trailing space and CR are fine", parseDpkgFields, "a \t b  \nx  y z\n  lead v\nq w \r\n"),
    pure("empty text", parseDpkgFields, ""),
  ];
  fixture["parse_apt_manual"] = [
    pure("JavaScript trim: BOM and NBSP go, blank lines go, repeats stay", parseAptManual, "﻿a \n \t\n b \r\n\nc\nc\n"),
    pure("empty text", parseAptManual, ""),
  ];

  const listFiles = (letters: string[]): [string, string][] => letters.map(l => info(`${l}.list`, `/usr/bin/${l}\n`));
  fixture["apt_packages"] = [
    await onHost("command path table and the priority filter", aptLaptop([
      info("a.list", "/usr/bin/\n/usr/bin/a/b\n/usr/sbin/a\n"), info("b.list", "/bin/b c\n"), info("c.list", "  /usr/bin/c\r\n"), info("d.list", ""),
      info("e.list", "/usr/local/bin/e\n/sbin/e\n/usr/bin\n"), info("f.list", "x\n/usr/bin/f"), ...listFiles(["g", "h", "i"]),
    ], "a\nb\nc\nd\ne\nf\ng\nh\ni\n", "a optional\nb extra\nc optional\nd optional\ne optional\nf optional\ng Optional\nh standard\ni optional extra\n"), h => aptPackages(h)),
    await onHost("package name from the list file name", aptLaptop([
      info(":amd64.list", "/usr/bin/nameless\n"), info("x:a:b.list", "/usr/bin/x\n"), info("x.list.list", "/usr/bin/xx\n"), info("y.md5sums", "/usr/bin/y\n"), info("z.LIST", "/usr/bin/z\n"),
    ], "x\ny\nz\nx.list\n\n", "x optional\ny optional\nz optional\nx.list optional\n"), h => aptPackages(h)),
    await onHost("a later entry of the directory listing replaces the earlier (t.list sorts before t:amd64.list)", aptLaptop([
      info("t:amd64.list", "/usr/bin/t\n"), info("t.list", "/usr/share/doc/t\n"),
    ], "t\n", "t optional\n"), h => aptPackages(h)),
    await onHost("NOT_A_CHOICE is exact and case-sensitive, applied after the trim, survivors in manual order", aptLaptop([
      info("tmux.list", "/usr/bin/tmux\n"), info("unminimize.list", "/usr/bin/unminimize\n"), info("apt-transport-https.list", "/usr/bin/q\n"), info("Sudo.list", "/usr/bin/q\n"),
    ], "unminimize\r\ntmux\r\nSudo\napt-transport-https\n", "tmux optional\nSudo optional\n"), h => aptPackages(h)),
    await onHost("a repeated priority line: the last wins (required then optional keeps it)", aptLaptop([info("t.list", "/usr/bin/t\n")], "t\n", "t required\nt optional\n"), h => aptPackages(h)),
    await onHost("a repeated priority line: the last wins (optional then required drops it, no read)", aptLaptop([info("t.list", "/usr/bin/t\n")], "t\n", "t optional\nt required\n"), h => aptPackages(h)),
    await onHost("dpkg-query answers nothing: no list is read", aptLaptop([info("t.list", "/usr/bin/t\n")], "t\n", "", {}), h => aptPackages(h)),
    await onHost("dpkg-query has no answer at all", { platform: "linux", files: { "/var/lib/dpkg/info/t.list": "/usr/bin/t\n" }, exec: { "apt-mark showmanual": "t\n" } }, h => aptPackages(h)),
    await onHost("apt-mark answers empty: nothing else runs", aptLaptop([info("t.list", "/usr/bin/t\n")], "", "t optional\n"), h => aptPackages(h)),
    await onHost("apt-mark answers blank lines only", aptLaptop([], "\n \n", ""), h => aptPackages(h)),
    await onHost("apt-mark has no answer", { platform: "linux", files: { "/var/lib/dpkg/info/t.list": "/usr/bin/t\n" } }, h => aptPackages(h)),
    await onHost("no list directory: two runs, no reads", aptLaptop([], "t\n", "t optional\n"), h => aptPackages(h)),
    await onHost("a repeated manual name with no command line is read once per entry", aptLaptop([info("t.list", "/usr/share/x\n")], "t\nt\n", "t optional\n"), h => aptPackages(h)),
    await onHost("a failing line read before any match is an error, later packages are not read", aptLaptop([
      info("a.list", "x\n\u0000fail:EIO: boom\n"), info("b.list", "/usr/bin/b\n"),
    ], "a\nb\n", "a optional\nb optional\n"), h => aptPackages(h)),
    await onHost("a failure after the matching line is never seen", aptLaptop([
      info("a.list", "/usr/bin/a\n\u0000fail:EIO: late\n"), info("b.list", "/usr/bin/b\n"),
    ], "a\nb\n", "a optional\nb optional\n"), h => aptPackages(h)),
    await onHost("a failure in the second package's file is an error after the first was read", aptLaptop([
      info("a.list", "/usr/bin/a\n"), info("b.list", "\u0000fail:EIO: second\n"),
    ], "a\nb\n", "a optional\nb optional\n"), h => aptPackages(h)),
  ];

  const sizeLaptop = (text?: string): Laptop => ({ ...(text === undefined ? {} : { exec: { [SIZE]: text } }) });
  fixture["apt_sizes"] = [
    await onHost("Number() forms", sizeLaptop(SIZE_TEXT), (h) => aptSizes(h, SIZE_NAMES), [SIZE_NAMES]),
    await onHost("names order, a missing name skipped, a repeated name once at its first place", sizeLaptop("a 1\nb 2\nc 3\n"), (h) => aptSizes(h, ["c", "a", "zz", "b", "a"]), [["c", "a", "zz", "b", "a"]]),
    await onHost("a repeated dpkg line: the last wins", sizeLaptop("a 1\na 5\n"), (h) => aptSizes(h, ["a"]), [["a"]]),
    await onHost("dpkg-query has no answer: empty after one run", sizeLaptop(), (h) => aptSizes(h, ["a"]), [["a"]]),
    await onHost("an empty name", sizeLaptop("a 1\n"), (h) => aptSizes(h, [""]), [[""]]),
    await onHost("no names: nothing runs", sizeLaptop("a 1\n"), (h) => aptSizes(h, []), [[]]),
  ];

  fixture["parse_brewfile"] = [
    pure("indent, tab, CR, single quotes, empty name, brewx, comment, split line, capital, cask, NBSP name, BOM line, unterminated", parseBrewfile,
      "  brew \"a\"  \n\tbrew\t\"b\"\r\nbrew 'c'\nbrew \"\"\nbrewx \"d\"\ntap \"e/f\", \"https://x\"\n# brew \"g\"\ntap  \"h i\"\nbrew \"a\"\nbrew \"j\" # c\nbrew\n\"k\"\nBrew \"l\"\ncask \"m\"\nbrew \"n \"\n﻿brew \"o\"\nbrew \"p"),
    pure("empty", parseBrewfile, ""),
  ];
  fixture["parse_npm_globals"] = [
    pure("version shapes and the names npm and corepack", parseNpmGlobals, JSON.stringify({ dependencies: { a: { version: "" }, b: { version: 1 }, c: "x", d: null, e: [], f: { version: "1", extra: 1 }, npm: {}, corepack: {}, npm2: {} } })),
    pure("duplicate JSON key: last value, first place", parseNpmGlobals, '{"dependencies":{"a":{"version":"1"},"b":{"version":"2"},"a":{"version":"3"}}}'),
    ...["{}", "", '{"dependencies":null}', '{"dependencies":"x"}', '"x"', "5", "[]", "null", '{"dependencies":[]}', "not json"].map(t => pure(`wrong shape ${JSON.stringify(t)}`, parseNpmGlobals, t)),
    pure("integer-like key first", parseNpmGlobals, '{"dependencies":{"b":{"version":"1"},"2":{"version":"2"},"a":{}}}'),
  ];
  fixture["parse_pnpm_globals"] = [
    pure("dependencies only, junk elements skipped, empty strings kept, no dedupe", parsePnpmGlobals,
      JSON.stringify([{ devDependencies: { d: { version: "1" } } }, { dependencies: { a: { version: "", path: "" } } }, null, "x", [], { dependencies: { b: { path: 5, version: "2" } } }, { dependencies: { a: { version: "9", path: "/p" } } }, { dependencies: null }])),
    ...["{}", "", "[]", "null"].map(t => pure(`not an array ${JSON.stringify(t)}`, parsePnpmGlobals, t)),
  ];
  fixture["parse_pipx_list"] = [
    pure("version shapes", parsePipxList, JSON.stringify({ venvs: { a: { metadata: { main_package: { package_version: "" } } }, b: { metadata: { main_package: { package_version: 3 } } }, c: null, d: { metadata: null }, e: { metadata: { main_package: [] } }, f: { metadata: { main_package: { package_version: "1", x: 1 } } } } })),
    ...["[]", '{"venvs":[]}', '{"venvs":null}', "{}"].map(t => pure(`wrong shape ${JSON.stringify(t)}`, parsePipxList, t)),
  ];
  fixture["parse_uv_tool_list"] = [pure("name and version lines", parseUvToolList, NAME_VERSION)];
  fixture["parse_cargo_installs"] = [pure("name and version lines", parseCargoInstalls, NAME_VERSION)];
  fixture["parse_bun_globals"] = [
    pure("tree lines must match whole", parseBunGlobals, "/home/x/.bun/install/global node_modules (4)\r\n├── a@1.0.0\r\n├── b@1.0.0\n│   ├── c@2\n└── @s/d@3\n  e@4\n├── f@1@2\n├── g@\n├── @h\n├── @@i@1\n├── j@1 x\n├── k@1.0.0 \n├──  l@5\nm@6\n├── @n@7\n└── @/o@8\n"),
  ];
  fixture["parse_go_version_m"] = [
    ["\tpath\ta\n\tmod\ta\tv1\th1:x\n", "path and mod"], ["path\ta\nmod\ta\tv1\n", "no leading tab"], ["x\tpath\t\nx\tmod\tm\t\n", "empty columns count as set"],
    ["x\tpath\ta\nx\tmod\tm\n", "mod line with three columns"], ["x\tpath\ta\nx\tmod\tm\tv1\nx\tmod\tm2\tv2\nx\tpath\tb\n", "last of each wins"],
    ["x\tpath\ta\nx\tdep\tm\tv1\n", "dep line"], ["x\tpath\ta\r\nx\tmod\tm\tv1\r\n", "CRLF keeps the CR"], ["x\tmod\tm\tv1\nx\tpath\ta\n", "mod first"], ["", "empty"],
    ["garbage", "garbage"],
  ].map(([t, n]) => pure(n!, parseGoVersionM, t));

  const folders = (name: string, laptop: Laptop, label: string): Promise<Case> => onHost(label, laptop, async h => (await jsGlobalFolders(h))(name), [name]);
  fixture["js_global_folders"] = [
    await folders("x", { exec: { "npm root -g": "/opt/homebrew/lib/node_modules\n" } }, "npm root equal to a fixed root: its place first, no repeat"),
    await folders("x", { exec: { "npm root -g": "/r\n", "pnpm ls -g --depth=0 --json": JSON.stringify([{ dependencies: { x: { path: "/r/x" } } }, { dependencies: { x: { path: "/r/x" } } }]) } }, "pnpm paths are not deduped, not even against the roots"),
    await folders("x", {}, "no answers: the three fixed roots, both commands still run"),
    await folders("x", { exec: { "npm root -g": "\n/zz\n" } }, "blank first line of npm root is no root"),
    await folders("x", { exec: { "npm root -g": "  /zz  \n/yy\n" } }, "padded npm root is trimmed"),
    await folders("x", { exec: { "npm root -g": "/zz\r\n" } }, "CRLF npm root"),
    await folders("x", { exec: { "pnpm ls -g --depth=0 --json": JSON.stringify([{ dependencies: { x: { path: "" } } }]) } }, "an empty pnpm path is kept"),
    await folders("x", { exec: { "pnpm ls -g --depth=0 --json": JSON.stringify([{ dependencies: { x: { path: 5 } } }]) } }, "a non-string pnpm path is dropped"),
    await folders("x", { exec: { "pnpm ls -g --depth=0 --json": "{}" } }, "pnpm listing of the wrong shape"),
    await folders("@s/p", { exec: { "npm root -g": "/r" } }, "scoped name"),
  ];

  const pc = (label: string, files: Record<string, number | string>, name: string, fl: string[]): Promise<Case> => onHost(label, { files }, h => packageCommands(h, name, fl), [name, fl]);
  fixture["package_commands"] = [
    await pc("bin an empty string is a string", { "/a/package.json": '{"bin":""}' }, "@s/tool", ["/a"]),
    await pc("bin an array, null, a number, an empty object; a package.json that is [], null or empty", {
      "/a/package.json": '{"bin":["x"]}', "/b/package.json": '{"bin":null}', "/c/package.json": '{"bin":5}', "/d/package.json": '{"bin":{}}', "/e/package.json": "[]", "/f/package.json": "null", "/g/package.json": "",
    }, "t", ["/a", "/b", "/c", "/d", "/e", "/f", "/g"]),
    await pc("integer-like key first, repeats keep the first place, last segment of the name", {
      "/a/package.json": '{"bin":{"b":"1","2":"x","a":"3"}}', "/b/package.json": '{"bin":{"a":"1","c":"2"}}', "/c/package.json": '{"bin":"x"}',
    }, "scope/deep/name", ["/a", "/b", "/c"]),
    await pc("empty name with a string bin", { "/a/package.json": '{"bin":"x"}' }, "", ["/a"]),
    await pc("name ending in a slash with a string bin", { "/a/package.json": '{"bin":"x"}' }, "@s/", ["/a"]),
    await pc("an empty bin key is kept and the package.json name is not read", { "/a/package.json": '{"name":"other","bin":{"":"x","cli":"y"}}' }, "t", ["/a"]),
    await pc("no folders", {}, "t", []),
  ];

  const tools = (label: string, laptop: Laptop, calls = true): Promise<Case> => onHost(label, laptop, h => detectTools(h), [], calls);
  const GO_OUT = (p: string, m: string, v: string) => `x\n\tpath\t${p}\n\tmod\t${m}\t${v}\th1:x\n`;
  fixture["detect_tools"] = [
    await tools("brew: repeated lines are repeated rows, taps and formulae in the Brewfile's order", { which: ["brew"], exec: { [BREW_DUMP]: 'brew "gh"\nbrew "gh"\ntap "a/b"\nbrew "a/b/c"\n' } }),
    await tools("brew: the dump has no answer", { which: ["brew"] }),
    await tools("brew on linux: every formula is yes and bring", { platform: "linux", which: ["brew"], exec: { [BREW_DUMP]: 'tap "x/y"\nbrew "mas"\n' } }),
    await tools("nix: only nixpkgs home.nix", { files: { "~/.config/nixpkgs/home.nix": 120 } }),
    await tools("nix: both, the folder with a size of its own", { files: { "~/.config/home-manager/": 7, "~/.config/nixpkgs/home.nix": 120 } }),
    await tools("pnpm duplicates and an empty version keep their rows; pipx and uv labels", {
      which: ["pnpm", "pipx", "uv", "cargo", "bun"], exec: {
        "pnpm ls -g --depth=0 --json": JSON.stringify([{ dependencies: { a: { version: "", path: "/p" }, b: {}, c: { version: "1" } } }, { dependencies: { a: { version: "2" } } }]),
        "pipx list --json": JSON.stringify({ venvs: { black: { metadata: {} }, e: { metadata: { main_package: { package_version: "" } } } } }),
        "uv tool list": "ruff v1\nruff v2\n", "cargo install --list": "x v1.0.0:\n    x\n", "bun pm ls -g": "h\n└── z@",
      },
    }, false),
    await tools("npm: a blank own prefix skips nothing, both fixed prefixes with globals are listed", {
      which: ["npm"], exec: { "npm prefix -g": "\n", [NPM_LS]: JSON.stringify({ dependencies: { a: { version: "" }, b: {} } }) },
      files: { "/opt/homebrew/lib/node_modules/": 1, "/usr/local/lib/node_modules/x/package.json": 1 },
    }),
    await tools("npm: own prefix /usr/local, homebrew listed second, a repeated name keeps the first", {
      which: ["npm"], exec: {
        "npm prefix -g": "/usr/local\n  ", [NPM_LS]: JSON.stringify({ dependencies: { a: { version: "1" } } }),
        [`${NPM_LS} --prefix /opt/homebrew`]: JSON.stringify({ dependencies: { b: { version: "2" }, a: { version: "0" } } }),
      }, files: { "/opt/homebrew/lib/node_modules/": 1, "/usr/local/lib/node_modules/y/package.json": 1 },
    }),
    await tools("npm: an own prefix with a trailing slash is not equal", {
      which: ["npm"], exec: {
        "npm prefix -g": "/opt/homebrew/\n", [NPM_LS]: JSON.stringify({ dependencies: { a: { version: "1" } } }),
        [`${NPM_LS} --prefix /opt/homebrew`]: JSON.stringify({ dependencies: { b: { version: "2" } } }),
      }, files: { "/opt/homebrew/lib/node_modules/y/package.json": 1 },
    }),
    await tools("npm: lib/node_modules as a plain file counts", {
      which: ["npm"], exec: { [`${NPM_LS} --prefix /opt/homebrew`]: JSON.stringify({ dependencies: { b: { version: "2" } } }) }, files: { "/opt/homebrew/lib/node_modules": 1 },
    }),
    await tools("npm: every answer missing", { which: ["npm"] }),
    await tools("go: order, an empty answer, a version that is (devel), path line not the mod name", {
      which: ["go"], files: { "~/go/bin/zz": 1, "~/go/bin/aa": 1, "~/go/bin/mid/inner": 1 }, exec: {
        "go version -m /Users/dev/go/bin/aa": "", "go version -m /Users/dev/go/bin/mid": GO_OUT("p/mid", "p/mid", "(devel)"), "go version -m /Users/dev/go/bin/zz": GO_OUT("p/zz", "p/other", "v1.2.3"),
      },
    }),
    await tools("go on PATH, no go/bin", { which: ["go"] }),
    await tools("go/bin present, go not on PATH", { files: { "~/go/bin/zz": 1 } }),
    await tools("apt rows: the catalog filter and an npm row for the same name", {
      platform: "linux", which: ["apt-mark", "dpkg-query", "npm"],
      files: Object.fromEntries(["tmux", "direnv", "fish", "starship", "jq", "git", "ripgrep", "neovim", "htop", "curl", "zsh"].map(n => [`/var/lib/dpkg/info/${n}.list`, `/usr/bin/${n}\n`])),
      exec: {
        "apt-mark showmanual": "tmux\ndirenv\nfish\nstarship\njq\ngit\nripgrep\nneovim\nhtop\ncurl\nzsh\n",
        [PRIORITY]: "tmux optional\ndirenv optional\nfish optional\nstarship optional\njq optional\ngit optional\nripgrep optional\nneovim optional\nhtop optional\ncurl optional\nzsh optional\n",
        [NPM_LS]: JSON.stringify({ dependencies: { tmux: { version: "1" } } }),
      },
    }, false),
    await tools("apt rows on a darwin host with apt-mark on PATH", { platform: "darwin", which: ["apt-mark"], files: { "/var/lib/dpkg/info/aa.list": "/usr/bin/aa\n" }, exec: { "apt-mark showmanual": "aa\n", [PRIORITY]: "aa optional\n" } }),
    await tools("a failing line read fails the whole rung", aptLaptop([info("a.list", "x\n\u0000fail:EIO: boom\n")], "a\n", "a optional\n", { which: ["apt-mark"] })),
    await tools("every manager on PATH with no answers: one go row", { which: ["npm", "pnpm", "bun", "pipx", "uv", "cargo", "go", "brew"], files: { "~/go/bin/g": 1 } }, false),
  ];

  write("apt_tools", fixture);
});
