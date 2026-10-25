// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers for the edge inputs wave 2's review of the specs found unpinned, one section per unit, written as
// tests/fixtures/scenes/units.json and read by each unit's `<unit>_pins.rs`. Math.random is pinned to 0 and random
// bytes to ab. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 unit_pins.oracle.ts
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

vi.mock("node:crypto", async importOriginal => {
  const real = await importOriginal<typeof import("node:crypto")>();
  return { ...real, randomBytes: (n: number) => Buffer.alloc(n, 0xab) };
});

const E = "../../../../packages/engine/src";
const errors = await import(`${E}/errors.ts`);
const facts = await import(`${E}/machine-facts.ts`);
const ctx = await import(`${E}/machine-context.ts`);
const box = await import(`${E}/box-backend.ts`);
const ex = await import(`${E}/exec-detached.ts`);
const ssh = await import(`${E}/ssh-backend.ts`);
const local = await import(`${E}/local-backend.ts`);
const none = await import(`${E}/no-provider-backend.ts`);
const fake = await import(`${E}/fake-backend.ts`);

const out = fileURLToPath(new URL("../tests/fixtures/scenes/", import.meta.url));
const tryIt = (f: () => unknown): unknown => {
  try {
    return { ok: f() ?? null };
  } catch (e) {
    return { err: (e as Error).message };
  }
};
const tryAsync = async (f: () => Promise<unknown>): Promise<unknown> => {
  try {
    return { ok: (await f()) ?? null };
  } catch (e) {
    return { err: (e as Error).message };
  }
};
const b64 = (s: string): string => Buffer.from(s).toString("base64");

it("unit pins", async () => {
  vi.spyOn(Math, "random").mockReturnValue(0);

  const errorsPins = {
    backoff: [0, 1, 4, 5, 9, 31, 63, 64, 70, 2000].map(a => ({ attempt: a, ms: errors.backoffMs(a) })),
    network: [
      { status: 503, error: "fetch failed" },
      { status: 502, code: "ECONNRESET" },
      { status: 500, code: "ECONNREFUSED" },
      { status: 502, error: "bad gateway" },
    ].map(c => ({ in: c, thrown: errors.isNetworkError(Object.assign(new Error(c.error ?? "x"), errors.classify(c.status, { ...(c.error !== undefined ? { error: c.error } : {}), ...(c.code !== undefined ? { code: c.code } : {}) }))), network: errors.isNetworkError(errors.classify(c.status, { ...(c.error !== undefined ? { error: c.error } : {}), ...(c.code !== undefined ? { code: c.code } : {}) })), neverSent: errors.neverSent(errors.classify(c.status, { ...(c.code !== undefined ? { code: c.code } : {}) })) })),
  };

  const factsPins = {
    uptime: [
      { uptime: "-0.0025" }, { uptime: "-0.0005" }, { uptime: "0.0005" }, { uptime: "1e3" }, { uptime: "0x10" }, { uptime: "Infinity" }, { uptime: " 12.3456 " },
      { boot: "{ usec = 5, sec = 7 }" }, { boot: "{ sec = 1, usec = 2 }" }, { boot: "sec = 007" }, { boot: "sec = 99999999999999999999" }, { boot: "sec = ٣" }, { uptime: "", boot: "sec = 1" }, { uptime: "abc", boot: "sec = 1" },
    ].map(v => ({ in: v, ms: facts.uptimeMsOf(v as Record<string, string>, 5_000_000) ?? null })),
    mem: ["0b11", "0o17", ".5", "+2048", "1_0", "1,000", "5kB", "1e400", "100", "511", "512", "1e30"].map(m => ({ in: m, mb: facts.memMbOf({ memkb: m }) ?? null })),
    lastValueWins: facts.readValues("a 1\na \nb  2\nc\td\n﻿arch x\n"),
  };

  const probe = (lines: string[]): unknown => {
    const p = ctx.parseProbe(["WSP_CTX", ...lines, "WSP_CTX_END"].join("\n"));
    return p === undefined ? null : { kernel: p.kernel ?? null, disk: p.disk ?? null, facts: p.facts ?? null, aliases: p.aliases, has: [...p.has] };
  };
  const contextPins = {
    disk: ["DISK 0x10 0x8", "DISK 1e3 5", "DISK 100 50 7", "DISK 100 -5", "DISK -0 5", "DISK 100 1e400", "DISK  100   50", "DISK 100"].map(l => ({ in: l, out: probe([l]) })),
    facts: [
      b64('{"tools":[],"agents":[],"files":[]}').replace(/=+$/, ""),
      `${b64("{}").slice(0, 2)} ${b64("{}").slice(2)}`,
      `${b64("{}")}junk`,
      b64("null"),
      b64("5"),
      b64("[]"),
      b64('"abc"'),
      "!!!",
      "",
    ].map(f => ({ in: f, out: probe([`FACTS ${f}`]) })),
    keys: [["KERNEL\t6.6"], [" KERNEL 6.6"], ["KERNEL   6.6  "], ["ALIAS ll\tls -l"], ["ALIAS  ll ls"], ["HAS  docker "]].map(l => ({ in: l, out: probe(l) })),
    trimEnd: [["WSP_CTX\u0085", "KERNEL a", "WSP_CTX_END"], ["WSP_CTX﻿", "KERNEL b", "WSP_CTX_END"], ["WSP_CTX ", "KERNEL c", "WSP_CTX_END"]].map(l => {
      const p = ctx.parseProbe(l.join("\n"));
      return { in: l, kernel: p === undefined ? null : p.kernel ?? null };
    }),
  };

  const planPins = {
    labels: ["wsp;o=%c3%a9", "wsp;o=%E2%82%AC;w=%41", "wsp;o=%", "wsp;o=%4"].map(n => ({ in: n, out: box.boxLabels(n) ?? null })),
    keylessTier: box.limitsOf({ sandboxPlanTiers: [{ dollars: 100 }] } as never),
    classForNaN: box.boxClassFor({ cpu: NaN, memMb: NaN }).type,
  };

  const requestPins = {
    stopRefusal: [
      { lastSnapshotStatus: "", lastSnapshotAttemptAt: "" },
      { lastSnapshotStatus: "failed", lastSnapshotAttemptAt: "" },
      { error: "   " },
      { lastSnapshotStatus: "failed", snapshotCompletedAt: "" },
    ].map(v => ({ in: v, out: box.stopRefusalOf(v as never) })),
  };

  const putCalls = async (files: { path: string; text: string; append?: boolean }[]): Promise<string[]> => {
    const sent: string[] = [];
    const machine = { id: "m1", exec: async (cmd: string) => (sent.push(cmd), { exitCode: 0, stdout: "", stderr: "" }) };
    await ex.putFiles(machine as never, files, {});
    return sent;
  };
  const putPins = {
    markers: ["M  x y\nM k a  b\nM\tk v\nM k2", "M a 1\nM a 2", " M b  2 "].map(s => ({ in: s, out: [...ex.markersOf(s, "M").entries()] })),
    dirnames: await Promise.all(["a.txt", "a//b/c", "/f", "///a", "a/./b", "a/b/"].map(async p => ({ path: p, sent: await putCalls([{ path: p, text: "x" }]) }))),
  };

  const keysPins = {
    target: ["", "0", "-1", "99999", "1e3", "0x16", "0x8ae", "22.0", "Infinity", " 2222 "].map(p => ({ port: p, out: ssh.knownHostTarget({ hostname: "h", port: p }) ?? null })),
    files: [
      ["/home/x/", "~/.ssh/k"], ["/home/x", "~/a//b"], ["/home/x", "~/../etc/k"], ["/home/x", "~/./k"], ["/home/x", "~/"], ["/home/x", "~/a ~/b"],
    ].map(([home, file]) => ({ home, file, out: ssh.knownHostFiles({ userknownhostsfile: file! }, home) })),
    refusal: [
      { stderr: `a\n${"b".repeat(299)}`, exitCode: 255 },
      { stderr: "", exitCode: 124 },
      { stderr: "debug1: x\n  \n", exitCode: -1 },
    ].map(s => ({ in: s, out: ssh.sshRefusalLine(s, { user: "u", host: "h", port: 22 }) })),
    clientWords: ["bind [::1]:80: Address already in use\r\nreal", "Warning: Permanently added 'a\rb' (ED25519) to the list of known hosts.", "debug١: x", "debug1: y\nkept"].map(s => ({ in: s, out: ssh.clientWords(s) })),
  };

  const sudoRead = async (stdout: string): Promise<unknown> =>
    tryAsync(() => ssh.readSshSudo({ user: "u", host: "h", port: 22 }, async () => ({ exitCode: 0, stdout, stderr: "" })));
  const machinePins = {
    putScript: ["a//b", "a/b//c", "//a", "///a/b", "/a//", "a/", "./a", "..", "a/b/"].map(p => ({ path: p, out: ssh.putBytesScript(p, 3, "/tmp/wsp-put-x") })),
    dials: ["localhost..", "[::1].", "127.foo.example", "LOCALHOST.", "::1", "0.0.0.0"].map(h => ({ host: h, out: ssh.sshDialsThisComputer({ user: "u", host: h, port: 22 }, ["mac"]) })),
    sudo: await Promise.all(["a\r\nWSP_SUDO root\r\nWSP_SUDO none\n", "x WSP_SUDO free", "foo WSP_SUDO root\n", "WSP_SUDO tty \n", "WSP_SUDO asks"].map(async s => ({ stdout: s, out: await sudoRead(s) }))),
  };

  const localPins = {
    pressure: ["0b11", "0o7", "+5", "5.", ".5e1", "Infinity", "1_0", "-0", "0x1f", " 7 "].map(l => ({ in: l, out: local.memoryOfPressure(l, 16 * 1024 ** 3) ?? null })),
    meminfo: ["MemTotal:\n2048 kB\nMemAvailable: 1024 kB", "MemAvailable: 1 kB\rMemTotal: 4096 kB", "MemTotal: 2048 KB\nMemAvailable: 10 kB", "MemTotal: 4096 kB\nMemAvailable: 1024 kB\nMemTotal: 8192 kB"].map(m => ({ in: m, out: local.memoryOfMeminfo(m) ?? null })),
  };

  const nonePins = {
    emptyLine: await tryAsync(() => new none.NoProviderBackend("").get("x")),
  };

  const fakePins = {
    rates: [{ cpu: 0, memMb: 3200 }, { cpu: 0, memMb: 9600 }, { cpu: 1, memMb: 512 }, { cpu: 0.5, memMb: 0 }].map(s => ({ in: s, out: new fake.FakeBackend({ path: "/nonexistent/records.json" }).pricing.rateUsdPerHour(s) })),
  };

  mkdirSync(out, { recursive: true });
  writeFileSync(
    join(out, "units.json"),
    `${JSON.stringify({ errors: errorsPins, machine_facts: factsPins, context_probe: contextPins, box_plan: planPins, box_request: requestPins, exec_put: putPins, ssh_keys: keysPins, ssh_machine: machinePins, local_backend: localPins, machine: nonePins, fake_backend: fakePins }, (_k, v) => (typeof v === "number" && !Number.isFinite(v) ? `${v}` : v), 1)}\n`,
  );
});
