// SPDX-License-Identifier: AGPL-3.0-only
// A run that stops moving fails, naming each file still unfinished and the case running in it. A worker whose event
// loop is held, by a synchronous wait or a loop that never yields, never fires vitest's own per-case timeout, so CI's
// job timeout cancelled the run instead, and GitHub keeps no log of a cancelled job to name the case by. Once every
// other file is done the unfinished ones are the held ones. CI's test commands name this reporter; a live run, whose
// one case may rightly run for many minutes, does not.
import { createHash } from "node:crypto";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { File } from "vitest";
import type { Vitest } from "vitest/node";
import type { Reporter } from "vitest/reporters";

/** Fifteen times the budget a case gets (20 s), and well inside CI's 20-minute job once the other files are done. */
const STALL_MS = 5 * 60_000;

/** The folder vitest.stall-note.ts writes each file's running case into, handed to the workers by this variable. */
export const STALL_NOTES_ENV = "VITEST_STALL_NOTES";

/** The note's name for a test file as one project runs it: the node and cloud projects run some files at once. */
export function noteName(project: string, filepath: string): string {
  return createHash("sha1").update(`${project}\0${filepath}`).digest("hex");
}

const DONE = new Set(["pass", "fail", "skip", "todo"]);

export default class StallReporter implements Reporter {
  private ctx: Vitest | undefined;
  private timer: NodeJS.Timeout | undefined;
  private readonly notes = mkdtempSync(join(tmpdir(), "vitest-stall-"));

  constructor(private readonly options: { stallMs?: number } = {}) {
    process.env[STALL_NOTES_ENV] = this.notes;
    process.once("exit", () => rmSync(this.notes, { recursive: true, force: true }));
  }

  onInit(ctx: Vitest): void {
    this.ctx = ctx;
    this.arm();
  }

  onCollected(): void {
    this.arm();
  }

  onTaskUpdate(): void {
    this.arm();
  }

  onWatcherRerun(): void {
    this.arm();
  }

  onFinished(): void {
    clearTimeout(this.timer);
  }

  private arm(): void {
    clearTimeout(this.timer);
    this.timer = setTimeout(() => this.fail(), this.options.stallMs ?? STALL_MS).unref();
  }

  private caseIn(file: File, now: number): string {
    try {
      const note = JSON.parse(readFileSync(join(this.notes, noteName(file.projectName ?? "", file.filepath)), "utf8")) as { name: string; done: boolean; at: number };
      const since = Math.round((now - note.at) / 1000);
      return note.done ? `held after ${note.name} ended, in an afterAll or a later block's beforeAll (${since} s)` : `${note.name} (running ${since} s)`;
    } catch {
      return "no case started: held while importing or in a beforeAll";
    }
  }

  private fail(): void {
    const ctx = this.ctx!;
    const now = Date.now();
    const open = ctx.state.getFiles().filter((file: File) => !DONE.has(file.result?.state ?? ""));
    const lines = [
      `no case started or finished for ${Math.round((this.options.stallMs ?? STALL_MS) / 1000)} s; ${open.length} file${open.length === 1 ? "" : "s"} unfinished:`,
      ...open.map(file => `  ${file.projectName ? `[${file.projectName}] ` : ""}${file.name}: ${this.caseIn(file, now)}`),
    ];
    process.stderr.write(`\n${lines.join("\n")}\n`);
    process.exitCode = 1;
    void ctx.exit(true);
  }
}
