// SPDX-License-Identifier: AGPL-3.0-only
// The shapes the broker and its services meet on. A service implements
// BrokerService structurally and never imports the broker, so these and the
// protocol's door and record are the whole of what the two sides agree on.
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Duplex } from "node:stream";
import type { BranchOfRecord, PipeDoor } from "@wsp/protocol";
import type { CodexClaims } from "@wsp/runtime";

/** What a token names, as the host's own token store reads it: whose workspace it is and what it may drive. */
export interface Grant {
  kind: "turn" | "op" | "terminal";
  workspaceId: string;
  threadId?: string;
  op?: string;
  branch?: string;
}

export interface BrokerWorkspace {
  agent?: "claude" | "codex";
  branchOfRecord?: BranchOfRecord;
  repo: { owner: string; name: string; id: number };
  trackers: string[];
}

export interface BrokerRequest {
  /** The workspace whose link the pipe came on, never anything the request says. */
  workspaceId: string;
  door: PipeDoor;
  grant: Grant;
  req: IncomingMessage;
  res: ServerResponse;
  heldMs: number;
}

export type ServiceId = "anthropic" | "chatgpt" | "openai" | "git" | "gh";

/** A grant the host holds live on a workspace now, with the agent of the turn where it is one. */
export type LiveGrant = Grant & { agent?: "claude" | "codex" };

/** The real credentials, held in memory for one request, each in the shape its service sends it in. `expiresAt`
 * (milliseconds since the epoch) is set only for a borrowed claude.ai login. */
export type AnthropicCredential =
  | { kind: "token"; token: string; expiresAt?: number }
  | { kind: "key"; key: string }
  | { kind: "helper"; value: string }
  | { kind: "gateway"; url: string; token: string; headers?: Readonly<Record<string, string>> };
export type ChatgptCredential = CodexClaims & { accessToken: string };
export interface OpenaiCredential {
  key: string;
  baseUrl?: string;
  organization?: string;
  project?: string;
}
export interface GithubCredential {
  token: string;
}
export interface CredentialOf {
  anthropic: AnthropicCredential;
  chatgpt: ChatgptCredential;
  openai: OpenaiCredential;
  github: GithubCredential;
}
export type CredentialKind = keyof CredentialOf;

/** The client a refusal is written for, so each prints its own error rather than a parse failure. */
export type RefusalShape = "anthropic" | "openai" | "git" | "github-rest" | "github-graphql";

/** One pushed ref as the git road judged it. */
export interface PushedRef {
  ref: string;
  old: string;
  new: string;
  verdict: string;
}

/** One brokered call. Never a body, a query string, a header value or a credential. */
export interface BrokerLogLine {
  at: string;
  machine: string;
  thread?: string;
  token?: Grant["kind"];
  service?: ServiceId;
  method: string;
  path: string;
  status: number;
  bytesUp: number;
  bytesDown: number;
  ms: number;
  heldMs: number;
  pushes?: PushedRef[];
}

export interface BrokerContext {
  credential<K extends CredentialKind>(kind: K): Promise<CredentialOf[K] | undefined>;
  workspace(id: string): Promise<BrokerWorkspace>;
  refuse(res: ServerResponse, status: 403 | 503, said: string, shape: RefusalShape): void;
  /** What a service adds to this call's line, such as the refs a push named; the broker writes the line once. */
  log(line: Partial<BrokerLogLine>): void;
}

export interface BrokerService {
  id: ServiceId;
  claims(req: IncomingMessage, door: PipeDoor, agent: "claude" | "codex" | undefined): boolean;
  serve(r: BrokerRequest, ctx: BrokerContext): Promise<void>;
  upgrade?(r: BrokerRequest, socket: Duplex, head: Buffer, ctx: BrokerContext): Promise<void>;
}
