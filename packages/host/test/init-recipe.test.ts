// SPDX-License-Identifier: AGPL-3.0-only
import { type ManifestEntry, parseManifest } from "@wsp/collect";
import { describe, expect, it } from "vitest";
import {
  answeredRows,
  hasChoices,
  initialChoice,
  lockRefused,
  refusedNote,
  initialTicks,
  isTickable,
  loginTool,
  tickLoginTools,
  withoutAgentTools,
} from "../src/init-recipe.js";
import { FIXTURE, byId } from "./init-fixture.js";

const ANTHROPIC = "sk-ant-x-fake-anthropic-key";
describe("manifest ticks", () => {
  it("a skip with a reason cannot be ticked; a bare skip can", () => {
    expect(isTickable(byId("identity/ssh-key"))).toBe(false);
    expect(isTickable(byId("agents/codex"))).toBe(true);
    expect(isTickable(byId("identity/git-user"))).toBe(true);
  });

  it("initial ticks follow the default, then a saved bring flag, and required is always on", () => {
    expect(initialTicks(byId("identity/ssh-config"))).toBe(true);
    expect(initialTicks(byId("agents/codex"))).toBe(false);
    expect(initialTicks(byId("identity/ssh-key"))).toBe(false);
    expect(initialTicks({ ...byId("agents/codex"), bring: true })).toBe(true);
    expect(initialTicks({ ...byId("identity/git-user"), bring: false })).toBe(true);
    expect(initialTicks({ ...byId("identity/ssh-key"), bring: true })).toBe(false);
  });
});

describe("login choices", () => {
  it("copy when the default is bring, left to first use when it is skip or cannot be copied, and a saved choice wins", () => {
    // The collector's default carries the catalog's word: a browser or device sign-in (gh, codex, the Claude OAuth
    // credential) is skip, a key or a tool with no sign-in is bring. A row with nothing here to copy can only be
    // signed in through a browser, which waits on the person, so it starts left to the first time it is needed.
    expect(initialChoice(byId("logins/gh"))).toBe("later");
    expect(initialChoice(byId("logins/codex"))).toBe("later");
    expect(initialChoice({ ...byId("logins/gh"), default: "bring" })).toBe("copy");
    // Claude Code mints its token on this computer, so its unanswered row opens there rather than on a machine.
    expect(initialChoice(byId("logins/claude"))).toBe("token");
    expect(initialChoice({ ...byId("logins/gh"), default: "skip", reason: "expires in hours" })).toBe("later");
    expect(initialChoice({ ...byId("logins/gh"), choice: "skip" })).toBe("skip");
    expect(initialChoice({ ...byId("logins/gh"), bring: false })).toBe("later");
    expect(initialChoice({ ...byId("logins/claude"), choice: "skip" })).toBe("skip");
    // Signing in while the build runs is the opt-in, and a recipe that saved it is read back as that and not as the default.
    expect(initialChoice({ ...byId("logins/gh"), choice: "machine" })).toBe("machine");
    expect(initialChoice({ ...byId("logins/gh"), choice: "later" })).toBe("later");
  });

});

describe("the command a login needs", () => {
  const AWSCLI: ManifestEntry = { rung: "tools", id: "tools/brew/awscli", label: "awscli", group: "Homebrew", paths: [], bytes: 0, default: "bring", linux: "yes" };
  const LOCKED_AWSCLI: ManifestEntry = { ...AWSCLI, default: "skip", reason: "no Linux bottle", linux: "no" };
  /** The bare row the recipe adds for a catalog tool this computer has no row for. */
  const catalogRow = (id: string, label: string): ManifestEntry => ({ rung: "tools", id: `tools/catalog/${id}`, label, group: "Catalog", paths: [], bytes: 0, default: "skip", linux: "yes" });
  const GCLOUD_ROW = catalogRow("gcloud", "Google Cloud CLI");
  const KUBECTL_ROW: ManifestEntry = { rung: "tools", id: "tools/brew/kubernetes-cli", label: "kubernetes-cli", group: "Homebrew", paths: [], bytes: 0, default: "skip", linux: "yes" };
  const login = (id: string, label: string): ManifestEntry => ({ rung: "logins", id: `logins/${id}`, label, group: "CLI logins", paths: [`~/.${id}`], bytes: 10, default: "bring" });
  const GCLOUD = login("gcloud", "Google Cloud login");
  const WRANGLER = login("wrangler", "Cloudflare Wrangler login");
  const KUBE = login("kube", "kubectl config");
  const AWS = login("aws", "AWS keys and profiles");

  it("a login's command is coming when its tools row is ticked, else the row is named as unticked, locked, or missing with the catalog entry that brings it", () => {
    const manifest = { entries: [GCLOUD_ROW, KUBECTL_ROW, AWSCLI, ...FIXTURE.entries, GCLOUD, WRANGLER, KUBE, AWS] };
    const none = new Set<string>();
    expect(loginTool(GCLOUD, manifest, none, new Map())).toEqual({ bin: "gcloud", row: GCLOUD_ROW, coming: false, why: "gcloud is not coming: its tool row is unticked; copy or sign in ticks it" });
    expect(loginTool(GCLOUD, manifest, new Set(["tools/catalog/gcloud"]), new Map())).toEqual({ bin: "gcloud", row: GCLOUD_ROW, coming: true });
    expect(loginTool(WRANGLER, manifest, none, new Map())).toEqual({ bin: "wrangler", coming: false, why: "wrangler is not coming: no row lists it; tick Cloudflare Wrangler under Tools to bring it" });
    expect(loginTool(KUBE, manifest, none, new Map())).toMatchObject({ bin: "kubectl", row: KUBECTL_ROW, coming: false, why: "kubectl is not coming: its tool row is unticked; copy or sign in ticks it" });
    expect(loginTool(KUBE, { entries: [KUBE] }, none, new Map())?.why).toBe("kubectl is not coming: no row lists it; tick kubectl under Tools to bring it");
    // A formula not named for its command still counts; a locked row says so.
    expect(loginTool(AWS, manifest, new Set(["tools/brew/awscli"]), new Map())).toEqual({ bin: "aws", row: AWSCLI, coming: true });
    expect(loginTool(AWS, { entries: [LOCKED_AWSCLI, AWS] }, none, new Map())).toMatchObject({ coming: false, why: "aws is not coming: its tool row cannot come (no Linux bottle)" });
    // A command the catalog does not know is named plainly.
    expect(loginTool({ ...KUBE, id: "logins/cloudflared" }, { entries: [] }, none, new Map())?.why).toBe("cloudflared is not coming: no row lists it; tick cloudflared under Tools to bring it");
    // gh's row is ticked in the fixture's defaults; an agent's login follows its agent, not a tools row.
    expect(loginTool(byId("logins/gh"), manifest, new Set(["tools/brew/gh"]), new Map())).toEqual({ bin: "gh", row: byId("tools/brew/gh"), coming: true });
    expect(loginTool(byId("logins/claude"), manifest, none, new Map())).toBeUndefined();
    // Any catalog tool's login names its command; a keys row beside an agent's login follows the agent.
    expect(loginTool(login("fly", "Fly login"), { entries: [] }, none, new Map())?.why).toBe("fly is not coming: no row lists it; tick flyctl under Tools to bring it");
    expect(loginTool(login("hermes-keys", "Hermes keys"), { entries: [] }, none, new Map())).toBeUndefined();
    expect(loginTool(byId("tools/brew/gh"), manifest, none, new Map())).toBeUndefined();
  });

  it("a wrangler row under any node package manager or Homebrew's own formula counts as the command", () => {
    for (const id of ["tools/npm/wrangler", "tools/pnpm/wrangler", "tools/bun/wrangler", "tools/brew/cloudflare-wrangler"]) {
      const row: ManifestEntry = { rung: "tools", id, label: "wrangler", paths: [], bytes: 0, default: "bring", linux: "yes" };
      expect(loginTool(WRANGLER, { entries: [row, WRANGLER] }, new Set([id]), new Map())).toEqual({ bin: "wrangler", row, coming: true });
    }
  });

  it("tickLoginTools ticks the row of every login answered copy or sign in; a skip, a coming row and a locked row leave the ticks alone", () => {
    const manifest = { entries: [GCLOUD_ROW, KUBECTL_ROW, GCLOUD, KUBE, WRANGLER, AWS, AWSCLI] };
    const ticks = new Set<string>(["tools/brew/awscli"]);
    const added = tickLoginTools(manifest, new Map([["logins/gcloud", "copy"], ["logins/kube", "machine"], ["logins/wrangler", "copy"], ["logins/aws", "copy"]]), ticks, new Map());
    expect(added).toEqual(["Google Cloud CLI", "kubernetes-cli"]);
    expect([...ticks]).toEqual(["tools/brew/awscli", "tools/catalog/gcloud", "tools/brew/kubernetes-cli"]);
    const untouched = new Set<string>();
    expect(tickLoginTools({ entries: [LOCKED_AWSCLI, AWS] }, new Map([["logins/aws", "copy"]]), untouched, new Map())).toEqual([]);
    expect(tickLoginTools(manifest, new Map([["logins/gcloud", "skip"]]), untouched, new Map())).toEqual([]);
    expect(untouched.size).toBe(0);
  });
});

describe("npm globals an agent installs itself", () => {
  const PI_NPM: ManifestEntry = { rung: "tools", id: "tools/npm/@earendil-works/pi-coding-agent", label: "@earendil-works/pi-coding-agent@0.84.1", group: "npm globals", paths: [], bytes: 0, default: "bring", linux: "yes", version: "0.84.1" };
  const WRANGLER_NPM: ManifestEntry = { ...PI_NPM, id: "tools/npm/wrangler", label: "wrangler@4.106.0", version: "4.106.0" };
  const PI_AGENT: ManifestEntry = { rung: "agents", id: "agents/pi", label: "Pi", paths: ["~/.pi/agent/settings.json"], bytes: 80, default: "bring" };

  it("the package's row goes when its agent is on the Agents screen, whatever the agent's tick; without the agent row it stays, as do other globals", () => {
    expect(withoutAgentTools([PI_NPM, WRANGLER_NPM, PI_AGENT, ...FIXTURE.entries])).toEqual([WRANGLER_NPM, PI_AGENT, ...FIXTURE.entries]);
    expect(withoutAgentTools([PI_NPM, WRANGLER_NPM])).toEqual([PI_NPM, WRANGLER_NPM]);
    expect(withoutAgentTools([{ ...PI_NPM, bring: true }, { ...PI_AGENT, bring: false }])).toEqual([{ ...PI_AGENT, bring: false }]);
  });
});

describe("rows the plan refuses by name", () => {
  const env: ManifestEntry = { rung: "shell", id: "shell/env", label: ".env", paths: ["~/.env"], bytes: 20, default: "skip" };
  const netrc: ManifestEntry = { rung: "shell", id: "shell/netrc", label: ".netrc", paths: ["~/.netrc"], bytes: 30, default: "skip", consent: true };

  const file = () => false;
  const dir = () => true;
  const missing = () => undefined;

  it("an unconsented .env or .netrc file gets the plan's note; a directory of that name, a consent row and a login's own .env do not", () => {
    expect(refusedNote(env, file)).toBe(".env files are never copied; set the values on the machine");
    expect(refusedNote(env, dir)).toBeUndefined();
    expect(refusedNote(env, missing)).toBeUndefined();
    expect(refusedNote(netrc, file)).toBeUndefined();
    expect(refusedNote({ ...env, rung: "logins", id: "logins/hermes", paths: ["~/.hermes/.env"] }, file)).toBeUndefined();
    expect(refusedNote({ ...env, paths: ["~/.env", "~/.app/config"] }, file)).toBeUndefined();
    expect(refusedNote(byId("tools/brew/gh"), file)).toBeUndefined();
    expect(refusedNote(byId("identity/ssh-key"), file)).toBe("private key, never copied");
  });

  it("lockRefused writes the plan's note onto the rows it would refuse whole and leaves every other row as it was", () => {
    const manifest = { entries: [env, netrc, byId("shell/zshrc")] };
    const locked = lockRefused(manifest, rel => (rel === ".env" ? false : undefined));
    expect(locked.entries[0]).toEqual({ ...env, default: "skip", reason: ".env files are never copied; set the values on the machine" });
    expect(isTickable(locked.entries[0]!)).toBe(false);
    expect(initialTicks({ ...locked.entries[0]!, bring: true })).toBe(false);
    expect(locked.entries.slice(1)).toEqual([netrc, byId("shell/zshrc")]);
    // A directory named .env is a Python environment more often than a secret: it stays a plain row.
    expect(lockRefused(manifest, dir).entries).toEqual(manifest.entries);
    expect(isTickable(env)).toBe(true);
  });

  it("a row carrying a detail still picks up the plan's refusal note; only a reason holds it off", () => {
    const config = byId("identity/ssh-config");
    expect(config.detail).toBeDefined();
    const locked = lockRefused({ entries: [config] }, dir).entries[0]!;
    expect(locked).toEqual({ ...config, default: "skip", reason: "a directory under .ssh is never copied whole" });
    expect(isTickable(locked)).toBe(false);
    // The same row with the line written as a reason instead would swallow the note and offer a tick the pack throws away.
    expect(lockRefused({ entries: [{ ...config, detail: undefined, reason: config.detail! }] }, dir).entries[0]!.default).toBe("bring");
  });
});

describe("consent rows", () => {
  const token: ManifestEntry = { rung: "agents", id: "agents/mcp/claude/github", label: "github", paths: [], bytes: 0, default: "bring", consent: true };
  const plain: ManifestEntry = { rung: "agents", id: "agents/mcp/claude/notes", label: "notes", paths: [], bytes: 0, default: "skip" };

  it("a login or a credential-shaped row is answered, not ticked; its answer starts at skip, a saved tick alone is not consent, and a saved choice wins", () => {
    expect(hasChoices(token)).toBe(true);
    expect(hasChoices(plain)).toBe(false);
    expect(hasChoices(byId("logins/gh"))).toBe(true);
    expect(initialChoice(token)).toBe("skip");
    expect(initialChoice({ ...token, bring: true })).toBe("skip");
    expect(initialChoice({ ...token, bring: true, choice: "copy" })).toBe("copy");
    // An answer a consent row never offered (saved as sign in by an older recipe) reads as skip.
    expect(initialChoice({ ...token, bring: true, choice: "machine" })).toBe("skip");
    expect(initialTicks(plain)).toBe(false);
    expect(initialTicks({ ...plain, bring: true })).toBe(true);
  });

  it("the schema takes a choice on a consent row and refuses one on a plain row", () => {
    expect(parseManifest({ entries: [{ ...token, choice: "copy" }] }).entries[0]).toMatchObject({ choice: "copy", consent: true });
    expect(() => parseManifest({ entries: [{ ...plain, choice: "copy" }] })).toThrow(/entries\.0\.choice: only a logins row or a consent row carries a choice/);
    expect(parseManifest({ entries: [{ ...byId("shell/zshrc"), excludes: ["~/.zshrc.d/secret"] }] }).entries[0]).toMatchObject({ excludes: ["~/.zshrc.d/secret"] });
  });

  it("answeredRows writes the screens' tick and answer on every row, so the MCP plan reads the copy a server was given and never the manifest's stale word", () => {
    const stale = { ...token, choice: "skip" as const };
    const rows = answeredRows({ entries: [byId("agents/claude"), stale, byId("shell/zshrc")] }, new Set(["agents/claude", "agents/mcp/claude/github"]), new Map([["agents/mcp/claude/github", "copy"], ["agents/claude", "nonsense"]]));
    expect(rows).toEqual([{ ...byId("agents/claude"), bring: true }, { ...token, bring: true, choice: "copy" }, { ...byId("shell/zshrc"), bring: false }]);
  });

});

describe("parseManifest", () => {
  it("accepts the collector shape and a saved recipe with bring flags", () => {
    expect(parseManifest(JSON.parse(JSON.stringify(FIXTURE)))).toEqual(FIXTURE);
    const saved = { entries: [{ ...byId("shell/zshrc"), bring: false }] };
    expect(parseManifest(saved).entries[0]).toMatchObject({ id: "shell/zshrc", bring: false });
  });

  it("names the row and field that is wrong, in the collector's words", () => {
    expect(() => parseManifest({ entries: [{ rung: "kitchen", id: "kitchen/x", label: "x", paths: [], bytes: 0, default: "bring" }] })).toThrow(/invalid manifest: entries\.0\.rung/);
    expect(() => parseManifest({ entries: [{ rung: "shell", id: "shell/x", label: "x", paths: [], bytes: "big", default: "bring" }] })).toThrow(/entries\.0\.bytes/);
    expect(() => parseManifest({ entries: [{ ...byId("logins/gh"), choice: "maybe" }] })).toThrow(/entries\.0\.choice/);
    expect(() => parseManifest({ entries: [{ ...byId("shell/zshrc"), id: "zshrc" }] })).toThrow(/entries\.0\.id: id must start with shell\//);
    expect(() => parseManifest({ entries: [byId("shell/zshrc"), byId("shell/zshrc")] })).toThrow(/entries\.1\.id: duplicate id shell\/zshrc/);
    expect(() => parseManifest({ items: [] })).toThrow(/entries/);
    expect(() => parseManifest("nope")).toThrow(/object/);
  });
});

