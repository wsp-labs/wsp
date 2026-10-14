// SPDX-License-Identifier: AGPL-3.0-only
// The host's broker, joined: the services registered with the broker core, the
// readers of the person's own sign-ins handed to its credentials, the host's
// own token store as the one that says who is calling, and the runtime given
// the switch, the pipe and the status. The switch reads ready only while the
// host serves, so a host that is stopping, or a line that never served,
// leaves every turn on today's road.
import { execFile } from "node:child_process";
import { randomBytes } from "node:crypto";
import { dirname, join } from "node:path";
import { SIGN_IN_ROWS, gitHostOf } from "@wsp/catalog";
import { ownerRepoOf } from "@wsp/protocol";
import type { Runtime, RuntimeOptions, RuntimeServer } from "@wsp/runtime";
import { claudeGateway } from "./anthropic/gateway.js";
import { claudeHelper } from "./anthropic/helper.js";
import { claudeLogin } from "./anthropic/login.js";
import { createBroker, type Broker } from "./broker/broker.js";
import { brokerCredentials, type CredentialPorts } from "./broker/credentials.js";
import { brokerLogFile } from "./broker/log.js";
import { offSwitch } from "./broker/off.js";
import type { AnthropicCredential, BrokerContext, BrokerService, BrokerWorkspace, Grant, LiveGrant } from "./broker/types.js";
import { codexLogin, type CodexLogin } from "./codexlogin/login.js";
import type { DoctorBroker } from "./doctor-fork.js";
import { loginFolders } from "./login-path.js";

/** The broker's log, one file per day beside the state file this host serves. */
export const brokerLogDir = (statePath: string): string => join(dirname(statePath), "broker");
/** The machines the broker has switched off, beside the state file, so a host start finds them still off. */
const brokerOffFile = (statePath: string): string => join(dirname(statePath), "revoked.json");

/** How many repos' GitHub ids a host keeps: one per fork a thread started from a pull request of. */
const REPO_IDS_MAX = 256;
/** How long a repo gh could not read is left unasked, since the broker reads the workspace on every call. */
const REPO_MISS_MS = 60_000;
const GH_READ_MS = 15_000;

/** The person's own Anthropic sign-ins beyond the vault, each answering nothing where it is not set up. */
export interface AnthropicReaders {
  gateway(): Promise<AnthropicCredential | undefined>;
  helper(): Promise<AnthropicCredential | undefined>;
  login(): Promise<AnthropicCredential | undefined>;
}

/** The person's own sign-ins the broker borrows. */
export interface SignIns {
  anthropic: AnthropicReaders;
  codex: CodexLogin;
}

/** The environment the sign-in readers read: the host's own, with the login shell's CLAUDE_CONFIG_DIR and CODEX_HOME
 * where the launch had none. A copy for the readers alone: on the host's own environment the two would move where its
 * local turns keep their agents' stores. */
export function signInEnv(env: NodeJS.ProcessEnv = process.env): NodeJS.ProcessEnv {
  const { claudeConfigDir, codexHome } = loginFolders();
  return { ...env, ...(claudeConfigDir !== undefined ? { CLAUDE_CONFIG_DIR: claudeConfigDir } : {}), ...(codexHome !== undefined ? { CODEX_HOME: codexHome } : {}) };
}

/** The readers of the person's own Claude Code and Codex logins, on the environment handed; `platform` is for a test,
 * which must not reach the Keychain. */
export const personSignIns = (env: NodeJS.ProcessEnv, o: { platform?: NodeJS.Platform } = {}): SignIns => ({
  anthropic: { gateway: claudeGateway({ env }), helper: claudeHelper({ env }), login: claudeLogin({ env, ...o }) },
  codex: codexLogin({ env }),
});

export interface HostBrokerOptions {
  statePath: string;
  /** The vault as it stands at each call. */
  vault(): Readonly<Record<string, string>>;
  /** Stand-ins for a test; the vendors' own services and the person's own sign-ins otherwise. */
  services?: readonly BrokerService[];
  signIns?: SignIns;
  run?: CredentialPorts["run"];
}

export interface HostBroker {
  /** What the runtime is made with. */
  readonly port: NonNullable<RuntimeOptions["broker"]>;
  readonly doctor: DoctorBroker;
  /** Loads the services; from then the switch reads ready, and every token is read off the serving host's own store. */
  start(serving: { rt: Runtime; server: Pick<RuntimeServer, "authorize" | "revokeDevice"> }): Promise<void>;
  /** The switch reads not ready, and every pipe the broker holds is cut. */
  stop(): void;
}

/** The order Claude Code itself picks in (its authentication precedence): a gateway's token, an API key, an
 * apiKeyHelper, a setup-token, then the claude.ai login. An answer of nothing hands the call to the broker's own
 * vault read, which is where the key and the setup-token live. */
function anthropicPort(readers: AnthropicReaders, vault: () => Readonly<Record<string, string>>): NonNullable<CredentialPorts["anthropic"]> {
  const held = (name: string): boolean => (vault()[name] ?? "").trim() !== "";
  return async () => {
    const gateway = await readers.gateway();
    if (gateway !== undefined || held(SIGN_IN_ROWS.claude.keyEnv)) return gateway;
    const helper = await readers.helper();
    if (helper !== undefined || held(SIGN_IN_ROWS.claude.tokenEnv)) return helper;
    return readers.login();
  };
}

const ghRepoId = (run: NonNullable<CredentialPorts["run"]>, ownerName: string): Promise<number | undefined> =>
  run("gh", ["api", `repos/${ownerName}`, "--jq", ".id"]).then(
    out => {
      const id = Number(out.stdout.trim());
      return out.code === 0 && Number.isSafeInteger(id) && id > 0 ? id : undefined;
    },
    () => undefined,
  );

const runGh: NonNullable<CredentialPorts["run"]> = (file, args) =>
  new Promise(resolve => {
    execFile(file, [...args], { timeout: GH_READ_MS, maxBuffer: 1 << 20, windowsHide: true }, (error, stdout) => {
      resolve({ code: error === null ? 0 : typeof error.code === "number" ? error.code : 1, stdout: String(stdout) });
    });
  });

/** What a token the host minted for a machine drives, read off its device's name: `op <op>` for one daemon operation,
 * `terminal <thread>` for a Terminal pane, and any other a turn's. */
const kindOf = (name: string): { kind: "turn" | "terminal" } | { kind: "op"; op: string } => {
  const op = /^op (.+)$/.exec(name)?.[1];
  return op !== undefined ? { kind: "op", op } : name.startsWith("terminal ") ? { kind: "terminal" } : { kind: "turn" };
};

/** A device token's grant. A token of no thread, the host's own or a paired computer's, drives nothing here. */
function grantOf(who: Awaited<ReturnType<RuntimeServer["authorize"]>>): Grant | undefined {
  const device = who?.kind === "device" ? who.device : undefined;
  const scope = device?.scope;
  if (device === undefined || scope === undefined) return undefined;
  return { ...kindOf(device.name), workspaceId: scope.workspaceId, threadId: scope.threadId };
}

/** A service whose refusals are also written into the calling thread: an agent may not print a refusal's body, so the
 * sentence is a row there too, once per turn. */
function withRows(service: BrokerService, row: (grant: Grant, said: string) => void): BrokerService {
  const told = (grant: Grant, ctx: BrokerContext): BrokerContext => ({
    ...ctx,
    refuse: (res, status, said, shape) => {
      row(grant, said);
      ctx.refuse(res, status, said, shape);
    },
  });
  const { upgrade } = service;
  return {
    id: service.id,
    claims: (req, door, agent) => service.claims(req, door, agent),
    serve: (r, ctx) => service.serve(r, told(r.grant, ctx)),
    ...(upgrade !== undefined ? { upgrade: (r, socket, head, ctx) => upgrade.call(service, r, socket, head, told(r.grant, ctx)) } : {}),
  };
}

const agentOf = (harness: string | undefined): "claude" | "codex" | undefined => (harness === "claude" || harness === "codex" ? harness : undefined);

export function hostBroker(o: HostBrokerOptions): HostBroker {
  const run = o.run ?? runGh;
  const { anthropic, codex } = o.signIns ?? personSignIns(signInEnv());
  const credentials: CredentialPorts = { vault: o.vault, anthropic: anthropicPort(anthropic, o.vault), chatgpt: () => codex.credential(), run };
  const github = brokerCredentials(credentials);
  const off = offSwitch(brokerOffFile(o.statePath));
  /** The doctor's model call is one op the host grants itself, held only in memory for that call. */
  const doctorGrants = new Map<string, Grant>();
  const repoIds = new Map<string, number>();
  const repoMisses = new Map<string, number>();
  let serving: { broker: Broker } | undefined;
  /** Moved by every start and stop, so a stop that lands while the services load leaves the switch off. */
  let turn = 0;

  const repoIdOf = async (ownerName: string): Promise<number | undefined> => {
    const key = ownerName.toLowerCase();
    const held = repoIds.get(key);
    if (held !== undefined) return held;
    const missed = repoMisses.get(key);
    if (missed !== undefined && Date.now() - missed < REPO_MISS_MS) return undefined;
    const id = await ghRepoId(run, ownerName);
    if (id === undefined) {
      repoMisses.delete(key);
      repoMisses.set(key, Date.now());
      if (repoMisses.size > REPO_IDS_MAX) repoMisses.delete(repoMisses.keys().next().value!);
      return undefined;
    }
    repoMisses.delete(key);
    if (repoIds.size >= REPO_IDS_MAX) repoIds.delete(repoIds.keys().next().value!);
    repoIds.set(key, id);
    return id;
  };

  /** The workspace as the services read it: its agent, its branch of record, the project's own repo by GitHub's id,
   * and the trackers its project lists. A thread from a fork's pull request pushes to the fork, while its pull
   * request and comments are on the project's repo, whose id is read once with the person's gh. */
  const workspaceOf = async (rt: Runtime, id: string): Promise<BrokerWorkspace | undefined> => {
    const view = await rt.workspaces.get(id).catch(() => undefined);
    if (view === undefined || view.phase === "gone") return undefined;
    const [project, sessions, prefs] = await Promise.all([rt.projects.list().then(all => all.find(p => p.id === view.project.id)), rt.sessions.list(id), rt.preferences.get()]);
    const bor = view.branchOfRecord;
    // Only a remote GitHub holds names a repo GitHub has an id for; any other host's owner/name is not the project's.
    const onGithub = project !== undefined && gitHostOf(project.remote)?.id === "github" ? ownerRepoOf(project.remote) : undefined;
    const ownerName = onGithub ?? (bor !== undefined ? `${bor.owner}/${bor.name}` : undefined);
    const [owner = "", name = ""] = ownerName?.split("/") ?? [];
    const same = bor !== undefined && `${bor.owner}/${bor.name}`.toLowerCase() === `${owner}/${name}`.toLowerCase();
    const repoId = same ? bor.repoId : ownerName !== undefined ? await repoIdOf(ownerName) : undefined;
    const latest = [...sessions].sort((a, b) => (b.startedAt ?? 0) - (a.startedAt ?? 0)).find(s => agentOf(s.harness) !== undefined);
    const agent = agentOf(latest?.harness);
    return {
      ...(agent !== undefined ? { agent } : {}),
      ...(bor !== undefined ? { branchOfRecord: bor } : {}),
      repo: { owner, name, id: repoId ?? 0 },
      trackers: [...(prefs.projectDefaults[view.project.id]?.trackers ?? [])],
    };
  };

  /** The turn and terminal tokens live on a workspace now, with their thread's agent: one turn per thread, since a
   * side question's token shares its turn's thread. */
  const liveOn = async (rt: Runtime, workspaceId: string): Promise<LiveGrant[]> => {
    const [devices, sessions] = await Promise.all([rt.devices.list(), rt.sessions.list(workspaceId)]);
    const harness = new Map(sessions.flatMap(s => (s.threadId === undefined ? [] : [[s.threadId, s.harness] as const])));
    const turns = new Set<string>();
    const live: LiveGrant[] = [];
    for (const d of devices) {
      const kind = kindOf(d.name).kind;
      if (d.scope?.workspaceId !== workspaceId || kind === "op" || (kind === "turn" && turns.has(d.scope.threadId))) continue;
      if (kind === "turn") turns.add(d.scope.threadId);
      const agent = agentOf(harness.get(d.scope.threadId));
      live.push({ kind, workspaceId, threadId: d.scope.threadId, ...(agent !== undefined ? { agent } : {}) });
    }
    return live;
  };

  const refusedOff = (): Error => new Error("the host's broker is not serving");

  return {
    port: {
      ready: () => serving !== undefined,
      codexClaims: () => codex.claims(),
      pipe: (workspaceId, open, duplex) => {
        if (serving === undefined) return void duplex.destroy();
        serving.broker.take(workspaceId, open, duplex);
      },
      status: () => (serving === undefined ? Promise.reject(refusedOff()) : serving.broker.status()),
      revoke: async workspaceId => (serving === undefined ? off.add(workspaceId) : serving.broker.revoke(workspaceId)),
      lift: workspaceId => off.delete(workspaceId),
    },
    doctor: {
      ready: () => serving !== undefined,
      status: () => (serving === undefined ? Promise.reject(refusedOff()) : serving.broker.status()),
      async grant(workspaceId) {
        const token = randomBytes(32).toString("base64url");
        doctorGrants.set(token, { kind: "op", workspaceId, op: "doctor" });
        return { token, revoke: () => void doctorGrants.delete(token) };
      },
    },
    async start(s) {
      if (serving !== undefined) return;
      const at = ++turn;
      const { services, storageHost } = o.services !== undefined ? { services: o.services, storageHost: undefined } : (await import("./server-broker-services.js")).brokerServices(() => github.credential("github"));
      if (at !== turn || serving !== undefined) return;
      const { rt, server } = s;
      const row = (grant: Grant, said: string): void => {
        if (grant.threadId !== undefined) void rt.sessions.noteWaited(grant.threadId, 0, said).catch(() => undefined);
      };
      const broker = createBroker({
        services: services.map(service => withRows(service, row)),
        authorize: async token => doctorGrants.get(token) ?? grantOf(await server.authorize(token)),
        workspace: id => workspaceOf(rt, id),
        live: id => liveOn(rt, id),
        ...(storageHost !== undefined ? { storageHost } : {}),
        revokeTokens: async workspaceId => {
          for (const d of await rt.devices.list()) if (d.scope?.workspaceId === workspaceId) await server.revokeDevice(d.id);
          for (const [token, grant] of doctorGrants) if (grant.workspaceId === workspaceId) doctorGrants.delete(token);
        },
        credentials,
        off,
        log: brokerLogFile(brokerLogDir(o.statePath)),
        waited: (grant, heldMs) => {
          if (grant.threadId !== undefined) void rt.sessions.noteWaited(grant.threadId, heldMs).catch(() => undefined);
        },
      });
      serving = { broker };
    },
    stop() {
      turn++;
      const was = serving;
      serving = undefined;
      if (was === undefined) return;
      was.broker.close();
    },
  };
}
