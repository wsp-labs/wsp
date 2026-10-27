// SPDX-License-Identifier: AGPL-3.0-only
// The shapes the cloud setup and every sign-in share: what a login came to, how a sign-in finishes and which provider
// keys a host holds.
import { z } from "zod";

/** What each login came to by the time the golden sealed: a sign-in on the machine, or a copy from this computer
 * checked there with the tool's status command. `copied` is a copy nothing checked: an update re-imported it, or the
 * tool has no status command or is not on the machine. `deferred` is a login nobody signed in during the build,
 * because it was left to first use or because the sign-in that ran hit the build's cap: the build never waits on a
 * person, so that is an outcome and not a failure. */
export const LoginState = z.enum(["signed-in", "not-signed-in", "not-verified", "skipped", "copied", "deferred"]);
export type LoginState = z.infer<typeof LoginState>;

/** What happens to a login: copied from this computer, signed in on the machine while the build runs, left to the
 * first time the tool needs it on a workspace, set on the machine as an API key the tool reads, minted here as a
 * long-lived token the vault hands every turn, or left out. One list, read by the collector's rows and by a recipe's
 * rows. */
export const LOGIN_CHOICES = ["copy", "machine", "later", "key", "skip", "token"] as const;
export const LoginChoice = z.enum(LOGIN_CHOICES);
export type LoginChoice = z.infer<typeof LoginChoice>;

/** How a sign-in finishes where nobody is at the machine's terminal, which is the app's case: `callback` for a page
 * that redirects to a port on the machine, forwarded from this computer, so no code ever comes back; `code` for a
 * page that hands a code back, which goes to the tool on the machine as the person would type it; `none` for a flow
 * finished on the page or in the tool's own prompts, which the tool's status then proves. */
export const SignInFinish = z.enum(["callback", "code", "none"]);
export type SignInFinish = z.infer<typeof SignInFinish>;

/** Which providers this computer holds a key for, keyed by the word WSP_PROVIDER holds: the presence alone, never a
 * value. One entry per provider that reads a key, read off the host's own provider table, so a provider added
 * tomorrow is a row there and nothing here. */
export const InitKeys = z.record(z.boolean());
export type InitKeys = z.infer<typeof InitKeys>;

/** The kinds a refused `init.keys` carries, so a client tells the provider's own refusal, which the person fixes by
 * typing another key, from a check that never got an answer, which is worth pressing again. */
export const KEY_REFUSED = "keyRefused";
export const KEY_UNCHECKED = "keyUnchecked";
