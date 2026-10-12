// SPDX-License-Identifier: AGPL-3.0-only
// A slate's command on the thread's own machine: the launcher run by this computer's bash as a machine's would run
// it, the road over a machine whose exec and run are this computer's bash (putFiles and all), and a box thread's run
// reaching its machine through the runtime while one `on` the host stays here.
import { execFileSync, spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, readFileSync, realpathSync, rmdirSync, rmSync, statSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { isAbsolute, join, relative } from "node:path";
import { afterEach, describe, expect, it, vi } from "vitest";
import { PlaceFolderMachine, type ExecResult, type Machine, type RunOptions } from "@wsp/engine";
import type { Caller } from "@wsp/protocol";
import { boxLauncher, boxLedger, boxRoad } from "../src/slate-box.js";
import { boxSlateDir } from "../src/slate-mcp-on.js";
import { cgroupJoinLine, FS_HASH_CAP_BYTES, FS_HASH_FILES_MAX, FS_HASH_PATHS_MAX, threadCgroup } from "@wsp/protocol";
import { slateHostAway, slateServerNotSetUp } from "@wsp/protocol/slate";
import type { McpPipeOpen } from "../src/slate-mcp.js";
import { pipeOn, type StdioRoad } from "../src/slate-mcp-on.js";
import { boxPins, type HashOn, type HashRead } from "../src/slate-files.js";
import { createSlateRuns, type RoadEnd } from "../src/slate-runs.js";
import { SLATES } from "../src/lazy-slates.js";
import { createSlates } from "../src/slates.js";
import { memoryStore } from "../src/store.js";
import { writeStub } from "../../protocol/test/stub-script.js";

const made: string[] = [];
afterEach(() => {
  for (const dir of made.splice(0)) rmSync(dir, { recursive: true, force: true });
});
const temp = (): string => {
  const dir = realpathSync(mkdtempSync(join(tmpdir(), "slate-box-")));
  made.push(dir);
  return dir;
};
const nul = (items: string[]): string => Buffer.from(items.map(i => `${i}\0`).join("")).toString("base64");

/** This computer's bash as a machine: exec runs a command to its end, run runs a script, streaming its lines, killed
 * at its deadline or on its signal as a detached run on a box is. */
function bashMachine(): Machine & { killed: () => boolean } {
  let killed = false;
  return {
    id: "m1",
    kind: "sandbox",
    killed: () => killed,
    async exec(cmd: string, opts?: { stdin?: Uint8Array }): Promise<ExecResult> {
      try {
        const stdout = execFileSync("bash", ["-c", cmd], { input: opts?.stdin, encoding: "utf8" });
        return { exitCode: 0, stdout, stderr: "" };
      } catch (e) {
        const x = e as { status?: number; stdout?: string; stderr?: string };
        return { exitCode: x.status ?? 1, stdout: x.stdout ?? "", stderr: x.stderr ?? "" };
      }
    },
    run(script: string, opts: RunOptions): Promise<ExecResult> {
      return new Promise(resolve => {
        const child = spawn("bash", ["-c", script], { detached: true });
        let out = "";
        let err = "";
        let code: number | undefined;
        const lines = (chunk: Buffer, into: "out" | "err"): void => {
          const text = chunk.toString("utf8");
          if (into === "out") out += text;
          else err += text;
          for (const line of text.split("\n").filter(Boolean)) opts.onLine?.(line);
        };
        child.stdout.on("data", c => lines(c as Buffer, "out"));
        child.stderr.on("data", c => lines(c as Buffer, "err"));
        const end = (exit: number): void => {
          code ??= exit;
          try {
            process.kill(-child.pid!, "SIGKILL");
          } catch {
            // already gone
          }
        };
        const deadline = setTimeout(() => end(124), opts.deadlineMs);
        opts.signal?.addEventListener("abort", () => {
          killed = true;
          end(130);
        });
        child.on("close", exit => {
          clearTimeout(deadline);
          resolve({ exitCode: code ?? exit ?? -1, stdout: out, stderr: err });
        });
      });
    },
  } as unknown as Machine & { killed: () => boolean };
}

describe("the launcher a machine runs", () => {
  it("hands the command its values as environment, positional parameters and stdin, whole, runs none of them, and leaves none on disk", () => {
    const dir = temp();
    const p = join(dir, ".run-x");
    const value = 'two\nlines "$HOME" `touch pwned-env` $(touch pwned-env2)';
    mkdirSync(p);
    writeFileSync(`${p}/env`, nul(["NAME", value, "SLATE_DIR", "/tmp/s"]));
    writeFileSync(`${p}/args`, nul(["a b", "$(touch pwned-arg)", ""]));
    writeFileSync(`${p}/in`, Buffer.from("in put\nsecond").toString("base64"));
    const cmd = `printf '%s|' "$NAME" "$1" "$2" "$3" "$#" "$SLATE_DIR"; echo; cat; echo; pwd; ls -a ${JSON.stringify(dir)} | grep -c run || true`;
    const out = execFileSync("bash", ["-c", boxLauncher({ payload: p, cwd: dir, cmd })], { cwd: tmpdir(), encoding: "utf8" });
    // The values whole, the arguments counted, stdin as typed, the folder, and no file of the values left behind.
    expect(out).toBe(`${value}|a b|$(touch pwned-arg)||3|/tmp/s|\nin put\nsecond\n${dir}\n0\n`);
    for (const pwned of ["pwned-env", "pwned-env2", "pwned-arg"]) expect(existsSync(join(dir, pwned))).toBe(false);
  });

  it("says plainly when the folder is not on the machine", () => {
    const dir = temp();
    const p = join(dir, ".run-y");
    mkdirSync(p);
    for (const name of ["env", "args", "in"]) writeFileSync(`${p}/${name}`, "");
    let said = "";
    try {
      execFileSync("bash", ["-c", boxLauncher({ payload: p, cwd: "/nowhere/at/all", cmd: "true" })], { encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] });
    } catch (e) {
      said = String((e as { stderr?: string }).stderr);
    }
    expect(said).toBe("the folder /nowhere/at/all does not exist\n");
  });
});

describe("the road to a thread's machine", () => {
  const ended = (start: (end: (o: RoadEnd) => void) => void): Promise<RoadEnd> => new Promise(resolve => start(resolve));

  it("writes the slate's files to its folder there, sweeping what the record no longer holds, and runs the command with its values, folder and lines", async () => {
    const machine = bashMachine();
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const dir = boxSlateDir(thread);
    made.push(dir);
    mkdirSync(dir, { recursive: true });
    writeFileSync(join(dir, "stale.py"), "old");
    const cwd = temp();
    const road = boxRoad(machine, thread, () => ({ "hi.sh": "echo from the file" }));
    expect(road.slateDir).toBe(dir);
    const lines: string[] = [];
    const end = await ended(done =>
      road.start({ cmd: `sh "$SLATE_DIR/hi.sh"; printf '%s|%s\\n' "$X" "$1"; pwd; cat`, args: ["$(touch nope)"], cwd, env: { SLATE_DIR: dir, X: "x y" }, stdin: "IN", timeoutS: 20, stream: true }, { line: (_s, text) => lines.push(text), end: done }),
    );
    expect(end).toMatchObject({ code: 0, timedOut: false, cut: false });
    expect(end.out).toBe(`from the file\nx y|$(touch nope)\n${cwd}\nIN`);
    expect(lines).toEqual(["from the file", "x y|$(touch nope)", cwd, "IN"]);
    expect(readFileSync(join(dir, "hi.sh"), "utf8")).toBe("echo from the file");
    expect(existsSync(join(dir, "stale.py"))).toBe(false);
    expect(execFileSync("bash", ["-c", `ls -a ${JSON.stringify(dir)}`], { encoding: "utf8" })).not.toContain(".run-");
  });

  it("the folder a run's values go up in is the account's alone before they are decoded into it", async () => {
    const base = bashMachine();
    const modes: number[] = [];
    const machine: Machine = {
      ...base,
      run: (script, opts) => {
        const payload = /^p='([^']+)'$/m.exec(script)?.[1];
        if (payload !== undefined) modes.push(statSync(payload).mode & 0o777);
        return base.run(script, opts);
      },
    };
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const end = await ended(done => boxRoad(machine, thread, () => ({})).start({ cmd: "true", args: [], cwd: temp(), env: { TOKEN: "s" }, stdin: "", timeoutS: 20, stream: false }, { line: () => {}, end: done }));
    expect(end).toMatchObject({ code: 0 });
    expect(modes).toEqual([0o700]);
  });

  it("hands the launch alone to the machine standing in the thread's cgroup, and runs and writes through the machine itself", async () => {
    const base = bashMachine();
    const handed: (Machine | undefined)[] = [];
    const grouped: string[] = [];
    const machine: Machine = { ...base, run: (script, opts) => (handed.push(opts.launchOn), base.run(script, opts)) };
    const launchOn: Machine = { ...base, run: (script, opts) => (grouped.push(script), base.run(script, opts)) };
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const end = await ended(done => boxRoad(machine, thread, () => ({ "a.sh": "echo a" }), async () => {}, boxLedger(), launchOn).start({ cmd: "true", args: [], cwd: temp(), env: {}, stdin: "", timeoutS: 20, stream: false }, { line: () => {}, end: done }));
    expect(end).toMatchObject({ code: 0 });
    expect([handed, grouped]).toEqual([[launchOn], []]);
    expect(readFileSync(join(boxSlateDir(thread), "a.sh"), "utf8")).toBe("echo a");
  });

  it("on a computer the person joined, launches the command from a shell standing in the thread's own cgroup", async () => {
    const base = bashMachine();
    const asked: string[] = [];
    const handed: (Machine | undefined)[] = [];
    const launchOn: Machine = { ...base };
    const joined = Object.assign(Object.create(PlaceFolderMachine.prototype) as PlaceFolderMachine, base, {
      run: (script: string, opts: RunOptions) => (handed.push(opts.launchOn), base.run(script, opts)),
      inCgroup: (cgroup: string) => (asked.push(cgroup), launchOn),
      framed: async () => undefined,
    });
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const end = await ended(done => boxRoad(joined, thread, () => ({})).start({ cmd: "true", args: [], cwd: temp(), env: {}, stdin: "", timeoutS: 20, stream: false }, { line: () => {}, end: done }));
    expect(end).toMatchObject({ code: 0 });
    expect([asked, handed]).toEqual([[threadCgroup(thread)], [launchOn]]);
  });

  it("on a computer the person joined, joins the thread's cgroup for the launch alone, never for its polls, its kill or its cleanup", async () => {
    // The real road: a PlaceFolderMachine whose daemon answers each exec frame by running it in a local bash as the
    // login it reads, its inCgroup, execDetached and boxRoad.
    const home = realpathSync(mkdtempSync(join(tmpdir(), "wsp-slate-place-")));
    made.push(home);
    const cgroup = threadCgroup(`t-${Date.now()}-${Math.random().toString(36).slice(2)}`);
    const thread = cgroup.slice(cgroup.lastIndexOf("/") + 1);
    made.push(boxSlateDir(thread));
    // A Mac has no setsid and its base64 takes no -w0: shims where there is no /proc, as the engine's own run tests do.
    const shims = join(home, ".shims");
    mkdirSync(shims);
    if (!existsSync("/proc/1/stat")) {
      writeStub(join(shims, "setsid"), "#!/bin/sh\nexec perl -e 'setpgrp(0, 0); exec @ARGV or die $!' -- \"$@\"\n");
      writeStub(join(shims, "base64"), '#!/bin/sh\nargs=""\nfor a in "$@"; do [ "$a" = "-w0" ] || args="$args $a"; done\nexec /usr/bin/base64 $args\n');
    }
    const frames: string[] = [];
    const road = {
      request: async (_op: string, params: Record<string, unknown> = {}) => {
        const cmd = String(params["cmd"] ?? "");
        frames.push(cmd);
        const stdin = typeof params["stdin"] === "string" ? Buffer.from(params["stdin"], "base64") : undefined;
        const ran = spawnSync("bash", ["-c", cmd], { input: stdin, encoding: "utf8", timeout: 20_000, env: { ...process.env, PATH: `${shims}:${process.env["PATH"] ?? ""}` } });
        return { exitCode: ran.status ?? 1, stdout: ran.stdout, stderr: ran.stderr, truncated: false };
      },
    };
    // Root on cgroup v2 may make the cgroup; anywhere else the join stands down and the run goes on where it is.
    const makesCgroups = process.getuid?.() === 0 && existsSync("/sys/fs/cgroup/cgroup.controllers");
    try {
      const end = await ended(done =>
        boxRoad(new PlaceFolderMachine(road, { id: "spoo", home }), thread, () => ({})).start(
          { cmd: "sleep 1; echo ran; sed -n 's/^0:://p' /proc/self/cgroup 2>/dev/null; true", args: [], cwd: home, env: {}, stdin: "", timeoutS: 20, stream: false },
          { line: () => {}, end: done },
        ),
      );
      expect(end, end.err).toMatchObject({ code: 0 });
      expect(end.out.split("\n")[0]).toBe("ran");
      const join = cgroupJoinLine(cgroup);
      const launched = frames.findIndex(cmd => cmd.includes("setsid nohup"));
      // The launch is the frames that write the run's script and start it; every frame after it reads, ends or cleans.
      expect(frames.some(cmd => cmd.includes(join))).toBe(true);
      expect(frames.every((cmd, at) => !cmd.includes(join) || at <= launched)).toBe(true);
      if (makesCgroups) expect(end.out.split("\n")[1]).toBe(cgroup);
    } finally {
      for (const at of makesCgroups ? [cgroup, cgroup.slice(0, cgroup.lastIndexOf("/"))] : []) {
        try {
          rmdirSync(`/sys/fs/cgroup${at}`);
        } catch {
          // gone already, or another thread's cgroup still stands under it
        }
      }
    }
  });

  it("values a box left when it stopped answering go once it answers, and every contact sweeps stale values but a run's in flight", async () => {
    const machine = bashMachine();
    let down = false;
    let gate: (() => void) | undefined;
    const exec = machine.exec.bind(machine);
    const run = machine.run.bind(machine);
    machine.exec = (cmd, opts) => (down ? Promise.reject(new Error("the link went")) : exec(cmd, opts));
    machine.run = async (script, opts) => {
      if (script.includes("LINK_GOES")) {
        down = true;
        throw new Error("the link went");
      }
      if (script.includes("GATED")) await new Promise<void>(r => (gate = r));
      return run(script, opts);
    };
    const ledger = boxLedger(200);
    const [one, two, three] = [1, 2, 3].map(n => `t-${n}-${Date.now()}-${Math.random().toString(36).slice(2)}`) as [string, string, string];
    for (const t of [one, two, three]) made.push(boxSlateDir(t));
    const runsIn = (t: string): string[] => (existsSync(boxSlateDir(t)) ? readdirSync(boxSlateDir(t)).filter(n => n.startsWith(".run-")) : []);
    const go = (t: string, cmd: string, env: Record<string, string> = {}): Promise<RoadEnd> =>
      ended(done => boxRoad(machine, t, () => ({}), async () => {}, ledger).start({ cmd, args: [], cwd: temp(), env, timeoutS: 20, stream: false }, { line: () => {}, end: done }));

    // The link goes after the values went up: they stay, owed, and go on the retry once the box answers again.
    expect((await go(one, "echo LINK_GOES", { TOKEN: "s3cret" })).error?.message).toBe("the link went");
    expect(runsIn(one)).toHaveLength(1);
    expect(ledger.owed(machine)).toHaveLength(1);
    down = false;
    await vi.waitFor(() => expect(runsIn(one)).toHaveLength(0), { timeout: 5_000 });
    expect(ledger.owed(machine)).toHaveLength(0);

    // Stale values in another thread's folder go at the next contact; the values of a run still starting stay.
    mkdirSync(join(boxSlateDir(two), ".run-stale"), { recursive: true });
    writeFileSync(join(boxSlateDir(two), ".run-stale", "env"), "s3cret");
    const gated = go(one, 'echo GATED "$TOKEN"', { TOKEN: "still here" });
    await vi.waitFor(() => expect(gate).toBeDefined());
    expect(runsIn(one)).toHaveLength(1);
    expect((await go(three, "echo hi")).out).toBe("hi\n");
    expect(runsIn(two)).toHaveLength(0);
    expect(runsIn(one)).toHaveLength(1);
    gate!();
    expect((await gated).out).toBe("GATED still here\n");
    expect(runsIn(one)).toHaveLength(0);
    ledger.close();
  });

  it("a kill cancels the command on the machine, and a then reshapes there", async () => {
    const machine = bashMachine();
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const road = boxRoad(machine, thread, () => ({}));
    const t0 = Date.now();
    const end = await ended(done => {
      const started = road.start({ cmd: "sleep 30", args: [], cwd: temp(), env: {}, timeoutS: 60, stream: false }, { line: () => {}, end: done });
      setTimeout(() => started.kill(), 500);
    });
    expect(Date.now() - t0).toBeLessThan(10_000);
    expect(machine.killed()).toBe(true);
    expect(end.code).toBe(130);
    const shaped = await road.reshape({ cmd: `printf '{"got":"%s"}' "$(cat)"`, input: "a b", cwd: temp(), env: {}, timeoutS: 20, scrub: t => t }).done;
    expect(shaped).toEqual({ json: { got: "a b" } });
  });
});

describe("a box thread's slate", () => {
  it("runs a command on the thread's own machine, in its folder there with SLATE_DIR there, and one on the host here", async () => {
    const machine = bashMachine();
    const scripts: string[] = [];
    const run = machine.run.bind(machine);
    machine.run = (script, opts) => (scripts.push(script), run(script, opts));
    const boxFolder = temp();
    const hereFolder = temp();
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const slates = createSlates({
      store: memoryStore(),
      now: () => Date.now(),
      record: () => {},
      emit: () => {},
      thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder: boxFolder, hostFolder: hereFolder, computer: "spoo" }),
      machineOf: () => machine,
      under: lead => [lead],
      threadOfToken: () => thread,
      sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
      deliver: async () => ({ outcome: "started" }),
      runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
    });
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    await slates.write({ text: `<slate title="Where"><value name="go" start={0} />
  <run name="box" cmd='pwd; printf %s "$SLATE_DIR"; cat "$SLATE_DIR/note.txt"' timeout={20} />
  <run name="here" cmd="pwd" on="host" timeout={20} />
  <when change={$go} do={[start($box), start($here)]} />
  <column><output run={$box} /><output run={$here} /></column>
  <file name="note.txt">{\`kept\`}</file>
</slate>` }, asThread);
    await slates.state({ threadId: thread, values: { $go: 1 } });
    for (const ask of (await slates.get(thread))!.asks) await slates.approve({ threadId: thread, key: ask.key, scope: "thread" });
    const value = async (run: string) => (await slates.get(thread))!.values[run] as { state: string; out?: string };
    for (let i = 0; i < 100 && ((await value("box")).state !== "done" || (await value("here")).state !== "done"); i++) await new Promise(r => setTimeout(r, 100));
    expect(await value("box")).toMatchObject({ state: "done", out: `${boxFolder}\n${boxSlateDir(thread)}kept` });
    expect(await value("here")).toMatchObject({ state: "done", out: `${hereFolder}\n` });
    // One launcher reached the machine, the box run's; the host run never went there.
    expect(scripts).toHaveLength(1);
    expect(scripts[0]).toContain(`exec bash -c 'pwd; printf %s "$SLATE_DIR"; cat "$SLATE_DIR/note.txt"'`);
    slates.close();
  }, 30_000);

  it("a press on a box whose turns and runs the host is still settling reaches the machine only once that is done", async () => {
    const machine = bashMachine();
    let open!: () => void;
    const settled = { at: false };
    const booted = new Promise<void>(resolve => (open = resolve)).then(() => void (settled.at = true));
    const early: string[] = [];
    const run = machine.run.bind(machine);
    machine.run = (script, opts) => (settled.at ? undefined : early.push(script), run(script, opts));
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const slates = createSlates({
      store: memoryStore(),
      now: () => Date.now(),
      record: () => {},
      emit: () => {},
      thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder: temp(), hostFolder: temp(), computer: "spoo" }),
      machineOf: () => machine,
      wake: () => booted,
      under: lead => [lead],
      threadOfToken: () => thread,
      sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
      deliver: async () => ({ outcome: "started" }),
      runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
    });
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    await slates.write({ text: `<slate title="Held"><value name="go" start={0} />
  <run name="box" cmd="echo ran" timeout={20} />
  <when change={$go} do={start($box)} />
  <column><output run={$box} /></column>
</slate>` }, asThread);
    await slates.state({ threadId: thread, values: { $go: 1 } });
    for (const ask of (await slates.get(thread))!.asks) await slates.approve({ threadId: thread, key: ask.key, scope: "thread" });
    const value = async () => (await slates.get(thread))!.values["box"] as { state: string; out?: string };
    await new Promise(r => setTimeout(r, 500));
    open();
    for (let i = 0; i < 100 && (await value()).state !== "done"; i++) await new Promise(r => setTimeout(r, 100));
    expect(await value()).toMatchObject({ state: "done", out: "ran\n" });
    expect(early).toEqual([]);
    slates.close();
  }, 30_000);

  it("a then on the box gets the raw result and SLATE_DIR, never the env its command was given", async () => {
    const machine = bashMachine();
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const slates = createSlates({
      store: memoryStore(),
      now: () => Date.now(),
      record: () => {},
      emit: () => {},
      thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder: temp(), hostFolder: temp(), computer: "spoo" }),
      machineOf: () => machine,
      under: lead => [lead],
      threadOfToken: () => thread,
      sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
      deliver: async () => ({ outcome: "started" }),
      runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
    });
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    await slates.write({ text: `<slate title="Then"><value name="go" start={0} /><value name="token" start="s3cret-token" />
  <run name="fetch" cmd='printf "%s" "$TOKEN" | wc -c' env={{ TOKEN: $token }} then='printf "{\\"raw\\":\\"%s\\",\\"token\\":\\"%s\\",\\"dir\\":\\"%s\\"}" "$(cat | tr -d " \\n")" "$TOKEN" "$SLATE_DIR"' timeout={20} />
  <when change={$go} do={start($fetch)} />
  <column><output run={$fetch} /></column>
</slate>` }, asThread);
    await slates.state({ threadId: thread, values: { $go: 1 } });
    for (const ask of (await slates.get(thread))!.asks) await slates.approve({ threadId: thread, key: ask.key, scope: "thread" });
    const value = async () => (await slates.get(thread))!.values["fetch"] as { state: string; json?: unknown; why?: string };
    for (let i = 0; i < 100 && !["done", "failed"].includes((await value()).state); i++) await new Promise(r => setTimeout(r, 100));
    expect(await value()).toMatchObject({ state: "done", json: { raw: "12", token: "", dir: boxSlateDir(thread) } });
    slates.close();
  }, 30_000);

  it("a tick on a napping box wakes nothing and runs nothing, saying so, and a press wakes it and runs there", async () => {
    const machine = bashMachine();
    const scripts: string[] = [];
    const run = machine.run.bind(machine);
    machine.run = (script, opts) => (scripts.push(script), run(script, opts));
    let asleep = true;
    let woke = 0;
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const folder = temp();
    const slates = createSlates({
      store: memoryStore(),
      now: () => Date.now(),
      record: () => {},
      emit: () => {},
      thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder, hostFolder: folder, computer: "spoo" }),
      machineOf: () => machine,
      asleep: () => asleep,
      wake: async () => {
        woke += 1;
        asleep = false;
      },
      under: lead => [lead],
      threadOfToken: () => thread,
      sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
      deliver: async () => ({ outcome: "started" }),
      runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
    });
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    const wrote = await slates.write({ text: `<slate title="Feed"><run name="feed" cmd="echo fed" every={60} timeout={20} /><column><output run={$feed} /><button id="go" label="Now" onPress={start($feed)} /></column></slate>` }, asThread);
    // Shown: the timer ticks at once, onto a napping box.
    slates.subscribe({ threadId: thread, sources: [] });
    const feed = async () => (await slates.get(thread))!.values["feed"] as { state: string; why?: string; out?: string };
    for (let i = 0; i < 50 && (await feed()).state !== "held"; i++) await new Promise(r => setTimeout(r, 50));
    expect(await feed()).toMatchObject({ state: "held", why: "the box was asleep, so this tick did not wake it; press to run it now" });
    expect((await slates.get(thread))!.asks).toEqual([]);
    expect({ woke, scripts: scripts.length }).toEqual({ woke: 0, scripts: 0 });
    // The person presses: approved, the box wakes once, and the command runs there.
    const pressed = await slates.event({ threadId: thread, version: wrote.version, piece: "go", event: "press", requestId: "p1" });
    await slates.approve({ threadId: thread, key: pressed.ask!.key, scope: "thread" });
    for (let i = 0; i < 100 && (await feed()).state !== "done"; i++) await new Promise(r => setTimeout(r, 50));
    expect(await feed()).toMatchObject({ state: "done", out: "fed\n" });
    expect({ woke, scripts: scripts.length }).toEqual({ woke: 1, scripts: 1 });
    slates.close();
  }, 30_000);
});

describe("a box thread's tool run", () => {
  const TOOLS = `<slate title="Zones"><run name="zones" tool="cloudflare.zones_list" every={60} /><run name="repos" resource="github:gh://repos" />
  <column><output run={$zones} /></column></slate>`;
  const AWAY = { computer: "hetzner", host: "studio" };
  const CLAUDE = { id: "claude", name: "Claude Code" };
  /** Slates for a thread on hetzner, where the agent's config there names github alone, or on this computer. */
  const slatesOn = (where: typeof AWAY | undefined | (() => typeof AWAY | undefined), open?: McpPipeOpen, machine?: Machine) => {
    const awayOf = typeof where === "function" ? where : () => where;
    const away = awayOf();
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const asked: string[][] = [];
    const slates = createSlates({
      store: memoryStore(),
      now: () => Date.now(),
      record: () => {},
      emit: () => {},
      thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder: temp(), hostFolder: temp(), computer: away?.computer ?? "studio" }),
      ...(typeof where === "function" || away !== undefined ? { machineOf: () => machine ?? bashMachine(), away: awayOf, missingOn: async (_t: string, servers: string[]) => (asked.push(servers), { agent: CLAUDE, thread, missing: servers.filter(s => s !== "github") }) } : {}),
      under: lead => [lead],
      threadOfToken: () => thread,
      sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
      deliver: async () => ({ outcome: "started" }),
      runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
      mcpServer: async () => ({ transport: { kind: "stdio", command: "stub", args: [], env: {} }, cwd: "/", env: {}, secrets: [], ...(open !== undefined ? { open } : {}) }),
    });
    const asThread: Caller = { origin: "relayed", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    return { slates, thread, asThread, asked };
  };
  const notSetUp = (thread: string): string => `slate refused: 1 error: K710 server-not-set-up ${slateServerNotSetUp("cloudflare", "hetzner", CLAUDE, thread)}`;

  it("is refused as the slate is written, checked or patched in where that computer's config names no such server, and a write keeping its servers asks nothing", async () => {
    const { slates, thread, asThread, asked } = slatesOn(AWAY);
    expect(slateServerNotSetUp("cloudflare", "hetzner", CLAUDE, thread)).toContain(`wsp servers add cloudflare ${thread} --agent claude`);
    await expect(slates.write({ text: TOOLS, check: true }, asThread)).rejects.toMatchObject({ kind: "invalid", message: notSetUp(thread) });
    await expect(slates.write({ text: TOOLS }, asThread)).rejects.toMatchObject({ kind: "invalid", message: notSetUp(thread) });
    expect(await slates.get(thread)).toBeNull();
    // A server it has writes; a patch naming one it lacks is refused the same way; one keeping its servers asks nothing.
    await slates.write({ text: `<slate title="Zones"><run name="repos" resource="github:gh://repos" /><column><text id="t">none yet</text></column></slate>` }, asThread);
    await expect(slates.write({ text: `<run name="zones" tool="cloudflare.zones_list" />` }, asThread)).rejects.toMatchObject({ kind: "invalid", message: notSetUp(thread) });
    const before = asked.length;
    await slates.write({ text: `<text id="t">two</text>` }, asThread);
    expect(asked.length).toBe(before);
    expect(asked).toEqual([["cloudflare", "github"], ["cloudflare", "github"], ["github"], ["cloudflare"]]);
    slates.close();
  });

  it("runs a command on its own computer, and one on=\"host\" is refused naming both computers, as it never is from a thread here", async () => {
    const onHost = `<slate title="Me"><run name="me" cmd="whoami" on="host" /><column><output run={$me} /></column></slate>`;
    const box = slatesOn(AWAY);
    await expect(box.slates.write({ text: onHost }, box.asThread)).rejects.toMatchObject({ kind: "invalid", message: `slate refused: 1 error: K711 host-away ${slateHostAway("$me", "hetzner", "studio")}` });
    await box.slates.write({ text: onHost.replace(' on="host"', "") }, box.asThread);
    const here = slatesOn(undefined);
    await here.slates.write({ text: onHost }, here.asThread);
    await here.slates.write({ text: TOOLS }, here.asThread);
    expect(Object.keys(((await here.slates.get(here.thread))!.document as { runs: object }).runs)).toEqual(["zones", "repos"]);
    box.slates.close();
    here.slates.close();
  });

  it("refuses a stored on=\"host\" run when it starts from a thread now off this computer, before anything asks the person", async () => {
    let now: typeof AWAY | undefined;
    const { slates, thread, asThread } = slatesOn(() => now);
    await slates.write({ text: `<slate title="Me"><run name="me" cmd="whoami" on="host" /><column><button id="go" label="Go" onPress={start($me)} /></column></slate>` }, asThread);
    now = AWAY;
    const pressed = await slates.event({ threadId: thread, version: (await slates.get(thread))!.version, piece: "go", event: "press", requestId: "go-0" });
    expect(pressed.ask).toBeUndefined();
    await vi.waitFor(async () => expect((await slates.get(thread))!.values["me"]).toMatchObject({ state: "failed", why: slateHostAway("$me", "hetzner", "studio") }), { timeout: 5_000 });
    expect((await slates.get(thread))!.asks).toEqual([]);
    slates.close();
  });

  it("keeps one server per thread and server running between calls, and closes it when the machine naps or the thread goes", async () => {
    const pipes: { killed: boolean }[] = [];
    // A server in the test's own hands: it answers initialize, tools/list and every call, as one on the box would.
    const open: McpPipeOpen = (_t, _spec, on) => {
      const pipe = { killed: false };
      pipes.push(pipe);
      let calls = 0;
      return {
        write(text) {
          for (const line of text.split("\n").filter(Boolean)) {
            const msg = JSON.parse(line) as { id?: number; method: string };
            if (msg.id === undefined) continue;
            const result = msg.method === "initialize" ? { protocolVersion: "2025-06-18", capabilities: { tools: {} } } : msg.method === "tools/list" ? { tools: [{ name: "list", annotations: { readOnlyHint: true } }] } : { content: [{ type: "text", text: JSON.stringify({ call: ++calls }) }] };
            setImmediate(() => on.out(Buffer.from(`${JSON.stringify({ jsonrpc: "2.0", id: msg.id, result })}\n`)));
          }
        },
        kill() {
          pipe.killed = true;
          setImmediate(() => on.exit("the server was stopped"));
        },
      };
    };
    const { slates, thread, asThread } = slatesOn({ ...AWAY }, open);
    await slates.write({ text: `<slate title="Repos"><run name="repos" tool="github.list" /><column><button id="go" label="Go" onPress={start($repos)} /></column></slate>` }, asThread);
    const view = async () => (await slates.get(thread))!;
    const press = async (n: number) => slates.event({ threadId: thread, version: (await view()).version, piece: "go", event: "press", requestId: `go-${n}` });
    await slates.approve({ threadId: thread, key: (await press(0)).ask!.key, scope: "thread" });
    await vi.waitFor(async () => expect((await view()).values["repos"]).toMatchObject({ state: "done", runs: 1, json: { call: 1 } }), { timeout: 5_000 });
    await press(1);
    await vi.waitFor(async () => expect((await view()).values["repos"]).toMatchObject({ state: "done", runs: 2, json: { call: 2 } }), { timeout: 5_000 });
    expect(pipes).toEqual([{ killed: false }]);
    // The machine naps: the server there goes, and the next call starts a new one.
    slates.napped("w1");
    await vi.waitFor(() => expect(pipes).toEqual([{ killed: true }]));
    await press(2);
    await vi.waitFor(async () => expect((await view()).values["repos"]).toMatchObject({ state: "done", runs: 3, json: { call: 1 } }), { timeout: 5_000 });
    expect(pipes).toEqual([{ killed: true }, { killed: false }]);
    // Another workspace's nap leaves it be; the thread's going takes it.
    slates.napped("w2");
    expect(pipes[1]!.killed).toBe(false);
    await slates.forget(thread);
    await vi.waitFor(() => expect(pipes[1]!.killed).toBe(true));
    slates.close();
  });

  it("a forget that keeps, as a delete with keep asks, leaves the slate's running run and its kept server running", async () => {
    const pipes: { killed: boolean }[] = [];
    const open: McpPipeOpen = (_t, _spec, on) => {
      const pipe = { killed: false };
      pipes.push(pipe);
      return {
        write(text) {
          for (const line of text.split("\n").filter(l => l.startsWith("{"))) {
            const msg = JSON.parse(line) as { id?: number; method: string };
            if (msg.id === undefined) continue;
            const result = msg.method === "initialize" ? { protocolVersion: "2025-06-18", capabilities: { tools: {} } } : msg.method === "tools/list" ? { tools: [{ name: "list", annotations: { readOnlyHint: true } }] } : { content: [{ type: "text", text: "{}" }] };
            setImmediate(() => on.out(Buffer.from(`${JSON.stringify({ jsonrpc: "2.0", id: msg.id, result })}\n`)));
          }
        },
        kill() {
          pipe.killed = true;
          setImmediate(() => on.exit("the server was stopped"));
        },
      };
    };
    const machine = bashMachine();
    const { slates, thread, asThread } = slatesOn({ ...AWAY }, open, machine);
    await slates.write({ text: `<slate title="Repos"><run name="repos" tool="github.list" /><run name="nap" cmd="sleep 2" /><column><button id="go" label="Go" onPress={start($repos)} /><button id="nap" label="Nap" onPress={start($nap)} /></column></slate>` }, asThread);
    const view = async () => (await slates.get(thread))!;
    const press = async (piece: string, n: number) => slates.event({ threadId: thread, version: (await view()).version, piece, event: "press", requestId: `${piece}-${n}` });
    for (const piece of ["go", "nap"]) {
      const said = await press(piece, 0);
      if (said.ask !== undefined) await slates.approve({ threadId: thread, key: said.ask.key, scope: "thread" });
    }
    await vi.waitFor(async () => expect((await view()).values["repos"]).toMatchObject({ state: "done" }), { timeout: 5_000 });
    await vi.waitFor(async () => expect((await view()).values["nap"]).toMatchObject({ state: "running" }), { timeout: 5_000 });
    await slates.forget(thread, { keep: true });
    await new Promise(resolve => setTimeout(resolve, 500));
    expect(await slates.get(thread)).toBeNull();
    expect([pipes.map(p => p.killed), machine.killed()]).toEqual([[false], false]);
    slates.close();
  });

  it("closes the road it opened when that computer's daemon refuses the start, and says the refusal, not the close", async () => {
    let closes = 0;
    let listening = 0;
    let gone!: () => void;
    const road: StdioRoad = {
      send: async frame => {
        if (frame["op"] === "stdio.open") throw new Error("unknown op: stdio.open");
        return {};
      },
      listen: () => (listening++, () => void listening--),
      closed: new Promise<void>(r => (gone = r)),
      close: () => (closes++, gone()),
    };
    const stub = { kind: "stdio" as const, command: "stub", args: [], env: {} };
    const exits: string[] = [];
    const pipe = pipeOn("hetzner", { platform: "linux", home: "/root", user: "root" }, async () => road)(stub, { transport: stub, cwd: "/", env: {}, secrets: [] }, {
      out: () => {},
      err: () => {},
      exit: why => (exits.push(why), pipe.kill()),
    });
    await vi.waitFor(() => expect(closes).toBe(1));
    expect({ first: exits[0], listening }).toEqual({ first: "it could not be started on hetzner: unknown op: stdio.open", listening: 0 });
  });
});

describe("a box thread's image", () => {
  it("is read through that computer's daemon by its whole path there, a relative one under the thread's folder there, never off this computer's disk", async () => {
    const png = Buffer.from("89504e470d0a1a0a0000000d494844520000000100000001", "hex");
    const here = temp();
    writeFileSync(join(here, "home.png"), png);
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    const asked: string[] = [];
    const slates = createSlates({
      store: memoryStore(),
      now: () => Date.now(),
      record: () => {},
      emit: () => {},
      thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder: "/root/acme", hostFolder: here, computer: "spoo" }),
      machineOf: () => bashMachine(),
      imageOn: () => ({
        name: "spoo",
        read: async path => {
          asked.push(path);
          if (path === "/root/acme/shots/home.png") return { size: png.length, mediaType: "image/png", content: png.toString("base64") };
          throw Object.assign(new Error(`${path} does not exist`), { code: "not-found" });
        },
      }),
      under: lead => [lead],
      threadOfToken: () => thread,
      sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
      deliver: async () => ({ outcome: "started" }),
      runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
    });
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    await slates.write({ text: `<slate><column><image src="shots/home.png" /></column></slate>` }, asThread);
    expect(await slates.image({ threadId: thread, src: "shots/home.png" })).toMatchObject({ mediaType: "image/png", bytes: png.toString("base64") });
    const gone = await slates.image({ threadId: thread, src: join(here, "home.png") });
    expect("problem" in gone && `${gone.problem.code} ${gone.problem.message}`).toBe(`R900 no file at ${join(here, "home.png")}`);
    expect(asked).toEqual(["/root/acme/shots/home.png", join(here, "home.png")]);
    slates.close();
  });
});

describe("an Always for a box thread's command", () => {
  it("covers the script an on=host command names in this computer's folder, and is not offered for one that names a file on the box", async () => {
    const machine = bashMachine();
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const hereFolder = temp();
    const boxFolder = temp();
    const script = join(boxFolder, ".wsp-system-resources.py");
    writeFileSync(join(hereFolder, "deploy.sh"), "echo one");
    writeFileSync(script, "print('one')");
    const slates = createSlates({
      store: memoryStore(),
      now: () => Date.now(),
      record: () => {},
      emit: () => {},
      thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder: boxFolder, hostFolder: hereFolder, computer: "spoo" }),
      machineOf: () => machine,
      under: lead => [lead],
      threadOfToken: () => thread,
      sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
      deliver: async () => ({ outcome: "started" }),
      runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
    });
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    await slates.write({ text: `<slate title="Deploy">
  <run name="here" cmd="bash deploy.sh" on="host" timeout={20} />
  <run name="there" cmd="python3 ${script}" timeout={20} />
  <column><button id="h" label="Here" onPress={start($here)} /><button id="t" label="There" onPress={start($there)} /></column>
</slate>` }, asThread);
    const press = async (piece: string) => slates.event({ threadId: thread, version: (await slates.get(thread))!.version, piece, event: "press", requestId: `${piece}-${Math.random()}` });
    const done = async (run: string, out: string) => vi.waitFor(async () => expect((await slates.get(thread))!.values[run]).toMatchObject({ state: "done", out }), { timeout: 10_000 });

    const here = await press("h");
    expect(here.ask).not.toHaveProperty("noAlways");
    await slates.approve({ threadId: thread, key: here.ask!.key, scope: "thread" });
    await done("here", "one\n");
    writeFileSync(join(hereFolder, "deploy.sh"), "echo two");
    expect((await press("h")).ask).toBeDefined();

    // The sheet offers Run once and Don't alone, so no choice it offers is refused; Run once runs it on the box.
    const there = await press("t");
    expect(there.ask).toMatchObject({ run: "there", noAlways: true });
    expect((await slates.get(thread))!.asks.find(a => a.run === "there")).toMatchObject({ noAlways: true });
    await slates.approve({ threadId: thread, key: there.ask!.key, scope: "once" });
    await done("there", "one\n");
    // No command line verb or tool approves a slate; another caller of slates.approve that asks for an Always is told why.
    const again = await press("t");
    await expect(slates.approve({ threadId: thread, key: again.ask!.key, scope: "thread" })).rejects.toThrow(/\.wsp-system-resources\.py there, which its computer could not hash/);
    slates.close();
  }, 30_000);
});

/** A box daemon's fs.hash over this computer's disk: each path under the root where it lands, a regular file, and
 * the paths as handed that land on anything but a folder outside it, on a file inside past the 32 files or over
 * 4 MiB, or on something inside that is not a regular file. */
const hashHere: HashOn = async (root, paths) => {
  const top = realpathSync(root);
  const read = { files: {} as Record<string, string>, outside: [] as string[], past: [] as string[], special: [] as string[] };
  for (const path of paths) {
    try {
      const at = realpathSync(isAbsolute(path) ? path : join(root, path));
      const rel = relative(top, at);
      const st = statSync(at);
      if (rel === "" || st.isDirectory() || read.files[rel] !== undefined) continue;
      if (rel.startsWith("..")) read.outside.push(path);
      else if (!st.isFile()) read.special.push(path);
      else if (st.size > FS_HASH_CAP_BYTES || Object.keys(read.files).length >= FS_HASH_FILES_MAX) read.past.push(path);
      else read.files[rel] = createHash("sha256").update(readFileSync(at)).digest("hex");
    } catch {
      // not there
    }
  }
  return read;
};

describe("an Always for a command on the box, pinned by its daemon's hash", () => {
  const boxThread = (hashOn: HashOn | undefined, o: { asleep?: () => boolean; wake?: () => Promise<void>; here?: true } = {}) => {
    const machine = bashMachine();
    const thread = `t-${Date.now()}-${Math.random().toString(36).slice(2)}`;
    made.push(boxSlateDir(thread));
    const folder = temp();
    const store = memoryStore();
    const open = () =>
      createSlates({
        store,
        now: () => Date.now(),
        record: () => {},
        emit: () => {},
        thread: () => ({ workspaceId: "w1", rootThreadId: thread, sessionId: "s1", folder, hostFolder: temp(), computer: "acme-box" }),
        ...(o.here === true ? {} : { machineOf: () => machine }),
        ...(hashOn !== undefined ? { hashOn: () => hashOn } : {}),
        ...(o.asleep !== undefined ? { asleep: o.asleep } : {}),
        ...(o.wake !== undefined ? { wake: o.wake } : {}),
        under: lead => [lead],
        threadOfToken: () => thread,
        sources: (threadId, workspaceId) => ({ threadId, workspaceId, now: Date.now(), rows: () => [] }) as never,
        deliver: async () => ({ outcome: "started" }),
        runEnv: () => ({ PATH: process.env["PATH"] ?? "/usr/bin:/bin" }),
      });
    const asThread: Caller = { origin: "here", by: { kind: "thread", threadId: thread, workspaceId: "w1", rootThreadId: thread } };
    const on = (slates: ReturnType<typeof open>) => ({
      press: async (piece: string) => slates.event({ threadId: thread, version: (await slates.get(thread))!.version, piece, event: "press", requestId: `${piece}-${Math.random()}` }),
      done: async (run: string, runs: number) => vi.waitFor(async () => expect((await slates.get(thread))!.values[run]).toMatchObject({ state: "done", runs }), { timeout: 10_000 }),
      askOf: async (run: string) => (await slates.get(thread))!.asks.find(a => a.run === run),
    });
    return { thread, folder, store, open, asThread, on };
  };

  it("holds across a reopen with no second ask, and asks again once the script on the box changes", async () => {
    const asked: string[][] = [];
    const box = boxThread(async (root, paths) => {
      asked.push(paths);
      return hashHere(root, paths);
    });
    writeFileSync(join(box.folder, "stats.sh"), "echo one");
    let slates = box.open();
    await slates.write({ text: `<slate><run name="stats" cmd="bash stats.sh" timeout={20} /><column><button id="s" label="Stats" onPress={start($stats)} /></column></slate>` }, box.asThread);
    let s = box.on(slates);

    const first = await s.press("s");
    expect(first.ask).toMatchObject({ run: "stats" });
    expect(first.ask).not.toHaveProperty("noAlways");
    expect(await s.askOf("stats")).not.toHaveProperty("noAlways");
    await slates.approve({ threadId: box.thread, key: first.ask!.key, scope: "thread" });
    await s.done("stats", 1);
    expect((await s.press("s")).ask).toBeUndefined();
    await s.done("stats", 2);
    slates.close();

    // A host that opens the same record reads the box again and finds the script the person allowed.
    slates = box.open();
    s = box.on(slates);
    expect((await s.press("s")).ask).toBeUndefined();
    await s.done("stats", 3);
    expect(asked.length).toBeGreaterThan(0);
    expect(asked.every(paths => paths.join() === `${box.folder}/stats.sh`)).toBe(true);

    writeFileSync(join(box.folder, "stats.sh"), "echo two");
    const changed = await s.press("s");
    expect(changed.ask).toMatchObject({ run: "stats" });
    expect(await s.askOf("stats")).toMatchObject({ why: "stats.sh changed since you allowed it, so it asks again" });
    expect(await s.askOf("stats")).not.toHaveProperty("noAlways");
    slates.close();
  }, 30_000);

  it("takes an Always for df -h / and sleep 0.5, which name no file inside the thread's folder", async () => {
    let fails = false;
    const box = boxThread(async (root, paths) => {
      if (fails) throw new Error("the box did not answer");
      return hashHere(root, paths);
    });
    const slates = box.open();
    await slates.write({ text: `<slate><run name="disk" cmd="df -h /" timeout={20} /><run name="nap" cmd="sleep 0.5" timeout={20} /><column><button id="d" label="Disk" onPress={start($disk)} /><button id="n" label="Nap" onPress={start($nap)} /></column></slate>` }, box.asThread);
    const s = box.on(slates);
    for (const [piece, run] of [["d", "disk"], ["n", "nap"]] as const) {
      const held = await s.press(piece);
      expect(held.ask).toMatchObject({ run });
      expect(await s.askOf(run)).not.toHaveProperty("noAlways");
      await slates.approve({ threadId: box.thread, key: held.ask!.key, scope: "thread" });
      await s.done(run, 1);
      expect((await s.press(piece)).ask).toBeUndefined();
      await s.done(run, 2);
    }
    fails = true;
    expect((await s.press("d")).ask).toBeDefined();
    expect(await s.askOf("disk")).toMatchObject({ noAlways: true, why: "/ could not be read on acme-box, so it asks again" });
    slates.close();
  }, 30_000);

  it("offers no Always for a command naming a script where the daemon cannot hash, and refuses one asked for", async () => {
    // What a daemon from before fs.hash answers, measured against one built from the base.
    const box = boxThread(async () => {
      throw new Error("unknown op: fs.hash");
    });
    writeFileSync(join(box.folder, "stats.sh"), "echo one");
    const slates = box.open();
    await slates.write({ text: `<slate><run name="stats" cmd="bash stats.sh" timeout={20} /><column><button id="s" label="Stats" onPress={start($stats)} /></column></slate>` }, box.asThread);
    const s = box.on(slates);
    const held = await s.press("s");
    expect(held.ask).toMatchObject({ run: "stats", noAlways: true });
    expect(await s.askOf("stats")).toMatchObject({ noAlways: true });
    await expect(slates.approve({ threadId: box.thread, key: held.ask!.key, scope: "thread" })).rejects.toThrow(/names stats\.sh there, which its computer could not hash/);
    await slates.approve({ threadId: box.thread, key: held.ask!.key, scope: "once" });
    await s.done("stats", 1);
    expect((await s.press("s")).ask).toMatchObject({ noAlways: true });
    slates.close();
  }, 30_000);

  const STATS = `<slate><run name="stats" cmd="bash stats.sh" timeout={20} /><column><button id="s" label="Stats" onPress={start($stats)} /></column></slate>`;

  it("asks when the read fails, even under an Always given before the script was there", async () => {
    let fails = false;
    const box = boxThread(async (root, paths) => {
      if (fails) throw new Error("unknown op: fs.hash");
      return hashHere(root, paths);
    });
    const slates = box.open();
    await slates.write({ text: STATS }, box.asThread);
    const s = box.on(slates);
    const first = await s.press("s");
    await slates.approve({ threadId: box.thread, key: first.ask!.key, scope: "thread" });
    // No stats.sh yet: bash fails, and the Always holds a key with no script in it.
    await vi.waitFor(async () => expect((await slates.get(box.thread))!.values["stats"]).toMatchObject({ state: "failed", runs: 1 }), { timeout: 10_000 });
    writeFileSync(join(box.folder, "stats.sh"), "echo evil");
    fails = true;
    expect((await s.press("s")).ask).toMatchObject({ run: "stats" });
    expect(await s.askOf("stats")).toMatchObject({ noAlways: true });
    expect((await slates.get(box.thread))!.values["stats"]).toMatchObject({ state: "held" });
    slates.close();
  }, 30_000);

  it("says the script could not be read there, not that it changed, when a read fails under an Always", async () => {
    let fails = false;
    const box = boxThread(async (root, paths) => {
      if (fails) throw new Error("the box did not answer");
      return hashHere(root, paths);
    });
    writeFileSync(join(box.folder, "stats.sh"), "echo one");
    const slates = box.open();
    await slates.write({ text: STATS }, box.asThread);
    const s = box.on(slates);
    const first = await s.press("s");
    await slates.approve({ threadId: box.thread, key: first.ask!.key, scope: "thread" });
    await s.done("stats", 1);
    fails = true;
    expect((await s.press("s")).ask).toBeDefined();
    expect(await s.askOf("stats")).toMatchObject({ noAlways: true, why: "stats.sh could not be read on acme-box, so it asks again" });
    slates.close();
  }, 30_000);

  for (const where of ["box", "here"] as const) {
    it(`reads the scripts of a run with cwd in the folder it runs in, so an edit there asks again (${where})`, async () => {
      const box = boxThread(hashHere, where === "here" ? { here: true } : {});
      mkdirSync(join(box.folder, "scripts"));
      writeFileSync(join(box.folder, "scripts", "stats.sh"), "echo one");
      const slates = box.open();
      await slates.write({ text: `<slate><run name="stats" cmd="bash stats.sh" cwd="scripts" timeout={20} /><column><button id="s" label="Stats" onPress={start($stats)} /></column></slate>` }, box.asThread);
      const s = box.on(slates);
      const first = await s.press("s");
      expect(await s.askOf("stats")).not.toHaveProperty("noAlways");
      await slates.approve({ threadId: box.thread, key: first.ask!.key, scope: "thread" });
      await s.done("stats", 1);
      expect((await s.press("s")).ask).toBeUndefined();
      await s.done("stats", 2);
      writeFileSync(join(box.folder, "scripts", "stats.sh"), "echo evil");
      expect((await s.press("s")).ask).toMatchObject({ run: "stats" });
      expect(await s.askOf("stats")).toMatchObject({ why: "scripts/stats.sh changed since you allowed it, so it asks again" });
      slates.close();
    }, 30_000);
  }

  it("wakes a napping machine for a press and holds to a standing Always there", async () => {
    let asleep = false;
    const seen: string[] = [];
    const box = boxThread(
      async (root, paths) => {
        seen.push(asleep ? "hash asleep" : "hash");
        return hashHere(root, paths);
      },
      {
        asleep: () => asleep,
        wake: async () => {
          seen.push("wake");
          asleep = false;
        },
      },
    );
    writeFileSync(join(box.folder, "stats.sh"), "echo one");
    const slates = box.open();
    await slates.write({ text: STATS }, box.asThread);
    const s = box.on(slates);
    const first = await s.press("s");
    await slates.approve({ threadId: box.thread, key: first.ask!.key, scope: "thread" });
    await s.done("stats", 1);
    asleep = true;
    seen.length = 0;
    expect((await s.press("s")).ask).toBeUndefined();
    await s.done("stats", 2);
    // Woken before the box is read; the road's own wake before the run follows.
    expect(seen.slice(0, 2)).toEqual(["wake", "hash"]);
    expect(seen).not.toContain("hash asleep");
    slates.close();
  }, 30_000);

  it("a held ask keeps its Always across a reopen", async () => {
    const box = boxThread(hashHere);
    writeFileSync(join(box.folder, "stats.sh"), "echo one");
    let slates = box.open();
    await slates.write({ text: STATS }, box.asThread);
    await box.on(slates).press("s");
    expect(await box.on(slates).askOf("stats")).not.toHaveProperty("noAlways");
    slates.close();
    slates = box.open();
    const s = box.on(slates);
    await vi.waitFor(async () => {
      const ask = await s.askOf("stats");
      expect(ask).toBeDefined();
      expect(ask).not.toHaveProperty("noAlways");
    });
    await slates.approve({ threadId: box.thread, key: (await s.askOf("stats"))!.key, scope: "thread" });
    await s.done("stats", 1);
    slates.close();
  }, 30_000);

  /** A slate with one button pressing $run, whose command is cmd. */
  const oneRun = (cmd: string, file = "") =>
    `<slate><run name="run" cmd=${cmd.includes('"') ? `'${cmd}'` : `"${cmd}"`} timeout={20} /><column><button id="r" label="Run" onPress={start($run)} /></column>${file}</slate>`;
  /** A unix socket left at a path, which bash cannot run and fails on at once. */
  const socketAt = (path: string): void => void execFileSync("python3", ["-c", "import socket, sys; socket.socket(socket.AF_UNIX).bind(sys.argv[1])", path]);
  /** Scripts s0.sh to s32.sh in the folder, one more than an Always pins, and a command that runs them all. */
  const pastTheCap = (folder: string, write: (n: number) => boolean = () => true): string => {
    const names = Array.from({ length: FS_HASH_FILES_MAX + 1 }, (_, n) => `s${n}.sh`);
    names.forEach((name, n) => write(n) && writeFileSync(join(folder, name), `echo ${n}`));
    return `cat ${names.join(" ")} | bash`;
  };
  const LAST = `s${FS_HASH_FILES_MAX}.sh`;
  const CAPS = "is past what one command can pin (32 files, each up to 4 MiB), so wsp cannot pin it and it asks every time";
  const SPECIAL = "is not a regular file, so wsp cannot pin it and it asks every time";
  const BUILT = "is a script wsp cannot read by name (a space, an escape or a $ in it), so wsp cannot pin it and it asks every time";
  /** A script one byte past the most one file can be and still be pinned, which bash still runs. */
  const big = (folder: string, name: string): void => writeFileSync(join(folder, name), `echo big\n#${"x".repeat(FS_HASH_CAP_BYTES)}\n`);
  /** A command naming 256 folders first, so the script after them is the 257th path it names. */
  const PAST_PATHS = `true ${Array.from({ length: FS_HASH_PATHS_MAX }, (_, n) => `d${n}/`).join(" ")}; bash ./evil.sh`;

  for (const where of ["box", "here"] as const) {
    const cases: { name: string; lay: (folder: string) => string; why: string; box?: true }[] = [
      {
        name: "a script through a link out of the folder",
        lay: folder => {
          writeFileSync(join(folder, "..", `away-${where}.sh`), "echo away");
          symlinkSync(join(folder, "..", `away-${where}.sh`), join(folder, "out.sh"));
          return "bash ./out.sh";
        },
        why: "./out.sh is a link out of the thread's folder, so wsp cannot pin it and it asks every time",
      },
      { name: "a script named outside the folder, named twice", lay: () => "bash ../away.sh; bash ../away.sh", why: "../away.sh is outside the thread's folder, so wsp cannot pin it and it asks every time" },
      { name: "an absolute path to a script outside the folder", lay: () => "python3 /opt/acme/stats.py", why: "/opt/acme/stats.py is outside the thread's folder, so wsp cannot pin it and it asks every time" },
      { name: "a script past the 32 files one command pins", lay: folder => pastTheCap(folder), why: `${LAST} ${CAPS}` },
      { name: "a script over 4 MiB", lay: folder => (big(folder, "big.sh"), "bash ./big.sh"), why: `./big.sh ${CAPS}` },
      { name: "a script past the 256 paths one ask names", lay: () => PAST_PATHS, why: "./evil.sh is past the 256 paths one command can name, so wsp cannot pin it and it asks every time", box: true },
      { name: "a script named from the home folder", lay: () => "bash ~/away.sh", why: "~/away.sh is outside the thread's folder, so wsp cannot pin it and it asks every time" },
      { name: "a script that is not a regular file", lay: folder => (socketAt(join(folder, "evil.sh")), "bash ./evil.sh"), why: `./evil.sh ${SPECIAL}` },
      { name: "a script whose name is quoted with a space", lay: () => "bash './my evil.sh'", why: `'./my evil.sh' ${BUILT}` },
      { name: "a script whose name is escaped", lay: () => "bash my\\ evil.sh", why: `my\\ evil.sh ${BUILT}` },
      { name: "a script whose name is built with $", lay: () => 'bash "$PWD/../away.sh"', why: `"$PWD/../away.sh" ${BUILT}` },
    ];
    for (const c of cases.filter(c => where === "box" || c.box === undefined)) {
      it(`offers Don't and Run once only for ${c.name}, saying why, and refuses an Always (${where})`, async () => {
        const box = boxThread(hashHere, where === "here" ? { here: true } : {});
        const slates = box.open();
        await slates.write({ text: oneRun(c.lay(box.folder)) }, box.asThread);
        const s = box.on(slates);
        const held = await s.press("r");
        expect(held.ask).toMatchObject({ run: "run", noAlways: true });
        expect(await s.askOf("run")).toMatchObject({ noAlways: true, why: c.why });
        await expect(slates.approve({ threadId: box.thread, key: held.ask!.key, scope: "thread" })).rejects.toThrow(/names a script wsp cannot pin/);
        await slates.approve({ threadId: box.thread, key: held.ask!.key, scope: "once" });
        await vi.waitFor(async () => expect((await slates.get(box.thread))!.values["run"]).toMatchObject({ runs: 1 }), { timeout: 10_000 });
        expect((await s.press("r")).ask).toMatchObject({ noAlways: true });
        slates.close();
      }, 30_000);
    }

    it(`an Always given while the script was absent stops holding once it is a link out or past the cap (${where})`, async () => {
      for (const [cmd, lay, why] of [
        ["bash ./out.sh", (folder: string) => {
          writeFileSync(join(folder, "..", `away-${where}.sh`), "echo evil");
          symlinkSync(join(folder, "..", `away-${where}.sh`), join(folder, "out.sh"));
        }, "./out.sh is a link out of the thread's folder, so wsp cannot pin it and it asks every time"],
        [undefined, (folder: string) => writeFileSync(join(folder, LAST), "echo evil"), `${LAST} ${CAPS}`],
        ["bash ./big.sh", (folder: string) => big(folder, "big.sh"), `./big.sh ${CAPS}`],
        ["bash ./evil.sh", (folder: string) => void execFileSync("mkfifo", [join(folder, "evil.sh")]), `./evil.sh ${SPECIAL}`],
      ] as const) {
        const box = boxThread(hashHere, where === "here" ? { here: true } : {});
        const slates = box.open();
        await slates.write({ text: oneRun(cmd ?? pastTheCap(box.folder, n => n < FS_HASH_FILES_MAX)) }, box.asThread);
        const s = box.on(slates);
        const first = await s.press("r");
        expect(first.ask).not.toHaveProperty("noAlways");
        await slates.approve({ threadId: box.thread, key: first.ask!.key, scope: "thread" });
        // A run counts at its start, so the wait is for its end: a press while it runs would read running, not held.
        await vi.waitFor(async () => expect((await slates.get(box.thread))!.values["run"]).toMatchObject({ runs: 1, state: expect.stringMatching(/^(done|failed)$/) }), { timeout: 10_000 });
        lay(box.folder);
        expect((await s.press("r")).ask).toMatchObject({ run: "run", noAlways: true });
        expect(await s.askOf("run")).toMatchObject({ why });
        expect((await slates.get(box.thread))!.values["run"]).toMatchObject({ state: "held", runs: 1 });
        slates.close();
      }
    }, 30_000);

    it(`an Always stored before this change for a script outside the folder asks again, naming it (${where})`, async () => {
      const box = boxThread(hashHere, where === "here" ? { here: true } : {});
      writeFileSync(join(box.folder, "..", "away.sh"), "echo evil");
      let slates = box.open();
      await slates.write({ text: oneRun("bash ../away.sh") }, box.asThread);
      slates.close();
      // The key every host before this change gave the command: nothing inside the folder, so no hashes in it.
      const before = createSlateRuns({ env: () => ({}), onRecord: () => {}, onLine: () => {}, onTimer: () => {} });
      const key = before.key({ kind: "cmd", cmd: "bash ../away.sh", timeout: 20 });
      before.close();
      const stored = (await box.store.get(SLATES, box.thread)) as { approvals: Record<string, unknown> };
      await box.store.put(SLATES, box.thread, { ...stored, approvals: { [key]: { state: "allowed", at: Date.now(), run: "run", cmd: "bash ../away.sh" } } });
      slates = box.open();
      const s = box.on(slates);
      expect((await s.press("r")).ask).toMatchObject({ run: "run", noAlways: true });
      expect(await s.askOf("run")).toMatchObject({ why: "../away.sh is outside the thread's folder, so wsp cannot pin it and it asks every time" });
      expect((await slates.get(box.thread))!.values["run"]).toMatchObject({ state: "held" });
      slates.close();
    }, 30_000);
  }

  for (const where of ["box", "here"] as const) {
    it(`keeps an Always for tools and interpreters outside the folder, a big file it only reads, a script inside spelled with .., inline code and the slate's own files (${where})`, async () => {
      const box = boxThread(hashHere, where === "here" ? { here: true } : {});
      mkdirSync(join(box.folder, ".venv", "bin"), { recursive: true });
      symlinkSync(execFileSync("bash", ["-c", "command -v python3"], { encoding: "utf8" }).trim(), join(box.folder, ".venv", "bin", "python3.12"));
      writeFileSync(join(box.folder, "stats.py"), "print('one')");
      mkdirSync(join(box.folder, "lib"));
      writeFileSync(join(box.folder, "ok.sh"), "true");
      big(box.folder, "big.log");
      const slates = box.open();
      const cmd = "cat /proc/loadavg; /usr/bin/env true; tail -n 1 ../notes.txt; tail -c 3 big.log; bash lib/../ok.sh; .venv/bin/python3.12 stats.py";
      const inline = "; bash -c 'echo two words'; python3 -c 'print(1)'; echo $HOME; cat ~/.profile; bash $SLATE_DIR/x.sh";
      await slates.write({ text: oneRun(cmd + inline, `<file name="x.sh">{\`true\`}</file>`) }, box.asThread);
      const s = box.on(slates);
      const held = await s.press("r");
      expect(held.ask).not.toHaveProperty("noAlways");
      await slates.approve({ threadId: box.thread, key: held.ask!.key, scope: "thread" });
      await vi.waitFor(async () => expect((await slates.get(box.thread))!.values["run"]).toMatchObject({ runs: 1 }), { timeout: 10_000 });
      expect((await s.press("r")).ask).toBeUndefined();
      slates.close();
    }, 30_000);
  }

  it("leaves a command unpinned where the daemon does not answer in time, and asks nothing of a napping computer", async () => {
    const decl = { kind: "cmd" as const, cmd: "bash stats.sh" };
    let calls = 0;
    const pins = boxPins(
      () => async () => {
        calls += 1;
        return new Promise<HashRead>(() => {});
      },
      20,
    );
    const unpinned = (cmd: typeof decl | { kind: "cmd"; cmd: string }) => pins.pin("t1", "/root/acme", cmd).unpinned.map(u => u.path);
    await pins.read("t1", [{ folder: "/root/acme", decl }], false);
    expect(unpinned(decl)).toEqual(["stats.sh"]);
    await pins.read("t1", [{ folder: "/root/acme", decl }], true);
    expect(calls).toBe(1);
    expect(unpinned({ kind: "cmd", cmd: "df -h" })).toEqual([]);
  });
});
