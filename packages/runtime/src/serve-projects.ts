// SPDX-License-Identifier: AGPL-3.0-only
// The project ops a socket asks, answered off the runtime's projects: what the reply carries beside id and ok.
import { placeSaid, runsInFolder, scopeOf, type Caller, type PlaceView, type RuntimeRequest } from "@wsp/protocol";
import type { Runtime } from "./types/api.js";

export type ProjectRequest = Extract<RuntimeRequest, { op: `projects.${string}` }>;

export const isProjectRequest = (msg: RuntimeRequest): msg is ProjectRequest => msg.op.startsWith("projects.");

export async function answerProject(projects: Runtime["projects"], msg: ProjectRequest, origin: Caller | undefined): Promise<Record<string, unknown>> {
  switch (msg.op) {
    case "projects.add": {
      const { notice, ...project } = await projects.add({ source: msg.source, ...(msg.on !== undefined ? { on: msg.on } : {}), ...(msg.name !== undefined ? { name: msg.name } : {}), ...(msg.base !== undefined ? { base: msg.base } : {}), ...(msg.into !== undefined ? { into: msg.into } : {}), ...(msg.seed !== undefined ? { seed: msg.seed } : {}) }, origin);
      return { project, ...(notice !== undefined ? { notice } : {}) };
    }
    case "projects.seed":
      return { ...(await projects.seed(msg.project, { ...(msg.keep !== undefined ? { keep: msg.keep } : {}), ...(msg.cut !== undefined ? { cut: msg.cut } : {}), ...(msg.none === true ? { none: true } : {}), ...(msg.save === true ? { save: true } : {}) }, origin)) };
    case "projects.list":
      return { projects: await projects.list(origin) };
    case "projects.defaults":
      return { defaults: await projects.defaults(origin) };
    case "projects.resolve":
      return { project: await projects.resolve(msg.ref, origin) };
    case "projects.branch":
      return { ...(await projects.branch(msg.projectId, origin)) };
    case "projects.remove":
      return { ...(await projects.remove(msg.projectId, origin, { ...(msg.force === true ? { force: true } : {}), ...(msg.check === true ? { check: true } : {}) })) };
    case "projects.icon":
      return { ...(await projects.icon(msg.projectId, msg.png)) };
    case "projects.icons":
      return { icons: await projects.icons(msg.hashes) };
  }
}

/** The computers a thread's own token is answered: every cloud by the name every table says it, and for a thread in a
 * folder each computer holding a project it may start work on, which are the places a start of its own can go. A
 * thread on a machine wsp forked starts work on no computer of the person's, so it reads the clouds alone, its kind read
 * off the workspace its token names; no row carries its road, since a thread starts work by a name, never an address. */
export async function threadPlaces(rt: Pick<Runtime, "projects" | "places" | "workspaces">, origin: Caller | undefined, now: number): Promise<PlaceView[]> {
  const scope = scopeOf(origin);
  const asking = scope === undefined ? undefined : await rt.workspaces.get(scope.workspaceId).catch(() => undefined);
  const reached = new Set(asking?.kind !== undefined && runsInFolder(asking.kind) ? (await rt.projects.list(origin)).map(p => p.computer) : []);
  return ((await rt.places?.list(now)) ?? []).filter(p => p.kind === "provider" || reached.has(p.id)).map(({ road: _road, ...p }) => (p.kind === "provider" ? { ...p, label: placeSaid(p) } : p));
}
