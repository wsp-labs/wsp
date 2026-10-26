// SPDX-License-Identifier: AGPL-3.0-only
import { z } from "zod";
import { THREAD_AGENTS } from "@wsp/catalog";
import {
  ThreadDefaults,
  COORDINATOR_HANDOFF,
  NOTIFY_CALLER,
  usageRefusal,
  ProjectView,
  addedProjectLine,
  MEMORY_KEPT_CLAUSE,
  ProjectSeed,
  projectSeedLines,
} from "@wsp/protocol";
import { HOST_RESTARTING_LINE, hostPlatform, table, usageIs, tool, type HostClient, type Verb, flag, flagList } from "./client.js";
import { workspaces, threads, threadsOf, threadRows, THREAD_HEAD, threadLines, diskGoneLines, threadTree, placeNames, projectsOf, projectOf, projectLine, NO_PROJECT_YET } from "./workspaces-help.js";
import { projectDefaultsOf, waitThrough } from "./turns-help.js";
import { confirmed, WaitOut, ThreadRowOut, asJson, asText, waitAnswer, timeoutFlag, ACCESS_IN_WORDS } from "./io.js";
import { drawRows, PROJECT_SET_RESETS, projectDefaultsSet, newThreadsHeadLine, threadDefaultsLines, defaultsCell, afterWorktreeLine, trackersLine } from "./agents-help.js";

/** What projects set answers in words: what a new thread there starts on, then the after-worktree command. */
const projectSetLines = (set: { project: ProjectView; defaults: ThreadDefaults; afterWorktree?: string; trackers?: string[] }): string[] => [
  newThreadsHeadLine(set.project.name),
  ...threadDefaultsLines(set.defaults),
  ...(set.afterWorktree !== undefined ? [afterWorktreeLine(set.afterWorktree)] : []),
  ...(set.trackers !== undefined ? [trackersLine(set.trackers)] : []),
];

/** A project's seed picks, off the host, with only the words a caller gave on the wire. */
const projectSeed = (client: HostClient, project: string, o: { keep: string[]; cut: string[]; none: boolean; save: boolean }): Promise<ProjectSeed> =>
  client
    .request<Record<string, unknown>>("projects.seed", {
      project,
      ...(o.keep.length > 0 ? { keep: o.keep } : {}),
      ...(o.cut.length > 0 ? { cut: o.cut } : {}),
      ...(o.none ? { none: true } : {}),
      ...(o.save ? { save: true } : {}),
    })
    .then(a => ProjectSeed.parse(a));

/** The picks as a terminal reads them: the head, the menu's table, and whether anything was saved. */
const projectSeedText = (seed: ProjectSeed): string => {
  const { head, rows, foot } = projectSeedLines(seed);
  return [head, "", ...(rows.length === 0 ? [] : [...table(rows).map(line => `  ${line}`), ""]), ...foot].join("\n");
};

export const PROJECT_VERBS: readonly Verb[] = [
  {
    name: "projects",
    usage: "wsp projects",
    about: "your projects, each on its computer: where its code comes from, where its folder sits, the branch a machine of it starts on and how many threads it has",
    page: "front",
    options: {},
    run: async ctx => {
      if (ctx.args.length !== 0) throw usageRefusal("wsp projects takes no positional arguments.", usageIs(ctx));
      const client = await ctx.client();
      const projects = await projectsOf(client);
      const held = projects.length === 0 ? [] : await workspaces(client);
      const running = projects.length === 0 ? [] : await threads(client);
      // The computer's own name, off the one places reading every other table takes; a thread's token is refused
      // that list, and its rows then read the id, which is what it can name a computer by anyway.
      const named = projects.length === 0 ? new Map<string, string>() : await placeNames(client).catch(() => new Map<string, string>());
      const defaults = projects.length === 0 ? {} : await projectDefaultsOf(client);
      ctx.out.emit({ projects, defaults }, projects.length === 0 ? NO_PROJECT_YET : table([["PROJECT", "ID", "COMPUTER", "SOURCE", "PATH", "BASE", "THREADS", "NEW THREADS"], ...projects.map(p => [...projectLine(p, held, running, named), defaultsCell(defaults[p.id])])]).join("\n"));
      return 0;
    },
    tool: tool({
      description:
        "Every project this host holds: its name, the computer it lives on, where its code comes from (a folder on the computer the app runs on, or a repo a computer clones), where its folder sits, the branch a machine of it starts on, and its repo's top folder where it is a folder here inside a git repo. The name is what run takes. A project is recorded with add and is one source on one computer; on the computer the app runs on its threads run in its folder, or in a worktree of its repo for another branch. defaults holds, by project id, the agent, model, effort and access a new thread there starts on when its start names none, each with where it came from: the project's own (projects_set), the person's default (agents_set, agents_default) or the agent's own.",
      input: {},
      output: { projects: z.array(ProjectView), defaults: z.record(z.string(), ThreadDefaults) },
      call: async (_args, deps) => {
        const client = await deps.client();
        return asJson({ projects: await projectsOf(client), defaults: await projectDefaultsOf(client) });
      },
    }),
  },
  {
    name: "projects add",
    toolOnly:
      "the command line records a project with wsp add, the same word that joins a computer and takes a provider's key; those two belong at the terminal the host runs at, so the tool door carries the project half alone",
    tool: tool({
      description:
        "Records a project: one source on one computer. A folder on the computer the app runs on is the project as it stands, a git repo or not, a folder inside a repo being a project of that repo; its threads run in it. A repo is cloned into a folder of its own name inside the folder named with into on the computer the app runs on, made where it does not exist yet, or into that folder itself where its name is the repo's or the repo's with a number, and the clone's folder is then a folder project there, or by the computer named with on, whose machines then hold a checkout of it. A repo with neither into nor on, a clone's folder that holds files, and a source already recorded on that computer are each refused in one line, and a clone that fails says git's own last line. The answer is the project, whose name is what run takes.",
      input: {
        source: z.string().describe("a folder on the computer the app runs on, or a repo's url"),
        on: z.string().optional().describe("the computer that clones the repo, by the name computers lists; a folder, and a repo cloned with into, take none"),
        into: z.string().optional().describe("an absolute path on the computer the app runs on to clone the repo in: into a folder of the repo's name inside it, made where it does not exist yet, or into the path itself where its name is the repo's or the repo's with a number; the project is then the clone's folder"),
        name: z.string().optional().describe("what to call the project here; the folder's or the repo's own last word without it"),
        base: z.string().optional().describe("the branch a machine of the project starts on; the remote's own default branch at the clone without it"),
      },
      output: { project: ProjectView, notice: z.string().optional() },
      call: async (args, deps) => {
        const client = await deps.client();
        const { project, notice } = await client.request<{ project: ProjectView; notice?: string }>("projects.add", {
          source: args.source,
          ...(args.on !== undefined ? { on: args.on } : {}),
          ...(args.name !== undefined ? { name: args.name } : {}),
          ...(args.base !== undefined ? { base: args.base } : {}),
          ...(args.into !== undefined ? { into: args.into } : {}),
        });
        const named = await placeNames(client).catch(() => new Map<string, string>());
        // What landed and is not what was asked for rides the answer: an add that stands with the commits left
        // behind reads as an add that stands, and the caller has to be told which.
        const said = addedProjectLine(project, named, hostPlatform());
        return asText(notice === undefined ? said : `${said}\n${notice}`, { project, ...(notice !== undefined ? { notice } : {}) });
      },
    }),
  },
  {
    name: "projects remove",
    usage: "wsp projects remove <project> [--yes] [--force]",
    about: "takes a project out of this wsp; its folder is left where it is, on this computer or on a computer you joined, a project with a machine standing on it is refused naming them, and one whose checkout on a computer of yours holds work no remote has is stopped naming it",
    page: "agent",
    options: { yes: { type: "boolean" }, force: { type: "boolean" } },
    run: async ctx => {
      const [ref] = ctx.args;
      if (ref === undefined || ctx.args.length !== 1) throw usageRefusal("wsp projects remove takes one project.", usageIs(ctx));
      const client = await ctx.client();
      const project = await projectOf(client, ref);
      const force = ctx.flags["force"] === true ? { force: true } : {};
      // The host's own refusals first, so the one question is asked only of a remove that would go.
      const { unsaved } = await client.request<{ unsaved?: string }>("projects.remove", { projectId: project.id, check: true, ...force });
      const losing = unsaved === undefined ? "" : `\nWith it goes work no remote has: ${unsaved}.`;
      if (!(await confirmed(ctx, `Remove ${project.name}?\nIts record leaves this wsp, with the folder wsp made for it on the computer holding it; a folder of yours stays where it is.${losing}`, project.name))) return 1;
      // The sentence comes off the wire: what a remove took is true differently on a computer of the person's, at
      // a provider and on this computer, and the runtime's own road for that computer is what says which.
      const { said } = await client.request<{ said: string }>("projects.remove", { projectId: project.id, ...force });
      ctx.out.emit({ project, said }, said);
      return 0;
    },
    tool: tool({
      description: `Takes a project's record out of this wsp. Its folder stays exactly where it is, on this computer or on a computer you joined, ${MEMORY_KEPT_CLAUSE}, and no repo is ever asked for anything. Refused in one line while a machine of it stands, naming them; delete those first. Refused in one line too while its checkout on a computer of yours holds work no remote has, naming it by its path there, unless force.`,
      input: {
        project: z.string().describe("the project's name, or its id when two share a name"),
        force: z.boolean().optional().describe("remove it even where its checkout on a computer of yours holds work no remote has"),
      },
      output: { project: ProjectView, said: z.string() },
      call: async ({ project: ref, force }, deps) => {
        const client = await deps.client();
        const project = await projectOf(client, ref);
        const { said } = await client.request<{ said: string }>("projects.remove", { projectId: project.id, ...(force === true ? { force: true } : {}) });
        return asText(said, { project, said });
      },
    }),
  },
  {
    name: "projects set",
    usage: "wsp projects set <project> [--agent <id>] [--model <slug>] [--effort <word>] [--access <word>] [--after-worktree <command>] [--tracker <owner/name>]... [--reset <field>]...",
    about: "what a new thread on one project starts on, over the agent's own defaults: its agent, model, effort and access; the shell line a new worktree of it runs once, after each ecosystem's own install, for what none of them knows; and the repos besides its own that gh on a sub-thread's machine may comment on, the whole list each time",
    page: "agent",
    options: { agent: { type: "string" }, model: { type: "string" }, effort: { type: "string" }, access: { type: "string" }, "after-worktree": { type: "string" }, tracker: { type: "string", multiple: true }, reset: { type: "string", multiple: true } },
    run: async ctx => {
      const [ref, ...rest] = ctx.args;
      if (ref === undefined || rest.length > 0) throw usageRefusal("wsp projects set takes one project.", usageIs(ctx));
      const set = await projectDefaultsSet(await ctx.client(), ref, {
        ...(flag(ctx.flags, "agent") !== undefined ? { agent: flag(ctx.flags, "agent")! } : {}),
        ...(flag(ctx.flags, "model") !== undefined ? { model: flag(ctx.flags, "model")! } : {}),
        ...(flag(ctx.flags, "effort") !== undefined ? { effort: flag(ctx.flags, "effort")! } : {}),
        ...(flag(ctx.flags, "access") !== undefined ? { access: flag(ctx.flags, "access")! } : {}),
        ...(flag(ctx.flags, "after-worktree") !== undefined ? { afterWorktree: flag(ctx.flags, "after-worktree")! } : {}),
        trackers: flagList(ctx.flags, "tracker"),
        reset: flagList(ctx.flags, "reset"),
      });
      ctx.out.emit(set, projectSetLines(set).join("\n"));
      return 0;
    },
    tool: tool({
      description:
        "Sets what a new thread on one project starts on, over each agent's own defaults and under what a start names: its agent, model, effort and access. Nothing about a computer is a project's to set (threads at once, an agent's program, config folder, launch words, variables, or whether it is on); agents_setup sets those per computer. A model or effort kept for the project's agent drops to the agent's own on a thread that runs another agent, and an access word the project's agent maps to none of its modes is refused naming the ones it takes. after_worktree is a shell line a new worktree of the project runs once at its top, after the locked install of each ecosystem whose lockfile it holds, each run in the folder holding that lockfile (uv sync --locked, cargo fetch --locked, go list -mod=readonly -deps -test ./..., npm install --no-save, pnpm install --frozen-lockfile where the node_modules carried in were installed from another lockfile, and the like), for what none of them knows. trackers is the whole list of repos besides the project's own, each owner/name, that gh on a sub-thread's machine may comment on, issues and pull requests both; every other gh write there stays refused. Answers what a new thread there now starts on, each value with where it came from, the after-worktree command and the trackers. reset puts a field back on the layer below: agent, model, effort or access, after-worktree takes the command away and trackers empties the list.",
      input: {
        project: z.string().describe("the project's name, or its id when two share a name"),
        agent: z.string().optional().describe(`the agent a new thread on it runs, one of ${THREAD_AGENTS.join(", ")}`),
        model: z.string().optional().describe("the model a new thread on it starts on, by the agent's own slug"),
        effort: z.string().optional().describe("the effort a new thread on it starts at, by the agent's own word"),
        access: z.string().optional().describe(ACCESS_IN_WORDS),
        after_worktree: z.string().optional().describe("a shell line a new worktree of the project runs once at its top, after each ecosystem's own install"),
        trackers: z.array(z.string()).optional().describe("the whole list of repos, each owner/name, besides the project's own that gh on a sub-thread's machine may comment on"),
        reset: z.array(z.enum(PROJECT_SET_RESETS)).optional().describe("fields to put back on the layer below: agent, model, effort, access, after-worktree to take the command away, or trackers to empty the list"),
      },
      output: { project: ProjectView, defaults: ThreadDefaults, afterWorktree: z.string().optional(), trackers: z.array(z.string()).optional() },
      call: async ({ project, agent, model, effort, access, after_worktree, trackers, reset }, deps) => {
        const set = await projectDefaultsSet(await deps.client(), project, { ...(agent !== undefined ? { agent } : {}), ...(model !== undefined ? { model } : {}), ...(effort !== undefined ? { effort } : {}), ...(access !== undefined ? { access } : {}), ...(after_worktree !== undefined ? { afterWorktree: after_worktree } : {}), ...(trackers !== undefined ? { trackers } : {}), ...(reset !== undefined ? { reset } : {}) });
        return asText(projectSetLines(set).join("\n"), set);
      },
    }),
  },
  {
    name: "projects seed",
    usage: "wsp projects seed <project> [--keep <path>]... [--cut <path>]... [--none] [--yes]",
    about: "what git ignores in a project's folder on this computer, and which of it goes to a sub-thread's machine: the menu with each path's size and why, the picks marked; --yes saves the picks, which every later sub-thread of the project takes",
    page: "agent",
    options: { keep: { type: "string", multiple: true }, cut: { type: "string", multiple: true }, none: { type: "boolean" }, yes: { type: "boolean" } },
    run: async ctx => {
      const [ref, ...rest] = ctx.args;
      if (ref === undefined || rest.length > 0) throw usageRefusal("wsp projects seed takes one project.", usageIs(ctx));
      const seed = await projectSeed(await ctx.client(), ref, { keep: flagList(ctx.flags, "keep"), cut: flagList(ctx.flags, "cut"), none: ctx.flags["none"] === true, save: ctx.flags["yes"] === true });
      ctx.out.emit(seed, projectSeedText(seed));
      return 0;
    },
    tool: tool({
      description:
        "What git ignores in a project's folder on this computer, and which of it goes to a sub-thread's machine: every path with its size, its kind (config, rebuilt, data, never for a login, unknown), the words its row ends in and whether it is picked. The picks start from the ones saved for the folder, else from nothing, since nothing git ignores is sent until the person saves picks; none empties them, keep adds paths off the menu and cut takes them away, each relative to the folder. A path git tracks or that is not there, the repository's own .git and a login are refused by name; a saved path no longer in the folder stays saved and is named in picked.missing. confirm saves the picks, which every later sub-thread of the project takes, as the same record wsp add --remember keeps; without it the menu is answered and nothing is saved. A thread's own token reads the menu and is refused the save, with the line to hand the person. A project whose folder is on another computer is refused.",
      input: {
        project: z.string().describe("the project, by name or id; its folder must be on this computer"),
        keep: z.array(z.string()).optional().describe("paths off the menu to send, each relative to the project's folder"),
        cut: z.array(z.string()).optional().describe("paths off the menu to keep on this computer"),
        none: z.boolean().optional().describe("send nothing git ignores"),
        confirm: z.boolean().optional().describe("true saves the picks; without it the menu is answered and nothing is saved"),
      },
      output: ProjectSeed.shape,
      call: async ({ project, keep, cut, none, confirm }, deps) => asJson(await projectSeed(await deps.client(), project, { keep: keep ?? [], cut: cut ?? [], none: none === true, save: confirm === true })),
    }),
  },
  {
    name: "threads",
    usage: "wsp threads [<project>] [--tree] [--watch]",
    about: "who is working, where and on which computer: every thread as the sidebar lists it, with its project, the folder it works in, that folder's branch, the agent, the state and who opened it, and under each thread the agent's own subagents with their SUBAGENT id; --tree indents the threads an agent spawned under the one that spawned them, and --watch draws the same table again every second where it stands",
    page: "front",
    options: { tree: { type: "boolean" }, watch: { type: "boolean" } },
    run: async ctx => {
      if (ctx.args.length > 1) throw usageRefusal("wsp threads takes at most one project; wsp threads wait is its one subcommand, and wsp thread read <thread> prints what one said.", usageIs(ctx));
      return drawRows(ctx, "wsp threads", async client => {
        const rows = await threadRows(client, ctx.args[0]);
        const lines = ctx.flags["tree"] === true ? threadTree(rows).flatMap(t => threadLines(t.row, "  ".repeat(t.depth))) : rows.flatMap(t => threadLines(t));
        return { value: { threads: rows }, rows: [...table([THREAD_HEAD, ...lines]), ...diskGoneLines(rows)] };
      });
    },
    tool: tool({
      description:
        "Every thread as the sidebar lists it: its project, the folder it works in (the project folder, or a worktree of the project's repo), that folder's branch as git reads it now, the computer it runs on, the agent inside, its state, who opened it (person, cli or agent) and its title. claudeSessionId is the agent's own id for the thread's session, absent until the agent announced one; for a Claude Code thread on this computer, cd into folder and run claude --resume with it to go on with the thread in a terminal, once the thread is not running; a thread on another computer goes on in a terminal there, its line also setting the CLAUDE_CONFIG_DIR its turns run with, as the app's Continue in terminal copies it. capped is on a thread waiting for a slot on its computer: the computer and what runs there against its threads at once; it starts on its own when one frees. Optionally within one project. A thread an agent inside another thread opened carries parentThreadId and rootThreadId, which is the tree stop ends as one. lastLine is the last line of the latest turn's reply and failure why that turn failed. subagents lists the agent's own subagents of every turn (id, title, state running, done, failed or stopped, model, asked: the start of what it was asked, lastLine or failure once it ends); stop takes one of them alone by its id.",
      input: { project: z.string().optional().describe("one project's threads, by the name or the id projects lists") },
      output: { threads: z.array(ThreadRowOut) },
      call: async ({ project: within }, deps) => asJson({ threads: await threadRows(await deps.client(), within) }),
    }),
  },
  {
    name: "threads wait",
    usage: "wsp threads wait <thread>... [--timeout <s>] [--tail]",
    about:
      "blocks until one of the threads leaves running and prints its finished line, with the reply whole under it; --tail prints the reply's last line alone, which is what a notify sends, and --timeout gives up after so many seconds and says so on stderr",
    page: "agent",
    options: { timeout: { type: "string" }, tail: { type: "boolean" } },
    run: async ctx => {
      if (ctx.args.length === 0) throw usageRefusal("wsp threads wait takes one thread or more.", usageIs(ctx));
      const timeoutMs = timeoutFlag(flag(ctx.flags, "timeout"));
      const named = await threadsOf(await ctx.client(), ctx.args);
      // The whole reply unless the tail was asked for: a last line is a paragraph's end or a code fence, and a
      // person waiting on a thread is waiting for its answer, not for the shape of its final line.
      const waited = await waitThrough(ctx, named, timeoutMs, () => ctx.io.error(HOST_RESTARTING_LINE));
      const { value, line } = waitAnswer(named, waited, ctx.flags["tail"] === true ? "tail" : "whole");
      if (value.timedOut === true) {
        ctx.out.emit(value);
        ctx.io.error(line);
      } else ctx.out.emit(value, line);
      return 0;
    },
    tool: tool({
      description: `Blocks until one of the named threads leaves running and answers with that thread's end: its id, status (completed, interrupted or failed), how long it worked, what it cost and the last line of its reply, and on a sub-thread result, what its machine was read to hold, the text being the one line a notify sends. One thread per call: a caller that started three builders calls this three times, dropping each returned id from the list, since a thread already over comes back at once and would come back again. With timeout, the seconds to wait before answering with nothing and timedOut true, so other work fits between calls; keep it under your own tool call limit and call again. This blocks, so it is for a shell script and not for your own conversation. ${NOTIFY_CALLER}. ${COORDINATOR_HANDOFF}. Never poll threads for a state change.`,
      input: {
        threads: z.array(z.string()).min(1).describe("thread ids, or prefixes that each pick one"),
        timeout: z.number().positive().optional().describe("seconds to wait; absent waits until one of the threads finishes"),
      },
      output: WaitOut.shape,
      call: async ({ threads: refs, timeout }, deps) => {
        const named = await threadsOf(await deps.client(), refs);
        const { value, line } = waitAnswer(named, await waitThrough(deps, named, timeout === undefined ? undefined : timeout * 1_000));
        return asText(line, value);
      },
    }),
  },
];
