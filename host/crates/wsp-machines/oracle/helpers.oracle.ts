// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers of places/helpers.ts and places/pane-ports.ts's pure parts on the inputs the Rust tests replay,
// written as tests/fixtures/oracle/helpers.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 helpers.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { CATALOG_AGENTS, SHARED_LOGINS, sharedFileIn } from "@wsp/catalog";
import { SSH_STORE_VARS } from "@wsp/engine";
import { RecipeFile } from "@wsp/protocol";

const h = await import("../../../../packages/runtime/src/places/helpers.ts");

const out = fileURLToPath(new URL("../tests/fixtures/oracle/helpers.json", import.meta.url));
const tryIt = (f: () => unknown): unknown => {
  try {
    const v = f();
    return { ok: v === undefined ? null : v };
  } catch (e) {
    return { err: (e as Error).message };
  }
};
const row = (id: string, outcome: string, extra: Record<string, unknown> = {}) => ({ id, label: id.toUpperCase(), outcome, ...extra });

it("writes the helpers fixture", () => {
  const facts = {
    agents: CATALOG_AGENTS.map(a => ({ id: a.id, signIn: a.signIn === undefined ? null : a.signIn.kind, stateHomeEnv: a.stateHomeEnv ?? null })),
    sharedLogins: SHARED_LOGINS.map(sharedFileIn),
    sshStoreVars: SSH_STORE_VARS,
    sharedLoginFile: Object.fromEntries(["claude", "codex", "gemini", "opencode", "cursor", "gh", "nobody"].map(a => [a, h.sharedLoginFile(a) ?? null])),
  };
  const login = (o: Record<string, string>) => ({ agents: [], login: o, shape: { cpus: 2, memMb: 4096, diskGb: 40 }, daemonVersion: 10 });
  const taken = [
    { HOME: "/root", PATH: "/usr/bin:/bin" },
    { HOME: "/home/a b", PATH: "/usr/bin:relative:/x;rm:/opt/bin" },
    { PATH: "/usr/bin" },
    { HOME: "" },
    { HOME: "relative" },
    { HOME: "/root/../etc" },
    { HOME: "/root;rm" },
    { HOME: "/root", CODEX_HOME: "/root/.codex", CLAUDE_CONFIG_DIR: "bad;dir", OTHER: "kept;x" },
    { HOME: "/root", PATH: "" },
  ].map(l => ({ login: l, out: tryIt(() => h.takenReport(login(l) as never)) }));
  const homeRefusal = [undefined, "", "rel", "/a;b"].map(s => ({ said: s ?? null, out: h.placeHomeRefusal(s) }));
  const installed = (id: string, o = "installed", extra = {}) => row(id, o, extra);
  const agentsOn = [
    { report: { agents: [] }, rows: [] },
    { report: { agents: ["codex", "claude"] }, rows: [] },
    { report: { agents: ["claude"] }, rows: [installed("agents/codex"), installed("agents/gemini", "present"), installed("agents/opencode", "failed"), installed("agents/claude")] },
    { report: { agents: ["zzz"] }, rows: [installed("agents/unknownagent"), installed("clis/gh")] },
  ].map(c => ({ ...c, out: h.agentsOn(c.report as never, c.rows as never) }));
  const signInsCases = [
    { report: { agents: ["claude", "codex"] }, vault: {}, rows: [] },
    { report: { agents: ["claude", "codex"], logins: [] }, vault: {}, rows: [] },
    { report: { agents: ["claude", "codex", "gemini", "opencode"], logins: SHARED_LOGINS.map(sharedFileIn) }, vault: {}, rows: [] },
    { report: { agents: ["claude", "codex", "gemini"], logins: [] }, vault: { CLAUDE_CODE_OAUTH_TOKEN: "t", ANTHROPIC_API_KEY: "k", OPENAI_API_KEY: "o", GEMINI_API_KEY: "g" }, rows: [] },
    { report: { agents: ["claude"], logins: [] }, vault: { ANTHROPIC_API_KEY: "k" }, rows: [] },
    { report: { agents: ["claude", "codex"], logins: [] }, vault: {}, rows: [row("signins/claude", "installed", { note: "signed in on that computer" }), row("signins/codex", "present", { note: "signed in on that computer" })] },
    { report: { agents: ["claude"], logins: [] }, vault: {}, rows: [row("signins/claude", "failed", { note: "signed in on that computer" })] },
    { report: { agents: [], logins: [] }, vault: {}, rows: [installed("agents/codex")] },
    { report: { agents: ["claude"], logins: [] }, vault: {}, rows: [], agents: ["gemini", "nobody"] },
  ].map(c => ({ ...c, out: h.signInsOf(c.report as never, c.vault, c.rows as never, ...(c.agents === undefined ? [] : [c.agents])) ?? null }));
  const vaultSignIn = ["claude", "codex", "gemini", "opencode", "cursor", "nobody"].flatMap(a =>
    [{}, { CLAUDE_CODE_OAUTH_TOKEN: "t" }, { ANTHROPIC_API_KEY: "k" }, { OPENAI_API_KEY: "o" }, { GEMINI_API_KEY: "g" }, { CURSOR_API_KEY: "c" }].map(v => ({ agent: a, vault: v, out: h.vaultSignIn(a, v) })),
  );
  const vaultHeld = ["claude", "codex", "nobody"].flatMap(a =>
    [{}, { CLAUDE_CODE_OAUTH_TOKEN: "t" }, { ANTHROPIC_API_KEY: "k" }, { CLAUDE_CODE_OAUTH_TOKEN: "t", ANTHROPIC_API_KEY: "k" }].flatMap(v =>
      [undefined, "key", "token", "vault", "machine"].map(way => ({ agent: a, vault: v, way: way ?? null, out: h.vaultHeldLine(a, v, way as never, "mac") })),
    ),
  );
  const picked = [
    { vault: { CLAUDE_CODE_OAUTH_TOKEN: "t", ANTHROPIC_API_KEY: "k", X: "1" }, picks: undefined },
    { vault: { CLAUDE_CODE_OAUTH_TOKEN: "t", ANTHROPIC_API_KEY: "k", X: "1" }, picks: { agents: { claude: { signin: "key" } } } },
    { vault: { CLAUDE_CODE_OAUTH_TOKEN: "t", X: "1" }, picks: { agents: { claude: { signin: "key" } } } },
    { vault: { CLAUDE_CODE_OAUTH_TOKEN: "t", ANTHROPIC_API_KEY: "k" }, picks: { agents: { claude: { signin: "token" } } } },
    { vault: { OPENAI_API_KEY: "o" }, picks: { agents: { codex: { signin: "key" } } } },
  ].map(c => ({ ...c, picks: c.picks ?? null, out: h.pickedVault(c.vault, c.picks as never) }));
  const options = { agents: [{ id: "claude", signins: ["vault", "machine", "token"] }, { id: "codex", signins: ["vault", "machine"] }, { id: "gemini", signins: ["machine"] }], clis: [], configs: [] };
  const servable = [
    { vault: {}, held: [] },
    { vault: { CLAUDE_CODE_OAUTH_TOKEN: "t" }, held: [] },
    { vault: {}, held: ["codex"] },
    { vault: { OPENAI_API_KEY: "o" }, held: [] },
  ].map(c => ({ ...c, options, out: h.servableOptions(options as never, c.vault, new Set(c.held)) }));
  const relisted = [
    { listed: [], found: [] },
    { listed: SHARED_LOGINS.map(sharedFileIn), found: SHARED_LOGINS.map(sharedFileIn) },
    { listed: ["zz/other", ...SHARED_LOGINS.map(sharedFileIn).slice(0, 1)], found: [] },
    { listed: ["zz/other"], found: [...SHARED_LOGINS.map(sharedFileIn)].reverse() },
    { listed: ["b", "a"], found: [] },
    { listed: ["a", "b"], found: ["not-shared"] },
  ].map(c => ({ ...c, out: h.relistedLogins(c.listed, c.found) ?? null }));
  const scripts = ["/root/.wsp/logins", "/home/a b/it's"].map(l => ({ logins: l, out: h.sharedLoginsScript(l) }));
  const landed = [
    { rows: [row("a", "skipped", { step: "signins" })], row: { id: "a", outcome: "installed", note: "n" } },
    { rows: [row("a", "failed")], row: { id: "a", outcome: "present", label: "ignored" } },
    { rows: [row("a", "installed")], row: { id: "a", outcome: "installed" } },
    { rows: [row("a", "present")], row: { id: "a", outcome: "installed" } },
    { rows: [row("a", "installed")], row: { id: "b", outcome: "installed" } },
    { rows: [row("a", "installed")], row: { id: "b", outcome: "installed", label: "B!", step: "github" } },
    { rows: [row("a", "skipped", { step: "github", fix: "f", ms: 3 }), row("c", "failed")], row: { id: "a", outcome: "installed" } },
  ].map(c => ({ ...c, out: h.landedOn(c.rows as never, c.row as never) ?? null }));
  const outcome = [
    { setup: { waiting: [] }, applied: null },
    { setup: { waiting: [] }, applied: { hash: "h", at: "t", rows: [row("x", "failed", { step: "clis" })] } },
    { setup: { waiting: [] }, applied: { hash: "h", at: "t", rows: [row("x", "failed", { step: "clis" }), row("y", "failed", { step: "agents" })] } },
    { setup: { waiting: [] }, applied: { hash: "h", at: "t", rows: [row("folders/a", "failed", { step: "folders", note: "no clone" })] } },
    { setup: { waiting: [] }, applied: { hash: "h", at: "t", rows: [row("github", "failed", { step: "github" })] } },
    { setup: { waiting: [{ row: "signins/claude", label: "Claude Code", expiresAt: "t", state: "waiting", url: "https://u", code: "AB-CD" }] }, applied: { hash: "h", at: "t", rows: [row("folders/a", "failed", { step: "folders" })] } },
    { setup: { waiting: [{ row: "signins/claude", label: "Claude Code", expiresAt: "t", state: "expired" }, { row: "github", label: "GitHub", expiresAt: "t", state: "waiting" }] }, applied: null },
  ].map(c => ({ ...c, out: h.setupOutcome(c.setup as never, (c.applied ?? undefined) as never) }));
  const step = (id: string, after?: string) => ({ id, label: id, ...(after === undefined ? {} : { after }) });
  const plan = { agents: 2, steps: [step("a1"), step("a2"), step("c1"), step("c2", "c1"), step("gh"), step("lfs", "c2"), step("c5")] };
  const cli = [[], ["gh"], ["lfs"], ["lfs", "c2"], ["c5", "gh"], ["nothere"], ["a1"], ["gh", "gh"]].map(ids => ({ ids, out: (h.cliFirst(plan as never, ids) as { steps: { id: string }[] }).steps.map(s => s.id), same: h.cliFirst(plan as never, ids) === plan }));
  const cyc = { agents: 0, steps: [step("x", "y"), step("y", "x")] };
  const cliCycle = (h.cliFirst(cyc as never, ["x"]) as { steps: { id: string }[] }).steps.map(s => s.id);
  const engineRow = [row("a", "installed"), row("a", "installed", { step: "clis" }), row("a", "installed", { step: "signins" }), row("a", "installed", { step: "folders" }), row("github", "installed", { step: "github" }), row("gh", "installed", { step: "github" })].map(r => ({ row: r, out: h.engineRow(r as never) }));
  const firstLine = [new Error("one\ntwo"), "plain\nsecond", new Error(""), 42, null].map(e => ({ out: h.firstLineOf(e) }));
  const hashes = [RecipeFile.parse({ name: "x" }), RecipeFile.parse({ name: "acme", agents: { claude: { signin: "vault" } }, clis: { gh: { via: "brew" } } })].map(f => ({ file: f, out: h.picksHash(f) }));
  const pending = h.pendingView({ id: "a1", address: "root@h", step: "wsp", startedAt: "t", keyPath: "/k", hostKey: "hk", undo: "u", login: "root@h" } as never);
  const consts = {
    JOIN_PROVE_MS: h.JOIN_PROVE_MS, LOGINS_READ_MS: h.LOGINS_READ_MS, SEEN_EVERY_MS: h.SEEN_EVERY_MS, REPLACED: h.REPLACED, LINK_FRAME_MS: h.LINK_FRAME_MS,
    RELINK_WAIT_MS: h.RELINK_WAIT_MS, BACKEND_FACTS_MS: h.BACKEND_FACTS_MS, ADD_FACTS_MS: h.ADD_FACTS_MS, CAPACITY_MS: h.CAPACITY_MS, DIAL_MS: h.DIAL_MS,
    JOIN_WAIT_MS: h.JOIN_WAIT_MS, ADDS_KEPT: h.ADDS_KEPT, ADD_RUNNING_LINE: h.ADD_RUNNING_LINE, ADD_RUNNING_FIX: h.ADD_RUNNING_FIX, PENDING: h.PENDING,
    ADD_STOPPED_LINE: h.ADD_STOPPED_LINE, ADD_STOPPED_FIX: h.ADD_STOPPED_FIX, ADD_NOT_TAKEN_BACK_LINE: h.ADD_NOT_TAKEN_BACK_LINE, NO_ESTIMATE_LINE: h.NO_ESTIMATE_LINE,
    LOG_READ_MS: h.LOG_READ_MS, UNDO_MS: h.UNDO_MS, SIGN_IN_SLACK_MS: h.SIGN_IN_SLACK_MS, GITHUB_MS: h.GITHUB_MS, INSTALLS: h.INSTALLS, FILES: h.FILES,
    PROBE_MS: h.PROBE_MS, SIGNIN_STATUS_MS: h.SIGNIN_STATUS_MS, PROVISION_LOG_EVERY_MS: h.PROVISION_LOG_EVERY_MS, PROVISION_LOG_LINES: h.PROVISION_LOG_LINES,
    UPDATE_WAIT_MS: h.UPDATE_WAIT_MS, UPDATE_POLL_MS: h.UPDATE_POLL_MS, UNMERGE_MS: h.UNMERGE_MS, PLACE_BAD_KEY_REFUSAL: h.PLACE_BAD_KEY_REFUSAL,
  };
  const ed = [
    { key: "MCowBQYDK2VwAyEAGb9ECWmEzf6FQbrBZ9w7lshQhqowtrbLDFw4rXAxZuE=", note: "ed25519 spki" },
    { key: "", note: "empty" },
    { key: "not base64 !!", note: "junk" },
    { key: "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEQ8Kg+Tq4QwYJkaSZbIP2Z4LgN3l9b0fBJmZ6P6lV+1t2wZfZ5Zf2x5Xn8LwYJbHc9n9JfQe2qPpYbBv6vQzv1A==", note: "p256 spki" },
  ].map(c => ({ ...c, out: h.readsAsEd25519(c.key) }));
  writeFileSync(out, `${JSON.stringify({ facts, taken, homeRefusal, agentsOn, signInsCases, vaultSignIn, vaultHeld, picked, servable, relisted, scripts, landed, outcome, cli, cliCycle, engineRow, firstLine, hashes, pending, consts, ed }, null, 2)}\n`);
});
