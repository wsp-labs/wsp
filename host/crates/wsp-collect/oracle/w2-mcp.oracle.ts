// SPDX-License-Identifier: AGPL-3.0-only
// Fixtures for spec/mcp.md: mcp.ts:292-464 on the edge inputs of the wave-2 audit (group d). `agents` args are catalog agent ids (null: the default list).
import { it } from "vitest";
import { MCP_AGENTS, type McpServer } from "../../../../packages/catalog/src/index.js";
import { detectMcp, mcpRemoteOf, mcpRemoteTokenFile, mcpRemoteTokens, mcpServerRow } from "../../../../packages/collect/src/detect/mcp.js";
import { type Laptop, host, plain, settle, write } from "./w2-host.js";

type Case = Record<string, unknown>;
const agentsOf = (ids: string[] | null) => (ids === null ? undefined : ids.map(id => MCP_AGENTS.find(a => a.id === id)!));
const onHost = async (name: string, laptop: Laptop, args: unknown[], run: (h: ReturnType<typeof host>) => unknown): Promise<Case> => {
  const h = host(laptop);
  const r = await settle(() => run(h));
  return { name, laptop, args, ...r, calls: h.calls };
};
const detect = (name: string, laptop: Laptop, agents: string[] | null = null) => onHost(name, laptop, [agents], h => detectMcp(h, agentsOf(agents)));
const ref = (n: string, d?: string): string => "$" + "{" + n + (d !== undefined ? ":-" + d : "") + "}";
const claude = (servers: unknown, projects?: unknown): string => JSON.stringify({ mcpServers: servers, ...(projects !== undefined ? { projects } : {}) });
const T = (n: string) => n.padEnd(32, "0").slice(0, 32);
const u = (n: string) => `https://${n}.example/mcp`;
const hashOf = (url: string): string => mcpRemoteOf(["mcp-remote", url])!.hash;
const STORE = "~/.mcp-auth/mcp-remote-v1";
const RM = (url: string) => ({ command: "npx", args: ["mcp-remote", url] });

it("mcp fixtures", async () => {
  const fixture: Record<string, Case[]> = {};
  const d: Case[] = (fixture["detect_mcp"] = []);

  d.push(await detect("row and store key order as the TS writes them", { files: { "~/.claude.json": claude({ gh: { command: "npx", args: ["-y", "pkg"], env: { GITHUB_TOKEN: "abc" } }, n: { type: "http", url: "https://n.example/mcp" } }), [`${STORE}/${T("a")}_tokens.json`]: 10, "~/.mcp-auth/mcp-remote-0.1.1/x": 1 } }));
  d.push(await detect("the same name in the user scope and the home folder's entry", { files: { "~/.claude.json": claude({ a: { command: "npx", args: ["p"] }, z: { command: "npx", args: ["p"] } }, { "/Users/dev": { mcpServers: { a: { command: "npx", args: ["q"] }, m: { command: "npx", args: ["q"] } } } }) } }));
  d.push(await detect("the same name under three agents", { files: { "~/.gemini/settings.json": JSON.stringify({ mcpServers: { x: { command: "npx", args: ["g"] } } }), "~/.claude.json": claude({ x: { command: "npx", args: ["c"] } }), "~/.codex/config.toml": '[mcp_servers.x]\ncommand = "npx"\nargs = ["t"]\n' } }));
  d.push(await detect("an empty agent list is not the default list", { files: { "~/.claude.json": claude({ a: { command: "npx" } }), [`${STORE}/x`]: 3 } }, []));
  d.push(await detect("one agent named by id", { files: { "~/.claude.json": claude({ a: { command: "npx" } }), "~/.codex/config.toml": '[mcp_servers.t]\ncommand = "npx"\n' } }, ["codex"]));
  d.push(await detect("servers an agent's file switches off still get a row", { files: {
    "~/.codex/config.toml": '[mcp_servers.off]\ncommand = "npx"\nenabled = false\n',
    "~/.config/opencode/opencode.json": JSON.stringify({ mcp: { off2: { type: "local", command: ["npx", "x"], enabled: false } } }),
    "~/.claude.json": claude({ a: { command: "npx" } }, { "/Users/dev": { disabledMcpServers: ["a", "b"], mcpServers: { b: { command: "npx" } } } }),
  } }));
  d.push(await detect("secrets, keys and signIn from one definition", { files: { "~/.claude.json": claude({
    mix: { command: "node", args: ["/Users/dev/x.js", "--api-key", "v1", "API_KEY=a", "API_KEY=b", "pos", "--token=t"], env: { X_TOKEN: "tok", Y_KEY: "yk", PLAIN: "p" } },
    dup: { command: "npx", args: ["p", "--api-key", "a", "--api-key", "b", "--token=1", "--token=2"] },
  }) } }));
  d.push(await detect("signIn corners", { files: { "~/.claude.json": claude({
    flagonly: { command: "npx", args: ["p", "--api-key", "v"] },
    posonly: { command: "npx", args: ["p", "positional"] },
    envkey: { command: "npx", args: ["p"], env: { MY_KEY: "k" } },
    envtoken: { command: "npx", args: ["p"], env: { MY_TOKEN: "k" } },
    envboth: { command: "npx", args: ["p", "--api-key", "v"], env: { MY_TOKEN: "k" } },
    envmonkey: { command: "npx", args: ["p"], env: { MONKEY: "k" } },
  }) } }));
  d.push(await detect("what a secret-named env value turns into", { files: {
    "~/.claude.json": claude({
      dir: { command: "npx", env: { A_KEY: "/Users/dev/d" } },
      rel: { command: "npx", env: { A_KEY: "rel/path" } },
      tildemissing: { command: "npx", env: { A_KEY: "~/missing" } },
      tildefile: { command: "npx", env: { A_KEY: "~/f.json" } },
      samefile: { command: "npx", env: { A_KEY: "~/f.json", B_TOKEN: "/Users/dev/f.json" } },
      zero: { command: "npx", env: { A_KEY: "~/zero.json" } },
      outside: { command: "npx", env: { CREDENTIALS_PATH: "/etc/creds.json" } },
      homeexact: { command: "npx", env: { A_KEY: "/Users/dev" } },
      empty: { command: "npx", env: { A_KEY: "" } },
      nonsecret_home: { command: "npx", env: { PLAIN: "~/f.json" } },
      unicode: { command: "npx", env: { A_KEY: "é😀" } },
    }),
    "~/d/x": 5, "~/f.json": 1500, "~/zero.json": 0, "/etc/creds.json": 10,
  } }));
  d.push(await detect("http definitions: headers, notes and odd urls", { files: { "~/.claude.json": claude({
    hdr: { type: "http", url: "https://a.example/mcp", headers: { Authorization: "Bearer é" } },
    hdrkey: { type: "http", url: "https://a.example/mcp", headers: { "X-API-Key": "k", Authorization: "x" } },
    plain: { type: "http", url: "https://a.example/mcp" },
    odd: { type: "http", url: "https://A.Example:443/a b?x=1#f" },
    badurl: { type: "http", url: "not a url" },
    mac: { type: "http", url: "https://a.example/Applications/x" },
    file: { type: "http", url: "file:///a/b" },
    v6: { type: "http", url: "http://[::1]:8080/a" },
    login: { type: "http", url: "https://U:p@x.example/p?k=v#f" },
    slash: { type: "http", url: "https://x.example/" },
    bare: { type: "http", url: "https://x.example" },
    monkey: { type: "http", url: "https://x.example/a", headers: { "X-Monkey": "v" } },
  }) } }));
  d.push(await detect("http header references", { files: { "~/.claude.json": claude({
    refd: { type: "http", url: "https://a.example/mcp", headers: { Authorization: "Bearer " + ref("MY_KEY") } },
    ref2: { type: "http", url: "https://a.example/mcp", headers: { "X-Thing": ref("MY_THING", "d") } },
    stdioref: { command: "npx", args: ["p"], env: { MY_KEY: ref("MY_KEY") } },
    argref: { command: "npx", args: ["p", "--x", ref("A_KEY")] },
  }) } }));
  d.push(await detect("Codex references", { files: { "~/.codex/config.toml": [
    '[mcp_servers.s]', 'url = "https://s.example/mcp"', 'bearer_token_env_var = "S_KEY"',
    '[mcp_servers.t]', 'url = "https://t.example/mcp"', 'http_headers = { "X-Id" = "abc" }',
    '[mcp_servers.u]', 'url = "https://u.example/mcp"',
    '[mcp_servers.s2]', 'command = "npx"', 'args = ["p"]', 'env_vars = ["MY_KEY", "OTHER"]',
    '[mcp_servers.t2]', 'url = "https://t2.example/mcp"', 'env_http_headers = { "X-Id" = "HDR_TOKEN" }', ''].join("\n") } }));

  const cfg = claude({ a: RM(u("a")), b: RM(u("a")), c: RM(u("c")), nope: RM(u("zz")) });
  d.push(await detect("two servers share one mcp-remote url", { files: { "~/.claude.json": cfg, [`${STORE}/${hashOf(u("a"))}_tokens.json`]: 1400 } }));
  d.push(await detect("three tokens, one matched", { files: { "~/.claude.json": claude({ c: RM(u("c")) }), [`${STORE}/${hashOf(u("c"))}_tokens.json`]: 700, [`${STORE}/${T("e1")}_tokens.json`]: 100, [`${STORE}/${T("e2")}_tokens.json`]: 200 } }));
  d.push(await detect("two tokens, one matched, one of zero bytes", { files: { "~/.claude.json": claude({ c: RM(u("c")) }), [`${STORE}/${hashOf(u("c"))}_tokens.json`]: 0, [`${STORE}/${T("e1")}_tokens.json`]: 100 } }));
  d.push(await detect("one token, none matched", { files: { "~/.claude.json": claude({}), [`${STORE}/${T("e1")}_tokens.json`]: 1536 } }));
  d.push(await detect("one token matched under two agents", { files: { "~/.claude.json": claude({ a: RM(u("a")) }), "~/.gemini/settings.json": JSON.stringify({ mcpServers: { a: RM(u("a")) } }), [`${STORE}/${hashOf(u("a"))}_tokens.json`]: 5 } }));
  const store = (name: string, files: Record<string, number | string>) => detect(name, { files: { "~/.claude.json": claude({}), ...files } });
  d.push(await store("store: only a file outside mcp-remote-v1", { "~/.mcp-auth/other.txt": 5 }));
  d.push(await store("store: only a lock file", { [`${STORE}/${T("a")}_lock.json`]: 5 }));
  d.push(await store("store: a lock file and a token", { [`${STORE}/${T("a")}_lock.json`]: 5, [`${STORE}/${T("a")}_tokens.json`]: 50 }));
  d.push(await store("store: which names are older folders", { "~/.mcp-auth/mcp-remote-0.1.37/a": 5, "~/.mcp-auth/mcp-remote-1/a": 7, "~/.mcp-auth/mcp-remote-/a": 1, "~/.mcp-auth/mcp-remote-v1x/a": 2, "~/.mcp-auth/mcp-remote-v2/a": 3 }));
  d.push(await store("store: older entries beside a token, a file counts as one", { "~/.mcp-auth/mcp-remote-2.0.0/a": 5, "~/.mcp-auth/mcp-remote-1.5.0": 7, [`${STORE}/${T("a")}_tokens.json`]: 50 }));
  d.push(await store("store: names that are neither a token nor a lock still count in the bytes", { [`${STORE}/${"A".repeat(32)}_tokens.json`]: 40, [`${STORE}/${"a".repeat(31)}_tokens.json`]: 41, [`${STORE}/${"a".repeat(33)}_tokens.json`]: 42, [`${STORE}/${T("b")}_tokens.json.bak`]: 43, [`${STORE}/${"A".repeat(32)}_lock.json`]: 44, [`${STORE}/${T("b")}_client_info.json`]: 45, [`${STORE}/sub/deep.json`]: 46 }));
  d.push(await store("store: one token of zero bytes", { [`${STORE}/${T("a")}_tokens.json`]: 0 }));
  d.push(await store("store: two tokens of 1024 bytes", { [`${STORE}/${T("a")}_tokens.json`]: 1024, [`${STORE}/${T("b")}_tokens.json`]: 1024 }));
  d.push(await store("store: excludes keep listing order, older folders then locks", { "~/.mcp-auth/mcp-remote-0.9.9/a": 1, "~/.mcp-auth/mcp-remote-0.1.1/a": 1, [`${STORE}/${T("b")}_lock.json`]: 1, [`${STORE}/${T("a")}_lock.json`]: 1 }));
  d.push(await store("store: ~/.mcp-auth is a file", { "~/.mcp-auth": 3 }));

  d.push(await detect("a server that does not fit keeps no needs text and no dependency notes", { files: { "~/.claude.json": claude({ macdep: { command: "/Applications/X.app/x", args: ["~/there", "~/gone"], env: { E: "~/envgone" } } }), "~/there": 5 } }));
  d.push(await detect("dependency notes: order, one each, carried paths left out", { files: { "~/.claude.json": claude({
    deps: { command: "npx", args: ["~/there", "~/gone", "--dir=~/dir"], cwd: "~/cwdthere", env: { E: "~/envgone", F: "~/there" } },
    secretdep: { command: "npx", args: ["~/there"], env: { A_KEY: "~/there" } },
    secretother: { command: "npx", args: ["~/there", "~/other"], env: { A_KEY: "~/there" } },
    gonehome: { command: "npx", args: ["~/gone"], env: { A_KEY: "k" } },
  }), "~/there": 5, "~/dir/x": 1, "~/cwdthere/x": 1, "~/other": 2 } }));
  d.push(await detect("a store folder moves the agent's own file", { files: { "~/cfg/.claude.json": claude({ s: { command: "npx" } }), "~/.claude.json": claude({ h: { command: "npx" } }) }, stores: { claude: "/Users/dev/cfg" } }));
  d.push(await detect("odd server names go into the id as written", { files: { "~/.claude.json": claude({ "a/b": { command: "npx" }, "": { command: "npx" }, "my server": { command: "npx" } }) } }));

  const tokensCase = (name: string, files: Record<string, number | string>) => onHost(name, { files }, [], h => mcpRemoteTokens(h));
  fixture["mcp_remote_tokens"] = [
    await tokensCase("tokens by hash in listing order, a lock and a client file left out", { [`${STORE}/${T("b")}_tokens.json`]: '{"access_token":"x"}', [`${STORE}/${T("a")}_tokens.json`]: 700, [`${STORE}/${T("a")}_lock.json`]: 5, [`${STORE}/${T("a")}_client_info.json`]: 90 }),
    await tokensCase("no store", {}),
    await tokensCase("a file where the folder should be", { "~/.mcp-auth": 3 }),
    await tokensCase("a zero-byte token", { [`${STORE}/${T("a")}_tokens.json`]: 0 }),
    await tokensCase("an upper-case name is no token", { [`${STORE}/${"A".repeat(32)}_tokens.json`]: 40 }),
  ];

  const tf = (name: string, hash: string, env: Record<string, string>, laptop: Laptop = {}) => onHost(name, laptop, [hash, env], h => mcpRemoteTokenFile(h, hash, env));
  fixture["mcp_remote_token_file"] = [
    await tf("no variable", "abc", {}),
    await tf("an empty variable", "abc", { MCP_REMOTE_CONFIG_DIR: "" }),
    await tf("a folder with a trailing slash is kept as written", "abc", { MCP_REMOTE_CONFIG_DIR: "/cfg/" }),
    await tf("a tilde folder is not expanded", "abc", { MCP_REMOTE_CONFIG_DIR: "~/c" }),
    await tf("a plain folder", "abc", { MCP_REMOTE_CONFIG_DIR: "/cfg" }),
  ];

  const row = (name: string, agent: string, server: McpServer, tokens: Record<string, number>, laptop: Laptop) => onHost(name, laptop, [agent, server, tokens], h => mcpServerRow(h, MCP_AGENTS.find(a => a.id === agent)!, server, new Map(Object.entries(tokens))));
  const S = (name: string, command: string, args: string[], env: Record<string, string> = {}, cwd?: string): McpServer => ({ name, scope: "user", transport: { kind: "stdio", command, args, env, ...(cwd !== undefined ? { cwd } : {}) }, envRefs: [] });
  fixture["mcp_server_row"] = [
    await row("a secret file under home travels on the row", "claude", S("gsc", "uvx", ["mcp-search-console"], { GSC_CREDENTIALS_PATH: "/Users/dev/.config/gsc/creds.json" }), {}, { files: { "~/.config/gsc/creds.json": 2100 } }),
    await row("a home-scope server keeps the local mark", "claude", { ...S("z", "npx", ["mcp-remote", u("a")]), scope: "home" }, { [hashOf(u("a"))]: 1400 }, {}),
    await row("an http server of an agent with no sign-in note", "codex", { name: "h", scope: "user", transport: { kind: "http", url: "https://h.example/x", headers: { Authorization: "Bearer é" } }, envRefs: ["H_KEY"] }, {}, {}),
    await row("env refs on a stdio definition each get a reads line", "codex", { ...S("e", "npx", ["p"]), envRefs: ["A", "A", "B_TOKEN"] }, {}, {}),
    await row("a token of zero bytes is a saved sign-in", "claude", S("z", "npx", ["mcp-remote", u("a")]), { [hashOf(u("a"))]: 0 }, {}),
    await row("a fit failure with a missing home path", "claude", S("m", "/Applications/X.app/x", ["~/gone"]), {}, {}),
  ];

  write("mcp", fixture);
});
