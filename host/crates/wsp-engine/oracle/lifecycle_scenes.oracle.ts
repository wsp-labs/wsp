// SPDX-License-Identifier: AGPL-3.0-only
// The Workspace through moves that fail half-way, recording every machine call and hook in one log and what the
// workspace holds after: its machine, phase and first life, and the failure's words. Written as
// tests/fixtures/scenes/lifecycle.json for tests/lifecycle_scenes.rs, whose FakeMachine logs the same words. Run from
// the repository root:
//   pnpm exec vitest run --config host/crates/wsp-engine/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 lifecycle_scenes.oracle.ts
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { it } from "vitest";

const lc = await import("../../../../packages/engine/src/lifecycle.ts");

const out = fileURLToPath(new URL("../tests/fixtures/scenes/", import.meta.url));

function machine(id: string, log: string[], o: { resumeFails?: string } = {}): Record<string, unknown> {
  return {
    id,
    pause: async () => void log.push("pause"),
    resume: async () => {
      if (o.resumeFails !== undefined) {
        log.push("resume:failed");
        throw new Error(o.resumeFails);
      }
      log.push("resume");
    },
    kill: async () => void log.push("kill"),
    snapshot: async (name: string, life: { firstLife: boolean }) => (log.push(`snapshot:${name}:${life.firstLife}`), "snap_x"),
  };
}

type Act = (ws: InstanceType<typeof lc.Workspace>) => Promise<unknown>;

async function scene(name: string, initial: { phase?: "running" | "napping"; firstLife?: boolean }, hooks: (log: string[]) => Record<string, unknown>, act: Act, o: { resumeFails?: string } = {}): Promise<Record<string, unknown>> {
  const log: string[] = [];
  const ws = new lc.Workspace(machine("m1", log, o) as never, { goldenSnapshot: "g", wakeAttempts: 2, ...hooks(log) } as never, initial);
  let answer: unknown = null;
  let err: string | null = null;
  try {
    answer = (await act(ws)) ?? null;
  } catch (e) {
    err = (e as Error).message;
  }
  return { name, initial, log, answer, err, machine: ws.machineId, phase: ws.currentPhase, firstLife: ws.isFirstLife };
}

it("lifecycle scenes", async () => {
  const resurrect = (log: string[]) => async () => (log.push("resurrect"), machine("m2", log));
  const fail = (log: string[], what: string) => async () => {
    log.push(what);
    throw new Error(`${what} broke`);
  };
  const scenes = [
    await scene("an_empty_fault_is_still_a_fault", { phase: "napping" }, () => {
      let n = 0;
      return { wakeCheck: async () => (n++ === 0 ? "" : undefined) };
    }, ws => ws.wake()),
    await scene("a_rebuild_whose_restore_fails_keeps_the_new_machine_and_leaves_the_old_alive", { phase: "napping", firstLife: false }, log => ({ resurrect: resurrect(log), restoreVault: fail(log, "restore") }), ws => ws.rebuild()),
    await scene("an_upgrade_whose_export_fails_changes_nothing", { phase: "napping", firstLife: false }, log => ({ resurrect: resurrect(log), vaultExport: fail(log, "export") }), ws => ws.upgrade()),
    await scene("an_upgrade_whose_resurrect_fails_after_the_retire_keeps_the_killed_machine", { phase: "napping", firstLife: false }, log => ({ resurrect: fail(log, "resurrect"), vaultExport: async () => (log.push("export"), Buffer.from("v")) }), ws => ws.upgrade()),
    await scene("an_upgrade_whose_import_fails_keeps_the_new_machine_and_the_old_phase", { phase: "napping", firstLife: false }, log => ({ resurrect: resurrect(log), vaultExport: async () => (log.push("export"), Buffer.from("v")), vaultImport: fail(log, "import") }), ws => ws.upgrade()),
    await scene("an_empty_payload_is_still_imported", { phase: "napping", firstLife: false }, log => ({ resurrect: resurrect(log), vaultExport: async () => (log.push("export"), Buffer.alloc(0)), vaultImport: async (_m: unknown, p: Buffer) => void log.push(`import:${p.length}`) }), ws => ws.upgrade()),
    await scene("a_failed_resume_keeps_the_first_life_and_naps", { phase: "napping" }, () => ({}), ws => ws.wake(), { resumeFails: "resume broke" }),
    await scene("a_wakes_re_pause_stashes_no_vault", { phase: "napping" }, log => {
      let n = 0;
      return { stashVault: async () => void log.push("stash"), wakeCheck: async () => (n++ === 0 ? "silent" : undefined) };
    }, ws => ws.wake()),
    await scene("a_checkpoint_of_a_napping_workspace_snapshots_and_moves_nothing", { phase: "napping" }, () => ({}), ws => ws.checkpoint("cp")),
    await scene("a_wake_that_runs_out_fails_with_plain_words", { phase: "napping" }, () => ({ wakeAttempts: 1, wakeCheck: async () => "silent" }), ws => ws.wake()),
  ];
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, "lifecycle.json"), `${JSON.stringify(scenes, null, 1)}\n`);
});
