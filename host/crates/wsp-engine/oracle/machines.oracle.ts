// SPDX-License-Identifier: AGPL-3.0-only
// The TS engine's own answers on the inputs the Rust hard tests of the machines slice read, written as
// tests/fixtures/oracle/*.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";
import * as facts from "../../../../packages/engine/src/machine-facts.js";
import * as errors from "../../../../packages/engine/src/errors.js";
import * as ctx from "../../../../packages/engine/src/machine-context.js";
import * as box from "../../../../packages/engine/src/box-backend.js";
import { BASE_VERSION_LINES, parseVersions } from "../../../../packages/engine/src/golden-base.js";
import { EXEC_ENV, TOOLS_PATH } from "../../../../packages/engine/src/golden-import.js";
import { TOOLS_DISK_FLOOR, dfKbCmd } from "../../../../packages/engine/src/golden-tools.js";
import { NoProviderBackend } from "../../../../packages/engine/src/no-provider-backend.js";
import { BUILDER_LABEL, CREATED_AT_LABEL, GOLDEN_LABEL, NAME_LABEL, OWNER_LABEL, WORKSPACE_LABEL, WSP_LABEL, HOST_LABEL, SMOKE_LABEL, DOCTOR_LABEL } from "../../../../packages/engine/src/labels.js";

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, name), `${JSON.stringify(value, (_k, v) => (v instanceof Set ? [...v] : v === undefined ? null : v), 1)}\n`);
};
const tryOf = <T>(f: () => T): { ok: T } | { threw: string } => {
  try {
    return { ok: f() };
  } catch (e) {
    return { threw: e instanceof Error ? e.message : String(e) };
  }
};

it("machine facts", () => {
  const values = ["", "kernel Linux 6.8.0\n", "a 1\na 2\nb \n b x\nnospace\n\nc  spaced  \r\n", "identityfile ~/.ssh/a\nidentityfile ~/.ssh/b\nuser x\n"];
  const mem = ["4128768", "8060000", "", "0", "-5", "1e3", "0x10", "Infinity", " 2048 ", "1536", "512", "abc", "4128768.4"];
  const uptime = ["96521.42", "", "0", "  ", "abc", "1e2", "-3"];
  const boot = ["{ sec = 1789041249, usec = 855807 } Thu", "", "sec = 0", "garbage"];
  write("machine_facts.json", {
    reads: { OS_READ: facts.OS_READ, UPTIME_READ: facts.UPTIME_READ, HOME_READ: facts.HOME_READ, SYSTEM_READ: facts.SYSTEM_READ, ARCH_READ: facts.ARCH_READ, MEM_READ: facts.MEM_READ, SHELL_READ: facts.SHELL_READ },
    readValues: values.map(v => ({ in: v, values: Object.entries(facts.readValues(v)), lists: [...facts.readLists(v)] })),
    memMbOf: mem.map(m => ({ in: m, out: facts.memMbOf({ memkb: m }) })),
    uptimeMsOf: [...uptime.map(u => ({ uptime: u, boot: "{ sec = 100, usec = 0 }", now: 200_000, out: facts.uptimeMsOf({ uptime: u, boot: "{ sec = 100, usec = 0 }" }, 200_000) })), ...boot.map(b => ({ uptime: null, boot: b, now: 1_789_041_249_000 + 5, out: facts.uptimeMsOf({ boot: b }, 1_789_041_249_000 + 5) }))],
    archOf: ["", "x86_64", null].map(a => ({ in: a, out: facts.archOf(a === null ? {} : { arch: a }) })),
  });
});

it("errors", () => {
  const statuses = [200, 400, 401, 402, 403, 404, 409, 429, 500, 501, 502, 503, 504, 505];
  write("errors.json", {
    classify: [
      ...statuses.map(s => ({ status: s, code: "c", error: "e", requestId: null, out: errors.classify(s, { code: "c", error: "e" }) })),
      { status: 502, code: null, error: "Failed to snapshot sandbox", requestId: "req_1", out: errors.classify(502, { error: "Failed to snapshot sandbox" }, "req_1") },
      { status: 502, code: null, error: "failed to snapshot sandbox", requestId: null, out: errors.classify(502, { error: "failed to snapshot sandbox" }) },
      { status: 500, code: null, error: null, requestId: "", out: errors.classify(500, {}, "") },
    ],
    fetchCapMs: [0, 0.5, 1, 1.5, -3, 2_147_483_647, 2_147_483_648, 4_294_967_295, 10_000.0001].map(ms => ({ in: ms, out: errors.fetchCapMs(ms) })),
    messages: {
      notFirstLife: new errors.NotFirstLifeError("m1", "snapshot").message,
      envNotLanded: new errors.EnvNotLandedError("m1", 124, "timed out").message,
      guestUnusable: new errors.GuestUnusableError("m1", "box", "the shell died", 500).message,
      machineUnreachable: new errors.MachineUnreachableError("m1", "Sandbox is not reachable", 502, "LINE").message,
      execFailed: new errors.ExecFailedError("m1", "exec failed", 502).message,
      stopRefused: new errors.StopRefusedError("m1", "its latest snapshot failed").message,
      napRefused: new errors.NapRefusedError("m1", "no").message,
      unreached: new errors.MachineUnreached(4, 31_500, new Error("x")).message,
    },
    constants: { PROVIDER_READ_CAP_MS: errors.PROVIDER_READ_CAP_MS, ROAD_TRIES: errors.ROAD_TRIES },
  });
});

it("golden seam", () => {
  write("golden_seam.json", {
    BASE_VERSION_LINES,
    TOOLS_PATH,
    EXEC_ENV,
    TOOLS_DISK_FLOOR,
    dfKbCmd: [{ columns: ["size", "free"], path: "/root", out: dfKbCmd(["size", "free"], "/root") }, { columns: ["free"], path: null, out: dfKbCmd(["free"]) }, { columns: ["used"], path: "/x y", out: dfKbCmd(["used"], "/x y") }],
    parseVersions: ["VERSION python3: Python 3.12.13\nVERSION cc: \nVERSION zip: Copyright 1990\nVERSION zip2: This is Zip 3.0 (July 5th 2008)\nnoise\nVERSION git: git version 2.43.0  \nVERSION a:b: 1.2.3.4\n", ""].map(s => ({ in: s, out: parseVersions(s) })),
  });
});

it("no provider", () => {
  const b = new NoProviderBackend();
  write("no_provider.json", { capabilities: b.capabilities, capabilitiesKeys: Object.keys(b.capabilities), defaultSize: b.pricing.defaultSize, snapshotStorage: b.pricing.snapshotStorage });
});

const FACTS = {
  tools: [{ id: "tools/brew-cask/raycast", label: "raycast", note: "macOS app, no Linux build" }],
  agents: [{ id: "agents/zed", label: "Zed", note: "no installer known" }],
  files: [{ path: "~/.ssh/id_ed25519", note: "private key, never copied" }],
};

it("machine context", () => {
  const probe = (over: Partial<ctx.ContextProbe> = {}): ctx.ContextProbe => ({
    kernel: "6.6.30",
    disk: { sizeBytes: 20466256 * 1024, freeBytes: 11720704 * 1024 },
    overlay: false,
    has: new Set(["tmux", "fish", "brew", "golden-path", "wsp-open"]),
    versions: [{ name: "python3", version: "3.12.13" }],
    agents: ctx.CONTEXT_AGENTS.map(a => a.id),
    secrets: ["OPENAI_API_KEY", "GH_TOKEN"],
    shell: "zsh",
    aliases: [{ name: "o", word: "open" }],
    conflicts: new Set(),
    ...over,
  });
  const GOLDEN = { version: 3, createdAt: "2026-09-05T14:02:11.000Z", setupSha: "9f2a7c1d4e5b6a7f8091a2b3c4d5e6f7", npmBin: "/opt/node/bin" };
  const inputs: { name: string; input: ctx.ContextInput }[] = [
    { name: "workspace-sealed", input: { workspace: { name: "ws-1" }, golden: GOLDEN, probe: probe(), facts: FACTS } },
    { name: "builder-bare", input: { probe: probe({ kernel: undefined, disk: undefined, has: new Set(), versions: [], secrets: [], aliases: [], overlay: true }), facts: { tools: [], agents: [], files: [] } } },
    { name: "workspace-no-golden-overlay-docker", input: { workspace: { name: "w" }, probe: probe({ overlay: true, has: new Set(["docker"]) }), facts: FACTS, atProvider: true } },
    { name: "aliases-shared", input: { workspace: { name: "w" }, probe: probe({ aliasesUnread: "shared" }), facts: FACTS } },
    { name: "aliases-late", input: { workspace: { name: "w" }, probe: probe({ aliasesUnread: "late", overlay: true, has: new Set(["podman", "wsp-open"]) }), facts: FACTS } },
    ...ctx.CONTEXT_AGENTS.map(a => ({ name: `agent-${a.id}`, input: { workspace: { name: "w" }, golden: GOLDEN, probe: probe(), facts: FACTS, agent: a, atProvider: true } })),
  ];
  const PROBE_OUT = ["noise", "WSP_CTX", "KERNEL 6.6.30", "KERNEL ", "DISK 20466256 11720704", "OVERLAY yes", "HAS tmux", "HAS ", "HAS tmux", "VERSION python3: Python 3.12.13", "AGENT claude", "AGENT claude", "AGENT nobody", "SECRET OPENAI_API_KEY", "SECRET 1BAD", "SECRET GH_TOKEN", "SECRET GH_TOKEN", "SHELL zsh", "ALIAS zz open", "ALIAS a  code ", "ALIAS lonely", "ALIAS B x", "ALIASES late", "CONFLICT gemini", "CONFLICT nobody", `FACTS ${Buffer.from(JSON.stringify(FACTS)).toString("base64")}`, "WSP_CTX_END", "SHELL fish"].join("\n");
  const probes = [
    PROBE_OUT,
    "WSP_CTX\nWSP_CTX_END\n",
    "WSP_CTX_END\nWSP_CTX\n",
    "WSP_CTX\n",
    "WSP_CTX\nDISK 0 5\nALIASES unread\nFACTS !!!notbase64\nWSP_CTX_END",
    "WSP_CTX\nDISK 10\nFACTS e30=\nWSP_CTX_END",
    "WSP_CTX\nDISK abc 5\nFACTS eyJ0b29scyI6IngiLCJhZ2VudHMiOlt7ImlkIjoiYSJ9XX0=\nWSP_CTX_END",
    "WSP_CTX  \r\nKERNEL 6.1   \r\nWSP_CTX_END   ",
  ];
  const results = [
    { recipeHash: "h", tools: [{ id: "tools/a", label: "a", outcome: "installed" }, { id: "tools/b", label: "b", outcome: "failed", note: "exit 1" }, { id: "tools/brew-cask/raycast", label: "raycast", outcome: "installed" }], agents: [{ id: "agents/zed", name: "Zed", outcome: "skipped" }, { id: "agents/x", name: "X", outcome: "failed", note: "boom" }], base: [{ id: "base/zip", label: "zip", outcome: "failed", note: "apt" }], retired: [{ id: "tools/b" }], files: { bytes: 1, skipped: [{ path: "~/.ssh/id_ed25519", note: "again" }, { path: "~/.netrc", note: "secret" }] } },
    { recipeHash: "h", tools: [], agents: [] },
  ];
  write("machine_context.json", {
    constants: { SKILL_DESCRIPTION: ctx.SKILL_DESCRIPTION, SERVERS_OFF_LINE: ctx.SERVERS_OFF_LINE, BROWSER_SHIM_PATH: ctx.BROWSER_SHIM_PATH, ALIAS_PROBES: ctx.ALIAS_PROBES, contextPath: ctx.contextPath(), skillPath: ctx.skillPath(), contextPathAlt: ctx.contextPath({ etc: "/x/etc", home: "/x/home" }), skillPathAlt: ctx.skillPath({ etc: "/x/etc", home: "/x/home" }) },
    contextAgents: ctx.CONTEXT_AGENTS.map(a => a.id),
    boundedCommand: [{ seconds: 8, cmd: "zsh -lic 'x'", out: ctx.boundedCommand(8, "zsh -lic 'x'") }],
    overlayProbe: [ctx.overlayProbe(), ctx.overlayProbe("/tmp/fs")],
    probeCommand: [
      { roots: null, path: null, shells: null, out: ctx.probeCommand() },
      { roots: { etc: "/r/etc", home: "/r/home" }, path: "/p/bin", shells: "none", out: ctx.probeCommand({ etc: "/r/etc", home: "/r/home" }, "/p/bin", "none") },
    ],
    parseProbe: probes.map(s => ({ in: s, out: ctx.parseProbe(s) ?? null })),
    mergeFacts: [
      { prior: FACTS, result: results[0], out: ctx.mergeFacts(FACTS, results[0] as never) },
      { prior: FACTS, result: null, out: ctx.mergeFacts(FACTS, undefined) },
      { prior: null, result: results[1], out: ctx.mergeFacts(undefined, results[1] as never) },
      { prior: null, result: null, out: ctx.mergeFacts(undefined, undefined) },
    ],
    render: inputs.map(({ name, input }) => ({ name, long: ctx.renderMachineContext(input), short: ctx.renderShortContext(input) })),
    renderInputs: inputs.map(({ name, input }) => ({ name, input: { ...input, agent: input.agent?.id ?? null } })),
    renderSkill: ctx.renderSkill("DOC\n"),
    contextLine: [
      { agent: "claude", outcome: "written", path: "/root/.claude/CLAUDE.md", skill: "/root/.claude/skills/wsp-machine" },
      { agent: "gemini", outcome: "fallback", path: "/root/.gemini/x", note: "settings.json sets y", skill: "/s" },
      { agent: "hermes", outcome: "not-loaded", note: "config sets z", skill: "/s" },
    ].map(r => ({ in: r, out: ctx.contextLine(r as ctx.ContextResult) })),
  });
});

it("box", () => {
  const sizes = [{}, { cpu: 2, memMb: 4096 }, { cpu: 4, memMb: 8192 }, { cpu: 3 }, { memMb: 9000 }, { cpu: 2, memMb: 8192 }, { cpu: 64, memMb: 1 }, { cpu: 16, memMb: 32768 }, { cpu: 0.5, memMb: 512 }];
  const raws: unknown[] = [
    {},
    { accessTier: "trial", maxActiveSandboxes: 2, activeSandboxes: 1, starts: { minute: { limit: 5, used: 1 }, hour: { limit: 20 }, day: { limit: 75, used: 70 } }, canStart: true, creditBalanceHours: 3.5, billingStatus: "" },
    { sandboxPlanKey: "box_20", maxActiveSandboxes: 100, starts: { unlimited: true, minute: { limit: 5, used: 0 }, hour: { limit: 1, used: 0 }, day: { limit: 1, used: 0 } }, canStart: false, startBlockedReason: "billing", billingStatus: "active" },
    { sandboxPlanKey: "pro", sandboxPlanTiers: [{ key: "pro", dollars: 100 }], maxActiveSandboxes: 10, starts: { minute: { limit: 5, used: 0 }, hour: null, day: { limit: 1 } } },
    { sandboxPlanKey: "box_250", sandboxPlanDollars: "x", maxActiveSandboxes: Infinity, activeSandboxes: NaN, canStart: "no" },
    { sandboxPlanKey: "trial", sandboxPlanDollars: 500, maxActiveSandboxes: 2 },
    { sandboxPlanKey: "", sandboxPlanDollars: 99.5, maxActiveSandboxes: 0, starts: null },
  ];
  const labelSets: Record<string, string>[] = [
    {},
    { [WSP_LABEL]: "1" },
    { [NAME_LABEL]: "my ws", [OWNER_LABEL]: "zing@x.dev", [WSP_LABEL]: "1", [WORKSPACE_LABEL]: "w1", [CREATED_AT_LABEL]: "2026-09-11T06:28:06.053Z", [BUILDER_LABEL]: "1", [GOLDEN_LABEL]: "g;=%v", [HOST_LABEL]: "h", [SMOKE_LABEL]: "1", [DOCTOR_LABEL]: "1", zeta: "z", Alpha: "a", beta: "b", "a=b": "c" },
    { [OWNER_LABEL]: "o".repeat(60), [WORKSPACE_LABEL]: "w".repeat(60), [CREATED_AT_LABEL]: "t" },
    { [OWNER_LABEL]: "o".repeat(117 - 3) },
    { [OWNER_LABEL]: "o".repeat(118 - 3) },
  ];
  const names = [undefined, null, "", "wsp", "Box 2026", "wsp;o=a%3bb%3dc%25d;w=w1;zz=1;=x;noeq;t=", "wsp;o=%zz%41", "WSP;o=a", "wsp;o=1;o=2"];
  const snapNames = ["wsp-golden-v3", "WSP Golden V3", "--lead", "___", "!!!", "a".repeat(63), "a".repeat(64), `owner-${"x".repeat(80)}`, `owner-${"x".repeat(55)}--tail-differs-1`, "Ünïcode-name", ""];
  write("box.json", {
    classes: box.BOX_CLASSES,
    rootDisk: box.BOX_ROOT_DISK_GB,
    pricing: { defaultSize: box.BOX_PRICING.defaultSize, snapshotStorage: box.BOX_PRICING.snapshotStorage },
    budgets: box.BOX_BUDGETS,
    constants: { BOX_API_URL: box.BOX_API_URL, BOX_BASE_TEMPLATE: box.BOX_BASE_TEMPLATE, BOX_PAUSE_MS: box.BOX_PAUSE_MS, BOX_RESUME_MS: box.BOX_RESUME_MS, BOX_CREATE_MS: box.BOX_CREATE_MS, BOX_SNAPSHOT_MS: box.BOX_SNAPSHOT_MS, BOX_POLL_MS: box.BOX_POLL_MS, BOX_RESTORE_MS: box.BOX_RESTORE_MS, BOX_KEY_KEPT_MS: box.BOX_KEY_KEPT_MS, LOST_ANSWER_MS: box.LOST_ANSWER_MS, LOST_ANSWER_EVERY_MS: box.LOST_ANSWER_EVERY_MS, IN_PROGRESS_WAIT_MS: box.IN_PROGRESS_WAIT_MS, BACKSTOP_SLACK_MS: box.BACKSTOP_SLACK_MS, BACKSTOP_RETRY_MS: box.BACKSTOP_RETRY_MS, BACKSTOP_RETRY_MAX_MS: box.BACKSTOP_RETRY_MAX_MS, BACKSTOP_CAP_MS: box.BACKSTOP_CAP_MS, TRIAL_TTL_S: box.TRIAL_TTL_S, LIMITS_KEPT_MS: box.LIMITS_KEPT_MS, REFUSAL_LOG_QUIET_MS: box.REFUSAL_LOG_QUIET_MS, FIREWALL_HOLD_MS: box.FIREWALL_HOLD_MS, FIREWALL_WATCH_MS: box.FIREWALL_WATCH_MS, ENV_LANDING_MS: box.ENV_LANDING_MS, FILE_PUT_MAX: box.FILE_PUT_MAX, FILE_GET_MAX: box.FILE_GET_MAX, BOX_INLINE_MAX_MS: box.BOX_INLINE_MAX_MS, BOX_WORK_DIR: box.BOX_WORK_DIR, BOX_NAME_MAX: box.BOX_NAME_MAX, SNAPSHOT_NAME_MAX: box.SNAPSHOT_NAME_MAX },
    capabilities: new box.BoxBackend({ apiKey: "k" }).capabilities,
    capabilitiesKeys: Object.keys(new box.BoxBackend({ apiKey: "k" }).capabilities),
    boxClassFor: sizes.map(s => ({ in: s, out: box.boxClassFor(s).type })),
    planSizes: [[true, undefined], [true, 500], [false, undefined], [false, 99.99], [false, 100], [false, 0]].map(([t, u]) => ({ trial: t, planUsd: u ?? null, out: box.planSizes(t as boolean, u as number | undefined) })),
    limitsOf: raws.map(r => {
      const limits = box.limitsOf(r as never);
      return { in: r, out: limits, keys: Object.keys(limits), place: box.placeLimitsOf(limits) ?? null };
    }),
    boxName: labelSets.map(l => ({ in: l, out: box.boxName(l) })),
    boxLabels: names.map(n => ({ in: n ?? null, isUndefined: n === undefined, out: box.boxLabels(n) ?? null, keys: Object.keys(box.boxLabels(n) ?? {}) })),
    boxSnapshotName: snapNames.map(n => ({ in: n, out: tryOf(() => box.boxSnapshotName(n)) })),
    stopRefusalOf: [
      { error: "the disk is full" },
      { error: "", lastSnapshotStatus: "failed", lastSnapshotAttemptAt: "2026-10-01T00:00:00Z", snapshotCompletedAt: "2026-09-30T00:00:00Z" },
      { error: null, lastSnapshotStatus: "in_progress" },
      {},
      { snapshotCompletedAt: "2026-09-30T00:00:00Z" },
    ].map(v => ({ in: v, out: box.stopRefusalOf(v) })),
    envLandingScript: [{ A: "1", B: 'he said "hi" \\ there', "C D": "$x\n`y`" }, {}].map(e => ({ in: e, out: box.envLandingScript(e) })),
    sudoCommand: ["true", "echo 'x' \"y\""].map(c => ({ in: c, out: box.sudoCommand(c) })),
    DAEMON_EXITS_CMD: box.DAEMON_EXITS_CMD,
    daemonExitsOf: [
      "",
      "noise\n",
      "WSP_DAEMON_EXITS auto-restart exit-code 2\n",
      "WSP_DAEMON_EXITS failed signal 9\n",
      "WSP_DAEMON_EXITS failed core-dump 11\n",
      "WSP_DAEMON_EXITS auto-restart oom-kill 9\n",
      "WSP_DAEMON_EXITS running success 0\n",
      "WSP_DAEMON_EXITS auto-restart  \n",
      "WSP_DAEMON_EXITS failed start-limit-hit 1\n",
      "WSP_DAEMON_EXITS\n",
      `x\nWSP_DAEMON_EXITS auto-restart exit-code 2\na\nb\n  a  \nc\nd\ne\n${"u".repeat(130)}\n\n`,
      "WSP_DAEMON_EXITS auto-restart exit-code 2\nWSP_DAEMON_EXITS again\nerr\n",
    ].map(s => ({ in: s, out: box.daemonExitsOf(s) ?? null })),
  });
});
