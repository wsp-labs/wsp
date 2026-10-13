// SPDX-License-Identifier: AGPL-3.0-only
// What a turn carries on a machine whose credentials the host keeps: the turn
// token in every place a credential would sit, and nothing from the vault. The
// host's broker checks the token and adds the real credential on the way out,
// so the machine, the provider's exec text and anything a turn leaves running
// hold a token that dies with the turn.
import { CATALOG_AGENTS, keyEnvOf } from "@wsp/catalog";
import { BROKER_PORT, BROKER_SOCKET_PATH, GUEST_WSP_HOME, HOST_TOKEN_ENV, brokeredAgentRefusal, shellQuote, type BrokerStatus, type ExecStreamFactory } from "@wsp/protocol";
import { BROKERED_AGENTS } from "../adapters.js";

/** The folder gh reads its config from on such a machine; the launch text writes the config there. */
export const BROKERED_GH_DIR = `${GUEST_WSP_HOME}/gh`;

/** The only way gh is sent down a unix socket is this config key: no variable sets it (measured on gh 2.97.0). */
const GH_DOOR_LINE = `mkdir -p ${BROKERED_GH_DIR} && printf '%s\\n' ${shellQuote(`http_unix_socket: ${BROKER_SOCKET_PATH}`)} > ${BROKERED_GH_DIR}/config.yml`;

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

/** The refusal for an agent the host's broker adds no key for, naming the agents it does; nothing for one it does. */
export function unbrokeredRefusal(harness: string): Error | undefined {
  const nameOf = (id: string): string => CATALOG_AGENTS.find(a => a.id === id)?.name ?? id;
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
