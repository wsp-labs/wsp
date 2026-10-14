// SPDX-License-Identifier: AGPL-3.0-only
// The sign-ins a build at a provider is checked against before it boots: the
// ticked agents' and GitHub's, on this computer, which that provider's machines
// use through wsp. Read off the collector's rows, the vault, and gh's hosts.yml
// for the account it names; no secret is kept or printed for them.
import { keyEnvOf, mintsToken } from "@wsp/catalog";
import { agentRowId, type Manifest, type Platform } from "@wsp/collect";
import { HOST_SIGN_IN_COMMANDS, HOST_SIGN_INS } from "@wsp/protocol";
import { copiedLogin } from "./init-import.js";
import { signInFor } from "./signin-table.js";

export interface HostSignIn {
  name: string;
  ready: boolean;
  /** Its state, or the line that fixes it. */
  said: string;
}

const held = (vault: Readonly<Record<string, string>>, name: string | undefined): boolean => name !== undefined && (vault[name] ?? "") !== "";

/** One line per ticked agent the host signs in for, then GitHub's, in that order. `found` is the collector's own rows:
 * the catalog's bare login rows join later and say nothing of this computer. */
export function hostSignIns(o: { found: Manifest; ticks: ReadonlySet<string>; vault: Readonly<Record<string, string>>; platform: Platform; home: string }): HostSignIn[] {
  const row = (id: string) => o.found.entries.find(e => e.id === `logins/${id}`);
  const agent = (id: "claude" | "codex", name: string): HostSignIn[] => {
    if (!o.ticks.has(agentRowId(id))) return [];
    const s = signInFor(id);
    const said = mintsToken(s) && held(o.vault, s.tokenEnv) ? HOST_SIGN_INS.token : held(o.vault, keyEnvOf(s)) ? HOST_SIGN_INS.key : row(id) !== undefined ? HOST_SIGN_INS.login : undefined;
    return [{ name, ready: said !== undefined, said: said ?? HOST_SIGN_INS.missing(HOST_SIGN_IN_COMMANDS[id]) }];
  };
  const gh = row("gh");
  const account = gh === undefined ? undefined : copiedLogin(gh, o.platform, o.home)?.carries;
  const github: HostSignIn = gh === undefined ? { name: "GitHub", ready: false, said: HOST_SIGN_INS.missing(HOST_SIGN_IN_COMMANDS.github) } : { name: "GitHub", ready: true, said: account === undefined ? HOST_SIGN_INS.login : HOST_SIGN_INS.as(account) };
  return [...agent("claude", "Claude Code"), ...agent("codex", "Codex"), github];
}
