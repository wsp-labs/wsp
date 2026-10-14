// SPDX-License-Identifier: AGPL-3.0-only
import { describe, expect, it } from "vitest";
import {
  BranchOfRecord,
  BrokerStatus,
  DaemonAuthRequest,
  DaemonRequest,
  EventUnion,
  pipeOpenOf,
  PIPE_DAEMON_VERSION,
  DAEMON_VERSION,
  DEFAULT_PREFERENCES,
  applyPreferencesPatch,
  PreferencesPatch,
  ProjectOverrides,
  ProjectView,
  ProviderKeyWorks,
  SealedImage,
  RUNTIME_OPS,
  RuntimeRequest,
  SessionEvent,
  THREAD_OPS,
  DEVICE_OPS,
  WorkspaceView,
} from "../src/index.js";

const project = {
  id: "p_1",
  name: "lab",
  computer: "boat",
  source: { kind: "github", repo: "acme/lab" },
  path: "/root/lab",
  remote: "https://github.com/acme/lab.git",
  defaultBranch: "main",
  memoryKey: "-root-lab",
  memoryDir: "/root/.wsp/memory/lab",
  createdAt: "2026-10-13T00:00:00.000Z",
};

const workspace = {
  id: "ws_1",
  name: "fix-login",
  machineId: "m_1",
  phase: "running",
  kind: "cloud",
  golden: "snap_1",
  createdAt: "2026-10-13T00:00:00.000Z",
  project: { id: "p_1", name: "lab", path: "/root/lab", computer: "boat" },
};

describe("the credential contracts", () => {
  it("a pipe's auth frame and its pipe.open parse, and a frame no daemon of pipes sends reads as no pipe", () => {
    expect(DaemonAuthRequest.parse({ id: 0, op: "auth", token: "t", pipe: true })).toEqual({ id: 0, op: "auth", token: "t", pipe: true });
    expect(pipeOpenOf(JSON.stringify({ op: "pipe.open", door: "gh", heldMs: 840_000 }))).toEqual({ op: "pipe.open", door: "gh", heldMs: 840_000 });
    // What daemon 157 sends a pipe auth after its auth reply, recorded off the binary v0.3.6 deploys: a control socket's hello.
    expect(pipeOpenOf(`{"type":"daemon.hello","root":"/root","version":157}`)).toBeUndefined();
    expect(pipeOpenOf("not json")).toBeUndefined();
    expect(pipeOpenOf(JSON.stringify({ op: "pipe.open", door: "ssh", heldMs: 0 }))).toBeUndefined();
  });

  it("each of the six git ops takes an op token, and an op frame without one parses as it always has", () => {
    for (const op of ["git.push", "git.startOn", "git.update", "git.mergeIn", "git.pr", "git.prCheckout"]) {
      const frame = { id: 1, op, cwd: "/root/lab", branch: "wsp/fix-login", number: 7 };
      expect((DaemonRequest.parse({ ...frame, token: "op-1" }) as { token?: string }).token, op).toBe("op-1");
      expect((DaemonRequest.parse(frame) as { token?: string }).token, op).toBeUndefined();
    }
  });

  it("the gate is a daemon this tree ships or one before it", () => {
    expect(PIPE_DAEMON_VERSION).toBeLessThanOrEqual(DAEMON_VERSION);
  });

  it("the three runtime ops parse, none on a thread's token, and only broker.status on a paired device", () => {
    expect(RuntimeRequest.parse({ id: 1, op: "providers.checkKey", provider: "boat", key: "k" })).toMatchObject({ op: "providers.checkKey" });
    expect(RuntimeRequest.parse({ id: 2, op: "agents.cloudSignIn", agent: "codex" })).toMatchObject({ op: "agents.cloudSignIn" });
    expect(RuntimeRequest.parse({ id: 3, op: "broker.status" })).toMatchObject({ op: "broker.status" });
    expect(RuntimeRequest.parse({ id: 4, op: "agents.cloudSignIn", agent: "claude" })).toMatchObject({ agent: "claude" });
    expect(() => RuntimeRequest.parse({ id: 4, op: "agents.cloudSignIn", agent: "gemini" })).toThrow();
    expect(() => RuntimeRequest.parse({ id: 5, op: "providers.checkKey", provider: "boat", key: "" })).toThrow();
    for (const op of ["providers.checkKey", "agents.cloudSignIn", "broker.status"]) {
      expect(RUNTIME_OPS).toContain(op);
      expect(THREAD_OPS).not.toContain(op);
      expect(DEVICE_OPS.includes(op), op).toBe(op === "broker.status");
    }
  });

  it("the answers of checkKey and broker.status parse", () => {
    expect(ProviderKeyWorks.parse({ works: true, account: "acme", plan: "team", running: 2, limit: 10 }).limit).toBe(10);
    expect(() => ProviderKeyWorks.parse({ works: false, account: "acme", plan: "team", running: 2, limit: 10 })).toThrow();
    expect(BrokerStatus.parse({ claude: "token", codex: "access-token", github: "octocat" })).toEqual({ claude: "token", codex: "access-token", github: "octocat" });
    expect(BrokerStatus.parse({ claude: "login" }).claude).toBe("login");
    expect(BrokerStatus.parse({})).toEqual({});
    expect(() => BrokerStatus.parse({ claude: "keychain" })).toThrow();
  });

  it("thread.waited is a stored event and a pushed one", () => {
    const waited = { type: "thread.waited", workspaceId: "ws_1", sessionId: "s_1", threadId: "t_1", ms: 840_000, said: "your computer was away" };
    expect(SessionEvent.parse(waited)).toEqual(waited);
    expect(EventUnion.parse({ ...waited, seq: 4 })).toMatchObject({ type: "thread.waited", ms: 840_000 });
    expect(() => SessionEvent.parse({ ...waited, threadId: undefined })).toThrow();
    expect(() => SessionEvent.parse({ ...waited, ms: -1 })).toThrow();
  });

  it("a workspace's branch of record is optional and parses when present", () => {
    expect(WorkspaceView.parse(workspace).branchOfRecord).toBeUndefined();
    const branch = { repoId: 1_296_269, owner: "acme", name: "lab", branch: "wsp/fix-login" };
    expect(WorkspaceView.parse({ ...workspace, branchOfRecord: branch }).branchOfRecord).toEqual(branch);
    expect(() => BranchOfRecord.parse({ ...branch, repoId: 0 })).toThrow();
    expect(BranchOfRecord.parse({ ...branch, startSha: "4b825dc6" }).startSha).toBe("4b825dc6");
    expect(ProjectView.parse({ ...project, trackers: ["acme/issues"] })).not.toHaveProperty("trackers");
  });

  it("a project's trackers live in its preferences, set, kept and taken away by a patch", () => {
    expect(ProjectOverrides.parse({ trackers: ["acme/issues"] })).toEqual({ trackers: ["acme/issues"] });
    expect(() => ProjectOverrides.parse({ trackers: ["issues"] })).toThrow();
    const set = applyPreferencesPatch(DEFAULT_PREFERENCES, PreferencesPatch.parse({ projectDefaults: { p_1: { trackers: ["acme/issues"], agent: "claude" } } }));
    expect(set.projectDefaults["p_1"]).toEqual({ trackers: ["acme/issues"], agent: "claude" });
    const cleared = applyPreferencesPatch(set, PreferencesPatch.parse({ projectDefaults: { p_1: { trackers: null } } }));
    expect(cleared.projectDefaults["p_1"]).toEqual({ agent: "claude" });
  });

  it("the cloud card's dismissal is kept by a patch and stays through one that leaves it out", () => {
    const dismissed = applyPreferencesPatch(DEFAULT_PREFERENCES, PreferencesPatch.parse({ cloudCardDismissed: true }));
    expect(dismissed.cloudCardDismissed).toBe(true);
    expect(applyPreferencesPatch(dismissed, PreferencesPatch.parse({ theme: "dark" })).cloudCardDismissed).toBe(true);
    expect(DEFAULT_PREFERENCES.cloudCardDismissed).toBeUndefined();
  });

  it("an image's noSecrets mark is optional", () => {
    const image = SealedImage.shape.noSecrets;
    expect(image.parse(true)).toBe(true);
    expect(image.parse(undefined)).toBeUndefined();
  });
});
