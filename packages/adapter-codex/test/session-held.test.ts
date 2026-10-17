// SPDX-License-Identifier: AGPL-3.0-only
// What a machine holds for a thread whose resume Codex refused, read the way
// the host reads it when the lost-session fallback fires: the guest line run
// in bash over a CODEX_HOME built here, with the thread index's columns as
// codex-cli 0.163.0 has them (rollout_path from its first migration,
// history_mode from 0040).
import { execFile } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import { afterAll, describe, expect, it } from "vitest";
import { createCodexAdapter } from "../src/adapter.js";
import { parseSessionHeld, sessionHeldCommand } from "../src/lost-session.js";

const THREAD = "01a1424a-7c2e-7d31-9f00-5b2a1c3d4e5f";
const OTHER = "01a1424b-fa29-7aa0-8c11-0d9e8f7a6b5c";
const DAY = "sessions/2026/10/16";
const OPENING = "Remember the word lantern.";
const run = promisify(execFile);

const homes: string[] = [];
afterAll(() => {
  for (const home of homes) rmSync(home, { recursive: true, force: true });
});

/** A CODEX_HOME holding the rollout files named, each one line long, and a state db of the schema given. */
async function codexHome(o: { files?: string[]; schema?: "0.163.0" | "older"; rows?: { id: string; path: string }[] }): Promise<string> {
  const home = mkdtempSync(join(tmpdir(), "wsp-codex-held-"));
  homes.push(home);
  for (const file of o.files ?? []) {
    mkdirSync(join(home, file, ".."), { recursive: true });
    writeFileSync(join(home, file), `${JSON.stringify({ type: "user_message", text: OPENING })}\n`);
  }
  if (o.schema !== undefined) {
    const mode = o.schema === "0.163.0" ? ", history_mode text not null default 'legacy'" : "";
    const rows = (o.rows ?? []).map(r => `insert into threads (id, rollout_path, title${mode === "" ? "" : ", history_mode"}) values ('${r.id}', '${join(home, r.path)}', '${OPENING}'${mode === "" ? "" : ", 'paginated'"});`);
    await run("sqlite3", [join(home, "state_5.sqlite"), [`create table threads (id text primary key, rollout_path text not null, title text not null${mode});`, ...rows].join("\n")]);
  }
  return home;
}

const heldIn = async (home: string, threadId = THREAD): Promise<string> => parseSessionHeld((await run("bash", ["-c", sessionHeldCommand({ home, threadId })])).stdout);
const BOOT = /^boot (unread|[0-9a-f-]{36}); /;

describe("what the machine holds for a session Codex would not resume", () => {
  it("names the row's path and says the file is missing, beside every rollout file that carries the thread id", async () => {
    const named = `${DAY}/rollout-2026-10-16T01-19-52-${THREAD}.jsonl`;
    const reverted = `${DAY}/rollout-2026-10-16T01-20-10-${THREAD}_0d9e8f7a.jsonl`;
    const home = await codexHome({ files: [reverted, `${DAY}/rollout-2026-10-16T01-20-36-${OTHER}.jsonl`], schema: "0.163.0", rows: [{ id: THREAD, path: named }] });
    const line = await heldIn(home);
    expect(line).toMatch(BOOT);
    const bytes = `${JSON.stringify({ type: "user_message", text: OPENING })}\n`.length;
    expect(line).toContain(`; rollout files: ${join(home, reverted)} (${bytes} bytes); `);
    expect(line).toContain(`; index row: state_5.sqlite {"path":"${join(home, named)}","mode":"paginated"}, the file it names is missing`);
    expect(line).not.toContain(OTHER);
    expect(line).not.toContain(OPENING);
  });

  it("says the file is there when the row names one that is", async () => {
    const named = `${DAY}/rollout-2026-10-16T01-19-52-${THREAD}.jsonl`;
    const line = await heldIn(await codexHome({ files: [named], schema: "0.163.0", rows: [{ id: THREAD, path: named }] }));
    expect(line).toContain(`; index row: state_5.sqlite {"path":"${join(homes.at(-1)!, named)}","mode":"paginated"}, the file it names is there`);
  });

  it("says none when no file and no row carry the thread, and reads an index from before history_mode by its path", async () => {
    expect(await heldIn(await codexHome({ schema: "0.163.0", rows: [{ id: OTHER, path: "x.jsonl" }] }))).toMatch(/; rollout files: none; index row: no row in state_5\.sqlite$/);
    const named = `${DAY}/rollout-2026-10-16T01-19-52-${THREAD}.jsonl`;
    const older = await codexHome({ files: [named], schema: "older", rows: [{ id: THREAD, path: named }] });
    expect(await heldIn(older)).toContain(`; index row: state_5.sqlite {"path":"${join(older, named)}"}, the file it names is there`);
    expect(await heldIn(await codexHome({}))).toMatch(/; index row: no state db$/);
    expect(await heldIn(join(tmpdir(), "wsp-no-such-codex-home"))).toMatch(/; index row: no Codex home$/);
  });

  it("is what the adapter's own reading answers, through the exec it is handed", async () => {
    const named = `${DAY}/rollout-2026-10-16T01-19-52-${THREAD}.jsonl`;
    const home = await codexHome({ schema: "0.163.0", rows: [{ id: THREAD, path: named }] });
    const codex = createCodexAdapter({ exec: () => { throw new Error("no turns here"); }, home, login: "codex login", baseEnv: { PATH: "/usr/bin:/bin" } });
    const line = await codex.sessionHeld(THREAD, async command => (await run("bash", ["-c", command])).stdout);
    expect(line).toMatch(/; rollout files: none; index row: state_5\.sqlite .*"mode":"paginated"\}, the file it names is missing$/);
  });

  it("says the index was not read, never that it holds no row, when sqlite3 cannot open the db", async () => {
    const home = await codexHome({});
    writeFileSync(join(home, "state_5.sqlite"), "lantern ".repeat(100));
    const line = await heldIn(home);
    expect(line).toMatch(/; index row: not read from state_5\.sqlite .*file is not a database/);
    expect(line).not.toContain("lantern");
  });

  it("says a reading stopped at its deadline was cut short, not that the home holds nothing more", () => {
    expect(parseSessionHeld("boot 1f0c\nfile 12 /root/.codex/sessions/a.jsonl\n")).toBe("boot 1f0c; rollout files: /root/.codex/sessions/a.jsonl (12 bytes), the listing cut short; index row: not read");
  });

  it("refuses an id that is not a plain slug before anything runs", () => {
    expect(() => sessionHeldCommand({ home: "/root/.codex", threadId: "x'; rm -rf /" })).toThrow(/plain slug/);
  });
});
