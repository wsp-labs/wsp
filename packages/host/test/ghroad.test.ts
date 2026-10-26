// SPDX-License-Identifier: AGPL-3.0-only
import { readFileSync } from "node:fs";
import { createServer, request, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import type { AddressInfo } from "node:net";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { type GhContext, ghRoad } from "../src/ghroad/api.js";
import type { BranchOfRecord } from "@wsp/protocol";

interface Recorded { label: string; method: string; host: string; path: string; body?: string; read: boolean }
const GH = JSON.parse(readFileSync(new URL("./ghroad-gh.json", import.meta.url), "utf8")) as { gh: string; requests: Recorded[] };
const GITHUB = JSON.parse(readFileSync(new URL("./ghroad-github-writes.json", import.meta.url), "utf8")) as { mutations: string[]; rest: string[] };

const TOKEN = "gho_person";
const PLACEHOLDER = "wsp-turn-placeholder";
const OWN: BranchOfRecord = { repoId: 101, owner: "acme", name: "lab", branch: "wsp/fix-login" };
const LAB = { id: 101, nameWithOwner: "acme/lab" };
const MAP = { id: 202, nameWithOwner: "acme/map" };
const OTHER = { id: 303, nameWithOwner: "acme/other" };
const FORK = { id: 404, nameWithOwner: "someone/lab" };
const pull = (head: string, headRepo: { id: number }, repo = LAB) => ({ __typename: "PullRequest", headRefName: head, headRepository: { databaseId: headRepo.id }, repository: { databaseId: repo.id, nameWithOwner: repo.nameWithOwner } });
const issue = (repo: { id: number; nameWithOwner: string }) => ({ __typename: "Issue", repository: { databaseId: repo.id, nameWithOwner: repo.nameWithOwner } });
const repository = (repo: { id: number; nameWithOwner: string }) => ({ __typename: "Repository", databaseId: repo.id, nameWithOwner: repo.nameWithOwner });
const NODES: Record<string, unknown> = {
  PR_acmelab7: pull("wsp/fix-login", LAB),
  PR_acmelab8: pull("wsp/fix-login", LAB),
  PR_acmelab9: pull("main", LAB),
  PR_fork: pull("wsp/fix-login", FORK),
  I_acmelab3: issue(LAB),
  I_acmemap3: issue(MAP),
  I_other: issue(OTHER),
  R_acmelab: repository(LAB),
  R_other: repository(OTHER),
  R_fork: repository(FORK),
};
const REPOS: Record<string, { id: number; nameWithOwner: string }> = { "/repos/acme/map": MAP, "/repos/acme/other": OTHER, "/repositories/101": LAB, "/repositories/202": MAP, "/repositories/303": OTHER };

interface Seen { host: string; method: string; path: string; authorization?: string; cookie?: string; body: string }

const servers: Server[] = [];
let seen: Seen[];
let looked: string[];
let road: string;
let thread: { branchOfRecord?: BranchOfRecord; repo: { owner: string; name: string; id: number }; trackers: string[] };
let credential: { token: string } | undefined;

async function listen(handler: (req: IncomingMessage, res: ServerResponse) => void): Promise<number> {
  const server = createServer(handler);
  servers.push(server);
  await new Promise<void>(done => server.listen(0, "127.0.0.1", done));
  return (server.address() as AddressInfo).port;
}

const json = (res: ServerResponse, status: number, value: unknown, headers: Record<string, string> = {}) => {
  res.writeHead(status, { "content-type": "application/json", ...headers });
  res.end(JSON.stringify(value));
};

/** GitHub as the road meets it: the reads the write list makes (sent as wsp) answered from the tables above, and
 * everything a client's request carried kept, so a test reads exactly what reached GitHub. */
function github(req: IncomingMessage, res: ServerResponse): void {
  const chunks: Buffer[] = [];
  req.on("data", chunk => chunks.push(chunk));
  req.on("end", () => {
    const [, , host, ...rest] = req.url!.split("/");
    const path = `/${rest.join("/")}`;
    const body = Buffer.concat(chunks).toString("utf8");
    if (req.headers["user-agent"] === "wsp") {
      looked.push(`${req.method} ${path}`);
      if (path === "/graphql") return json(res, 200, { data: { node: NODES[(JSON.parse(body) as { variables: { id: string } }).variables.id] ?? null } });
      if (path === "/repos/acme/old-lab") return json(res, 301, { message: "Moved Permanently" }, { location: "https://api.github.com/repositories/101" });
      const repo = REPOS[path];
      return repo === undefined ? json(res, 404, { message: "Not Found" }) : json(res, 200, { id: repo.id, full_name: repo.nameWithOwner });
    }
    seen.push({ host: host!, method: req.method!, path, authorization: req.headers.authorization, cookie: req.headers.cookie, body });
    if (path === "/redirect") return json(res, 302, {}, { location: "https://release-assets.githubusercontent.com/asset" });
    if (path === "/unauthorized") return json(res, 401, { message: "Bad credentials" });
    json(res, 200, { ok: true }, { "x-github-sso": "required; url=https://github.com/orgs/acme/sso", "x-oauth-scopes": "repo, workflow", link: '<https://api.github.com/x?page=2>; rel="next"', "x-ratelimit-remaining": "4999" });
  });
}

/** The broker's side, as B1 hands a request over: the person's token, the thread's record and the two refusal
 * shapes gh prints. */
const context: GhContext = {
  credential: async () => credential,
  workspace: async id => {
    expect(id).toBe("w1");
    return thread;
  },
  refuse(res, status, said, shape) {
    if (shape === "github-graphql") json(res, 200, { errors: [{ type: "FORBIDDEN", message: said }] });
    else json(res, status, { message: said });
  },
};

beforeEach(async () => {
  seen = [];
  looked = [];
  credential = { token: TOKEN };
  thread = { branchOfRecord: OWN, repo: { owner: "acme", name: "lab", id: 101 }, trackers: ["acme/map"] };
  const upstream = await listen(github);
  const service = ghRoad({ origin: host => `http://127.0.0.1:${upstream}/_/${host}`, computer: () => "acme's MacBook Pro" });
  road = String(await listen((req, res) => void service.serve({ workspaceId: "w1", req, res }, context)));
});

afterEach(async () => {
  await Promise.all(servers.splice(0).map(server => new Promise(done => {
    server.closeAllConnections();
    server.close(done);
  })));
});

interface Got { status: number; headers: IncomingMessage["headers"]; body: string }

function send(method: string, path: string, body?: unknown, host = "api.github.com", headers: Record<string, string> = {}): Promise<Got> {
  const payload = body === undefined ? undefined : typeof body === "string" ? body : JSON.stringify(body);
  return new Promise((resolve, reject) => {
    const req = request({ host: "127.0.0.1", port: road, method, path, headers: { host, authorization: `token ${PLACEHOLDER}`, "content-type": "application/json", ...(payload === undefined ? {} : { "content-length": String(Buffer.byteLength(payload)) }), ...headers } }, res => {
      const chunks: Buffer[] = [];
      res.on("data", chunk => chunks.push(chunk));
      res.on("end", () => resolve({ status: res.statusCode!, headers: res.headers, body: Buffer.concat(chunks).toString("utf8") }));
    });
    req.on("error", reject);
    req.end(payload);
  });
}

const graphql = (query: string, variables?: unknown) => send("POST", "/graphql", variables === undefined ? { query } : { query, variables });
const mutation = (field: string, input: Record<string, unknown>) => graphql(`mutation Do($input: ${field[0]!.toUpperCase()}${field.slice(1)}Input!) { ${field}(input: $input) { clientMutationId } }`, { input });

/** What gh prints for a refusal: GraphQL's error message, or REST's message under a 403; undefined when it passed. */
function refusal(got: Got): string | undefined {
  const body = JSON.parse(got.body) as { errors?: Array<{ type: string; message: string }>; message?: string };
  if (body.errors?.[0]?.type === "FORBIDDEN") return body.errors[0].message;
  if (got.status === 403) return body.message;
  return undefined;
}

describe("the gh road refuses every write on the never list", () => {
  const NEVER: ReadonlyArray<{ what: string; rest: Array<[string, string, unknown?, string?]>; graphql: Array<[string, Record<string, unknown>]> }> = [
    { what: "merge pull requests", rest: [["PUT", "/repos/acme/lab/pulls/7/merge"], ["POST", "/repos/acme/lab/merges", { base: "main", head: "wsp/fix-login" }]], graphql: [["mergePullRequest", { pullRequestId: "PR_acmelab7" }], ["mergeBranch", { repositoryId: "R_acmelab", base: "main", head: "wsp/fix-login" }]] },
    { what: "auto-merge", rest: [], graphql: [["enablePullRequestAutoMerge", { pullRequestId: "PR_acmelab7" }], ["enqueuePullRequest", { pullRequestId: "PR_acmelab7" }]] },
    {
      what: "write refs or file contents",
      rest: [["PATCH", "/repos/acme/lab/git/refs/heads/main", { sha: "0".repeat(40) }], ["POST", "/repos/acme/lab/git/refs", { ref: "refs/heads/x", sha: "0".repeat(40) }], ["PUT", "/repos/acme/lab/contents/README.md", { message: "m", content: "eA==" }]],
      graphql: [["createRef", { repositoryId: "R_acmelab", name: "refs/heads/x", oid: "0".repeat(40) }], ["updateRef", { refId: "REF_x", oid: "0".repeat(40) }], ["updateRefs", { repositoryId: "R_acmelab", refUpdates: [] }], ["createCommitOnBranch", { branch: { branchName: "main" } }]],
    },
    { what: "make releases", rest: [["POST", "/repos/acme/lab/releases", { tag_name: "v9.9.9" }], ["POST", "/repos/acme/lab/releases/1/assets?name=x.zip", "zip", "uploads.github.com"]], graphql: [] },
    { what: "start or re-run workflows", rest: [["POST", "/repos/acme/lab/actions/workflows/42/dispatches", { ref: "main" }], ["POST", "/repos/acme/lab/actions/runs/11/rerun"], ["POST", "/repos/acme/lab/actions/jobs/21/rerun"], ["POST", "/repos/acme/lab/dispatches", { event_type: "x" }]], graphql: [["rerequestCheckSuite", { repositoryId: "R_acmelab", checkSuiteId: "CS_x" }]] },
    { what: "approve pull request reviews", rest: [["POST", "/repos/acme/lab/pulls/7/reviews", { event: "APPROVE" }]], graphql: [["addPullRequestReview", { pullRequestId: "PR_acmelab7", event: "APPROVE" }], ["submitPullRequestReview", { pullRequestReviewId: "PRR_x", event: "APPROVE" }]] },
    { what: "SSH or GPG keys", rest: [["POST", "/user/keys", { key: "ssh-ed25519 AAAA" }], ["POST", "/user/ssh_signing_keys", { key: "ssh-ed25519 AAAA" }]], graphql: [] },
    { what: "deploy keys", rest: [["POST", "/repos/acme/lab/keys", { key: "ssh-ed25519 AAAA", read_only: false }]], graphql: [] },
    { what: "change who has access", rest: [["PUT", "/repos/acme/lab/collaborators/hubot", { permission: "admin" }]], graphql: [["removeOutsideCollaborator", { userId: "U_x", organizationId: "O_x" }]] },
    { what: "change webhooks", rest: [["POST", "/repos/acme/lab/hooks", { config: { url: "https://example.test" } }], ["POST", "/orgs/acme/hooks", { name: "web" }]], graphql: [] },
    { what: "set or delete secrets", rest: [["PUT", "/repos/acme/lab/actions/secrets/NPM_TOKEN", { encrypted_value: "x" }], ["PUT", "/repos/acme/lab/environments/prod/secrets/KEY", { encrypted_value: "x" }]], graphql: [] },
    { what: "rulesets or branch protection", rest: [["POST", "/repos/acme/lab/rulesets", { name: "x" }], ["PUT", "/repos/acme/lab/branches/main/protection", {}]], graphql: [["createRepositoryRuleset", { sourceId: "R_acmelab" }], ["deleteBranchProtectionRule", { branchProtectionRuleId: "BPR_x" }]] },
    { what: "change repository settings", rest: [["PATCH", "/repos/acme/lab", { default_branch: "wsp/fix-login" }], ["POST", "/repos/acme/lab/transfer", { new_owner: "someone" }], ["PUT", "/repos/acme/lab/topics", { names: [] }]], graphql: [["updateRepository", { repositoryId: "R_acmelab", name: "x" }], ["archiveRepository", { repositoryId: "R_acmelab" }]] },
    { what: "delete things on GitHub", rest: [["DELETE", "/repos/acme/lab"], ["DELETE", "/repos/acme/lab/issues/comments/9"]], graphql: [["deleteIssue", { issueId: "I_acmelab3" }], ["deleteIssueComment", { id: "IC_x" }]] },
  ];

  for (const entry of NEVER) {
    it(`${entry.what}: refused through REST and GraphQL with a sentence naming it`, async () => {
      for (const [method, path, body, host] of entry.rest) {
        const said = refusal(await send(method, path, body, host));
        expect(said, `${method} ${path}`).toContain(entry.what);
        expect(said).toContain(`${method} ${path.split("?")[0]}`);
      }
      for (const [field, input] of entry.graphql) {
        const said = refusal(await mutation(field, input));
        expect(said, field).toContain(entry.what);
        expect(said).toContain(field);
      }
      expect(seen).toEqual([]);
    });
  }

  it("GitHub's schema has no mutation for releases, SSH keys, deploy keys, webhooks or secrets, so those are refused as REST", () => {
    expect(GITHUB.mutations.filter(name => /Release|SshKey|SshSigningKey|GpgKey|Hook|Secret/.test(name))).toEqual([]);
    expect(GITHUB.mutations.filter(name => /DeployKey/.test(name))).toEqual(["updateEnterpriseDeployKeySetting"]);
  });

  it("finds a mutation behind a fragment spread, an inline fragment, an alias, a misleading name or a spread doubled 40 times", async () => {
    const hidden = [
      'fragment PullRequestFields on Mutation { mergePullRequest(input: {pullRequestId: "PR_acmelab7"}) { clientMutationId } }\nmutation PullRequestByNumber { ...PullRequestFields }',
      'mutation ViewerQuery { viewer: mergePullRequest(input: {pullRequestId: "PR_acmelab7"}) { clientMutationId } }',
      'mutation { ... on Mutation { mergePullRequest(input: {pullRequestId: "PR_acmelab7"}) { clientMutationId } } }',
      'fragment A on Mutation { ...B }\nfragment B on Mutation { mergePullRequest(input: {pullRequestId: "PR_acmelab7"}) { clientMutationId } }\nmutation CommentCreate { ...A }',
      'mutation CommentCreate { addComment(input: {subjectId: "PR_acmelab7", body: "hi"}) { clientMutationId } m: mergePullRequest(input: {pullRequestId: "PR_acmelab7"}) { clientMutationId } }',
      `fragment F0 on Mutation { mergePullRequest(input: {pullRequestId: "PR_acmelab7"}) { clientMutationId } }\n${Array.from({ length: 40 }, (_, i) => `fragment F${i + 1} on Mutation { ...F${i} ...F${i} }`).join("\n")}\nmutation { ...F40 }`,
    ];
    for (const query of hidden) expect(refusal(await graphql(query)), query).toContain("merge pull requests or branches (mergePullRequest)");
    expect(seen).toEqual([]);
  });

  it("refuses what it cannot judge as one document: a query spreading a Mutation fragment, two operations, a batch, GET and noise", async () => {
    const cases: Array<[Promise<Got>, string]> = [
      [graphql('fragment F on Mutation { mergePullRequest(input: {pullRequestId: "PR_acmelab7"}) { clientMutationId } }\nquery Q { ...F }'), "a fragment on Mutation"],
      [send("POST", "/graphql", { query: "query A { viewer { login } }\nmutation B { deleteIssue(input: {issueId: \"I_acmelab3\"}) { clientMutationId } }", operationName: "A" }), "2 operations"],
      [send("POST", "/graphql", [{ query: "query { viewer { login } }" }]), "batch"],
      [send("GET", "/graphql?query=query%20%7Bviewer%7Blogin%7D%7D"), "GET"],
      [graphql("mutation {"), "does not parse"],
      [send("POST", "/graphql", "not json"), "not JSON"],
      [graphql("fragment F on Mutation { ...F }\nmutation { ...F }"), "spreads itself"],
    ];
    for (const [got, words] of cases) expect(refusal(await got), words).toContain(words);
    expect(seen).toEqual([]);
  });

  it("refuses every mutation in GitHub's schema but the write list, naming the field", async () => {
    const listed = new Set(["createPullRequest", "updatePullRequest", "markPullRequestReadyForReview", "convertPullRequestToDraft", "requestReviews", "requestReviewsByLogin", "addComment"]);
    expect(GITHUB.mutations.length).toBeGreaterThan(250);
    for (const name of listed) expect(GITHUB.mutations).toContain(name);
    for (const name of GITHUB.mutations.filter(field => !listed.has(field))) {
      const said = refusal(await mutation(name, { pullRequestId: "PR_acmelab7", subjectId: "PR_acmelab7", repositoryId: "R_acmelab" }));
      expect(said, name).toMatch(new RegExp(`^wsp: a cloud thread cannot .*\\(${name}\\)`));
    }
    expect(seen).toEqual([]);
  });

  it("refuses every REST write GitHub describes but the write list, naming the path", async () => {
    const listed = new Set(["POST /repos/{owner}/{repo}/pulls", "POST /repos/{owner}/{repo}/issues/{issue_number}/comments", "POST /repos/{owner}/{repo}/pulls/{pull_number}/comments", "POST /repos/{owner}/{repo}/pulls/{pull_number}/comments/{comment_id}/replies"]);
    expect(GITHUB.rest.length).toBeGreaterThan(500);
    for (const operation of listed) expect(GITHUB.rest).toContain(operation);
    const fill = (path: string) => path.replace("{owner}", "acme").replace("{repo}", "lab").replace(/\{[a-z_]*(number|id)\}/g, "7").replace(/\{[a-z_]+\}/g, "x");
    for (const operation of GITHUB.rest.filter(op => !listed.has(op))) {
      const [method, path] = operation.split(" ") as [string, string];
      const said = refusal(await send(method, fill(path), { head: "wsp/fix-login", base: "main" }));
      expect(said, operation).toContain(`(${method} ${fill(path)})`);
    }
    expect(seen).toEqual([]);
  });
});

describe(`the gh road passes what gh ${GH.gh} sends for its own commands`, () => {
  const ALLOWED = new Set(["pr-create", "pr-create-draft", "pr-edit", "pr-edit-reviewer", "pr-ready-draft", "pr-ready-undo", "issue-comment", "pr-comment", "api-rest-pr-create", "api-rest-issue-comment"]);
  const REFUSED = new Set(["pr-merge-admin", "pr-merge-auto", "pr-close", "pr-review-approve", "pr-review-comment", "release-create", "api-patch-ref", "workflow-run", "run-rerun", "repo-delete", "repo-edit", "issue-create", "issue-close", "label-create", "variable-set", "deploy-key-add", "ssh-key-add", "api-graphql-merge"]);

  it("passes every read, unchanged but for the credential, and API reads only carry it", async () => {
    const reads = GH.requests.filter(r => r.read);
    expect(new Set(reads.map(r => r.label)).size).toBeGreaterThanOrEqual(20);
    for (const recorded of reads) {
      const got = await send(recorded.method, recorded.path, recorded.body, recorded.host);
      expect(refusal(got), `${recorded.label} ${recorded.method} ${recorded.path}`).toBeUndefined();
      const reached = seen.at(-1)!;
      expect({ host: reached.host, method: reached.method, path: reached.path, body: reached.body }).toEqual({ host: recorded.host, method: recorded.method, path: recorded.path, body: recorded.body ?? "" });
      expect(reached.authorization).toBe(recorded.host.endsWith("api.github.com") ? `token ${TOKEN}` : undefined);
    }
    expect(seen).toHaveLength(reads.length);
  });

  it("passes pr create on the branch of record, pr edit, pr ready, reviewers and comments on a listed tracker", async () => {
    const writes = GH.requests.filter(r => !r.read);
    expect(new Set(writes.map(r => r.label))).toEqual(new Set([...ALLOWED, ...REFUSED]));
    for (const recorded of writes.filter(r => ALLOWED.has(r.label))) {
      const before = seen.length;
      const got = await send(recorded.method, recorded.path, recorded.body, recorded.host);
      expect(refusal(got), recorded.label).toBeUndefined();
      expect(seen.slice(before).map(s => [s.method, s.path, s.body])).toEqual([[recorded.method, recorded.path, recorded.body ?? ""]]);
    }
    expect(looked).toContain("POST /graphql");
  });

  it("refuses the writes gh sends for merge, auto-merge, close, reviews, releases, workflows, settings and keys", async () => {
    for (const recorded of GH.requests.filter(r => !r.read && REFUSED.has(r.label))) {
      expect(refusal(await send(recorded.method, recorded.path, recorded.body, recorded.host)), recorded.label).toMatch(/^wsp: a cloud thread cannot /);
    }
    expect(seen).toEqual([]);
  });
});

describe("the write list holds each write to its target", () => {
  const create = (input: Record<string, unknown>) => mutation("createPullRequest", { repositoryId: "R_acmelab", baseRefName: "main", headRefName: "wsp/fix-login", title: "t", ...input });

  it("opens a pull request only from the branch of record onto the project's repo", async () => {
    expect(refusal(await create({}))).toBeUndefined();
    expect(refusal(await graphql('mutation { createPullRequest(input: {repositoryId: "R_acmelab", baseRefName: "main", headRefName: "wsp/fix-login", title: "t"}) { pullRequest { url } } }'))).toBeUndefined();
    expect(refusal(await create({ headRefName: "main" }))).toBe("wsp: this thread opens pull requests only from its own branch, wsp/fix-login (createPullRequest asked for main).");
    expect(refusal(await create({ headRefName: "someone:wsp/fix-login" }))).toContain("only from its own branch");
    expect(refusal(await create({ headRepositoryId: "R_fork" }))).toContain("only from its own branch");
    expect(refusal(await create({ repositoryId: "R_other" }))).toBe("wsp: this thread opens pull requests only on acme/lab (createPullRequest).");
    expect(refusal(await send("POST", "/repos/acme/lab/pulls", { head: "wsp/fix-login", base: "main" }))).toBeUndefined();
    expect(refusal(await send("POST", "/repos/acme/lab/pulls", { head: "acme:wsp/fix-login", base: "main" }))).toBeUndefined();
    expect(refusal(await send("POST", "/repos/acme/lab/pulls", { head: "main", base: "wsp/fix-login" }))).toContain("asked for main");
    expect(refusal(await send("POST", "/repos/acme/other/pulls", { head: "wsp/fix-login", base: "main" }))).toContain("only on acme/lab");
    expect(seen.map(s => s.path)).toEqual(["/graphql", "/graphql", "/repos/acme/lab/pulls", "/repos/acme/lab/pulls"]);
  });

  it("a thread from a fork's pull request opens it from the fork's branch onto the project", async () => {
    thread.branchOfRecord = { repoId: 404, owner: "someone", name: "lab", branch: "wsp/fix-login" };
    expect(refusal(await create({ headRepositoryId: "R_fork" }))).toBeUndefined();
    expect(refusal(await create({ headRefName: "someone:wsp/fix-login" }))).toBeUndefined();
    expect(refusal(await create({}))).toBe("wsp: this thread opens pull requests only from its own branch, someone:wsp/fix-login (createPullRequest asked for wsp/fix-login).");
    expect(refusal(await mutation("updatePullRequest", { pullRequestId: "PR_fork", title: "t" }))).toBeUndefined();
    expect(refusal(await mutation("updatePullRequest", { pullRequestId: "PR_acmelab7", title: "t" }))).toContain("only pull requests from its own branch");
  });

  it("edits only a pull request from the branch of record, and only its title and body", async () => {
    expect(refusal(await mutation("updatePullRequest", { pullRequestId: "PR_acmelab7", title: "t", body: "b" }))).toBeUndefined();
    expect(refusal(await mutation("markPullRequestReadyForReview", { pullRequestId: "PR_acmelab9" }))).toBe(
      "wsp: this thread changes only pull requests from its own branch, wsp/fix-login (markPullRequestReadyForReview on a pull request from main).",
    );
    expect(refusal(await mutation("requestReviews", { pullRequestId: "PR_fork", userIds: [] }))).toContain("only pull requests from its own branch");
    expect(refusal(await mutation("updatePullRequest", { pullRequestId: "PR_acmelab7", baseRefName: "release" }))).toBe(
      "wsp: a cloud thread changes only a pull request's title and body (updatePullRequest baseRefName). Run it from your own computer.",
    );
    expect(refusal(await mutation("updatePullRequest", { pullRequestId: "PR_acmelab7", state: "CLOSED" }))).toContain("(updatePullRequest state)");
    expect(refusal(await mutation("convertPullRequestToDraft", { pullRequestId: "I_acmelab3" }))).toContain("names no pull request");
  });

  it("comments only on the project's repo and its listed trackers, by REST and GraphQL", async () => {
    expect(refusal(await mutation("addComment", { subjectId: "I_acmemap3", body: "b" }))).toBeUndefined();
    expect(refusal(await mutation("addComment", { subjectId: "PR_acmelab7", body: "b" }))).toBeUndefined();
    expect(refusal(await mutation("addComment", { subjectId: "I_other", body: "b" }))).toBe(
      "wsp: a cloud thread comments only on acme/lab and the trackers listed for its project (addComment on acme/other). Add one with wsp projects set <project> --tracker <owner/name>.",
    );
    expect(refusal(await send("POST", "/repos/acme/map/issues/3/comments", { body: "b" }))).toBeUndefined();
    expect(refusal(await send("POST", "/repos/ACME/Lab/pulls/7/comments", { body: "b", commit_id: "x", path: "a", line: 1 }))).toBeUndefined();
    expect(refusal(await send("POST", "/repos/acme/lab/pulls/7/comments/9/replies", { body: "b" }))).toBeUndefined();
    expect(refusal(await send("POST", "/repos/acme/other/issues/3/comments", { body: "b" }))).toContain("(POST /repos/acme/other/issues/3/comments on acme/other)");
    expect(refusal(await send("POST", "/repos/acme/old-lab/issues/3/comments", { body: "b" }))).toBeUndefined();
    expect(refusal(await send("POST", "/repositories/202/issues/3/comments", { body: "b" }))).toBeUndefined();
    expect(refusal(await send("POST", "/repositories/303/issues/3/comments", { body: "b" }))).toContain("on acme/other");
  });

  it("with no branch of record, opens and edits no pull request but still comments", async () => {
    thread.branchOfRecord = undefined;
    expect(refusal(await create({}))).toBe("wsp: this thread has no branch of record, so it opens and edits no pull requests (createPullRequest).");
    expect(refusal(await mutation("updatePullRequest", { pullRequestId: "PR_acmelab7", title: "t" }))).toContain("no branch of record");
    expect(refusal(await send("POST", "/repos/acme/lab/pulls", { head: "wsp/fix-login", base: "main" }))).toContain("no branch of record");
    expect(refusal(await mutation("addComment", { subjectId: "I_acmelab3", body: "b" }))).toBeUndefined();
  });
});

describe("REST paths are judged and sent as GitHub routes them", () => {
  it("decodes %2F and drops dot segments before judging", async () => {
    expect(refusal(await send("POST", "/repos/acme/lab/issues/1/comments/../../../git/refs", { ref: "refs/heads/main", sha: "0".repeat(40) }))).toBe(
      "wsp: a cloud thread cannot write refs or file contents (POST /repos/acme/lab/git/refs). Run it from your own computer.",
    );
    expect(refusal(await send("POST", "/repos/acme%2Flab/releases", { tag_name: "v1" }))).toContain("make releases, which make tags (POST /repos/acme/lab/releases)");
    expect(refusal(await send("PUT", "/repos/acme/lab/pulls/7/comments/%2e%2e/merge"))).toContain("merge pull requests or branches (PUT /repos/acme/lab/pulls/7/merge)");
    expect(refusal(await send("POST", "/repos/acme/lab/pulls/7/comments/../..", { head: "main" }))).toContain("only from its own branch");
    expect(seen).toEqual([]);
  });

  it("sends a passing write on its canonical path, and maps /repositories/{id} to the same repo", async () => {
    expect(refusal(await send("POST", "/repos/acme/lab/./issues//3/comments?x=1", { body: "b" }))).toBeUndefined();
    expect(refusal(await send("POST", "/repositories/101/pulls", { head: "wsp/fix-login", base: "main" }))).toBeUndefined();
    expect(seen.map(s => s.path)).toEqual(["/repos/acme/lab/issues/3/comments?x=1", "/repositories/101/pulls"]);
  });
});

describe("the gh road routes by Host", () => {
  it("adds the person's token for GitHub's API and drops the placeholder and cookies", async () => {
    await send("GET", "/user", undefined, "api.github.com", { cookie: "user_session=x" });
    await send("GET", "/repos/acme/lab/releases", undefined, "uploads.github.com");
    expect(seen.map(s => [s.host, s.authorization, s.cookie])).toEqual([["api.github.com", `token ${TOKEN}`, undefined], ["uploads.github.com", `token ${TOKEN}`, undefined]]);
  });

  it("fetches GitHub's storage hosts with no credential, and refuses any other host", async () => {
    for (const host of ["release-assets.githubusercontent.com", "objects.githubusercontent.com", "codeload.github.com", "results-receiver.actions.githubusercontent.com", "productionresultssa18.blob.core.windows.net"]) {
      expect(refusal(await send("GET", "/asset", undefined, host)), host).toBeUndefined();
    }
    expect(seen.map(s => s.authorization)).toEqual([undefined, undefined, undefined, undefined, undefined]);
    expect(refusal(await send("POST", "/asset", "x", "release-assets.githubusercontent.com"))).toBe("wsp: a cloud thread only downloads from release-assets.githubusercontent.com.");
    expect(refusal(await send("GET", "/", undefined, "example.com"))).toBe("wsp: example.com is not a GitHub host gh reaches from a cloud thread.");
    expect(refusal(await send("GET", "/", undefined, "evil.blob.core.windows.net"))).toContain("not a GitHub host");
    expect(seen).toHaveLength(5);
  });

  it("passes a redirect unfollowed and GitHub's own headers back unchanged", async () => {
    const moved = await send("GET", "/redirect");
    expect([moved.status, moved.headers.location]).toEqual([302, "https://release-assets.githubusercontent.com/asset"]);
    const got = await send("GET", "/user");
    expect([got.headers["x-github-sso"], got.headers["x-oauth-scopes"], got.headers.link, got.headers["x-ratelimit-remaining"]]).toEqual([
      "required; url=https://github.com/orgs/acme/sso",
      "repo, workflow",
      '<https://api.github.com/x?page=2>; rel="next"',
      "4999",
    ]);
  });

  it("never answers 401, and says where to sign gh in", async () => {
    const rest = await send("GET", "/unauthorized");
    expect([rest.status, refusal(rest)]).toEqual([403, "wsp: GitHub refused the gh sign-in on acme's MacBook Pro. Run gh auth login there."]);
    credential = undefined;
    const graph = await graphql("query { viewer { login } }");
    expect([graph.status, refusal(graph)]).toEqual([200, "wsp: gh is not signed in on acme's MacBook Pro. Run gh auth login there."]);
    expect(seen).toHaveLength(1);
  });

  it("refuses a call gh gave up on only on the response gh closed, which the broker writes no row for", async () => {
    let reached!: () => void;
    const held = new Promise<void>(done => (reached = done));
    const holding = await listen(req => {
      req.resume();
      reached();
    });
    const refusals: { said: string; callerLeft: boolean }[] = [];
    const watched: GhContext = { ...context, refuse: (res, _status, said) => void refusals.push({ said, callerLeft: res.destroyed }) };
    const service = ghRoad({ origin: host => `http://127.0.0.1:${holding}/_/${host}`, computer: () => "acme's MacBook Pro" });
    const port = await listen((req, res) => void service.serve({ workspaceId: "w1", req, res }, watched));
    const call = request({ host: "127.0.0.1", port, method: "GET", path: "/repos/acme/lab", headers: { host: "api.github.com", authorization: `token ${PLACEHOLDER}` } });
    call.on("error", () => undefined);
    call.end();
    await held;
    call.destroy();
    await expect.poll(() => refusals).toEqual([{ said: "wsp: api.github.com did not answer acme's MacBook Pro. gh can try again.", callerLeft: true }]);
  });
});
