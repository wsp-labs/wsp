// SPDX-License-Identifier: AGPL-3.0-only
// A project here whose Claude Code is older than the one wsp's table was read
// off: the lists a new thread's composer reads for the project are that
// binary's own, and a start that names a model it does not run is refused
// naming its version and the update, never run on another model.
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, describe, expect, it } from "vitest";
import { LocalBackend } from "@wsp/engine";
import { everyModel, markedDefault } from "@wsp/protocol";
import { HARNESS_ADAPTERS } from "../src/adapters.js";
import { harnessCatalog } from "../src/harness-catalog.js";
import { createRuntime, type LocalWiring, type Runtime } from "../src/runtime.js";
import { localExecStream } from "../src/local-exec.js";
import { memoryStore } from "../src/store.js";
import { gitCopier } from "./git-copier.js";
import { writeStub } from "../../protocol/test/stub-script.js";
import { stubBackend, testPlatform } from "./stub-backend.js";

/** The models 2.1.278's initialize handshake lists, as the binary answered it on 2026-10-13. */
const MODELS_2_1_278 = [
  { value: "default", resolvedModel: "claude-opus-5[1m]", displayName: "Default (recommended)", supportedEffortLevels: ["low", "medium", "high", "xhigh", "max"] },
  { value: "opus[1m]", resolvedModel: "claude-opus-5[1m]", displayName: "Opus (1M context)", supportedEffortLevels: ["low", "medium", "high", "xhigh", "max"] },
  { value: "claude-fable-5-1", resolvedModel: "claude-fable-5-1", displayName: "Fable", supportedEffortLevels: ["low", "medium", "high", "xhigh", "max"] },
  { value: "sonnet", resolvedModel: "claude-sonnet-5", displayName: "Sonnet", supportedEffortLevels: ["low", "medium", "high", "xhigh", "max"] },
  { value: "haiku", resolvedModel: "claude-haiku-4-5-20251001", displayName: "Haiku" },
];
/** A release newer than wsp's table that no longer lists Fable 5.1, the way 2.1.296 dropped models 2.1.278 had. */
const MODELS_2_1_310 = [
  { value: "default", resolvedModel: "claude-opus-5-5", displayName: "Default (recommended)", supportedEffortLevels: ["low", "medium", "high", "xhigh", "max"] },
  { value: "opus", resolvedModel: "claude-opus-5-5", displayName: "Opus", supportedEffortLevels: ["low", "medium", "high", "xhigh", "max"] },
  { value: "sonnet", resolvedModel: "claude-sonnet-5-5", displayName: "Sonnet", supportedEffortLevels: ["low", "medium", "high", "xhigh", "max"] },
  { value: "haiku", resolvedModel: "claude-haiku-5-5", displayName: "Haiku", supportedEffortLevels: ["low", "medium", "high", "xhigh", "max"] },
];
const handshake = (models: readonly object[]): string => JSON.stringify({ type: "control_response", response: { subtype: "success", request_id: "init", response: { models } } });

/** Claude Code as wsp asks it: its version, its flags, its handshake, and a turn on Opus 5.5 refused the way the API
 * refuses 2.1.278. */
const claudeAt = (version: string, models: readonly object[]): string => `#!/bin/sh
case "$1" in
  --version) echo "${version} (Claude Code)"; exit 0 ;;
  --help) echo '  --permission-mode <mode>  Permission mode (choices: "acceptEdits", "auto", "bypassPermissions", "manual", "dontAsk", "plan")'; exit 0 ;;
esac
for a in "$@"; do
  if [ "$a" = "--bare" ]; then cat > /dev/null; echo '${handshake(models)}'; exit 0; fi
done
prev=""
for a in "$@"; do
  if [ "$prev" = "--model" ]; then
    case "$a" in claude-opus-5-5*) echo "API Error: 400 Claude Code ${version} does not support this model; version 2.1.280 or newer is required." >&2; exit 1 ;; esac
  fi
  prev="$a"
done
exit 1
`;

const roots: string[] = [];
const scratch = (): string => {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "wsp-older-agent-")));
  roots.push(root);
  return root;
};
let rt: Runtime | undefined;
afterEach(async () => {
  await rt?.close();
  rt = undefined;
  for (const at of roots.splice(0)) rmSync(at, { recursive: true, force: true });
});

function here(claude = claudeAt("2.1.278", MODELS_2_1_278)): Runtime {
  const root = scratch();
  const bin = join(root, "bin");
  mkdirSync(bin);
  writeStub(join(bin, "claude"), claude);
  mkdirSync(join(root, "state"));
  const local: LocalWiring = {
    backend: new LocalBackend({ root }),
    execStream: o => localExecStream({ root, runDir: join(root, "runs"), ...o }),
    home: () => join(root, ".claude"),
    homeDir: root,
    rootsPath: join(root, "roots"),
    env: () => ({ PATH: `${bin}:/usr/bin:/bin` }),
    platform: testPlatform(),
    copier: gitCopier(),
    daemonRoad: async () => ({ url: "http://127.0.0.1:1", expiresAt: Number.MAX_SAFE_INTEGER, daemonToken: "t" }),
  };
  rt = createRuntime({ backend: stubBackend(), store: memoryStore(), adapters: { claude: HARNESS_ADAPTERS.claude }, local, statePath: join(root, "state", "state.json") });
  return rt;
}

const project = async (runtime: Runtime) => {
  const at = scratch();
  execFileSync("git", ["-C", at, "init", "-q", "-b", "main"]);
  return runtime.projects.add({ source: at });
};

describe("a project here on a Claude Code older than wsp's table", () => {
  it("lists for its new thread the models that binary runs, with its own default, not the table's", async () => {
    const runtime = here();
    const p = await project(runtime);
    // The table offers Opus 5.5 as the default, which this binary cannot run.
    expect(markedDefault(harnessCatalog("claude")!.models)?.value).toBe("claude-opus-5-5");
    const claude = (await runtime.harnesses.list(undefined, undefined, { project: p.id })).find(c => c.harness === "claude")!;
    expect(claude).toMatchObject({ source: "harness", version: "2.1.278" });
    expect(markedDefault(claude.models)?.value).toBe("claude-opus-5");
    expect(everyModel(claude).map(m => m.value)).not.toContain("claude-opus-5-5");
  });

  it("refuses a start that names Opus 5.5, naming the version there and the update, before a folder is made", async () => {
    const runtime = here();
    const p = await project(runtime);
    const refused = runtime.workspaces.folderFor({ project: p.id, picks: { harness: "claude", model: "claude-opus-5-5" } });
    await expect(refused).rejects.toThrow("Claude Code 2.1.278 does not run Opus 5.5; a newer Claude Code does. Update it with claude update, or pick a model it lists.");
  });

  it("refuses a send into its folder that names Opus 5.5 the same way, and runs no turn on another model", async () => {
    const runtime = here();
    const p = await project(runtime);
    const at = await runtime.workspaces.folderFor({ project: p.id });
    await expect(runtime.sessions.start(at.workspace.id, { prompt: "hi", harness: "claude", model: "claude-opus-5-5" })).rejects.toThrow(/^Claude Code 2\.1\.278 does not run Opus 5\.5; a newer Claude Code does\. Update it with claude update/);
    expect(await runtime.sessions.list(at.workspace.id)).toEqual([]);
  });

  it("says what new threads start on for each project, and lists its computer's models, off that computer's binary", async () => {
    const runtime = here();
    const p = await project(runtime);
    await runtime.preferences.set({ agentDefaults: { claude: { model: "claude-opus-5-5" } } });
    expect((await runtime.projects.defaults())[p.id]?.model).toEqual({ value: "claude-opus-5", from: "catalog" });
    const claude = (await runtime.harnesses.list(undefined, undefined, { computer: p.computer })).find(c => c.harness === "claude")!;
    expect(claude).toMatchObject({ source: "harness", version: "2.1.278" });
    expect(everyModel(claude).map(m => m.value)).not.toContain("claude-opus-5-5");
  });
});

describe("a project here on a Claude Code newer than wsp's table", () => {
  it("refuses a model the binary dropped by its own version, telling the person to pick one it lists and not to update", async () => {
    const runtime = here(claudeAt("2.1.310", MODELS_2_1_310));
    const p = await project(runtime);
    const refused = runtime.workspaces.folderFor({ project: p.id, picks: { harness: "claude", model: "claude-fable-5-1" } });
    await expect(refused).rejects.toThrow(
      "Claude Code 2.1.310 does not run Fable 5.1; it lists Opus 5.5 (claude-opus-5-5), Sonnet 5.5 (claude-sonnet-5-5), Haiku 5.5 (claude-haiku-5-5). Pick a model it lists.",
    );
  });
});
