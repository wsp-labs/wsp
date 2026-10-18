// SPDX-License-Identifier: AGPL-3.0-only
// The Pull request pane's states, read through the stand-in gh in app/tests/pr_pane/gh/ (the host's own gh on the
// fixture: bin/ leads its PATH), and a 5,000-line change in Changes. Each pull request state has a project of its own
// whose repo is on branch work with origin on github.com; git's https and http are refused in the fixture's
// .gitconfig before that repo exists, so neither git nor gh ever reaches GitHub.
import { spawnSync } from "node:child_process";
import { appendFileSync, existsSync, mkdirSync, readFileSync, readlinkSync, renameSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PANEL = "[data-preview-panel-mode]";
const GH = join(dirname(fileURLToPath(import.meta.url)), "../../app/tests/pr_pane/gh/gh");
const REMOTE = "https://github.com/wsp-test/repo.git";
const NO_NETWORK = '[protocol "https"]\n\tallow = never\n[protocol "http"]\n\tallow = never\n';
const css = selector => `css:${selector}`;
const has = (app, selector, ms = 15_000) => app.waitFor(s => document.querySelector(s) !== null, ms, selector);

function git(host, cwd, ...args) {
  const ran = spawnSync("git", ["-c", "user.name=parity", "-c", "user.email=parity@example.com", "-c", "commit.gpgsign=false", ...args], {
    cwd,
    env: { HOME: host.home, PATH: "/usr/bin:/bin:/usr/local/bin", LANG: "en_US.UTF-8" },
    encoding: "utf8",
  });
  if (ran.status !== 0) throw new Error(`git ${args.join(" ")}: ${ran.stderr.trim()}`);
  return ran.stdout;
}

/** The stand-in gh reads its scenario and state from $HOME/gh-stub, so the gh on the host's PATH hands it a HOME of
 * the project's own, found from the repo gh runs in (a worktree's too); anywhere else it answers from the last one. */
const ghFor = parity => `#!/bin/sh
common=$(git rev-parse --path-format=absolute --git-common-dir 2>/dev/null)
case $common in
  ${JSON.stringify(`${parity}/`)}*/.git) key=\${common%/.git}; HOME=${JSON.stringify(`${parity}/.gh/`)}\${key##*/}; export HOME ;;
esac
exec ${JSON.stringify(GH)} "$@"
`;

/** The stand-in gh on the host's PATH, the scenario each project's pull request answers with, and git kept off the
 * network. */
function stub(host, scenario, key) {
  const config = join(host.home, ".gitconfig");
  if (!existsSync(config) || !readFileSync(config, "utf8").includes('[protocol "https"]')) appendFileSync(config, NO_NETWORK);
  const parity = join(host.home, "parity-pr");
  const link = join(host.home, "bin", "gh");
  const script = ghFor(parity);
  if (!existsSync(link) || readFileSync(link, "utf8") !== script) {
    writeFileSync(`${link}.next`, script, { mode: 0o755 });
    renameSync(`${link}.next`, link);
  }
  for (const box of [join(host.home, "gh-stub"), join(parity, ".gh", key, "gh-stub")]) {
    mkdirSync(box, { recursive: true });
    writeFileSync(join(box, "scenario"), `${scenario}\n`);
  }
}

/** A project whose repo has main and, one commit ahead, work checked out, origin set before the host records it. */
async function prProject(host, scenario, key = scenario) {
  stub(host, scenario, key);
  const name = `pr-${key}`;
  const folder = join(host.home, "parity-pr", key);
  if (!existsSync(join(folder, ".git"))) {
    mkdirSync(join(folder, "src"), { recursive: true });
    writeFileSync(join(folder, "src/pane.rs"), "fn main() {}\n");
    git(host, folder, "init", "-q", "-b", "main");
    git(host, folder, "add", "-A");
    git(host, folder, "commit", "-qm", "init");
    git(host, folder, "remote", "add", "origin", REMOTE);
    git(host, folder, "checkout", "-qb", "work");
    writeFileSync(join(folder, "src/pane.rs"), 'fn main() {\n    println!("pane");\n}\n');
    git(host, folder, "commit", "-qam", "Teach the pane to read checks");
    await host.wsp("add", folder, "--name", name);
  }
  return { name, folder };
}

/** The thread taken back out of the fold where it settled: a merged or closed pull request settles its thread at
 * once, and a finished review's thread settles too, which leaves the sidebar's list the driver opens threads from. */
async function shown(host, id) {
  if ((await host.threads()).find(t => t.threadId === id || t.id === id)?.settledAt !== undefined) await host.op("sessions.restore", { threadIds: [id] });
  return id;
}

/** A thread on the scenario's project (or the one `key` names); its turn's end reads the pull request. */
async function prThread(host, scenario, key = scenario) {
  const { name } = await prProject(host, scenario, key);
  const id = await host.thread(`Parity pull request, ${key}.`, { project: name, reply: { kind: "text", text: "Done." } });
  // Every state is #42 on one remote, whose page the host holds for a minute and reads in its own home, where gh
  // answers from the scenario written last: read this state's page now, while that is this state's.
  const workspaceId = (await host.threads()).find(t => t.threadId === id || t.id === id).workspaceId;
  for (let tries = 0; ; tries++) {
    try {
      await host.op("workspaces.pullRequestView", { workspaceId, fresh: true });
      break;
    } catch (error) {
      if (tries === 20) throw error;
      await new Promise(resolve => setTimeout(resolve, 500));
    }
  }
  return shown(host, id);
}

/** Opens a pane by its panes.ts kind on the thread, as states.mjs opens the others: open, behind a tab or shut. */
async function pane(app, thread, kind, label, root) {
  await app.route.thread(thread);
  for (let tries = 0; tries < 3; tries++) {
    const at = await app.page(
      (k, r, words) => {
        if (document.querySelector(r) !== null) return "open";
        if (document.querySelector(`[data-surface-launch="${k}"]`)) return "launcher";
        const tab = [...document.querySelectorAll("[data-right-panel-tab-list] button")].find(b => b.textContent.trim() === words);
        if (tab !== undefined) return tab.click(), "tab";
        return document.querySelector('button[aria-label="Add a panel"]') ? "tabs" : "closed";
      },
      kind,
      root,
      label,
    );
    if (at === "open" || at === "tab") break;
    if (at === "launcher") {
      await app.click(css(`[data-surface-launch="${kind}"]`));
      break;
    }
    if (at === "tabs") {
      await app.click(css('button[aria-label="Add a panel"]'));
      await app.waitFor(w => [...document.querySelectorAll("[role=menuitem]")].some(e => e.textContent.includes(w)), 10_000, label);
      await app.page(w => [...document.querySelectorAll("[role=menuitem]")].find(e => e.textContent.includes(w)).click(), label);
      break;
    }
    await app.click(css('button[aria-label="Toggle right panel"]'));
    await app.sleep(600);
  }
  await has(app, root, 30_000);
}

async function prPane(app, thread) {
  await pane(app, thread, "pr", "Pull request", "[data-pr-pane]");
  await has(app, "[data-pr-pane] [data-pr-title]", 30_000);
  await app.waitFor(() => document.querySelector("[data-pr-pane] [data-pr-skeleton], [data-pr-pane] [aria-busy=true]") === null, 30_000);
  await app.sleep(800);
}

const REVIEW = `Looks fine.

${"```"}json
${JSON.stringify({
  verdict: "request_changes",
  summary: "Two notes.",
  comments: [
    { path: "src/pane.rs", line: 7, side: "RIGHT", body: "Name the field." },
    { path: "src/pane.rs", line: 1, side: "RIGHT", body: "Keep main small." },
  ],
})}
${"```"}`;

/** A claude that answers every message with REVIEW, the block a reviewer ends with, and offers the read-only mode a
 * review starts in, which the parity stub does not. As tests/pr_review.rs's. */
const REVIEWER = `#!/usr/bin/env node
const argv = process.argv.slice(2);
const at = flag => argv[argv.indexOf(flag) + 1];
const say = o => process.stdout.write(JSON.stringify(o) + "\\n");
if (argv.includes("--version")) console.log("2.1.295 (Claude Code)");
else if (argv.includes("--help")) console.log('--permission-mode <mode>  (choices: "default", "dontAsk", "bypassPermissions")');
else if (argv.includes("--bare")) say({ type: "control_response", response: { subtype: "success", request_id: "init", response: { models: [{ value: "default", resolvedModel: "claude-haiku-5-5" }] } } });
if (!argv.includes("--input-format")) process.exit(0);
const session = at("--session-id") ?? at("--resume");
const text = ${JSON.stringify(REVIEW)};
require("node:readline").createInterface({ input: process.stdin }).on("line", () => {
  say({ type: "system", subtype: "init", session_id: session, model: "claude-haiku-5-5", cwd: process.cwd(), tools: [] });
  say({ type: "assistant", session_id: session, message: { role: "assistant", content: [{ type: "text", text }] } });
  say({ type: "result", subtype: "success", is_error: false, result: text, session_id: session });
});
`;

/** A review workspace on the open pull request whose reviewer left a draft; answers its thread. Made once per
 * fixture on a project of its own: the review's worktree takes branch work, so its folder goes back to main. */
async function reviewed(host) {
  const made = join(host.home, "parity-pr", "review.json");
  if (existsSync(made)) return shown(host, JSON.parse(readFileSync(made, "utf8")).thread);
  const thread = await prThread(host, "open", "review");
  const { folder } = await prProject(host, "open", "review");
  const workspaceId = (await host.threads()).find(t => t.threadId === thread || t.id === thread).workspaceId;
  // A review on this computer runs in a worktree on the pull request's branch, which a fetch would make.
  git(host, folder, "switch", "-q", "main");
  git(host, folder, "worktree", "add", "-q", join(host.home, "parity-pr", "review-tree"), "work");
  const claude = join(host.home, "bin", "claude");
  const was = readlinkSync(claude);
  const reviewer = join(host.home, "parity-pr", "reviewer.cjs");
  writeFileSync(reviewer, REVIEWER, { mode: 0o755 });
  rmSync(claude);
  symlinkSync(reviewer, claude);
  try {
    const opened = await host.op("workspaces.review", { workspaceId, agent: "claude" });
    const review = opened.workspace?.id ?? opened.workspaceId;
    for (const until = Date.now() + 60_000; ; await new Promise(r => setTimeout(r, 300))) {
      const { review: draft } = await host.op("workspaces.reviewDraft", { workspaceId: review });
      if (draft !== undefined && draft !== null && typeof draft === "object" && Array.isArray(draft.comments)) break;
      if (Date.now() > until) throw new Error(`the reviewer's reply never read as a draft: ${JSON.stringify(draft)}`);
    }
    const id = opened.threadId ?? (await host.threads()).find(t => t.workspaceId === review)?.threadId;
    writeFileSync(made, JSON.stringify({ thread: id, workspace: review }));
    return shown(host, id);
  } finally {
    rmSync(claude);
    symlinkSync(was, claude);
  }
}

/** A project holding a committed 5,000-line file, every line of which the state rewrites uncommitted. */
async function large(host) {
  const folder = join(host.home, "parity-large");
  const lines = shift => `${Array.from({ length: 5000 }, (_, i) => `export const value${i + 1} = ${i + 1 + shift};`).join("\n")}\n`;
  if (!existsSync(join(folder, ".git"))) {
    mkdirSync(join(folder, "src"), { recursive: true });
    writeFileSync(join(folder, "src/large.ts"), lines(0));
    git(host, folder, "init", "-q", "-b", "main");
    git(host, folder, "add", "-A");
    git(host, folder, "commit", "-qm", "large");
    await host.wsp("add", folder, "--name", "large");
  }
  writeFileSync(join(folder, "src/large.ts"), lines(1));
  return host.thread("Parity state W2-PRP-901.", { project: "large", reply: { kind: "text", text: "Done." } });
}

export default [
  {
    id: "W2-PRP-044",
    clip: PANEL,
    data: host => prThread(host, "open"),
    go: (app, id) => prPane(app, id),
  },
  {
    id: "W2-PRP-045",
    clip: PANEL,
    data: host => prThread(host, "open"),
    go: async (app, id) => {
      await prPane(app, id);
      await app.click(css('[data-pr-tabs] [data-segment="files"]'));
      await has(app, "[data-pr-files]", 20_000);
      if (!(await app.page(() => document.querySelector('[data-pr-files] [data-changed-file="src/pane.rs"]') !== null))) {
        await app.page(() => [...document.querySelectorAll("[data-pr-files] button")].find(b => b.textContent.trim().startsWith("src"))?.click());
      }
      await app.click(css('[data-pr-files] [data-changed-file="src/pane.rs"]'));
      await app.waitFor(() => document.querySelector('[data-pr-file-diff="src/pane.rs"]') !== null && document.querySelector("[data-pr-skeleton=diff]") === null, 30_000);
      await app.sleep(1500);
    },
  },
  {
    id: "W2-PRP-046",
    clip: PANEL,
    data: host => reviewed(host),
    go: async (app, id) => {
      await prPane(app, id);
      // A page loaded before the review workspace was made hears of it without its review fact until a reload.
      if (!(await app.waitFor(() => document.querySelector("[data-pr-draft]") !== null, 10_000).then(() => true, () => false))) {
        await app.page(() => location.reload());
        await app.sleep(2000);
        await prPane(app, id);
      }
      await has(app, "[data-pr-draft]", 60_000);
      await app.page(() => document.querySelector("[data-pr-draft]").scrollIntoView({ block: "start" }));
      await app.sleep(800);
    },
  },
  { id: "W2-PRP-047-merged", clip: PANEL, data: host => prThread(host, "merged"), go: (app, id) => prPane(app, id) },
  { id: "W2-PRP-047-closed", clip: PANEL, data: host => prThread(host, "closed"), go: (app, id) => prPane(app, id) },
  {
    id: "W2-PRP-901",
    clip: PANEL,
    data: host => large(host),
    go: async (app, id) => {
      await pane(app, id, "diff", "Changes", "[data-diff-surface]");
      if (!(await app.page(() => document.querySelector('[data-diff-scope="head"]') !== null))) {
        await app.click(css('[aria-label^="Changes scope:"]'));
        const items = "[role=menuitem], [role=menuitemradio], [role=option]";
        await app.waitFor(s => [...document.querySelectorAll(s)].some(e => e.textContent.includes("Uncommitted")), 10_000, items);
        await app.page(s => [...document.querySelectorAll(s)].find(e => e.textContent.includes("Uncommitted")).click(), items);
        await has(app, '[data-diff-scope="head"]');
      }
      await has(app, "[data-changed-files]", 30_000);
      await app.sleep(2000);
    },
  },
];
