// SPDX-License-Identifier: AGPL-3.0-only
// The words and limits the status tests compare to, read off the TS the area imports (packages/protocol and
// packages/runtime at 65f6b89c7): the reason a recorded-again machine's status carries, and how long a deleted
// machine is watched and an unreached one is kept. Written as tests/fixtures/oracle/status.json. Run from the
// repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 status.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { RECORD_RESTORED } from "@wsp/protocol";
import { CREATED_AT_LABEL, GOLDEN_LABEL, WORKSPACE_LABEL } from "@wsp/engine";

const { DROPPED_WATCH_MS, UNRECORDED_DELETE_MS, DROPPED, UNRECORDED, WORKSPACE_NAMES, WORKSPACES } = await import("../../../../packages/runtime/src/types/internal.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/status.json", import.meta.url));

it("writes the status fixture", () => {
  writeFileSync(
    out,
    `${JSON.stringify({ recordRestored: RECORD_RESTORED, droppedWatchMs: DROPPED_WATCH_MS, unrecordedDeleteMs: UNRECORDED_DELETE_MS, collections: { dropped: DROPPED, unrecorded: UNRECORDED, names: WORKSPACE_NAMES, workspaces: WORKSPACES }, labels: { workspace: WORKSPACE_LABEL, golden: GOLDEN_LABEL, createdAt: CREATED_AT_LABEL } }, null, 1)}\n`,
  );
});
