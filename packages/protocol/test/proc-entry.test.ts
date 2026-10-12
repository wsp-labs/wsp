// SPDX-License-Identifier: AGPL-3.0-only
// A process row from a daemon a version ahead still parses: a kind or an owner rung this version does not know reads
// as other or as no owner, since a row that fails its schema drops the whole frame it rides in.
import { describe, expect, it } from "vitest";
import { ProcEntry } from "../src/index.js";

const row = { pid: 7, ppid: 1, user: "tester", state: "S", comm: "vite", cmdline: "node vite", cpu: 0, rss: 1, startedAt: 1 };

describe("a process row from a newer daemon", () => {
  it("reads a kind it does not know as other and an owner rung it does not know as no owner", () => {
    const parsed = ProcEntry.parse({ ...row, kind: "container", owner: { by: "docker", thread: "t1" } });
    expect(parsed.kind).toBe("other");
    expect(parsed.owner).toBeUndefined();
  });

  it("keeps what it knows, a warm agent's owner included", () => {
    const owner = { by: "env", thread: "t1", workspace: "ws_1", warm: true };
    expect(ProcEntry.parse({ ...row, kind: "slate", owner })).toMatchObject({ kind: "slate", owner });
  });
});
