// SPDX-License-Identifier: AGPL-3.0-only
// The shapes a guest session is opened and held through, which the host's guest door and its kinds share.

/** What a kind's module is handed when a guest session opens: what the guest asked for, the environment its verbs
 * run under, and the two ways back to it. The host's guest door hands one for a process inside a machine, and this
 * socket for the wsp command's forwarder on this computer. */
export interface GuestOpening {
  argv: readonly string[];
  cwd: string;
  /** The pair a verb reads its host and its token off, as a turn's own launch leaves them, plus the turn's token. */
  env: Record<string, string>;
  /** The token is a thread another thread started, which has no slate: its tool server says nothing of one. */
  noSlate?: true;
  /** No app on this computer drew looks when the session opened: its tool server lists no slate_look. */
  noLook?: true;
  reply(message: unknown): void;
  close(error?: string): void;
}

/** One open session, as the door talks to it. */
export interface GuestSession {
  message(message: unknown): void;
  /** The guest's end went, or the workspace did: whatever this session holds open is dropped. A link that drops
   * and redials is neither, and the session stands across it. */
  close(): void;
}

/** A kind of guest session: the tool server, or one command line. */
export interface GuestKindModule {
  open(opening: GuestOpening): GuestSession;
}
