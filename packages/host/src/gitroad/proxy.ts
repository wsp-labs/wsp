// SPDX-License-Identifier: AGPL-3.0-only
// The host's git road: a cloud machine's git sends absolute-form proxy requests for github.com to its HTTP door, and
// the road adds the person's gh credential on the way to GitHub. Fetches go to any repo the person reads; pushes go
// only to the thread's recorded repo, by GitHub's repo id, and only to its branch of record.
// Never a 401 down the road: git answers one by asking credential helpers and then prompting.
import { createReadStream, createWriteStream } from "node:fs";
import { mkdtemp, rm } from "node:fs/promises";
import { request as httpRequest, type IncomingHttpHeaders, type IncomingMessage, type ServerResponse } from "node:http";
import { request as httpsRequest } from "node:https";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { Transform } from "node:stream";
import { pipeline } from "node:stream/promises";
import { createGunzip } from "node:zlib";
import type { BranchOfRecord } from "@wsp/protocol";
import {
  GUARD_FAILED,
  judge,
  lfsRefusal,
  NO_BRANCH_REFUSAL,
  NO_SIGNIN_REFUSAL,
  PACK_TOO_LARGE,
  SPOOL_FAILED,
  pathRefusal,
  PUSH_CERT_REFUSAL,
  pushRepoRefusal,
  repoForbiddenRefusal,
  repoHiddenRefusal,
  SIGNIN_REFUSED,
  UNREACHABLE,
  UNREADABLE_PUSH,
  type RefCommand,
} from "./branch.js";
import { capList, encodePushHead, mergeReport, PktReader, readPushHead, refusalReport, rewriteAdvertisement, sideBandOf, type Refused } from "./receive-pack.js";

type Door = "http" | "gh";

/** The broker's request for one local connection, the fields the git road reads. */
export interface GitRequest {
  workspaceId: string;
  door: Door;
  grant: { kind: "turn" | "op" | "terminal"; workspaceId: string; threadId?: string; op?: string; branch?: string };
  req: IncomingMessage;
  res: ServerResponse;
  heldMs: number;
}

/** What the road adds to the broker's line for a push; the broker writes the thread, machine, path and bytes itself. */
export interface GitLogLine {
  pushes: { ref: string; old: string; new: string; verdict: "allowed" | "refused" }[];
}

/** What the git road asks of the broker. */
export interface GitContext {
  credential(kind: "github"): Promise<{ token: string } | undefined>;
  workspace(id: string): Promise<{ branchOfRecord?: BranchOfRecord }>;
  refuse(res: ServerResponse, status: 403 | 503, said: string, shape: "git"): void;
  log(line: GitLogLine): void;
}

/** One accepted push the `.github/` guard reads before it goes upstream: a self-contained pack on disk. */
export interface GuardedPush {
  workspaceId: string;
  repo: { owner: string; name: string; id: number };
  commands: RefCommand[];
  pack: string;
  /** The branch of record's start commit; never read off the pack, which a crafted push could build on an old commit. */
  base?: string;
}

/** The refusal sentence for a push, or undefined to let it through. */
export type PushGuard = (push: GuardedPush) => Promise<string | undefined>;

export interface GitRoadOptions {
  guard: PushGuard;
  /** Where github.com's smart HTTP answers. */
  origin?: string;
  /** Where GitHub's REST API answers, for a repo's id. */
  api?: string;
  /** The folder a push's pack is spooled under while the guard reads it. */
  scratch?: string;
  /** The most a push's pack may hold on disk. */
  packCap?: number;
}

/** Why there is no repo id to compare: a refusal with its status. */
interface NoId {
  status: 403 | 503;
  said: string;
}

const REPO_ID_TTL_MS = 5 * 60_000;
const REPO_ID_CAP = 512;
const REPORT_CAP = 16 * 1024 * 1024;
/** GitHub refuses a push over 2 GB, so a pack past it is refused before it fills the person's disk. */
const PACK_CAP = 2 * 1024 * 1024 * 1024;
const PACK_SIGNATURE = Buffer.from("PACK");
const PATH = /^\/([A-Za-z0-9](?:[A-Za-z0-9-]*[A-Za-z0-9])?)\/([A-Za-z0-9._-]+?)(?:\.git)?\/(info\/refs|git-upload-pack|git-receive-pack)$/;
const FORWARDED = ["accept", "accept-language", "content-type", "git-protocol", "user-agent"];
const STREAMED = ["accept-encoding", "content-encoding", "content-length"];
const DROPPED_BACK = new Set(["connection", "keep-alive", "proxy-authenticate", "proxy-connection", "set-cookie", "te", "trailer", "transfer-encoding", "upgrade", "www-authenticate"]);

class SpoolStop extends Error {
  constructor(readonly why: "too large" | "not a pack") {
    super(why);
  }
}

/** Writes a push's pack to disk: its byte count, or why it stopped, past the cap or on bytes that do not open as a pack. Rejects when the disk refuses the write. */
export async function spool(source: AsyncIterable<Buffer>, path: string, cap: number): Promise<number | "too large" | "not a pack"> {
  let size = 0;
  let opening = Buffer.alloc(0);
  const check = new Transform({
    transform(chunk: Buffer, _encoding, done) {
      size += chunk.length;
      if (size > cap) return done(new SpoolStop("too large"));
      if (opening.length < PACK_SIGNATURE.length) {
        opening = Buffer.concat([opening, chunk]).subarray(0, PACK_SIGNATURE.length);
        if (!PACK_SIGNATURE.subarray(0, opening.length).equals(opening)) return done(new SpoolStop("not a pack"));
      }
      done(null, chunk);
    },
  });
  try {
    await pipeline(source, check, createWriteStream(path));
  } catch (err) {
    if (err instanceof SpoolStop) return err.why;
    throw err;
  }
  if (size > 0 && opening.length < PACK_SIGNATURE.length) return "not a pack";
  return size;
}

export function gitRoad(options: GitRoadOptions) {
  const origin = options.origin ?? "https://github.com";
  const api = options.api ?? "https://api.github.com";
  const ids = new Map<string, { id: number; at: number }>();

  async function repoId(owner: string, name: string, token: string, record: BranchOfRecord): Promise<number | NoId> {
    const key = `${owner}/${name}`.toLowerCase();
    const known = ids.get(key);
    if (known && Date.now() - known.at < REPO_ID_TTL_MS) return known.id;
    // A renamed repo's old name answers 301 to /repositories/<id>: the id is read off the Location, never followed.
    const answer = await fetch(`${api}/repos/${owner}/${name}`, {
      redirect: "manual",
      headers: { authorization: `Bearer ${token}`, accept: "application/vnd.github+json", "user-agent": "wsp" },
    }).catch(() => undefined);
    if (!answer || answer.status >= 500) return { status: 503, said: UNREACHABLE };
    if (answer.status === 401) return { status: 403, said: SIGNIN_REFUSED };
    if (answer.status === 403) return { status: 403, said: repoForbiddenRefusal(owner, name) };
    const own = key === `${record.owner}/${record.name}`.toLowerCase();
    if (answer.status === 404) return { status: 403, said: own ? repoHiddenRefusal(owner, name) : pushRepoRefusal(owner, name, record) };
    const moved = answer.status === 301 ? /\/repositories\/(\d+)$/.exec(answer.headers.get("location") ?? "") : null;
    const body = answer.ok ? ((await answer.json().catch(() => ({}))) as { id?: unknown }) : {};
    const id = moved ? Number(moved[1]) : body.id;
    if (typeof id !== "number" || !Number.isSafeInteger(id)) return { status: 503, said: UNREACHABLE };
    ids.delete(key);
    ids.set(key, { id, at: Date.now() });
    if (ids.size > REPO_ID_CAP) ids.delete(ids.keys().next().value as string);
    return id;
  }

  function upstream(method: string, path: string, headers: Record<string, string>, token: string) {
    const url = new URL(path, origin);
    const send = url.protocol === "https:" ? httpsRequest : httpRequest;
    const basic = Buffer.from(`x-access-token:${token}`).toString("base64");
    return send(url, { method, headers: { ...headers, authorization: `Basic ${basic}` } });
  }

  function clientHeaders(req: IncomingMessage, streamed: boolean): Record<string, string> {
    const out: Record<string, string> = {};
    for (const name of streamed ? [...FORWARDED, ...STREAMED] : FORWARDED) {
      const value = req.headers[name];
      if (typeof value === "string") out[name] = value;
    }
    return out;
  }

  function backHeaders(headers: IncomingHttpHeaders, rewritten: boolean): Record<string, string | string[]> {
    const out: Record<string, string | string[]> = {};
    for (const [name, value] of Object.entries(headers)) {
      if (value === undefined || DROPPED_BACK.has(name)) continue;
      if (rewritten && (name === "content-length" || name === "content-encoding")) continue;
      out[name] = value;
    }
    return out;
  }

  /** Sends one request upstream and answers with its response, through `answer` once the status is known. */
  function relay(
    r: GitRequest,
    ctx: GitContext,
    up: ReturnType<typeof upstream>,
    answer: (response: IncomingMessage) => Promise<void>,
  ): Promise<void> {
    return new Promise<void>((resolve) => {
      r.res.on("close", () => {
        up.destroy();
        resolve();
      });
      up.on("error", () => {
        if (!r.res.headersSent) ctx.refuse(r.res, 503, UNREACHABLE, "git");
        else r.res.destroy();
        resolve();
      });
      up.on("response", (response) => {
        if (response.statusCode === 401) {
          response.resume();
          ctx.refuse(r.res, 403, SIGNIN_REFUSED, "git");
          return resolve();
        }
        answer(response).then(resolve, () => {
          r.res.destroy();
          resolve();
        });
      });
    });
  }

  const streamBack = (res: ServerResponse) => async (response: IncomingMessage) => {
    res.writeHead(response.statusCode ?? 502, backHeaders(response.headers, false));
    await pipeline(response, res);
  };

  async function fetchRoad(r: GitRequest, ctx: GitContext, path: string, token: string): Promise<void> {
    const up = upstream(r.req.method ?? "GET", path, clientHeaders(r.req, true), token);
    const done = relay(r, ctx, up, streamBack(r.res));
    r.req.pipe(up);
    await done;
  }

  async function advertise(r: GitRequest, ctx: GitContext, path: string, token: string): Promise<void> {
    const headers: Record<string, string> = { ...clientHeaders(r.req, false), "accept-encoding": "identity" };
    delete headers["git-protocol"];
    const up = upstream("GET", path, headers, token);
    const done = relay(r, ctx, up, async (response) => {
      if (response.statusCode !== 200) return streamBack(r.res)(response);
      r.res.writeHead(200, backHeaders(response.headers, true));
      let head: Buffer | undefined = Buffer.alloc(0);
      for await (const chunk of response as AsyncIterable<Buffer>) {
        if (!head) {
          r.res.write(chunk);
          continue;
        }
        head = Buffer.concat([head, chunk]);
        const served = rewriteAdvertisement(head);
        if (served) {
          r.res.write(served);
          head = undefined;
        }
      }
      r.res.end(head);
    });
    up.end();
    await done;
  }

  async function push(r: GitRequest, ctx: GitContext, path: string, token: string, repo: GuardedPush["repo"], record: BranchOfRecord): Promise<void> {
    const encoding = r.req.headers["content-encoding"];
    const body = encoding === "gzip" || encoding === "x-gzip" ? r.req.pipe(createGunzip()) : r.req;
    const reader = new PktReader(body);
    const drain = async () => {
      for await (const _ of reader.rest());
    };
    const head = await readPushHead(reader).catch(() => undefined);
    if (!head) {
      await drain().catch(() => undefined);
      return ctx.refuse(r.res, 403, UNREADABLE_PUSH, "git");
    }
    if (head.certified) {
      await drain();
      return ctx.refuse(r.res, 403, PUSH_CERT_REFUSAL, "git");
    }
    const sideBand = sideBandOf(head.caps);
    const judged = judge(head.commands, record.branch, capList(head.caps).includes("atomic"));
    const refused: Refused[] = judged.flatMap((j) => (j.said ? [{ ref: j.command.ref, said: j.said }] : []));
    const kept = head.commands.filter((_, i) => !judged[i]?.said);
    const log = (verdicts: Refused[]) =>
      ctx.log({ pushes: head.commands.map((c) => ({ ref: c.ref, old: c.old, new: c.new, verdict: verdicts.some((v) => v.ref === c.ref) ? "refused" : "allowed" })) });
    const answer = (all: Refused[]) => {
      r.res.writeHead(200, { "content-type": "application/x-git-receive-pack-result", "cache-control": "no-cache" });
      r.res.end(refusalReport(all, sideBand));
    };
    /** Refuses every command without reading the rest of the body, which may never end. */
    const answerAndClose = (said: string) => {
      const all = head.commands.map((c) => ({ ref: c.ref, said }));
      log(all);
      r.res.writeHead(200, { "content-type": "application/x-git-receive-pack-result", "cache-control": "no-cache", connection: "close" });
      r.res.end(refusalReport(all, sideBand), () => r.req.destroy());
    };
    if (head.commands.length > 0 && kept.length === 0) {
      await drain();
      log(refused);
      return answer(refused);
    }
    const updates = kept.some((c) => !/^0+$/.test(c.new));
    const folder = await mkdtemp(join(options.scratch ?? tmpdir(), "wsp-push-")).catch(() => undefined);
    if (!folder) return answerAndClose(SPOOL_FAILED);
    try {
      const pack = join(folder, "pack");
      const spooled = await spool(reader.rest(), pack, options.packCap ?? PACK_CAP).catch(() => "failed" as const);
      if (spooled === "failed") return answerAndClose(SPOOL_FAILED);
      if (spooled === "too large") return answerAndClose(PACK_TOO_LARGE);
      if (spooled === "not a pack") return answerAndClose(UNREADABLE_PUSH);
      const size = updates ? spooled : 0;
      if (size > 0) {
        const commands = kept.map(({ old, new: next, ref }) => ({ old, new: next, ref }));
        const said = await options.guard({ workspaceId: r.workspaceId, repo, commands, pack, base: record.startSha }).catch(() => GUARD_FAILED);
        if (said) {
          const all = head.commands.map((c) => refused.find((x) => x.ref === c.ref) ?? { ref: c.ref, said });
          log(all);
          return answer(all);
        }
      }
      const lines = encodePushHead(head, kept);
      const up = upstream("POST", path, {
        ...clientHeaders(r.req, false),
        "accept-encoding": "identity",
        "content-type": "application/x-git-receive-pack-request",
        "content-length": String(lines.length + size),
      }, token);
      const done = relay(r, ctx, up, async (response) => {
        if (refused.length === 0 || response.statusCode !== 200) return streamBack(r.res)(response);
        const chunks: Buffer[] = [];
        let total = 0;
        for await (const chunk of response as AsyncIterable<Buffer>) {
          total += chunk.length;
          if (total > REPORT_CAP) throw new Error("report past the cap");
          chunks.push(chunk);
        }
        r.res.writeHead(200, backHeaders(response.headers, true));
        r.res.end(mergeReport(Buffer.concat(chunks), refused, sideBand));
      });
      up.write(lines);
      if (size > 0) await pipeline(createReadStream(pack), up).catch(() => up.destroy());
      else up.end();
      await done;
      if (head.commands.length > 0) log(refused);
    } finally {
      await rm(folder, { recursive: true, force: true });
    }
  }

  return {
    id: "git" as const,

    claims(req: IncomingMessage, door: Door): boolean {
      if (door !== "http" || !req.url?.startsWith("http://")) return false;
      const url = URL.canParse(req.url) ? new URL(req.url) : undefined;
      return url?.hostname === "github.com" && (url.port === "" || url.port === "80");
    },

    async serve(r: GitRequest, ctx: GitContext): Promise<void> {
      const url = new URL(r.req.url ?? "", "http://github.com");
      const method = r.req.method ?? "GET";
      const match = PATH.exec(url.pathname);
      const service = url.searchParams.get("service");
      const [, owner = "", name = "", tail] = match ?? [];
      const kind =
        tail === "info/refs" && method === "GET" && (service === "git-upload-pack" || service === "git-receive-pack") ? service
        : tail !== "info/refs" && method === "POST" ? tail
        : undefined;
      if (!kind || name === "." || name === "..") {
        r.req.resume();
        return ctx.refuse(r.res, 403, url.pathname.includes("/info/lfs") ? lfsRefusal : pathRefusal(url.pathname), "git");
      }
      const path = tail === "info/refs" ? `/${owner}/${name}.git/info/refs?service=${kind}` : `/${owner}/${name}.git/${kind}`;
      const credential = await ctx.credential("github");
      if (!credential) {
        r.req.resume();
        return ctx.refuse(r.res, 403, NO_SIGNIN_REFUSAL, "git");
      }
      if (kind === "git-upload-pack") return fetchRoad(r, ctx, path, credential.token);
      const record = (await ctx.workspace(r.workspaceId)).branchOfRecord;
      if (!record) {
        r.req.resume();
        return ctx.refuse(r.res, 403, NO_BRANCH_REFUSAL, "git");
      }
      const id = await repoId(owner, name, credential.token, record);
      if (typeof id !== "number" || id !== record.repoId) {
        r.req.resume();
        const no = typeof id === "number" ? { status: 403 as const, said: pushRepoRefusal(owner, name, record) } : id;
        return ctx.refuse(r.res, no.status, no.said, "git");
      }
      if (tail === "info/refs") return advertise(r, ctx, path, credential.token);
      return push(r, ctx, path, credential.token, { owner, name, id }, record);
    },
  };
}
