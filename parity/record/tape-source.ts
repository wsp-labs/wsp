// SPDX-License-Identifier: AGPL-3.0-only
// Names every tape a case's hosts write after the case: its file, then its describe and it lines.
import { relative } from "node:path";
import { beforeEach, type Task } from "vitest";
import { TAPE_SOURCE_ENV } from "@wsp/runtime";

beforeEach(({ task }) => {
  const names: string[] = [];
  for (let t: Task | undefined = task; t !== undefined && t !== task.file; t = t.suite) names.unshift(t.name);
  process.env[TAPE_SOURCE_ENV] = [relative(process.cwd(), task.file.filepath), ...names].join(" > ");
});
