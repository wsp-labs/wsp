// SPDX-License-Identifier: AGPL-3.0-only
// The conversations the agents kept in a project's folder outside wsp, as this window holds them: each project's
// list as the host last answered it, the picker and the project it opened on, the newest previews it read, the one a
// project's New thread page holds as a draft for its next send with the rows that draft shows, and the confirm a
// conversation another app holds open waits on. In memory alone, one entry per project at most and PREVIEWS_KEPT
// previews: a reload reads the list again and drops a draft nobody sent.
import { create } from "zustand";
import { failureOf } from "../protocol/failure.js";
import { projectHomeKey, useStore } from "../protocol/store.js";
import { requestComposerFocus } from "./shellRequests.js";
import { newId, useComposerDraftStore } from "../components/chat/composerDraftStore.js";
import type { ConversationRead, ConversationsAnswer, OutsideConversation } from "@wsp/protocol";

/** How many previews the picker keeps, the oldest read let go first. */
export const PREVIEWS_KEPT = 20;

/** The previews on their way, so a row highlighted twice while it is read asks once. */
const previewing = new Set<string>();

/** A conversation held on a project's New thread page: its next send opens the thread on it, or on a copy of it. */
export interface HeldPick {
  readonly row: OutsideConversation;
  readonly copy: boolean;
}

/** The picks a send off a project's New thread page carries, as its composer holds them. */
export interface SendPicks {
  readonly model?: string;
  readonly effort?: string;
  readonly permissionMode?: string;
}

/** A send that waits on the person's answer: its message, the picks it went with, and the composer it came from. */
export interface HeldSend {
  readonly prompt: string;
  readonly picks: SendPicks;
  readonly key: string;
}

/** A conversation another app holds open, waiting on the person's answer: picked off the page, or refused at a send,
 * which kept the message typed. */
export interface OpenConfirm {
  readonly project: string;
  readonly row: OutsideConversation;
  readonly send?: HeldSend;
}

/** A refusal in two halves. */
export type Said = { readonly said: string; readonly fix?: string };

/** A list as the host answered it, or its refusal in two halves; absent while it is read. */
export type ConversationsRead = ConversationsAnswer | Said;

/** One conversation as the host read it, or its refusal; absent while it is read. */
export type OneRead = ConversationRead | Said;

/** A row of the picker: a conversation and the project it was listed under. */
export interface PickedRow {
  readonly project: string;
  readonly row: OutsideConversation;
}

interface ConversationsState {
  readonly answers: Readonly<Record<string, ConversationsRead>>;
  readonly held: Readonly<Record<string, HeldPick>>;
  /** The rows each held draft shows, by project, under the pick they were read for. */
  readonly drafts: Readonly<Record<string, { readonly pick: HeldPick; readonly read?: OneRead }>>;
  /** The newest previews read, oldest first, by previewKey. */
  readonly previews: ReadonlyMap<string, OneRead>;
  /** The project the picker opened on, while it is open. */
  readonly picker: string | null;
  readonly confirm: OpenConfirm | null;
  read(project: string): Promise<void>;
  preview(project: string, row: OutsideConversation): Promise<void>;
  hold(project: string, pick: HeldPick | null): void;
  openPicker(project: string | null): void;
  ask(confirm: OpenConfirm | null): void;
}

export const isAnswer = (read: ConversationsRead | undefined): read is ConversationsAnswer => read !== undefined && "rows" in read;
export const isRead = (read: OneRead | undefined): read is ConversationRead => read !== undefined && "rows" in read;
export const previewKey = (project: string, row: Pick<OutsideConversation, "agent" | "id">): string => `${project}\0${row.agent}\0${row.id}`;

const saidOf = (e: unknown): Said => {
  const { said, fix } = failureOf(e);
  return { said, ...(fix !== undefined ? { fix } : {}) };
};

export const useConversationsStore = create<ConversationsState>((set, get) => ({
  answers: {},
  held: {},
  drafts: {},
  previews: new Map(),
  picker: null,
  confirm: null,
  async read(project) {
    const list = useStore.getState().api?.conversationsList;
    if (list === undefined) return;
    try {
      const answer = await list(project);
      set(s => ({ answers: { ...s.answers, [project]: answer } }));
    } catch (e) {
      set(s => ({ answers: { ...s.answers, [project]: saidOf(e) } }));
    }
  },
  async preview(project, row) {
    const key = previewKey(project, row);
    const read = useStore.getState().api?.conversationRead;
    // A refusal stands in the pane until the highlight rests on the row again, which asks again.
    if (read === undefined || isRead(get().previews.get(key)) || previewing.has(key)) return;
    previewing.add(key);
    const got = await read({ project, agent: row.agent, id: row.id, preview: true }).catch(saidOf);
    previewing.delete(key);
    set(s => {
      const previews = new Map(s.previews);
      previews.delete(key);
      previews.set(key, got);
      for (const old of previews.keys()) {
        if (previews.size <= PREVIEWS_KEPT) break;
        previews.delete(old);
      }
      return { previews };
    });
  },
  hold(project, pick) {
    set(s => {
      const { [project]: _was, ...rest } = s.held;
      const { [project]: _shown, ...drafts } = s.drafts;
      return pick === null ? { held: rest, drafts } : { held: { ...rest, [project]: pick }, drafts: { ...drafts, [project]: { pick } } };
    });
    const read = useStore.getState().api?.conversationRead;
    if (pick === null || read === undefined) return;
    void read({ project, agent: pick.row.agent, id: pick.row.id })
      .catch(saidOf)
      .then(got => set(s => (s.drafts[project]?.pick === pick ? { drafts: { ...s.drafts, [project]: { pick, read: got } } } : s)));
  },
  openPicker(project) {
    set({ picker: project });
  },
  ask(confirm) {
    set({ confirm });
  },
}));

/** Opens the picker on a project's conversations, the list read again under the one the host last answered. */
export function openResumePicker(project: string): void {
  useConversationsStore.getState().openPicker(project);
  void useConversationsStore.getState().read(project);
}

/** Opens a conversation as a draft on its project's New thread page, the person landed there with the focus in its
 * composer: the one road into a draft, off the picker and off the confirm. */
export function openDraft(project: string, pick: HeldPick): void {
  useConversationsStore.getState().hold(project, pick);
  useStore.getState().openProjectHome(project);
  requestComposerFocus(projectHomeKey(project));
}

/** What a pick off the picker does: a conversation another app holds asks first unless the pick is a copy, which
 * leaves it untouched; any other opens as a draft. */
export function pickConversation(project: string, row: OutsideConversation, copy = false): void {
  useConversationsStore.getState().openPicker(null);
  if (row.live && !copy) {
    useConversationsStore.getState().ask({ project, row });
    return;
  }
  openDraft(project, { row, copy });
}

/** A thread opened on a conversation, from a project's New thread page: the host reads the conversation and opens the
 * thread in the folder it ran in, which this window then shows, the page's draft and pick gone. Its refusal comes
 * back in its halves and kind, and the draft is left as the caller holds it. */
export async function sendOn(project: string, pick: HeldPick, send: HeldSend): Promise<ReturnType<typeof failureOf> | null> {
  const { api, select } = useStore.getState();
  const resume = api?.resumeConversation;
  if (resume === undefined) return null;
  try {
    const view = await resume({
      project,
      prompt: send.prompt,
      requestId: newId(),
      harness: pick.row.agent,
      resume: { id: pick.row.id, ...(pick.copy ? { copy: true } : {}) },
      ...(send.picks.model !== undefined ? { model: send.picks.model } : {}),
      ...(send.picks.effort !== undefined ? { effort: send.picks.effort } : {}),
      ...(send.picks.permissionMode !== undefined ? { permissionMode: send.picks.permissionMode } : {}),
    });
    useConversationsStore.getState().hold(project, null);
    useComposerDraftStore.getState().setDraft(send.key, { prompt: "", cursor: 0 });
    select(view.workspaceId, view.threadId ?? null);
    return null;
  } catch (e) {
    return failureOf(e);
  }
}
