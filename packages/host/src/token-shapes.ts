// SPDX-License-Identifier: AGPL-3.0-only
// The one reading of what a secret looks like in a recipe or a file it lands,
// so the recipe writer and the configs' cuts cannot disagree.

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
