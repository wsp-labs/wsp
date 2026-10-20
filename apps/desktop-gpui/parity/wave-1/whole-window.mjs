// SPDX-License-Identifier: AGPL-3.0-only
// Whole-window states: the New thread page (on a project with threads and on one with none) and a
// computer named apart from its hostname in the tiles, the hover card and the composer's footer. Each is driven as the
// sweep drives it (../wave-sweep/drives.mjs), on a fixture with its real host shape.
import { drives, withIds } from "../wave-sweep/drives.mjs";

export default [
  { id: "W1-FRM-195", data: withIds(), go: drives.link("workspace", d => d.bench) },
  { id: "W1-THR-195", data: withIds(), go: drives.newThread },
  { id: "W1-THR-196", data: withIds(), go: drives.link("project", d => d.quiet) },
  { id: "W1-SBR-172", data: withIds(), go: async (app, data) => (await app.route.thread(data.panes), await app.park()) },
  { id: "W1-SBR-173", data: withIds(), go: drives.hover },
];
