// SPDX-License-Identifier: AGPL-3.0-only
// The TS runtime's own answers for machines/brokered.ts, written as tests/fixtures/oracle/brokered.json. Run from the
// repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import * as b from "../../../../packages/runtime/src/machines/brokered.js";

const out = fileURLToPath(new URL("../tests/fixtures/oracle/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, name), `${JSON.stringify(value, (_k, v) => (v === undefined ? null : v), 1)}\n`);
};

it("brokered", async () => {
  const statuses = [undefined, {}, { claude: "token" }, { claude: "key" }, { claude: "gateway" }, { claude: "nonsense" }, { codex: "key" }, { codex: "chatgpt" }, { claude: "key", codex: "key" }];
  const claims = [undefined, { accountId: "acct-acme" }];
  const models = { models: [{ slug: "gpt-5.5" }] };
  const launched: { command: string; env: Record<string, string> }[] = [];
  const exec = Object.assign(
    (command: string, options: { env: Record<string, string> }) => {
      launched.push({ command, env: options.env });
      return { command } as never;
    },
    { attach: () => "attached", sweep: () => "swept" },
  ) as never;
  const wrapped = b.withGhDoor(exec) as unknown as ((c: string, o: { env: Record<string, string> }) => unknown) & { attach?: unknown; sweep?: unknown };
  wrapped("echo hi", { env: { GH_CONFIG_DIR: b.BROKERED_GH_DIR } });
  wrapped("echo hi", { env: { GH_CONFIG_DIR: `${b.BROKERED_GH_DIR}/` } });
  wrapped("echo hi", { env: {} });
  const bare = b.withGhDoor(((c: string) => c) as never) as unknown as { attach?: unknown; sweep?: unknown };
  const reads = {
    none: () => undefined,
    claude: (token: string | undefined) => (token === undefined ? undefined : { vault: {}, claude: { token, kind: "token" as const } }),
    claudeKey: (token: string | undefined) => (token === undefined ? undefined : { vault: {}, claude: { token, kind: "key" as const } }),
    codexChatgpt: (token: string | undefined) => ({ vault: token === undefined ? {} : {}, codex: { mode: "chatgpt" as const, claims: { accountId: "acct-acme" } } }),
    codexKey: (token: string | undefined) => ({ vault: token === undefined ? {} : { OPENAI_API_KEY: `sk-wsp-${token}` }, codex: { mode: "apikey" as const } }),
    codexOnlyWithToken: (token: string | undefined) => (token === undefined ? undefined : { vault: {}, codex: { mode: "apikey" as const } }),
  };
  write("brokered.json", {
    constants: { BROKERED_GH_DIR: b.BROKERED_GH_DIR, BROKERED_BIN_DIR: b.BROKERED_BIN_DIR, TERMINAL_ENV: b.TERMINAL_ENV, TERMINAL_PROFILE: b.TERMINAL_PROFILE },
    terminalBinOf: ["claude", "codex", "", "a b"].map(own => ({ own, out: b.terminalBinOf(own) })),
    otherAgentProgram: [["codex", "claude"], ["claude", "codex"], ["cursor", "claude"], ["nobody", "else"], ["it's", "claude"]].map(([other, own]) => ({ other, own, out: b.otherAgentProgram(other!, own!) })),
    terminalLine: [["claude", undefined], ["claude", "#!/bin/sh\nexec claude \"$@\"\n"], ["codex", "#!/bin/sh\necho 'quoted'\n"], ["codex", undefined], ["cursor", undefined], ["nobody", "x"]].map(([own, program]) => ({ own, program: program ?? null, out: b.terminalLine(own!, program) })),
    brokeredEnv: [
      { token: "tok", repo: null, direct: [] },
      { token: "tok", repo: { owner: "acme", name: "site", public: true }, direct: [] },
      { token: "tok", repo: { owner: "acme", name: "site", public: false }, direct: ["acme/lib"] },
      { token: "tok", repo: { owner: "acme", name: "site", public: true }, direct: ["acme/site", "other/x"] },
    ].map(c => ({ ...c, out: Object.entries(b.brokeredEnv(c.token, c.repo ?? undefined, c.direct)) })),
    placeholdersOf: statuses.map(status => ({ status: status ?? null, out: b.placeholdersOf(status as never, "tok") })),
    codexBrokerOf: statuses.flatMap(status => claims.flatMap(claim => [undefined, models].map(m => ({ status: status ?? null, claims: claim ?? null, models: m ?? null, out: b.codexBrokerOf(status as never, claim as never, m as never) ?? null })))),
    unbrokeredRefusal: ["claude", "codex", "cursor", "opencode", "nobody", ""].map(h => ({ harness: h, out: b.unbrokeredRefusal(h)?.message ?? null, kind: (b.unbrokeredRefusal(h) as { kind?: string } | undefined)?.kind ?? null })),
    terminalSignIn: Object.entries(reads).flatMap(([read, fn]) => ["claude", "codex", "cursor"].map(own => ({ own, read, out: b.terminalSignIn(own, fn as never, "tok") ?? null }))),
    withGhDoor: { launched, attach: typeof wrapped.attach, sweep: typeof wrapped.sweep, bareAttach: "attach" in bare, bareSweep: "sweep" in bare },
  });
});
