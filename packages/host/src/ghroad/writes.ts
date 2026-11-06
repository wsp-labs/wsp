// SPDX-License-Identifier: AGPL-3.0-only
import type { BranchOfRecord } from "@wsp/protocol";
import type { RootField } from "./graphql.js";

/** What the write list reads about the calling thread, from the record the host holds and never from the request. */
export interface GhThread {
  branchOfRecord?: BranchOfRecord;
  repo: { owner: string; name: string; id: number };
  trackers: readonly string[];
}

export interface Repo {
  id: number;
  nameWithOwner: string;
}

/** What a node id names, read from GitHub with the person's token before a write that names its target by id. */
export type NodeFacts =
  | { type: "PullRequest"; headRefName: string; headRepoId?: number; repo: Repo }
  | { type: "Issue"; repo: Repo }
  | { type: "Repository"; repo: Repo }
  | { type: "other"; typename: string };

/** Reads from GitHub the decisions need; each answers undefined for a target GitHub does not know and throws when
 * GitHub did not answer. */
export interface Lookups {
  node(id: string): Promise<NodeFacts | undefined>;
  repo(owner: string, name: string): Promise<Repo | undefined>;
  repoById(id: number): Promise<Repo | undefined>;
}

export type Decision = { ok: true } | { ok: false; said: string };

const ALLOW: Decision = { ok: true };
const no = (said: string): Decision => ({ ok: false, said: `wsp: ${said}` });
const same = (a: string, b: string): boolean => a.toLowerCase() === b.toLowerCase();
const record = (thread: GhThread): string => `${thread.repo.owner}/${thread.repo.name}`;

/** The writes never let through, each kind with the words a refusal names it by; a write on none of them is still
 * refused, since only the write list passes. */
const NEVER: ReadonlyArray<{ phrase: string; graphql?: RegExp; rest?: RegExp }> = [
  { phrase: "merge pull requests or branches", graphql: /^(mergePullRequest|mergeBranch)$/, rest: /^(put repo\/pulls\/[^/]+\/merge(-async)?|post repo\/(merges|merge-upstream))$/ },
  { phrase: "turn on auto-merge or use the merge queue", graphql: /^((enable|disable)PullRequestAutoMerge|(en|de)queuePullRequest)$/ },
  { phrase: "change rulesets or branch protection", graphql: /Ruleset|BranchProtection/, rest: /^\w+ (repo\/(rulesets|rules)|repo\/branches\/.+\/protection|\/orgs\/[^/]+\/rulesets)(\/|$)/ },
  { phrase: "write refs or file contents", graphql: /^((create|update|delete)Refs?|createCommitOnBranch|updatePullRequestBranch|(create|delete)LinkedBranch)$/, rest: /^\w+ repo\/(git\/|contents(\/|$)|pulls\/[^/]+\/update-branch$|branches\/.+\/rename$)/ },
  { phrase: "make releases, which make tags", graphql: /Release/, rest: /^\w+ repo\/(releases|immutable-releases)(\/|$)/ },
  { phrase: "start or re-run workflows", graphql: /^(rerequestCheckSuite|createCheckSuite)$/, rest: /^(post|put) repo\/(actions\/(workflows|runs|jobs)\/|dispatches$|check-suites\/|check-runs\/[^/]+\/rerequest$)/ },
  { phrase: "submit or approve pull request reviews", graphql: /^(add|submit|dismiss|update|delete)PullRequestReview$/, rest: /^\w+ repo\/pulls\/[^/]+\/reviews(\/|$)/ },
  { phrase: "add or remove SSH or GPG keys", rest: /^\w+ \/user\/(keys|ssh_signing_keys|gpg_keys)(\/|$)/ },
  { phrase: "add or remove deploy keys", graphql: /DeployKey/, rest: /^\w+ repo\/keys(\/|$)/ },
  { phrase: "change who has access", graphql: /Collaborator|Invit|Member/, rest: /^\w+ (repo\/(collaborators|invitations)|\/user\/repository_invitations|\/orgs\/[^/]+\/(members|memberships|invitations|outside_collaborators))(\/|$)/ },
  { phrase: "change webhooks", rest: /^\w+ (repo|\/orgs\/[^/]+)\/hooks(\/|$)/ },
  { phrase: "set or delete secrets", rest: /\/secrets(\/|$)/ },
  {
    phrase: "change repository settings",
    graphql: /^(updateRepository|archiveRepository|unarchiveRepository|updateTopics|updateRepositoryWebCommitSignoffSetting|setRepositoryInteractionLimit|setRepositoryCustomPropertyValues|updateTeamsRepository)$/,
    rest: /^(patch|put|post) repo(\/(topics|transfer|vulnerability-alerts|automated-security-fixes|private-vulnerability-reporting|pages|environments|actions\/permissions|interaction-limits|properties|autolinks|lfs)(\/.*)?)?$/,
  },
  { phrase: "delete things on GitHub", graphql: /^delete/, rest: /^delete / },
];

function never(name: string, kind: "graphql" | "rest", key: string): Decision {
  const phrase = NEVER.find(entry => entry[kind]?.test(key))?.phrase ?? "make this write";
  return no(`a cloud thread cannot ${phrase} (${name}). Run it from your own computer.`);
}

/** A REST path as GitHub routes it: decoded once (`%2F` is a slash), empty and dot segments dropped. Undefined for
 * a target that is not a path, or one that does not decode. */
export function canonicalPath(url: string): string[] | undefined {
  const raw = url.split(/[?#]/)[0]!;
  if (!raw.startsWith("/")) return undefined;
  let decoded: string;
  try {
    decoded = decodeURIComponent(raw);
  } catch {
    return undefined;
  }
  const out: string[] = [];
  for (const segment of decoded.split(/[/\\]/)) {
    if (segment === "" || segment === ".") continue;
    if (segment === "..") out.pop();
    else out.push(segment);
  }
  return out;
}

/** The repo a REST path is under, `/repos/{o}/{r}/...` or `/repositories/{id}/...` alike, and the path past it. */
function repoPath(segments: readonly string[]): { owner: string; name: string } | { id: number } | undefined {
  if (segments[0]?.toLowerCase() === "repos" && segments.length >= 3) return { owner: segments[1]!, name: segments[2]! };
  if (segments[0]?.toLowerCase() === "repositories" && /^\d+$/.test(segments[1] ?? "")) return { id: Number(segments[1]) };
  return undefined;
}

async function repoOf(at: { owner: string; name: string } | { id: number }, thread: GhThread, look: Lookups): Promise<Repo | undefined> {
  if ("id" in at) return at.id === thread.repo.id ? { id: thread.repo.id, nameWithOwner: record(thread) } : look.repoById(at.id);
  if (same(`${at.owner}/${at.name}`, record(thread))) return { id: thread.repo.id, nameWithOwner: record(thread) };
  return look.repo(at.owner, at.name);
}

const listed = (repo: Repo, thread: GhThread): boolean => repo.id === thread.repo.id || thread.trackers.some(tracker => same(tracker, repo.nameWithOwner));

function commentOn(repo: Repo | undefined, name: string, thread: GhThread): Decision {
  if (repo !== undefined && listed(repo, thread)) return ALLOW;
  const target = repo?.nameWithOwner ?? "a repo GitHub did not name";
  return no(`a cloud thread comments only on ${record(thread)} and the trackers listed for its project (${name} on ${target}). Add one with wsp projects set <project> --tracker <owner/name>.`);
}

function own(thread: GhThread): string {
  const bor = thread.branchOfRecord!;
  return bor.repoId === thread.repo.id ? bor.branch : `${bor.owner}:${bor.branch}`;
}

const noRecord = (name: string): Decision => no(`this thread has no branch of record, so it opens and edits no pull requests (${name}).`);

/** A pull request's head, as the create names it, against the branch of record: the head repo by id where the
 * request names one by id, by owner where it names `owner:branch`, and the base repo where it names neither. */
function headIs(thread: GhThread, branch: string, repo: { id?: number; owner?: string; name?: string }): boolean {
  const bor = thread.branchOfRecord!;
  if (branch !== bor.branch) return false;
  if (repo.id !== undefined) return repo.id === bor.repoId;
  if (repo.owner !== undefined) return same(repo.owner, bor.owner) && (repo.name === undefined || same(repo.name, bor.name));
  return bor.repoId === thread.repo.id;
}

const headMismatch = (thread: GhThread, name: string, asked: string): Decision =>
  no(`this thread opens pull requests only from its own branch, ${own(thread)} (${name} asked for ${asked}).`);
const baseMismatch = (thread: GhThread, name: string): Decision => no(`this thread opens pull requests only on ${record(thread)} (${name}).`);

const text = (value: unknown): string | undefined => (typeof value === "string" ? value : undefined);

async function ownPull(id: string | undefined, name: string, thread: GhThread, look: Lookups): Promise<Decision> {
  if (thread.branchOfRecord === undefined) return noRecord(name);
  const node = id === undefined ? undefined : await look.node(id);
  if (node?.type !== "PullRequest") return no(`${name} names no pull request GitHub knows, so it is refused.`);
  const bor = thread.branchOfRecord;
  if (node.headRefName === bor.branch && node.headRepoId === bor.repoId) return ALLOW;
  return no(`this thread changes only pull requests from its own branch, ${own(thread)} (${name} on a pull request from ${node.headRefName}).`);
}

const PULL_EDITS = new Set(["updatePullRequest", "markPullRequestReadyForReview", "convertPullRequestToDraft", "requestReviews", "requestReviewsByLogin"]);
const TITLE_AND_BODY = new Set(["pullRequestId", "title", "body", "clientMutationId"]);

/** One root field of a mutation against the write list, its target resolved by node id first. */
export async function judgeField(field: RootField, thread: GhThread, look: Lookups): Promise<Decision> {
  const raw = field.args.input;
  const input = typeof raw === "object" && raw !== null && !Array.isArray(raw) ? (raw as Record<string, unknown>) : {};
  if (field.name === "createPullRequest") {
    if (thread.branchOfRecord === undefined) return noRecord(field.name);
    const repositoryId = text(input.repositoryId);
    const base = repositoryId === undefined ? undefined : await look.node(repositoryId);
    if (base?.type !== "Repository" || base.repo.id !== thread.repo.id) return baseMismatch(thread, field.name);
    const head = text(input.headRefName) ?? "";
    const [owner, branch] = head.includes(":") ? [head.slice(0, head.indexOf(":")), head.slice(head.indexOf(":") + 1)] : [undefined, head];
    const headRepositoryId = text(input.headRepositoryId);
    let headId: number | undefined;
    if (headRepositoryId !== undefined) {
      const node = await look.node(headRepositoryId);
      if (node?.type !== "Repository") return headMismatch(thread, field.name, head);
      headId = node.repo.id;
    }
    return headIs(thread, branch, { id: headId, owner }) ? ALLOW : headMismatch(thread, field.name, head);
  }
  if (PULL_EDITS.has(field.name)) {
    const extra = field.name === "updatePullRequest" ? Object.keys(input).find(key => !TITLE_AND_BODY.has(key)) : undefined;
    if (extra !== undefined) return no(`a cloud thread changes only a pull request's title and body (updatePullRequest ${extra}). Run it from your own computer.`);
    return ownPull(text(input.pullRequestId), field.name, thread, look);
  }
  if (field.name === "addComment") {
    const subjectId = text(input.subjectId);
    const node = subjectId === undefined ? undefined : await look.node(subjectId);
    return commentOn(node !== undefined && node.type !== "other" ? node.repo : undefined, field.name, thread);
  }
  return never(field.name, "graphql", field.name);
}

const COMMENT_PATHS = [/^issues\/\d+\/comments$/, /^pulls\/\d+\/comments$/, /^pulls\/\d+\/comments\/\d+\/replies$/];

/** One REST write against the write list. `body` reads the request's JSON body; only the pull request create asks. */
export async function judgeRest(method: string, segments: readonly string[], body: () => Promise<unknown>, thread: GhThread, look: Lookups): Promise<Decision> {
  const at = repoPath(segments);
  const tail = at === undefined ? [] : segments.slice("id" in at ? 2 : 3);
  const shown = `${method} /${segments.join("/")}`;
  const key = (at === undefined ? `${method} /${segments.join("/")}` : `${method} repo${tail.length > 0 ? `/${tail.join("/")}` : ""}`).toLowerCase();
  const rest = tail.join("/").toLowerCase();
  if (at !== undefined && method === "POST" && COMMENT_PATHS.some(path => path.test(rest))) return commentOn(await repoOf(at, thread, look), shown, thread);
  if (at !== undefined && method === "POST" && rest === "pulls") {
    if (thread.branchOfRecord === undefined) return noRecord(shown);
    const base = await repoOf(at, thread, look);
    if (base?.id !== thread.repo.id) return baseMismatch(thread, shown);
    const json = await body();
    const fields = typeof json === "object" && json !== null ? (json as Record<string, unknown>) : {};
    const head = text(fields.head) ?? "";
    const headRepo = text(fields.head_repo);
    const [owner, branch] = head.includes(":") ? [head.slice(0, head.indexOf(":")), head.slice(head.indexOf(":") + 1)] : [undefined, head];
    const pathOwner = "id" in at ? thread.repo.owner : at.owner;
    const repo = owner !== undefined ? { owner, name: headRepo } : headRepo !== undefined ? { owner: pathOwner, name: headRepo } : {};
    return headIs(thread, branch, repo) ? ALLOW : headMismatch(thread, shown, head);
  }
  return never(shown, "rest", key);
}
