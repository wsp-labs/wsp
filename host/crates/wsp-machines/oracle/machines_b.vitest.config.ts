// SPDX-License-Identifier: AGPL-3.0-only
// Runs machines-b's oracle under the repository's own node project, so @wsp/protocol resolves as the TS tests resolve it.
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import workspace from "../../../../vitest.workspace.js";

const node = (workspace as { resolve: unknown; plugins: unknown }[])[0]!;

export default defineConfig({
  root: fileURLToPath(new URL("../../../../", import.meta.url)),
  resolve: node.resolve as never,
  plugins: node.plugins as never,
  test: { include: ["host/crates/wsp-machines/oracle/machines_b*.oracle.ts"], environment: "node" },
});
