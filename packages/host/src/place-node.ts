// SPDX-License-Identifier: AGPL-3.0-only
import { NODE_RELEASES, nodeInstallScript } from "@wsp/catalog";
import type { Machine } from "@wsp/engine";
import { machineLacking, machineUnanswered, nodeInstallFailedLine, nodePutLine, nodeUnansweredLine, shellQuote } from "@wsp/protocol";
import { NODE_LACKING, WSP_COMMAND_NODE_MAJOR } from "./doctor.js";

/** The catalog's own Node install, run only where the preflight's own test says the box lacks a node of the least
 * major, so a Node 24 somebody keeps stays. `bash -e` because that script is a list of lines and a failed download
 * must stop it before the archive is unpacked. A success is the script's last line followed by the Node the box had
 * and where it stood, read before anything was written; a failure is the last line it said, behind one word the
 * host reads. */
const NODE_FAILED_LINE = "NODE_FAILED";
const LAST_LINE = String.raw`printf '%s\n' "$said" | tr '\r' '\n' | tail -n 1`;
export const NODE_ENSURE_SCRIPT = [
  `if ! { ${NODE_LACKING}; }; then echo NODE_KEPT; exit 0; fi`,
  `had="$(node --version 2>/dev/null || echo none)"; at="$(command -v node 2>/dev/null || echo none)"`,
  `if said="$(bash -e -c ${shellQuote(nodeInstallScript(WSP_COMMAND_NODE_MAJOR, NODE_RELEASES[WSP_COMMAND_NODE_MAJOR]))} 2>&1)"; then echo "$(${LAST_LINE}) had=$had at=$at"; exit 0; fi`,
  `echo "${NODE_FAILED_LINE} $(${LAST_LINE})"`,
  "exit 1",
].join("\n");

/** A node archive is about thirty megabytes from nodejs.org, which a slow link takes a while over. */
const NODE_INSTALL_MS = 240_000;

/** Puts Node 22 on a machine that has none, before the preflight asks for it: the wsp command beside the daemon
 * runs on it, and a fresh box has none, so refusing there would leave every fresh box to a hand install. Only the
 * joined road calls it: the machine is root's by then, which an install under /usr/local needs. Answers what it did
 * to the computer's Node, for the step's row, and nothing where the computer already had one. */
export async function ensureNode(machine: Machine): Promise<string | undefined> {
  const res = await machine.run(NODE_ENSURE_SCRIPT, { deadlineMs: NODE_INSTALL_MS });
  const said = res.stdout.split("\n").map(line => line.trim()).filter(line => line !== "").at(-1);
  if (res.exitCode === 0 && said === "NODE_KEPT") return undefined;
  const put = res.exitCode === 0 ? /^NODE_INSTALLED (v\S+) had=(\S+) at=(.*)$/.exec(said ?? "") : null;
  if (put !== null) return nodePutLine(put[1]!, put[2]!, put[3]!);
  if (res.exitCode === 1 && said?.startsWith(`${NODE_FAILED_LINE} `) === true) throw machineLacking(nodeInstallFailedLine(said.slice(NODE_FAILED_LINE.length + 1)));
  throw machineUnanswered(nodeUnansweredLine(said ?? res.stderr.split(/[\r\n]+/).filter(line => line.trim() !== "").at(-1) ?? ""));
}
