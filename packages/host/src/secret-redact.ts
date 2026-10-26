// SPDX-License-Identifier: AGPL-3.0-only
// What a log may not carry: credential-shaped assignments lose their value, and every value handed in as hidden is
// blanked wherever it appears.
import { redacted } from "@wsp/protocol";
import { blankTokens } from "./token-shapes.js";

/** A name that says credential, anywhere in it. */
export const SECRET_NAME = /KEY|TOKEN|SECRET|PASSWORD/i;
/** A NAME=value whose name says credential, quoted or bare; the value goes. */
const SECRET_ASSIGN = new RegExp(`\\b([A-Za-z0-9_]*(?:${SECRET_NAME.source})[A-Za-z0-9_]*)=("[^"]*"|'[^']*'|\\S+)`, "gi");
/** The same name as a quoted JSON key, a bearer or basic header value, and a URL's login. */
const SECRET_FIELD = new RegExp(`("[A-Za-z0-9_-]*(?:${SECRET_NAME.source})[A-Za-z0-9_-]*"\\s*:\\s*)"[^"]*"`, "gi");
const AUTH_VALUE = /(\bBearer |[Aa]uthorization['"]?:\s*['"]?Basic )[^\s"',;]+/g;
const URL_LOGIN = /(\b[a-z][a-z0-9+.-]*:\/\/)[^\s/@]+@/gi;

export function redact(text: string, hidden: readonly string[] = []): string {
  const named = text.replace(SECRET_ASSIGN, "$1=<redacted>").replace(SECRET_FIELD, '$1"<redacted>"').replace(AUTH_VALUE, "$1<redacted>").replace(URL_LOGIN, "$1<redacted>@");
  return blankTokens(redacted(named, hidden));
}
