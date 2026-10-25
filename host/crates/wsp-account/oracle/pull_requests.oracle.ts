// SPDX-License-Identifier: AGPL-3.0-only
// The TS answers the pull requests tests compare to: the sentences pullRequestsArea (packages/runtime/src/account/
// pull-requests.ts at 65f6b89c7) builds from @wsp/protocol and its own templates, taken by running the area on a
// minimal context: the stopped and unread lines a read that found no command line leaves on the status, the refusal of
// a workspace with no pull request, the post line for each shape of remote (which is where hostOfRemote shows), and the
// two warnings settleTree prints. Written as tests/fixtures/oracle/pull_requests.json. Run from the repository root:
//   pnpm exec vitest run --config host/crates/wsp-account/oracle/vitest.config.ts --minWorkers=1 --maxWorkers=1 pull_requests.oracle.ts
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { expect, it, vi } from "vitest";
import { gitHostOf, remoteHost } from "@wsp/catalog";
import { pullRequestPostLine, pullRequestStoppedLine, pullRequestUnreadLine, noPullRequestRefusal } from "@wsp/protocol";

const { pullRequestsArea } = await import("../../../../packages/runtime/src/account/pull-requests.ts");
const { DaemonRefusal } = await import("../../../../packages/runtime/src/types/internal.ts");
const out = fileURLToPath(new URL("../tests/fixtures/oracle/pull_requests.json", import.meta.url));

const REMOTE = "https://github.com/wsp/pr-lab.git";
const noHostCli = () => new DaemonRefusal("no-host-cli", "no signed-in command line for github.com is on this computer");

/** One area on a context holding only what the paths run here read. */
function build(o: { record: Record<string, unknown>; remote?: string; onThis?: () => Promise<unknown>; withDaemon?: () => Promise<unknown>; mark?: () => Promise<void>; napWith?: () => Promise<void>; rows?: unknown[] }) {
  const cache = new Map<string, unknown>();
  const live = new Map<string, unknown>();
  const entry = { record: o.record, checkout: { branch: "fix/ci" } } as Record<string, unknown>;
  live.set(String(o.record["id"]), entry);
  const ctx = {
    clock: { now: () => 1000, schedule: () => () => {} },
    githubCache: { get: (k: string) => cache.get(k), set: (k: string, v: unknown) => void cache.set(k, v), delete: (k: string) => void cache.delete(k) },
    live,
    sessions: new Map((o.rows ?? []).map((view, i) => [`s${i}`, { view }])),
    status: { watched: () => false },
    slates: { watchesPr: () => false },
    projectHeld: () => ({ id: "p1", remote: o.remote ?? REMOTE, base: "main", defaultBranch: "main" }),
    readCheckout: async () => entry["checkout"],
    onThisComputer: o.onThis ?? (async () => ({})),
    withDaemon: o.withDaemon ?? (async () => ({})),
    checkoutOf: () => "/root/stub-1",
    statusNow: async () => {},
    persist: async () => {},
    entryOf: async () => entry,
    mark: o.mark ?? (async () => {}),
    pauses: () => true,
    napWith: o.napWith ?? (async () => {}),
  };
  return { area: pullRequestsArea(ctx as never), entry };
}

const caught = async (f: () => Promise<unknown>): Promise<{ message: string; fix?: string; kind?: string }> => {
  try {
    await f();
  } catch (e) {
    const err = e as Error & { fix?: string; kind?: string };
    return { message: err.message, ...(err.fix !== undefined ? { fix: err.fix } : {}), ...(err.kind !== undefined ? { kind: err.kind } : {}) };
  }
  throw new Error("nothing was thrown");
};

it("writes the pull requests oracle", async () => {
  const cloud = (phase: string) => ({ id: "w1", name: "pr-lab", kind: "cloud", project: "p1", machineId: "m1", phase });

  // A read of a stopped copy, and of a running one, with no command line on this computer or the copy.
  const stopped = build({ record: cloud("napping"), onThis: async () => Promise.reject(noHostCli()) });
  await stopped.area.readPullRequest(stopped.entry as never, true);
  const unread = build({ record: cloud("running"), onThis: async () => Promise.reject(noHostCli()), withDaemon: async () => Promise.reject(noHostCli()) });
  await unread.area.readPullRequest(unread.entry as never, true);
  const whyOf = (e: Record<string, unknown>) => (e["pr"] as { why: string }).why;
  expect(whyOf(stopped.entry)).toBe(pullRequestStoppedLine("github.com", "pr-lab"));
  expect(whyOf(unread.entry)).toBe(pullRequestUnreadLine("github.com"));

  // A workspace with no pull request is refused by name.
  const bare = build({ record: cloud("running") });
  const noPullRequest = await caught(() => bare.area.pullRequestOn("w1", undefined));
  expect(noPullRequest.message).toBe(noPullRequestRefusal("pr-lab"));

  // The post line for each shape of remote: the host it names is the git host's first ssh host, else the remote's own.
  const remotes = [
    "https://github.com/wsp/pr-lab.git",
    "git@github.com:wsp/pr-lab.git",
    "ssh://git@ssh.github.com/wsp/pr-lab.git",
    "HTTPS://GitHub.com/wsp/pr-lab",
    "https://gitlab.com/wsp/pr-lab.git",
    "git@altssh.gitlab.com:wsp/pr-lab.git",
    "https://git.example.com/wsp/pr-lab.git",
    "git@git.example.com:wsp/pr-lab.git",
    "https://Code.Example.ORG:8443/wsp/pr-lab",
    "not a url",
    "",
  ];
  const posts: { remote: string; host: string; line: string }[] = [];
  for (const remote of remotes) {
    const { area } = build({ record: cloud("running"), remote, onThis: async () => Promise.reject(noHostCli()) });
    const line = (await caught(() => area.postAsPerson(remote, 12, cwd => ({ op: "git.prReact", cwd, remote, number: 12 }) as never, r => r))).message;
    const host = gitHostOf(remote)?.sshHosts[0] ?? remoteHost(remote) ?? remote;
    expect(line).toBe(pullRequestPostLine(host));
    posts.push({ remote, host, line });
  }

  // The two warnings a settled tree prints when its mark or its nap fails.
  const warn = vi.spyOn(console, "warn").mockImplementation(() => {});
  const rows = [{ id: "s0", workspaceId: "w1", threadId: "tA", status: "completed" }];
  const failing = build({ record: { ...cloud("running"), pr: { number: 12, state: "merged" } }, rows, mark: async () => Promise.reject(new Error("boom")), napWith: async () => Promise.reject(new Error("no machine")) });
  await failing.area.settleTree(failing.entry as never);
  await new Promise(r => setTimeout(r, 0));
  const warns = warn.mock.calls.map(c => String(c[0]));
  warn.mockRestore();
  expect(warns).toHaveLength(2);

  writeFileSync(out, `${JSON.stringify({ stopped: whyOf(stopped.entry), unread: whyOf(unread.entry), noPullRequest, posts, warns }, null, 1)}\n`);
});
