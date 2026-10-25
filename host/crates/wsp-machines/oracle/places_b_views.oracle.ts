// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the slice b views tests compare to: the sentences views.ts builds from the protocol, and the fork room
// a capacity reads as; written as tests/fixtures/oracle/places_b_views.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-machines/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 places_b_views.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { it } from "vitest";
import { forkRoom, floorFailedLine, placeBlocked, pluginOffLine, SETUP_STEP_WORDS, PLACE_KEY_REFUSAL, PLACE_CODE_REFUSAL } from "@wsp/protocol";

const h = await import("../../../../packages/runtime/src/places/helpers.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/places_b_views.json", import.meta.url));

it("writes the places b views fixture", () => {
  const capacity = { cores: 4, memMb: 8192, memRoomMb: 16384, machineMemMb: 4096, diskFreeBytes: 10 * 1024 ** 3, images: [{ id: "i1", sizeBytes: 2 * 1024 ** 3 }, { id: "i2", sizeBytes: 3 * 1024 ** 3 }], machines: { running: 2, paused: 1 } };
  writeFileSync(
    out,
    `${JSON.stringify(
      {
        blocked: placeBlocked("old-macbook", { runsWorkspaces: false, workspacesBlocked: "docker is not running" }),
        homeRefusal: h.placeHomeRefusal("relative"),
        floorFailed: floorFailedLine([{ label: "git" }, { label: "curl" }]),
        floorWord: SETUP_STEP_WORDS.floor,
        pluginOff: pluginOffLine("lint"),
        keyRefusal: PLACE_KEY_REFUSAL,
        codeRefusal: PLACE_CODE_REFUSAL,
        addStopped: h.ADD_STOPPED_LINE,
        addStoppedFix: h.ADD_STOPPED_FIX,
        addNotTakenBack: h.ADD_NOT_TAKEN_BACK_LINE,
        capacity,
        room: forkRoom(capacity, Math.min(4096, capacity.machineMemMb), 3 * 1024 ** 3),
        roomNoImage: forkRoom({ ...capacity, images: [] } as never, 4096, undefined),
      },
      null,
      2,
    )}\n`,
  );
});
