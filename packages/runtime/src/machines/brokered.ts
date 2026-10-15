// SPDX-License-Identifier: AGPL-3.0-only
// What a turn carries on a machine whose credentials the host keeps: the turn
// token in every place a credential would sit, and nothing from the vault. The
// host's broker checks the token and adds the real credential on the way out,
// so the machine, the provider's exec text and anything a turn leaves running
// hold a token that dies with the turn.
import { brokeredSignInEnv, type BrokeredSignIn } from "@wsp/adapter-claude";
import { brokeredCodexEnv, codexForShell, type CodexBroker } from "@wsp/adapter-codex";
import { CATALOG_AGENTS, keyEnvOf } from "@wsp/catalog";
import { BROKER_PORT, BROKER_SOCKET_PATH, GUEST_SOCKET_DIR, HOST_TOKEN_ENV, brokeredAgentRefusal, shellQuote, type BrokerStatus, type CodexClaims, type ExecStreamFactory } from "@wsp/protocol";
import { BROKERED_AGENTS, secretsOf } from "../adapters.js";
import type { HarnessAdapterContext } from "../types/harness.js";

/** The folder gh reads its config from on such a machine; the launch text writes the config there. */
export const BROKERED_GH_DIR = `${GUEST_SOCKET_DIR}/gh`;

/** The only way gh is sent down a unix socket is this config key: no variable sets it (measured on gh 2.97.0). */
const GH_DOOR_LINE = `mkdir -p ${BROKERED_GH_DIR} && printf '%s\\n' ${shellQuote(`http_unix_socket: ${BROKER_SOCKET_PATH}`)} > ${BROKERED_GH_DIR}/config.yml`;

/** The folder a terminal on such a machine finds the agents' programs in, one folder in it per agent a terminal's
 * thread runs, since threads of two agents may share the machine. Not under the daemon's own folder: /run is mounted
 * noexec there (measured on a Boat machine), so a shell passes over a program in it. */
export const BROKERED_BIN_DIR = "/usr/local/libexec/wsp-terminal";
/** The variable only a terminal's shell on such a machine carries, naming its thread's agent, whose folder in that one
 * it puts first on its PATH. */
export const TERMINAL_ENV = "WSP_TERMINAL";
/** The folder a terminal whose thread runs `own` finds the agents' programs in. */
export const terminalBinOf = (own: string): string => `${BROKERED_BIN_DIR}/${own}`;
/** The profile file that does it: every image sets its PATH outright in profile.d's wsp-golden.sh, so a PATH the shell
 * is opened with does not survive its login, and profile.d runs in name order, so this file comes after that one. */
export const TERMINAL_PROFILE = "/etc/profile.d/wsp-terminal.sh";

const nameOf = (id: string): string => CATALOG_AGENTS.find(a => a.id === id)?.name ?? id;

/** What a terminal there finds as `other`'s command where its thread runs `own`: the host's broker sends the machine's
 * model calls to the provider of its thread's agent alone, so `other` cannot sign in there. */
export function otherAgentProgram(other: string, own: string): string {
  const said = `wsp: ${nameOf(other)} cannot sign in on this machine. This terminal's thread runs ${nameOf(own)}, and your computer signs in only that agent here. Start a thread on ${nameOf(other)} to use it.`;
  return ["#!/bin/sh", `printf '%s\\n' ${shellQuote(said)} >&2`, "exit 1", ""].join("\n");
}

/** The line a terminal's open runs on such a machine whose thread runs `own`: in that agent's folder, its program for
 * the terminal where it needs one and a refusal under every other agent's command, and the profile line that puts the
 * folder of the agent a shell names first, each written whole and swapped in by a rename, so a shell or an agent
 * starting meanwhile reads the old file or the new. */
export function terminalLine(own: string, ownProgram: string | undefined): string {
  const put = (path: string, text: string, mode: string): string => `printf %s ${shellQuote(text)} > ${path}.$$ && chmod ${mode} ${path}.$$ && mv -f ${path}.$$ ${path}`;
  const folder = terminalBinOf(own);
  const programs = CATALOG_AGENTS.filter(a => BROKERED_AGENTS.has(a.id)).map(a => {
    const file = `${folder}/${a.bin}`;
    if (a.id !== own) return put(file, otherAgentProgram(a.id, own), "755");
    return ownProgram !== undefined ? put(file, ownProgram, "755") : `rm -f ${file}`;
  });
  const named = `${BROKERED_BIN_DIR}/"$${TERMINAL_ENV}"`;
  const profile = `[ -z "\${${TERMINAL_ENV}:-}" ] || [ ! -d ${named} ] || PATH=${named}:$PATH\n`;
  return [`mkdir -p ${folder}`, ...programs, put(TERMINAL_PROFILE, profile, "644")].join(" && ");
}

/** Every remote form git may be handed for GitHub, each rewritten to plain http so the HTTP door can read it as git's
 * proxy while `git remote -v` still reads github.com; a rewrite to the door's own address makes gh refuse the
 * checkout (measured on git 2.43.0 and gh 2.97.0). */
const GITHUB_REMOTES = ["https://github.com/", "git@github.com:", "ssh://git@github.com/", "ssh://git@ssh.github.com:443/"];

/** The environment every process of a turn on such a machine runs under: the turn token as gh's token and as git's
 * proxy password, gh pointed at the config naming the gh door, and git configured by GIT_CONFIG_COUNT pairs alone,
 * with no helper ever asked. cargo's own git follows insteadOf but ignores git's proxy, so it is told to run git. */
export function brokeredEnv(token: string): Record<string, string> {
  const pairs: [string, string][] = [
    ...GITHUB_REMOTES.map((from): [string, string] => ["url.http://github.com/.insteadOf", from]),
    ["http.http://github.com/.proxy", `http://wsp:${token}@127.0.0.1:${BROKER_PORT}`],
    ["http.proxyAuthMethod", "basic"],
    ["credential.helper", ""],
  ];
  return {
    [HOST_TOKEN_ENV]: token,
    GH_CONFIG_DIR: BROKERED_GH_DIR,
    GH_TOKEN: token,
    // gh's telemetry otherwise sends a TLS handshake down the socket.
    GH_TELEMETRY: "false",
    GIT_CONFIG_COUNT: String(pairs.length),
    ...Object.fromEntries(pairs.flatMap(([key, value], i) => [[`GIT_CONFIG_KEY_${i}`, key], [`GIT_CONFIG_VALUE_${i}`, value]])),
    GIT_TERMINAL_PROMPT: "0",
    CARGO_NET_GIT_FETCH_WITH_CLI: "true",
  };
}

/** The sign-in a turn's agent is handed on such a machine: the turn token as the kind of credential the host adds,
 * as its broker says, so the agent sends the header the host's swap expects. The Claude adapter writes
 * Claude Code's placeholder from `claude`: a setup-token and the person's own claude.ai login the host borrows both
 * take the token kind, which is also what a broker that names none gets. A Codex key rides the vault under its own
 * name, marked so the broker reads it as one of its own; Codex on the host's ChatGPT login has a file of its own. */
export function placeholdersOf(status: BrokerStatus | undefined, token: string): { claude: { token: string; kind: "token" | "key" | "gateway" }; vault: Record<string, string> } {
  const claude = status?.claude === "key" || status?.claude === "gateway" ? status.claude : "token";
  const codexKey = keyEnvOf(CATALOG_AGENTS.find(a => a.id === "codex")!.signIn);
  return { claude: { token, kind: claude }, vault: status?.codex === "key" && codexKey !== undefined ? { [codexKey]: `sk-wsp-${token}` } : {} };
}

/** A launch on such a machine as the host reads it for an agent, with the shell's token or with none. */
type LaunchRead = (token: string | undefined) => { vault: Readonly<Record<string, string>>; claude?: BrokeredSignIn; codex?: CodexBroker } | undefined;

/** What each agent a terminal's thread runs signs in with in that terminal's shell, on the shell's token: the variables
 * its turn's launch adds, and its own program for the terminal where the environment alone cannot sign it in. Codex
 * reads its door off flags alone, so it has a program; its login is read with no token first, since with one a host
 * holding no Codex login it has read refuses rather than answers. */
const TERMINAL_SIGN_INS = new Map<string, (read: LaunchRead, token: string) => { env: Record<string, string>; program?: string } | undefined>([
  ["claude", (read, token) => {
    const claude = read(token)?.claude;
    return claude === undefined ? undefined : { env: brokeredSignInEnv(claude) };
  }],
  ["codex", (read, token) => {
    const codex = read(undefined)?.codex;
    if (codex === undefined) return undefined;
    const vault = read(token)?.vault ?? {};
    return { env: brokeredCodexEnv(codex, secretsOf(vault, "codex").apiKey), program: codexForShell(codex, terminalBinOf("codex")) };
  }],
]);

/** The sign-in of the agent `own` in a terminal's shell on such a machine; nothing for an agent the broker adds no key
 * for, or one the host holds no login of. */
export function terminalSignIn(own: string, read: LaunchRead, token: string): { env: Record<string, string>; program?: string } | undefined {
  return TERMINAL_SIGN_INS.get(own)?.(read, token);
}

/** The placeholder login Codex runs on, as the broker says it holds: the host's ChatGPT login under the claims the
 * host read off it, or its key. Nothing where the host holds neither, or its login answered no claims in time. */
export function codexBrokerOf(status: BrokerStatus | undefined, claims: CodexClaims | undefined): HarnessAdapterContext["codexBroker"] {
  if (status?.codex === "key") return { mode: "apikey" };
  return status?.codex !== undefined && claims !== undefined ? { mode: "chatgpt", claims } : undefined;
}

/** The refusal for an agent the host's broker adds no key for, naming the agents it does; nothing for one it does. */
export function unbrokeredRefusal(harness: string): Error | undefined {
  return BROKERED_AGENTS.has(harness) ? undefined : brokeredAgentRefusal(nameOf(harness), [...BROKERED_AGENTS].map(nameOf));
}

/** An exec road whose launch text writes gh's config before whatever it runs, wherever the launch's environment points
 * gh at that config; every other launch is left as it was. */
export function withGhDoor(exec: ExecStreamFactory): ExecStreamFactory {
  const wrapped = (command: string, options: Parameters<ExecStreamFactory>[1]) => exec(options.env["GH_CONFIG_DIR"] === BROKERED_GH_DIR ? `${GH_DOOR_LINE} && ${command}` : command, options);
  return Object.assign(wrapped, {
    ...(exec.attach !== undefined ? { attach: exec.attach.bind(exec) } : {}),
    ...(exec.sweep !== undefined ? { sweep: exec.sweep.bind(exec) } : {}),
  });
}
