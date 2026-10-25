// SPDX-License-Identifier: AGPL-3.0-only
// The one reading of what a key or a token looks like, so the host's cuts and the tap cannot disagree.

const PRIVATE_KEY = /-----BEGIN [A-Z ]*PRIVATE KEY-----/;

/** A path segment long and token-like enough to be a key the address carries, as Zapier's per-person path is; a key
 * carries a digit, where a long slug such as a repository's name does not. */
export const KEYLIKE_SEGMENT = /^(?=.*\d)[A-Za-z0-9_=-]{24,}$/;

/** What a key or a token looks like, anywhere in a name or a value: OpenAI's and Anthropic's keys, Google's, GitHub's
 * and GitLab's tokens, Slack's, an AWS access key id, a JWT and a private key from its header through its END line,
 * or to the end of the text where the END line is not in it. */
const TOKEN_SHAPES: readonly RegExp[] = [
  /(?<![A-Za-z0-9])sk-(?:proj-|ant-)?(?=[A-Za-z_-]*[0-9])[A-Za-z0-9_-]{20,}/,
  /(?<![A-Za-z0-9])AIza[A-Za-z0-9_-]{30,}/,
  /(?<![A-Za-z0-9])github_pat_[A-Za-z0-9_]{20,}/,
  /(?<![A-Za-z0-9])gh[opsur]_[A-Za-z0-9]{20,}/,
  /(?<![A-Za-z0-9])glpat-[A-Za-z0-9_-]{20,}/,
  /(?<![A-Za-z0-9])xox[abpr]-[A-Za-z0-9-]{10,}/,
  /(?<![A-Za-z0-9])AKIA[0-9A-Z]{16}(?![A-Za-z0-9])/,
  /(?<![A-Za-z0-9_-])eyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}/,
  /-----BEGIN ([A-Z ]*)PRIVATE KEY-----(?:[\s\S]*?-----END \1PRIVATE KEY-----|[\s\S]*)/,
];

export const tokenShaped = (text: string): boolean => TOKEN_SHAPES.some(shape => shape.test(text));

/** Whether a text carries a private key's header. */
export const holdsPrivateKey = (text: string): boolean => PRIVATE_KEY.test(text);

/** Every run shaped like a token blanked, for a line a log keeps, a file a provider's machine gets or a tape's frame;
 * a function is handed each run and answers its mark. */
export const blankTokens = (text: string, mark: string | ((run: string) => string) = "<redacted>"): string =>
  TOKEN_SHAPES.reduce((out, shape) => (typeof mark === "string" ? out.replace(new RegExp(shape.source, "g"), mark) : out.replace(new RegExp(shape.source, "g"), run => mark(run))), text);
