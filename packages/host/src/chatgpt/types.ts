// SPDX-License-Identifier: AGPL-3.0-only
// The broker's service shape as this service implements it. The broker core calls services structurally and no
// service imports it, so these name only the fields this service reads.
import type { IncomingMessage, ServerResponse } from "node:http";
import type { Duplex } from "node:stream";

export interface CodexClaims {
  accountId: string;
  planType?: string;
  userId?: string;
  fedramp?: boolean;
}

export type ChatgptCredential = CodexClaims & { accessToken: string };

export interface OpenaiCredential {
  key: string;
  baseUrl?: string;
  organization?: string;
  project?: string;
}

export type Door = "http" | "gh";

export interface BrokerRequest {
  workspaceId: string;
  door: Door;
  grant: { kind: "turn" | "op" | "terminal"; workspaceId: string; threadId?: string; op?: string; branch?: string };
  req: IncomingMessage;
  res: ServerResponse;
  heldMs: number;
}

export interface BrokerContext {
  credential(kind: "chatgpt"): Promise<ChatgptCredential | undefined>;
  credential(kind: "openai"): Promise<OpenaiCredential | undefined>;
  refuse(res: ServerResponse, status: 403 | 503, said: string, shape: "openai"): void;
}

export interface BrokerService {
  id: "chatgpt" | "openai";
  claims(req: IncomingMessage, door: Door, agent: "claude" | "codex" | undefined): boolean;
  serve(r: BrokerRequest, ctx: BrokerContext): Promise<void>;
  upgrade?(r: BrokerRequest, socket: Duplex, head: Buffer, ctx: BrokerContext): Promise<void>;
}
