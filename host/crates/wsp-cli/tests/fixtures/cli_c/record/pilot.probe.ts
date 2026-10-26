import { it } from "vitest";
import { record, recorded, type Case, type Run } from "./lib.ts";
const stopped = recorded("stop", "stopped with the tree");
const cases: Case[] = [
  { fn: "threadOf", case: "by id prefix", args: { ref: "thread-7f" }, script: { replies: stopped } },
  { fn: "threadOf", case: "none", args: { ref: "zzz" }, script: { replies: stopped } },
  { fn: "threadOf", case: "none cloud", args: { ref: "zzz" }, cloud: true, script: { replies: stopped } },
];
const runs: Record<string, Run> = {
  threadOf: ({ mod, client, args }) => (mod["workspaces"] as any).threadOf(client, args["ref"]),
};
it("pilot", async () => {
  await record("pilot", cases, runs);
});
