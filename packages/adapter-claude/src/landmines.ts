// Encoded Claude Code deployment quirks. Sources: pingdotgg/t3code (MIT, see
// NOTICE; logic only) and measured behavior in solari-poc/RESULTS.md.

import { randomUUID } from "node:crypto";
import { agentWord, BROKER_PORT, inFolder, launchHasSlate, MCP_SERVER_NAME, programWord, shellLine, shellQuote, SLATE_BRIEF } from "@wsp/protocol";
import type { AgentLaunch, BrokerStatus, McpServerSpec, TurnImage } from "@wsp/protocol";
import { PERMISSION_PROMPT_TOOL, SKIP_PROMPTS_MODE } from "./permissions.js";

// Inherited CLAUDE_CODE_*/CLAUDECODE mark the child as nested inside another
// Claude Code run; FORCE_CODE_TERMINAL flips terminal detection (t3code unsets
// it for headless probes).
export const ENV_STRIP_PATTERNS: readonly RegExp[] = [
  /^CLAUDE_CODE_/,
  /^CLAUDECODE$/,
  /^FORCE_CODE_TERMINAL$/,
];

// From t3code's probe options: headless runs must not probe for IDEs, or the
// CLI spawns discovery process trees on every invocation. A headless run also
// leaves the task list tools off, which the CLI's own terminal has on. A print
// run ends background tasks still working 600 s after its agent goes idle
// (claude-code#98170); 0 is the CLI's own word for never, so the turn's held
// reply and the person's stop are the only ends a background subagent has.
// Unset, a stream-json run given --mcp-config holds its first model call until
// every server has connected (8.3 s behind one 8 s server, 2.1.299 and 2.1.300);
// at 0 it calls at once and names a late server on its next request.
const HEADLESS_OVERRIDES = {
  CLAUDE_CODE_AUTO_CONNECT_IDE: "0",
  CLAUDE_CODE_IDE_SKIP_AUTO_INSTALL: "1",
  CLAUDE_CODE_ENABLE_TODO_TOOLS: "1",
  CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS: "0",
  CLAUDE_CODE_MCP_STARTUP_WAIT_MS: "0",
} as const;

/** The variable that tells the CLI which folder under its projects directory to keep this run's sessions and its
 * auto memory in. Set after the strip, never through `base`: the strip drops every inherited CLAUDE_CODE_* as a
 * nesting mark, and this one is ours. */
export const PROJECT_DIR_ENV = "CLAUDE_CODE_PROJECT_DIR_NAME";

/** The command that opens one of this CLI's sessions in the person's own terminal, the session id going after it: the
 * program a turn runs, under the config folder a turn's environment names, since a session is found only in the store
 * it was written to. Measured on 2.1.296: `claude --resume <id>` opens a print-mode session from any folder of that
 * store, where the picker, `-c` and a resume by title never list it. */
export function terminalResumeCommand(o: { base?: Readonly<Record<string, string | undefined>>; launch?: AgentLaunch }): string {
  const dir = o.base?.["CLAUDE_CONFIG_DIR"];
  return `${dir === undefined ? "" : `CLAUDE_CONFIG_DIR=${shellLine([dir])} `}${programWord("claude", o.launch)} --resume`;
}

export interface ClaudeEnvOptions {
  /** The machine's login environment: a guest's carries its config dir and IS_SANDBOX, a person's own carries theirs. */
  base?: Readonly<Record<string, string | undefined>>;
  apiKey?: string;
  /** The long-lived token from claude setup-token. Set after the strip: the strip removes an inherited CLAUDE_CODE_*
   * as a nesting mark, and this one is ours. An API key beside it wins inside the CLI, so the caller hands one or
   * the other and never both, decided by what the vault holds: its token where there is one, else its key. */
  oauthToken?: string;
  /** The folder under the CLI's projects directory this run keys its sessions and its memory to, which the CLI reads
   * only beside CLAUDE_CONFIG_DIR; the launch names the memory folder as well, see memorySettings. Absent leaves the
   * CLI keying off the folder the turn runs in. */
  projectDirName?: string;
  /** A launch on a brokered place: the host holds the credential and adds it at the door, so the launch carries the
   * turn token in a placeholder of the credential's kind and nothing the vault or the login holds. Wins over apiKey
   * and oauthToken. */
  brokered?: BrokeredSignIn;
}

/** The turn token and the kind of credential the host holds for it, as broker.status answers it. A borrowed claude.ai
 * login reaches the CLI as a setup-token does. */
export interface BrokeredSignIn {
  token: string;
  kind: NonNullable<BrokerStatus["claude"]>;
}

/** The daemon's HTTP door on the machine's loopback, where the host adds the real credential. */
export const BROKER_DOOR_URL = `http://127.0.0.1:${BROKER_PORT}`;

/** Each variable through which a credential, or a route around the door, would reach the CLI. */
const BROKERED_DROP = ["ANTHROPIC_AUTH_TOKEN", "ANTHROPIC_BASE_URL", "ANTHROPIC_CUSTOM_HEADERS", "ANTHROPIC_FOUNDRY_API_KEY", "AWS_BEARER_TOKEN_BEDROCK"] as const;

/** The placeholder per kind, which must match the credential the host adds: the CLI sends a token as a Bearer with the
 * oauth beta and a key as x-api-key without it (2.1.296). A gateway's Bearer needs the key set empty. */
const PLACEHOLDER: Readonly<Record<BrokeredSignIn["kind"], (token: string) => Record<string, string>>> = {
  login: token => ({ CLAUDE_CODE_OAUTH_TOKEN: token }),
  token: token => ({ CLAUDE_CODE_OAUTH_TOKEN: token }),
  key: token => ({ ANTHROPIC_API_KEY: token }),
  gateway: token => ({ ANTHROPIC_AUTH_TOKEN: token, ANTHROPIC_API_KEY: "" }),
};

/** A brokered launch's own variables. Without the nonessential-traffic switch side calls carry the placeholder
 * straight to api.anthropic.com; without the provider switch a copied settings env block's base URL beats the
 * door's and the turn hangs; DISABLE_AUTOUPDATER leaves `claude update` working where DISABLE_UPDATES does not; the
 * client waits API_TIMEOUT_MS on a request the host holds; and a base URL that is not first party turns MCP tool
 * search off, which only a gateway's upstream should. */
function brokeredEnv(b: BrokeredSignIn): Record<string, string> {
  return {
    ANTHROPIC_BASE_URL: BROKER_DOOR_URL,
    ...PLACEHOLDER[b.kind](b.token),
    CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC: "1",
    CLAUDE_CODE_PROVIDER_MANAGED_BY_HOST: "1",
    DISABLE_UPDATES: "1",
    API_TIMEOUT_MS: "3600000",
    ...(b.kind === "gateway" ? {} : { ENABLE_TOOL_SEARCH: "true" }),
  };
}

export function stripLandmineEnv(
  base: Readonly<Record<string, string | undefined>>,
): Record<string, string> {
  const clean: Record<string, string> = {};
  for (const [key, value] of Object.entries(base)) {
    if (value === undefined) continue;
    if (ENV_STRIP_PATTERNS.some((pattern) => pattern.test(key))) continue;
    clean[key] = value;
  }
  return clean;
}

/**
 * Sets neither CLAUDE_CONFIG_DIR nor IS_SANDBOX: both ride in `base` where they are true. The CLI keys its Keychain
 * item by whether the variable is set, not by its path (the service name gains a hash of the path once it is set),
 * so a Mac that exports it even as ~/.claude reports a claude.ai login as "Not logged in" (measured on 2.1.257 and
 * 2.1.259, 2026-09-10). A machine's login carries IS_SANDBOX=1, what lets --dangerously-skip-permissions run as root
 * there (solari-poc P1), and a guest's carries its config dir, never a changed HOME, which relocates the Keychain
 * lookup (t3code ClaudeHome.ts).
 */
export function buildEnv(options: ClaudeEnvOptions = {}): Record<string, string> {
  const { [KEY_ENV]: _inherited, ...base } = stripLandmineEnv(options.base ?? {});
  if (options.brokered !== undefined) {
    for (const name of BROKERED_DROP) delete base[name];
    return {
      ...base,
      ...HEADLESS_OVERRIDES,
      ...brokeredEnv(options.brokered),
      ...(options.projectDirName === undefined ? {} : { [PROJECT_DIR_ENV]: options.projectDirName }),
    };
  }
  return {
    ...base,
    ...HEADLESS_OVERRIDES,
    ...(vaulted(options) ? {} : inheritedSignIn(options.base ?? {})),
    ...(options.apiKey === undefined ? {} : { ANTHROPIC_API_KEY: options.apiKey }),
    ...(options.oauthToken === undefined ? {} : { CLAUDE_CODE_OAUTH_TOKEN: options.oauthToken }),
    ...(options.projectDirName === undefined ? {} : { [PROJECT_DIR_ENV]: options.projectDirName }),
  };
}

export const KEY_ENV = "ANTHROPIC_API_KEY";
const TOKEN_ENV = "CLAUDE_CODE_OAUTH_TOKEN";

const vaulted = (options: ClaudeEnvOptions): boolean => options.brokered !== undefined || options.apiKey !== undefined || options.oauthToken !== undefined;

/** A token or key the launch inherited, from the login shell or the terminal that started the host: the token where
 * both are, since inside the CLI the key outranks it and the token would never be used. */
function inheritedSignIn(base: Readonly<Record<string, string | undefined>>): Record<string, string> {
  const token = base[TOKEN_ENV];
  if (token !== undefined && token !== "") return { [TOKEN_ENV]: token };
  const key = base[KEY_ENV];
  return key !== undefined && key !== "" ? { [KEY_ENV]: key } : {};
}

/** The words a launch starts with when it would sign in with what it inherited. An inherited token or key is a
 * fallback: wsp's own sign-in from the vault stands above it, which buildEnv already settles, and so does the person's
 * own Claude login on the computer (a Keychain or file login, a cloud provider), which only the CLI can see. So the CLI
 * is asked with the inherited values out of the way, and they are unset where it reports one. An apiKeyHelper is not
 * such a login: the CLI ranks a key in its environment above it, and auth status reports one without running it
 * (2.1.296). `claude -p` uses a key from its environment even one the person declined in their own `claude`. Nothing
 * where the vault signs the launch in or nothing was inherited. */
export function signInGuard(options: ClaudeEnvOptions & { launch?: AgentLaunch } = {}): string {
  if (vaulted(options) || Object.keys(inheritedSignIn(options.base ?? {})).length === 0) return "";
  const status = `env -u ${KEY_ENV} -u ${TOKEN_ENV} ${programWord("claude", options.launch)} auth status 2>/dev/null`;
  return `wsp_auth=$(${status}); case "$wsp_auth" in *'"authMethod": "api_key_helper"'*) ;; *'"loggedIn": true'*) unset ${KEY_ENV} ${TOKEN_ENV} ;; esac; unset wsp_auth; `;
}

/**
 * The caller generates the session UUID and passes it via --session-id, so the
 * session is addressable (registry, transcript path, --resume) before the CLI
 * prints anything (t3code startSession).
 */
export function newSessionId(): string {
  return randomUUID();
}

export const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export interface BuildCommandOptions {
  /** Fresh session: the self-generated UUID passed as --session-id. */
  sessionId?: string;
  /** Existing session: passed as --resume instead. */
  resume?: string;
  /** Beside a fresh sessionId: the session that one starts as a copy of, `--resume <copyOf> --fork-session`, which the
   * CLI's own `--bg --resume` runs on a session open in another process (measured on 2.1.296: the copy's file holds the
   * original's active branch, every line stamped sdk-cli, and the original's file is byte for byte as it was). */
  copyOf?: string;
  cwd?: string;
  /** The CLI's own slugs, from the harness catalog; absent leaves the CLI's default in place. */
  model?: string;
  effort?: string;
  /** The CLI's own mode slug; absent keeps skipping permissions, what every session did before there was a picker. */
  permissionMode?: string;
  /** "1m" or "200k" from the catalog; the CLI takes 1M as a "[1m]" suffix on the model, so it needs one. */
  contextWindow?: string;
  /** The display name the session is opened under, whatever characters it holds; the CLI writes it into the session's
   * own store as the person's, which is where its resume list and this adapter's title read both take it from. */
  name?: string;
  /** MCP servers this turn gets on top of the config dir's own, by the name each takes in a config. */
  mcpServers?: Readonly<Record<string, McpServerSpec>>;
  /** The run's environment names a file under SERVER_VALUES_ENV holding servers with their values, which stand for
   * the config's own servers of those names on this launch alone. */
  serverValues?: true;
  /** The config's own servers this launch leaves out, by name. A deniedMcpServers entry passed in --settings drops one
   * whole: it is not started, listed or offered a sign-in (2.1.299; a denylist merges from every settings scope). */
  offServers?: readonly string[];
  /** The model's faster output. The CLI takes it as the fastMode setting, which --settings carries for this launch
   * alone; the result's fast_mode_state says whether the account served it (2.1.283, 2026-09-27). */
  fast?: boolean;
  /** The folder the run keeps its auto memory in; see memorySettings. */
  memoryDir?: string;
  /** The program run in place of claude and the words added after -p, from the person's setup on that computer. */
  launch?: AgentLaunch;
  /** Forward a subagent's own text and thinking, not only its tool calls; see forwardsSubagentText. */
  subagentText?: boolean;
  /** A side question: `resume` names the copy of the thread's session it is asked on, at the default mode with its
   * prompts routed here, hooks off, two calls at most and the answer's words streamed. Every flag that shapes the
   * request stays the turn's, since a copy whose tools or system prompt differ reads none of the thread's cache. */
  aside?: boolean;
}

/** The settings that put a run's auto memory in `dir`. The only road to it that holds on a computer with no
 * CLAUDE_CONFIG_DIR: 2.1.280 reads PROJECT_DIR_ENV only beside that variable, and keys a worktree's memory to its
 * repo's main checkout. */
export const memorySettings = (dir: string | undefined): { autoMemoryDirectory?: string } => (dir === undefined ? {} : { autoMemoryDirectory: dir });

/** The settings that leave the named servers out of a launch. */
const deniedServers = (names: readonly string[] | undefined): Record<string, unknown> => (names === undefined || names.length === 0 ? {} : { deniedMcpServers: names.map(serverName => ({ serverName })) });

/** Where a launch on a cloud machine denies its off servers to every claude there: a `claude mcp list` the agent runs
 * never sees the turn's --settings, but reads this managed drop-in, merged with any other (2.1.299 on Boat: list, get
 * and login all answer `No MCP server named`, 2026-10-14). */
const OFF_SERVERS_DIR = "/etc/claude-code/managed-settings.d";
export const OFF_SERVERS_FILE = `${OFF_SERVERS_DIR}/wsp-servers-off.json`;

/** The step ahead of a launch that leaves `names` out, run before its cd: a write that fails costs the drop-in, and
 * the turn keeps its --settings deny. An empty list removes the last launch's, which would hide a server on now. */
const offServersStep = (names: readonly string[] | undefined): string =>
  names === undefined ? "" : names.length === 0 ? `rm -f ${OFF_SERVERS_FILE} 2>/dev/null; ` : `{ mkdir -p ${OFF_SERVERS_DIR} && printf '%s' ${shellQuote(JSON.stringify(deniedServers(names)))} > ${OFF_SERVERS_FILE}; } 2>/dev/null; `;

/** One --settings flag for every setting a launch carries, none where it carries none. */
export const settingsFlag = (settings: Readonly<Record<string, unknown>>): string[] =>
  Object.keys(settings).length === 0 ? [] : [`--settings ${shellQuote(JSON.stringify(settings))}`];

/** The person's launch words less a --settings that holds a JSON object, and that object, for the launch to fold into
 * its own one flag: 2.1.280 keeps the last --settings whole and drops any before it. A --settings naming a file is
 * the CLI's to read, so the words keep it and `file` says the launch adds no flag of its own. */
export function personSettings(args: readonly string[]): { words: string[]; settings: Record<string, unknown>; file: boolean } {
  const words: string[] = [];
  let settings: Record<string, unknown> = {};
  for (let i = 0; i < args.length; i++) {
    const arg = args[i]!;
    const joined = arg.startsWith("--settings=");
    if (arg !== "--settings" && !joined) {
      words.push(arg);
      continue;
    }
    const value = joined ? arg.slice("--settings=".length) : args[++i];
    let parsed: unknown;
    try {
      parsed = JSON.parse(value ?? "");
    } catch {
      parsed = undefined;
    }
    if (typeof parsed !== "object" || parsed === null || Array.isArray(parsed)) return { words: [...args], settings: {}, file: true };
    settings = parsed as Record<string, unknown>;
  }
  return { words, settings, file: false };
}

/** The person's launch words less every --settings and the value it names. */
function withoutSettings(args: readonly string[]): string[] {
  const words: string[] = [];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === "--settings") i++;
    else if (!args[i]!.startsWith("--settings=")) words.push(args[i]!);
  }
  return words;
}

/** The first CLI that takes --forward-subagent-text: the SDK of 0.3.270 passes it, and a CLI before it exits on a flag
 * it does not know. Below it, or where no version was read, a subagent still shows off its tool calls. */
const SUBAGENT_TEXT_SINCE = [2, 1, 270] as const;

export function forwardsSubagentText(version: string | undefined): boolean {
  const parts = /^(\d+)\.(\d+)\.(\d+)/.exec(version ?? "")?.slice(1).map(Number);
  if (parts === undefined) return false;
  for (let i = 0; i < SUBAGENT_TEXT_SINCE.length; i++) if (parts[i] !== SUBAGENT_TEXT_SINCE[i]) return parts[i]! > SUBAGENT_TEXT_SINCE[i]!;
  return true;
}

// Model names carry a context suffix like "claude-opus-5[1m]"; nothing else a catalog value needs is outside this set.
const SLUG_RE = /^[A-Za-z0-9][A-Za-z0-9._:\[\]-]*$/;

export function slugFlag(flag: string, name: string, value: string | undefined): string[] {
  if (value === undefined) return [];
  if (!SLUG_RE.test(value)) throw new Error(`${name} must be a plain slug, got "${value}"`);
  return [`${flag} ${shellQuote(value)}`];
}

function modelWithContext(model: string | undefined, contextWindow: string | undefined): string | undefined {
  if (contextWindow === undefined) return model;
  if (model === undefined) throw new Error("contextWindow needs a model to ride on");
  if (contextWindow === "200k") return model;
  if (contextWindow === "1m") return `${model}[1m]`;
  throw new Error(`contextWindow must be "200k" or "1m", got "${contextWindow}"`);
}

/**
 * The access flags for one mode. Bypass, and no mode at all, skip permissions outright. Every other mode names
 * itself, "default" included: sending no flag for it left the person's own settings deciding the turn's access, and
 * on 2.1.263 a turn launched that way came back in the auto mode their store had (measured 2026-09-08), which is
 * not what the picker said. A mode that is not bypass may raise a prompt, and --permission-prompt-tool routes it to
 * this process over the control channel; without the flag the CLI denies every such call by itself. Bypass takes the
 * flag too: without it the CLI has no road to a person and switches AskUserQuestion off, while with it every other
 * tool still runs unasked and only the question comes here (measured 2026-10-03).
 */
function permissionFlags(mode: string | undefined): string[] {
  const prompts = `--permission-prompt-tool ${PERMISSION_PROMPT_TOOL}`;
  if (mode === undefined || mode === SKIP_PROMPTS_MODE) return ["--dangerously-skip-permissions", prompts];
  return [...slugFlag("--permission-mode", "permissionMode", mode), prompts];
}

/** The slate's brief at the end of the system prompt, for a turn whose launch gives its thread a slate. */
function briefFlag(servers: Readonly<Record<string, McpServerSpec>> | undefined): string[] {
  return launchHasSlate(servers) ? [`--append-system-prompt ${shellQuote(SLATE_BRIEF)}`] : [];
}

/** The variable a run's environment names its servers-with-values file under: a path, the values being in the file. A
 * second --mcp-config stands for the config's own server of a name it holds (measured on 2.1.280). */
export const SERVER_VALUES_ENV = "WSP_MCP_VALUES";

/** What a run lands for its servers with their values, a turn's or a side question's alike: one file keyed by
 * SERVER_VALUES_ENV; nothing where no server reads a value. */
export function serverValuesFile(values: { entries?: Readonly<Record<string, Readonly<Record<string, unknown>>>> } | undefined): { files: Record<string, string> } | undefined {
  const entries = values?.entries ?? {};
  return Object.keys(entries).length === 0 ? undefined : { files: { [SERVER_VALUES_ENV]: JSON.stringify({ mcpServers: entries }) } };
}

/**
 * The servers a turn is handed, as this CLI takes them: one --mcp-config carrying the JSON a config file would hold.
 * Not --strict-mcp-config, which would drop the config dir's own servers and leave the turn with these alone.
 */
export function mcpConfigFlag(servers: Readonly<Record<string, McpServerSpec>> | undefined): string[] {
  if (servers === undefined || Object.keys(servers).length === 0) return [];
  // The wsp server loads its tools up front and marks every one but the slate's to stay behind tool search: Claude Code
  // 2.1.289 takes a tool's own alwaysLoad only as an exception to its server's, and Haiku never searched for the slate.
  // With no slate it buys nothing, and the first call still waits up to 5 s on an alwaysLoad server (2.1.299).
  const upFront = launchHasSlate(servers);
  const mcpServers = Object.fromEntries(Object.entries(servers).map(([name, s]) => [name, { command: s.command, args: [...s.args], ...(upFront && name === MCP_SERVER_NAME ? { alwaysLoad: true } : {}) }]));
  return [`--mcp-config ${shellQuote(JSON.stringify({ mcpServers }))}`];
}

/**
 * Print-mode stream-json refuses to run without --verbose. `claude -p` reads
 * stdin to the end, so stdin is either closed or a stream-json channel the
 * caller writes and closes on purpose, never a silent open pipe (solari-poc
 * probes, RESULTS.md P1): the prompt and every later message are user lines
 * on that channel, and EOF ends the process after its current turn.
 */
export function buildCommand(options: BuildCommandOptions): string {
  const { sessionId, resume, cwd, model, effort, permissionMode, contextWindow, name, mcpServers, fast, memoryDir, subagentText } = options;
  const named = personSettings(options.launch?.args ?? []);
  // A settings file in the person's words would replace the copy's own flag and its disableAllHooks with it, so the
  // copy leaves that file out and keeps its hooks off.
  const person = options.aside === true && named.file ? { words: withoutSettings(options.launch?.args ?? []), settings: {}, file: false } : named;
  if ((sessionId === undefined) === (resume === undefined)) {
    throw new Error("buildCommand needs exactly one of sessionId or resume");
  }
  const id = sessionId ?? resume ?? "";
  if (!UUID_RE.test(id)) {
    throw new Error(`session identifier must be a UUID, got "${id}"`);
  }
  if (options.copyOf !== undefined && (sessionId === undefined || !UUID_RE.test(options.copyOf))) throw new Error(`a copy opens a new session of a session named by its UUID, got "${options.copyOf}"`);
  const aside = options.aside === true;
  if (aside && resume === undefined) throw new Error("a side question resumes the copy it is asked on");
  const idFlag =
    aside
      ? `--max-turns 2 --resume ${id}`
      : sessionId === undefined
        ? `--resume ${id}`
        : `--session-id ${id}${options.copyOf === undefined ? "" : ` --resume ${options.copyOf} --fork-session`}`;
  const claude = [
    `${agentWord("claude", options.launch)} -p`,
    ...person.words.map(shellQuote),
    "--input-format stream-json",
    "--output-format stream-json",
    "--verbose",
    ...(subagentText === true ? ["--forward-subagent-text"] : []),
    ...(aside ? ["--include-partial-messages"] : []),
    ...permissionFlags(aside ? "default" : permissionMode),
    ...slugFlag("--model", "model", modelWithContext(model, contextWindow)),
    ...slugFlag("--effort", "effort", effort),
    ...(name === undefined ? [] : [`--name ${shellQuote(name)}`]),
    ...mcpConfigFlag(mcpServers),
    ...(options.serverValues === true ? [`--mcp-config "$${SERVER_VALUES_ENV}"`] : []),
    ...briefFlag(mcpServers),
    ...(person.file ? [] : settingsFlag({ ...(fast === true ? { fastMode: true } : {}), ...memorySettings(memoryDir), ...person.settings, ...deniedServers(options.offServers), ...(aside ? { disableAllHooks: true } : {}) })),
    idFlag,
  ].join(" ");
  return `${offServersStep(options.offServers)}${inFolder(cwd, claude)}`;
}

/** A turn's launch, or a launch ahead of one, in the adapter's words: a fresh session under `localId` unless it resumes
 * one, the thread's title as the CLI's name for it, and subagents' text forwarded on a version that forwards it. */
export function launchCommand(o: Omit<BuildCommandOptions, "sessionId" | "name" | "serverValues" | "subagentText"> & { title?: string; version?: string; serverValues?: unknown }, localId: string, valued: boolean): string {
  const { title, version, resume, serverValues: _values, ...rest } = o;
  return buildCommand({
    ...rest,
    ...(resume === undefined ? { sessionId: localId } : { resume }),
    ...(title !== undefined ? { name: title } : {}),
    ...(valued ? { serverValues: true as const } : {}),
    ...(forwardsSubagentText(version) ? { subagentText: true } : {}),
  });
}

/** How much of a session's file the saved cost is looked for in: the CLI appends it as its process exits, so on a
 * resume the last one sits behind the previous turn's few closing lines, and a file of any length costs one read. */
export const SAVED_SPEND_TAIL_BYTES = 8 * 1024 * 1024;

/** Prints the session's saved running cost ahead of a resumed turn's CLI: Claude Code appends its totals to the
 * session's file as cost-state lines, and a resume carries the last one into every result's total_cost_usd. */
export function savedSpendCommand(o: { configDir: string; sessionId: string }): string {
  const file = `${shellQuote(`${o.configDir}/projects`)}/*/${shellQuote(`${o.sessionId}.jsonl`)}`;
  return `wsp_saved=$(ls -1td ${file} 2>/dev/null | head -n 1); [ -z "$wsp_saved" ] || tail -c ${SAVED_SPEND_TAIL_BYTES} "$wsp_saved" | grep -F ${shellQuote('"type":"cost-state"')} | tail -n 1; `;
}

/**
 * One line of the stdin channel: a user message in the CLI's stream-json input shape. Images ride the same message as
 * base64 content blocks ahead of the text, the shape the CLI took on 2.1.263 (measured 2026-09-08: a 64px block sent
 * this way came back described), so nothing has to land on the machine for this harness.
 */
export function userMessageLine(text: string, sessionId: string, images: readonly TurnImage[] = [], uuid?: string): string {
  return JSON.stringify({
    type: "user",
    message: {
      role: "user",
      content: [
        ...images.map(image => ({ type: "image", source: { type: "base64", media_type: image.mediaType, data: image.bytes } })),
        { type: "text", text },
      ],
    },
    parent_tool_use_id: null,
    session_id: sessionId,
    // A message that carries a uuid is reported by the CLI as it queues, starts and completes it, under that uuid
    // (command_lifecycle, measured on 2.1.280); one without is never reported.
    ...(uuid !== undefined ? { uuid } : {}),
  });
}
