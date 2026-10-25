// SPDX-License-Identifier: AGPL-3.0-only
// The held ssh forward and the detached run's poll loop driven by scripted children and machines, scene by scene:
// what each wrote, in which pieces, and what the TS answered (tests/fixtures/scenes/roads.json, replayed by
// tests/ssh_dial_scenes.rs and tests/exec_run_scenes.rs). Random bytes are pinned to ab. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 roads_scenes.oracle.ts
import { EventEmitter } from "node:events";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { PassThrough } from "node:stream";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

vi.mock("node:crypto", async importOriginal => {
  const real = await importOriginal<typeof import("node:crypto")>();
  return { ...real, randomBytes: (n: number) => Buffer.alloc(n, 0xab) };
});

const E = "../../../../packages/engine/src";
const ssh = await import(`${E}/ssh-backend.ts`);
const ex = await import(`${E}/exec-detached.ts`);
const link = await import(`${E}/link-backend.ts`);

const out = fileURLToPath(new URL("../tests/fixtures/scenes/", import.meta.url));
const tick = (): Promise<void> => new Promise(r => setTimeout(r, 5));

type Step = { out: string } | { err: string } | { exit: { code: number | null; signal: string | null } } | { fail: string };

/** One held forward: the steps written to the child in order, each its own pipe write, then what up and ended said. */
async function forward(name: string, steps: Step[]): Promise<Record<string, unknown>> {
  const stdout = new PassThrough();
  const stderr = new PassThrough();
  const child = Object.assign(new EventEmitter(), { stdin: new PassThrough(), stdout, stderr, kill: () => true });
  const held = ssh.holdBackForward({ reach: { user: "root", host: "box", port: 22 }, options: [] }, 4640, 4640, () => child as never);
  let up: string | null = "pending";
  held.up.then(() => (up = "up"), (e: Error) => (up = `rejected: ${e.message}`));
  for (const step of steps) {
    if ("out" in step) stdout.write(step.out);
    else if ("err" in step) stderr.write(step.err);
    else if ("exit" in step) child.emit("exit", step.exit.code, step.exit.signal);
    else child.emit("error", new Error(step.fail));
    await tick();
  }
  const ended = await Promise.race([held.ended, tick().then(() => null)]);
  return { name, steps, up, ended };
}

/** One detached run on a machine whose exec answers the launch with the handshake and each poll with the next of
 * `polls` (the last repeated), and everything else with exit 0. */
async function run(name: string, polls: (string | { reject: string })[], opts: { deadlineMs?: number; pollMs?: number; launchDelayMs?: number } = {}): Promise<Record<string, unknown>> {
  const calls: string[] = [];
  const lines: string[] = [];
  let at = 0;
  const machine = {
    id: "m1",
    exec: async (cmd: string) => {
      calls.push(cmd);
      if (cmd.includes("echo WSP_LAUNCHED")) {
        if (opts.launchDelayMs !== undefined) await new Promise(r => setTimeout(r, opts.launchDelayMs));
        return { exitCode: 0, stdout: "WSP_LAUNCHED\n", stderr: "" };
      }
      if (cmd.includes("echo WSP_POLL\n")) {
        const next = polls[Math.min(at++, polls.length - 1)]!;
        if (typeof next !== "string") throw new Error(next.reject);
        return { exitCode: 0, stdout: next, stderr: "" };
      }
      return { exitCode: 0, stdout: "", stderr: "" };
    },
  };
  let result: unknown = null;
  let err: string | null = null;
  try {
    result = await ex.execDetached(machine as never, "echo hi", { deadlineMs: opts.deadlineMs ?? 60_000, pollMs: opts.pollMs ?? 1, onLine: (l: string) => lines.push(l) }, "/tmp/wsp-run");
  } catch (e) {
    err = (e as Error).message;
  }
  return { name, polls, opts, calls, lines, result, err };
}

const b64 = (s: string | Buffer): string => Buffer.from(s).toString("base64");
const poll = (exit: string, o: string | Buffer, e: string | Buffer, up: boolean, raw?: { out?: string; err?: string }): string =>
  `WSP_POLL\n${exit}\n${raw?.out ?? b64(o)}\n${raw?.err ?? b64(e)}\n${up ? "up" : "down"}\nWSP_POLL_END\n`;

it("roads scenes", async () => {
  const known = "Warning: Permanently added 'box' (ED25519) to the list of known hosts.\n";
  const forwards = [
    await forward("the_cut_keeps_a_lines_head", [{ err: `${"A".repeat(200)}${"B".repeat(800)}\n` }, { exit: { code: 255, signal: null } }]),
    await forward("only_the_last_50_stderr_lines_are_kept", [...Array.from({ length: 10 }, (_, i) => ({ err: `early ${i}\n` })), { err: known.repeat(50) }, { exit: { code: 255, signal: null } }]),
    await forward("the_last_line_client_words_keeps_is_the_words", [{ err: "Permission denied (publickey).\n" }, { err: known }, { err: "debug1: Exit status 255\n" }, { exit: { code: 255, signal: null } }]),
    await forward("up_on_a_line_written_in_two_pieces_and_padded", [{ out: "  WSP_BA" }, { out: "CK_UP \r\n" }, { exit: { code: 0, signal: null } }]),
    await forward("the_up_word_with_more_on_its_line_is_not_up", [{ out: "WSP_BACK_UP now\n" }, { out: "WSP_BACK_UP" }, { exit: { code: 255, signal: null } }]),
    await forward("an_exit_with_code_zero_says_so", [{ exit: { code: 0, signal: null } }]),
    await forward("a_child_that_fails_to_start_says_the_error", [{ fail: "spawn ssh ENOENT" }]),
    await forward("one_trailing_cr_goes_with_the_line_and_box_word_strips_the_rest", [{ err: "bad\tword\r\r\n" }, { exit: { code: 255, signal: null } }]),
    await forward("a_stderr_tail_with_no_newline_is_never_read", [{ err: "said first\n" }, { err: "never ended" }, { exit: { code: 255, signal: null } }]),
    await forward("a_flood_without_a_newline_keeps_only_its_last_4096_units", [{ err: "A".repeat(5000) }, { err: `${"B".repeat(100)}\n` }, { exit: { code: 255, signal: null } }]),
  ];
  const full = Buffer.alloc(262_144, 0x61);
  const runs = [
    await run("an_exit_in_hex_is_a_number", [poll("0x10", "", "", false)]),
    await run("an_exit_with_a_fraction_is_minus_one_with_the_note", [poll("1.5", "", "boom", false)]),
    await run("lines_go_stdout_then_stderr_per_poll_and_the_tails_at_the_end", [poll("", "a\nb", "c\n", true), poll("0", "", "d", false)]),
    await run("a_blank_line_is_not_handed_over_and_a_cr_is_kept", [poll("0", "x\r\n\n\ny\n", "", false)]),
    await run("a_full_chunk_with_an_exit_reads_again_before_the_exit", [poll("0", full, "", false), poll("0", "tail", "", false)]),
    await run("past_the_deadline_the_last_polls_exit_is_ignored", [poll("0", "late\n", "", false)], { deadlineMs: 0, launchDelayMs: 5 }),
    await run("a_block_closed_late_is_no_answer", ["WSP_POLL\n0\n\n\nup\nnoise\nWSP_POLL_END\n", poll("7", "", "", false)]),
    await run("the_streams_decode_as_node_decodes_base64", [poll("0", "", "", false, { out: "YQ", err: "Yg==Yw==" })]),
    await run("a_poll_that_rejects_is_no_answer_and_is_asked_again", [{ reject: "502 bad gateway" }, poll("3", "x", "", false)]),
    await run("a_utf8_char_split_across_polls_reaches_the_listener_whole", [poll("", Buffer.from([0xc3]), "", true), poll("0", Buffer.from([0xa9, 0x0a]), "", false)]),
  ];
  // The link's frames: every call a scene makes on a link machine and backend, recorded as sent, with the answers.
  const FACTS = {
    offer: "docker",
    capabilities: { liveCloneForks: false, replacesMachine: true, previewUrls: true, signedUrls: false, callbackRelay: true, diskSnapshots: true, images: true, snapshotsAnyLife: false, snapshotListing: true, templates: true, kept: false, copies: true, ownNetwork: true, sizes: [{ cpu: 2, memMb: 4096, rateUsdPerHour: 0.5 }] },
    pricing: { defaultSize: { cpu: 2, memMb: 4096 }, snapshotStorage: { freeGb: 0, usdPerGbMonth: 0, billedFrom: "" } },
  };
  const linkScene = async (name: string, previewUrl: string, act: (b: any, m: any) => Promise<unknown>): Promise<Record<string, unknown>> => {
    const sent: unknown[] = [];
    const forwarded: number[] = [];
    const l = {
      request: async (op: string, params: Record<string, unknown> = {}, opts: Record<string, unknown> = {}) => {
        sent.push({ op, params, opts });
        if (op === "machine.backend") return FACTS;
        if (op === "machine.get") return { machine: { id: "c1", kind: "sandbox", roads: { previewUrl: true, daemonAnswers: true, putBytes: true, describe: true, facts: true, metrics: true } } };
        if (op === "machine.list") return { machines: [] };
        if (op === "machine.exec") return { result: { exitCode: 0, stdout: "", stderr: "" } };
        if (op === "machine.state") return { state: "running" };
        if (op === "machine.downloadUrl" || op === "machine.uploadUrl") return { url: "https://u" };
        if (op === "machine.describe") return { shape: { cpu: 2 } };
        if (op === "machine.previewUrl") return { reach: { url: previewUrl, token: "", expiresAt: 0 } };
        if (op === "git.refused") throw Object.assign(new Error(""), { code: 5 });
        if (op === "git.coded") throw Object.assign(new Error("no remote"), { code: "no-remote" });
        if (op === "git.ok") return { ok: false, x: 1 };
        return {};
      },
      forward: async (port: number) => (forwarded.push(port), { localPort: 51000 + forwarded.length }),
    };
    const backend = await link.LinkBackend.open(l as never);
    const machine = await backend.get("c1");
    sent.length = 0;
    let answer: unknown = null;
    let err: string | null = null;
    try {
      answer = (await act(backend, machine)) ?? null;
    } catch (e) {
      err = (e as Error).message;
    }
    return { name, previewUrl, sent, forwarded, answer, err };
  };
  const links = [
    await linkScene("frames_of_the_roads_no_other_test_sends", "", async (b, m) => {
      await m.pause();
      await m.kill();
      const state = await m.state();
      const down = await m.downloadUrl("/a b");
      const up = await m.uploadUrl("/x");
      const shape = await m.describe();
      await m.metrics();
      await b.get("c1");
      await b.list();
      await b.list({});
      return { state, down, up, shape };
    }),
    await linkScene("an_empty_stdin_is_sent_empty", "", async (_b, m) => m.exec("cat", { stdin: new Uint8Array() })),
    await linkScene("a_daemon_frame_keeps_its_own_machine_id_place_and_its_replys_ok", "", async (_b, m) => [
      await m.daemonFrame({ op: "git.ok", machineId: "other", a: 1 }),
      await m.daemonFrame({ a: 2, op: "git.refused" }),
      await m.daemonFrame({ op: "git.coded" }),
    ]),
    await linkScene("a_route_on_the_schemes_own_port_names_no_port", "http://127.0.0.1:80/", async (_b, m) => m.previewUrl(3000)),
    await linkScene("a_route_on_a_scheme_with_no_default_keeps_its_port", "foo://h:80", async (_b, m) => m.previewUrl(3000)),
    await linkScene("a_route_on_an_ipv6_loopback_reads_its_port", "http://[::1]:8080/x", async (_b, m) => m.previewUrl(3000)),
    await linkScene("a_route_the_url_reader_refuses_is_invalid_url", "http://h:99999", async (_b, m) => m.previewUrl(3000)),
  ];
  mkdirSync(out, { recursive: true });
  // A string past 1,000 units is written as its length and its two ends, which tests/roads_scenes.rs reads the same way.
  const short = (_k: string, v: unknown): unknown => (typeof v === "string" && v.length > 1000 ? { len: v.length, head: v.slice(0, 20), tail: v.slice(-20) } : v);
  writeFileSync(join(out, "roads.json"), `${JSON.stringify({ forwards, runs, links }, short, 1)}\n`);
});
