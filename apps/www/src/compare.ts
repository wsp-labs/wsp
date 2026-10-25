// SPDX-License-Identifier: AGPL-3.0-only
// What /compare and its page per tool say about every tool. Each cell holds the short answer the table shows, its mark,
// and the long sentence it sums with the pages that sentence was read from. A cell says only what its pages say on the
// day in READ; a fact read later is read again from the product's own page and the day moves with it. wsp's answers are
// read from the repo at WSP_AT, not from this site.

export const READ = "2026-10-14";
const WSP_AT = "https://github.com/wsp-labs/wsp/blob/522e3d0bb61c0eddb350dbce14a66a73a6fff2ff";
// The one wsp answer that lands with this page, so no commit before it holds the line.
const ONE_PERSON = "https://github.com/wsp-labs/wsp/blob/main/apps/docs-next/content/index.mdx#L59";

/** The words, then every page they were read from. */
export type Said = readonly [text: string, source: string, ...more: string[]];
/** What a table cell shows, at most six words, and whether it is a check or a dash, beside the record it sums. */
export type Cell = { short: string; good: boolean; says: Said };

const yes = (short: string, ...says: Said): Cell => ({ short, good: true, says });
const no = (short: string, ...says: Said): Cell => ({ short, good: false, says });

export const ROWS = [
  { key: "runs", label: "Runs agents on", about: "Where the agent's process runs." },
  { key: "second", label: "Another computer", about: "Whether threads can run on a second computer of yours." },
  { key: "has", label: "What that computer has when the agent starts", about: "What comes along onto that computer for the agent." },
  { key: "keys", label: "Where your keys and sign-ins live", about: "Who holds your API keys and agent logins." },
  { key: "worktrees", label: "Worktrees per thread", about: "Whether each thread gets a checkout of its own." },
  { key: "subthreads", label: "Agents start other agents", about: "Whether an agent can start threads you see and answer." },
  { key: "cloud", label: "Cloud machines on demand", about: "Whether it makes a cloud machine when you ask." },
  { key: "monitor", label: "Resource monitor", about: "Each thread's processes, memory and CPU." },
  { key: "panel", label: "A panel the agent builds", about: "A live panel beside the chat that the agent keeps current." },
  { key: "review", label: "Review and pull requests in the app", about: "Diffs, checks and the pull request beside the thread." },
  { key: "agents", label: "Which agents", about: "The coding agents it runs threads on." },
  { key: "platforms", label: "Platforms", about: "Where the app itself runs." },
  { key: "phone", label: "Phone app", about: "An app on your phone to read and answer threads." },
  { key: "teams", label: "Teams", about: "Plans for several people, with shared seats or SSO." },
  { key: "price", label: "Price", about: "What you pay to use it." },
  { key: "code", label: "Source code", about: "The license its code is under." },
] as const;

export type Row = (typeof ROWS)[number]["key"];
export type Answers = Record<Row, Cell>;
/** One sentence. A point on a row restates that row, so its pages are the row's two cells and its side holds the check. */
export type Point = { row: Row; text: string } | { said: Said };

export type Tool = {
  name: string;
  /** Its page, at /compare/<slug>. */
  slug: string;
  /** The card's line on /compare. */
  line: string;
  /** The sentence under the page's heading, which is also its description. */
  differs: string;
  /** Where it is stronger than wsp, after what AHEAD already says of it. */
  stronger: Point[];
  /** Where wsp is stronger than it. */
  weaker: Point[];
  pickThem: Point[];
  pickWsp: Point[];
  cells: Answers;
};
export type Kind = { name: string; tools: Tool[] };

/** What the vendors' clouds carry onto their machine, as /compare's intro and the landing page's table both say it. */
export const VENDORS_BRING = "your repo, a setup script, and on Claude and Cursor some skills you turn on";

export const WSP: Answers = {
  runs: yes("Computers you own", "Your computer, and Linux computers you own", `${WSP_AT}/skills/wsp/SKILL.md#L8`, `${WSP_AT}/AGENTS.md#L77`),
  second: yes("Yes, over ssh or dialing out", "Yes. Added over ssh, or it dials out to join", `${WSP_AT}/skills/wsp/SKILL.md#L72`, `${WSP_AT}/skills/wsp/SKILL.md#L162`),
  has: yes(
    "Your setup, agents signed in",
    "Your agents signed in, MCP servers, CLIs, skills, Claude Code plugins, GitHub sign-in, git and shell settings, your projects",
    `${WSP_AT}/apps/web/src/settings/add/addFlow.ts#L45-L52`,
  ),
  keys: yes(
    "Your computers, none on ours",
    "On your computers. wsp keeps none of your keys, and you sign in on each computer you add",
    `${WSP_AT}/apps/www/src/pages/legal.tsx#L33`,
    `${WSP_AT}/apps/www/src/pages/legal.tsx#L62`,
  ),
  worktrees: yes("On a branch", "On a branch you name: that thread runs in a worktree, and threads with no branch share the project folder", `${WSP_AT}/apps/docs-next/content/features/projects.mdx#L5`),
  subthreads: yes("Yes, any agent, any computer", "An agent in one thread can start other threads, on any agent and computer, and every one is in your sidebar", `${WSP_AT}/apps/docs-next/content/features/leads.mdx#L3`),
  cloud: yes("Sub-threads, your Boat account", "Sub-threads only, each on a Boat machine forked from your setup image, on your own Boat account", `${WSP_AT}/apps/docs-next/content/reference/contract.mdx#L124-L126`, `${WSP_AT}/packages/host/src/cloud.ts#L13-L15`),
  monitor: yes("Yes, Settings, Activity", "Every thread's processes on one computer, with their memory, CPU, power on a Mac, and ports", `${WSP_AT}/apps/docs-next/content/features/settings.mdx#L117-L119`),
  panel: yes("Yes, the slate", "A slate is a panel beside a thread that the agent builds and keeps current", `${WSP_AT}/apps/docs-next/content/features/slate.mdx#L5`),
  review: yes("PR pane; failed checks go back", "The Pull request pane shows checks, reviews and comments, and sends a failed check or a review comment back to the agent in one click", `${WSP_AT}/apps/docs-next/content/features/pull-requests.mdx#L5`),
  agents: yes("Claude Code, Codex, OpenCode, Cursor", "Claude Code, Codex, OpenCode, Cursor", `${WSP_AT}/packages/catalog/src/thread-agents.ts#L9`),
  platforms: no("macOS, Linux", "macOS, Linux", `${WSP_AT}/.github/workflows/release.yml#L3-L5`),
  phone: no("No app; a browser pairs", "No phone app. A phone's browser can pair with your host and open the app", `${WSP_AT}/apps/docs-next/content/features/devices.mdx#L5`),
  teams: no("No, one person", "Not yet. wsp is for one person and their computers", ONE_PERSON),
  price: yes("Free", "Free", `${WSP_AT}/apps/www/src/sections/close.tsx#L13`),
  code: yes("AGPL-3.0", "AGPL-3.0", `${WSP_AT}/LICENSE`),
};

const CONDUCTOR = "https://www.conductor.build";
const SUPERSET = "https://docs.superset.sh";
const ORCA = "https://www.onorca.dev";
const EMDASH = "https://emdash.com";
const T3 = "https://github.com/pingdotgg/t3code/blob/main/docs/user";
const SOLO = "https://soloterm.com";
const HERDR = "https://herdr.dev";
const CLAUDE = "https://code.claude.com/docs/en";
const CODEX = "https://learn.chatgpt.com/docs";
const CURSOR = "https://cursor.com/docs/cloud-agent";

export const KINDS: Kind[] = [
  {
    name: "Apps on the computer you sit at",
    tools: [
      {
        name: "Conductor",
        slug: "conductor",
        line: "A Mac app for agents side by side, with cloud sandboxes on Pro.",
        differs: "Conductor runs agents on your Mac or in its own cloud. wsp runs them on your computer and on Linux computers you own, each set up from yours.",
        stronger: [
          { said: ["Its cloud workspaces keep working after you close the app, with no computer of yours on.", `${CONDUCTOR}/docs/cloud/faq`] },
          { said: ["It has a hosted MCP server and routines that run agents on a schedule.", `${CONDUCTOR}/docs/api/mcp`, `${CONDUCTOR}/changelog/0.85.0-sections-routines-and-a-new-model-picker`] },
        ],
        weaker: [
          { row: "second", text: "wsp runs threads on a second computer of yours today; Conductor says not yet." },
          { row: "has", text: "wsp copies your agents, MCP servers, CLIs and skills onto each computer you add." },
          { row: "code", text: "wsp's code is AGPL-3.0; Conductor's is closed." },
        ],
        pickThem: [
          { said: ["You want agents in a hosted cloud that keeps working with your laptop closed.", `${CONDUCTOR}/docs/cloud/faq`] },
          { row: "phone", text: "You want to answer agents from an iPhone app." },
          { row: "teams", text: "Your team needs shared seats and SSO." },
        ],
        pickWsp: [
          { row: "second", text: "You have a second computer, a rented server or an old laptop, and want agents on it." },
          { row: "has", text: "You want your agents there signed in, with your MCP servers, CLIs and skills." },
          { row: "code", text: "You want the app's code under AGPL-3.0." },
        ],
        cells: {
          runs: yes("Your Mac, or its cloud", "Your Mac, or its cloud sandboxes on Pro", `${CONDUCTOR}/docs/cloud`),
          second: no('No, "not yet"', 'No. "Not yet"', `${CONDUCTOR}/docs/cloud/faq`),
          has: no(
            "Cloud: your keys, MCP servers",
            "Cloud only: API keys, env settings and MCP servers imported from your Mac; other tools from an install script you write",
            `${CONDUCTOR}/changelog/0.78.0-introducing-conductor-cloud`,
            `${CONDUCTOR}/changelog/0.83.0-auto-port-forwarding`,
            `${CONDUCTOR}/docs/cloud/cloud-computer`,
          ),
          keys: no("Your Mac; its cloud on Pro", "On your Mac. Cloud setup can import the API keys configured on your Mac", `${CONDUCTOR}/changelog/0.78.0-introducing-conductor-cloud`),
          worktrees: yes("One worktree per workspace", "Each workspace gets its own Git worktree with a branch checked out inside it", `${CONDUCTOR}/docs/concepts/git-worktrees`),
          subthreads: yes("Cloud workspaces only, via its CLI", "Agents in cloud workspaces create workspaces and start chats through its CLI; agents in local workspaces no longer do", `${CONDUCTOR}/changelog/0.80.0-stacks`, `${CONDUCTOR}/changelog/0.90.0-conductor-for-ios`),
          cloud: yes("Vercel sandbox, on Pro", "A Vercel sandbox per cloud workspace, 8 vCPUs and 16 GB of RAM, on Pro at $50/mo", `${CONDUCTOR}/docs/cloud/faq`, `${CONDUCTOR}/pricing`),
          monitor: no("Experimental; per chat not stated", "An experimental CPU and memory readout and a sidebar resource usage inspector; whether it splits by chat is not stated", `${CONDUCTOR}/changelog/0.37.1-polish-and-bug-fixes`, `${CONDUCTOR}/changelog/0.87.0-search-across-files-opencode-2-0`),
          panel: no("No; HTML file previews", "No panel the agent keeps; it previews HTML files", `${CONDUCTOR}/changelog/0.55.0-per-repo-spotlight-html-previews`),
          review: yes("Diff comments, PR page, checks", "Comments on the diff, and a PR page to view comments and checks, edit the title and merge", `${CONDUCTOR}/changelog/0.73.0-pr-page`, `${CONDUCTOR}/docs/reference/diff-viewer`),
          agents: yes("Claude Code, Codex, Cursor, OpenCode", "Claude Code, Codex, Cursor, OpenCode", `${CONDUCTOR}/docs/reference/harnesses`),
          platforms: no("macOS, iOS", "macOS, iOS", `${CONDUCTOR}/docs/installation`, `${CONDUCTOR}/changelog/0.90.0-conductor-for-ios`),
          phone: yes("iOS, cloud workspaces only", "Chat with agents and merge PRs from your phone, with cloud workspaces", `${CONDUCTOR}/changelog/0.90.0-conductor-for-ios`),
          teams: yes("Teams $60/user/mo; SSO on Enterprise", "Teams at $60 a user a month; SAML SSO and SCIM on Enterprise", `${CONDUCTOR}/pricing`),
          price: yes("Free; cloud $50/mo", "Free; cloud on Pro, $50/mo", `${CONDUCTOR}/pricing`),
          code: no("Closed", "Closed", CONDUCTOR),
        },
      },
      {
        name: "Superset",
        slug: "superset",
        line: "A desktop app that reaches your other computers through its relay.",
        differs: "Superset reaches your other computers and leaves each one for you to set up. wsp sets each computer up from yours when you add it.",
        stronger: [
          { said: ["Its automations run an agent session on a schedule.", `${SUPERSET}/automations`] },
        ],
        weaker: [
          { row: "has", text: "wsp copies your agents, MCP servers, CLIs and skills onto each computer; Superset leaves each host to you." },
          { row: "cloud", text: "wsp can start sub-threads on cloud machines today; Superset's cloud is in early access." },
          { row: "panel", text: "Each wsp thread has a slate, a live panel the agent builds beside the chat." },
          { row: "code", text: "wsp is open source under AGPL-3.0; Superset is source available under ELv2." },
        ],
        pickThem: [
          { row: "phone", text: "You want native iPhone and iPad apps." },
          { row: "teams", text: "Your team needs per-seat plans and SSO." },
          { said: ["You run agents beyond the four wsp knows.", `${SUPERSET}/agent-integration`, `${WSP_AT}/packages/catalog/src/thread-agents.ts#L9`] },
        ],
        pickWsp: [
          { row: "has", text: "You want each computer set up from yours, agents signed in, when you add it." },
          { row: "panel", text: "You want a live panel the agent keeps beside each thread." },
          { row: "code", text: "You want the code under AGPL-3.0." },
        ],
        cells: {
          runs: yes("Your computers, through its relay", "Your computer, and others you own through its relay", `${SUPERSET}/remote-access`),
          second: yes("Yes, on Pro", "Yes, on Pro", "https://superset.sh/pricing"),
          has: no("Nothing carried over", "Nothing carried. You install the agents and sign in on each host", `${SUPERSET}/cli/host-server`, `${SUPERSET}/usage`),
          keys: yes("On each host you sign in", "On each host: the host server reuses that computer's gh login and runs under your Superset session", `${SUPERSET}/cli/host-server`),
          worktrees: yes("Yes, every local workspace", "Local project workspaces use isolated git worktrees", `${SUPERSET}/workspaces`),
          subthreads: yes("Yes, CLI, MCP and skill", "One agent coordinates workers in separate workspaces, and sessions started outside the app appear in the sidebar", `${SUPERSET}/orchestration`, `${SUPERSET}/workspaces`),
          cloud: no("Early access, price not stated", "Cloud workspaces are in early access and not generally available, with no price on its pricing page", `${SUPERSET}/workspaces`, "https://superset.sh/pricing"),
          monitor: yes("Yes, CPU, memory, processes", "A process table for the app and every workspace, down to each terminal pane; remote hosts are not counted", `${SUPERSET}/usage`),
          panel: no("Published Pages, not a panel", "Pages lets agents present designs, reports and analysis to your team, published rather than beside the chat", `${SUPERSET}/pages`),
          review: yes("Diff, PR, checks, replies", "A pull request view with every CI check and its status, and Create PR from the diff viewer", `${SUPERSET}/pull-requests`, `${SUPERSET}/diff-viewer`),
          agents: yes("Any CLI agent, 22 listed", "Any CLI agent; 22 listed", `${SUPERSET}/agent-integration`),
          platforms: no("macOS, iOS; Linux experimental", "macOS, iPhone, iPad; Linux experimental", `${SUPERSET}/faq`, "https://superset.sh/mobile"),
          phone: yes("iPhone and iPad, on Pro", "Apps for iPhone and iPad on iOS 26 and up, included with Pro; Android has a waitlist", "https://superset.sh/mobile", `${SUPERSET}/faq`),
          teams: yes("Per seat; SSO on Enterprise", "Pro is $20 per user a month, billed per active seat; SAML SSO and SCIM on Enterprise", "https://superset.sh/pricing"),
          price: yes("Free; Pro $20/user/mo", "Free; Pro $20/user/mo", "https://superset.sh/pricing"),
          code: no("ELv2, source available", "ELv2, source available", "https://github.com/superset-sh/superset"),
        },
      },
      {
        name: "Orca",
        slug: "orca",
        line: "An open source app for 41 agents, on every desktop and phone.",
        differs: "Orca runs agents on ssh hosts and Orca servers you set up by hand. wsp sets each computer up from yours when you add it.",
        stronger: [
          { said: ["It shows pull requests from GitHub, GitLab, Bitbucket, Azure DevOps and Gitea.", `${ORCA}/docs/review/github`] },
        ],
        weaker: [
          { row: "has", text: "wsp copies your agents, sign-ins, MCP servers, CLIs and skills onto each computer; Orca leaves logins to each host." },
          { row: "cloud", text: "wsp can start sub-threads on cloud machines; Orca's cloud VMs are experimental and need a recipe in the repo." },
          { row: "panel", text: "Each wsp thread has a slate that stays beside the chat; Orca's visuals sit inline in it." },
        ],
        pickThem: [
          { said: ["You work on Windows.", `${ORCA}/docs/install`, `${WSP_AT}/.github/workflows/release.yml#L3-L5`] },
          { row: "phone", text: "You want iOS and Android apps." },
          { said: ["You run agents beyond the four wsp knows.", `${ORCA}/docs/agents/supported`, `${WSP_AT}/packages/catalog/src/thread-agents.ts#L9`] },
        ],
        pickWsp: [
          { row: "has", text: "You want each computer set up from yours when you add it, agents signed in." },
          { row: "panel", text: "You want a live panel the agent keeps beside each thread." },
        ],
        cells: {
          runs: yes("Your computer, ssh hosts, servers", "Your computer, ssh hosts, Orca servers, a cloud VM on your own account in its experimental mode", `${ORCA}/docs/ways-to-run`),
          second: yes("Yes, ssh or an Orca server", "Yes, over ssh or an Orca server", `${ORCA}/docs/ways-to-run`),
          has: no("Not carried; can install shared skills", 'Not carried: "A login on your laptop does not automatically carry over". Shared skills can be installed on a host', `${ORCA}/docs/remote-servers`, `${ORCA}/docs/cli/skills`),
          keys: yes("On each host you sign in", 'On each host: "Remote sessions use the server\'s PATH, home directory, and credentials", not the client\'s', `${ORCA}/docs/remote-servers`),
          worktrees: yes("Yes, one per task", "Every task gets its own copy of the repo via git worktree", `${ORCA}/docs/model/worktrees`),
          subthreads: yes("Yes; orchestration is experimental", "Its CLI records a new worktree as a child; orchestration is under Settings, Experimental", `${ORCA}/docs/cli/reference`, `${ORCA}/docs/cli/orchestration`),
          cloud: no("Your own provider, experimental", "Orca does not sell managed hosting; an experimental mode runs a VM on your own cloud account", `${ORCA}/docs/ways-to-run`),
          monitor: yes("Yes, its Resource Manager", "A Resource Manager in its settings shows CPU, memory and sessions", `${ORCA}/docs/settings`),
          panel: no("Inline chat visuals, experimental", "Agents draw charts, diagrams and mockups inline in the chat, not a panel beside it", `${ORCA}/changelog/1-4-224`),
          review: yes("PRs, checks, reviews, diff notes", "GitHub checks, reviews and comments open inline in a PR tab, and you can annotate an agent's diff", `${ORCA}/docs/review/github`, `${ORCA}/docs/review/annotate-ai-diff`),
          agents: yes("41 listed", "41 listed", `${ORCA}/docs/agents/supported`),
          platforms: yes("Desktop, web, iOS, Android", "macOS, Windows, Linux, iOS, Android, web", `${ORCA}/docs/install`, `${ORCA}/docs/mobile`, `${ORCA}/docs/android-apk`),
          phone: yes("iOS and Android apps", "An iOS and Android app that pairs with your desktop Orca", `${ORCA}/docs/mobile`),
          teams: no("Enterprise by contact only", "An enterprise page with a contact form; no seats, SSO or price stated", `${ORCA}/enterprise`),
          price: yes("Free", "Free", ORCA),
          code: yes("MIT", "MIT", "https://github.com/stablyai/orca"),
        },
      },
      {
        name: "Emdash",
        slug: "emdash",
        line: "An open source app for 35 agents, with remote projects over ssh.",
        differs: "Emdash opens projects on ssh hosts you set up yourself. wsp sets each computer up from yours when you add it.",
        stronger: [
          { said: ["It turns issues from Linear, GitHub, Jira and seven more trackers into tasks.", "https://github.com/generalaction/emdash"] },
          { said: ["Its automations create tasks on a schedule.", `${EMDASH}/docs/automations`] },
        ],
        weaker: [
          { row: "subthreads", text: "In wsp an agent starts other threads you see in the sidebar; Emdash keeps subagents inside one chat." },
          { row: "has", text: "wsp copies your setup onto each computer you add; Emdash can install an agent and leaves the rest to you." },
          { row: "monitor", text: "wsp's Activity page splits processes, memory and CPU by thread; Emdash shows them per computer." },
          { row: "panel", text: "Each wsp thread has a slate, a live panel the agent builds beside the chat." },
        ],
        pickThem: [
          { said: ["You work on Windows.", `${EMDASH}/docs/installation`, `${WSP_AT}/.github/workflows/release.yml#L3-L5`] },
          { said: ["You want tasks made from your issue tracker.", "https://github.com/generalaction/emdash"] },
          { said: ["You run agents beyond the four wsp knows.", `${EMDASH}/docs/providers`, `${WSP_AT}/packages/catalog/src/thread-agents.ts#L9`] },
        ],
        pickWsp: [
          { row: "subthreads", text: "You want agents to hand work to each other as threads you can read and answer." },
          { row: "has", text: "You want each computer set up from yours when you add it." },
        ],
        cells: {
          runs: yes("Your computer, ssh hosts", "Your computer, ssh hosts", `${EMDASH}/docs/remote-development`),
          second: yes("Yes, over ssh", "Yes, over ssh, since v0.4.9 on 2026-02-12", "https://github.com/generalaction/emdash/releases/tag/v0.4.9"),
          has: no("Not carried; can install agents", "Not carried. It can run an agent's installer on the host", `${EMDASH}/docs/remote-development/remote-projects`),
          keys: yes("On each host you set up", "On each ssh host: it uses the Git credentials configured on the remote host", `${EMDASH}/docs/remote-development/remote-projects`),
          worktrees: yes("Yes, each task by default", "Each task gets its own git branch and worktree", `${EMDASH}/docs/tasks`),
          subthreads: no("No; subagents inside one chat", "No page lets an agent start a task; it shows an agent's own subagents inside its chat", `${EMDASH}/changelog`),
          cloud: no("Emdash Cloud, by contact", "Emdash Cloud offers compute for every task through a contact form, with no price stated", `${EMDASH}/cloud`),
          monitor: no("Per computer, not per task", "Resource usage per computer since v1.2.0, not per task", `${EMDASH}/changelog`),
          panel: no("No; browser preview only", "An in-app browser previews a local dev server", `${EMDASH}/docs/in-app-browser`),
          review: yes("Diff, PR, checks, merge", "The diff view shows PR files, commits, checks, comments and merge state", `${EMDASH}/docs/diff-view`, `${EMDASH}/docs/ci-checks`),
          agents: yes("35 listed", "35 listed", `${EMDASH}/docs/providers`),
          platforms: yes("macOS, Windows, Linux", "macOS, Windows, Linux", `${EMDASH}/docs/installation`),
          phone: no("Not stated", "No phone app on its site, downloads, docs or changelog", EMDASH, `${EMDASH}/download`, `${EMDASH}/docs/installation`),
          teams: no("Enterprise by contact only", "Reach out to bring Emdash to your team; a project config can be shared in the repo", `${EMDASH}/enterprise`, `${EMDASH}/docs/project-config`),
          price: yes("Free", "Free", EMDASH),
          code: yes("Apache-2.0", "Apache-2.0", "https://github.com/generalaction/emdash"),
        },
      },
      {
        name: "T3 Code",
        slug: "t3-code",
        line: "An open source app that spreads threads across your computers.",
        differs: "T3 Code reaches your other computers and leaves each one's setup to you. wsp sets each computer up from yours when you add it.",
        stronger: [
          { said: ["Its Device panel shows a live iOS Simulator or Android Emulator the agent works in.", `${T3}/devices.md`] },
        ],
        weaker: [
          { row: "has", text: "wsp copies your agents, MCP servers, CLIs and skills onto each computer; T3 Code leaves each one's setup to you." },
          { row: "monitor", text: "wsp's Activity page shows each thread's processes, memory and CPU; T3 Code's view covers the whole app." },
          { row: "cloud", text: "wsp can start sub-threads on cloud machines; T3 Code runs only on your own computers." },
          { row: "panel", text: "A wsp slate stays beside the thread and keeps updating; T3 Code's HTML pages sit inline in the chat." },
        ],
        pickThem: [
          { row: "phone", text: "You want native iOS and Android apps." },
          { said: ["You work on Windows or in WSL.", `${T3}/install.md`, `${WSP_AT}/.github/workflows/release.yml#L3-L5`] },
          { said: ["You want the agent to work in an iOS Simulator or Android Emulator.", `${T3}/devices.md`] },
        ],
        pickWsp: [
          { row: "has", text: "You want each computer set up from yours when you add it." },
          { row: "monitor", text: "You want to see which thread is using the memory or CPU." },
        ],
        cells: {
          runs: yes("Your computer, ssh, its relay", "Your computer, ssh hosts, any computer through its relay", `${T3}/remote-access.md`),
          second: yes("Yes, threads spread across them", "Yes, and it can spread new threads across them", `${T3}/remote-access.md`),
          has: no(
            "Not carried; installs some agents",
            'Not carried: "Installation, login, and configuration belong to that environment\'s machine". It installs Codex and ACP agents itself',
            `${T3}/install.md`,
            `${T3}/welcome-wizard.md`,
          ),
          keys: yes("On each computer it runs on", 'On each computer: "Installation, login, and configuration belong to that environment\'s machine"', `${T3}/install.md`),
          worktrees: yes("Optional new worktree per thread", "Choose New worktree when a thread's changes need a separate branch", `${T3}/thread-sidebar.md`),
          subthreads: yes("Yes, delegated child threads", "Its delegate tool creates durable child threads, and the sidebar follows work delegated to subagents", `${T3}/providers-pi.md`, `${T3}/thread-sidebar.md`),
          cloud: no("No, your own computers", "T3 Code runs coding agents on your computer", `${T3}/install.md`),
          monitor: no("App-wide, not per thread", "A resource telemetry view in its settings covers the app, not each thread", "https://github.com/pingdotgg/t3code/blob/main/apps/web/src/components/settings/ResourceTelemetryDiagnostics.tsx"),
          panel: no("Inline HTML pages in thread", "The agent builds a self-contained HTML page that appears in the thread above its reply", `${T3}/html-renders.md`),
          review: yes("PRs on seven forges", "Review changes and comments, request reviewers, check out a branch or merge, and the agent wakes on a failed check", `${T3}/source-control.md`),
          agents: yes("8 named, any ACP agent", "8 named, any ACP agent", "https://t3.codes", `${T3}/providers-acp.md`),
          platforms: yes("Desktop, web, iOS, Android", "macOS, Windows, Linux, web, iOS, Android", "https://t3.codes/download"),
          phone: yes("iOS and Android apps", "Apps for iPhone and iPad on the App Store, and Android on Google Play", "https://t3.codes/download"),
          teams: no("Not stated", "No team plan on its site, downloads, terms, docs or README", "https://t3.codes", "https://t3.codes/download", "https://t3.codes/terms-of-service"),
          price: yes("Free", "Free", "https://t3.codes"),
          code: yes("MIT", "MIT", "https://github.com/pingdotgg/t3code"),
        },
      },
      {
        name: "Solo",
        slug: "solo",
        line: "An app for the agents and dev processes on one computer.",
        differs: "Solo runs agents on the computer you sit at and no other. wsp runs them there and on Linux computers you own.",
        stronger: [
          { said: ["It runs your dev stack from a solo.yml beside the agents, and restarts commands that crash.", SOLO, `${SOLO}/docs/commands/auto-restart`] },
          { said: ["Agents share todos, scratchpads, locks and state through it.", SOLO] },
          { said: ["Its activity monitor can kill any process it tracks.", `${SOLO}/docs/activity/activity-monitor`] },
        ],
        weaker: [
          { row: "second", text: "wsp runs threads on other computers you own; Solo stays on the one you sit at." },
          { row: "worktrees", text: "wsp gives a thread its own worktree on a branch you name; Solo is not a worktree tool." },
          { row: "review", text: "wsp shows a thread's pull request and sends a failed check back to the agent; Solo does not review diffs." },
          { row: "code", text: "wsp's code is AGPL-3.0; Solo's is closed." },
        ],
        pickThem: [
          { said: ["You want your dev servers and agents run side by side on one computer.", SOLO] },
          { said: ["You work on Windows.", `${SOLO}/docs/getting-started/installation`, `${WSP_AT}/.github/workflows/release.yml#L3-L5`] },
        ],
        pickWsp: [
          { row: "second", text: "You want agents on a second computer too." },
          { row: "review", text: "You want pull requests and checks beside each thread." },
        ],
        cells: {
          runs: yes("The computer you sit at", "The computer you sit at", SOLO),
          second: no("No", "No", SOLO),
          has: no("One computer, nothing to carry", "One computer, nothing to carry", SOLO),
          keys: yes("Your computer, never through Solo", 'On your computer: "your API keys and provider connection never pass through Solo"', SOLO),
          worktrees: no("No, not a worktree tool", "\"Solo is not a git-worktree orchestrator\"", `${SOLO}/solo-vs-conductor`),
          subthreads: yes("Yes, across agents, in sidebar", "A Claude session can ask Solo's MCP server to start a Codex worker, and workers appear in the project's Agents section", `${SOLO}/docs/workflows/agents-spawning-agents`, `${SOLO}/docs/workflows/agent-orchestration`),
          cloud: no("No", "\"Solo does not currently provide remote-host or headless session management\"", SOLO),
          monitor: yes("Yes, per process tree", "Its activity monitor tracks commands, terminals, agents and their subprocesses across projects", `${SOLO}/docs/activity/activity-monitor`, `${SOLO}/docs/sidebar/cpu-memory-stats`),
          panel: no("No; Markdown scratchpads", "Project-scoped Markdown notes, not a panel the agent builds", `${SOLO}/docs/scratchpads/using-scratchpads`),
          review: no("No", "\"Solo does not review diffs or manage branches\"", `${SOLO}/solo-vs-conductor`),
          agents: yes("8 built in, any CLI", "8 built in, any CLI", `${SOLO}/docs/agents/setting-up-tools`),
          platforms: no("macOS, Windows", "macOS, Windows", `${SOLO}/docs/getting-started/installation`),
          phone: no("Not stated", "No phone app on its site, downloads, docs or changelog", SOLO, `${SOLO}/download`),
          teams: yes("Team seats, no SSO", "Seats from $99 a year each, less in bulk; team licenses do not include SSO", SOLO, `${SOLO}/docs/account/team-licenses`),
          price: yes("Free for 4 projects, then $99/yr", "Free for 4 projects; Pro $99/yr", SOLO),
          code: no("Closed", "Closed", `${SOLO}/terms-of-service`),
        },
      },
    ],
  },
  {
    name: "A runtime over ssh",
    tools: [
      {
        name: "herdr",
        slug: "herdr",
        line: "An open source terminal runtime for agents, on your computer and over ssh.",
        differs: "herdr runs agents on ssh machines and copies nothing of yours onto them. wsp copies your setup onto each computer you add.",
        stronger: [
          { said: ["It runs inside the terminal you already use.", HERDR] },
        ],
        weaker: [
          { row: "has", text: "wsp copies your agents, MCP servers, CLIs and skills onto each computer; herdr copies nothing of yours to ssh hosts." },
          { row: "review", text: "wsp shows a thread's pull request and checks beside it; herdr pairs with other apps for review." },
          { row: "panel", text: "Each wsp thread has a slate, a live panel the agent builds beside the chat." },
        ],
        pickThem: [
          { said: ["You live in a terminal and want agents in panes there.", HERDR] },
          { said: ["You want to reach agents from your phone over plain ssh.", `${HERDR}/docs/how-to-work/`] },
          { said: ["You work on Windows.", `${HERDR}/docs/install/`, `${WSP_AT}/.github/workflows/release.yml#L3-L5`] },
        ],
        pickWsp: [
          { row: "has", text: "You want each computer set up from yours when you add it." },
          { row: "review", text: "You want pull requests and checks in the app." },
        ],
        cells: {
          runs: yes("Your computer, saved ssh machines", "Your computer, and saved ssh machines", HERDR, "https://github.com/herdrdev/herdr/releases/tag/v0.9.0"),
          second: yes("Yes, over ssh", "Yes, over ssh, since 2026-09-07", "https://github.com/herdrdev/herdr/releases/tag/v0.9.0"),
          has: no("Only herdr itself", 'Only herdr. It "does not copy local command plugins, configuration, executables, or secrets"', `${HERDR}/docs/connecting-machines/`),
          keys: yes("On each machine, none copied", 'On each machine: herdr "does not store passwords, private keys, agent tickets" and copies no secrets to ssh hosts', `${HERDR}/docs/connecting-machines/`),
          worktrees: yes("Yes, on demand per workspace", "worktree create makes a Git worktree checkout and opens it as a workspace", `${HERDR}/docs/cli-reference/`),
          subthreads: yes("Yes, in visible panes", "Agents split panes, start each other and prompt each other", HERDR, `${HERDR}/docs/agent-automation/`),
          cloud: no("No; Herdr Cloud waitlist", "\"You bring the machines. We connect them.\" Herdr Cloud has a waitlist", `${HERDR}/cloud/`),
          monitor: no("Not stated", "No resource view in its docs, home page or recent release notes", `${HERDR}/llms-full.txt`, HERDR),
          panel: no("No", "Native non-terminal plugin UI is not part of plugin v1", `${HERDR}/docs/plugins/`),
          review: no("No; pairs with other apps", "For worktree and diff review it pairs with other apps", `${HERDR}/compare/`),
          agents: yes("22 detected, any terminal program", "22 detected, any terminal program", HERDR, `${HERDR}/docs/agents/`),
          platforms: yes("macOS, Linux, Windows", "macOS, Linux, Windows", `${HERDR}/docs/install/`),
          phone: no("No app; any ssh client", "\"Herdr works on your phone without a mobile app or web dashboard\"", `${HERDR}/docs/how-to-work/`),
          teams: no("Not stated", "No team plan; its README lists an enterprise email only", "https://github.com/herdrdev/herdr"),
          price: yes("Free", "Free: \"The runtime, what you use right now, stays free\"", `${HERDR}/blog/herdr-is-joining-y-combinator/`),
          code: yes("Apache-2.0", "Apache-2.0", "https://github.com/herdrdev/herdr"),
        },
      },
    ],
  },
  {
    name: "The vendors' clouds",
    tools: [
      {
        name: "Claude Code on the web",
        slug: "claude-code-web",
        line: "Claude Code on Anthropic's VM, from the web, desktop or phone.",
        differs: "Claude Code on the web runs on Anthropic's VM with your repo. wsp runs it on computers you own, with your setup on them.",
        stronger: [
          { said: ["Your GitHub credentials never enter the session's VM.", `${CLAUDE}/claude-code-on-the-web`] },
          { said: ["It can watch a pull request and answer CI failures and review comments by itself.", `${CLAUDE}/claude-code-on-the-web`] },
        ],
        weaker: [
          { row: "has", text: "wsp brings your plugins and local MCP servers; Claude's cloud VM loads neither." },
          { row: "agents", text: "wsp runs Codex, OpenCode and Cursor beside Claude Code." },
          { row: "second", text: "wsp runs threads on any Linux computer you own; Claude's own runners need Team or Enterprise." },
          { row: "panel", text: "Each wsp thread has a slate; Claude's artifacts do not reach cloud sessions." },
        ],
        pickThem: [
          { said: ["You want no computer of your own to keep on.", `${CLAUDE}/claude-code-on-the-web`] },
          { row: "phone", text: "You want to start and answer sessions from the Claude phone app." },
          { row: "teams", text: "Your team is already on a Claude Team plan." },
        ],
        pickWsp: [
          { row: "has", text: "You want Claude Code with your plugins and local MCP servers." },
          { row: "agents", text: "You want Codex or OpenCode in the same window." },
        ],
        cells: {
          runs: no("Anthropic's VM; runners in beta", "Anthropic's VM; your own runners on Team and Enterprise, in beta", `${CLAUDE}/claude-code-on-the-web`, `${CLAUDE}/self-hosted-environments`),
          second: no("Runners, on Team and Enterprise", "Only through runners, on Team and Enterprise", `${CLAUDE}/self-hosted-environments`),
          has: no("Committed repo, skills you turn on", "What the repo has committed, and skills you turn on at claude.ai. Not your ~/.claude, plugins or local MCP servers", `${CLAUDE}/cloud-environments`),
          keys: no("Anthropic's servers", "Environment variables and network secrets are saved on the cloud environment, and a proxy holds your GitHub token", `${CLAUDE}/cloud-environments`),
          worktrees: yes("Own VM and branch each", "Each task gets its own session and its own branch", `${CLAUDE}/web-quickstart`),
          subthreads: yes("Projects start sessions, in beta", "Projects start separate sessions, in public beta on Pro and Max", `${CLAUDE}/claude-projects`),
          cloud: yes("Yes, in Pro and up", "No separate charge for the cloud VM: 4 vCPUs, 16 GB of RAM, 30 GB of disk", `${CLAUDE}/claude-code-on-the-web`, `${CLAUDE}/cloud-environments`),
          monitor: no("Not stated", "No resource view in its cloud, environment or projects pages", `${CLAUDE}/claude-code-on-the-web`, `${CLAUDE}/cloud-environments`, `${CLAUDE}/claude-projects`),
          panel: no("Not in cloud sessions", "Artifacts work in the CLI, desktop and remote-control sessions, not in cloud sessions", `${CLAUDE}/artifacts`),
          review: yes("Diff comments, PRs, CI fixes", "Inline comments on the diff, pull requests, and replies to CI failures and review comments; a review bot on Team and Enterprise", `${CLAUDE}/claude-code-on-the-web`, `${CLAUDE}/code-review`),
          agents: no("Claude Code", "Claude Code", `${CLAUDE}/claude-code-on-the-web`),
          platforms: yes("Web, desktop, CLI, phones, Slack", "Web, desktop, CLI, iOS, Android; Slack on Pro and Max", `${CLAUDE}/claude-code-on-the-web`, `${CLAUDE}/slack`),
          phone: yes("iOS and Android, Claude app", "The Claude app for iOS and Android is a client for Claude Code sessions", `${CLAUDE}/mobile`),
          teams: yes("Team from $25/seat, SSO", "Team at $25 a seat a month billed monthly, with single sign-on", "https://claude.com/pricing"),
          price: no("In Pro, $20/mo, and up", "In Pro, $20/mo, and up", "https://claude.com/pricing"),
          code: no("Closed", "Closed", "https://github.com/anthropics/claude-code/blob/main/LICENSE.md"),
        },
      },
      {
        name: "Codex cloud",
        slug: "codex-cloud",
        line: "Codex on OpenAI's VM, from the web, desktop, IDE or iOS.",
        differs: "Codex cloud runs on OpenAI's VM with your repo. wsp runs Codex on computers you own, with your setup on them.",
        stronger: [
          { said: ["Codex works out missing setup values and asks only for the ones it cannot infer.", `${CODEX}/environments/cloud-environments`] },
          { said: ["Programs in the VM get a placeholder, and a proxy swaps in the real secret.", `${CODEX}/environments/cloud-environments`] },
        ],
        weaker: [
          { row: "has", text: "wsp brings your personal skills and MCP servers; Codex cloud does not sync personal skills." },
          { row: "subthreads", text: "In wsp a thread starts other threads you read and answer; Codex keeps subagents inside one task." },
          { row: "agents", text: "wsp runs Claude Code, OpenCode and Cursor beside Codex." },
          { row: "second", text: "wsp runs threads on computers you own; Codex cloud runs only on OpenAI's VM." },
        ],
        pickThem: [
          { said: ["You want tasks to keep running while your computer is asleep.", `${CODEX}/environments/cloud-environments`] },
          { row: "phone", text: "You want Codex from the ChatGPT app on iOS." },
          { row: "teams", text: "Your team is on ChatGPT Business with SSO." },
        ],
        pickWsp: [
          { row: "has", text: "You want Codex with your own skills and MCP servers." },
          { row: "subthreads", text: "You want a Codex thread to start a Claude Code thread, or the other way round." },
        ],
        cells: {
          runs: no("OpenAI's VM", "OpenAI's VM", `${CODEX}/cloud`),
          second: no("No", "No", `${CODEX}/environments/cloud-environments`),
          has: no("Repo skills and AGENTS.md", 'Repo skills and AGENTS.md. "Personal skills from your local computer aren\'t synced"', `${CODEX}/environments/cloud-environments`),
          keys: no("OpenAI's servers", "Environment variables and secrets saved on the environment or in your personal vault", `${CODEX}/environments/cloud-environments`),
          worktrees: yes("Own workspace per task", "Each new task gets its own isolated workspace", `${CODEX}/environments/cloud-environments`),
          subthreads: no("Subagents inside one chat", "Subagents show as read-only Active and Done lists inside one task", `${CODEX}/agent-configuration/subagents`),
          cloud: yes("Yes, in Plus and up", "A VM per task: 2 vCPUs and 8 GiB on Plus, up to 4 vCPUs and 16 GiB on Pro", `${CODEX}/environments/cloud-environments`, `${CODEX}/pricing`),
          monitor: no("Not stated", "Only VM sizes are documented", `${CODEX}/environments/cloud-environments`),
          panel: no("Not stated for cloud tasks", "Its artifacts viewer is in the ChatGPT desktop app, not cloud tasks", `${CODEX}/artifacts-viewer`),
          review: yes("Diffs, PRs, review bot", "Commit or open a pull request from a task, and Codex posts reviews on pull requests", `${CODEX}/cloud`, `${CODEX}/third-party/github`),
          agents: no("Codex", "Codex", `${CODEX}/models`),
          platforms: yes("Web, desktop, CLI, IDE, iOS", "Web, desktop, CLI, IDE, iOS", `${CODEX}/environments/cloud-environments`),
          phone: yes("iOS, in the ChatGPT app", "In the ChatGPT mobile app for iOS, open Codex", `${CODEX}/mobile`),
          teams: yes("Business $25/user/mo, SSO", "Business at $25 per user a month billed monthly, with SAML SSO", `${CODEX}/pricing`),
          price: no("In Plus, $20/mo, and up", "In Plus, $20/mo, and up", `${CODEX}/pricing`),
          code: no("Closed; CLI Apache-2.0", "Closed; CLI Apache-2.0", `${CODEX}/open-source`),
        },
      },
      {
        name: "Cursor cloud agents",
        slug: "cursor-cloud-agents",
        line: "Cursor's agent on its VM, or on your own machine with My Machines.",
        differs: "Cursor cloud agents run on Cursor's VM, set up from its dashboard. wsp runs agents on computers you own, set up from yours.",
        stronger: [
          { said: ["Each cloud agent gets a full desktop it controls with a mouse and keyboard.", `${CURSOR}/capabilities`] },
          { said: ["You can run as many agents in parallel as you want, with your computer off.", CURSOR] },
        ],
        weaker: [
          { row: "keys", text: "wsp keeps your keys on your computers; Cursor holds secrets in its dashboard." },
          { row: "has", text: "wsp copies your MCP servers and skills from the computer you sit at, not a dashboard." },
          { row: "agents", text: "wsp runs Claude Code, Codex and OpenCode beside Cursor." },
          { row: "panel", text: "Each wsp thread has a slate; Cursor names its canvases for the desktop app, not cloud agents." },
        ],
        pickThem: [
          { said: ["You want many agents in parallel on machines you never run.", CURSOR] },
          { row: "phone", text: "You want a native iOS app." },
          { row: "teams", text: "Your team is on Cursor Teams with SSO." },
        ],
        pickWsp: [
          { row: "keys", text: "You want your keys to stay on computers you own." },
          { row: "agents", text: "You want Claude Code and Codex beside Cursor." },
        ],
        cells: {
          runs: yes("Cursor's VM, or My Machines", "Cursor's VM, or your own machine with My Machines", CURSOR, `${CURSOR}/self-hosted/my-machines`),
          second: yes("Yes, My Machines on paid plans", "Yes, My Machines on paid plans", `${CURSOR}/self-hosted`),
          has: no("Rules, synced skills, dashboard MCP servers", "Your rules, skills if you turn on sync, MCP servers set in its dashboard", `${CURSOR}/best-practices`, "https://cursor.com/docs/skills", `${CURSOR}/capabilities`),
          keys: no("Cursor's servers", "Secrets added in its dashboard, scoped to the workspace or team and injected when an agent starts", CURSOR),
          worktrees: yes("Own VM and branch each", "Each agent works on a separate branch inside its own VM", CURSOR, `${CURSOR}/security`),
          subthreads: yes("Projects start agents", "Projects create and manage agents on your behalf, on all paid plans", "https://cursor.com/docs/agent/projects"),
          cloud: yes("Yes, at API price", "Cloud agents bill at API pricing for the model, as many in parallel as you want", CURSOR),
          monitor: no("Not stated", "Only detected ports are documented", `${CURSOR}/setup`),
          panel: no("Canvases, desktop only", "Canvases render next to the chat in the desktop app; nothing says cloud agents get them", "https://cursor.com/docs/agent/tools/canvas"),
          review: yes("Diffs, PRs, Bugbot, CI fixes", "Review and commit changes and manage PRs; cloud agents try to fix CI failures on Teams", "https://cursor.com/docs/agent/agents-window", `${CURSOR}/capabilities`),
          agents: no("Cursor's agent, many models", "Cursor's agent, many models", CURSOR),
          platforms: yes("Desktop, web, iOS, Slack", "Desktop, web, iOS, Slack", CURSOR, `${CURSOR}/mobile`),
          phone: yes("iOS app; Android web app", "Cursor for iOS is a native app; Android runs it as a web app", `${CURSOR}/mobile`, CURSOR),
          teams: yes("Teams plans, SSO", "Teams plans with SAML 2.0 SSO at no extra cost; its price lists disagree on the seat price", "https://cursor.com/pricing", "https://cursor.com/docs/account/teams/sso"),
          price: no(
            "Not stated",
            "Its two price lists disagree on the day: Premium $30/mo on its pricing page, Pro $20/mo in its help. Cloud agents bill models at API price",
            "https://cursor.com/pricing",
            "https://cursor.com/help/account-and-billing/pricing",
            CURSOR,
          ),
          code: no("Not stated", "Not stated", "https://cursor.com/cli"),
        },
      },
    ],
  },
];

export const TOOLS: Tool[] = KINDS.flatMap(k => k.tools);

/** "Switch in three steps", the same on every page. The second is the one a person with no other computer skips. */
export const STEPS: { title: string; said: Said }[] = [
  {
    title: "Install wsp",
    said: ["Run the line in a terminal on a Mac or a Linux computer, or download the app.", `${WSP_AT}/apps/docs-next/content/install/mac.mdx#L13`, `${WSP_AT}/apps/docs-next/content/install/linux.mdx#L13`, `${WSP_AT}/apps/docs-next/content/install/mac.mdx#L24`],
  },
  {
    title: "Add a computer, if you have one",
    said: [
      "Point wsp at a Linux computer you reach over SSH and pick what it copies there. With no other computer, threads run on the one you sit at.",
      `${WSP_AT}/apps/docs-next/content/start/first-computer.mdx#L5-L9`,
      `${WSP_AT}/apps/docs-next/content/start/first-thread.mdx#L19`,
    ],
  },
  {
    title: "Start your first thread",
    said: [
      "Add a project, open a new thread and type what to build. `wsp conversations` lists the Claude Code and Codex sessions you already had, and `wsp run --resume` opens one.",
      `${WSP_AT}/apps/docs-next/content/start/first-thread.mdx#L14-L27`,
      `${WSP_AT}/apps/docs-next/content/agents/skill.mdx#L116-L117`,
    ],
  },
];

/** Where the others are ahead: what wsp lacks, then each tool that has it, by slug, with the pages that say so. */
export const AHEAD: { title: string; row?: Row; wsp: Said; them: Partial<Record<string, Said>> }[] = [
  {
    title: "Windows",
    wsp: ["wsp runs on macOS and Linux.", `${WSP_AT}/.github/workflows/release.yml#L3-L5`, `${WSP_AT}/apps/www/src/sections/close.tsx#L11`],
    them: {
      "t3-code": ["T3 Code runs on Windows.", "https://t3.codes/download"],
      herdr: ["herdr runs on Windows.", `${HERDR}/docs/install/`],
      orca: ["Orca runs on Windows.", `${ORCA}/docs/install`],
      emdash: ["Emdash runs on Windows.", `${EMDASH}/docs/installation`],
      solo: ["Solo runs on Windows.", `${SOLO}/docs/getting-started/installation`],
    },
  },
  {
    title: "Your phone",
    row: "phone",
    wsp: ["wsp has no phone app; a phone's browser pairs with your host instead.", `${WSP_AT}/apps/docs-next/content/features/devices.mdx#L5`],
    them: {
      "t3-code": ["T3 Code has apps for iOS and Android.", "https://t3.codes/download"],
      orca: ["Orca has apps for iOS and Android.", `${ORCA}/docs/mobile`, `${ORCA}/docs/android-apk`],
      superset: ["Superset has apps for iPhone and iPad.", "https://superset.sh/mobile"],
      conductor: ["Conductor has an iOS app.", `${CONDUCTOR}/changelog/0.90.0-conductor-for-ios`],
      "claude-code-web": ["Claude Code on the web runs from iOS and Android.", `${CLAUDE}/claude-code-on-the-web`],
      "codex-cloud": ["Codex cloud runs from iOS.", `${CODEX}/environments/cloud-environments`],
      "cursor-cloud-agents": ["Cursor cloud agents run from iOS.", `${CURSOR}/mobile`],
    },
  },
  {
    title: "No computer of your own",
    wsp: ["wsp runs every thread you start on a computer you own; only sub-threads can go to Boat.", `${WSP_AT}/skills/wsp/SKILL.md#L8`, `${WSP_AT}/apps/docs-next/content/reference/contract.mdx#L124-L126`],
    them: {
      "claude-code-web": ["Claude Code on the web runs agents on Anthropic's machines, in the plan you pay for.", `${CLAUDE}/claude-code-on-the-web`],
      "codex-cloud": ["Codex cloud runs agents on OpenAI's machines, in the plan you pay for.", `${CODEX}/cloud`],
      "cursor-cloud-agents": ["Cursor runs agents on its own machines, in the plan you pay for.", CURSOR],
    },
  },
  {
    title: "More agents",
    wsp: ["wsp runs threads on four.", `${WSP_AT}/packages/catalog/src/thread-agents.ts#L9`],
    them: {
      orca: ["Orca lists 41 agents.", `${ORCA}/docs/agents/supported`],
      emdash: ["Emdash lists 35 agents.", `${EMDASH}/docs/providers`],
      superset: ["Superset works with any CLI agent and lists 22.", `${SUPERSET}/agent-integration`],
      "t3-code": ["T3 Code names 8 agents and runs any ACP agent.", "https://t3.codes", `${T3}/providers-acp.md`],
      solo: ["Solo has 8 agents built in and runs any other CLI.", `${SOLO}/docs/agents/setting-up-tools`],
      herdr: ["herdr detects 22 agents and runs any terminal program.", HERDR, `${HERDR}/docs/agents/`],
    },
  },
  {
    title: "Teammates",
    row: "teams",
    wsp: ["wsp is for one person.", ONE_PERSON],
    them: {
      conductor: ["Conductor sells plans for teams.", `${CONDUCTOR}/pricing`],
      superset: ["Superset sells plans for teams.", "https://superset.sh/pricing"],
      "claude-code-web": ["Anthropic sells Team and Enterprise plans.", "https://claude.com/pricing"],
      "codex-cloud": ["OpenAI sells plans for teams.", `${CODEX}/pricing`],
      "cursor-cloud-agents": ["Cursor sells plans for teams.", "https://cursor.com/pricing"],
    },
  },
  {
    title: "Size",
    wsp: ["wsp has 9; it started in September 2026.", "https://github.com/wsp-labs/wsp", `https://github.com/wsp-labs/wsp/commit/2e2674411d0226cb831df1135790b1a4db1bdbde`],
    them: {
      orca: ["Orca has 91,603 GitHub stars.", "https://github.com/stablyai/orca", "https://github.com/wsp-labs/wsp"],
      herdr: ["herdr has 43,825 GitHub stars.", "https://github.com/herdrdev/herdr", "https://github.com/wsp-labs/wsp"],
      "t3-code": ["T3 Code has 27,407 GitHub stars.", "https://github.com/pingdotgg/t3code", "https://github.com/wsp-labs/wsp"],
      superset: ["Superset has 15,144 GitHub stars.", "https://github.com/superset-sh/superset", "https://github.com/wsp-labs/wsp"],
      emdash: ["Emdash has 5,978 GitHub stars.", "https://github.com/generalaction/emdash", "https://github.com/wsp-labs/wsp"],
    },
  },
];

/** The rows of AHEAD a tool is ahead on, each in one sentence for that tool and one for wsp. */
export function aheadOf(tool: Tool): { title: string; row?: Row; said: Said; wsp: Said }[] {
  return AHEAD.flatMap(a => {
    const said = a.them[tool.slug];
    return said === undefined ? [] : [{ title: a.title, row: a.row, said, wsp: a.wsp }];
  });
}

/** A point's sentence and the pages it was read from. */
export function saidOf(tool: Tool, point: Point): Said {
  if ("said" in point) return point.said;
  const [, first, ...rest] = tool.cells[point.row].says;
  return [point.text, first, ...rest, ...WSP[point.row].says.slice(1)];
}

/** "Where <tool> is stronger": what AHEAD says of it first, each beside wsp's own line, then its own points. */
export function strongerOf(tool: Tool): { said: Said; wsp?: Said }[] {
  return [...aheadOf(tool).map(a => ({ said: a.said, wsp: a.wsp })), ...tool.stronger.map(p => ({ said: saidOf(tool, p) }))];
}

/** Every page a tool's page reads, in the order the page first uses them. */
export function sources(tool: Tool): string[] {
  const cells = ROWS.flatMap(r => [...tool.cells[r.key].says.slice(1), ...WSP[r.key].says.slice(1)]);
  const stronger = strongerOf(tool).flatMap(({ said, wsp }) => [...said.slice(1), ...(wsp?.slice(1) ?? [])]);
  const points = [...tool.weaker, ...tool.pickThem, ...tool.pickWsp].flatMap(p => saidOf(tool, p).slice(1));
  return [...new Set([...cells, ...stronger, ...points, ...STEPS.flatMap(s => s.said.slice(1))])];
}
