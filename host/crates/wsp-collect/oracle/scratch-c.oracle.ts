// SPDX-License-Identifier: AGPL-3.0-only
import { mkdtempSync, mkdirSync, writeFileSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { it } from "vitest";
import { scanProject } from "../../../../packages/collect/src/project/index.js";
import { fakeHost } from "../../../../packages/collect/test/fake-host.js";

const P = "/Users/dev/proj";
const show = (label: string, v: unknown): void => console.log(`@@ ${label}\n${JSON.stringify(v)}`);
const scan = async (label: string, files: Record<string, string | number>, folder = P): Promise<void> => {
  const host = fakeHost({ files: Object.fromEntries(Object.entries(files).map(([k, v]) => [`${P}/${k}`, v])) });
  show(`project ${label}`, { files, scan: await scanProject(host, folder), calls: host.calls });
};

it("project", async () => {
  await scan("crlf tool-versions", { ".tool-versions": "# pinned\r\nnodejs 22.1.0\r\npython 3.12 # c\r\nruby 3.3\r\n" });
  await scan("crlf mise", { "mise.toml": "[tools]\r\ngo = \"1.24\" # c\r\nzig = \"0.13\"\r\n" });
  await scan("mise quirks", { "mise.toml": "[tools] # x\n'node' = \"22\"\n\"zig' = \"0.1\"\nnode.version = \"1\"\n= \"v\"\nrust = \"1.80#x\"\npy = [\n  \"3.12\",\n]\ngo = 1.24\nbun = 'a' # \"b\"\n[ tools ]\nzz = \"1\"\n" });
  await scan("mise tools reopen", { "mise.toml": "[tools]\na = \"1\"\n[env]\nb = \"2\"\n[tools]\nc = \"3\"\n[[tools]]\nd = \"4\"\n" });
  await scan("tv quirks", { ".tool-versions": "nodejs\u00a022.1\n\tgolang\t1.24\n#\nzig # c\n  \n" });
  await scan("pins first wins", { ".tool-versions": "zig 0.13\nnodejs 20\n", "mise.toml": "[tools]\nzig = \"0.14\"\nnode = \"22\"\n", ".mise.toml": "[tools]\nzig = \"0.15\"\n" });
  await scan("crlf workflow", { ".github/workflows/a.yml": "steps:\r\n  - uses: actions/setup-go@v5\r\n  - run: gh pr list\r\n  - run: |\r\n      docker build .\r\n  - run:\r\n      npm ci\r\n" });
  await scan("workflow setup variants", { ".github/workflows/a.yml": [
    "steps:",
    "  - uses: foo/setup-action@v1",
    "  - uses: x/setup-go-action@v1",
    "  - uses: pnpm/foo/action-setup@v4",
    "  - uses: bar/baz/action-setup",
    "  -uses: actions/setup-java@v4",
    "  - uses: \"actions/setup-ruby\"",
    "  # - uses: actions/setup-deno@v1",
    "  - uses: ./local/setup-node",
    "  - uses: actions/checkout@v4",
    "  - uses: a/setup-.@v1",
    "  - uses: a/setup-@v1",
    "  - uses: a/b/setup-lua@v1",
    "  - uses: Actions/Setup-Node@v1",
    "  uses: actions/setup-php@v1 # c",
    "\t- uses: actions/setup-zig@v1",
  ].join("\n") });
  await scan("workflow run indent", { ".github/workflows/a.yml": "jobs:\n  b:\n    steps:\n      - run: |\n          docker build .\n        env:\n          X: gh\n      - name: n\n        run: |\n          npm ci\n        env: {}\n" });
  await scan("workflow run indent dash", { ".github/workflows/a.yml": "  - run: |\n    gh pr list\n" });
  await scan("workflow run inside block", { ".github/workflows/a.yml": "- run: |\n    echo hi\n    run: gh pr\n    - run: docker x\n- run: npm ci\n" });
  await scan("workflow run block shapes", { ".github/workflows/a.yml": "- run: |-\n    gh a\n\n    docker b\n\n- run: >\n    npm i\n- run: |+\n    cargo b\n- run: |2\n    go b\n- run: 'gh pr list'\n- run: \"docker build .\"\n- run:   \n    uv sync\n" });
  await scan("workflow non yml", { ".github/workflows/notes.txt": "- run: gh pr list\n", ".github/workflows/sub/ci.yml": "- run: docker x\n", ".github/workflows/z.yml": "- run: npm ci\n" });
  await scan("workflow tab block", { ".github/workflows/a.yml": "- run: |\n\tgh pr list\n" });
  await scan("workflow empty file", { ".github/workflows/a.yml": "" });
});

it("project 2", async () => {
  await scan("tv bom and nel", { ".tool-versions": "\ufeffnodejs 22.1\ngolang\u00851.24\nzig\u000b0.1\n" });
  await scan("run key vs dash siblings", { ".github/workflows/a.yml": "      - run: |\n          npm ci\n        shell: x && gh pr list\n      - name: n\n        run: |\n          uv sync\n        shell: x && docker build .\n" });
  await scan("run plus", { ".github/workflows/a.yml": "- run: |+\n    gh pr list\n" });
  await scan("run pipe digit", { ".github/workflows/a.yml": "- run: |2\n    gh pr list\n" });
  await scan("run quoted", { ".github/workflows/a.yml": "- run: 'gh pr list'\n- run: \"docker build .\"\n" });
  await scan("run blank lines", { ".github/workflows/a.yml": "- run: |\n    gh a\n\n    \n\n- run: |\n\n    docker b\n" });
  await scan("run block at eof and empty", { ".github/workflows/a.yml": "- run:\n- run: npm ci" });
  await scan("pkg array scripts", { "package.json": JSON.stringify({ scripts: ["gh release create"], engines: ["node"], dependencies: ["playwright"] }) });
  await scan("pkg integer keys", { "package.json": JSON.stringify({ scripts: { b: "gh x", "1": "docker x", a: "uv x" } }) });
  await scan("pkg duplicate keys", { "package.json": '{"scripts":{"a":"gh x","b":"docker y","a":"uv z"}}' });
  await scan("pkg empty ranges", { "package.json": JSON.stringify({ engines: { node: "" }, devDependencies: { playwright: "" } }) });
  await scan("pkg packageManager shapes", { "package.json": JSON.stringify({ packageManager: "weirdpm@1.0.0+sha512.abc+x" }), "apps/a/package.json": JSON.stringify({ packageManager: "pnpm" }), "apps/b/package.json": JSON.stringify({ packageManager: "+sha" }), "apps/c/package.json": JSON.stringify({ packageManager: "yarn@" }), "apps/d/package.json": JSON.stringify({ packageManager: "Yarn@4" }) });
  await scan("pkg scripts version checks", { "package.json": JSON.stringify({ scripts: { a: "node --version", b: "node -v && gh --help", c: "docker version", d: "FOO=1 npm ci && cd x && uv sync | tee y; cargo build", e: "echo hi; python <<EOF\ngh x\nEOF" } }) });
  await scan("pkg dep vs devdep order", { "package.json": '{"devDependencies":{"@playwright/test":"1.0"},"dependencies":{"playwright":"2.0"}}' });
  await scan("pkg engines npm and non object", { "package.json": JSON.stringify({ engines: { npm: ">=10", node: ">=22" } }), "apps/x/package.json": JSON.stringify({ engines: "node", scripts: "gh x", dependencies: null }) });
  await scan("pkg wrong shapes", { "package.json": "null", "apps/a/package.json": "\"str\"", "apps/b/package.json": "7", "apps/c/package.json": "", "apps/d/package.json": "{}" });
  await scan("pkg workspace order", { "packages/b/package.json": JSON.stringify({ scripts: { x: "gh x" } }), "packages/a/package.json": JSON.stringify({ scripts: { x: "gh y" } }), "apps/z/package.json": JSON.stringify({ scripts: { x: "docker y" } }), "package.json": JSON.stringify({ scripts: { x: "uv y" } }) });
  await scan("pkg nested too deep", { "apps/a/b/package.json": JSON.stringify({ scripts: { x: "gh y" } }), "apps/package.json": JSON.stringify({ scripts: { x: "docker y" } }), "packages/a/package.jsonx": "{}" });
  await scan("pkg path why in workspace", { "apps/a/package.json": JSON.stringify({ packageManager: "pnpm@9", engines: { node: "22" }, scripts: { x: "gh y" }, dependencies: { playwright: "1" } }) });
  await scan("marker dir and empty", { "Dockerfile/x": "FROM", "go.mod": "", "Gemfile": "x" });
  await scan("marker empty text reader files", { "package.json": "", "mise.toml": "" });
  await scan("trailing slashes", { "go.mod": "x" }, `${P}///`);
  await scan("root slash", { "go.mod": "x" }, "/");
  await scan("no trailing trim of spaces", { "go.mod": "x" }, `${P} `);
  await scan("candidates first wins", { "package.json": JSON.stringify({ packageManager: "weirdpm@1" }), ".tool-versions": "weirdpm 2\nzig 1\n", ".github/workflows/a.yml": "- uses: x/setup-zig@v1\n" });
});

it("project 3", async () => {
  await scan("mise version regex", { "mise.toml": "[tools]\na = \"a'b\"\nb = \"\"\nc = { version = \"22\", x = \"1\" }\nd = \"\" \"1\"\ne = 'x\"\nf=\"6\"\n  g  =  \"7\"  \n\"h\" = \"8\"\n'i\" = \"9\"\n" });
  await scan("tv extra versions and empty", { ".tool-versions": "nodejs 22 20 18\n   \n\n# only\n" });
});
