// SPDX-License-Identifier: AGPL-3.0-only
// The one reading of what a secret looks like in a recipe or a file it lands,
// so the recipe writer and the configs' cuts cannot disagree.
import { KEYLIKE_SEGMENT, secretNamed } from "@wsp/collect";

const PRIVATE_KEY = /-----BEGIN [A-Z ]*PRIVATE KEY-----/;

/** What a key or a token looks like, anywhere in a name or a value: OpenAI's and Anthropic's keys, Google's, GitHub's
 * and GitLab's tokens, Slack's, an AWS access key id, a JWT and any private key's header. */
const TOKEN_SHAPES: readonly RegExp[] = [
  /(?<![A-Za-z0-9])sk-(?:proj-|ant-)?(?=[A-Za-z_-]*[0-9])[A-Za-z0-9_-]{20,}/,
  /(?<![A-Za-z0-9])AIza[A-Za-z0-9_-]{30,}/,
  /(?<![A-Za-z0-9])github_pat_[A-Za-z0-9_]{20,}/,
  /(?<![A-Za-z0-9])gh[opsur]_[A-Za-z0-9]{20,}/,
  /(?<![A-Za-z0-9])glpat-[A-Za-z0-9_-]{20,}/,
  /(?<![A-Za-z0-9])xox[abpr]-[A-Za-z0-9-]{10,}/,
  /(?<![A-Za-z0-9])AKIA[0-9A-Z]{16}(?![A-Za-z0-9])/,
  /(?<![A-Za-z0-9_-])eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/,
  PRIVATE_KEY,
];

export const tokenShaped = (text: string): boolean => TOKEN_SHAPES.some(shape => shape.test(text));

/** Whether a text carries a private key, which blanking its header would not take out. */
export const holdsPrivateKey = (text: string): boolean => PRIVATE_KEY.test(text);

/** Every run shaped like a token blanked, for a line a log keeps or a file a provider's machine gets. */
export const blankTokens = (text: string, mark = "<redacted>"): string => TOKEN_SHAPES.reduce((out, shape) => out.replace(new RegExp(shape.source, "g"), mark), text);

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
  return carries(value) && (secretNamed(words.join("_")) || words.some(w => SIGNATURE_WORDS.has(w)));
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
