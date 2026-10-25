// SPDX-License-Identifier: AGPL-3.0-only
// The TS engine's own answers for the roads units (spec/roads-README.md), written as tests/fixtures/roads/*.json for
// the Rust hard tests. Every random name is pinned to ab bytes, and the clock to 2026-10-25T00:00:00Z. Run from the
// repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 roads.oracle.ts
import { mkdirSync, mkdtempSync, readFileSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

vi.mock("node:crypto", async importOriginal => {
  const real = await importOriginal<typeof import("node:crypto")>();
  return { ...real, randomBytes: (n: number) => Buffer.alloc(n, 0xab) };
});

const E = "../../../../packages/engine/src";
const ssh = await import(`${E}/ssh-backend.ts`);
const ex = await import(`${E}/exec-detached.ts`);
const place = await import(`${E}/place-machine.ts`);
const link = await import(`${E}/link-backend.ts`);
const fake = await import(`${E}/fake-backend.ts`);
const local = await import(`${E}/local-backend.ts`);
const P = await import("../../../../packages/protocol/src/index.ts");

const out = fileURLToPath(new URL("../tests/fixtures/roads/", import.meta.url));
const write = (name: string, value: unknown): void => {
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, name), `${JSON.stringify(value, null, 1)}\n`);
};
const tryIt = (f: () => unknown): unknown => {
  try {
    return { ok: f() };
  } catch (e) {
    return { err: (e as Error).message };
  }
};
const tryAsync = async (f: () => Promise<unknown>): Promise<unknown> => {
  try {
    return { ok: await f() };
  } catch (e) {
    return { err: (e as Error).message, missingKnownHosts: e instanceof ssh.MissingKnownHostsError };
  }
};

it("ssh_dial", async () => {
  const reaches = [
    { user: "dev", host: "10.0.0.5", port: 2222, keyPath: "/tmp/k/id_ed25519" },
    { user: "dev", host: "box", port: 22 },
    { user: "a b@c", host: "Box.Example.COM", port: 22, keyPath: "/Users/John Smith/.ssh/id&x=1" },
    { user: "root", host: "[::1]", port: 22 },
    { user: "ü", host: "héllo", port: 65535 },
  ];
  const addresses = [
    "dev@box", "dev@box:2222", "box", "@box", "dev@", "dev@box:", "dev@box:abc", "dev@box: 22", "dev@box:0x16", "dev@box:22.0",
    "dev@box:1e3", "dev@box:65536", "dev@box:0", "a@b@box", "dev@[::1]:22", "dev@::1", "dev@box:-1",
  ];
  write("ssh_dial.json", {
    machineId: reaches.map(r => tryIt(() => ssh.sshMachineId(r))),
    parse: addresses.map(a => ({ address: a, plain: tryIt(() => ssh.parseSshAddress(a)), withPort: tryIt(() => ssh.parseSshAddress(a, { port: 2200, keyPath: "/tmp/k" })) })),
    loginWord: reaches.map(r => ssh.sshLoginWord(r)),
    machineName: ["box.example.com", "box", "10.0.0.5", "10.0.0", "1.2.3.4.example", ".hidden", "box.", "[::1]"].map(host => ssh.sshMachineName({ user: "u", host, port: 22 })),
    controlPath: reaches.map(r => ssh.sshControlPath(r, "/c")),
    boxWord: ["\x1b]0;pwned\x07motd\x1b[2J", "a\u0085b\u009fc\u00a0d", "x".repeat(60), "é".repeat(60), "😀".repeat(30)].map(s => [ssh.boxWord(s), ssh.boxWord(s, 300), ssh.boxWord(s, 4)]),
    sshArgs: reaches.flatMap(r => (["root", "root-by-password", "login"] as const).flatMap(as => [false, true].map(stdin => ({ as, stdin, args: ssh.sshArgs(r, "echo 'it works' $HOME", { controlDir: "/c", stdin, as }) })))),
    dialArgs: reaches.map(r => ssh.sshDialArgs(r)),
    backArgs: ssh.sshBackArgs({ reach: reaches[0], options: ["ProxyCommand=nc %h %p", "IdentityFile=\"~/my key\""] }, 4640, 4641),
    missingKnownHosts: [ssh.missingKnownHostsLine("root@box", "/etc/x"), ssh.missingKnownHostsLine("u".repeat(60), "/x".repeat(50))],
  });

  // carriedSshValues over fake `ssh -G` answers.
  const dir = mkdtempSync(join(tmpdir(), "wsp-oracle-known-"));
  const kept = join(dir, "kept");
  writeFileSync(kept, "");
  const DEFAULTS = ["user root", "hostname 203.0.113.9", "port 22", "checkhostip no", "identitiesonly no", "hostkeyalgorithms ssh-ed25519", "identityfile ~/.ssh/id_rsa", "identityfile ~/.ssh/id_ed25519", "globalknownhostsfile /etc/ssh/ssh_known_hosts", "userknownhostsfile /Users/dev/.ssh/known_hosts", "addkeystoagent false"];
  const runOf = (config: string[], code = 0) => async (_file: string, args: readonly string[]) =>
    args.includes("/dev/null") ? { exitCode: 0, stdout: DEFAULTS.join("\n") + "\n", stderr: "" } : { exitCode: code, stdout: config.join("\n") + "\n", stderr: code === 0 ? "" : "line 3: Bad configuration option: frob\n" };
  const configs: [string, string[], number?][] = [
    ["defaults", DEFAULTS],
    ["jump", [...DEFAULTS.map(l => l.replace(/^hostname .*/, "hostname 10.0.0.7")), "proxyjump bastion,ops@edge:2222"]],
    ["one jump", [...DEFAULTS, "proxyjump bastion"]],
    ["jump none", [...DEFAULTS, "proxyjump none"]],
    ["jump and command", [...DEFAULTS, "proxyjump bastion", "proxycommand nc %h %p"]],
    ["quoted", [...DEFAULTS.filter(l => !l.startsWith("identityfile")), "identityfile ~/keys/my key", 'identityfile ~/odd/a"b\\c', "identityfile it's", "identityagent /Users/dev/%h dir"]],
    ["known kept", [...DEFAULTS.filter(l => !l.startsWith("userknownhostsfile")), `userknownhostsfile ${kept} none`]],
    ["known missing", [...DEFAULTS.filter(l => !l.startsWith("userknownhostsfile")), `userknownhostsfile ${kept} relative/hosts`]],
    ["global none", [...DEFAULTS.filter(l => !l.startsWith("globalknownhostsfile")), "globalknownhostsfile none"]],
    ["port junk", DEFAULTS.map(l => l.replace(/^port .*/, "port abc"))],
    ["no user or host", DEFAULTS.filter(l => !/^(user|hostname) /.test(l))],
    ["unreadable", DEFAULTS, 255],
  ];
  const carried: unknown[] = [];
  for (const [name, config, code] of configs) carried.push({ name, config, answer: await tryAsync(() => ssh.carriedSshValues({ user: "root", host: "box", port: 2222, keyPath: "/k" }, runOf(config, code ?? 0))) });
  write("ssh_carried.json", carried);
});

it("ssh_keys", async () => {
  const FOUND = [
    "# Host [10.0.0.5]:2222 found: line 3 ",
    "[10.0.0.5]:2222 ssh-rsa AAAAB3NzaC1yc2EAAAADAQABAAAAgQDdxs0iI8J5qm7+4hxS2LKxvEImRo4YOgbsh7o/q6LcNs+/euQcFejNmlxWKGPcPx6ZAFZEvYmdwe/yDUYsoAH66WB32nACpcm8ELAXEkXxQZEjmr8daugXOpWv0qDzNu/2u5+eMJJJRNDDDupbkvmCM74T9cCAVkHDjisNybGndQ==",
    "|1|g4CYAxJ751PqMLyUC5r0xLd/ov4=|8lv3RgXEmKyoPgL0K37Xx452RDA= ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIC6sV8jQCzynkpUOM40rRIjA7tPstUSjC+A/LVMnAZhP",
    "",
  ].join("\n");
  const found = [
    [FOUND, "ssh-ed25519,rsa-sha2-512"],
    [FOUND, "rsa-sha2-512,ssh-ed25519"],
    [FOUND, "rsa-sha2-512-cert-v01@openssh.com"],
    [FOUND, ""],
    ["  h ssh-ed25519   AAAAC3NzaC1lZDI1NTE5AAAAIC6sV8jQCzynkpUOM40rRIjA7tPstUSjC+A/LVMnAZhP  trailing comment", "ssh-ed25519"],
    ["h ssh-ed25519 not*base64!", "ssh-ed25519"],
    ["h ssh-ed25519 QUJD", "ssh-ed25519"],
    ["h ssh-ed25519 QUJ", "ssh-ed25519"],
    ["h ssh-rsa QUJD\nh ssh-rsa QUJE", "ssh-rsa"],
  ];
  write("ssh_keys.json", {
    hostKeyFound: found.map(([f, a]) => ({ found: f, algorithms: a, key: ssh.hostKeyFound(f, a) ?? null })),
    target: [
      { hostname: "10.0.0.5", port: "2222" },
      { hostname: "10.0.0.5", port: "22" },
      { hostname: "10.0.0.5", port: "abc" },
      { hostname: "10.0.0.5", port: "2222.5" },
      { hostname: "10.0.0.5" },
      { hostname: "", port: "2222" },
      { hostname: "h", hostkeyalias: "alias" },
      { hostname: "h", hostkeyalias: "" },
    ].map(v => ({ values: v, target: ssh.knownHostTarget(v) ?? null })),
    files: [
      { userknownhostsfile: "/a /b  /c", globalknownhostsfile: "none /g" },
      { userknownhostsfile: "~/.ssh/known_hosts ~ ~x/y" },
      {},
    ].map(v => ({ values: v, files: ssh.knownHostFiles(v, "/home/me") })),
    clientWords: [
      "debug1: x\ndev@h: Permission denied (publickey).\n",
      "Warning: Permanently added '10.0.0.5' (ED25519) to the list of known hosts.\nreal\n",
      "Warning: Permanently added 'x' to the list of known hosts\n",
      "Warning: Permanently added 'x' to the list of known hosts.\r\n",
      "bind [127.0.0.1]:8080: Address already in use\nchannel_setup_fwd_listener_tcpip: cannot listen to port: 8080\nCould not request local forwarding.\nkept\n",
      "  debug12: deep\n  spaced  \n",
      "",
    ].map(t => ({ text: t, words: ssh.clientWords(t) })),
    refusal: [
      { stderr: "", exitCode: 255 },
      { stderr: "debug1: only\n", exitCode: 1 },
      { stderr: "x".repeat(400), exitCode: 255 },
    ].map(s => ssh.sshRefusalLine(s, { user: "dev", host: "box", port: 22 })),
    wordRefusal: ssh.SSH_WORD_REFUSAL("nonsense"),
    identity: ssh.sshIdentity("ssh-ed25519 SHA256:x", "dev"),
  });
});

it("ssh_machine", async () => {
  write("ssh_machine.json", {
    storeVars: ssh.SSH_STORE_VARS,
    loginRead: ssh.LOGIN_READ,
    readScript: ssh.SSH_READ_SCRIPT,
    factsScript: ssh.SSH_FACTS_SCRIPT,
    sudoRead: ssh.SSH_SUDO_READ,
    sudoTry: ssh.SSH_SUDO_TRY,
    putBytesScript: [ssh.putBytesScript("/home/maya/.wsp/daemon.tgz", 5, "/home/maya/.wsp/daemon.tgz.wsp-in-abababababab"), ssh.putBytesScript("file", 0, "file.t"), ssh.putBytesScript("/it's here/f", 12, "/it's here/f.t")],
    plainPath: [undefined, "", "/usr/bin::/bin", "./bin:/bin", '/a"b:/c', "/opt/my tools/bin:/usr/bin", "/usr/bin:/usr/bin", "/x//y:/z", "/a/../b:/c", "/tab\there:/d"].map(p => ({ path: p ?? null, plain: ssh.plainPath(p) })),
    dialsThis: [
      ["localhost", []], ["LOCALHOST.", []], ["127.9.9.9", []], ["[::1]", []], ["::1", []], ["0.0.0.0", []],
      ["mac", ["Mac.local"]], ["mac.local", ["mac"]], ["MAC.LOCAL.", ["mac"]], ["other", ["mac"]], ["mac", [""]], ["box.lan", ["box"]],
    ].map(([host, names]) => ({ host, names, here: ssh.sshDialsThisComputer({ user: "u", host: host as string, port: 22 }, names as string[]) })),
  });
  // readSshMachine over scripted answers, every value the TS returns.
  const answers = [
    "home /home/dev\nuser dev\npath /home/dev/.local/bin:/usr/bin\ncpu 8\nmemkb 16384000\n",
    "home /root\nuser \npath \ncpu abc\nmemkb x\nshell \nsystem Linux\narch aarch64\nstore:CLAUDE_CONFIG_DIR /root/.c\nstore:CODEX_HOME relative\n",
    "home /root\ncpu\n",
    "home /root\nhome /second\nuser a\nuser b\n",
  ];
  const reads: unknown[] = [];
  for (const stdout of answers) reads.push({ stdout, read: await tryAsync(() => ssh.readSshMachine({ user: "dev", host: "box", port: 22 }, async () => ({ exitCode: 0, stdout, stderr: "" }))) });
  reads.push({ stdout: "", read: await tryAsync(() => ssh.readSshMachine({ user: "dev", host: "box", port: 22 }, async () => ({ exitCode: 255, stdout: " out ", stderr: "debug1: a\n" }))) });
  write("ssh_read.json", reads);
});

it("exec_put", async () => {
  const recorder = (answer: (cmd: string) => { exitCode: number; stdout: string; stderr: string }) => {
    const calls: { cmd: string; timeoutMs?: number; idempotencyKey?: string; stdin?: string }[] = [];
    const machine = {
      id: "m1",
      exec: async (cmd: string, o: { timeoutMs?: number; idempotencyKey?: string; stdin?: Uint8Array } = {}) => {
        calls.push({ cmd, ...(o.timeoutMs !== undefined ? { timeoutMs: o.timeoutMs } : {}), ...(o.idempotencyKey !== undefined ? { idempotencyKey: o.idempotencyKey } : {}), ...(o.stdin !== undefined ? { stdin: Buffer.from(o.stdin).toString("base64") } : {}) });
        return answer(cmd);
      },
    };
    return { machine, calls };
  };
  const ok = (cmd: string) => ({ exitCode: 0, stdout: cmd.includes("WSP_PIECE") ? "WSP_PIECE\n" : "done\n", stderr: "" });
  const BIG = Array.from({ length: 200 }, (_, i) => `prompt line ${i} with a 'quote' and ünïcödé ${"z".repeat(60)}`).join("\n");
  const cases: [string, unknown[], unknown, unknown?][] = [
    ["one small file", [{ path: "/tmp/wsp-run/a.sh", text: "echo hi" }], {}],
    ["two folders", [{ path: "/tmp/a/x", text: "1" }, { path: "/var/b/y", text: "2" }, { path: "/tmp/a/z", text: "3" }], { before: ["b1"], after: ["a1", "a2"] }],
    ["big and small", [{ path: "/tmp/wsp-run/t1.sh", text: "claude -p" }, { path: "/tmp/wsp-run/t1.in", text: BIG }], { before: ["mkdir /tmp/wsp-run/t1.d || exit 0"], after: ["echo WSP_LAUNCHED"] }],
    ["append small", [{ path: "/tmp/wsp-run/t3.in", text: "second\n", append: true }], {}],
    ["append big", [{ path: "/tmp/wsp-run/t2.in", text: `${BIG}\n`, append: true }], {}],
    ["two appends", [{ path: "/p/t8.in", text: "a\n", append: true }, { path: "/p/t8.in", text: "b\n", append: true }], {}],
    ["empty text", [{ path: "/p/e", text: "" }], {}],
    ["stdin and timeout", [{ path: "/p/s", text: "x" }], { stdin: [1, 2, 3], timeoutMs: 5000 }],
    ["no files", [], { after: ["echo WSP_OK"] }],
    ["quote in path", [{ path: "/p/it's/f", text: "x" }], {}],
  ];
  const puts: unknown[] = [];
  for (const [name, files, o] of cases) {
    const r = recorder(ok);
    const opts = o as { stdin?: number[] };
    const res = await tryAsync(() => ex.putFiles(r.machine, files, { ...(o as object), ...(opts.stdin ? { stdin: new Uint8Array(opts.stdin) } : {}) }));
    puts.push({ name, files, opts: o, res, calls: r.calls });
  }
  // A framing that leaves no room, and one that leaves a little.
  for (const [name, pad, noRoom] of [["framed tight", 16_000, "no room for it"], ["framed some", 8_000, "no room"], ["framed no words", 16_200, undefined]] as const) {
    const r = recorder(ok);
    const framing = { wrap: (cmd: string) => `${"P".repeat(pad)}\n${cmd}`, ...(noRoom !== undefined ? { noRoom } : {}) };
    const res = await tryAsync(() => ex.putFiles({ ...r.machine, framed: async () => framing }, [{ path: "/p/f.in", text: BIG }], {}));
    puts.push({ name, pad, noRoom: noRoom ?? null, res, calls: r.calls });
  }
  let longest = 16_384;
  while (!ex.execFits("x".repeat(longest))) longest--;
  write("exec_put.json", {
    puts,
    longestFit: longest,
    fitsMulti: [ex.execFits("é".repeat(7936)), ex.execFits("é".repeat(7937))],
    machineAnswer: [
      { exitCode: 0, stdout: "", stderr: "" },
      { exitCode: 1, stdout: " a \n", stderr: "\nb\n" },
      { exitCode: 3, stdout: "", stderr: "warn" },
    ].map(r => ex.machineAnswer(r)),
    markers: [
      ["M 0 yes\nM 1 two words\nX 2 no\nM\nM 3\n  M 4 padded  \nM 0 again", "M"],
      ["", "M"],
    ].map(([s, m]) => [...ex.markersOf(s!, m!)]),
    appendMarker: ex.appendMarker("/p/x"),
  });
  // pagedReads over a recorder.
  const r = recorder(cmd => ({ exitCode: 0, stdout: cmd, stderr: "" }));
  const rows = Array.from({ length: 19 }, (_, i) => i);
  const pages = await ex.pagedReads(r.machine, rows, (row: number, at: number) => `R ${at} ${row} ${"w".repeat(row === 3 || row === 4 ? 9000 : row === 10 ? 16000 : 10)}`, "export PATH=/x");
  write("exec_paged.json", { pages: pages.map((p: { rows: number[] }) => p.rows), calls: r.calls });
});

it("exec_run", async () => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2026-10-25T00:00:00Z"));
  const calls: string[] = [];
  let polls = 0;
  const machine = {
    id: "m1",
    exec: async (cmd: string) => {
      calls.push(cmd);
      if (cmd.includes("echo WSP_LAUNCHED")) return { exitCode: 0, stdout: "WSP_LAUNCHED\n", stderr: "" };
      if (cmd.includes("echo WSP_POLL")) {
        polls++;
        return polls === 1
          ? { exitCode: 0, stdout: `WSP_POLL\n\n${Buffer.from("a\n").toString("base64")}\n\nup\nWSP_POLL_END\n`, stderr: "" }
          : { exitCode: 0, stdout: `WSP_POLL\n0\n\n${Buffer.from("e").toString("base64")}\ndown\nWSP_POLL_END\n`, stderr: "" };
      }
      return { exitCode: 0, stdout: "", stderr: "" };
    },
  };
  const res = await ex.execDetached(machine, "echo it's", { deadlineMs: 60_000, pollMs: 1 });
  vi.useRealTimers();
  const killCalls: string[] = [];
  const killer = {
    id: "m1",
    exec: async (cmd: string) => {
      killCalls.push(cmd);
      return cmd.includes("echo WSP_LAUNCHED") ? { exitCode: 0, stdout: "WSP_LAUNCHED\n", stderr: "" } : { exitCode: 0, stdout: "", stderr: "" };
    },
  };
  const killed = await ex.execDetached(killer, "sleep 9", { deadlineMs: 0, pollMs: 1 }, "/r");
  write("exec_run.json", { res, calls, killed, killCalls });
});

it("place_machine", async () => {
  const frames: unknown[] = [];
  const road = {
    request: async (op: string, params: Record<string, unknown> = {}, opts: Record<string, unknown> = {}) => {
      frames.push({ op, params, opts });
      const cmd = String(params["cmd"] ?? "");
      if (cmd.startsWith("uname -s")) return { exitCode: 0, stdout: "Linux\n1000\nmaya\nmaya\n1000\n/home/maya\n/usr/bin:/bin\n", stderr: "", truncated: false };
      if (cmd.includes("-ilc ")) return { exitCode: 0, stdout: "WSP_PATH_BEGIN/home/maya/.local/bin:/usr/bin:/binWSP_PATH_END\n", stderr: "", truncated: false };
      return { exitCode: 0, stdout: "", stderr: "", truncated: false };
    },
  };
  const m = new place.PlaceMachine(road, { id: "spoo", home: "/root" });
  await m.putBytes("/root/x y/f", Buffer.alloc(place.PLACE_PART_BYTES + 3, 1));
  const putFrames = frames.splice(0).map(f => {
    const g = f as { params: Record<string, unknown> };
    return { ...g, params: { ...g.params, ...(g.params["stdin"] !== undefined ? { stdin: `${String(g.params["stdin"]).length} base64 chars` } : {}) } };
  });
  const folder = new place.PlaceFolderMachine(road, { id: "spoo", home: "/home/maya", path: "/usr/bin:/bin" });
  await folder.exec("echo hi");
  const folderFrames = frames.splice(0);
  const grouped = folder.inCgroup(P.threadCgroup("thread-1"), "/home/maya/.wsp/bin");
  await grouped.exec("echo hi");
  const groupedFrames = frames.splice(0);
  const framed = await folder.framed();
  write("place_machine.json", {
    bounds: [0, 1, 1024 * 1024, 4 * 1024 * 1024, 64 * 1024 * 1024, 5_000_000].map(b => [b, place.placePartBoundMs(b), place.placePartBoundMs(b, 1)]),
    putFrames,
    folderFrames,
    groupedFrames,
    framedNoRoom: framed.noRoom ?? null,
    framedWrap: framed.wrap("echo hi"),
  });
});

it("link_backend", async () => {
  const sent: unknown[] = [];
  const FACTS = {
    offer: "docker",
    capabilities: { liveCloneForks: false, replacesMachine: true, previewUrls: false, signedUrls: false, callbackRelay: true, diskSnapshots: true, images: true, snapshotsAnyLife: false, snapshotListing: true, templates: true, kept: false, copies: true, ownNetwork: true, sizes: [{ cpu: 2, memMb: 4096, rateUsdPerHour: 0.5 }] },
    pricing: { defaultSize: { cpu: 2, memMb: 4096 }, snapshotStorage: { freeGb: 0, usdPerGbMonth: 0, billedFrom: "" } },
    logins: "/wsp/logins",
  };
  const l = {
    request: async (op: string, params: Record<string, unknown> = {}, opts: Record<string, unknown> = {}) => {
      sent.push({ op, params, opts });
      if (op === "machine.backend") return FACTS;
      if (op === "machine.create" || op === "machine.get") return { machine: { id: "c1", kind: "sandbox", roads: { previewUrl: true, daemonAnswers: true, putBytes: true, describe: true, facts: true, metrics: true } } };
      if (op === "machine.exec") return { result: { exitCode: 0, stdout: "", stderr: "" } };
      if (op === "exec") return { exitCode: 0, stdout: "up\n", stderr: "", truncated: false };
      return {};
    },
    forward: async () => ({ localPort: 1 }),
  };
  const backend = await link.LinkBackend.open(l);
  const machine = await backend.create({ kind: "sandbox", labels: { a: "1" }, idempotencyKey: "k1", cpu: 2 });
  await machine.exec("echo", { stdin: new Uint8Array([0, 255]), idempotencyKey: "e1", timeoutMs: 7 });
  await machine.putBytes("/root/f", new Uint8Array([1, 2, 3]));
  await backend.onComputer("uptime");
  write("link_backend.json", { sent, rates: [backend.pricing.rateUsdPerHour({ cpu: 2, memMb: 4096 }), backend.pricing.rateUsdPerHour({ cpu: 4, memMb: 4096 })] });
});

it("fake_backend", async () => {
  vi.useFakeTimers({ toFake: ["Date"] });
  vi.setSystemTime(new Date("2026-10-25T00:00:00Z"));
  const dir = mkdtempSync(join(tmpdir(), "wsp-oracle-fake-"));
  const records = join(dir, "deep", "records.json");
  const b = new fake.FakeBackend({ records });
  const made = await b.create({ kind: "sandbox", cpu: 2, labels: { "wsp-workspace": "w1" } });
  await (await b.get("fk_c0ffee")).pause();
  await made.snapshot("v1", { firstLife: true });
  const afterCreate = readFileSync(records, "utf8");
  await b.deleteSnapshot("nothing");
  vi.useRealTimers();
  write("fake_backend.json", {
    records: afterCreate,
    list: await b.list(),
    capabilities: b.capabilities,
    withGuest: new fake.FakeBackend({ guest: { folder: () => "/", tokenPath: () => "/t", reach: async () => ({ url: "", token: "", expiresAt: 0 }) } }).capabilities.previewUrls,
    rates: [[2, 4096], [4, 8192], [8, 16384], [3, 1000], [1, 1]].map(([cpu, memMb]) => b.pricing.rateUsdPerHour({ cpu: cpu!, memMb: memMb! })),
    seen: made.seen,
    describe: await made.describe!(),
    facts: await made.facts!(),
    unreadable: fake.standInRecordsUnreadable("/r.json", "Unexpected end of JSON input"),
  });
});

it("local_backend", async () => {
  write("local_backend.json", {
    shape: [[10, 16_654_508_032], [10, 16 * 1024 ** 3], [8, 8 * 1024 ** 3], [1, 512 * 1024 ** 2], [4, 1536 * 1024 ** 2], [2, 0]].map(([c, t]) => local.localShape(c, t)),
    meminfo: ["MemTotal:       16303412 kB\nMemFree: 1 kB\nMemAvailable:    9216000 kB\n", "MemTotal: 10 kB\n", "MemAvailable: 2047 kB\nMemTotal: 2048 kB\n", " MemTotal: 2048 kB\nMemAvailable: 1024 kB\n", "MemTotal:2048kB\nMemAvailable:\t1024 kB"].map(s => local.memoryOfMeminfo(s) ?? null),
    pressure: ["54\n", "", "x", "101", "-1", "5.5", " 7 ", "0", "100", "1e1", "0x10"].map(s => [s, local.memoryOfPressure(s, 16 * 1024 ** 3) ?? null]),
  });
});
