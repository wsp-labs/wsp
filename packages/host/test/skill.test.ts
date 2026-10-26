// SPDX-License-Identifier: AGPL-3.0-only
// The wsp skill for agents on this computer: one file in the repo, read as
// text at build time, that names every verb and tool and gives the MCP server
// its instructions.
import { CLOUD_CAP_DEFAULT, CLOUD_ENV, DELETE_AFTER_MS, RUN_BLOCK_WORDS } from "@wsp/protocol";
import { BOX_CLASSES } from "@wsp/engine";
import { readFileSync } from "node:fs";
import { describe, expect, it, vi } from "vitest";
import { CATALOG_AGENTS, MCP_AGENT_IDS, THREAD_AGENTS } from "@wsp/catalog";
import { ANOTHER_AGENT_WORDS, BACKGROUND_WORK_WORDS, COORDINATOR_HANDOFF, NOTIFY_CALLER, SessionStartOutcome, SLATE_BRIEF, SLATE_PURPOSE, backgroundTasksLine, notifyLine, stillRunningLine, stillWorkingLine } from "@wsp/protocol";
import { slateCatalog } from "@wsp/protocol/slate";
import { BOAT_MARK, BOAT_SPAN_END, CLOUD_MARK, instructions, INSTRUCTIONS_KEPT, SLATE_WORDS, THREAD_SLATE_WORDS, RULES_HEADING, SETUP_HEADING, SHELL_HEADING, SKILL_NAME, VERBS_HEADING, wspSkill, agentsLine, instructionsOf, skillFor } from "../src/skill.js";
import { CLOUD_ON } from "../src/cloud.js";
import { ALL_VERBS, hasTool, CLI_VERBS, VERBS, toolName } from "../src/verbs.js";
import { SERVICE_MANAGERS } from "../src/service.js";
import { SLATE_VERBS } from "../src/verbs/slate.js";

const said = (text: string, word: string): boolean => new RegExp(`\\b${word}\\b`, "i").test(text);

describe("the wsp skill", () => {
  it("names, in the skill a public build carries, no verb or tool that build lacks, a bare backticked name included", () => {
    const publicSkill = skillFor(readFileSync(new URL("../../../skills/wsp/SKILL.md", import.meta.url), "utf8"), false);
    const names = ALL_VERBS.flatMap(v => [v.name, ...(hasTool(v) ? [toolName(v.name)] : [])].map(name => ({ name, cloudOnly: "cloud" in v && v.cloud === true })));
    // A span names the longest verb or tool its words open with, so `image build` is not read as the public `image`.
    const named = (span: string) =>
      names.filter(n => span === n.name || span.startsWith(`${n.name} `) || span.startsWith(`${n.name}(`)).sort((a, b) => b.name.length - a.name.length)[0];
    const lacking = [...publicSkill.matchAll(/`([^`\n]+)`/g)].map(m => m[1]!.replace(/^wsp /, "")).filter(span => named(span)?.cloudOnly === true);
    expect(lacking).toEqual([]);
    expect(named("pause")?.cloudOnly).toBe(true);
    expect(named("image build")?.cloudOnly).toBe(false);
  });

  it("carries, as a public build inlines it, Boat's spans and no word of Solari's, and keeps both with the cloud on", () => {
    const skill = readFileSync(new URL("../../../skills/wsp/SKILL.md", import.meta.url), "utf8");
    // What the skill-text plugin inlines where PUBLIC_BUILD=1.
    const publicSkill = skillFor(skill, false);
    expect(publicSkill).not.toMatch(/solari|slr_live_/i);
    expect(skillFor(skill, true)).toContain("SOLARI_API_KEY");
    const spans = [...skill.matchAll(/<!-- boat -->(.*?)<!-- \/boat -->/g)].map(m => m[1]!);
    const lines = skill.split("\n").filter(line => line.endsWith(` ${BOAT_MARK}`)).map(line => line.slice(0, -BOAT_MARK.length - 1));
    for (const text of [...spans, ...lines]) expect(publicSkill, text).toContain(text);
    expect(skillFor(`# t\nsub-threads ${BOAT_MARK}\nrun ${BOAT_MARK}--on boat ${BOAT_SPAN_END}here\nSolari ${CLOUD_MARK}\n`, false)).toBe("# t\nsub-threads\nrun --on boat here\n");
  });

  it("teaches sub-threads on Boat right after add, and prices Boat in the costs, in the skill a public build carries", () => {
    const publicSkill = skillFor(readFileSync(new URL("../../../skills/wsp/SKILL.md", import.meta.url), "utf8"), false);
    const headings = [...publicSkill.matchAll(/^### (.+)$/gm)].map(m => m[1]);
    expect(headings[headings.indexOf("add") + 1]).toBe("Sub-threads on Boat");
    const section = (heading: string): string => {
      const from = publicSkill.indexOf(`${heading}\n`);
      return publicSkill.slice(from, publicSkill.indexOf("\n#", from + heading.length));
    };
    const boat = section("### Sub-threads on Boat");
    expect(boat).toContain('`wsp run --on boat --notify me --detach --title "Fix the upload test" "<the brief>"`');
    expect(boat).toContain("`wsp merge in <you> <sub-thread>`");
    expect(boat).toContain(`deleted ${DELETE_AFTER_MS / 60_000} minutes after it is done`);
    expect(boat).toContain("12 a minute and 200 a day on the $20 plan");
    const costs = section("## Costs and limits");
    for (const c of BOX_CLASSES) expect(costs).toContain(`\`${c.cpu}x${c.memMb / 1024}\` (\`${c.name}\`, ${c.diskGb} GB) $${c.rateUsdPerHour.toFixed(3)}`);
    expect(costs).toContain("The trial, until the first payment, runs 2 machines at once, sells `2x4` and `4x8` only, and takes 5 starts a minute, 25 an hour and 75 a day.");
    expect(costs).toContain(`wsp holds Boat to ${CLOUD_CAP_DEFAULT.machines} machines at once`);
  });

  it("is the repo's skills/wsp/SKILL.md as this process reads it, with the frontmatter name and a one-line description without a colon or a quote", () => {
    expect(wspSkill()).toBe(skillFor(readFileSync(new URL("../../../skills/wsp/SKILL.md", import.meta.url), "utf8"), CLOUD_ON));
    const [open, name, description, close] = wspSkill().split("\n");
    expect(open).toBe("---");
    expect(name).toBe(`name: ${SKILL_NAME}`);
    expect(close).toBe("---");
    expect(description).toMatch(/^description: \S/);
    expect(description!.slice("description: ".length)).not.toMatch(/[:"']/);
    expect(description!.length).toBeLessThan(1024);
  });

  it("names every verb in the verbs table and the agents the MCP server installs for; a new verb without a line fails here", () => {
    for (const verb of CLI_VERBS) expect(wspSkill(), verb.name).toContain(`\`wsp ${verb.name}`);
    for (const verb of VERBS.filter(hasTool)) expect(wspSkill(), verb.name).toContain(`\`${toolName(verb.name)}\``);
    expect(wspSkill()).toContain(`get the server: ${MCP_AGENT_IDS}.`);
    // The install's flags are stated where the reader is sent to find them, not only in the walkthrough and the help.
    expect(wspSkill()).toContain("`--agent` repeats to do several in one call");
    expect(wspSkill()).toContain("`--json` answers with one line holding the `server` command every config now runs, what each agent took, its `docs` naming the files the section went into, and a `failures` array");
    expect(wspSkill()).toContain("An entry under `installed` with no `path` took the skill and not the server");
    // The section it keeps in the project's own instructions, both of the things a second run does to it, and the
    // agent a run with no --agent picks off a terminal: an agent reading the skill decides on those.
    expect(wspSkill()).toContain("wsp's own marked section into the instructions the folder it runs in keeps");
    expect(wspSkill()).toContain("A second run replaces that section where it stands");
    expect(wspSkill()).toContain("`wsp mcp install --agent <id> --remove` takes it back out");
    expect(wspSkill()).toContain("off a terminal a run that names none takes every agent whose own command is on the PATH");
  });

  it("quotes the notify line as the protocol prints it and names every send outcome the protocol knows", () => {
    expect(wspSkill()).toContain(`\`${notifyLine("1a2b3c4d-0000", { status: "completed", durationMs: 724_000, costUsd: 0.41, text: "first line\n<last line of the reply>" })}\``);
    for (const outcome of SessionStartOutcome.options) expect(wspSkill(), outcome).toContain(`(outcome \`${outcome}\`)`);
    // A target is a thread this caller drives, so the flag's own paragraph says which threads its tree holds.
    expect(wspSkill()).toContain("`--notify <thread>` names another thread outright, one of your own tree: the lead that started you, a thread beside you under it, or one you started");
  });

  it("tells an orchestrating agent to start its builders with notify me and end its turn, and hands the blocking wait to a shell script", () => {
    // The verb that blocks is out of the table an agent reads and in the section for a script, with its row intact.
    const agentRows = wspSkill().slice(wspSkill().indexOf(VERBS_HEADING), wspSkill().indexOf(SHELL_HEADING));
    expect(agentRows).not.toContain("`wsp threads wait");
    const shell = wspSkill().slice(wspSkill().indexOf(SHELL_HEADING), wspSkill().indexOf(RULES_HEADING));
    expect(shell).toContain("| `wsp threads wait <thread>... [--timeout <s>] [--tail]` | `threads_wait` (threads, timeout) |");
    const section = wspSkill().slice(wspSkill().indexOf("### threads wait"), wspSkill().indexOf("### stop"));
    expect(section).toContain("wsp run dev --detach --notify me");
    expect(section).toContain("wsp threads wait 1a2b3c4d 5e6f7a8b --timeout 600");
    expect(section).toContain("One thread per call");
    expect(section).toContain("`thread 1a2b3c4d still running after 10m`");
    expect(section).toContain("This is a shell script's verb");
    // The wait prints the agent's reply whole, and the tail is the flag: a reply's last line is as often a code
    // fence as an answer, which is what a tester waiting on two threads read off both.
    expect(section).toContain("with the agent's reply whole under it");
    expect(section).toContain("`--tail` prints the last line alone");
    expect(section).toContain("Do not call it in your own conversation");
    expect(section).toContain("Never poll `threads`");
    const loop = wspSkill().slice(wspSkill().indexOf("## The loop for building with wsp"), wspSkill().indexOf("## Where the person steps in"));
    expect(loop).toContain("--detach");
    expect(loop).toContain("--notify me");
    expect(loop).toContain("end your turn");
    expect(loop).not.toContain("threads wait");
    expect(loop).not.toContain("nohup wsp run");
    // The MCP instructions carry both roads, since an agent holding only the tools reads nothing else.
    for (const words of [NOTIFY_CALLER, COORDINATOR_HANDOFF, BACKGROUND_WORK_WORDS]) expect(instructions(), words.slice(0, 40)).toContain(words);
  });

  it("opens the instructions with what another agent is, and keeps them whole inside what Claude Code keeps", () => {
    expect(instructions().length).toBeLessThanOrEqual(INSTRUCTIONS_KEPT);
    expect(instructions(true).length).toBeLessThanOrEqual(INSTRUCTIONS_KEPT);
    expect(instructions().startsWith(`${ANOTHER_AGENT_WORDS}.`)).toBe(true);
    expect(instructions()).toContain("in the wsp skill");
  });

  it("leaves room under the cut for a thread agent or two more, with the cloud on or off", async () => {
    // Each agent the catalog adds lengthens the agents line, which every server's instructions carry.
    const room = 60;
    try {
      for (const cloud of ["", "1"]) {
        vi.resetModules();
        vi.stubEnv(CLOUD_ENV, cloud);
        const fresh = await import("../src/skill.js");
        for (const scoped of [false, true]) expect(fresh.instructions(scoped).length, `cloud "${cloud}", scoped ${scoped}`).toBeLessThanOrEqual(INSTRUCTIONS_KEPT - room);
      }
    } finally {
      vi.unstubAllEnvs();
    }
  });

  it("opens a thread's own instructions with its slate, keyed on what the person wants to see, and leaves the others as they are", () => {
    expect(instructions(true)).toBe(`${THREAD_SLATE_WORDS}\n\n${instructions().replace(`${SLATE_WORDS}. `, "")}`);
    // A thread another thread started has no slate: its server's instructions are the thread's own with no word of one.
    expect(instructions(true, false)).toBe(instructions().replace(`${SLATE_WORDS}. `, ""));
    expect(instructions(true, false).toLowerCase()).not.toContain("slate");
    expect(THREAD_SLATE_WORDS.length).toBeLessThan(INSTRUCTIONS_KEPT / 2);
    expect(instructions()).not.toContain(THREAD_SLATE_WORDS);
    expect(instructions(true).slice(0, INSTRUCTIONS_KEPT)).toContain(`${ANOTHER_AGENT_WORDS}.`);
  });

  it("says once a slate is written a request to see lands there, the person's result stays there, and the slate's own code lives in its files", () => {
    expect(THREAD_SLATE_WORDS).toContain("Once the slate is written, a request to see something lands there.");
    expect(THREAD_SLATE_WORDS).toContain("Build and read it with the slate tools, never wsp from a shell.");
    // A one-off answer stays in chat, and the reply after a write is short and checked first.
    expect(THREAD_SLATE_WORDS).toContain("A one-off answer or an explanation stays in chat unless they ask to see it.");
    // At medium effort agents fetched with another tool and answered in chat: the first call is the catalog, whatever fetches.
    expect(THREAD_SLATE_WORDS).toContain("your first tool call is slate_catalog, then slate_write; a list in chat is not a slate.");
    expect(THREAD_SLATE_WORDS).toContain("Show data fetched with another tool on the slate, not in chat.");
    for (const said of ["tick off", "as it goes", "keep an eye on", "what's unread", "live"]) expect(THREAD_SLATE_WORDS.slice(0, THREAD_SLATE_WORDS.indexOf("your first tool call")), said).toContain(said);
    expect(THREAD_SLATE_WORDS).toContain("Read the sketch a write answers before saying it works, then reply briefly: what you built and what waits on the person.");
    const text = wspSkill();
    const section = text.slice(text.indexOf("### slate\n"), text.indexOf("\n## ", text.indexOf("### slate\n")));
    for (const words of [
      "a request to see something lands there", "gets its result there unless they ask in the chat",
      "Code only the slate uses goes in a `<file name=\"x.py\">`, never in the project, and runs as `\"$SLATE_DIR/x.py\"`; project code runs where it is.",
      "give a `<secret name=\"token\" />` input; never ask them to paste it into a file or the chat",
      "Build and read it with the slate tools (`slate_catalog`, `slate_write`, `slate_state`, `slate_read`), never `wsp` from a shell",
      "one heading per section", "status and last-checked lines small and muted", "actions at the end of their row with one primary per section",
      "mono only for figures, ids, times and paths", "every live number with its window and unit", "nothing centered but a lone figure or card",
    ]) expect(section, words).toContain(words);
  });

  // slate-reach.json is what people say when a slate is the answer, the owner's own words among them; no model reads
  // it here, it only holds the instructions and the tool descriptions, which tool search matches, to those words.
  it("says every word the slate reach prompts rely on in a thread's instructions and in a slate tool's description", () => {
    const reach = JSON.parse(readFileSync(new URL("./slate-reach.json", import.meta.url), "utf8")) as { prompt: string; intent: string[] }[];
    const kept = instructions(true).slice(0, INSTRUCTIONS_KEPT);
    const described = VERBS.filter(hasTool).filter(v => v.name.startsWith("slate ")).map(v => v.tool.description);
    expect(described).toHaveLength(5);
    for (const { prompt, intent } of reach) {
      expect(intent.length, prompt).toBeGreaterThan(0);
      for (const word of intent) {
        expect(said(prompt, word), `${word} in "${prompt}"`).toBe(true);
        expect(said(kept, word), `${word} in a thread's instructions`).toBe(true);
        expect(described.some(d => said(d, word)), `${word} in a slate tool's description`).toBe(true);
      }
    }
  });

  // A Codex thread asked for a live panel wrote its sampler into the project, where Changes listed it.
  it("says in the slate_write description, the brief, the catalog and the skill that a run's script is a <file> in the thread's slate folder, never in the project", () => {
    const write = VERBS.find(v => v.name === "slate write");
    const section = wspSkill().slice(wspSkill().indexOf("### slate\n"), wspSkill().indexOf("\n## ", wspSkill().indexOf("### slate\n")));
    const texts: Record<string, string> = {
      "Claude slate brief": SLATE_BRIEF,
      "slate_write description": write !== undefined && hasTool(write) ? write.tool.description : "",
      "catalog runs": slateCatalog("runs"),
      "skill": section,
    };
    for (const [where, text] of Object.entries(texts)) {
      for (const words of ['<file name="x.py">', "$SLATE_DIR/x.py", "never in the project"]) expect(text, `${words} in the ${where}`).toContain(words);
    }
  });

  // The record is what the tool server greets with, a Codex thread's wsp_slate server among them; the brief is Claude's.
  it("names the slate's chart kinds and its image wherever an agent learns its pieces, and no look the owner dropped", () => {
    const served = JSON.parse(readFileSync(new URL("../../../daemon/crates/wsp-mcp/record/server.json", import.meta.url), "utf8")) as { instructions: Record<string, string> };
    const texts: Record<string, string> = {
      catalog: slateCatalog(),
      "Claude slate brief": SLATE_BRIEF,
      "thread instructions": instructions(true),
      "served scoped, cloud off": served.instructions["scopedCloudOff"]!,
      "served scoped, cloud on": served.instructions["scopedCloudOn"]!,
    };
    for (const [where, text] of Object.entries(texts)) {
      for (const kind of ["chart", "donut", "timeline", "treemap", "diagram", "image"]) expect(said(text, kind), `${kind} in the ${where}`).toBe(true);
      for (const dropped of ["ramp", "outlined"]) expect(said(text, dropped), `${dropped} in the ${where}`).toBe(false);
    }
    for (const prop of ["<series", "stack"]) expect(slateCatalog(), prop).toContain(prop);
    expect(served.instructions["scopedCloudOn"]!.length).toBeLessThanOrEqual(INSTRUCTIONS_KEPT);
    // The skill sends an agent to the catalog for the pieces and names none of its own.
    const section = wspSkill().slice(wspSkill().indexOf("### slate\n"), wspSkill().indexOf("\n## ", wspSkill().indexOf("### slate\n")));
    for (const kind of ["sparkline", "chart", "donut", "timeline", "treemap"]) expect(said(section, kind), `${kind} in the skill`).toBe(false);
  });

  it("says a send is never refused for meeting a turn, and names the steer, the queue and the reply tail in the runtime's own words", () => {
    const section = wspSkill().slice(wspSkill().indexOf("### send"), wspSkill().indexOf("### threads wait"));
    expect(section).toContain("A send is never refused for meeting a turn");
    expect(section).toContain("(outcome `steered`)");
    expect(section).toContain("(outcome `queued`)");
    expect(section).toContain(`\`${stillWorkingLine()}\``);
    expect(section).toContain("Two sends keep the order they arrived in");
  });

  it("no section of the skill still teaches the wait or the refusal, whichever section an agent opens first", () => {
    // Read whole, not by section: the two rules this ticket removed slipped in through a paragraph no test read.
    for (const words of ["the send is refused", "a `send` before then is refused"]) expect(wspSkill(), words).not.toContain(words);
    // Three sections may name the wait: the one whose table holds its row, its own section under the contract, and
    // the rules learned the hard way, which is a log every line of which restates a rule from the body. No other
    // paragraph may send a reader to it, which is how the run paragraph kept teaching it.
    const owns =
      wspSkill().slice(wspSkill().indexOf(SHELL_HEADING), wspSkill().indexOf(RULES_HEADING)) +
      wspSkill().slice(wspSkill().indexOf("### threads wait"), wspSkill().indexOf("### stop")) +
      wspSkill().slice(wspSkill().indexOf("## Rules learned the hard way"));
    const elsewhere = wspSkill().split("\n").filter(line => /threads.wait/.test(line) && !owns.includes(line));
    expect(elsewhere).toEqual([]);
  });

  it("tells an agent it can talk to another thread: the verb, where the id comes from, and how the answer comes back", () => {
    const section = wspSkill().slice(wspSkill().indexOf("### send"), wspSkill().indexOf("### threads wait"));
    expect(section).toContain("Threads talk to each other");
    expect(section).toContain("`wsp send <thread> \"<message>\"` (the `send` tool)");
    expect(section).toContain("`wsp threads` (the `threads` tool) is where you find that id");
    expect(section).toContain("comes back as its own next message");
    expect(section).toContain("`--notify me`");
    // Which threads that road reaches: every thread of this caller's own tree, its lead, its siblings and its
    // children, wherever each runs, and the person for anything else, so the sentence an agent reads is the one
    // the host keeps.
    expect(section).toContain("one of your own tree, the lead that started you, a thread beside you under it or one you started, wherever it runs");
    expect(section).not.toContain("whoever opened it");
    expect(section).not.toContain("one you started or one under it");
    expect(section).toContain("goes to the person");
  });

  it("says a thread reaches its whole tree wherever each runs, the lead that started it included, and that stopping the lead stops the tree", () => {
    const opening = wspSkill().slice(0, wspSkill().indexOf(SETUP_HEADING));
    expect(opening).toContain("the lead that started it, the threads beside it under that lead and the threads it started, wherever each runs");
    expect(opening).toContain("the person's own thread and another lead's tree in the same folder are left out of `threads`");
    expect(opening).toContain("Stopping a thread stops every thread under it, so a child that stops its lead stops its siblings and itself with it.");
    // The old rule, downward only, is taught nowhere: a paragraph that kept it would send a child's report to the person.
    expect(wspSkill()).not.toContain("the threads it started and the threads under those, and nothing else");
    const section = wspSkill().slice(wspSkill().indexOf("### send"), wspSkill().indexOf("### threads wait"));
    expect(section).toContain("A stop cascades: stopping your lead stops every thread under it, your siblings and you among them.");
  });

  it("quotes the line a reply ends on when a background command is still running, as the adapter words it", () => {
    const rules = wspSkill().slice(wspSkill().indexOf("## Rules learned the hard way"));
    expect(rules).toContain(`\`${backgroundTasksLine(1)}\``);
    expect(rules).toContain(`\`${stillRunningLine(1)}\``);
  });

  it("says where a thread on this computer starts, and its example asks nothing of a folder it may not be in", () => {
    const section = wspSkill().slice(wspSkill().indexOf("### run"), wspSkill().indexOf("### send"));
    expect(section).toContain("wsp run --agent codex \"Say in one line which folder you are in");
    // One statement of where a thread starts: the project's folder, a worktree for another branch, or beside the
    // thread asking, so the paragraph cannot say two things about the same start.
    expect(section).toContain("`wsp run <project>` opens a thread in the project's folder on this computer");
    expect(section).toContain("`--branch <branch>` runs it in a worktree of the project's repo on that branch instead");
    expect(section).toContain("from a thread it runs beside you in your own folder");
    // No constant path stands in for that folder: the host decides it, and a line naming one would go stale.
    expect(section).not.toContain("~/wsp-work");
  });

  it("tells a person with no image, in the setup walkthrough, to build the default one with wsp doctor on the cloud computer", () => {
    const skill = readFileSync(new URL("../../../skills/wsp/SKILL.md", import.meta.url), "utf8");
    const step = skill.split("\n").find(line => line.startsWith("3. "))!;
    expect(step).toContain("<!-- cloud -->Where threads run. A thread on a cloud runs on a machine forked from the person's image");
    expect(step).toContain("A host with no image has none to fork, and `wsp doctor <computer>`, naming the cloud computer, builds the default one.<!-- /cloud -->");
    expect(skillFor(skill, false)).not.toContain("wsp doctor <computer>, naming");
  });

  it("names the unit wsp join writes as the manager itself names a place's unit, and says a Mac refuses", () => {
    const start = wspSkill().indexOf("`wsp join <address>...");
    const sentence = wspSkill().slice(start, wspSkill().indexOf("`--code-file", start));
    const place = { role: "place" as const, statePath: "/root/.wsp/place.json", home: "/root", uid: 0 };
    expect(sentence).toContain(SERVICE_MANAGERS.systemd.held(place)[0]!.words);
    expect(sentence).not.toContain(SERVICE_MANAGERS.launchd.held(place)[0]!.words);
    expect(sentence).not.toContain("systemd user unit");
    expect(sentence).toContain("a Mac refuses to join");
  });

  it("carries no em dash", () => {
    expect(wspSkill()).not.toContain("\u2014");
  });

  it("keeps the host the agent's own job and the keys the person's", () => {
    const setup = wspSkill().slice(wspSkill().indexOf(SETUP_HEADING), wspSkill().indexOf("## Verbs and tools"));
    // A key written after the host started is read by the next host, so the one step that writes one says how.
    expect(setup.slice(setup.indexOf("\n2. "), setup.indexOf("\n3. "))).toContain("wsp down");
    expect(setup).toContain("Starting the host is neither: any command that needs one starts it");
    expect(setup).toContain("it reads those keys off the file itself, so nothing about them passes through you");
    expect(setup).toContain("The tools show up only after that agent restarts");
    expect(setup).toContain("prefer that over `npx @wsp-labs/wsp`");
    // Every install line names the published package; a bare npx or a stale name would send them to another one.
    expect(wspSkill().match(/\b(?:npm i -g|npx) (?!@wsp-labs\/wsp\b)\S+/g)).toBeNull();
  });

  it("is a runbook: every step ends in its own Expect line, and the section ends with what to run inside the agent", () => {
    const setup = wspSkill().slice(wspSkill().indexOf(SETUP_HEADING), wspSkill().indexOf("## Verbs and tools"));
    const closing = setup.indexOf("\nLast, ");
    const lines = setup.slice(0, closing).split("\n");
    const steps = lines.filter(l => /^\d+\. /.test(l));
    expect(setup.split("\n").filter(l => /^ *Expect: \S/.test(l))).toHaveLength(steps.length + 1);
    for (const [at, line] of lines.entries()) {
      if (!/^\d+\. /.test(line)) continue;
      const next = lines.slice(at + 1).findIndex(l => /^\d+\. /.test(l));
      const body = lines.slice(at, next === -1 ? undefined : at + 1 + next);
      expect(body.filter(l => /^ *Expect: /.test(l)), line.slice(0, 40)).toHaveLength(1);
    }
    expect(setup).toContain("Every step below ends in an `Expect:` line");
    expect(setup).toContain("say what to run next inside their own agent");
    expect(setup).toContain("In Claude Code the skill is then `/wsp`");
    expect(setup.trimEnd().endsWith("that thread shows in the person's sidebar.")).toBe(true);
  });

  it("the MCP instructions are what another agent is, the slate, what wsp is and where the skill is, the agents, and the roads to a child's end, whole inside what Claude Code keeps", () => {
    expect(instructions()).toBe(instructionsOf(THREAD_AGENTS));
    expect(instructions().startsWith(`${ANOTHER_AGENT_WORDS}. ${SLATE_WORDS}. Agents work on a project, a folder on one computer`)).toBe(true);
    // The slate's sentence is second and under 220 characters with its full stop.
    expect(`${SLATE_WORDS}.`.length).toBeLessThan(220);
    // This server is the person's own, not a thread's, so a slate call from it names the thread.
    expect(SLATE_WORDS).not.toContain("This thread");
    expect(SLATE_WORDS).toContain("naming the thread");
    expect(instructions().indexOf(`${SLATE_WORDS}.`)).toBe(ANOTHER_AGENT_WORDS.length + 2);
    // The skill's slate section stays under 600 tokens, counted at four characters a token.
    const text = wspSkill();
    const section = text.slice(text.indexOf("### slate\n"), text.indexOf("\n## ", text.indexOf("### slate\n")));
    expect(section.length / 4).toBeLessThan(600);
    // The setup walkthrough and the other rules are the skill's alone: Claude Code cut them off and Codex copied them
    // into every deferred tool's entry.
    expect(instructions()).toContain("Setup, every verb and the rules are in the wsp skill");
    expect(instructions()).not.toContain("The road is a health check");
    expect(wspSkill()).toContain("The road is a health check");
    // Everything but the rules is one line, so a client that shows the instructions as a paragraph shows them whole.
    expect(instructions().split("\n").filter(line => !line.startsWith("- "))).toHaveLength(1);
    expect(instructions()).not.toContain("## ");
  });

  it("the rules for running work on a machine are thirteen lines stated as facts about machines, twelve with no cloud, and the instructions carry the first two and the background one", () => {
    const from = wspSkill().indexOf(`\n${RULES_HEADING}\n`);
    expect(from, RULES_HEADING).toBeGreaterThan(-1);
    const section = wspSkill().slice(from, wspSkill().indexOf("\n## ", from + 1));
    const rules = section.split("\n").filter(line => line.startsWith("- "));
    expect(rules).toHaveLength(CLOUD_ON ? 13 : 12);
    // A command meant for the person closes the section, in the one sentence the launch context quotes too.
    expect(rules.at(-1)).toBe(`- ${RUN_BLOCK_WORDS}.`);
    // The two roads to a child's end open the section: which one holds is the first thing a caller has to decide.
    expect(rules[0]).toContain(NOTIFY_CALLER);
    expect(rules[1]).toContain(COORDINATOR_HANDOFF);
    // Then what another agent is, before any rule that starts one.
    expect(rules[2]).toBe(`- ${ANOTHER_AGENT_WORDS}.`);
    // The instructions end on the roads to a child's end and the background road, each the skill's words.
    const carried = instructions().split("\n").slice(1);
    expect(carried).toHaveLength(3);
    for (const [line, words] of carried.map((line, at) => [line, [NOTIFY_CALLER, COORDINATOR_HANDOFF, BACKGROUND_WORK_WORDS][at]!] as const)) expect(line).toContain(words);
    // Whole sentences a reader with no history can act on: no ticket number, no date, nothing that happened once.
    for (const rule of rules) {
      expect(rule, rule.slice(0, 40)).toMatch(/\.$/);
      expect(rule, rule.slice(0, 40)).not.toMatch(/#\d|\bticket\b|\b20\d\d\b/);
    }
    // The rest, each by the fact it turns on: which kind of workspace the work goes on, the golden, the count, the
    // worktree, long work in the background, the send, the restart, the pause, the person reading along, and the
    // block a person runs.
    expect(section).toContain("A thread on this computer runs in the project's folder, beside any other thread there");
    expect(section).toContain("`wsp run <project> --branch <branch>`");
    expect(section).toContain("a quick subtask or a second harness");
    if (CLOUD_ON) {
      expect(section).toContain("Fork a cloud machine for builds that run beside each other");
      expect(section).toContain("anything that should not touch this computer");
      expect(section).toContain("`wsp pause <workspace>`");
    } else {
      expect(section).not.toContain("--from");
    }
    expect(section).toContain("on 2 vCPU and 4 GB one thread runs tests or a build at a time");
    expect(section).toContain("its own git worktree");
    expect(section).toContain("`pnpm install --offline`");
    expect(section).toContain(BACKGROUND_WORK_WORDS);
    expect(section).toContain("a send into a thread whose turn is still running opens no second turn");
    expect(section).toContain("Restarting the host cuts every turn running anywhere");
    expect(section).toContain("shows in their sidebar");
    expect(section).toContain("`--title`");
  });

  it("the instructions name every agent the host has an adapter for and no other catalog agent, read from the registry", () => {
    const named = (id: string): boolean => new RegExp(`\\b${id}\\b`).test(instructions());
    for (const id of THREAD_AGENTS) expect(named(id), id).toBe(true);
    for (const a of CATALOG_AGENTS) if (!THREAD_AGENTS.some(id => id === a.id)) expect(named(a.id), a.id).toBe(false);
    expect(instructions()).toContain(` as agent: ${THREAD_AGENTS.join(", ")}.`);
    expect(agentsLine(["claude", "codex"])).toBe(`The agents this host runs threads on, the only values ${CLOUD_ON ? "run and fork take" : "run takes"} as agent: claude, codex.`);
  });
});

describe("what a slate is for", () => {
  it("reads the same in the catalog's index, the server's instructions, the slate tools' descriptions and the skill", () => {
    const skill = readFileSync(new URL("../../../skills/wsp/SKILL.md", import.meta.url), "utf8");
    const places: Record<string, string> = {
      "the catalog's index": slateCatalog(),
      "a thread's server instructions": instructions(true),
      "a thread's system prompt": SLATE_BRIEF,
      "the skill": skill.slice(skill.indexOf("### slate\n")),
    };
    for (const verb of SLATE_VERBS.filter(hasTool).filter(v => v.name === "slate catalog" || v.name === "slate write")) places[`${verb.name}'s description`] = verb.tool.description;
    expect(Object.keys(places)).toHaveLength(6);
    for (const [where, text] of Object.entries(places)) expect(text, where).toContain(SLATE_PURPOSE);
    expect(slateCatalog().startsWith(SLATE_PURPOSE)).toBe(true);
  });

  it("says never to poll in chat where every agent reads it, not in Claude's brief alone", () => {
    const write = SLATE_VERBS.filter(hasTool).find(v => v.name === "slate write")!;
    for (const [where, text] of Object.entries({ "the catalog's index": slateCatalog(), "slate write's description": write.tool.description })) expect(text, where).toMatch(/never poll in chat/i);
  });
});
