// SPDX-License-Identifier: AGPL-3.0-only
// The node project with the tap on: every host a case serves writes its sockets' frames into PARITY_TAPE_DIR, each
// tape named for the case it came from. The variable is not a WSP_ one, since the root test environment drops those
// before a worker starts; the project hands it to the workers under the name the tap reads.
import { fileURLToPath } from "node:url";
import workspace from "../../vitest.workspace.js";

const dir = process.env["PARITY_TAPE_DIR"];
if (dir === undefined || dir === "") throw new Error("PARITY_TAPE_DIR names no folder to write tapes into");

type Project = { test?: { name?: string; include?: string[]; env?: Record<string, string>; setupFiles?: string[] } };
const node = (workspace as Project[]).find(p => typeof p === "object" && p.test?.name === "node")!;

export default [
  {
    ...node,
    test: {
      ...node.test,
      include: [...(node.test?.include ?? []), "parity/record/*.record.ts"],
      env: { ...node.test?.env, WSP_TAPE_DIR: dir },
      setupFiles: [...(node.test?.setupFiles ?? []), fileURLToPath(new URL("./tape-source.ts", import.meta.url))],
    },
  },
];
