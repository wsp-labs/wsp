// SPDX-License-Identifier: AGPL-3.0-only
// BoxBackend and BoxMachine run against a scripted Boat API and a clock whose sleeps cost nothing, scene by scene,
// recording every request (method, path, query in order, headers, the body's exact bytes), every sleep, every warn
// line and the answer or the error's words. tests/fixtures/scenes/box.json holds each scene's script and what the TS
// did; tests/box_machine_scenes.rs and tests/box_fleet_scenes.rs replay the script on the Rust backend. Random bytes are pinned to ab and
// Math.random to 0, which the Rust FakeClock's draw matches. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 box_scenes.oracle.ts
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it, vi } from "vitest";

vi.mock("node:crypto", async importOriginal => {
  const real = await importOriginal<typeof import("node:crypto")>();
  return { ...real, randomBytes: (n: number) => Buffer.alloc(n, 0xab) };
});

const box = await import("../../../../packages/engine/src/box-backend.ts");

type Reply = { status: number; body: unknown } | { road: "offline" | "dropped" };
type Routes = Record<string, Reply[]>;

const ID = "bx_tumrjngm";
const P = (suffix = ""): string => `/sandboxes/${ID}${suffix}`;

const BOX = (id: string, state: string, extra: Record<string, unknown> = {}): Record<string, unknown> => ({
  id, name: "Box 2026-09-11 06:28", state, error: null, type: "small", vcpu: 2, memoryGB: 4, createdAt: "2026-09-11T06:28:06.053Z", ...extra,
});
const INFO = (id: string, state: string, extra: Record<string, unknown> = {}): Reply => ({ status: 200, body: { ok: true, type: "sandbox.info", sandbox: BOX(id, state, extra) } });
const ERR = (status: number, code: string, message: string): Reply => ({ status, body: { ok: false, type: "sandbox.error", status, code, message, requestId: "req_1" } });
const LIMITS = (tier: string): Reply => ({ status: 200, body: { ok: true, type: "limits.info", accessTier: tier, maxActiveSandboxes: 2, startLimits: { perMinute: 5, perHour: 25, perDay: 75 } } });
const CMD = (exitCode: number | null, extra: Record<string, unknown> = {}): Reply => ({
  status: 200,
  body: { ok: true, type: "command.finished", success: exitCode === 0, exitCode, signal: null, stdout: "", stderr: "", timedOut: false, ...extra },
});
const OK = (body: unknown = { ok: true }): Reply => ({ status: 200, body });
const NAMED = (name: string, status: string, extra: Record<string, unknown> = {}): Record<string, unknown> => ({ name, status, sourceSandboxId: ID, createdAt: "2026-09-11T06:47:23.959Z", ...extra });

const roadError = (r: "offline" | "dropped"): TypeError =>
  r === "offline"
    ? new TypeError("fetch failed", { cause: Object.assign(new Error("getaddrinfo ENOTFOUND api.example"), { code: "ENOTFOUND" }) })
    : new TypeError("fetch failed", { cause: Object.assign(new Error("read ECONNRESET"), { code: "ECONNRESET" }) });

interface Call { method: string; path: string; query: [string, string][]; headers: Record<string, string>; raw: string | null }

async function scene(
  name: string,
  routes: Routes,
  budgets: Record<string, number>,
  act: (b: InstanceType<typeof box.BoxBackend>, m: InstanceType<typeof box.BoxMachine>) => Promise<unknown>,
): Promise<Record<string, unknown>> {
  routes = { "GET /limits": [LIMITS("trial")], ...routes };
  const queues = new Map(Object.entries(structuredClone(routes)));
  const calls: Call[] = [];
  const fetch = async (url: RequestInfo | URL, init?: RequestInit): Promise<Response> => {
    const u = new URL(String(url));
    const method = init?.method ?? "GET";
    const path = u.pathname.startsWith("/api/v1/") ? u.pathname.slice("/api/v1".length) : u.href;
    const headers = Object.fromEntries(Object.entries((init?.headers ?? {}) as Record<string, string>).map(([k, v]) => [k.toLowerCase(), v]));
    calls.push({ method, path, query: [...u.searchParams.entries()], headers, raw: typeof init?.body === "string" ? init.body : null });
    const queue = queues.get(`${method} ${path}`);
    const next: Reply = queue === undefined || queue.length === 0 ? ERR(404, "not_found", "not_found") : queue.length > 1 ? queue.shift()! : queue[0]!;
    if ("road" in next) throw roadError(next.road);
    return new Response(JSON.stringify(next.body), { status: next.status });
  };
  const sleeps: number[] = [];
  const clock = { at: 1_000_000, now: () => clock.at, sleep: async (ms: number) => { sleeps.push(ms); clock.at += ms; } };
  const warns: string[] = [];
  const warn = vi.spyOn(console, "warn").mockImplementation((line: string) => { warns.push(String(line)); });
  const random = vi.spyOn(Math, "random").mockReturnValue(0);
  const backend = new box.BoxBackend({ apiKey: "box_x", fetch: fetch as typeof globalThis.fetch, clock, budgets: { pollMs: 10, pauseMs: 100, resumeMs: 100, createMs: 100, snapshotMs: 100, restoreMs: 100, ...budgets } });
  const machine = new box.BoxMachine(backend, ID, "sandbox");
  let out: unknown = null;
  let err: string | null = null;
  try {
    out = (await act(backend, machine)) ?? null;
  } catch (e) {
    err = e instanceof Error ? e.message : String(e);
  } finally {
    warn.mockRestore();
    random.mockRestore();
  }
  return { name, routes, budgets, out, err, calls, sleeps, warns, elapsed: clock.at - 1_000_000 };
}

const json = (v: unknown): unknown => JSON.parse(JSON.stringify(v ?? null));

it("box scenes", async () => {
  const scenes: Record<string, unknown>[] = [];
  const add = async (...a: Parameters<typeof scene>): Promise<void> => {
    scenes.push(await scene(...a));
  };
  const host = (url: string): Reply => OK({ ok: true, url });
  const cmds = (...r: Reply[]): Routes => ({ [`POST ${P("/commands")}`]: r, [`POST ${P("/host")}`]: [host(`https://3000-x.on.boat.dev/?_token=t9`)] });

  // The firewall loop (box-backend.ts:1330).
  await add("firewall_wiped_past_the_add_window_warns_without_words", cmds(CMD(1), CMD(0), CMD(0), CMD(0), CMD(0), CMD(0), CMD(0), CMD(1), CMD(0), CMD(1)), {}, (_b, m) => m.previewUrl(3000));
  await add("firewall_keeps_the_last_refusal_after_a_later_add_held", cmds(CMD(1), CMD(1, { stderr: "  ERROR: ufw busy \n" }), CMD(0), CMD(0), CMD(0), CMD(0), CMD(0), CMD(0), CMD(1), CMD(0), CMD(1)), {}, (_b, m) => m.previewUrl(3000));
  await add("firewall_blank_stderr_hides_stdout", cmds(CMD(1), CMD(1, { stderr: " \n", stdout: "out" })), { pollMs: 1000 }, (_b, m) => m.previewUrl(3000));
  await add("firewall_keeps_the_last_200_units_of_a_long_refusal", cmds(CMD(1), CMD(1, { stdout: `${"A".repeat(150)}${"B".repeat(150)}\n` })), { pollMs: 1000 }, (_b, m) => m.previewUrl(3000));
  await add("firewall_standing_read_that_throws_reads_as_not_standing", { [`POST ${P("/commands")}`]: [ERR(400, "bad_request", "no shell"), CMD(0), CMD(0), CMD(0), CMD(0), CMD(0), CMD(0), CMD(0)], [`POST ${P("/host")}`]: [host("https://x.on.boat.dev/")] }, {}, (_b, m) => m.previewUrl(8080));
  await add("route_token_is_the_decoded_query_value", { [`POST ${P("/host")}`]: [host("https://7070-x.on.boat.dev/p?a=1&_token=a+b%2Fc&_token=second")] }, {}, (_b, m) => m.previewUrl(7070));

  // pause (:1221).
  await add("pause_of_an_archived_box_sends_nothing_and_sleeps_not", { [`GET ${P()}`]: [INFO(ID, "archived")] }, {}, (_b, m) => m.pause());
  await add("pause_archived_at_the_budget_read_is_done", { [`GET ${P()}`]: [INFO(ID, "running"), ...Array(9).fill(INFO(ID, "archiving")), INFO(ID, "archived")], [`POST ${P("/stop")}`]: [OK()] }, {}, (_b, m) => m.pause());
  await add("pause_archiving_at_the_budget_read_times_out", { [`GET ${P()}`]: [INFO(ID, "archiving")] }, {}, (_b, m) => m.pause());
  await add("pause_of_a_lost_box_fails_missing_and_sends_no_stop", {}, {}, (_b, m) => m.pause());
  await add("pause_wait_stands_in_for_the_budget", { [`GET ${P()}`]: [INFO(ID, "running"), INFO(ID, "archiving")], [`POST ${P("/stop")}`]: [OK()] }, {}, (_b, m) => m.pause({ waitMs: 35 }));

  // snapshot (:1202).
  const snapRoutes = (...r: Reply[]): Routes => ({ "POST /named-snapshots": [OK({ ok: true })], "GET /named-snapshots/wsp-v1": r });
  await add("snapshot_failed_with_empty_error_has_no_colon", snapRoutes(OK({ snapshot: NAMED("wsp-v1", "saving") }), OK({ snapshot: NAMED("wsp-v1", "failed", { error: "" }) })), {}, (_b, m) => m.snapshot("wsp-v1", "running"));
  await add("snapshot_ready_at_the_budget_read_wins", snapRoutes(...Array(4).fill(OK({ snapshot: NAMED("wsp-v1", "saving") })), OK({ snapshot: NAMED("wsp-v1", "ready") })), { snapshotMs: 2500, pollMs: 500 }, (_b, m) => m.snapshot("wsp-v1", "running"));
  await add("snapshot_budget_words_round_half_up", snapRoutes(OK({ snapshot: NAMED("wsp-v1", "saving") })), { snapshotMs: 2500, pollMs: 500 }, (_b, m) => m.snapshot("wsp-v1", "paused"));
  await add("snapshot_budget_under_half_a_second_reads_0_s", snapRoutes(OK({ snapshot: NAMED("wsp-v1", "saving") })), {}, (_b, m) => m.snapshot("wsp-v1", "running"));

  // kill (:1277) and the backstop it forgets.
  const kept = async (b: InstanceType<typeof box.BoxBackend>, m: InstanceType<typeof box.BoxMachine>, then: () => Promise<unknown>): Promise<unknown> => {
    await b.backstop(m, 1_000_000 + 600_000);
    let threw: string | null = null;
    try {
      await then();
    } catch (e) {
      threw = (e as Error).message;
    }
    return { threw, timer: b.lastTimer(ID) ?? null, deletion: m.deletion ?? null };
  };
  await add("kill_answered_without_an_operation_keeps_no_deletion_and_forgets_the_timer", { [`PATCH ${P()}`]: [OK()], [`DELETE ${P()}`]: [OK({ ok: true })] }, {}, (b, m) => kept(b, m, () => m.kill()));
  await add("kill_refused_keeps_the_timer", { [`PATCH ${P()}`]: [OK()], [`DELETE ${P()}`]: [ERR(409, "conflict", "busy")] }, {}, (b, m) => kept(b, m, () => m.kill()));
  await add("kill_of_a_gone_box_forgets_the_timer", { [`PATCH ${P()}`]: [OK()] }, {}, (b, m) => kept(b, m, () => m.kill()));

  // exec's readings (:1128, :1147).
  const ex = (opts: { timeoutMs?: number }) => (_b: unknown, m: InstanceType<typeof box.BoxMachine>) => m.exec("echo hi", opts);
  await add("exec_exit_code_wins_over_timed_out", cmds(CMD(0, { timedOut: true, stdout: "x" })), {}, ex({}));
  await add("exec_null_exit_timed_out_reads_124", cmds(CMD(null, { timedOut: true })), {}, ex({ timeoutMs: 1500 }));
  await add("exec_null_exit_reads_minus_1_and_absent_streams_read_empty", cmds({ status: 200, body: { ok: true, exitCode: null } }), {}, ex({ timeoutMs: 0 }));
  await add("exec_at_the_inline_cap_stays_inline", cmds(CMD(0)), {}, ex({ timeoutMs: 30_000 }));
  await add("exec_loader_emfile_on_a_later_line_is_unusable", cmds(CMD(0, { stderr: "warn\nsudo: error while loading shared libraries: libc.so.6: cannot open shared object file: Error 24\n" })), {}, ex({}));
  await add("exec_loader_emfile_not_at_a_line_start_is_a_plain_answer", cmds(CMD(1, { stderr: " bash: error while loading shared libraries: x: Error 24" })), {}, ex({}));
  await add("exec_no_work_folder_wins_over_machine_not_running", cmds(ERR(409, "machine_not_running", "CWD must be an existing directory")), {}, ex({}));

  // bytes (:1360, :1393).
  await add("put_of_no_bytes_into_tmp_is_one_empty_put", { [`PUT ${P("/files")}`]: [OK()] }, {}, (_b, m) => m.putBytes("/tmp/empty", new Uint8Array()));
  await add("put_to_the_bare_tmp_path_joins_into_root", { [`PUT ${P("/files")}`]: [OK()], [`POST ${P("/commands")}`]: [CMD(0)] }, {}, (_b, m) => m.putBytes("/tmp", new Uint8Array([1])));
  await add("put_to_a_relative_path_cuts_the_folder_as_the_ts_does", { [`PUT ${P("/files")}`]: [OK()], [`POST ${P("/commands")}`]: [CMD(0)] }, {}, (_b, m) => m.putBytes("file.txt", new Uint8Array([1, 2])));
  await add("put_join_failure_names_stdout_when_stderr_is_blank", { [`PUT ${P("/files")}`]: [OK()], [`POST ${P("/commands")}`]: [CMD(1, { stdout: "WSP_SHORT 1\n" })] }, {}, (_b, m) => m.putBytes("/root/a b/x", new Uint8Array([1, 2])));
  const readOut = (p: Promise<Buffer>): Promise<unknown> => p.then(b => ({ bytes: [...b] }));
  await add("read_with_a_blank_size_reads_zero_and_takes_the_straight_road", { [`POST ${P("/commands")}`]: [CMD(0, { stdout: "\n" })], [`GET ${P("/files")}`]: [OK({ content: "AQID" })] }, {}, (_b, m) => readOut(m.readBytes("/tmp/x")));
  await add("read_size_in_exponent_form_is_a_number", { [`POST ${P("/commands")}`]: [CMD(0, { stdout: " 3e0 \n" }), CMD(0), CMD(0)], [`GET ${P("/files")}`]: [OK({ content: "AQID" })] }, {}, (_b, m) => readOut(m.readBytes("/root/x")));
  await add("read_size_with_a_fraction_is_refused", { [`POST ${P("/commands")}`]: [CMD(0, { stdout: "12.5\n" })] }, {}, (_b, m) => readOut(m.readBytes("/tmp/x")));
  await add("read_cut_refused_still_removes_every_piece", { [`POST ${P("/commands")}`]: [CMD(1, { stderr: "split: no such file\n" }), CMD(0)] }, {}, (_b, m) => readOut(m.readBytes("/root/x", { size: 4 * 1024 * 1024 + 1 })));
  await add("read_short_still_removes_and_names_both_sizes", { [`POST ${P("/commands")}`]: [CMD(0), ERR(400, "bad", "rm refused")], [`GET ${P("/files")}`]: [OK({ content: "AQID" }), OK({})] }, {}, (_b, m) => readOut(m.readBytes("/root/x", { size: 4 * 1024 * 1024 + 1 })));

  // describe (:1297).
  await add("describe_reads_half_a_gigabyte_and_leaves_out_what_is_missing", { [`GET ${P()}`]: [INFO(ID, "running", { vcpu: undefined, memoryGB: 0.5, type: "huge", createdAt: null })] }, {}, (_b, m) => m.describe());

  // create's POST and its lost answers (:810, :831).
  const created = (state: string, status?: string): Reply => ({ status: 201, body: { ok: true, ...(status !== undefined ? { status } : {}), sandbox: BOX("bx_new", state, { name: "wsp" }) } });
  const createRoutes = (...posts: Reply[]): Routes => ({
    "POST /sandboxes": posts,
    "GET /sandboxes/bx_new": [INFO("bx_new", "ready", { name: "wsp" })],
    "POST /sandboxes/bx_new/commands": [CMD(0)],
    "DELETE /sandboxes/bx_new": [OK({ ok: true, operation: { id: "op_1" } })],
  });
  const spec = (extra: Record<string, unknown> = {}) => ({ kind: "sandbox" as const, idempotencyKey: "key-1", ...extra });
  const made = async (p: Promise<{ id: string; replayed?: boolean }>): Promise<unknown> => {
    const m = await p;
    return { id: m.id, replayed: m.replayed ?? false };
  };
  await add("create_in_progress_waits_2000_and_the_request_asks_a_gateway_again_itself", createRoutes(ERR(409, "idempotency_in_progress", "in progress"), ERR(502, "bad_gateway", "bad gateway"), created("provisioning", "provisioning")), {}, b => made(b.create(spec())));
  await add("create_lost_then_answered_warns_the_first_lost_words_and_is_not_replayed", createRoutes({ road: "dropped" }, ERR(409, "idempotency_in_progress", "in progress"), { road: "offline" }, created("ready", "ready")), {}, b => made(b.create(spec())));
  await add("create_lost_until_the_window_answers_the_last_failure", createRoutes({ road: "dropped" }), {}, b => made(b.create(spec())));
  await add("create_refused_after_a_lost_answer_is_the_refusal", createRoutes({ road: "dropped" }, ERR(400, "bad_request", "bad name")), {}, b => made(b.create(spec())));
  await add("create_with_an_empty_key_sends_it_empty", createRoutes(created("provisioning", "provisioning")), {}, b => made(b.create(spec({ idempotencyKey: "" }))));
  await add("create_with_sent_bytes_names_the_box_after_the_bytes", { ...createRoutes(created("provisioning", "provisioning")), "PATCH /sandboxes/bx_new": [OK()] }, {}, b => made(b.create(spec(), undefined, JSON.stringify({ type: "small", ttlSeconds: 60, noEnv: true, name: "wsp;w=from-bytes" }))));
  const timerAfter = async (b: InstanceType<typeof box.BoxBackend>, p: Promise<unknown>): Promise<unknown> => {
    let threw: string | null = null;
    try {
      await p;
    } catch (e) {
      threw = (e as Error).message;
    }
    return { threw, timer: b.lastTimer("bx_new") ?? null };
  };
  await add("create_replayed_that_fails_keeps_the_box_and_its_timer", { ...createRoutes(created("ready", "ready")), "POST /sandboxes/bx_new/commands": [ERR(400, "bad", "no")] }, {}, b => timerAfter(b, b.create(spec({ idleTimeoutMs: 900_000 }))));
  await add("create_fresh_that_fails_with_a_refused_kill_keeps_its_timer", { ...createRoutes(created("provisioning", "provisioning")), "POST /sandboxes/bx_new/commands": [ERR(400, "bad", "no")], "DELETE /sandboxes/bx_new": [ERR(409, "conflict", "busy")] }, {}, b => timerAfter(b, b.create(spec({ idleTimeoutMs: 900_000 }))));
  await add("create_rename_refused_kills_and_answers_the_rename", { ...createRoutes(created("provisioning", "provisioning")), "GET /sandboxes/bx_new": [INFO("bx_new", "ready", { name: "snap-name" })], "PATCH /sandboxes/bx_new": [ERR(400, "bad", "name taken")] }, {}, b => timerAfter(b, b.create(spec())));

  // replay (:880).
  const bytes = JSON.stringify({ type: "small", ttlSeconds: 60, noEnv: true, name: "wsp;w=w1" });
  const replayOut = async (b: InstanceType<typeof box.BoxBackend>, at: string): Promise<unknown> => {
    const before = json(await b.limits());
    const m = await b.replayCreate("key-1", bytes, at);
    const after = json(await b.limits());
    return { machine: m === undefined ? null : { id: m.id, kind: m.kind, labels: m.labels ?? null, seen: m.seen ?? null, replayed: m.replayed ?? false }, countedUnchanged: JSON.stringify(before) === JSON.stringify(after) };
  };
  const iso = (ms: number): string => new Date(ms).toISOString();
  await add("replay_of_an_unparsable_time_is_asked", { "POST /sandboxes": [created("ready", "ready")], "PATCH /sandboxes/bx_new": [OK()] }, {}, b => replayOut(b, "not a time"));
  await add("replay_at_the_window_edge_is_not_asked", { "POST /sandboxes": [created("ready", "ready")] }, {}, b => replayOut(b, iso(1_000_000 - 86_275_000)));
  await add("replay_a_millisecond_inside_the_window_is_asked_and_not_counted", { "POST /sandboxes": [created("ready", "ready")], "PATCH /sandboxes/bx_new": [OK()] }, {}, b => replayOut(b, iso(1_000_000 - 86_274_999)));
  await add("replay_of_a_cancelled_box_is_not_renamed", { "POST /sandboxes": [created("cancelled", "cancelled")] }, {}, b => replayOut(b, iso(1_000_000)));

  // paging, both listings, snapshots (:917-:982).
  const page = (rows: unknown[], next?: string | null): Reply => OK({ sandboxes: rows, ...(next !== undefined ? { pageInfo: { nextCursor: next } } : {}) });
  await add("list_pages_follow_an_empty_cursor_and_encode_it", { "GET /sandboxes": [page([BOX("bx_1", "running", { name: "wsp;w=a" })], "a/b=c d"), page([BOX("bx_2", "running", { name: "wsp;w=b" })], ""), page([BOX("bx_1", "archived", { name: "wsp;w=a" }), BOX("bx_3", "archived", { name: null })], null)] }, {}, b => b.list());
  await add("named_after_skips_nameless_boxes_and_keeps_the_first_row_per_id", { "GET /sandboxes": [page([BOX("bx_1", "running", { name: "snap-a", createdAt: null }), BOX("bx_2", "running", { name: null })]), page([BOX("bx_1", "archived", { name: "snap-a" }), BOX("bx_4", "archived", { name: "snap-b" })])] }, {}, b => b.namedAfter(new Set(["snap-a", "snap-b"])));
  const U = "dc8cd70e-d16b-472a-b5d7-070bc84db7a7";
  const hist = (rows: unknown[], next?: string): Reply => OK({ snapshots: rows, ...(next !== undefined ? { pageInfo: { nextCursor: next } } : {}) });
  const snapRoutes2: Routes = {
    "GET /named-snapshots": [OK({ snapshots: [NAMED("wsp-v1", "ready", { sizeBytes: null })] })],
    "GET /snapshots": [hist([{ id: U, sizeBytes: null, contentSizeBytes: "5", completedAt: "", createdAt: "2026-09-11T00:00:00Z", chainId: null }], "c2"), hist([{ id: "11111111-2222-3333-4444-555555555555", completedAt: "2026-09-12T00:00:00Z", contentSizeBytes: 0 }])],
  };
  await add("snapshot_rows_read_blank_completion_null_chain_and_string_sizes_as_the_ts", snapRoutes2, {}, b => b.listSnapshots());
  await add("snapshot_history_asked_in_capitals_is_read_to_its_end", snapRoutes2, {}, b => b.listSnapshots([U.toUpperCase()]));
  await add("snapshot_history_stops_at_the_page_holding_every_asked_id", snapRoutes2, {}, b => b.listSnapshots([U, "wsp-v1"]));
  await add("delete_snapshot_by_a_capital_uuid_takes_the_history_road", {}, {}, b => b.deleteSnapshot(U.toUpperCase()));

  // backstops (:1019).
  const bs = (b: InstanceType<typeof box.BoxBackend>, m: InstanceType<typeof box.BoxMachine>, until: number, exact = false) => b.backstop(m, until, exact ? { exact: true } : {}).then(() => "ok", (e: Error) => `threw: ${e.message}`);
  await add("backstop_after_an_hour_reads_the_kept_limits_of_any_age", { [`PATCH ${P()}`]: [OK()] }, {}, async (b, m) => {
    await b.limits();
    (b as unknown as { clock: { at: number } }).clock.at += 3_600_000;
    return [await bs(b, m, 99_000_000), json(b.lastTimer(ID)), b.capabilities.sizes.length];
  });
  await add("backstop_whose_limits_read_fails_backs_off", { "GET /limits": [ERR(400, "bad", "limits down"), LIMITS("trial")], [`PATCH ${P()}`]: [OK()] }, {}, async (b, m) => [await bs(b, m, 2_000_000), await bs(b, m, 2_000_000), json(b.lastTimer(ID))]);
  await add("backstop_in_the_past_asks_one_second", { "GET /limits": [LIMITS("paid")], [`PATCH ${P()}`]: [OK()] }, {}, async (b, m) => [await bs(b, m, 0), json(b.lastTimer(ID))]);
  await add("backstop_exact_ignores_the_backoff", { "GET /limits": [LIMITS("paid")], [`PATCH ${P()}`]: [ERR(400, "bad", "no"), OK()] }, {}, async (b, m) => [await bs(b, m, 2_000_000), await bs(b, m, 2_000_000), await bs(b, m, 2_000_000, true), json(b.lastTimer(ID))]);
  await add("backstop_backoff_doubles_from_the_failure_read_at_entry", { "GET /limits": [LIMITS("paid")], [`PATCH ${P()}`]: [ERR(400, "bad", "no")] }, {}, async (b, m) => {
    const out: unknown[] = [];
    for (const wait of [0, 30_000, 60_000, 120_000, 240_000, 300_000, 299_999]) {
      (b as unknown as { clock: { at: number } }).clock.at += wait;
      out.push(await bs(b, m, 9_000_000));
    }
    return out;
  });

  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "box.json"), `${JSON.stringify(scenes, null, 1)}\n`);
});

const out = fileURLToPath(new URL("../tests/fixtures/scenes/", import.meta.url));
