// SPDX-License-Identifier: AGPL-3.0-only
// The doctor's fork road: which cloud account it forks on, and its model call
// from the fork through the door to this host, which adds the person's
// credential and answers which kind it used, or the broker's own sentence for
// why it added none.
import { BROKER_PORT, LOOPBACK, shellQuote, type BrokerStatus, type PlaceView } from "@wsp/protocol";
import { harnessCatalog, smallestModel } from "@wsp/runtime";

/** The cloud account a fork-road run forks on: the one named, else the only one this host holds. With more than one
 * and none named the run is refused with their names, since a fork on the wrong one proves the wrong cloud. */
export function forkPlaceOf(named: PlaceView | undefined, rows: readonly PlaceView[]): string | undefined {
  if (named !== undefined) return named.id;
  const clouds = rows.filter(r => r.kind === "provider" && r.takesForks === true);
  if (clouds.length > 1) throw new Error(`this host forks on ${clouds.length} cloud accounts, so wsp doctor proves the one you name: ${clouds.map(c => `wsp doctor ${c.name}`).join(" or ")}`);
  return clouds[0]?.id;
}

/** The broker as the doctor asks it: whether it serves, the kind of credential it adds to a Claude call, and a token
 * the broker takes from one workspace for one call, revoked when the call ends. */
export interface DoctorBroker {
  ready(): boolean;
  status(): Promise<BrokerStatus>;
  grant(workspaceId: string): Promise<{ token: string; revoke(): void }>;
}

/** Each kind of Claude credential as the step names it, and the header a machine sends its placeholder in, the one
 * Claude Code sends for that kind. */
const CLAUDE_KINDS: Readonly<Record<NonNullable<BrokerStatus["claude"]>, { said: string; header: (token: string) => string[] }>> = {
  login: { said: "a subscription", header: token => [`Authorization: Bearer ${token}`, "anthropic-beta: oauth-2025-04-20"] },
  token: { said: "a token", header: token => [`Authorization: Bearer ${token}`, "anthropic-beta: oauth-2025-04-20"] },
  key: { said: "an API key", header: token => [`x-api-key: ${token}`] },
  gateway: { said: "your gateway", header: token => [`Authorization: Bearer ${token}`] },
};

export const MODEL_CALL_STEP = "a model call through your computer";
export const modelCallSkipped = "skipped: the host's broker is not serving yet, so the machine calls with its own key";
export const noClaudeSignInLine = (here: string): string => `${here} holds no Claude Code sign-in for machines at a provider to use: sign Claude Code in there, or run wsp agents key claude`;

/** One model call from the fork through the door to the host, which adds the credential: the smallest request the
 * smallest model answers, so it proves the road and bills next to nothing. Answers the kind the host used; a refusal
 * is the broker's own sentence, which names the cause. */
export async function modelCall(exec: (cmd: string) => Promise<{ exitCode: number; stdout: string; stderr: string }>, workspaceId: string, broker: DoctorBroker | undefined, here: string): Promise<string> {
  if (broker === undefined || !broker.ready()) return modelCallSkipped;
  const kind = (await broker.status()).claude;
  if (kind === undefined) throw new Error(noClaudeSignInLine(here));
  const body = JSON.stringify({ model: smallestModel(harnessCatalog("claude")), max_tokens: 1, messages: [{ role: "user", content: "hi" }] });
  const { token, revoke } = await broker.grant(workspaceId);
  try {
    const headers = [...CLAUDE_KINDS[kind].header(token), "anthropic-version: 2023-06-01", "content-type: application/json"].map(h => `-H ${shellQuote(h)}`).join(" ");
    const r = await exec(`curl -sS -m 60 -w '\n%{http_code}' ${headers} --data-binary ${shellQuote(body)} http://${LOOPBACK}:${BROKER_PORT}/v1/messages`);
    const lines = r.stdout.trimEnd().split("\n");
    const code = Number(lines.at(-1));
    if (r.exitCode !== 0 || !Number.isFinite(code)) throw new Error(`the machine could not reach the door to your computer: ${r.stderr.trim() || `curl exited ${r.exitCode}`}`);
    if (code === 200) return `Claude Code, ${CLAUDE_KINDS[kind].said}`;
    throw new Error(brokerSaid(lines.slice(0, -1).join("\n")) ?? `the host answered the model call ${code}`);
  } finally {
    revoke();
  }
}

/** The sentence in an Anthropic-shaped error body, which is where the broker says why it added no credential. */
function brokerSaid(body: string): string | undefined {
  try {
    const said = (JSON.parse(body) as { error?: { message?: unknown } }).error?.message;
    return typeof said === "string" ? said : undefined;
  } catch {
    return undefined;
  }
}
