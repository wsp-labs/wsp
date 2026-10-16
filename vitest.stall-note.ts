// SPDX-License-Identifier: AGPL-3.0-only
// The case each test file is in, written as it starts, for vitest.stall.ts to name a held worker by: the runner
// reports a case's start on a 10 ms timer, which a held event loop never fires. Nothing is written unless that
// reporter is on.
import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { afterEach, beforeEach } from "vitest";
import type { Task } from "vitest";
import { STALL_NOTES_ENV, noteName } from "./vitest.stall.js";

const notes = process.env[STALL_NOTES_ENV];

function note(task: Task, done: boolean): void {
  const path: string[] = [];
  for (let t: Task | undefined = task; t !== undefined && t !== task.file; t = t.suite) path.unshift(t.name);
  writeFileSync(join(notes!, noteName(task.file.projectName ?? "", task.file.filepath)), JSON.stringify({ name: path.join(" > "), done, at: Date.now() }));
}

// The after note is written once the file's own afterEach hooks are done, so a hold in an afterAll or in a later
// block's beforeAll is not read as the last case still running.
if (notes !== undefined) {
  beforeEach(({ task }) => note(task, false));
  afterEach(({ task }) => note(task, true));
}
