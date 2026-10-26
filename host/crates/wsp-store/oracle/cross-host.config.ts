// SPDX-License-Identifier: AGPL-3.0-only
// Runs cross-host.step.ts under the repository's own node project, as vitest.config.ts runs the oracles.
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import workspace from "../../../../vitest.workspace.js";

const node = (workspace as { resolve: unknown; plugins: unknown }[])[0]!;

export default defineConfig({
  root: fileURLToPath(new URL("../../../../", import.meta.url)),
  resolve: node.resolve as never,
  plugins: node.plugins as never,
  test: { include: ["host/crates/wsp-store/oracle/cross-host.step.ts"], environment: "node" },
});
