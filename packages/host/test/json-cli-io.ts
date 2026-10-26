// SPDX-License-Identifier: AGPL-3.0-only
import type { Writable } from "node:stream";
import { authRefusal } from "@wsp/protocol";
import { KEY_LAYER_WORDS } from "../src/env-keys.js";
import type { CliIO } from "../src/cli.js";

/** An IO for a run that speaks no prompt: every line goes to the stream given, stderr by default, and a key that is not
 * in the environment or a .env file is an error rather than a question on a stream nobody is reading. */
export function jsonCliIO(err: Writable = process.stderr): CliIO {
  const say = (line: string): void => void err.write(`${line}\n`);
  const nobodyLine = (q: string, variable?: string): string => `${q.split("\n")[0]}: --json asks nothing; set ${variable ?? "it"} in ${KEY_LAYER_WORDS}.`;
  const nobody = (q: string): Promise<never> => Promise.reject(new Error(nobodyLine(q)));
  const noKey = (q: string, variable?: string): Promise<never> => Promise.reject(authRefusal(nobodyLine(q, variable)));
  return { log: say, error: say, stream: text => void err.write(text), ask: nobody, askSecret: noKey };
}
