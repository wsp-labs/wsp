// SPDX-License-Identifier: AGPL-3.0-only
// The shell line that runs one Codex turn on a workspace: `codex app-server`
// on its own stdio, as codex-cli 0.155.1 spells it. Everything about the turn
// (the prompt, the model, the sandbox) travels as JSON-RPC on stdin (rpc.ts),
// so the line carries only the folder and the MCP servers' config overrides.
// No listener is ever named: stdio is the server's default transport, and a
// socket would let anything on the machine drive the agent.
import { agentWord, BROKER_PORT, HOST_TOKEN_ENV, inFolder, LAUNCH_ENV, launchHasSlate, launchWords, MCP_SERVER_NAME, PLACEHOLDER_KEY_PREFIX, PLACEHOLDER_REFRESH_PREFIX, PLACEHOLDER_SIGNATURE, programWord, shellQuote, SLATE_SERVER_NAME, SLATE_TOOLS, TURN_CLAIM, WSP_TOOL_TIMEOUT_SEC, type AgentLaunch, type CodexClaims, type McpServerSpec } from "@wsp/protocol";

const SLUG_RE = /^[A-Za-z0-9][A-Za-z0-9._:-]*$/;
/** The sandbox modes thread/start takes; the one that turns the sandbox off is the one that asks nobody. */
const SANDBOXED = ["read-only", "workspace-write"] as const;
const NO_SANDBOX = "danger-full-access";

/** The HTTP door the daemon serves on the machine's loopback, where the host adds the real credential. */
const DOOR = `http://127.0.0.1:${BROKER_PORT}`;
/** The same door under another origin, for chatgpt_base_url alone: when both base URLs share an origin, codex-cli
 * 0.162.1 moves model calls to the backend origin its account check answers (apply_workspace_routing), which is
 * chatgpt.com itself, past the door. */
const BACKEND_DOOR = `http://localhost:${BROKER_PORT}`;
/** The variable each custom provider's env_key names on a brokered turn, holding the turn's placeholder key. */
export const PROVIDER_KEY_ENV = "WSP_CODEX_PROVIDER_KEY";

/** ChatGPT's list of the models the host's sign-in may run on the codex version named, as its /codex/models answers it;
 * undefined where the host could not read it. A machine reads it nowhere itself, since outside a turn it holds no
 * live login. */
export type AccountModels = (clientVersion: string) => Promise<unknown>;

/** A turn on a machine whose credentials the host keeps: the kind of placeholder login it runs on, and the person's
 * own `model_providers` entries, each rewritten to the door. */
export type CodexBroker = ({ mode: "chatgpt"; claims: CodexClaims; models?: AccountModels } | { mode: "apikey" }) & {
  providers?: Readonly<Record<string, Readonly<Record<string, unknown>>>>;
};

export interface CodexEnvOptions {
  base?: Readonly<Record<string, string | undefined>>;
  /** Absolute path for CODEX_HOME, where the box's own login is mounted or a sign-in there wrote auth.json. */
  home: string;
  /** The API key the vault holds for this agent; set on every turn's environment. On a brokered turn it is the
   * placeholder key the host shaped, which goes into the key file alone. */
  apiKey?: string;
  broker?: CodexBroker;
}

/** Where a brokered launch finds its placeholder key until it writes the key file; unset before codex starts. */
export const KEY_FILE_ENV = "WSP_CODEX_KEY";
const PLACEHOLDER_KEY_RE = /^[A-Za-z0-9._~+/=-]+$/;

/** The key a turn carries: under both of codex's names on today's road, and on a brokered one only where the
 * launch's key file reads it, refused where it has no place. A brokered launch with no turn token carries none and
 * runs on the key file the last turn left; a turn's own line refuses to start without it. */
function keyEnv(apiKey: string | undefined, broker: CodexBroker | undefined): Record<string, string> {
  if (broker?.mode === "chatgpt" && apiKey !== undefined) throw new Error("a turn on the host's ChatGPT login carries no key; its login is a placeholder");
  if (broker?.mode === "apikey") {
    if (apiKey === undefined) return {};
    if (!PLACEHOLDER_KEY_RE.test(apiKey)) throw new Error("a turn on the host's API key needs the placeholder key the host shaped for it, one plain word");
    return { [KEY_FILE_ENV]: apiKey };
  }
  // The key travels under both names. CODEX_API_KEY is the one this CLI's own login reads and prefers over the
  // store under CODEX_HOME (read_codex_api_key_from_env, codex-rs/login/src/auth/manager.rs at rust-v0.153.0);
  // OPENAI_API_KEY is read by a provider a person configured with env_key and by nothing in the CLI's own login.
  return apiKey === undefined ? {} : { CODEX_API_KEY: apiKey, OPENAI_API_KEY: apiKey };
}

/** CODEX_HOME points codex at the home the sign-in wrote, since a guest exec carries no HOME to derive it from. */
export function buildEnv(options: CodexEnvOptions): Record<string, string> {
  const home = options.home.trim();
  if (!home.startsWith("/")) throw new Error(`home must be an absolute path, got "${options.home}"`);
  const clean: Record<string, string> = {};
  for (const [key, value] of Object.entries(options.base ?? {})) if (value !== undefined) clean[key] = value;
  return { ...clean, CODEX_HOME: home, ...(options.broker === undefined ? keyEnv(options.apiKey, undefined) : brokeredCodexEnv(options.broker, options.apiKey)) };
}

/** What every process that may start codex on a brokered machine carries for it: the placeholder key where the launch
 * reads one, and the refresh and revoke addresses at the door. */
export function brokeredCodexEnv(broker: CodexBroker, apiKey: string | undefined): Record<string, string> {
  return { ...keyEnv(apiKey, broker), CODEX_REFRESH_TOKEN_URL_OVERRIDE: `${DOOR}/oauth/token`, CODEX_REVOKE_TOKEN_URL_OVERRIDE: `${DOOR}/oauth/revoke` };
}

const base64url = (text: string): string => Buffer.from(text).toString("base64url");
const JWT_HEADER = base64url(JSON.stringify({ alg: "none", typ: "JWT" }));
const YEAR_S = 365 * 24 * 60 * 60;
const TURN = "@@wsp-turn@@";
const JWT = "@@wsp-jwt@@";
const KEY = "@@wsp-key@@";

/** JSON whose marks become the shell words standing for them, every other run of it quoted as it is. */
const spliced = (json: string, words: Readonly<Record<string, string>>): string =>
  json
    .split(/(@@wsp-[a-z]+@@)/)
    .map(part => words[part] ?? (part === "" ? "" : shellQuote(part)))
    .join("");

/** The step before the server starts on a brokered turn: CODEX_HOME's auth.json as a placeholder login carrying the
 * turn token, which the host checks and swaps for the real credential at the door. The token is read off the turn's
 * environment where the line runs, so the line holds none. A ChatGPT login is unsigned JWTs whose exp is a year out
 * and whose claims are the host's own, with the real account id: without it codex skips reload on a 401, and a
 * pinned workspace that differs makes codex delete the file and exit. codex-cli 0.162.1 refuses a JWT whose
 * signature part is empty, so it is a word. An API key is the file `codex login --with-api-key` writes, holding the
 * placeholder key as the host shaped it, since app-server reads no key off its environment. A turn refuses to start
 * without its token; any other launch writes the login only when it carries what the login holds, and otherwise runs
 * on the file the last turn left, still at the door. */
function placeholderLogin(broker: CodexBroker, now: number, turn: boolean): string {
  const words = { [TURN]: `"$${HOST_TOKEN_ENV}"`, [JWT]: '"$jwt"', [KEY]: `"$${KEY_FILE_ENV}"` };
  const needs = broker.mode === "apikey" ? KEY_FILE_ENV : HOST_TOKEN_ENV;
  const write = (steps: string[]): string => {
    const body = [`umask 077 && mkdir -p "$CODEX_HOME"`, ...steps].join(" && ");
    const keyGuard = broker.mode === "apikey" ? ` && : "\${${KEY_FILE_ENV}:?a turn on the key the host keeps was launched without its placeholder key}"` : "";
    return turn ? `( : "\${${HOST_TOKEN_ENV}:?a turn whose credentials the host keeps was launched without its token}"${keyGuard} && ${body} )` : `{ [ -z "\${${needs}:-}" ] || ( ${body} ); }`;
  };
  if (broker.mode === "apikey") return write([`printf %s ${spliced(JSON.stringify({ auth_mode: "apikey", OPENAI_API_KEY: KEY }), words)} > "$CODEX_HOME/auth.json"`]);
  const { accountId, planType, userId, fedramp } = broker.claims;
  const payload = JSON.stringify({
    exp: Math.floor(now / 1000) + YEAR_S,
    "https://api.openai.com/auth": { chatgpt_account_id: accountId, ...(planType !== undefined ? { chatgpt_plan_type: planType } : {}), ...(userId !== undefined ? { chatgpt_user_id: userId } : {}), chatgpt_account_is_fedramp: fedramp === true },
    [TURN_CLAIM]: TURN,
  });
  const jwt = `${JWT_HEADER}.${JWT}.${PLACEHOLDER_SIGNATURE}`;
  const file = JSON.stringify({
    auth_mode: "chatgpt",
    OPENAI_API_KEY: null,
    // The refresh token carries the turn token too: codex's refresh sends no Authorization, only this in its body.
    tokens: { id_token: jwt, access_token: jwt, refresh_token: `${PLACEHOLDER_REFRESH_PREFIX}${TURN}`, account_id: accountId },
    last_refresh: new Date(now).toISOString(),
  });
  return write([`jwt=$(printf %s ${spliced(payload, words)} | base64 | tr -d '\\n=' | tr '+/' '-_')`, `printf %s ${spliced(file, words)} > "$CODEX_HOME/auth.json"`]);
}

const AZURE_MARKERS = ["openai.azure.", "cognitiveservices.azure.", "aoai.azure.", "azure-api.", "azurefd.", "windows.net/openai"];
/** The placeholder key as a provider's auth command prints it, off the environment that command runs in. */
const PRINT_KEY = ["-c", `printf %s "${PLACEHOLDER_KEY_PREFIX}$${HOST_TOKEN_ENV}"`];

/** One of the person's providers as the machine runs it, as overrides of the entry the copied config holds: codex
 * merges a `-c` table into the file's rather than replacing it (measured on codex-cli 0.162.1), so a credential field
 * cannot be taken away, only given the placeholder key. The base_url goes to the door, which maps it onto the one the
 * host recorded; an auth command prints the placeholder key, a bearer token is it, and any other entry reads it through
 * env_key, unless it asks for OpenAI's own sign-in and so takes the placeholder login. Every http_headers value goes
 * blank. An entry codex read as Azure keeps a name that reads azure, since a loopback URL carries no Azure marker. An
 * AWS-signed entry keeps today's road, untouched. */
export function providerFlags(id: string, entry: Readonly<Record<string, unknown>>, mode: CodexBroker["mode"]): { flags: string[]; keyed: boolean } {
  if (!SERVER_NAME_RE.test(id)) throw new Error(`a model provider's id must be one plain word of a config key, got "${id}"`);
  if (entry.aws !== undefined) return { flags: [], keyed: false };
  const at = `model_providers.${id}`;
  const name = typeof entry.name === "string" ? entry.name : "";
  const url = typeof entry.base_url === "string" ? entry.base_url.toLowerCase() : "";
  const azure = name.toLowerCase() !== "azure" && AZURE_MARKERS.some(m => url.includes(m));
  const headers = typeof entry.http_headers === "object" && entry.http_headers !== null ? Object.keys(entry.http_headers) : [];
  const openaiAuth = entry.requires_openai_auth === true;
  const keyed = !openaiAuth && entry.auth === undefined && entry.experimental_bearer_token === undefined;
  const credential =
    entry.auth !== undefined
      ? [config(`${at}.auth.command`, "sh"), configRaw(`${at}.auth.args`, JSON.stringify(PRINT_KEY))]
      : entry.experimental_bearer_token !== undefined
        ? [`-c ${at}.experimental_bearer_token='"${PLACEHOLDER_KEY_PREFIX}'"$${HOST_TOKEN_ENV}"'"'`]
        : keyed
          ? [config(`${at}.env_key`, PROVIDER_KEY_ENV)]
          : [];
  return {
    keyed,
    flags: [
      config(`${at}.base_url`, openaiAuth && mode === "chatgpt" ? `${DOOR}/backend-api/codex` : `${DOOR}/v1`),
      ...(azure ? [config(`${at}.name`, "azure")] : []),
      ...credential,
      ...(headers.length > 0 ? [configRaw(`${at}.http_headers`, `{${headers.map(h => `${JSON.stringify(h)}=""`).join(",")}}`)] : []),
    ],
  };
}

const providersOf = (broker: CodexBroker): { flags: string[]; keyed: boolean }[] =>
  Object.entries(broker.providers ?? {}).map(([id, entry]) => providerFlags(id, entry, broker.mode));

/** The overrides and the steps a brokered launch adds: the placeholder login, the placeholder key the providers that
 * read one find, both base URLs at the door, the login read from its file, analytics off, and each provider's fields.
 * A copied config that keeps the login in the keyring would hide the file: codex-cli 0.162.1 reads only the keyring
 * then, and a machine has no secret service. */
function brokeredLine(broker: CodexBroker, now: number, turn: boolean): { steps: string[]; flags: string[] } {
  const providers = providersOf(broker);
  return {
    steps: [placeholderLogin(broker, now, turn), ...(broker.mode === "apikey" ? [`unset ${KEY_FILE_ENV}`] : []), ...(providers.some(p => p.keyed) ? [`export ${PROVIDER_KEY_ENV}="${PLACEHOLDER_KEY_PREFIX}$${HOST_TOKEN_ENV}"`] : [])],
    flags: doorFlags(broker, providers, "file"),
  };
}

function doorFlags(broker: CodexBroker, providers: readonly { flags: string[] }[], store: "file" | "ephemeral"): string[] {
  return [
    config("openai_base_url", broker.mode === "chatgpt" ? `${DOOR}/backend-api/codex` : `${DOOR}/v1`),
    config("chatgpt_base_url", `${BACKEND_DOOR}/backend-api/`),
    config("cli_auth_credentials_store", store),
    configRaw("analytics.enabled", "false"),
    ...providers.flatMap(p => p.flags),
  ];
}

/** How every Codex launch that is not a turn starts codex, on the environment buildEnv gives it: on a brokered
 * machine, a shell that writes the placeholder login and becomes codex, and the broker's overrides, so a title or a
 * draft reaches the door and never chatgpt.com. The overrides go beside any `-c` the subcommand takes: codex-cli
 * 0.162.1 drops every root `-c` once the subcommand names one of its own (measured on `codex exec`). */
export function codexLaunch(launch: AgentLaunch | undefined, broker: CodexBroker | undefined): { word: string; flags: string[] } {
  if (broker === undefined) return { word: programWord("codex", launch), flags: [] };
  const { steps, flags } = brokeredLine(broker, Date.now(), false);
  return { word: `sh -c ${shellQuote([...steps, 'exec "$0" "$@"'].join(" && "))} ${programWord("codex", launch)}`, flags };
}

/** How a launch that asks no model starts codex, a probe, a rename or a conversation read, with the overrides ahead of
 * the subcommand. On a brokered machine it runs at the door on no login at all, since outside a turn the token the
 * last turn wrote is dead, and with plugins off, whose sync codex runs at every start: on codex-cli 0.163.0 a probe on
 * the last turn's login made 11 calls the door refused 403, and one on neither made none. With no login, model/list
 * answers only the models the binary ships: codex keys its models cache by the login, so the account's own list a turn
 * fetched is never read (codex-cli 0.155.1). A probe takes the account's list from the host instead. */
export function codexWord(launch: AgentLaunch | undefined, broker: CodexBroker | undefined): string {
  const program = programWord("codex", launch);
  if (broker === undefined) return program;
  const { steps, flags } = noLoginLine(broker);
  const word = steps.length > 0 ? `sh -c ${shellQuote([...steps, 'exec "$0" "$@"'].join(" && "))} ${program}` : program;
  return [word, ...flags].join(" ");
}

function noLoginLine(broker: CodexBroker): { steps: string[]; flags: string[] } {
  return { steps: broker.mode === "apikey" ? [`unset ${KEY_FILE_ENV}`] : [], flags: [...doorFlags(broker, providersOf(broker), "ephemeral"), configRaw("features.plugins", "false")] };
}

/** The program a terminal on a brokered machine finds as codex in `folder`, ahead of the real one on its PATH: codex
 * reads its base URLs off flags and its login off a file alone, so what the person types runs as codexLaunch starts
 * codex, on the shell's own token, with `folder` taken off the PATH first so the real codex is the one it becomes. The
 * overrides go ahead of the subcommand, so a subcommand given a `-c` of its own drops them. */
export function codexForShell(broker: CodexBroker, folder: string): string {
  const { word, flags } = codexLaunch(undefined, broker);
  const unpath = `IFS=:; p=; for d in $PATH; do [ "$d" = ${shellQuote(folder)} ] || p=\${p:+$p:}$d; done; PATH=$p; unset IFS`;
  // codex exec refuses a folder outside a git repo unless told otherwise, whatever its trust table says (codex-rs
  // exec/src/lib.rs at rust-v0.163.0), and a sub-thread's folder holds none until its agent fetches the code. The
  // flag given twice is refused (codex-cli 0.155.1: "cannot be used multiple times"), so one already typed stands.
  const anyFolder = 'case "$1" in exec|e) k=; for a; do [ "$a" = --skip-git-repo-check ] && k=1; done; [ -n "$k" ] || { s=$1; shift; set -- "$s" --skip-git-repo-check "$@"; };; esac';
  // A pane's shell names no CODEX_HOME, and codex's own default is ~/.codex.
  return ["#!/bin/sh", unpath, 'export CODEX_HOME="${CODEX_HOME:-$HOME/.codex}"', anyFolder, `exec ${[word, ...flags].join(" ")} "$@"`, ""].join("\n");
}

export interface BuildCommandOptions {
  cwd?: string;
  /** MCP servers this turn gets besides the ones its config names, by the name each takes there. */
  mcpServers?: Readonly<Record<string, McpServerSpec>>;
  /** The program run in place of codex and the words added after app-server, from the person's setup there. */
  launch?: AgentLaunch;
  broker?: CodexBroker;
  /** The config's own servers this launch leaves out, by name: on a cloud machine, those that need the person's sign-in. */
  offServers?: readonly string[];
  /** A server run that starts no turn, a revert or a fork's look for its turn: it asks no model, so it runs on no login
   * as codexWord's launches do, since a revert carries no token and the last turn's is dead. */
  turn?: false;
}

/** A value that may ride a codex command line or a SQL literal unquoted; anything else is refused before it does. */
export function slug(name: string, value: string): string {
  if (!SLUG_RE.test(value)) throw new Error(`${name} must be a plain slug, got "${value}"`);
  return value;
}

/** A config override whose value is a TOML string, which is JSON's quoting for these plain words. */
const config = (key: string, value: string): string => `-c ${key}=${shellQuote(JSON.stringify(value))}`;

/** A config override whose value is TOML already: a JSON array of strings is one. */
const configRaw = (key: string, toml: string): string => `-c ${key}=${shellQuote(toml)}`;

const SERVER_NAME_RE = /^[A-Za-z0-9_-]+$/;

/** Each server as a whole config entry: an override naming only env_vars for a server the config does not hold stops
 * codex at start (measured on codex-cli 0.155.1). */
function serverFlags(servers: Readonly<Record<string, McpServerSpec>>): string[] {
  const entry = (name: string, spec: McpServerSpec): string[] => {
    const at = `mcp_servers.${name}`;
    return [
      config(`${at}.command`, spec.command),
      configRaw(`${at}.args`, JSON.stringify(spec.args)),
      // Codex hands a server only its own short list of variables (codex-rs/rmcp-client/src/utils.rs at
      // rust-v0.155.1), so the launch's are named for the wsp one: names only, the values stay in the environment.
      ...(name === MCP_SERVER_NAME || name === SLATE_SERVER_NAME ? [configRaw(`${at}.env_vars`, JSON.stringify(LAUNCH_ENV)), configRaw(`${at}.tool_timeout_sec`, String(WSP_TOOL_TIMEOUT_SEC))] : []),
    ];
  };
  return Object.entries(servers).flatMap(([name, spec]) => {
    if (!SERVER_NAME_RE.test(name)) throw new Error(`an MCP server name must be one plain word of a config key, got "${name}"`);
    if (name !== MCP_SERVER_NAME) return entry(name, spec);
    const slate = JSON.stringify(SLATE_TOOLS);
    // A thread another thread started has no slate: its wsp server lists none of the slate's tools and no slate server stands.
    if (!launchHasSlate(servers)) return entry(name, spec);
    return [
      ...entry(name, spec),
      configRaw(`mcp_servers.${name}.disabled_tools`, slate),
      ...entry(SLATE_SERVER_NAME, spec),
      configRaw(`mcp_servers.${SLATE_SERVER_NAME}.enabled_tools`, slate),
      configRaw(`mcp_servers.${SLATE_SERVER_NAME}.omit_tools_from`, JSON.stringify(["deferred"])),
      // The slate's own four run without an ask in any sandbox or approval policy, each by name, so nothing else on
      // the wsp server ever does (approval_mode, codex-rs/codex-mcp/src/mcp/mod.rs at rust-v0.155.1).
      ...SLATE_TOOLS.map(tool => config(`mcp_servers.${SLATE_SERVER_NAME}.tools.${tool}.approval_mode`, "approve")),
    ];
  });
}

/** One `enabled=false` override per off server, each only where the config under CODEX_HOME still names it: an
 * override for a server the bootstrap config does not hold, one a project file alone names or one removed since the
 * list was read, stops app-server at start with `invalid transport` (codex-cli 0.155.1). A name the check misses
 * keeps its server on. */
function offFlags(names: readonly string[]): string[] {
  return names.filter(name => SERVER_NAME_RE.test(name)).map(name => `$(grep -Eqs ${shellQuote(`^[[:space:]]*(\\[mcp_servers\\.${name}[].]|mcp_servers\\.${name}\\.)`)} "$CODEX_HOME/config.toml" && printf %s ${shellQuote(`-c mcp_servers.${name}.enabled=false`)})`);
}

export interface AccessParams {
  sandbox: "read-only" | "workspace-write" | "danger-full-access";
  approvalPolicy: "on-request" | "never";
}

/** A sandboxed mode asks the person before the agent goes past its sandbox; full access, and a turn that names no
 * mode, as every turn in a throwaway machine does, runs unsandboxed and asks nobody. */
export function accessParams(mode: string | undefined): AccessParams {
  if (mode === undefined || mode === NO_SANDBOX) return { sandbox: NO_SANDBOX, approvalPolicy: "never" };
  const sandboxed = SANDBOXED.find(m => m === mode);
  if (sandboxed === undefined) throw new Error(`permissionMode must be one of ${[...SANDBOXED, NO_SANDBOX].join(", ")}, got "${mode}"`);
  return { sandbox: sandboxed, approvalPolicy: "on-request" };
}

/** The server as one bash line in the folder the turn runs in. Guest exec carries no HOME, so the default folder is
 * `~`, which bash reads from passwd. */
export function buildCommand(options: BuildCommandOptions): string {
  const brokered = options.broker === undefined ? { steps: [], flags: [] } : options.turn === false ? noLoginLine(options.broker) : brokeredLine(options.broker, Date.now(), true);
  // The plan tool is off in a headless server unless its config turns it on; its updates are the turn's steps.
  const server = [`${agentWord("codex", options.launch)} app-server`, ...launchWords(options.launch), configRaw("tools.update_plan.enabled", "true"), ...brokered.flags, ...serverFlags(options.mcpServers ?? {}), ...offFlags(options.offServers ?? [])].join(" ");
  return inFolder(options.cwd, [...brokered.steps, server].join(" && "));
}

/** An image path is a plain absolute path on the machine, where the runtime landed it before the turn. */
export function imagePath(path: string): string {
  if (!path.startsWith("/") || path.includes("\n")) throw new Error(`an image path must be one absolute path on the machine, got "${path}"`);
  return path;
}
