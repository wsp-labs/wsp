// SPDX-License-Identifier: AGPL-3.0-only
import { spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DOCKER_NOTES_CAP_BYTES, dockerShim } from "../src/docker-shim.js";
import { writeStub } from "./stub-script.js";

/** What the docker after the shim was handed: every word, and the variables compose and the shim read. */
type Seen = { argv: string[]; project?: string; passed?: string };

const REAL = `#!${process.execPath}
process.stdout.write(JSON.stringify({ argv: process.argv.slice(2), env: process.env }));
process.exit(7);
`;

describe("the docker a thread finds first on its PATH", () => {
  let dir: string;
  let bin: string;
  let real: string;
  let notes: string;
  let folder: string;
  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), "wsp-docker-shim-"));
    bin = join(dir, "thread-bin");
    real = join(dir, "usr bin");
    notes = join(dir, "docker-notes");
    folder = join(dir, "acme", "lab");
    for (const d of [bin, real, folder]) mkdirSync(d, { recursive: true });
    // A shell started in a folder with no PWD reads the folder's real path: /private/var on a Mac.
    folder = realpathSync(folder);
    writeStub(join(bin, "docker"), dockerShim(bin, notes));
    writeStub(join(real, "docker"), REAL);
  });
  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  const run = (args: string[], env: Record<string, string> = {}, path = `${bin}:${real}:/usr/bin:/bin`): { seen: Seen; status: number | null; stderr: string } => {
    const r = spawnSync(join(bin, "docker"), args, { env: { PATH: path, ...env }, cwd: folder, encoding: "utf8" });
    if (r.stdout === "") return { seen: { argv: [] }, status: r.status, stderr: r.stderr };
    const { argv, env: theirs } = JSON.parse(r.stdout) as { argv: string[]; env: Record<string, string> };
    return { seen: { argv, project: theirs["COMPOSE_PROJECT_NAME"], passed: theirs["WSP_DOCKER_PASSED"] }, status: r.status, stderr: r.stderr };
  };
  const inThread = (args: string[], env: Record<string, string> = {}): ReturnType<typeof run> => run(args, { WSP_THREAD: "t_1f0c-ab", ...env });
  const label = ["--label", "wsp.thread=t_1f0c-ab"];
  const notesOf = (thread: string): string[][] => readFileSync(join(notes, thread), "utf8").trimEnd().split("\n").map(l => l.split("\t"));

  it("labels run, create, container run and create and compose run, after any global flag, and hands back the real exit code", () => {
    const cases: [string[], string[]][] = [
      [["run", "-d", "--name", "x", "busybox", "sleep", "60"], ["run", ...label, "-d", "--name", "x", "busybox", "sleep", "60"]],
      [["create", "busybox"], ["create", ...label, "busybox"]],
      [["container", "run", "busybox"], ["container", "run", ...label, "busybox"]],
      [["container", "create", "busybox"], ["container", "create", ...label, "busybox"]],
      [["compose", "run", "web"], ["compose", "run", ...label, "web"]],
      [["compose", "-f", "a b.yaml", "--project-directory", "run", "run", "web"], ["compose", "-f", "a b.yaml", "--project-directory", "run", "run", ...label, "web"]],
      [["compose", "--workdir", "run", "run", "web"], ["compose", "--workdir", "run", "run", ...label, "web"]],
      [["--log-level", "warn", "run", "busybox"], ["--log-level", "warn", "run", ...label, "busybox"]],
      [["-l", "warn", "-H", "unix:///var/run/docker.sock", "run", "busybox"], ["-l", "warn", "-H", "unix:///var/run/docker.sock", "run", ...label, "busybox"]],
      [["--context", "run", "run", "busybox"], ["--context", "run", "run", ...label, "busybox"]],
      [["-c", "default", "--config", "/tmp/cfg dir", "create", "busybox"], ["-c", "default", "--config", "/tmp/cfg dir", "create", ...label, "busybox"]],
      [["--host=tcp://h:2375", "-Dl", "info", "--tls", "container", "run", "busybox"], ["--host=tcp://h:2375", "-Dl", "info", "--tls", "container", "run", ...label, "busybox"]],
      [["--debug", "--tlscert", "c.pem", "compose", "-p", "lab", "run", "--rm", "web", "sh", "-c", "echo 'a b'"], ["--debug", "--tlscert", "c.pem", "compose", "-p", "lab", "run", ...label, "--rm", "web", "sh", "-c", "echo 'a b'"]],
    ];
    for (const [args, want] of cases) {
      const { seen, status } = inThread(args);
      expect(seen.argv, args.join(" ")).toEqual(want);
      expect(status).toBe(7);
    }
  });

  it("hands every other call to the real docker with the same words and its exit code", () => {
    for (const args of [
      [],
      ["ps", "-a", "--filter", "label=wsp.thread"],
      ["exec", "-it", "x", "run"],
      ["image", "create"],
      ["container", "ls", "run"],
      ["compose", "ps", "run"],
      ["compose", "down"],
      ["-H", "run", "ps"],
      ["--version"],
      ["build", "-t", "run", "."],
      ["", "run"],
    ]) {
      const { seen, status } = inThread(args);
      expect(seen.argv, args.join(" ")).toEqual(args);
      expect(status).toBe(7);
    }
    expect(existsSync(notes)).toBe(false);
  });

  it("labels nothing outside a thread, or for a thread id the daemon would not read", () => {
    const envs: Record<string, string>[] = [{}, { WSP_THREAD: "" }, { WSP_THREAD: "../../etc" }, { WSP_THREAD: "a b" }, { WSP_THREAD: "a".repeat(65) }];
    for (const env of envs) {
      expect(run(["run", "busybox"], env).seen.argv).toEqual(["run", "busybox"]);
      expect(run(["compose", "up", "-d"], env).seen.argv).toEqual(["compose", "up", "-d"]);
    }
    expect(inThread(["run", "x"], { WSP_THREAD: "a".repeat(64) }).seen.argv).toEqual(["run", "--label", `wsp.thread=${"a".repeat(64)}`, "x"]);
    expect(existsSync(notes)).toBe(false);
  });

  it("runs compose up and create with the person's words and environment unchanged, and notes thread, folder, project and time", () => {
    const before = Math.floor(Date.now() / 1000);
    const cases: [string[], Record<string, string>, string][] = [
      [["compose", "up", "-d"], {}, ""],
      [["compose", "-p", "lab", "up", "-d", "--build"], {}, "lab"],
      [["compose", "--project-name=lab-2", "create"], {}, "lab-2"],
      [["compose", "--project-name", "lab_3", "up"], {}, "lab_3"],
      [["compose", "-plab4", "up"], {}, "lab4"],
      [["compose", "-p=lab5", "up"], {}, "lab5"],
      [["compose", "--workdir", "up", "-p", "lab6", "up"], {}, "lab6"],
      [["--log-level", "warn", "compose", "-f", "compose.yaml", "up", "-d"], { COMPOSE_PROJECT_NAME: "from-env" }, "from-env"],
      [["compose", "-p", "flag", "up"], { COMPOSE_PROJECT_NAME: "from-env" }, "flag"],
    ];
    for (const [args, env] of cases) {
      const { seen, status } = inThread(args, env);
      expect(seen.argv, args.join(" ")).toEqual(args);
      expect(seen.project).toBe(env["COMPOSE_PROJECT_NAME"]);
      expect(status).toBe(7);
    }
    const lines = notesOf("t_1f0c-ab");
    expect(lines.map(([thread, at, project]) => [thread, at, project])).toEqual(cases.map(([, , project]) => ["t_1f0c-ab", folder, project]));
    for (const line of lines) {
      expect(line).toHaveLength(4);
      expect(Number(line[3])).toBeGreaterThanOrEqual(before);
      expect(Number(line[3])).toBeLessThanOrEqual(Math.ceil(Date.now() / 1000));
    }
  });

  it("notes the folder compose takes for the project: --project-directory's, else the first file's, else this one", () => {
    mkdirSync(join(folder, "sub", "deeper"), { recursive: true });
    const elsewhere = realpathSync(mkdtempSync(join(dir, "elsewhere-")));
    const cases: [string[], Record<string, string>, string][] = [
      [["compose", "-f", "sub/compose.yaml", "up", "-d"], {}, join(folder, "sub")],
      [["compose", "--file=sub/deeper/compose.yaml", "-f", "compose.yaml", "up"], {}, join(folder, "sub", "deeper")],
      [["compose", "-f", `${elsewhere}/compose.yaml`, "up"], {}, elsewhere],
      [["compose", "--project-directory", "sub", "-f", "sub/deeper/compose.yaml", "up"], {}, join(folder, "sub")],
      [["compose", "--project-directory=../lab/sub", "up"], {}, join(folder, "sub")],
      [["compose", "--workdir", "sub/deeper", "create"], {}, join(folder, "sub", "deeper")],
      [["compose", "up"], { COMPOSE_FILE: "sub/compose.yaml:compose.yaml" }, join(folder, "sub")],
      [["compose", "-f", "-", "up"], {}, folder],
      [["compose", "-f", "gone/compose.yaml", "up"], {}, folder],
    ];
    for (const [args, env] of cases) expect(inThread(args, env).seen.argv, args.join(" ")).toEqual(args);
    expect(notesOf("t_1f0c-ab").map(([, at]) => at)).toEqual(cases.map(([, , at]) => at));
  });

  it("takes COMPOSE_FILE from the env file compose reads when nothing else names the file, and prints none of that file", () => {
    mkdirSync(join(folder, "sub", "deeper"), { recursive: true });
    const cases: [string, string[], Record<string, string>, string][] = [
      ["COMPOSE_FILE=sub/compose.yaml", ["compose", "up", "-d"], {}, join(folder, "sub")],
      ['A=1\nexport COMPOSE_FILE = "sub/deeper/compose.yaml:compose.yaml"\r', ["compose", "up"], {}, join(folder, "sub", "deeper")],
      ["COMPOSE_FILE='sub/compose.yaml'", ["compose", "create"], {}, join(folder, "sub")],
      ["COMPOSE_FILE=sub/compose.yaml", ["compose", "up"], { COMPOSE_FILE: "sub/deeper/compose.yaml" }, join(folder, "sub", "deeper")],
      ["COMPOSE_FILE=sub/compose.yaml", ["compose", "-f", "compose.yaml", "up"], {}, folder],
      ["COMPOSE_FILE=sub/compose.yaml", ["compose", "--env-file", "other.env", "up"], {}, join(folder, "sub", "deeper")],
      ["SECRET=hunter2", ["compose", "up"], {}, folder],
    ];
    writeFileSync(join(folder, "other.env"), "COMPOSE_FILE=sub/deeper/compose.yaml\n");
    for (const [line, args, env] of cases) {
      writeFileSync(join(folder, ".env"), `SECRET=hunter2\n${line}\n`);
      const r = spawnSync(join(bin, "docker"), args, { env: { PATH: `${bin}:${real}:/usr/bin:/bin`, WSP_THREAD: "t_1f0c-ab", ...env }, cwd: folder, encoding: "utf8", timeout: 5_000 });
      expect([r.status, JSON.parse(r.stdout).argv], line).toEqual([7, args]);
      expect(r.stdout + r.stderr).not.toContain("hunter2");
    }
    expect(notesOf("t_1f0c-ab").map(([, at]) => at)).toEqual(cases.map(([, , , at]) => at));
  });

  it("never waits on an env file that is a pipe", () => {
    rmSync(join(folder, ".env"), { force: true });
    expect(spawnSync("mkfifo", [join(folder, ".env")]).status).toBe(0);
    const r = spawnSync(join(bin, "docker"), ["compose", "up"], { env: { PATH: `${bin}:${real}:/usr/bin:/bin`, WSP_THREAD: "t_1f0c-ab" }, cwd: folder, encoding: "utf8", timeout: 5_000 });
    expect([r.error, r.status]).toEqual([undefined, 7]);
    expect(notesOf("t_1f0c-ab").map(([, at]) => at)).toEqual([folder]);
  });

  it("hands a second thread's compose up the very words and environment the first one's had, so compose recreates nothing", () => {
    const args = ["compose", "-p", "lab", "up", "-d"];
    const { seen: first, status } = inThread(args, { COMPOSE_PROJECT_NAME: "x" });
    const second = run(args, { WSP_THREAD: "t_other", COMPOSE_PROJECT_NAME: "x" }).seen;
    const person = run(args, { COMPOSE_PROJECT_NAME: "x" }).seen;
    expect([first.argv, first.project, status]).toEqual([args, "x", 7]);
    expect(first).toEqual(second);
    expect({ ...first, passed: undefined }).toEqual({ ...person, passed: undefined });
    expect(notesOf("t_1f0c-ab")).toHaveLength(1);
    expect(notesOf("t_other")).toHaveLength(1);
  });

  it("keeps a thread's notes file to its newest lines once it passes the cap", () => {
    mkdirSync(notes);
    const line = `t_1f0c-ab\t${folder}\tlab\t1700000000\n`;
    writeFileSync(join(notes, "t_1f0c-ab"), line.repeat(Math.ceil(DOCKER_NOTES_CAP_BYTES / line.length) + 1));
    inThread(["compose", "up"]);
    const lines = notesOf("t_1f0c-ab");
    expect(lines).toHaveLength(101);
    expect(lines.at(-1)!.slice(0, 3)).toEqual(["t_1f0c-ab", folder, ""]);
  });

  it("runs the call when the note cannot be written, and says nothing of it", () => {
    writeFileSync(notes, "a file where the folder goes");
    const { seen, status, stderr } = inThread(["compose", "up"]);
    expect([seen.argv, status, stderr]).toEqual([["compose", "up"], 7, ""]);
  });

  it("says docker is not installed and exits 127 where no docker follows it on PATH", () => {
    const empty = join(dir, "empty");
    mkdirSync(empty);
    for (const path of [`${bin}:${empty}`, `${bin}:${bin}`, bin]) {
      const r = run(["run", "busybox"], { WSP_THREAD: "t_1f0c-ab" }, path);
      expect([r.status, r.stderr]).toEqual([127, "docker is not installed on this computer\n"]);
    }
  });

  it("finds the real docker past its own folder wherever PATH repeats it, and past a second shim, labelling once", () => {
    const other = join(dir, "place-bin");
    mkdirSync(other);
    writeStub(join(other, "docker"), dockerShim(other, join(dir, "other-notes")));
    const path = `${bin}:${bin}:${other}:${bin}:${real}`;
    const { seen, status } = run(["run", "busybox"], { WSP_THREAD: "t_1f0c-ab" }, path);
    expect([seen.argv, status]).toEqual([["run", ...label, "busybox"], 7]);
    expect(seen.passed).toBe(`${other}:${bin}`);
    expect(run(["compose", "up"], { WSP_THREAD: "t_1f0c-ab" }, `${path}:/usr/bin:/bin`).seen.argv).toEqual(["compose", "up"]);
    expect(notesOf("t_1f0c-ab")).toHaveLength(1);
    expect(existsSync(join(dir, "other-notes"))).toBe(false);
  });

  it("passes over its own folder however PATH spells it", () => {
    for (const spelt of [`${bin}/`, `${bin}//`, join(bin, "..", "thread-bin")]) {
      const { seen, status } = run(["run", "busybox"], { WSP_THREAD: "t_1f0c-ab" }, `${spelt}:${real}`);
      expect([seen.argv, status], spelt).toEqual([["run", ...label, "busybox"], 7]);
    }
    const other = join(dir, "place-bin");
    mkdirSync(other);
    writeStub(join(other, "docker"), dockerShim(other, join(dir, "other-notes")));
    const { seen, status } = run(["run", "busybox"], { WSP_THREAD: "t_1f0c-ab" }, `${bin}/:${other}/:${bin}//:${other}:${real}`);
    expect([seen.argv, status]).toEqual([["run", ...label, "busybox"], 7]);
  });
});
