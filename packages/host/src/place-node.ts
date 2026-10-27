// SPDX-License-Identifier: AGPL-3.0-only
import { NODE_RELEASES, nodeInstallScript } from "@wsp/catalog";
import type { Machine } from "@wsp/engine";
import { machineLacking, machineUnanswered, nodeInstallFailedLine, shellQuote } from "@wsp/protocol";
import { NODE_MAJOR, WSP_COMMAND_NODE_MAJOR } from "./doctor.js";

/** The catalog's own Node install, run only where the box has no node of the least major, so a Node 24 somebody
 * keeps stays. `bash -e` because that script is a list of lines and a failed download must stop it before the
 * archive is unpacked. The success lines are the script's own; a failure is the last line it said, behind one word
 * the host reads. */
const NODE_FAILED_LINE = "NODE_FAILED";
export const NODE_ENSURE_SCRIPT = [
  `if command -v node >/dev/null 2>&1 && [ "$(${NODE_MAJOR} 2>/dev/null || echo 0)" -ge ${WSP_COMMAND_NODE_MAJOR} ]; then echo NODE_KEPT; exit 0; fi`,
  `if said="$(bash -e -c ${shellQuote(nodeInstallScript(WSP_COMMAND_NODE_MAJOR, NODE_RELEASES[WSP_COMMAND_NODE_MAJOR]))} 2>&1)"; then printf '%s\\n' "$said" | tr '\\r' '\\n' | tail -n 1; exit 0; fi`,
  `echo "${NODE_FAILED_LINE} $(printf '%s\\n' "$said" | tr '\\r' '\\n' | tail -n 1)"`,
  "exit 1",
].join("\n");

/** A node archive is about thirty megabytes from nodejs.org, which a slow link takes a while over. */
const NODE_INSTALL_MS = 240_000;

/** Puts Node 22 on a machine that has none, before the preflight asks for it: the wsp command beside the daemon
 * runs on it, and a fresh box has none, so refusing there would leave every fresh box to a hand install. Only the
 * joined road calls it: the machine is root's by then, which an install under /usr/local needs. */
export async function ensureNode(machine: Machine): Promise<void> {
  const res = await machine.run(NODE_ENSURE_SCRIPT, { deadlineMs: NODE_INSTALL_MS });
  const said = res.stdout.split("\n").map(line => line.trim()).filter(line => line !== "").at(-1);
  if (res.exitCode === 0 && said !== undefined && /^NODE_(KEPT|INSTALLED)\b/.test(said)) return;
  if (res.exitCode === 1 && said?.startsWith(`${NODE_FAILED_LINE} `) === true) throw machineLacking(nodeInstallFailedLine(said.slice(NODE_FAILED_LINE.length + 1)));
  throw machineUnanswered(`the machine did not answer what a daemon needs: ${(said ?? res.stderr.slice(-200)).trim()}`);
}
