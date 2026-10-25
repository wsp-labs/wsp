// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/project.md: the project scan and its four readers on the edge inputs of the wave-2 audit (group c).
// paths_of, files_for and run_blocks are not exported by the TS; their effects show in scan_project's answers and calls.
import { it } from "vitest";
import { scanProject } from "../../../../packages/collect/src/project/index.js";
import { markerReader } from "../../../../packages/collect/src/project/readers/marker.js";
import { packageJsonReader } from "../../../../packages/collect/src/project/readers/package-json.js";
import { toolchainPinsReader } from "../../../../packages/collect/src/project/readers/toolchain-pins.js";
import { workflowsReader } from "../../../../packages/collect/src/project/readers/workflows.js";
import { host, plain, settle, write, type Laptop } from "./w2-host.js";

const P = "/Users/dev/proj";
const laptopOf = (files: Record<string, string | number>): Laptop => ({ files: Object.fromEntries(Object.entries(files).map(([k, v]) => [`${P}/${k}`, v])) });
const scan = async (name: string, files: Record<string, string | number>, folder = P) => {
  const laptop = laptopOf(files);
  const h = host(laptop);
  return { name, laptop, args: [folder], ...(await settle(() => scanProject(h, folder))), calls: h.calls };
};
const pkg = (o: unknown): string => JSON.stringify(o);
const reader = (r: { read(f: { path: string; text: string }): unknown }, name: string, path: string, text: string) => ({ name, args: [{ path, text }], out: plain(r.read({ path, text })) });
const MARKERS = ["pnpm-lock.yaml", "yarn.lock", "bun.lock", "bun.lockb", "package-lock.json", "pyproject.toml", "uv.lock", "requirements.txt", ".tool-versions", "mise.toml", ".mise.toml", "go.mod", "Cargo.toml", "Gemfile", "composer.json", "Dockerfile", "compose.yaml", "compose.yml", "docker-compose.yaml", "docker-compose.yml"];
const SETUPS = ["  - uses: foo/setup-action@v1", "  - uses: x/setup-go-action@v1", "  - uses: pnpm/foo/action-setup@v4", "  - uses: bar/baz/action-setup", "  -uses: actions/setup-java@v4", "  - uses: \"actions/setup-ruby\"", "  # - uses: actions/setup-deno@v1", "  - uses: ./local/setup-node", "  - uses: actions/checkout@v4", "  - uses: a/setup-.@v1", "  - uses: a/setup-@v1", "  - uses: a/b/setup-lua@v1", "  - uses: Actions/Setup-Node@v1", "  uses: actions/setup-php@v1 # c", "\t- uses: actions/setup-zig@v1"];

it("project fixtures", async () => {
  const scans: [string, Record<string, string | number>][] = [
    ["CRLF .tool-versions: a comment line is not cut, so it is a pin named #", { ".tool-versions": "# pinned\r\nnodejs 22.1.0\r\npython 3.12 # c\r\nruby 3.3\r\n" }],
    ["CRLF mise.toml: header, keys and versions still read", { "mise.toml": "[tools]\r\ngo = \"1.24\" # c\r\nzig = \"0.13\"\r\n" }],
    ["mise key and version quirks", { "mise.toml": "[tools] # x\n'node' = \"22\"\n\"zig' = \"0.1\"\nnode.version = \"1\"\n= \"v\"\nrust = \"1.80#x\"\npy = [\n  \"3.12\",\n]\ngo = 1.24\nbun = 'a' # \"b\"\n[ tools ]\nzz = \"1\"\n" }],
    ["mise tables: [tools] reopened counts, [[tools]] does not", { "mise.toml": "[tools]\na = \"1\"\n[env]\nb = \"2\"\n[tools]\nc = \"3\"\n[[tools]]\nd = \"4\"\n" }],
    ["mise version regex", { "mise.toml": "[tools]\na = \"a'b\"\nb = \"\"\nc = { version = \"22\", x = \"1\" }\nd = \"\" \"1\"\ne = 'x\"\nf=\"6\"\n  g  =  \"7\"  \n\"h\" = \"8\"\n'i\" = \"9\"\n" }],
    [".tool-versions whitespace: a BOM is trimmed, U+0085 does not split, a vertical tab does", { ".tool-versions": "\ufeffnodejs 22.1\ngolang\u00851.24\nzig\u000b0.1\n" }],
    [".tool-versions blank, tab and nbsp", { ".tool-versions": "nodejs\u00a022.1\n\tgolang\t1.24\n#\nzig # c\n  \n" }],
    [".tool-versions first version only", { ".tool-versions": "nodejs 22 20 18\n   \n\n# only\n" }],
    ["first pin wins across .tool-versions, mise.toml, .mise.toml", { ".tool-versions": "zig 0.13\nnodejs 20\n", "mise.toml": "[tools]\nzig = \"0.14\"\nnode = \"22\"\n", ".mise.toml": "[tools]\nzig = \"0.15\"\n" }],
    ["candidates dedupe by word across readers", { "package.json": pkg({ packageManager: "weirdpm@1" }), ".tool-versions": "weirdpm 2\nzig 1\n", ".github/workflows/a.yml": "- uses: x/setup-zig@v1\n" }],
    ["CRLF workflow: run lines with text never match, an empty run: does", { ".github/workflows/a.yml": "steps:\r\n  - uses: actions/setup-go@v5\r\n  - run: gh pr list\r\n  - run: |\r\n      docker build .\r\n  - run:\r\n      npm ci\r\n" }],
    ["workflow setup variants", { ".github/workflows/a.yml": ["steps:", ...SETUPS].join("\n") }],
    ["run block indent counts the spaces before the dash", { ".github/workflows/a.yml": "  - run: |\n    gh pr list\n" }],
    ["run block: a sibling key deeper than the dash is inside it, a same-column one ends it", { ".github/workflows/a.yml": "      - run: |\n          npm ci\n        shell: x && gh pr list\n      - name: n\n        run: |\n          uv sync\n        shell: x && docker build .\n" }],
    ["run block: an env sibling", { ".github/workflows/a.yml": "jobs:\n  b:\n    steps:\n      - run: |\n          docker build .\n        env:\n          X: gh\n      - name: n\n        run: |\n          npm ci\n        env: {}\n" }],
    ["a run: line inside a block is not scanned again", { ".github/workflows/a.yml": "- run: |\n    echo hi\n    run: gh pr\n    - run: docker x\n- run: npm ci\n" }],
    ["run |+ is an inline command, not a block", { ".github/workflows/a.yml": "- run: |+\n    gh pr list\n" }],
    ["run |2 is an inline command, not a block", { ".github/workflows/a.yml": "- run: |2\n    gh pr list\n" }],
    ["inline run text keeps its quotes", { ".github/workflows/a.yml": "- run: 'gh pr list'\n- run: \"docker build .\"\n" }],
    ["blank lines stay in a block, |- and > open blocks, trailing spaces after run:", { ".github/workflows/a.yml": "- run: |-\n    gh a\n\n    docker b\n\n- run: >\n    npm i\n- run: |+\n    cargo b\n- run: |2\n    go b\n- run: 'gh pr list'\n- run: \"docker build .\"\n- run:   \n    uv sync\n" }],
    ["blank lines before the first block line", { ".github/workflows/a.yml": "- run: |\n    gh a\n\n    \n\n- run: |\n\n    docker b\n" }],
    ["an empty run: and a run: at the end of the file", { ".github/workflows/a.yml": "- run:\n- run: npm ci" }],
    ["tab-indented block line", { ".github/workflows/a.yml": "- run: |\n\tgh pr list\n" }],
    ["every file in the workflows folder is read, a subfolder gives nothing", { ".github/workflows/notes.txt": "- run: gh pr list\n", ".github/workflows/sub/ci.yml": "- run: docker x\n", ".github/workflows/z.yml": "- run: npm ci\n" }],
    ["an empty workflow file", { ".github/workflows/a.yml": "" }],
    ["package.json arrays: scripts read by index, engines and dependencies give names 0", { "package.json": pkg({ scripts: ["gh release create"], engines: ["node"], dependencies: ["playwright"] }) }],
    ["package.json integer-like keys come first", { "package.json": pkg({ scripts: { b: "gh x", "1": "docker x", a: "uv x" } }) }],
    ["package.json duplicate keys: last value, first place", { "package.json": '{"scripts":{"a":"gh x","b":"docker y","a":"uv z"}}' }],
    ["package.json empty ranges leave a trailing space", { "package.json": pkg({ engines: { node: "" }, devDependencies: { playwright: "" } }) }],
    ["packageManager shapes", { "package.json": pkg({ packageManager: "weirdpm@1.0.0+sha512.abc+x" }), "apps/a/package.json": pkg({ packageManager: "pnpm" }), "apps/b/package.json": pkg({ packageManager: "+sha" }), "apps/c/package.json": pkg({ packageManager: "yarn@" }), "apps/d/package.json": pkg({ packageManager: "Yarn@4" }) }],
    ["scripts: version checks are not runs, heredoc bodies are skipped", { "package.json": pkg({ scripts: { a: "node --version", b: "node -v && gh --help", c: "docker version", d: "FOO=1 npm ci && cd x && uv sync | tee y; cargo build", e: "echo hi; python <<EOF\ngh x\nEOF" } }) }],
    ["dependencies before devDependencies whatever the file order", { "package.json": '{"devDependencies":{"@playwright/test":"1.0"},"dependencies":{"playwright":"2.0"}}' }],
    ["engines: first entry that lands on a row wins; non-object values give nothing", { "package.json": pkg({ engines: { npm: ">=10", node: ">=22" } }), "apps/x/package.json": pkg({ engines: "node", scripts: "gh x", dependencies: null }) }],
    ["wrong JSON types and empty text give nothing", { "package.json": "null", "apps/a/package.json": "\"str\"", "apps/b/package.json": "7", "apps/c/package.json": "", "apps/d/package.json": "{}" }],
    ["workspace order: root, apps, packages", { "packages/b/package.json": pkg({ scripts: { x: "gh x" } }), "packages/a/package.json": pkg({ scripts: { x: "gh y" } }), "apps/z/package.json": pkg({ scripts: { x: "docker y" } }), "package.json": pkg({ scripts: { x: "uv y" } }) }],
    ["only one level under apps and packages", { "apps/a/b/package.json": pkg({ scripts: { x: "gh y" } }), "apps/package.json": pkg({ scripts: { x: "docker y" } }), "packages/a/package.jsonx": "{}" }],
    ["only scripts and depends lines carry the file path", { "apps/a/package.json": pkg({ packageManager: "pnpm@9", engines: { node: "22" }, scripts: { x: "gh y" }, dependencies: { playwright: "1" } }) }],
    ["a directory named like a marker is skipped, an empty marker file counts", { "Dockerfile/x": "FROM", "go.mod": "", Gemfile: "x" }],
    ["empty text readers are read and give nothing", { "package.json": "", "mise.toml": "" }],
    ["every marker file", Object.fromEntries(MARKERS.map(f => [f, "x"]))],
    ["a folder with nothing to read", { "README.md": "# x" }],
  ];
  const cases = [];
  for (const [name, files] of scans) cases.push(await scan(name, files));
  cases.push(await scan("folder with several trailing slashes", { "go.mod": "x" }, `${P}///`));
  cases.push(await scan("folder with a trailing space is not trimmed", { "go.mod": "x" }, `${P} `));
  cases.push(await scan("a folder that is not there", {}, "/Users/dev/none"));

  write("project", {
    scan_project: cases,
    package_json_read: [
      reader(packageJsonReader, "packageManager with corepack suffix", "package.json", pkg({ packageManager: "pnpm@10.15.0+sha512.abc" })),
      reader(packageJsonReader, "packageManager without @", "package.json", pkg({ packageManager: "pnpm" })),
      reader(packageJsonReader, "packageManager that is only a suffix", "package.json", pkg({ packageManager: "+sha" })),
      reader(packageJsonReader, "packageManager scoped name", "package.json", pkg({ packageManager: "@yarnpkg/cli@4" })),
      reader(packageJsonReader, "engines, scripts, dependencies and devDependencies in order", "apps/web/package.json", pkg({ engines: { node: ">=22", npm: 7 }, scripts: { build: "tsup && gh x", n: 1 }, dependencies: { a: "1", b: 2 }, devDependencies: { c: "3" } })),
      reader(packageJsonReader, "arrays are walked by index", "package.json", pkg({ scripts: ["gh x"], engines: ["node"], dependencies: ["a"] })),
      reader(packageJsonReader, "integer-like keys first", "package.json", pkg({ scripts: { b: "gh x", "1": "docker x" } })),
      reader(packageJsonReader, "not JSON", "package.json", "not json"),
      reader(packageJsonReader, "empty text", "package.json", ""),
      reader(packageJsonReader, "an array root", "package.json", "[1,2]"),
      reader(packageJsonReader, "null", "package.json", "null"),
      reader(packageJsonReader, "empty ranges", "package.json", pkg({ engines: { node: "" }, devDependencies: { playwright: "" } })),
    ],
    toolchain_pins_read: [
      reader(toolchainPinsReader, ".tool-versions with CRLF", ".tool-versions", "# pinned\r\nnodejs 22.1.0\r\npython 3.12 # c\r\n"),
      reader(toolchainPinsReader, ".tool-versions name only and blanks", ".tool-versions", "nodejs\n  python 3.12 3.11 # pinned\n#all\n\n"),
      reader(toolchainPinsReader, ".tool-versions BOM, U+0085, vertical tab", ".tool-versions", "\ufeffnodejs 22.1\ngolang\u00851.24\nzig\u000b0.1\n"),
      reader(toolchainPinsReader, "mise.toml tools table", "mise.toml", "[tools]\n'node' = { version = '22' }\nrust = 'stable'\n[tools.extra]\nzig = '0.13'\n"),
      reader(toolchainPinsReader, "mise.toml header with a comment, odd values", "mise.toml", "[tools] # x\n'node' = \"22\"\n\"zig' = \"0.1\"\nnode.version = \"1\"\n= \"v\"\nrust = \"1.80#x\"\npy = [\n  \"3.12\",\n]\ngo = 1.24\nbun = 'a' # \"b\"\n[ tools ]\nzz = \"1\"\n"),
      reader(toolchainPinsReader, "mise.toml version regex", ".mise.toml", "[tools]\na = \"a'b\"\nb = \"\"\nd = \"\" \"1\"\ne = 'x\"\n"),
      reader(toolchainPinsReader, "mise.toml with CRLF", "mise.toml", "[tools]\r\ngo = \"1.24\" # c\r\n"),
      reader(toolchainPinsReader, "empty text", "mise.toml", ""),
    ],
    marker_read: [
      ...MARKERS.map(f => reader(markerReader, `marker ${f}`, f, "")),
      reader(markerReader, "a path that is no marker", "README.md", ""),
      reader(markerReader, "a marker in a subfolder is no marker", "apps/Dockerfile", ""),
    ],
    workflows_read: [
      reader(workflowsReader, "setup actions and run steps", ".github/workflows/ci.yml", "jobs:\n  build:\n    steps:\n      - uses: actions/setup-go@v5\n      - uses: pnpm/action-setup@v4\n      - run: go build ./...\n      - run: |\n          docker compose up -d\n          npm ci\n      - name: after\n        run: echo done\n"),
      reader(workflowsReader, "setup variants", "w.yml", SETUPS.join("\n")),
      reader(workflowsReader, "CRLF run lines", "w.yml", "  - run: gh pr list\r\n  - run: |\r\n      docker build .\r\n  - run:\r\n      npm ci\r\n"),
      reader(workflowsReader, "indent before the dash", "w.yml", "  - run: |\n    gh pr list\n"),
      reader(workflowsReader, "sibling keys in a block", "w.yml", "      - run: |\n          npm ci\n        shell: x && gh pr list\n      - name: n\n        run: |\n          uv sync\n        shell: x && docker build .\n"),
      reader(workflowsReader, "a run: inside a block", "w.yml", "- run: |\n    echo hi\n    run: gh pr\n    - run: docker x\n- run: npm ci\n"),
      reader(workflowsReader, "|+ and |2 are inline", "w.yml", "- run: |+\n    gh pr list\n- run: |2\n    go b\n"),
      reader(workflowsReader, "blank lines in blocks, empty run", "w.yml", "- run: |-\n    gh a\n\n    docker b\n\n- run:\n- run: >\n    npm i\n"),
      reader(workflowsReader, "empty text", "w.yml", ""),
    ],
  });
});
