// SPDX-License-Identifier: AGPL-3.0-only
// A slate's command server on the computer its thread runs on, a box or a cloud machine: started there through that
// computer's daemon (stdio.open), as the login its lines run as, never on this computer with the sign-ins here. Its
// variables are written first on its own stdin as NUL-separated pairs ended by an empty name, which bash reads one
// byte at a time into its environment before it becomes the server, so none is on a command line and the server
// reads the rest of the stream as it was sent. The daemon kills it with the road it was opened on. An http server
// is asked from there too (http.post), so an address on that computer's own loopback answers, and a sign-in the
// agent keeps there is read there by the daemon and never comes here.
import { randomBytes } from "node:crypto";
import type { McpTransport } from "@wsp/catalog";
import { asLogin, type TargetLogin } from "@wsp/engine";
import { EXEC_TIMEOUT_MAX_MS, HttpPostReply, shellQuote } from "@wsp/protocol";
import type { McpPipeOpen, McpPost, McpSignIn } from "./slate-mcp.js";

/** One computer's daemon for one server: its frames, its events, and its end. */
export interface StdioRoad {
  /** Throws the daemon's refusal. */
  send(frame: Record<string, unknown>): Promise<unknown>;
  /** Every event the road carries; the answer stops them. */
  listen(fn: (event: Record<string, unknown>) => void): () => void;
  readonly closed: Promise<unknown>;
  close(): void;
}

const ENV_THEN_SERVER = `while IFS= read -r -d '' k && [ -n "$k" ] && IFS= read -r -d '' v; do export "$k=$v"; done; `;

/** The command stdio.open runs: the server as that login, in its folder, its variables off its stdin first. */
export const serverLine = (login: TargetLogin, t: Extract<McpTransport, { kind: "stdio" }>, cwd: string): string =>
  asLogin(login, `${ENV_THEN_SERVER}cd -- ${shellQuote(t.cwd ?? cwd)} 2>/dev/null; exec ${[t.command, ...t.args].map(shellQuote).join(" ")}`);

export const envPairs = (env: Readonly<Record<string, string>>): string => `${Object.entries(env).map(([k, v]) => `${k}\0${v}\0`).join("")}\0`;

const base64 = (text: string): string => Buffer.from(text).toString("base64");

/** Starts a server through `road`, the writes before the start answers held and sent after the variables. */
export function pipeOn(computer: string, login: TargetLogin, road: () => Promise<StdioRoad>): McpPipeOpen {
  return (t, spec, on) => {
    const stdioId = randomBytes(8).toString("hex");
    const held: string[] = [envPairs({ ...spec.env, ...t.env })];
    let dialled: StdioRoad | undefined;
    let opened: StdioRoad | undefined;
    let killed = false;
    let stop = (): void => {};
    const write = (r: StdioRoad, text: string): void => void r.send({ op: "stdio.write", stdioId, data: base64(text) }).catch(() => {});
    const end = (r: StdioRoad): void => {
      stop();
      void r.send({ op: "stdio.close", stdioId }).catch(() => {}).finally(() => r.close());
    };
    void road()
      .then(async r => {
        if (killed) return r.close();
        dialled = r;
        stop = r.listen(e => {
          if (e["stdioId"] !== stdioId) return;
          if (e["type"] === "stdio.out") on.out(Buffer.from(String(e["data"]), "base64"));
          else if (e["type"] === "stdio.err") on.err(Buffer.from(String(e["data"]), "base64").toString("utf8"));
          else if (e["type"] === "stdio.exit") on.exit(typeof e["code"] === "number" ? `the server exited with ${e["code"]}` : "the server was stopped");
        });
        void r.closed.then(() => on.exit(`the link to ${computer} closed`));
        await r.send({ op: "stdio.open", stdioId, cmd: serverLine(login, t, spec.cwd) });
        if (killed) return end(r);
        opened = r;
        for (const text of held.splice(0)) write(r, text);
      })
      .catch((e: unknown) => {
        on.exit(`it could not be started on ${computer}: ${e instanceof Error ? e.message : String(e)}`);
        stop();
        dialled?.close();
      });
    return {
      write: text => (opened === undefined ? void held.push(text) : write(opened, text)),
      kill() {
        if (killed) return;
        killed = true;
        if (opened !== undefined) end(opened);
      },
    };
  };
}

/** Why a run on a server whose sign-in the agent there no longer holds failed, and what the person runs there. */
export const signInGoneLine = (computer: string, line: string): string => `its address wants a sign-in that ${computer} does not hold now; run ${line} there, then start the run again`;

/** An http server asked through `road`, one per connection, opened at the first message. Where the agent keeps its
 * sign-in there, a refusal for want of one asks the agent's own word again, which renews a token near its end, once
 * at a time however many messages were refused, and sends the message once more. */
export function postOn(computer: string, road: () => Promise<StdioRoad>, signIn: McpSignIn | undefined): () => McpPost {
  return () => {
    let opened: Promise<StdioRoad> | undefined;
    let renewing: Promise<boolean> | undefined;
    let closed = false;
    const ask = async (t: Extract<McpTransport, { kind: "http" }>, req: { headers: Record<string, string>; body: string; ms: number }) => {
      opened ??= road();
      const r = await opened;
      if (closed) throw new Error("the server's connection is closed");
      const frame = { op: "http.post", url: t.url, headers: req.headers, body: req.body, timeoutMs: Math.min(req.ms, EXEC_TIMEOUT_MAX_MS), ...(signIn !== undefined ? { bearer: signIn.bearer } : {}) };
      return HttpPostReply.parse(await r.send(frame));
    };
    const refused = (status: number): boolean => status === 401 || status === 403;
    return {
      async send(t, req) {
        if (closed) throw new Error("the server's connection is closed");
        const first = await ask(t, req);
        if (signIn === undefined || !refused(first.status)) return first;
        renewing ??= signIn.renew().finally(() => (renewing = undefined));
        if (!(await renewing)) throw new Error(signInGoneLine(computer, signIn.line));
        const again = await ask(t, req);
        if (refused(again.status)) throw new Error(signInGoneLine(computer, signIn.line));
        return again;
      },
      close() {
        closed = true;
        void opened?.then(r => r.close(), () => {});
      },
    };
  };
}
