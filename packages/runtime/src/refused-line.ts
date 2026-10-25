// SPDX-License-Identifier: AGPL-3.0-only
import { isObjectFrame, markedCut, redacted, requestSecrets, RUNTIME_OPS, SCANNED_SECRETS } from "@wsp/protocol";

const DECLARED_OPS = new Set(RUNTIME_OPS);

/** The log line of one refused frame: an op the protocol declares, else `frame`, since the frame may be a stranger's;
 * the kind; and the sentence's first line with the request's secrets blanked, then cut. Blanked before the cut, since
 * a cut through a secret would leave its head where the blanking no longer matches it. */
export function refusedLine(frame: unknown, payload: Record<string, unknown>, said?: string): string {
  const asked = isObjectFrame(frame) ? frame["op"] : undefined;
  const op = typeof asked === "string" && DECLARED_OPS.has(asked) ? asked : "frame";
  const kind = typeof payload["kind"] === "string" ? payload["kind"] : "none";
  const secrets = [...new Set(requestSecrets(frame))];
  if (secrets.length > SCANNED_SECRETS) return `refused ${op} kind=${kind}: (sentence withheld, ${secrets.length} secret values)`;
  const text = said ?? String(payload["error"]);
  const end = text.indexOf("\n");
  const first = redacted(end === -1 ? text : text.slice(0, end), secrets);
  return `refused ${op} kind=${kind}: ${markedCut(first)}`;
}
