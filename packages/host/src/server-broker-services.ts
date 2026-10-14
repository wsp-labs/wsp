// SPDX-License-Identifier: AGPL-3.0-only
// The services the host's broker routes to, in the order it asks them: model
// calls, then git as a proxy, then gh. A module of its own, loaded when a host
// starts its broker, since the gh road's GraphQL parser alone is megabytes of
// heap a line that never serves has no use for.
import { createReadStream } from "node:fs";
import { anthropicService } from "./anthropic/service.js";
import type { BrokerService } from "./broker/types.js";
import { chatgptService, openaiService } from "./chatgpt/service.js";
import { ghRoad, isStorageHost } from "./ghroad/api.js";
import { gitRoad, type PushGuard } from "./gitroad/proxy.js";
import { githubEntryReader, guardWorkflows } from "./packguard/workflows.js";

/** The `.github/` check behind the git road's guard: every command that moves a ref is read against the default
 * branch's and the start commit's `.github`, through GitHub's tree API with the person's own gh login. */
function workflowGuard(github: () => Promise<{ token: string } | undefined>): PushGuard {
  return async push => {
    const credential = await github();
    if (credential === undefined) throw new Error("no gh sign-in on this computer to read the repo's .github with");
    const entryAt = githubEntryReader({ repoId: push.repo.id, token: credential.token });
    for (const command of push.commands) {
      const verdict = await guardWorkflows({ pack: createReadStream(push.pack), head: command.new, ...(push.base !== undefined ? { base: push.base } : {}), entryAt });
      if (!verdict.allow) return verdict.said;
    }
    return undefined;
  };
}

export function brokerServices(github: () => Promise<{ token: string } | undefined>): { services: BrokerService[]; storageHost(host: string): boolean } {
  return {
    services: [anthropicService(), chatgptService(), openaiService(), gitRoad({ guard: workflowGuard(github) }), ghRoad()],
    storageHost: isStorageHost,
  };
}
