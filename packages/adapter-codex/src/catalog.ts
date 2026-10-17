// SPDX-License-Identifier: AGPL-3.0-only
// What the installed Codex binary reports about itself, read without a turn:
// `codex --version`, the sandbox choices in `codex --help`, and four requests
// to `codex app-server`, the JSON-RPC face codex's own clients use. model/list
// answers the models it offers with each model's reasoning efforts, config/read
// the model and provider a turn without flags would run (an OpenRouter route
// names a model no OpenAI catalog carries), and account/read whether the
// provider wants an OpenAI sign-in the machine has not got. Measured on
// codex-cli 0.153.0: all four answers land about 140 ms after the pipe opens,
// no model is called.

import { ENV_FROM_INPUT, codexNotSignedInLine } from "@wsp/protocol";
import type { AgentLaunch, HarnessCatalogAnswer, HarnessCatalogModelProbe, HarnessCatalogProbe } from "@wsp/protocol";
import { CATALOG_AGENTS, answersOf, appServerScript, initializeRequest, request, resultOf } from "@wsp/catalog";
import { codexWord, type AccountModels, type CodexBroker } from "./command.js";

const SEP = "__WSP_CATALOG_SEP__";
const INSTALL = CATALOG_AGENTS.find(a => a.id === "codex")?.installRoad;
/** wsp's npm pin for codex, which the account's list is asked for where no binary has named its version. A machine may
 * run another codex: Boat's own first-run installer puts 0.163.0 there. */
const PINNED_VERSION = INSTALL?.road === "npm" ? INSTALL.version : undefined;
/** Request ids, in the order the probe sends them; the parser reads each answer by its own id. */
const INIT = 1;
const MODELS = 2;
const CONFIG = 3;
const ACCOUNT = 4;
const REQUESTS = [initializeRequest(INIT), request(MODELS, "model/list"), request(CONFIG, "config/read"), request(ACCOUNT, "account/read")];
/** A brokered machine signs in at the door, so its codex reads no login and account/read would always say none. */
const BROKERED_REQUESTS = REQUESTS.slice(0, -1);

/**
 * The probe as one bash script for the guest, since guest exec is `bash -c` and may span lines. `cd ~` for the same
 * reason as a session: guest exec carries no HOME, and the app-server writes into the home it is pointed at, so the
 * probe runs under the session's own CODEX_HOME, read off its input with the rest of `buildEnv`.
 */
export function catalogProbeCommand(options: { launch?: AgentLaunch; broker?: CodexBroker } = {}): string {
  const codex = codexWord(options.launch, options.broker);
  const lines = options.broker === undefined ? REQUESTS : BROKERED_REQUESTS;
  const server = appServerScript(codex, [{ lines, answers: lines.length }]);
  return `${versionProbeCommand(options)}; echo ${SEP}; ${codex} --help; echo ${SEP}\n${server}`;
}

/** The probe's first command alone, under the same environment: what says whether lists read off the binary before
 * are still its lists. */
export function versionProbeCommand(options: { launch?: AgentLaunch; broker?: CodexBroker } = {}): string {
  return `cd ~ && ${ENV_FROM_INPUT}; ${codexWord(options.launch, options.broker)} --version`;
}

/** The version `codex --version` prints, the one the probe records; null where it printed none. */
export const parseVersion = (stdout: string): string | null => /(\d+\.\d+\.\d+)/.exec(stdout)?.[1] ?? null;

/** The `[possible values: ...]` list `codex --help` prints under a flag; empty when the flag or the list is missing. */
function possibleValues(help: string, flag: string): string[] {
  const at = help.indexOf(flag);
  if (at === -1) return [];
  const values = /\[possible values:\s*([^\]]*)\]/.exec(help.slice(at, at + 600));
  if (values === null) return [];
  return values[1]!.split(",").map(s => s.trim()).filter(s => s.length > 0);
}

function rec(value: unknown): Record<string, unknown> | undefined {
  return typeof value === "object" && value !== null && !Array.isArray(value) ? (value as Record<string, unknown>) : undefined;
}

function str(value: unknown): string | undefined {
  return typeof value === "string" && value !== "" ? value : undefined;
}

/** The models model/list offers, the ones it hides left out, each with the efforts and default effort it reports; the
 * effective model is the one marked default, and it leads the list where codex's own catalog does not carry it. */
function modelsOf(listed: Record<string, unknown> | undefined, configured: string | undefined): HarnessCatalogModelProbe[] {
  const rows = Array.isArray(listed?.data) ? listed.data : [];
  const models: HarnessCatalogModelProbe[] = [];
  for (const raw of rows) {
    const entry = rec(raw);
    const slug = str(entry?.id);
    if (entry === undefined || slug === undefined || entry.hidden === true) continue;
    const efforts = Array.isArray(entry.supportedReasoningEfforts)
      ? entry.supportedReasoningEfforts.map(e => str(rec(e)?.reasoningEffort)).filter((e): e is string => e !== undefined)
      : [];
    const description = str(entry.description);
    const defaultEffort = str(entry.defaultReasoningEffort);
    models.push({
      slug,
      label: str(entry.displayName) ?? slug,
      ...(description !== undefined ? { description } : {}),
      efforts,
      ...(defaultEffort !== undefined ? { defaultEffort } : {}),
      contextWindows: [],
      isDefault: configured === undefined ? entry.isDefault === true : slug === configured,
      ...(Array.isArray(entry.additionalSpeedTiers) && entry.additionalSpeedTiers.includes("fast") ? { fast: true } : {}),
    });
  }
  // A config that routes to another provider names a model codex's own catalog does not carry; it is what a turn
  // without -m runs, so it leads the list. Which efforts it takes is the provider's to answer and codex reports
  // none of them, so it names no list and every effort the catalog carries stays open to it, as it was before.
  if (configured !== undefined && !models.some(m => m.slug === configured)) {
    models.unshift({ slug: configured, label: configured, contextWindows: [], isDefault: true });
  }
  return models;
}

const num = (value: unknown): number => (typeof value === "number" && Number.isFinite(value) ? value : 0);

/** ChatGPT's /codex/models answer as model/list rows, the way codex turns the same answer into its picker with a
 * ChatGPT login: sorted by priority, the rows marked list shown, the first shown the default (codex-rs
 * protocol/src/openai_models.rs at rust-v0.155.1). Undefined where it shows no model. */
function accountRows(answer: unknown): Record<string, unknown> | undefined {
  const raw = rec(answer)?.["models"];
  if (!Array.isArray(raw)) return undefined;
  const rows = raw.map(rec).filter((m): m is Record<string, unknown> => str(m?.["slug"]) !== undefined).sort((a, b) => num(a["priority"]) - num(b["priority"]));
  const first = rows.find(m => m["visibility"] === "list");
  if (first === undefined) return undefined;
  const data = rows.map(m => ({
    id: m["slug"],
    displayName: m["display_name"],
    description: m["description"],
    supportedReasoningEfforts: (Array.isArray(m["supported_reasoning_levels"]) ? m["supported_reasoning_levels"] : []).map(l => ({ reasoningEffort: rec(l)?.["effort"] })),
    defaultReasoningEffort: m["default_reasoning_level"],
    additionalSpeedTiers: m["additional_speed_tiers"],
    hidden: m["visibility"] !== "list",
    isDefault: m === first,
  }));
  return { data };
}

/** The efforts the listed models take between them, in the order the binary first named each. */
function effortsOf(models: readonly HarnessCatalogModelProbe[]): string[] {
  const efforts: string[] = [];
  for (const model of models) for (const effort of model.efforts ?? []) if (!efforts.includes(effort)) efforts.push(effort);
  return efforts;
}

/**
 * Null when the app-server said nothing at all: the caller falls back to its table. A refusal when it answered and
 * offered no model because its provider wants a sign-in the machine has not got, so the table stands with words for
 * why. `login` is the catalog's command for signing codex in on a machine, the one a failed turn already names.
 */
export function parseCatalogProbe(stdout: string, login: string, accountList?: unknown): HarnessCatalogAnswer {
  const parts = stdout.split(SEP);
  if (parts.length < 3) return null;
  const [versionPart, help, serverPart] = parts as [string, string, string];
  const answers = answersOf(serverPart);
  if (resultOf(answers, INIT) === undefined) return null;
  const config = rec(resultOf(answers, CONFIG)?.["config"]);
  const models = modelsOf(accountRows(accountList) ?? resultOf(answers, MODELS), str(config?.["model"]));
  if (models.length === 0) {
    const account = resultOf(answers, ACCOUNT);
    const wantsSignIn = account?.["requiresOpenaiAuth"] === true && account["account"] === null;
    return wantsSignIn ? { refused: codexNotSignedInLine(login) } : null;
  }
  const probe: HarnessCatalogProbe = {
    version: parseVersion(versionPart),
    models,
    efforts: effortsOf(models),
    permissionModes: possibleValues(help, "--sandbox <SANDBOX_MODE>"),
  };
  return probe;
}

/** The probe's answer where the host holds the machine's ChatGPT sign-in: the models are the account's own, which the
 * host reads for the version the binary printed, and the binary's where the host could not read them. */
export async function readCatalogProbe(stdout: string, login: string, models: AccountModels | undefined): Promise<HarnessCatalogAnswer> {
  const version = parseVersion(stdout.split(SEP)[0] ?? "");
  const accountList = models !== undefined && version !== null ? await models(version).catch(() => undefined) : undefined;
  return parseCatalogProbe(stdout, login, accountList);
}

/** The account's models where no binary on the machine has described itself yet, read for the version named, else
 * wsp's npm pin: what a start is checked against before its machine answers. Undefined where the host could not
 * read them. */
export async function accountModelsOf(models: AccountModels, version: string | undefined = PINNED_VERSION): Promise<HarnessCatalogModelProbe[] | undefined> {
  if (version === undefined) return undefined;
  const rows = accountRows(await models(version).catch(() => undefined));
  return rows === undefined ? undefined : modelsOf(rows, undefined);
}
