// SPDX-License-Identifier: AGPL-3.0-only
// A folder's path and Git metadata are read by the computer holding it, never by this host's filesystem.
import { INLINE_EXEC_MS, type Machine } from "@wsp/engine";
import type { ResolvedSource } from "../project-sources.js";
import { projectNameOf, shellQuote, usageRefusal } from "@wsp/protocol";

export async function existingFolder(machine: Machine, home: string, asked: string): Promise<{ source: { kind: "folder"; path: string }; top: string | undefined; resolved: ResolvedSource }> {
  const path = asked === "~" ? home : asked.startsWith("~/") ? `${home.replace(/\/+$/, "")}/${asked.slice(2)}` : asked;
  if (!path.startsWith("/") || path.includes("\0")) throw usageRefusal("The folder needs an absolute path on that computer.", "Choose a folder there, or type its path starting with / or ~/.");
  const script = [
    `cd -P ${shellQuote(path)} || exit 1`,
    `test -r . && test -x . || exit 1`,
    `printf '%s\\0' "$PWD"`,
    `if top=$(git rev-parse --show-toplevel 2>/dev/null); then`,
    `  printf '%s\\0' "$top"`,
    `  git config --get remote.origin.url 2>/dev/null || :`,
    `  printf '\\0'`,
    `  branch=$(git symbolic-ref --quiet --short refs/remotes/origin/HEAD 2>/dev/null) && printf '%s' "\${branch#origin/}" || git symbolic-ref --quiet --short HEAD 2>/dev/null || :`,
    `else printf '\\0\\0'; fi`,
  ].join("\n");
  const read = await machine.exec(script, { timeoutMs: INLINE_EXEC_MS });
  if (read.exitCode !== 0) throw usageRefusal(`The folder ${path} could not be read on that computer.`, "Check that it exists and that the login used to add the computer can read it.");
  const fields = read.stdout.split("\0");
  if (fields.length !== 4 || !fields[0]?.startsWith("/")) throw new Error(`The computer did not return the folder metadata for ${path}. Try again.`);
  const source = { kind: "folder" as const, path: fields[0] };
  const top = fields[1]!.replace(/\r?\n$/, "");
  return { source, top: top === "" ? undefined : top, resolved: { remote: fields[2]!.replace(/\r?\n$/, ""), defaultBranch: fields[3]!.replace(/\r?\n$/, "") || "main", name: projectNameOf(source) } };
}
