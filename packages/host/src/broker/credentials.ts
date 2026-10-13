// SPDX-License-Identifier: AGPL-3.0-only
// The real credentials the broker hands its services, each read where it
// lives and held in memory only for the call: the agents' keys from the vault
// beside the state file, the ChatGPT login from the host's own Codex sign-in,
// and GitHub from the person's live gh login. The gh token is never stored by
// wsp, since a copy goes stale the moment gh signs in again.
import { execFile } from "node:child_process";
import { SIGN_IN_ROWS } from "@wsp/catalog";
import type { BrokerStatus } from "@wsp/protocol";
import type { AnthropicCredential, ChatgptCredential, CredentialKind, CredentialOf, OpenaiCredential } from "./types.js";

/** How long a gh token read stays good before gh is asked again, so a token the person replaced is refused upstream
 * for this long at most. */
export const GITHUB_CACHE_MS = 5 * 60_000;
const GH_TIMEOUT_MS = 15_000;

export type RunCommand = (file: string, args: readonly string[]) => Promise<{ code: number; stdout: string }>;

export interface CredentialPorts {
  /** The vault as it stands now, names to values; read at every call so a key saved while the host runs is used. */
  vault(): Readonly<Record<string, string>>;
  /** The person's own Anthropic sign-in beyond the vault (a borrowed claude.ai login, a helper, a gateway), tried
   * first; the vault's key or setup-token answers when this answers nothing or is not wired. */
  anthropic?(): Promise<AnthropicCredential | undefined>;
  /** The host's own ChatGPT login; nothing until it is signed in. */
  chatgpt?(): Promise<ChatgptCredential | undefined>;
  run?: RunCommand;
  now?(): number;
}

export interface BrokerCredentials {
  credential<K extends CredentialKind>(kind: K): Promise<CredentialOf[K] | undefined>;
  status(): Promise<BrokerStatus>;
}

const runFile: RunCommand = (file, args) =>
  new Promise(resolve => {
    execFile(file, [...args], { timeout: GH_TIMEOUT_MS, maxBuffer: 1 << 20, windowsHide: true }, (error, stdout) => {
      const code = error === null ? 0 : typeof error.code === "number" ? error.code : 1;
      resolve({ code, stdout: String(stdout) });
    });
  });

const nonEmpty = (v: string | undefined): string | undefined => (v === undefined || v.trim() === "" ? undefined : v.trim());

/** The kind Settings, Agents names for an Anthropic credential: a token that ends is a borrowed claude.ai login, and a
 * helper's output is a key. */
const claudeStatus = (c: AnthropicCredential): NonNullable<BrokerStatus["claude"]> =>
  c.kind === "token" ? (c.expiresAt !== undefined ? "login" : "token") : c.kind === "helper" ? "key" : c.kind;

export function brokerCredentials(p: CredentialPorts): BrokerCredentials {
  const run = p.run ?? runFile;
  const now = p.now ?? Date.now;
  let github: { token: string; at: number } | undefined;
  let reading: Promise<string | undefined> | undefined;
  let account: { login?: string; at: number } | undefined;

  /** Claude Code puts a key ahead of an OAuth token when both are set, so the broker does too. */
  const vaultAnthropic = (): AnthropicCredential | undefined => {
    const vault = p.vault();
    const key = nonEmpty(vault[SIGN_IN_ROWS.claude.keyEnv]);
    if (key !== undefined) return { kind: "key", key };
    const token = nonEmpty(vault[SIGN_IN_ROWS.claude.tokenEnv]);
    return token === undefined ? undefined : { kind: "token", token };
  };
  const anthropic = async (): Promise<AnthropicCredential | undefined> => (await p.anthropic?.()) ?? vaultAnthropic();

  const openai = (): OpenaiCredential | undefined => {
    const key = nonEmpty(p.vault()[SIGN_IN_ROWS.codex.keyEnv]);
    return key === undefined ? undefined : { key };
  };

  const githubAccount = async (): Promise<string | undefined> => {
    const out = await run("gh", ["auth", "status", "--active", "--hostname", "github.com", "--json", "hosts"]);
    if (out.code !== 0) return undefined;
    try {
      const hosts = (JSON.parse(out.stdout) as { hosts?: Record<string, Array<{ login?: unknown; state?: unknown }>> }).hosts;
      const row = hosts?.["github.com"]?.[0];
      return row?.state === "success" && typeof row.login === "string" ? row.login : undefined;
    } catch {
      return undefined;
    }
  };

  /** One gh read at a time: calls that arrive while one runs share its answer. A missing login is not kept, so a gh
   * signed in a minute later is read on the next call. */
  const readGithub = (): Promise<string | undefined> => {
    if (github !== undefined && now() - github.at < GITHUB_CACHE_MS) return Promise.resolve(github.token);
    reading ??= (async () => {
      const out = await run("gh", ["auth", "token", "--hostname", "github.com"]);
      const token = out.code === 0 ? nonEmpty(out.stdout) : undefined;
      if (token !== undefined) github = { token, at: now() };
      return token;
    })().finally(() => {
      reading = undefined;
    });
    return reading;
  };

  const read = async (kind: CredentialKind): Promise<CredentialOf[CredentialKind] | undefined> => {
    switch (kind) {
      case "anthropic":
        return anthropic();
      case "openai":
        return openai();
      case "chatgpt":
        return p.chatgpt?.();
      case "github": {
        const token = await readGithub();
        return token === undefined ? undefined : { token };
      }
    }
  };

  return {
    credential: <K extends CredentialKind>(kind: K) => read(kind) as Promise<CredentialOf[K] | undefined>,
    async status() {
      const claude = await anthropic();
      const codex = (await p.chatgpt?.()) !== undefined ? "chatgpt" : openai() !== undefined ? "key" : undefined;
      const token = await readGithub();
      // The account costs a GitHub API call and every brokered launch reads this status, so it is kept as long as the token.
      if (token !== undefined && (account === undefined || now() - account.at >= GITHUB_CACHE_MS)) account = { login: await githubAccount(), at: now() };
      const login = token === undefined ? undefined : account?.login;
      return {
        ...(claude !== undefined ? { claude: claudeStatus(claude) } : {}),
        ...(codex !== undefined ? { codex } : {}),
        ...(login !== undefined ? { github: login } : {}),
      };
    },
  };
}
