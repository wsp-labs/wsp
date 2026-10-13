// SPDX-License-Identifier: AGPL-3.0-only
import { request as httpRequest, type IncomingMessage, type ServerResponse } from "node:http";
import { request as httpsRequest } from "node:https";
import { pipeline } from "node:stream";
import { computerNameHere } from "../places/this-computer.js";
import { judgeGraphql } from "./graphql.js";
import { type BranchOfRecord, canonicalPath, type Decision, type GhThread, judgeField, judgeRest, type Lookups, type NodeFacts, type Repo } from "./writes.js";

/** The person's gh token as the broker hands it over, read live from `gh auth token` on the host. */
export interface GithubCredential {
  token: string;
}

/** The parts of the broker's request and context this service reads, written here so it imports no broker. */
export interface GhRequest {
  workspaceId: string;
  req: IncomingMessage;
  res: ServerResponse;
}
export interface GhContext {
  credential(kind: "github"): Promise<GithubCredential | undefined>;
  workspace(id: string): Promise<{ branchOfRecord?: BranchOfRecord; repo: { owner: string; name: string; id: number }; trackers: string[] }>;
  refuse(res: ServerResponse, status: 403 | 503, said: string, shape: "github-rest" | "github-graphql"): void;
}

const API_HOSTS = new Set(["api.github.com", "uploads.github.com"]);
const STORAGE_HOSTS = new Set(["release-assets.githubusercontent.com", "objects.githubusercontent.com", "codeload.github.com"]);

/** GitHub's storage hosts that gh's redirects name (release assets, archives, Actions logs and artifacts), which the
 * host fetches with no credential. Measured with gh 2.97.0: `run view --log` lands on results-receiver and on a
 * productionresultssa blob host, `run download` on a blob host, `release download` on release-assets and codeload. */
export function isStorageHost(host: string): boolean {
  return STORAGE_HOSTS.has(host) || host.endsWith(".actions.githubusercontent.com") || /^productionresultssa\d+\.blob\.core\.windows\.net$/.test(host);
}

const DROPPED = new Set(["authorization", "x-api-key", "cookie", "proxy-authorization", "host"]);
// Hop-by-hop headers describe one connection; the client's and the upstream's are not the same connection.
const HOP = new Set(["connection", "keep-alive", "proxy-connection", "te", "trailer", "transfer-encoding", "upgrade"]);
/** A body the road reads to judge (a GraphQL document, a pull request create) larger than this is refused unread. */
const JUDGED_BODY_MAX = 4 << 20;

function headersOut(raw: readonly string[], host: string, token?: string, length?: number): string[] {
  const out = ["Host", host];
  for (let i = 0; i < raw.length; i += 2) {
    const name = raw[i]!.toLowerCase();
    if (DROPPED.has(name) || HOP.has(name) || (length !== undefined && name === "content-length")) continue;
    out.push(raw[i]!, raw[i + 1]!);
  }
  if (token !== undefined) out.push("Authorization", `token ${token}`);
  if (length !== undefined) out.push("Content-Length", String(length));
  return out;
}

function withoutHop(raw: readonly string[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < raw.length; i += 2) if (!HOP.has(raw[i]!.toLowerCase())) out.push(raw[i]!, raw[i + 1]!);
  return out;
}

class TooLarge extends Error {}

function readBody(req: IncomingMessage): Promise<Buffer> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let size = 0;
    req.on("data", (chunk: Buffer) => {
      size += chunk.length;
      if (size > JUDGED_BODY_MAX) {
        req.removeAllListeners("data");
        req.resume();
        reject(new TooLarge());
      } else chunks.push(chunk);
    });
    req.on("end", () => resolve(Buffer.concat(chunks)));
    req.on("error", reject);
  });
}

const NODE_QUERY =
  "query($id:ID!){node(id:$id){__typename ... on PullRequest{headRefName headRepository{databaseId} repository{databaseId nameWithOwner}} ... on Issue{repository{databaseId nameWithOwner}} ... on Repository{databaseId nameWithOwner}}}";

interface NodeRepo {
  databaseId?: number;
  nameWithOwner?: string;
}

const repoFrom = (r: NodeRepo | null | undefined): Repo | undefined =>
  typeof r?.databaseId === "number" && typeof r.nameWithOwner === "string" ? { id: r.databaseId, nameWithOwner: r.nameWithOwner } : undefined;

export interface GhRoadOptions {
  /** Where a host's requests go, as an origin URL; `https://<host>` unless a test points it elsewhere. */
  origin?: (host: string) => string;
  computer?: () => string;
}

/** gh's calls from a brokered machine, on the gh door. The host routes by `Host`: GitHub's API with the
 * person's token, GitHub's storage hosts with none, anything else refused. Reads pass; a write passes only on the
 * write list, judged on the parsed GraphQL document or the canonical REST path, its target resolved by node id. */
export function ghRoad(options: GhRoadOptions = {}) {
  const origin = options.origin ?? ((host: string) => `https://${host}`);
  const computer = options.computer ?? computerNameHere;

  /** One request to GitHub, streamed back to the client as it came: status, headers (SSO, scopes, Link and the
   * rate limits among them) and body. A redirect goes back unfollowed; a 401 never reaches gh, which would ask for
   * a login the machine has nothing to make. */
  function forward(r: GhRequest, host: string, path: string, token: string | undefined, body: Buffer | undefined, shape: "github-rest" | "github-graphql", ctx: GhContext): Promise<void> {
    const { req, res } = r;
    const target = new URL(`${origin(host).replace(/\/+$/, "")}${path}`);
    const send = target.protocol === "https:" ? httpsRequest : httpRequest;
    return new Promise<void>(done => {
      const out = send(target, { method: req.method, headers: headersOut(req.rawHeaders, target.host, token, body?.length), setHost: false }, up => {
        if (up.statusCode === 401) {
          up.resume();
          ctx.refuse(res, 403, `wsp: GitHub refused the gh sign-in on ${computer()}. Run gh auth login there.`, shape);
          done();
          return;
        }
        res.writeHead(up.statusCode ?? 502, up.statusMessage, withoutHop(up.rawHeaders));
        pipeline(up, res, () => done());
      });
      out.on("error", () => {
        if (!res.headersSent) ctx.refuse(res, 503, `wsp: ${host} did not answer ${computer()}. gh can try again.`, shape);
        else if (!res.writableEnded) res.destroy();
        done();
      });
      res.on("close", () => {
        if (!res.writableFinished) out.destroy();
      });
      if (body !== undefined) out.end(body);
      else pipeline(req, out, () => {});
    });
  }

  /** The reads the write list makes with the person's token, never through the client's request. */
  function lookups(token: string): Lookups {
    const call = (path: string, init: { method: string; body?: string }): Promise<{ status: number; json: unknown; location?: string }> =>
      new Promise((resolve, reject) => {
        const target = new URL(`${origin("api.github.com").replace(/\/+$/, "")}${path}`);
        const send = target.protocol === "https:" ? httpsRequest : httpRequest;
        const headers: Record<string, string> = { Host: target.host, Authorization: `token ${token}`, Accept: "application/vnd.github+json", "User-Agent": "wsp" };
        if (init.body !== undefined) headers["Content-Type"] = "application/json";
        const out = send(target, { method: init.method, headers, setHost: false }, up => {
          const chunks: Buffer[] = [];
          up.on("data", (chunk: Buffer) => chunks.push(chunk));
          up.on("end", () => {
            let json: unknown;
            try {
              json = JSON.parse(Buffer.concat(chunks).toString("utf8"));
            } catch {
              json = undefined;
            }
            resolve({ status: up.statusCode ?? 0, json, location: up.headers.location });
          });
          up.on("error", reject);
        });
        out.on("error", reject);
        out.end(init.body);
      });
    const restRepo = async (path: string): Promise<Repo | undefined> => {
      let answer = await call(path, { method: "GET" });
      // A renamed repo's old name answers 301 to /repositories/<id>, which is the same repo.
      if (answer.status === 301 && answer.location !== undefined) {
        const moved = /\/repositories\/(\d+)$/.exec(new URL(answer.location, "https://api.github.com").pathname);
        if (moved !== null) answer = await call(`/repositories/${moved[1]}`, { method: "GET" });
      }
      if (answer.status === 404) return undefined;
      if (answer.status !== 200) throw new Error(`GitHub answered ${answer.status}`);
      const json = answer.json as { id?: unknown; full_name?: unknown };
      return typeof json.id === "number" && typeof json.full_name === "string" ? { id: json.id, nameWithOwner: json.full_name } : undefined;
    };
    return {
      async node(id: string): Promise<NodeFacts | undefined> {
        const answer = await call("/graphql", { method: "POST", body: JSON.stringify({ query: NODE_QUERY, variables: { id } }) });
        if (answer.status !== 200) throw new Error(`GitHub answered ${answer.status}`);
        const node = (answer.json as { data?: { node?: Record<string, unknown> | null } } | undefined)?.data?.node;
        if (node === undefined || node === null) return undefined;
        const typename = String(node.__typename);
        if (typename === "Repository") {
          const repo = repoFrom(node as NodeRepo);
          return repo === undefined ? undefined : { type: "Repository", repo };
        }
        const repo = repoFrom(node.repository as NodeRepo | undefined);
        if (repo === undefined) return { type: "other", typename };
        if (typename === "Issue") return { type: "Issue", repo };
        if (typename === "PullRequest") {
          const headRepoId = (node.headRepository as NodeRepo | null | undefined)?.databaseId;
          return { type: "PullRequest", headRefName: String(node.headRefName), ...(typeof headRepoId === "number" ? { headRepoId } : {}), repo };
        }
        return { type: "other", typename };
      },
      repo: (owner, name) => restRepo(`/repos/${encodeURIComponent(owner)}/${encodeURIComponent(name)}`),
      repoById: id => restRepo(`/repositories/${id}`),
    };
  }

  async function serveApi(r: GhRequest, host: string, ctx: GhContext): Promise<void> {
    const { req, res } = r;
    const url = req.url ?? "/";
    const segments = canonicalPath(url);
    const graphql = segments?.length === 1 && segments[0] === "graphql";
    const shape = graphql ? "github-graphql" : "github-rest";
    const stop = (status: 403 | 503, said: string): void => {
      req.resume();
      ctx.refuse(res, status, said, shape);
    };
    if (segments === undefined) return stop(403, `wsp: ${url} is not a path GitHub serves, so a cloud thread cannot send it through gh.`);
    const credential = await ctx.credential("github");
    if (credential === undefined) return stop(403, `wsp: gh is not signed in on ${computer()}. Run gh auth login there.`);
    const method = req.method ?? "GET";
    if (graphql && method === "GET") return stop(403, "wsp: GraphQL over GET is refused; gh sends its documents with POST.");
    if (!graphql && (method === "GET" || method === "HEAD")) return forward(r, host, url, credential.token, undefined, shape, ctx);
    const query = url.includes("?") ? url.slice(url.indexOf("?")) : "";
    const path = `/${segments.map(encodeURIComponent).join("/")}${query}`;
    let body: Buffer | undefined;
    const read = async (): Promise<Buffer> => (body ??= await readBody(req));
    let decision: Decision;
    try {
      const thread: GhThread = await ctx.workspace(r.workspaceId);
      const look = lookups(credential.token);
      if (graphql) {
        if (method !== "POST") return stop(403, `wsp: GraphQL takes POST, not ${method}.`);
        const verdict = judgeGraphql((await read()).toString("utf8"));
        if (verdict.kind === "refused") decision = { ok: false, said: verdict.said };
        else if (verdict.kind === "read") decision = { ok: true };
        else {
          decision = { ok: true };
          for (const field of verdict.fields) {
            decision = await judgeField(field, thread, look);
            if (!decision.ok) break;
          }
        }
      } else {
        decision = await judgeRest(method, segments, async () => JSON.parse((await read()).toString("utf8") || "null"), thread, look);
      }
    } catch (error) {
      if (error instanceof TooLarge) return stop(403, "wsp: this request is too large for the host to judge, so it is refused.");
      if (error instanceof SyntaxError) return stop(403, "wsp: this request's body is not JSON, so the host cannot judge it.");
      return stop(503, `wsp: GitHub did not answer ${computer()} while it checked this write. gh can try again.`);
    }
    if (!decision.ok) return stop(403, decision.said);
    return forward(r, host, path, credential.token, body, shape, ctx);
  }

  return {
    id: "gh" as const,
    claims(_req: IncomingMessage, door: "http" | "gh"): boolean {
      return door === "gh";
    },
    async serve(r: GhRequest, ctx: GhContext): Promise<void> {
      const host = (r.req.headers.host ?? "").toLowerCase().replace(/:\d+$/, "");
      if (API_HOSTS.has(host)) return serveApi(r, host, ctx);
      const storage = isStorageHost(host);
      if (storage && (r.req.method === "GET" || r.req.method === "HEAD") && r.req.url?.startsWith("/")) return forward(r, host, r.req.url, undefined, undefined, "github-rest", ctx);
      r.req.resume();
      const said = storage ? `wsp: a cloud thread only downloads from ${host}.` : `wsp: ${host || "a request with no host"} is not a GitHub host gh reaches from a cloud thread.`;
      ctx.refuse(r.res, 403, said, "github-rest");
    },
  };
}
