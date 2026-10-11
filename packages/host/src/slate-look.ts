// SPDX-License-Identifier: AGPL-3.0-only
// How a host draws a slate for a look: through the app's own Electron, which is the program running this host when
// the app started it (its command line runs as ELECTRON_RUN_AS_NODE), so a box thread's slate is drawn here the same.
// A host on plain Node has no renderer and draws nothing. One child per look, gone when the PNG is back or the
// deadline passes, so nothing stays in memory between looks.
import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { createInterface } from "node:readline";
import { fileURLToPath } from "node:url";
import type { DrawSlate } from "@wsp/runtime";

/** A look that has not drawn by now never will: the page waits at most 8 s on its pictures. */
export const LOOK_DEADLINE_MS = 30_000;

/** The Electron this host can draw with, and the script it runs, or nothing on plain Node or where the script was
 * not built beside this one. */
export function lookRoad(): { electron: string; script: string } | undefined {
  if (process.versions["electron"] === undefined) return undefined;
  const script = fileURLToPath(new URL("./slate-look-main.js", import.meta.url));
  return existsSync(script) ? { electron: process.execPath, script } : undefined;
}

export function drawSlateThrough(road: { electron: string; script: string }, base: () => string): DrawSlate {
  return drawing =>
    new Promise((done, fail) => {
      const env = { ...process.env };
      delete env["ELECTRON_RUN_AS_NODE"];
      const child = spawn(road.electron, [road.script], { env, stdio: ["pipe", "pipe", "pipe"] });
      let said = "";
      child.stderr.on("data", (chunk: Buffer) => void (said = `${said}${chunk.toString()}`.slice(-2000)));
      const timer = setTimeout(() => {
        child.kill("SIGKILL");
        fail(new Error(`the slate did not draw within ${LOOK_DEADLINE_MS / 1000} s`));
      }, LOOK_DEADLINE_MS);
      const send = (line: Record<string, unknown>): void => void child.stdin.write(`${JSON.stringify(line)}\n`);
      child.on("error", e => (clearTimeout(timer), fail(e)));
      child.on("close", code => {
        clearTimeout(timer);
        fail(new Error(`the app's renderer quit (exit ${code}) before the slate drew${said.trim() === "" ? "" : `: ${said.trim().split("\n").at(-1)}`}`));
      });
      createInterface({ input: child.stdout }).on("line", line => {
        let msg: { type?: string; id?: number; kind?: "image" | "page"; src?: string; png?: string; width?: number; height?: number; message?: string };
        try {
          msg = JSON.parse(line);
        } catch {
          return;
        }
        if (msg.type === "read" && msg.id !== undefined && msg.kind !== undefined && msg.src !== undefined) {
          const id = msg.id;
          void drawing.read(msg.kind, msg.src).then(
            answer => send({ type: "answer", id, answer }),
            (e: unknown) => send({ type: "answer", id, answer: { problem: { code: "R903", name: "source-unavailable", message: e instanceof Error ? e.message : String(e) } } }),
          );
        } else if (msg.type === "done" && msg.png !== undefined) {
          clearTimeout(timer);
          done({ png: Buffer.from(msg.png, "base64"), width: msg.width ?? drawing.width, height: msg.height ?? 0 });
          child.stdin.end();
        } else if (msg.type === "failed") {
          clearTimeout(timer);
          fail(new Error(`the app's renderer could not draw the slate: ${msg.message ?? "no reason given"}`));
        }
      });
      send({ type: "slate", base: base(), view: drawing.view, theme: drawing.theme, width: drawing.width });
    });
}
