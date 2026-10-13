// SPDX-License-Identifier: AGPL-3.0-only
// The tokens a brokered Codex holds instead of a login. Codex reads the JWTs' claims and never checks a signature
// (codex login/src/auth/token_data.rs), so the real account, plan, user and FedRAMP claims ride unsigned beside the
// turn token, and the host reads the turn back on every request.
import type { CodexClaims } from "./types.js";

export const TURN_CLAIM = "https://wsp.dev/turn";
const AUTH_CLAIM = "https://api.openai.com/auth";
const REFRESH_PREFIX = "rt_wsp_";
const KEY_PREFIX = "sk-wsp-";
const YEAR_S = 365 * 24 * 3600;

export interface CodexTokens {
  id_token: string;
  access_token: string;
  refresh_token: string;
}

const part = (value: object): string => Buffer.from(JSON.stringify(value)).toString("base64url");

function jwt(claims: CodexClaims, turn: string, nowS: number): string {
  const auth = {
    chatgpt_account_id: claims.accountId,
    chatgpt_plan_type: claims.planType,
    chatgpt_user_id: claims.userId,
    chatgpt_account_is_fedramp: claims.fedramp === true,
  };
  return `${part({ alg: "none", typ: "JWT" })}.${part({ iat: nowS, exp: nowS + YEAR_S, [AUTH_CLAIM]: auth, [TURN_CLAIM]: turn })}.wsp`;
}

export function codexPlaceholders(claims: CodexClaims, turn: string, now = Date.now()): CodexTokens {
  const nowS = Math.floor(now / 1000);
  const token = jwt(claims, turn, nowS);
  return { id_token: token, access_token: token, refresh_token: `${REFRESH_PREFIX}${turn}` };
}

export function codexAuthFile(claims: CodexClaims, turn: string, now = Date.now()): object {
  return { auth_mode: "chatgpt", tokens: { ...codexPlaceholders(claims, turn, now), account_id: claims.accountId }, last_refresh: new Date(now).toISOString() };
}

export function codexKeyFile(turn: string): object {
  return { auth_mode: "apikey", OPENAI_API_KEY: `${KEY_PREFIX}${turn}` };
}

export function turnOfRefreshToken(token: unknown): string | undefined {
  if (typeof token !== "string" || !token.startsWith(REFRESH_PREFIX)) return undefined;
  return token.slice(REFRESH_PREFIX.length) || undefined;
}
