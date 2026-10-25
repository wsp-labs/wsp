// SPDX-License-Identifier: AGPL-3.0-only
// The TS catalog's own answers, written as the fixtures the Rust tests compare against. Every module object a row
// holds is written by its id, every regex by its source, so the Rust structs serialize to the same JSON. Run from the
// repository root:
//   pnpm exec vitest run --config host/crates/wsp-catalog/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import * as c from "../../../../packages/catalog/src/index.js";
import { BWRAP_APPARMOR } from "../../../../packages/catalog/src/bwrap-apparmor.js";
import { BASE_IMAGE_COMMANDS } from "../../../../packages/catalog/src/base-image.js";
import { MAC_BIN_DIRS, MAC_BREW, MAC_ONLY } from "../../../../packages/catalog/src/mac-paths.js";
import { CURSOR } from "../../../../packages/catalog/src/linux-casks.js";
import { HERMES_BUNDLED_SKILLS, CLAUDE_PLUGIN_SKILLS } from "../../../../packages/catalog/src/skills.js";
import { CLAUDE_MCP_OAUTH } from "../../../../packages/catalog/src/mcp-oauth.js";
import { CLAUDE_MCP_LOGIN, CODEX_MCP_LOGIN, GEMINI_MCP_LOGIN, OPENCODE_MCP_LOGIN } from "../../../../packages/catalog/src/mcp-login.js";
import { CLAUDE_MCP_CHECK, CODEX_MCP_CHECK } from "../../../../packages/catalog/src/mcp-check.js";
import { CLAUDE_CONTEXT, CODEX_CONTEXT, GEMINI_CONTEXT, HERMES_CONTEXT, OPENCODE_CONTEXT, PI_CONTEXT } from "../../../../packages/catalog/src/context.js";
import { CLAUDE_HOOKS } from "../../../../packages/catalog/src/hooks.js";
import { CODEX_HOOKS } from "../../../../packages/catalog/src/codex-hooks.js";
import { CLAUDE_PLUGIN_SHELF } from "../../../../packages/catalog/src/plugins-claude.js";
import { CODEX_PLUGIN_SHELF } from "../../../../packages/catalog/src/plugins-codex.js";

/** The release module's two scripts, reached through its install: a road with assets takes the asset script, one
 * without takes the listing script with its standing pin. */
const release = c.ROAD_MODULES.release as c.RoadModule<c.InstallRoad>;
const assetInstallForOracle = (name: string, repo: string, tag: string, assets: c.ReleaseAssets, go: string | undefined): unknown =>
  release.install({ road: "release", repo, version: tag, assets, ...(go !== undefined ? { go } : {}) }, name);
const releaseInstallForOracle = (name: string, repo: string, tag: string, pin: c.ToolPin | undefined, go: string | undefined): unknown =>
  release.install({ road: "release", repo, version: tag, ...(pin !== undefined ? { pin } : {}), ...(go !== undefined ? { go } : {}) }, name);

const OUT = fileURLToPath(new URL("../tests/fixtures/", import.meta.url));

const ids = new Map<unknown, string>([
  [c.MCP_SERVERS_JSON, "mcp-servers-json"],
  [c.GEMINI_SETTINGS_JSON, "gemini-settings-json"],
  [c.OPENCODE_JSON, "opencode-json"],
  [c.CODEX_TOML, "codex-toml"],
  [CLAUDE_MCP_CHECK, "claude"],
  [CODEX_MCP_CHECK, "codex"],
  [CLAUDE_MCP_LOGIN, "claude"],
  [CODEX_MCP_LOGIN, "codex"],
  [OPENCODE_MCP_LOGIN, "opencode"],
  [GEMINI_MCP_LOGIN, "gemini"],
  [CLAUDE_MCP_OAUTH, "claude"],
  [CLAUDE_CONTEXT, "claude"],
  [CODEX_CONTEXT, "codex"],
  [GEMINI_CONTEXT, "gemini"],
  [OPENCODE_CONTEXT, "opencode"],
  [PI_CONTEXT, "pi"],
  [HERMES_CONTEXT, "hermes"],
  [CLAUDE_HOOKS, "claude"],
  [CODEX_HOOKS, "codex"],
  [c.CLAUDE_PLUGINS, "claude"],
  [CLAUDE_PLUGIN_SHELF, "claude"],
  [CODEX_PLUGIN_SHELF, "codex"],
  [HERMES_BUNDLED_SKILLS, "hermes"],
]);
const statusRow = new Map<unknown, string>(Object.entries(c.SIGN_IN_ROWS as Record<string, c.SignIn>).flatMap(([k, s]) => (s.status === undefined ? [] : [[s.status, k] as const])));

/** A TS value as the Rust structs serialize it. */
function ser(v: unknown, key = ""): unknown {
  if (ids.has(v)) return ids.get(v);
  if (statusRow.has(v)) {
    const s = v as c.StatusCheck;
    return { command: s.command, ...(s.typed !== undefined ? { typed: s.typed } : {}), reader: statusRow.get(v) };
  }
  if (v instanceof RegExp) return v.source;
  if (typeof v === "function") {
    if (key === "trusts") return "codex";
    throw new Error(`no id for the function at ${key}`);
  }
  if (Array.isArray(v)) return v.map(x => ser(x, key));
  if (v instanceof Set) return [...v].map(x => ser(x, key));
  if (v instanceof Map) return Object.fromEntries([...v].map(([k, x]) => [k, ser(x, k)]));
  if (typeof v === "object" && v !== null) return Object.fromEntries(Object.entries(v).filter(([, x]) => x !== undefined).map(([k, x]) => [k, ser(x, k)]));
  return v;
}

const write = (name: string, value: unknown): void => {
  mkdirSync(OUT, { recursive: true });
  writeFileSync(join(OUT, name), `${JSON.stringify(value, null, 2)}\n`);
};

const tryText = (f: () => unknown): unknown => {
  try {
    return { ok: f() };
  } catch (e) {
    return { error: (e as Error).message };
  }
};

it("writes the catalog fixtures", () => {
  // --- the table and its readers ------------------------------------------------------------------------------
  write("catalog.json", {
    entries: c.CATALOG.map(e => ser(e)),
    perEntry: Object.fromEntries(
      c.CATALOG.map(e => [
        e.id,
        {
          installLine: tryText(() => c.installLine(e)),
          installShown: tryText(() => c.installShown(e)),
          smoke: c.smokeOf(e),
          sizeBytes: c.sizeBytes(e.size) ?? null,
          ...(e.kind === "tool" ? { installAfter: c.installAfter(e) ?? null, baseNote: { none: c.baseNote(e, undefined, "darwin"), v99: c.baseNote(e, "v99.1.2", "darwin"), linux99: c.baseNote(e, "99.1", "linux"), same: e.major === undefined ? null : c.baseNote(e, `v${e.major.version}.4`, "darwin") } } : {}),
          ...(e.kind === "agent" ? { guestEnv: c.guestEnv(e), setupNeedRows: c.setupNeedRows(e.id), aptNeedRows: c.aptNeedRows(e.id), signInWays: c.signInWaysOf(e.id), sharedOn: c.sharedOn(e.id) ?? null, agentName: c.agentName(e.id), mark: c.agentMark(e.id) ?? null, mcpSwitch: c.mcpSwitch(e.id) } : {}),
        },
      ]),
    ),
    catalogAgents: c.CATALOG_AGENTS.map(a => a.id),
    catalogTools: c.CATALOG_TOOLS.map(t => t.id),
    defaultAgent: c.DEFAULT_AGENT.id,
    mcpAgents: c.MCP_AGENTS.map(a => a.id),
    mcpAgentIds: c.MCP_AGENT_IDS,
    baseFloor: c.BASE_FLOOR.map(e => e.id),
    floorBytes: { compiler: c.floorBytes(true), none: c.floorBytes(false) },
    loginRows: c.LOGIN_ROWS.map(r => ({ id: r.id, entry: r.entry.id, signIn: ser(r.signIn), ...(r.keys !== undefined ? { keys: r.keys } : {}) })),
    toolFor: Object.fromEntries(["rg", "cli/cli", "awscli", "npm", "openjdk@21", "cargo", "rustup", "rustup-init", "claude", "node@22", "nodejs", "python3", "python@3.12", "fd-find", "fdfind", "unzip", "docker compose", "docker-compose", "bunx", "chromium", "playwright", "@playwright/test", "gh", "bazelisk", "", "nothing"].map(p => [p, c.catalogToolFor(p)?.id ?? null])),
    toolForDependency: Object.fromEntries(["playwright", "@playwright/test", "playwright-core", "chromium", "typescript"].map(p => [p, { npm: c.catalogToolForDependency("npm", p)?.id ?? null, pnpm: c.catalogToolForDependency("pnpm", p)?.id ?? null }])),
    toolByRoad: Object.fromEntries((
      [["brew", "go"], ["brew", "awscli"], ["npm", "pnpm"], ["npm", "agent-browser"], ["apt", "git"], ["apt", "unzip"], ["apt", "xz-utils"], ["uv", "ruff"], ["release", "cli/cli"], ["npm", "nothing"]] as const
    ).map(([road, p]) => [`${road} ${p}`, c.catalogToolByRoad(road, p)?.id ?? null])),
    idOfRow: Object.fromEntries(["agents/claude", "agents/mcp/linear", "tools/brew/go", "tools/npm/pnpm", "tools/apt/ripgrep", "tools/brew/unknown-thing", "logins/gh", "agents/", "tools/"].map(id => [id, c.catalogIdOfRow({ id }) ?? null])),
    loginSignIn: Object.fromEntries(["gh", "logins/gh", "kube", "logins/kube", "hermes-keys", "logins/hermes-keys", "logins/a/b", "claude", "nothing"].map(r => [r, c.loginSignIn(r) === undefined ? null : ser(c.loginSignIn(r))])),
    sharedAgentsOn: c.sharedAgentsOn(["claude", "codex", "gemini", "codex", "nothing"]),
    agentName: Object.fromEntries(["claude", "gh", "nothing"].map(id => [id, c.agentName(id)])),
    harnessLine: {
      plain: c.harnessLine("claude", "claude mcp list"),
      folder: c.harnessLine("claude", "claude mcp list", { folder: "/home/me/it's" }),
      store: c.harnessLine("claude", "claude mcp list", { stores: { claude: "/w/logins/claude", codex: "/w/c" } }),
      both: c.harnessLine("codex", "codex mcp list", { stores: { codex: "/w/logins/codex" }, folder: "/p" }),
      noVariable: c.harnessLine("gemini", "gemini", { stores: { gemini: "/w/g" } }),
    },
    nodeAtLeast: ["22.13", "20", "18.0", "x.y"].map(f => c.nodeAtLeast(f)),
    pinnedNote: c.pinnedNote("10.1.0", "11.9.0"),
    atCatalogPin: [
      [{ road: "npm", package: "pnpm" }, undefined],
      [{ road: "npm", package: "pnpm" }, "10.1.0"],
      [{ road: "npm", package: "pnpm" }, "11.9.0"],
      [{ road: "npm", package: "pnpm" }, "12.0.0"],
      [{ road: "npm", package: "agent-browser" }, "0.20.0"],
      [{ road: "npm", package: "prettier" }, "1.0.0"],
      [{ road: "pnpm", package: "pnpm" }, "1.0.0"],
      [{ road: "apt", packages: ["git"] }, "1.0"],
    ].map(([road, asked]) => ({ road, asked: asked ?? null, out: c.atCatalogPin(road as c.InstallRoad, asked as string | undefined) })),
    belowLine: (() => {
      const at = c.atCatalogPin({ road: "npm", package: "pnpm" }, "10.1.0");
      return c.belowLine(at.road, at.below!, "pnpm", c.installHomes());
    })(),
    serverSignInRoad: Object.fromEntries(
      (["claude", "codex", "opencode", "gemini", "pi"] as const).flatMap(a => (["here", "relay", "none"] as const).map(reach => [`${a} ${reach}`, c.serverSignInRoad(a, "linear", reach) ?? null])),
    ),
    sizeMethods: c.SIZE_METHODS,
    historyFormats: c.HISTORY_FORMATS,
    unmarked: c.UNMARKED_AGENTS,
    bwrapApparmor: BWRAP_APPARMOR,
  });

  // --- roads, casks, pins and the brew constants --------------------------------------------------------------
  const r = c as unknown as Record<string, unknown>;
  write("roads.json", {
    constants: Object.fromEntries(
      [
        "MIB", "GUEST_HOME", "HOME_BIN", "LOCAL_BIN", "APT_ENV", "CLAUDE_CONFIG_REL", "GUEST_REWRITES", "CLAUDE_CONFIG_DIR", "CLAUDE_PLUGIN_SEED", "CLAUDE_KEY_FILE", "CLAUDE_CODE", "CLAUDE_LATEST", "CLAUDE_INSTALL", "GOLDEN_SETUP", "GOLDEN_SMOKE", "UV", "UV_INSTALL", "NODE_RELEASES", "NODE_PATH_LINE", "NODE_BIN", "COMPOSE", "COMPOSE_PLUGIN", "DOCKER_INSTALL", "PYTHON_INSTALL", "RUSTUP", "RUSTUP_INSTALL", "FD_INSTALL", "YARN_VERSION", "YARN_INSTALL", "HERMES", "HERMES_INSTALL", "PLAYWRIGHT", "PLAYWRIGHT_INSTALL", "SWIFT", "SWIFT_INSTALL", "OP_INSTALL",
        "NET_READ_S", "NET_RETRIES", "CURL_NET", "CARGO_BIN", "APT_BIN", "APT_INDEX", "HOMEBREW_STEP", "APT_UPDATE", "HOMEBREW", "LINUXBREW_HOME", "BREW_PREFIX", "BREW_REPO", "BREW_ENV", "BREW", "BREW_REAL", "FROM_A_READABLE_DIR", "LINUXBREW_SHIM", "BREW_SHIM_PRESENT", "ASSET_ARCH", "ASSET_SKIPPED", "INSTALL_HOMES", "ROADS", "TOOL_PREFIX",
      ].map(k => {
        if (r[k] === undefined) throw new Error(`${k} is not exported`);
        return [k, r[k]];
      }),
    ),
    nodeInstallScript: { floor22: c.nodeInstallScript(22, c.NODE_RELEASES[22]), floor18on20: c.nodeInstallScript(18, c.NODE_RELEASES[20]) },
    casks: c.LINUX_CASKS,
    cursorCask: CURSOR,
    releasePins: c.RELEASE_PINS,
    pinnedRelease: { gh: c.pinnedRelease("cli/cli", "github.com/cli/cli/v2/cmd/gh"), mise: c.pinnedRelease("jdx/mise"), missing: tryText(() => c.pinnedRelease("nobody/nothing")) },
    macPaths: { MAC_BREW, MAC_ONLY, MAC_BIN_DIRS },
    baseImageCommands: [...BASE_IMAGE_COMMANDS],
    rewrittenRel: Object.fromEntries([".claude/settings.json", ".claude", ".claude.json", ".claude.jsonx", ".claudex/a", "x/.claude/a", ""].map(p => [p, c.rewrittenRel(p)])),
    pinCheckLine: c.pinCheckLine("$asset", "$tag", "0123456789abcdef0123456789abcdef"),
    goBinary: Object.fromEntries(["github.com/cli/cli/v2/cmd/gh", "github.com/x/y/v2", "github.com/x/y/v2@v2.1.0", "github.com/x/y@v1.0.0", "v2", "github.com/x/v0", "example.com/tool"].map(m => [m, c.goBinary(m)])),
    formulaShortName: Object.fromEntries(["go", "felixkratz/formulae/sketchybar", "a/b"].map(f => [f, c.formulaShortName(f)])),
    brewHasCheck: c.brewHasCheck("bat", "go"),
    asLinuxbrew: c.asLinuxbrew("install bat"),
    asLinuxbrewScript: c.asLinuxbrewScript("brew update\nbrew upgrade"),
  });

  // --- every road module against sample roads -----------------------------------------------------------------
  const pin = { tag: "v1.0.0", sha256: "a".repeat(64) };
  const samples: [string, c.InstallRoad, string][] = [
    ["brew core", { road: "brew", formula: "go" }, "go"],
    ["brew tap", { road: "brew", formula: "felixkratz/formulae/sketchybar" }, "sketchybar"],
    ["npm", { road: "npm", package: "prettier" }, "prettier"],
    ["npm at", { road: "npm", package: "pnpm", version: "11.9.0" }, "pnpm"],
    ["npm pinned", { road: "npm", package: "x", pin: { tag: "1.2.3" } }, "x"],
    ["npm ignore scripts", { road: "npm", package: "@scope/x", ignoreScripts: true }, "x"],
    ["pnpm", { road: "pnpm", package: "@scope/tool", version: "2.0.0" }, "tool"],
    ["bun", { road: "bun", package: "cowsay" }, "cowsay"],
    ["uv", { road: "uv", package: "ruff", version: "0.6.0" }, "ruff"],
    ["pipx", { road: "pipx", package: "black" }, "black"],
    ["cargo", { road: "cargo", package: "ripgrep", version: "14.1.0" }, "rg"],
    ["cargo plain", { road: "cargo", package: "bat" }, "bat"],
    ["go", { road: "go", module: "github.com/x/y/cmd/z", version: "v1.2.3" }, "z"],
    ["go bare", { road: "go" }, "z"],
    ["release assets", c.pinnedRelease("cli/cli", "github.com/cli/cli/v2/cmd/gh"), "gh"],
    ["release assets no go", c.pinnedRelease("jdx/mise"), "mise"],
    ["release half assets", { road: "release", repo: "a/b", version: "v1", assets: { x86_64: { name: "b-linux-amd64", sha256: "c".repeat(64) } }, go: "github.com/a/b" }, "b"],
    ["release own", { road: "release", repo: "a/b", version: "v1.0.0" }, "b"],
    ["release own pinned", { road: "release", repo: "a/b", pin }, "b"],
    ["release own arch pin", { road: "release", repo: "a/b", version: "v1.0.0", pin: { ...pin, arch: "aarch64" }, go: "github.com/a/b/cmd/c@v1.0.0" }, "b"],
    ["release no tag", { road: "release", repo: "a/b" }, "b"],
    ["release no repo", { road: "release", version: "v1" }, "b"],
    ["vendor", { road: "vendor", cask: c.GCLOUD, version: c.GCLOUD.version }, "gcloud"],
    ["apt", { road: "apt", packages: ["build-essential", "cmake"] }, "cc"],
    ["apt empty", { road: "apt", packages: [] }, "x"],
    ["script", { road: "script", script: "echo hi", bins: [c.LOCAL_BIN] }, "hi"],
    ["script rustup", { road: "script", script: c.RUSTUP_INSTALL, bins: [c.CARGO_BIN] }, "cargo"],
    ["script none", { road: "script", script: "echo hi" }, "hi"],
  ];
  const prefix = c.installHomes(c.TOOL_PREFIX);
  write("road_modules.json", {
    homes: { own: c.installHomes(), prefix },
    installEnv: { own: c.installEnv(c.installHomes()), prefix: c.installEnv(prefix) },
    roadSteps: c.ROAD_STEPS,
    modules: Object.fromEntries(
      c.ROADS.map(name => {
        const m = c.ROAD_MODULES[name] as c.RoadModule<c.InstallRoad>;
        return [name, { words: m.words, roots: m.roots, pastes: m.pastes ?? true, after: m.after ?? null, env: m.env === undefined ? null : { own: m.env(c.installHomes()), prefix: m.env(prefix) }, hasAt: m.at !== undefined, hasFromRow: m.fromRow !== undefined }];
      }),
    ),
    samples: samples.map(([label, road, bin]) => {
      const m = c.roadModule(road) as c.RoadModule<c.InstallRoad>;
      return {
        label,
        road: ser(road),
        bin,
        bins: { own: m.bins(road), prefix: m.bins(road, prefix) },
        present: m.present?.(road, bin) ?? null,
        check: m.check?.(road, bin) ?? null,
        shown: m.shown?.(road, bin) ?? null,
        install: { own: m.install(road, bin), prefix: m.install(road, bin, prefix) },
        uninstall: { own: m.uninstall(road, bin), prefix: m.uninstall(road, bin, prefix) },
        names: m.names(road),
        roadBin: m.bin?.(road) ?? null,
        at: m.at === undefined ? null : ser(m.at(road, "9.9.9")),
        installed: m.installed === undefined ? null : { own: m.installed(road, bin), prefix: m.installed(road, bin, prefix) },
        fixesVersion: c.fixesVersion(road),
        versionOf: c.versionOf(road) ?? null,
        standingPin: c.standingPin(road) ?? null,
        unpinned: ser(c.unpinned(road)),
      };
    }),
    fromRow: ["brew", "npm", "pnpm", "bun", "uv", "pipx", "cargo", "go", "release", "vendor", "apt", "script", "nothing"].map(manager => ({
      manager,
      reads: c.readsRowRoad(manager),
      rows: [
        { name: "tool", paths: [], label: "tool" },
        { name: "tool", version: "1.2.3", paths: [], label: "tool 1.2.3" },
        { name: "gopls", paths: ["golang.org/x/tools/gopls@v0.16.1"], label: "gopls" },
        { name: "gopls", version: "v0.17.0", paths: ["golang.org/x/tools/gopls@v0.16.1"], label: "gopls" },
        { name: "old", paths: [], label: "old (example.com/old@v1.0.0)" },
        { name: "bad", paths: ["@v1"], label: "bad" },
        { name: "bad", paths: ["example.com/x@"], label: "bad" },
      ].map(row => {
        const read = c.rowRoadReader(manager);
        return read === undefined ? null : ser(read(row));
      }),
    })),
    homeBins: {
      own: c.homeBins([c.CARGO_BIN, c.LOCAL_BIN, "/opt/x/bin", c.HOME_BIN, "/root/.local/share/pnpm"], c.installHomes()),
      prefix: c.homeBins([c.CARGO_BIN, c.LOCAL_BIN, "/opt/x/bin", c.HOME_BIN, "/root/.local/share/pnpm"], prefix),
    },
    releaseLines: {
      assetComplete: assetInstallForOracle("gh", "cli/cli", "v2", { x86_64: { name: "a b.tar.gz", sha256: "1".repeat(64) }, aarch64: { name: "c.zip", sha256: "2".repeat(64) } }, undefined),
      assetHalfGo: assetInstallForOracle("b", "a/b", "v1", { aarch64: { name: "b-arm64", sha256: "2".repeat(64) } }, "github.com/a/b/cmd/other"),
      assetNone: assetInstallForOracle("b", "a/b", "v1", {}, undefined),
      releaseNoPin: releaseInstallForOracle("b", "a/b", "v1", undefined, undefined),
      releasePinNoArch: releaseInstallForOracle("b", "a/b", "v1", { tag: "v1", sha256: "f".repeat(64) }, "github.com/a/b@v0.9.0"),
      releasePinArch: releaseInstallForOracle("b", "a/b", "v1", { tag: "v1", sha256: "f".repeat(64), arch: "x86_64" }, "github.com/a/b"),
      releasePinNoSum: releaseInstallForOracle("b", "a/b", "v1", { tag: "v1" }, undefined),
    },
  });

  // --- sign-ins --------------------------------------------------------------------------------------------------
  const outputs = [
    "",
    "Logged in to github.com account me\n",
    "Logged in to github.com\nFailed to log in to ghe.example",
    "me@example.com",
    '{"Arn": "arn:aws:sts::1:assumed-role"}',
    "You are logged in with an OAuth Token",
    "No existing credentials found",
    "Not logged in",
    "Logged in as me",
    "Logged in using ChatGPT",
    "Logged in using an API key - sk-***",
    "oauth_creds.json",
    "GEMINI_API_KEY",
    "warn\nGOOGLE_API_KEY\n",
    "2 credentials\nEnvironment\n1 environment variable",
    "0 credentials",
    "Login successful",
    "cert.pem",
    "prod-cluster\n",
    "provider   model\nanthropic  claude",
    "anthropic (2 credentials):\n  OPENAI_API_KEY",
    'Some text {"loggedIn": true, "authMethod": "claude.ai", "subscriptionType": "max"} end',
    '{"loggedIn": true, "authMethod": "claude.ai"}',
    '{"loggedIn": true, "authMethod": "claude.ai", "subscriptionType": ""}',
    '{"loggedIn": true, "authMethod": "oauth_token"}',
    '{"loggedIn": false, "apiKeySource": "ANTHROPIC_API_KEY"}',
    '{"loggedIn": true, "authMethod": "other"}',
    "{not json}",
  ];
  const secrets = new Map([["GEMINI_API_KEY", "~/.zshrc"], ["OPENAI_API_KEY", "~/.config/env"], ["ANTHROPIC_API_KEY", "~/.zshrc"]]);
  const rows = c.SIGN_IN_ROWS as Record<string, c.SignIn>;
  write("signin.json", {
    rows: ser(rows),
    readers: Object.fromEntries(
      Object.entries(rows).flatMap(([k, s]) =>
        s.status === undefined
          ? []
          : [
              [
                k,
                outputs.map(o => ({
                  output: o,
                  signedIn0: s.status!.signedIn(o, 0),
                  signedIn1: s.status!.signedIn(o, 1),
                  detail: s.status!.detail?.(o, secrets) ?? null,
                  detailNoSecrets: s.status!.detail?.(o, new Map()) ?? null,
                  kind: s.status!.kind?.(o) ?? null,
                  plan: s.status!.plan?.(o) ?? null,
                })),
              ],
            ],
      ),
    ),
    perRow: Object.fromEntries(
      Object.entries(rows).map(([k, s]) => [
        k,
        {
          hasLogin: c.hasLogin(s),
          mintsToken: c.mintsToken(s),
          loginThere: c.loginThere(s) === undefined ? null : ser(c.loginThere(s)),
          sharedLoginOf: c.sharedLoginOf(s) ?? null,
          livesOnComputer: c.livesOnComputer(s),
          keyEnvOf: c.keyEnvOf(s) ?? null,
          questionsOf: ser(c.questionsOf(s)),
          asksThePerson: c.asksThePerson(s),
          signInRoadOf: c.signInRoadOf(s),
          loginWords: c.hasLogin(s) ? c.loginWords(s) : null,
          signsInByDefault: c.signsInByDefault(s),
          loginStatePaths: c.loginStatePaths({ signIn: s }),
        },
      ]),
    ),
    loginIdOf: Object.fromEntries(["kubectl", "gh", "kube"].map(id => [id, c.loginIdOf(id)])),
    keysIdOf: Object.fromEntries(["kubectl", "hermes"].map(id => [id, c.keysIdOf(id)])),
    keysRowOf: ser(c.keysRowOf({ paths: ["~/.hermes/.env"], note: "n" }, rows.hermes!.status)),
    keysRowOfNoStatus: ser(c.keysRowOf({ paths: ["~/.x"], note: "n" }, undefined)),
    noSignIn: c.NO_SIGN_IN,
    sharedLogins: c.SHARED_LOGINS,
    sharesIn: c.sharesIn("/var/lib/wsp/logins"),
    neverInImage: c.NEVER_IN_IMAGE,
    secretsFiles: c.SECRETS_FILES,
    loginPaths: c.LOGIN_PATHS,
    neverOnBrokered: c.NEVER_ON_BROKERED,
    ghLoginPaths: c.GH_LOGIN_PATHS,
    vaultVariables: [...c.VAULT_VARIABLES],
    serverValuesOf: c.serverValuesOf({ ANTHROPIC_API_KEY: "a", CLAUDE_CODE_OAUTH_TOKEN: "t", WSP_MCP_LINEAR_AUTHORIZATION: "tok", NOTION_TOKEN: "n", OPENAI_API_KEY: "o", WSP_MCP_X_AUTHORIZATION_BEARER: "kept" }),
    vaultVariableRow: Object.fromEntries(["CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY", "OPENAI_API_KEY", "GEMINI_API_KEY", "CURSOR_API_KEY", "NOTION_TOKEN"].map(n => [n, c.vaultVariableRow(n) ?? null])),
    tokenIn: ["sk-ant-oat01-abcdefghijklmnopqrstuvwxyz", "  sk-ant-oat01-abcdefghijklmnopqrstuvwxyz\n", "token: sk-ant-oat01-abcdefghijklmnopqrstuvwxyz", "sk-ant-oat01-short", ""].map(p => ({ pasted: p, out: c.tokenIn(rows.claude as c.TokenSignIn, p) ?? null })),
    secretNamed: ["uses OPENAI_API_KEY here", "OPENAI_API_KEYS", "GEMINI_API_KEY and OPENAI_API_KEY", ""].map(o => ({ output: o, out: c.secretNamed(o, secrets) ?? null })),
    geminiSource: ["oauth_creds.json", "GEMINI_API_KEY", "x GOOGLE_API_KEY", "", "  \n"].map(o => ({ output: o, out: c.geminiSource(o, secrets) ?? null, bare: c.geminiSource(o, new Map()) ?? null })),
  });

  // --- marks, seeds, ecosystems, git hosts ------------------------------------------------------------------------
  write("marks.json", {
    brand: c.BRAND_MARKS,
    provider: c.PROVIDER_MARKS,
    editor: c.EDITOR_MARKS,
    serverMark: [
      { kind: "http", host: "api.githubcopilot.com" },
      { kind: "http", host: "MCP.Linear.App:443" },
      { kind: "http", host: "example.com" },
      { kind: "stdio", line: "npx -y @modelcontextprotocol/server-github@1.2.0" },
      { kind: "stdio", line: "docker run -i ghcr.io/github/github-mcp-server:latest" },
      { kind: "stdio", line: "npx mcp-remote https://mcp.linear.app/sse" },
      { kind: "stdio", line: "node server.js" },
    ].map(t => ({ transport: t, mark: c.serverMark(t as never)?.id ?? null })),
  });
  write("seed.json", {
    rows: c.SEED_ROWS,
    judged: [
      ...c.SEED_ROWS.flatMap(row => Object.values(row.examples).flat().flatMap(p => [[p, false], [p, true]] as [string, boolean][])),
      ["./.env/", true],
      ["a/b/.env.example", false],
      ["myconfig/master.key", false],
      ["config/credentials/x/y.key", false],
      ["deep/node_modules", true],
      ["deep/node_modules", false],
      ["README.md", false],
      ["", false],
    ].map(([path, dir]) => {
      const got = c.seedRowFor(path as string, dir as boolean);
      return { path, dir, row: got?.row.id ?? null, kind: got?.kind ?? null };
    }),
    installs: [["package-lock.json", "pnpm-lock.yaml"], ["bun.lockb", "bun.lock"], ["uv.lock", "Cargo.lock", "go.sum"], [], ["poetry.lock"]].map(names => ({ names, out: c.seedInstallsFor(names).map(f => [f.row.id, f.install.lockfile]) })),
    ecosystems: c.ECOSYSTEM_MODULES,
    gitHosts: c.GIT_HOSTS.map(h => ({ ...h, httpsUrl: h.httpsUrl("o/r"), clone: h.clone("o/r", "/d") })),
    remoteHost: Object.fromEntries(["git@github.com:o/r.git", "https://GitHub.com/o/r", "ssh://git@ssh.github.com:443/o/r.git", "git+ssh://me@gitlab.com/o/r", "o/r", "", "file:///x", "user@host"].map(rm => [rm, c.remoteHost(rm) ?? null])),
    gitHostOf: Object.fromEntries(["git@github.com:o/r.git", "https://altssh.gitlab.com/o/r", "git@bitbucket.org:o/r"].map(rm => [rm, c.gitHostOf(rm)?.id ?? null])),
    noGitCliLine: c.noGitCliLine(c.gitHost("gitlab")!, "the box"),
  });

  // --- the machine context ----------------------------------------------------------------------------------------
  const roots = { etc: "/etc", home: "/root" };
  write("context.json", {
    hooks: Object.fromEntries(
      [CLAUDE_CONTEXT, CODEX_CONTEXT, GEMINI_CONTEXT, OPENCODE_CONTEXT, PI_CONTEXT, HERMES_CONTEXT].map(ctx => [
        ids.get(ctx),
        {
          cd: ctx.cd ?? null,
          conflict: ctx.conflict?.(roots) ?? null,
          plain: ctx.hooks({ short: "SHORT\n", skill: "SKILL\n", source: "/etc/wsp/short.md", roots, conflict: false, fish: false }),
          conflicted: ctx.hooks({ short: "SHORT\n", skill: "SKILL\n", source: "/etc/wsp/short.md", roots, conflict: true, fish: true }),
          fish: ctx.hooks({ short: "a ''' b", skill: "SKILL", source: "/s", roots: { etc: "/x/etc", home: "/x/home" }, conflict: false, fish: true }),
        },
      ]),
    ),
    pluginSkills: CLAUDE_PLUGIN_SKILLS,
  });
});
