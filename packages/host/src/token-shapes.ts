// SPDX-License-Identifier: AGPL-3.0-only
// The one reading of what a secret looks like in a recipe or a file it lands,
// so the recipe writer and the configs' cuts cannot disagree.
import { secretNamed } from "@wsp/collect";
import { KEYLIKE_SEGMENT, tokenShaped } from "@wsp/protocol";

export { blankTokens, holdsPrivateKey, tokenShaped } from "@wsp/protocol";

export const TOKEN_NAME = "a value shaped like a token";
export const LOGIN_NAME = "a login in an address";

/** Every reference to a variable an agent or a shell expands: `${X}` with or without a default, a bare `$X`, OpenCode's `{env:X}`. */
const REFS = /\$\{[A-Za-z_]\w*(?::-[^}]*)?\}|\$[A-Za-z_]\w*|\{env:[A-Za-z_]\w*\}/g;
/** A command substitution: the shell runs it and its output is the value, so the text holds none. */
const COMMAND = /\$\(|`/;
export const ADDRESS_RUN = /[A-Za-z][A-Za-z0-9+.-]*:\/\/[^\s"'<>`]+/g;
export const unquoted = (v: string): string => (/^(["']).*\1$/.test(v) ? v.slice(1, -1) : v);

/** Whether a value holds something of the person's: not empty, not references alone, not a command. */
export const carries = (value: string): boolean => {
  const v = unquoted(value);
  return !COMMAND.test(v) && v.replace(REFS, "").trim() !== "";
};

/** The words a signed address names its signature with: Azure's SAS `sig`, S3's `X-Amz-Signature`, Cloud Storage's
 * `X-Goog-Signature`, CloudFront's `Signature`. */
const SIGNATURE_WORDS = new Set(["SIG", "SIGNATURE"]);
/** A query parameter that holds something under a secret's name or a signature's, in an address or in Codex's `query_params`. */
export const querySecret = (name: string, value: string): boolean => {
  const words = name.toUpperCase().split(/[-_]/);
  return carries(value) && (secretNamed(name.replace(/-/g, "_")) || words.some(w => SIGNATURE_WORDS.has(w)));
};

/** An address's login is a secret where it holds a password, or a user name shaped like a key; `git@` and `dev@` are
 * who signs in, never how. */
const loginSecret = (login: string): boolean => {
  const colon = login.indexOf(":");
  return colon === -1 ? KEYLIKE_SEGMENT.test(login) && carries(login) : carries(login.slice(colon + 1));
};

/** What makes a text one that never lands in a copied config, named for the note: a token's shape, a secret login in
 * an address, or the query parameter that holds a secret. A path segment is never one, so a long repository name stays. */
export function secretIn(text: string): string | undefined {
  if (tokenShaped(text)) return TOKEN_NAME;
  for (const [url] of text.matchAll(ADDRESS_RUN)) {
    const login = /^[^:]+:\/\/([^/?#]*)@/.exec(url)?.[1];
    if (login !== undefined && loginSecret(login)) return LOGIN_NAME;
    for (const [, name, value] of url.matchAll(/[?&]([^=&#]+)=([^&#]*)/g)) if (querySecret(name!, value!)) return name;
  }
  return undefined;
}
