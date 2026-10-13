// SPDX-License-Identifier: AGPL-3.0-only
// The shape of the placeholder credentials a machine whose credentials the host keeps holds: the Codex adapter writes
// them into the turn's login and the host's broker reads the turn token back out of them at the door, so a word
// renamed on one side alone would leave every request without its token.

/** The JWT claim a placeholder ChatGPT login carries the turn token in. */
export const TURN_CLAIM = "https://wsp.dev/turn";
/** A placeholder API key: this, then the turn token. */
export const PLACEHOLDER_KEY_PREFIX = "sk-wsp-";
/** A placeholder refresh token: this, then the turn token, since Codex's refresh request carries no other. */
export const PLACEHOLDER_REFRESH_PREFIX = "rt_wsp_";
/** The signature part of a placeholder JWT, which is unsigned: codex-cli 0.162.1 refuses a JWT whose last part is
 * empty. */
export const PLACEHOLDER_SIGNATURE = "wsp";

/** The claims of the host's own ChatGPT login, copied into the placeholder login Codex runs on such a machine: the
 * `https://api.openai.com/auth` claim's account, plan, user and whether the account is FedRAMP. */
export interface CodexClaims {
  accountId: string;
  planType?: string;
  userId?: string;
  fedramp?: boolean;
}
