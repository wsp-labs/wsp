// SPDX-License-Identifier: AGPL-3.0-only
// Runs the oracle under the repository's own node project, so @wsp/protocol resolves as the TS tests resolve it.
import { fileURLToPath } from "node:url";
import { defineConfig } from "vitest/config";
import workspace from "../../../../vitest.workspace.js";

const node = (workspace as { resolve: unknown; plugins: unknown }[])[0]!;

export default defineConfig({
  root: fileURLToPath(new URL("../../../../", import.meta.url)),
  resolve: node.resolve as never,
  plugins: node.plugins as never,
  // The repository config strips every WSP_ variable from the shell, so the cloud run is asked for under its own name.
  test: { include: ["host/crates/wsp-cli/oracle/*.oracle.ts"], environment: "node", env: process.env["ORACLE_CLOUD"] === "1" ? { WSP_CLOUD: "1" } : {} },
});
