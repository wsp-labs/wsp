// SPDX-License-Identifier: AGPL-3.0-only
// The TS catalog's text helpers on edge inputs, written as tests/fixtures/text.json for the Rust hard tests. Run with
// dump.oracle.ts (see its header).
import { createHash } from "node:crypto";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import * as c from "../../../../packages/catalog/src/index.js";
import { bearerOf, mcpBearerVariable, withBearerVariables } from "../../../../packages/catalog/src/mcp-bearer.js";
import { travelledServers } from "../../../../packages/catalog/src/mcp-travelled.js";
import { HERMES_BUNDLED_SKILLS } from "../../../../packages/catalog/src/skills.js";
import { CLAUDE_MCP_OAUTH } from "../../../../packages/catalog/src/mcp-oauth.js";
import { CLAUDE_MCP_CHECK, CODEX_MCP_CHECK } from "../../../../packages/catalog/src/mcp-check.js";
import { CLAUDE_HOOKS } from "../../../../packages/catalog/src/hooks.js";
import { CODEX_HOOKS } from "../../../../packages/catalog/src/codex-hooks.js";

const OUT = fileURLToPath(new URL("../tests/fixtures/", import.meta.url));
const attempt = (f: () => unknown): unknown => {
  try {
    const v = f();
    return { ok: v === undefined ? null : v };
  } catch (e) {
    return { error: (e as Error).message };
  }
};

it("writes the text fixtures", () => {
  const jsonc = [
    '{"a":1,}',
    "[1,2,]",
    '// x\n{"a": /* c */ 1}',
    '{"u":"http://x,y", "v": "a\\"//b"}',
    '{"a":1, // c\n}',
    '{"a":[1,],}',
    '{"a":1,"b":2,"a":3}',
    "{a:1}",
    "",
    '{"a":1,,}',
    '{"a":1 /* x',
    '{"a":1} // end',
    '{"a":"\\u00e9\\n"}',
    "  [ ]  ",
    '{"__proto__": {"x": 1}}',
  ].map(text => ({ text, out: attempt(() => c.readJsonc(text).value) }));
  const numbers = '{"a":1.0,"b":1e21,"c":-0,"d":0.1,"e":12345678901234567890,"f":1e-7,"g":100,"h":-1.5e-10,"i":123456789012345680000}';
  const strings = [" ", "\u0007", "é", "😀", '"q"', "back\\slash", "\t\n\r\b\f", "\u007f", "</script>"];
  const editCases: [string, c.JsonEdit[]][] = [
    ['{\n  // keep\n  "a": 1\n}\n', [[["b"], 2]]],
    ['{\n  "a": 1,\n  // about b\n  "b": 2\n}\n', [[["b"], undefined]]],
    ['{\n  "a": 1,\n  "b": 2 // b\n}\n', [[["a"], undefined]]],
    ['{\n  "a": 1,\n  "b": 2\n}\n', [[["b"], undefined]]],
    ["{\n}\n", [[["a"], { x: [1, 2] }]]],
    ["{}", [[["a"], { x: 1 }]]],
    ['{"a": 1, "b": 2}', [[["c"], 3]]],
    ['{\n  "a": {\n    "x": 1\n  }\n}\n', [[["a"], { y: { z: true } }]]],
    ['{"arr": [1, 2]}', [[["arr", -1], 3]]],
    ['{\n  "arr": [\n    1\n  ]\n}', [[["arr", -1], { k: "v" }]]],
    ['{"arr": [1, 2]}', [[["arr", 0], "v"]]],
    ['{"arr": [1, 2]}', [[["arr", 1], undefined]]],
    ['{"a":1,"a":2}', [[["a"], 3]]],
    ['{\r\n  "a": 1\r\n}\r\n', [[["b"], { c: 1 }]]],
    ["{}", [[["__proto__"], 1]]],
    ['{"x": 1}', [[["y", "z", "w"], "deep"]]],
    ['{\n  "m": {\n    "k": 0\n  }\n}\n', [[["m", "a"], 1], [["m", "b"], { n: null }]]],
    ['{"a": 1, /* c */ "b": 2}', [[["a"], undefined]]],
    ['{"a": {/* in */ "x": 1}}', [[["a"], 2]]],
    ['{\n  "a": 1, // one\n  "b": 2\n}', [[["a"], undefined]]],
    ['{"a": 1,}', [[["b"], 2]]],
    ['{\n  "a": 1,\n}\n', [[["b"], 2]]],
    ['{"a": 1}', [[["missing", "x"], undefined]]],
    ["[1]", [[["a"], 1]]],
    ['{\n  "projects": {\n    "/p": {}\n  }\n}\n', [[["projects", "/p", "mcpServers", "s"], { command: "x" }]]],
  ];
  const canonical = [{ b: 1, a: [3, { d: 1, c: 2 }], "é": null, "😀": 1, "￿": 2 }, [1, "x"], "s", 1.5, null, true, {}];
  const cksum = ["", "a", "abc\n", "x".repeat(300), "héllo"].map(t => ({ text: t, out: c.configSum(new TextEncoder().encode(t)) }));
  const answers = [
    '{"id":1,"result":{"codexHome":"/h"}}',
    "noise",
    '  {"id":2,"error":{"code":-32601,"message":"unknown variant `plugin/list`","data":{"x":1}}}',
    '{"id":3,"error":{"message":5}}',
    '{"id":"4","result":{}}',
    '{"id":5,"result":[1]}',
    '{"id":1,"result":{"second":true}}',
    "{broken",
    '{"method":"note"}',
  ].join("\n");
  const shaCalls: string[] = [];
  const sha = (t: string): string => {
    shaCalls.push(t);
    return createHash("sha256").update(t).digest("hex");
  };
  const placer = (abs: string): string | undefined => (abs.includes("nope") ? undefined : `/guest${abs}`);
  const home = "/Users/me";
  const hookWords = ["~/bin/x.sh", "$HOME/a", "${HOME}/b", "/usr/bin/env", "/usr/bin/", "/opt/nope/x", "plain", "./rel", "/abs/tool"];
  const settings = JSON.stringify({ hooks: { PreToolUse: [{ matcher: "Bash", hooks: [{ type: "command", command: "~/hooks/a.sh --flag" }, { type: "command", command: "bash ~/hooks/b.sh && /opt/nope/c" }] }], Stop: [{ hooks: [{ type: "prompt", prompt: "x" }] }] } }, null, 2);
  const server = (transport: c.McpTransport, envRefs: string[] = []): c.McpServer => ({ name: "s", scope: "user", transport, envRefs });
  const credentialCases: c.McpServer[] = [
    server({ kind: "stdio", command: "npx", args: ["-y", "x"], env: {} }),
    server({ kind: "stdio", command: "npx", args: [], env: { A: "${ACME_PAT}" } }),
    server({ kind: "stdio", command: "npx", args: [], env: { A: "${HOME}/x", B: "{env:PATH}" } }),
    server({ kind: "stdio", command: "npx", args: [], env: { A: "{env:ACME_PAT}" } }),
    server({ kind: "stdio", command: "uvx", args: ["mcp-proxy-for-aws@1.7.0", "https://x"], env: {} }),
    server({ kind: "stdio", command: "uvx", args: ["mcp-proxy-for-aws@1.7.0", "--skip-auth"], env: {} }),
    server({ kind: "stdio", command: "/opt/bin/mcp-proxy-for-aws-cli", args: [], env: {} }),
    server({ kind: "stdio", command: "sh", args: ["-c", "pipx run mcp-proxy-for-aws==1.0 x"], env: {} }),
    server({ kind: "stdio", command: "npx", args: [], env: {} }, ["HOME"]),
    server({ kind: "stdio", command: "npx", args: [], env: {} }, ["ACME_PAT"]),
    server({ kind: "http", url: "https://signs.example/mcp", headers: {} }),
    server({ kind: "http", url: "https://open.example/mcp", headers: {} }),
    server({ kind: "http", url: "https://unasked.example/mcp", headers: {} }),
    server({ kind: "http", url: "https://h.example/mcp", headers: { Authorization: "Bearer " } }),
    server({ kind: "http", url: "https://h.example/mcp", headers: { Authorization: "Bearer tok" } }),
    server({ kind: "http", url: "https://h.example/mcp", headers: { "X-Key": "" } }),
    server({ kind: "http", url: "https://h.example/mcp", headers: { "X-Key": "  token  " } }),
    server({ kind: "http", url: "https://open.example/mcp", headers: {} }, ["LANG"]),
  ];
  const signsIn = (url: string): boolean | undefined => (url.includes("signs") ? true : url.includes("open") ? false : undefined);
  const out = {
    credentials: credentialCases.map(s => ({ server: s, needs: c.needsOwnCredential(s, signsIn) })),
    addressesToAsk: c.addressesToAsk([...credentialCases, server({ kind: "http", url: "https://signs.example/mcp", headers: {} }, ["ACME_PAT"])]),
    jsonc,
    numbers: { text: numbers, compact: JSON.stringify(JSON.parse(numbers)), pretty: JSON.stringify(JSON.parse(numbers), null, 2) },
    strings: strings.map(v => ({ value: v, out: JSON.stringify(v) })),
    pretty: JSON.stringify({ a: {}, b: [], c: [{}], d: { e: [1, { f: [] }] } }, null, 2),
    edits: editCases.map(([text, edits]) => ({ text, edits: edits.map(([p, v]) => ({ path: p, value: v === undefined ? "UNDEFINED" : v })), out: attempt(() => c.editJson(text, edits)) })),
    canonical: canonical.map(v => ({ value: v, out: c.MCP_SERVERS_JSON.entryOf(JSON.stringify({ mcpServers: { s: v } }), "s") ?? null })),
    bearer: {
      variable: mcpBearerVariable("WSP_MCP_X_AUTHORIZATION"),
      of: Object.fromEntries(["WSP_MCP_X_AUTHORIZATION_BEARER", "WSP_MCP__AUTHORIZATION_BEARER", "WSP_MCP_X_AUTHORIZATION", "X_BEARER", "_BEARER", "", "WSP_MCP_A_B_AUTHORIZATION_BEARER"].map(n => [n, bearerOf(n) ?? null])),
      with: withBearerVariables({ A: "1", WSP_MCP_X_AUTHORIZATION: "tok", WSP_MCP_X_AUTHORIZATION_BEARER: "own", WSP_MCP_Y_AUTHORIZATION: "y" }),
    },
    travelled: [
      [JSON.stringify({ mcpServers: { a: { command: "x" } } }), "mcpServers", undefined],
      [JSON.stringify({ projects: { "/from": { mcpServers: { f: {} } }, "/to": { mcpServers: { t: {} } } } }), "mcpServers", { from: "/from", to: "/to" }],
      [JSON.stringify({ projects: { "/to": { mcpServers: { t: {} } } } }), "mcpServers", { from: "/from", to: "/to" }],
      [JSON.stringify({ projects: {} }), "mcpServers", { from: "/from", to: "/to" }],
      [JSON.stringify({ mcpServers: [] }), "mcpServers", undefined],
      ["[1]", "mcpServers", undefined],
      ["not json", "mcpServers", undefined],
    ].map(([t, key, project]) => ({ text: t, key, project: project ?? null, out: attempt(() => travelledServers(t as string, key as string, project as never)) })),
    toml: {
      uncomment: ['a = "x # y" # z', "a = 'x # y' # z", 'a = "x \\" # y" # z', "# all", "plain", 'k = "unterminated # c'].map(l => ({ line: l, out: c.uncommentToml(l) })),
      strings: ['["a", \'b\', "c\\nd", "\\u00e9", "\\U0001F600", "\\q"]', "x = 1", '"one" "two"'].map(s => ({ text: s, out: c.tomlStrings(s) })),
      openArray: ["[", "[1, 2]", "[1,", "x", "[]"].map(v => ({ value: v, out: c.tomlOpenArray(v) })),
      rewrite: [
        ['command = "/Users/me/bin/x" # here', "/Users/me/bin/x", "/root/bin/x"],
        ["args = ['/Users/me/a', \"b\"]", "/Users/me/a", "/root/a"],
        ['k = "tab\\there"', "tab\there", "new\tline\n"],
        ['k = "same\\u0041"', "zzz", "yyy"],
        ['k = "x" # "x"', "x", "y"],
      ].map(([line, from, to]) => ({ line, from, to, out: c.rewriteTomlLine(line!, v => (v === from ? to! : v)) })),
    },
    configSum: cksum,
    appServer: {
      request: c.request(7, "plugin/read", { pluginName: "p", marketplacePath: "/m" }),
      requestNoParams: c.request(2, "plugin/list"),
      notification: c.notification("initialized"),
      initialize: c.initializeRequest(1),
      script: c.appServerScript("codex", [{ lines: [c.initializeRequest(1), c.request(2, "x")], answers: 2 }, { lines: ["it's"], answers: 1 }]),
      scriptOne: c.appServerScript("/opt/codex", [{ lines: [], answers: 0 }]),
      answers: Object.fromEntries([...c.answersOf(answers)].map(([id, a]) => [id, a])),
    },
    oauth: {
      bearer: CLAUDE_MCP_OAUTH.bearer({ name: "linear", url: "https://mcp.linear.app/mcp", headers: { b: "1", a: "2" } }, "/root/.claude-cfg", sha),
      calls: shaCalls,
    },
    check: [
      'No MCP server named "x"',
      "Error\nNo MCP server found with name: x",
      "linear:\n  Status: ✓ Connected\n",
      "  Status: ! Needs authentication",
      "Status: ✗ Failed to connect",
      "Status: something else",
      "",
      'warn\n[{"name":"x","auth_status":"o_auth"},{"name":"y","auth_status":"not_logged_in"},{"name":"z","auth_status":"bearer"}]',
      '[{"name":"x"}]',
      "[not json",
      "{}",
    ].map(o => ({ output: o, claude: CLAUDE_MCP_CHECK.auth(o, "x") ?? null, codexX: CODEX_MCP_CHECK.auth(o, "x") ?? null, codexY: CODEX_MCP_CHECK.auth(o, "y") ?? null, codexZ: CODEX_MCP_CHECK.auth(o, "z") ?? null, codexW: CODEX_MCP_CHECK.auth(o, "w") ?? null })),
    checkLines: { claude: CLAUDE_MCP_CHECK.line("it's"), codex: CODEX_MCP_CHECK.line("x") },
    hooks: {
      words: hookWords.map(w => ({ word: w, pathOf: c.pathOf(w, home) ?? null, onMachine: c.onMachine(w), runsAFile: c.runsAFile(w, home), placed: c.placeWord(w, home, placer) })),
      claude: attempt(() => CLAUDE_HOOKS.carry(settings, home, placer)),
      claudeFiles: attempt(() => c.hookFiles(CLAUDE_HOOKS, settings, home)),
      codex: CODEX_HOOKS.carry('model = "x"\n# runs after a turn\nnotify = ["python3", "~/bin/notify.py"]\n[tui]\nnotify = ["/opt/nope"]\n', home, placer),
      codexLeft: CODEX_HOOKS.carry('# doc for notify\n# more\nnotify = [\n  "/opt/tool/bin/x",\n  "a"\n]\nk = 1\n', home, placer),
      codexOpen: CODEX_HOOKS.carry('notify = ["~/x",\n', home, placer),
    },
    plugins: {
      plainValues: c.plainValues('{"installPath": "/a", "x": {"installPath":"/b\\\\c"}, "installPath" : "/d"}', "installPath"),
      idParts: Object.fromEntries(["a@b", "a@b@c", "@b", "a", ""].map(id => [id, c.pluginIdParts(id)])),
      inStore: [["~/.claude/settings.json", "/s"], ["~/.claude", "/s"], ["~/.clauder/x", "/s"], ["~", undefined], ["~/x", undefined], ["/abs", "/s"]].map(([p, store]) => ({ path: p, store: store ?? null, out: c.inStore({ stateHome: ".claude" }, p!, { home: "/h", ...(store !== undefined ? { store } : {}) }) })),
      joinPath: [["/a", "b"], ["/a/", "/b/"], ["/a", "../.."], ["a", "./b"], ["/"], []].map(parts => ({ parts, out: c.joinPath(...parts) })),
      baseName: Object.fromEntries(["/a/b", "/a/b//", "b", "", "/"].map(p => [p, c.baseName(p)])),
      bundled: HERMES_BUNDLED_SKILLS.names("a:hash\n b \n\n:x\nc:d:e\n"),
    },
  };
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, "text.json"), `${JSON.stringify(out, null, 2)}\n`);
});
