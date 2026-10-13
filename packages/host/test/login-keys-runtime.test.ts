// SPDX-License-Identifier: AGPL-3.0-only
// The Claude key a person exports in their login shell's rc file and the turns
// on this computer: a host launchd started carries it to a thread as it carries
// the PATH, and says it nowhere it writes or sends.
//
// Its own file: the shell is read once per process, so a case sharing a file
// with another host start would read what that start already took.
import { execFileSync, spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { homedir, tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { HARNESS_ADAPTERS, type HarnessAdapterFactory } from "@wsp/runtime";
import { cli, localWiring, makeRuntime, serve } from "../src/cli.js";
import { noClaudeKeyNote } from "../src/cli/serve.js";
import { LAUNCHD_PATH } from "../src/login-path.js";
import type { HostHandle } from "../src/server.js";
import { dialHost, type HostClient } from "../src/verbs/client.js";
import { PAGE, captured, copyingFake, doneOnlyAgent, fakeDaemonStart, threadOn } from "./verbs-fixture.js";
import { writeStub } from "../../protocol/test/stub-script.js";

const KEY = "sk-ant-x-login-shell-fake-key";

/** What the turn reports instead of the key, so the frames carrying its output carry no key either. */
const cksum = (value: string): string => spawnSync("cksum", { input: value, encoding: "utf8" }).stdout.trim();

describe("the Claude key in the login shell and a thread on this computer", () => {
  let dir: string;
  let statePath: string;
  let handle: HostHandle | undefined;
  let client: HostClient | undefined;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "wsp-login-keys-"));
    const webDir = join(dir, "web");
    mkdirSync(join(webDir, "assets"), { recursive: true });
    writeFileSync(join(webDir, "assets", "app.js"), "console.log('app')\n");
    writeFileSync(join(webDir, "index.html"), PAGE);
    statePath = join(dir, "home", "state.json");
    mkdirSync(join(dir, "home"), { recursive: true });
    // A login shell whose rc file put a folder of the person's in front and exported the key, then ran what it was
    // asked to, as `$SHELL -ilc <script>` does.
    const shell = join(dir, "login-shell");
    writeStub(shell, `#!/bin/sh\nexport PATH=${JSON.stringify(`${join(dir, "their-bin")}:${LAUNCHD_PATH.join(":")}`)}\nexport ANTHROPIC_API_KEY=${KEY}\nexec /bin/sh -c "$2"\n`);
    vi.stubEnv("SOLARI_API_KEY", "");
    vi.stubEnv("ANTHROPIC_API_KEY", "");
    vi.stubEnv("HOME", join(dir, "user"));
    vi.stubEnv("WSP_HOME", join(dir, "home"));
    vi.stubEnv("SHELL", shell);
    vi.stubEnv("PATH", LAUNCHD_PATH.join(":"));
  });
  afterEach(async () => {
    client?.close();
    client = undefined;
    await handle?.close();
    handle = undefined;
    vi.unstubAllEnvs();
    rmSync(dir, { recursive: true, force: true });
  });

  it("reaches the turn's environment, and no log line, frame or state file holds it", async () => {
    const io = captured();
    const turn = doneOnlyAgent(() => "done").adapter;
    const claude: HarnessAdapterFactory = c => Object.assign(HARNESS_ADAPTERS["claude"]!(c), { start: turn(c).start, steers: false });
    const built = makeRuntime({}, statePath, undefined, process.env, undefined, localWiring(homedir(), process.env, fakeDaemonStart, statePath, copyingFake()), undefined, undefined, { ...HARNESS_ADAPTERS, claude });
    // The copy of the environment a command line parses before the login shell is read, as wsp up hands it on.
    handle = await serve(io, { port: 0, statePath, webDir: join(dir, "web"), runtime: built, providerEnv: { ...process.env } });

    const frames: string[] = [];
    client = await dialHost(statePath, { aim: { kind: "here" } });
    client.onFrame(frame => frames.push(JSON.stringify(frame)));
    await client.events();

    const folder = mkdtempSync(join(dir, "repo-"));
    execFileSync("git", ["init", "-q", folder]);
    const project = await handle.addProject(folder);
    const workspace = await handle.createWorkspace(project.name, undefined, project.id);
    const thread = await threadOn(built, workspace.id);

    // exec runs under claude's own launch environment, the one a turn on this computer starts with.
    const ran = captured();
    expect(await cli(["exec", "--state", statePath, thread, "--", "sh", "-c", 'printf %s "$ANTHROPIC_API_KEY" | cksum'], ran)).toBe(0);
    expect(ran.lines.join("").trim()).toBe(cksum(KEY));

    expect(io.lines.filter(line => line === noClaudeKeyNote(true) || line === noClaudeKeyNote(false))).toEqual([]);
    expect([...io.lines, ...io.errors, ...ran.lines, ...ran.errors].join("\n")).not.toContain(KEY);
    expect(frames.length).toBeGreaterThan(0);
    expect(frames.join("\n")).not.toContain(KEY);
    // Whichever store holds the state, every file the host wrote beside it, the database's pages among them.
    const written = readdirSync(dirname(statePath), { recursive: true, encoding: "utf8" }).map(f => join(dirname(statePath), f)).filter(f => statSync(f).isFile());
    expect(written.length).toBeGreaterThan(0);
    for (const file of written) expect(readFileSync(file, "latin1"), file).not.toContain(KEY);
  });
});
