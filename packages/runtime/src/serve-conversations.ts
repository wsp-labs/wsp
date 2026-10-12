// SPDX-License-Identifier: AGPL-3.0-only
// The conversation ops a socket asks, answered off the runtime's conversations: what the reply carries beside id and ok.
import type { Caller, RuntimeRequest } from "@wsp/protocol";
import type { Runtime } from "./types/api.js";

export type ConversationsRequest = Extract<RuntimeRequest, { op: `conversations.${string}` }>;

export const isConversationsRequest = (msg: RuntimeRequest): msg is ConversationsRequest => msg.op.startsWith("conversations.");

export async function answerConversations(conversations: Runtime["conversations"], msg: ConversationsRequest, origin: Caller | undefined): Promise<Record<string, unknown>> {
  switch (msg.op) {
    case "conversations.list":
      return { ...(await conversations.list({ ...(msg.project !== undefined ? { project: msg.project } : {}), ...(msg.agent !== undefined ? { agent: msg.agent } : {}) }, origin)) };
    case "conversations.read":
      return { ...(await conversations.read({ ...(msg.project !== undefined ? { project: msg.project } : {}), agent: msg.agent, id: msg.conversation, ...(msg.preview === true ? { preview: true } : {}) }, origin)) };
  }
}
