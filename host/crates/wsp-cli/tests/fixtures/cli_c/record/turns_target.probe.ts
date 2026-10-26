// turns_target: turns-help.ts lines 238-420 at 65f6b89c7: where a run goes and the threads a line names. Each case calls one
// helper of the TS against a scripted host and writes down every frame sent, every line said, what it returned and
// what it threw.
//
// A target carries a function (`opened`), which JSON drops: the run answers it as the two lines it says, for a thread with no
// folder and for a thread in /root/work/lab (HOME is /root here and in the Rust driver, so `here` shortens it to ~/work/lab).
// A project's folder in a case is `<cwd>/...`, the case's own folder; `.git` entries are `files`.
import { it } from "vitest";
import { record, recorded, type Case, type Obj, type Run } from "./lib.ts";
import { BOTH, J, agentIs, defaultsOf, frame, landing, listed, project, gitTop, refused, resolved, session, sessions, thread, workspace, wsList, wsOne } from "./turns_views.ts";

process.env["HOME"] = "/root";

const cases: Case[] = [];
const add = (c: Case): void => void cases.push(c);

const WSP = project("p-1", "wsp", "here", "<cwd>/wsp", gitTop("<cwd>/wsp"));
const SITE = project("p-2", "site", "pl-box", "/root/site");
const CLOUDY = project("p-3", "cloudy", "pl-attic", "/root/cloudy");
const OTHER = project("p-4", "other", "here", "<cwd>/other");
const wsOf = (id: string, name: string, kind: string, p: Obj, extra: Obj = {}): Obj => workspace(id, name, kind, p, extra);
const MACHINE = wsOf("ws-7", "attic-work", "cloud", WSP);
const LOCAL = wsOf("ws-1", "local-one", "local", WSP);
const PLACE = wsOf("ws-2", "place-one", "place", SITE);
const T = "thread-7f3a9c21e4b5";

// ---- runTarget
const run = (case_: string, a: { ref?: string; cwd?: string; elsewhere?: boolean; where?: Obj }, script: Obj, extra: Partial<Case> = {}): void =>
  add({ fn: "runTarget", case: case_, args: { ref: a.ref, callerCwd: a.cwd, elsewhere: a.elsewhere, where: a.where ?? {} }, script, ...extra });
const both = (case_: string, a: Parameters<typeof run>[1], script: Obj, extra: Partial<Case> = {}): void => {
  run(case_, a, script, extra);
  run(`${case_}, cloud on`, a, script, { ...extra, cloud: true });
};
const noneNamed = (name: string): string => refused(`no project "${name}"; this host holds wsp, site`, "not-found");
const proj = (...ps: Obj[]): Obj => ({ replies: { "projects.resolve": resolved(ps[0]!), "projects.list": listed(...ps) } });

// a project named, on this computer
both("a project here by name", { ref: "wsp" }, proj(WSP));
both("a project here by id", { ref: "p-1" }, proj(WSP));
both("a project here with a branch", { ref: "wsp", where: { branch: "feat/x" } }, proj(WSP));
both("a project here with a folder", { ref: "wsp", where: { cwd: "<cwd>/wsp/web" } }, proj(WSP));
both("a project here with a folder and a branch, branch first", { ref: "wsp", where: { cwd: "/work/lab", branch: "feat/x" } }, proj(WSP));
both("a project here: a relative folder is the start's to refuse", { ref: "wsp", where: { cwd: "web" } }, proj(WSP));
both("a project here with an empty branch and an empty folder", { ref: "wsp", where: { branch: "", cwd: "" } }, proj(WSP));
both("a project here whose second read is refused has no path", { ref: "wsp" }, { replies: { "projects.resolve #1": resolved(WSP), "projects.resolve #2": refused("projects.resolve failed", "provider"), "projects.list": listed(WSP) } });
both("a project here, a thread asking: the ref is read before the token", { ref: "wsp" }, proj(WSP), { env: { WSP_TURN: "tok-1" } });
both("a project here, a caller on a machine", { ref: "wsp", elsewhere: true }, proj(WSP));
both("a project the host's resolve refuses is found in the list by name", { ref: "wsp" }, { replies: { "projects.resolve": refused("projects.resolve failed", "usage"), "projects.list": listed(SITE, WSP) } });
both("a project the host's resolve refuses is found in the list by id", { ref: "p-1" }, { replies: { "projects.resolve": refused("projects.resolve failed", "usage"), "projects.list": listed(SITE, WSP) } });
both("two projects of one name in the list: the first", { ref: "wsp" }, { replies: { "projects.resolve": refused("no", "usage"), "projects.list": listed({ ...WSP, id: "p-8" }, WSP) } });
both("the list's name is matched before its id", { ref: "p-1" }, { replies: { "projects.resolve": refused("no", "usage"), "projects.list": listed({ ...OTHER, id: "x", name: "p-1" }, WSP) } });
both("with the projects not listable, the thread's own workspaces name one", { ref: "wsp" }, {
  replies: {
    "projects.resolve": refused("no project", "not-found"),
    "projects.list": refused("projects.list is not a thread's to ask for", "usage"),
    "workspaces.list": wsList(MACHINE, wsOf("ws-8", "other-machine", "local", OTHER)),
  },
});
both("with the projects not listable, the first workspace of a project names it", { ref: "p-1" }, {
  replies: {
    "projects.resolve #1": refused("no project", "not-found"),
    "projects.resolve #2": resolved(WSP),
    "projects.list": refused("projects.list is not a thread's to ask for", "usage"),
    "workspaces.list": wsList(MACHINE, LOCAL),
  },
});
// a project named, elsewhere
both("a project on a computer the person joined runs in its folder there", { ref: "site" }, { replies: { ...proj(SITE).replies, "workspaces.landing": landing("place") } });
both("a project elsewhere that opens in its folder, a branch asked, is not refused", { ref: "site", where: { branch: "feat/x", cwd: "/root/site/web" } }, { replies: { ...proj(SITE).replies, "workspaces.landing": landing("place") } });
both("a project elsewhere whose landing is local", { ref: "site" }, { replies: { ...proj(SITE).replies, "workspaces.landing": landing("local") } });
both("a project on a cloud forks a machine", { ref: "cloudy" }, { replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": wsList() } });
both("a project on a cloud with a branch is refused", { ref: "cloudy", where: { branch: "feat/x" } }, { replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": wsList() } });
both("a project on a cloud with a folder forks and leaves the folder to the start", { ref: "cloudy", where: { cwd: "/w" } }, { replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": wsList() } });
both("a project whose landing names no kind forks", { ref: "cloudy" }, { replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing(undefined), "workspaces.list": wsList() } });
both("a project whose landing is refused forks", { ref: "cloudy" }, { replies: { ...proj(CLOUDY).replies, "workspaces.landing": refused("workspaces.landing failed", "provider"), "workspaces.list": wsList() } });
both("a project whose landing is no op the host knows forks", { ref: "cloudy" }, { replies: { ...proj(CLOUDY).replies, "workspaces.list": wsList() } });
both("a project elsewhere named by its id", { ref: "p-3" }, { replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": wsList() } });
both("a project on a cloud found only in the list", { ref: "cloudy" }, { replies: { "projects.resolve": refused("no", "not-found"), "projects.list": listed(CLOUDY), "workspaces.landing": landing("cloud"), "workspaces.list": wsList() } });
both("a project on a cloud with a machine of another name forks", { ref: "cloudy" }, { replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": wsList(MACHINE) } });
both("a project on a cloud with a machine of its name: the machine with a cloud, the fork without", { ref: "cloudy" }, {
  replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": wsList(wsOf("ws-5", "cloudy", "cloud", CLOUDY)), "workspaces.resolve": wsOne(wsOf("ws-5", "cloudy", "cloud", CLOUDY)) },
});
both("a project on a cloud with a machine of its name and a branch: the branch is refused first", { ref: "cloudy", where: { branch: "b" } }, {
  replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": wsList(wsOf("ws-5", "cloudy", "cloud", CLOUDY)), "workspaces.resolve": wsOne(wsOf("ws-5", "cloudy", "cloud", CLOUDY)) },
});
both("a project on a cloud typed by id beside a machine of that id's name", { ref: "p-3" }, {
  replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": wsList(wsOf("ws-5", "p-3", "cloud", CLOUDY)), "workspaces.resolve": wsOne(wsOf("ws-5", "p-3", "cloud", CLOUDY)) },
});
both("the machine list refused on a cloud project is the answer with a cloud", { ref: "cloudy" }, { replies: { ...proj(CLOUDY).replies, "workspaces.landing": landing("cloud"), "workspaces.list": refused("workspaces.list failed", "provider") } });
// a word that is no project
both("a word that names nothing", { ref: "nothing" }, { replies: { "projects.resolve": noneNamed("nothing"), "projects.list": listed(WSP, SITE), "workspaces.resolve": refused('no workspace "nothing"', "not-found") } });
both("a word that names nothing, the host's own refusal on the second read", { ref: "nothing" }, { replies: { "projects.resolve #1": refused("no", "not-found"), "projects.resolve #2": noneNamed("nothing"), "projects.list": listed(WSP), "workspaces.resolve": refused('no workspace "nothing"', "not-found") } });
both("a word that names nothing, the second read answers", { ref: "nothing" }, { replies: { "projects.resolve #1": refused("no", "not-found"), "projects.resolve #2": resolved(WSP), "projects.list": listed(WSP), "workspaces.resolve": refused('no workspace "nothing"', "not-found") } });
both("a word that names nothing, the second read refused of another kind", { ref: "nothing" }, { replies: { "projects.resolve #1": refused("no", "not-found"), "projects.resolve #2": refused("this project is hidden from a thread", "auth"), "projects.list": listed(WSP), "workspaces.resolve": refused('no workspace "nothing"', "not-found") } });
both("a word that names nothing, the second read refused with no kind", { ref: "nothing" }, { replies: { "projects.resolve #1": refused("no", "not-found"), "projects.resolve #2": refused("projects.resolve exploded"), "projects.list": listed(WSP), "workspaces.resolve": refused('no workspace "nothing"', "not-found") } });
both("a word that names nothing and a list that lists nothing", { ref: "nothing" }, { replies: { "projects.resolve": noneNamed("nothing"), "projects.list": listed(), "workspaces.resolve": refused('no workspace "nothing"', "not-found") } });
both("an empty word names nothing", { ref: "" }, { replies: { "projects.resolve": noneNamed(""), "projects.list": listed(WSP), "workspaces.resolve": refused('no workspace ""', "not-found") } });
both("a word naming a machine", { ref: "attic-work" }, { replies: { "projects.resolve": noneNamed("attic-work"), "projects.list": listed(WSP), "workspaces.resolve": wsOne(MACHINE) } });
both("a word naming a machine, with a branch", { ref: "attic-work", where: { branch: "b" } }, { replies: { "projects.resolve": noneNamed("attic-work"), "projects.list": listed(WSP), "workspaces.resolve": wsOne(MACHINE) } });
both("a word naming a machine, with a folder", { ref: "attic-work", where: { cwd: "/w" } }, { replies: { "projects.resolve": noneNamed("attic-work"), "projects.list": listed(WSP), "workspaces.resolve": wsOne(MACHINE) } });
both("a word naming a machine of a shape this wsp does not read", { ref: "attic-work" }, { replies: { "projects.resolve": noneNamed("attic-work"), "projects.list": listed(WSP), "workspaces.resolve": frame({ workspace: { id: 1 } }) } });
both("a word naming a machine the host refuses", { ref: "attic-work" }, { replies: { "projects.resolve": noneNamed("attic-work"), "projects.list": listed(WSP), "workspaces.resolve": refused("that machine is not a thread's to drive", "auth") } });
both("a word naming a project hidden from the thread", { ref: "hidden" }, { replies: { "projects.resolve": refused("projects.resolve: hidden is not this thread's project", "auth"), "projects.list": listed(WSP), "workspaces.resolve": refused('no workspace "hidden"', "not-found") } });
both("a word naming a project hidden from the thread, the list refused too", { ref: "hidden" }, { replies: { "projects.resolve": refused("hidden is not this thread's project", "auth"), "projects.list": refused("no", "usage"), "workspaces.list": wsList(), "workspaces.resolve": refused('no workspace "hidden"', "not-found") } });
// ports of run-beside.test.ts, each with the arguments the TS test gives it
const BESIDE = { WSP_TURN: "turn-token-fake" };
run("run-beside: starts beside the thread whose line it is, a line typed on a computer the person joined included (no folder named)", { cwd: "/root/spoo-ts", elsewhere: true }, {}, { env: BESIDE });
run("run-beside: starts beside the thread whose line it is, a line typed on a computer the person joined included (a folder named)", { cwd: "/root/spoo-ts", elsewhere: true, where: { cwd: "/root/spoo-ts/web" } }, {}, { env: BESIDE });
run("run-beside: still names the workspace on a line from a machine that is no thread's", { cwd: "/root/spoo-ts", elsewhere: true }, {}, { env: {} });
const MAC = project("pr_mac", "spoo-ts", "here", "/Users/dev/spoo-ts");
const BOX = project("pr_box", "spoo-ts", "pl_hetzner", "/root/spoo-ts");
run("run-beside: takes the thread's own project where a project on another computer shares its name", { ref: "spoo-ts", cwd: "/root/spoo-ts", elsewhere: true }, { replies: { "projects.list": listed(MAC, BOX), "projects.resolve": resolved(BOX), "workspaces.landing": landing("place") } }, { env: BESIDE });
// no project named
const TOKENS: [string, Record<string, string>][] = [["a turn token", { WSP_TURN: "tok-1" }], ["a host token", { WSP_HOST_TOKEN: "host-1" }], ["both tokens", { WSP_TURN: "tok-1", WSP_HOST_TOKEN: "host-1" }]];
for (const [word, env] of TOKENS) {
  both(`no project, ${word}: beside the thread asking, nothing is asked`, { cwd: "/root/spoo-ts" }, {}, { env });
  both(`no project, ${word} and a folder and a branch`, { cwd: "/root/spoo-ts", where: { branch: "feat/x", cwd: "/root/spoo-ts/web" } }, {}, { env });
  both(`no project, ${word} on a caller on a machine`, { cwd: "/root/spoo-ts", elsewhere: true }, {}, { env });
  both(`no project, ${word} and no folder of the caller's`, {}, {}, { env });
}
both("no project, an empty turn token is no token", { cwd: "<cwd>/wsp/web", elsewhere: false }, proj(WSP), { env: { WSP_TURN: "" } });
both("no project, an empty host token is no token", { cwd: "<cwd>/wsp/web" }, proj(WSP), { env: { WSP_HOST_TOKEN: "" } });
both("no project, an empty turn token on a caller on a machine", { cwd: "<cwd>/wsp", elsewhere: true }, {}, { env: { WSP_TURN: "", WSP_HOST_TOKEN: "" } });
both("no project, other variables are not tokens", { cwd: "<cwd>/wsp" }, proj(WSP), { env: { WSP_TURNS: "x", HOME: "/root", WSP_HOST: "x" } });
both("no project, a caller on a machine with no token is told to name the workspace, and nothing is asked", { cwd: "/root/spoo-ts", elsewhere: true }, {});
both("no project, a caller on a machine, no folder", { elsewhere: true }, {});
both("no project, a caller on this computer with no folder", {}, proj(WSP));
both("no project, elsewhere false is this computer", { cwd: "<cwd>/wsp", elsewhere: false }, proj(WSP));
// no project named: the caller's folder
both("no project, the caller in a project's folder", { cwd: "<cwd>/wsp" }, proj(WSP));
both("no project, the caller in a folder of the project", { cwd: "<cwd>/wsp/web/src" }, proj(WSP));
both("no project, the caller in a folder of the project with a branch", { cwd: "<cwd>/wsp/web", where: { branch: "feat/x" } }, proj(WSP));
both("no project, the caller in a folder of the project with a folder named", { cwd: "<cwd>/wsp/web", where: { cwd: "<cwd>/wsp/api" } }, proj(WSP));
both("no project, the caller's folder with a trailing slash of the project's", { cwd: "<cwd>/wsp/web" }, proj({ ...WSP, path: "<cwd>/wsp/" }));
both("no project, a folder that only shares the project's name as a prefix is not in it", { cwd: "<cwd>/wsp-two" }, { replies: { "projects.list": listed(WSP) } });
both("no project, the deepest project holding the folder wins", { cwd: "<cwd>/wsp/web/src" }, { replies: { "projects.list": listed(WSP, { ...OTHER, id: "p-5", name: "web", path: "<cwd>/wsp/web" }) } });
both("no project, two projects at one path: the first listed", { cwd: "<cwd>/wsp/x" }, { replies: { "projects.list": listed({ ...WSP, id: "p-6", name: "first" }, WSP) } });
both("no project, a project on another computer never holds the folder", { cwd: "<cwd>/wsp/x" }, { replies: { "projects.list": listed({ ...SITE, path: "<cwd>/wsp" }) } });
both("no project, a folder in none of the projects", { cwd: "<cwd>/nowhere" }, { replies: { "projects.list": listed(WSP, OTHER) } });
both("no project, no projects at all", { cwd: "<cwd>/wsp" }, { replies: { "projects.list": listed() } });
both("no project, the projects not listable", { cwd: "<cwd>/wsp" }, { replies: { "projects.list": refused("projects.list is not a thread's to ask for", "usage") } });
both("no project, a worktree of the project's repo carries its own folder", { cwd: "<cwd>/wt" }, { replies: { "projects.list": listed(WSP) } }, { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n" } });
both("no project, a worktree with a branch asked leaves the folder off", { cwd: "<cwd>/wt", where: { branch: "b" } }, { replies: { "projects.list": listed(WSP) } }, { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n" } });
both("no project, a worktree with a folder asked leaves the caller's off", { cwd: "<cwd>/wt", where: { cwd: "<cwd>/wt/web" } }, { replies: { "projects.list": listed(WSP) } }, { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n" } });
both("no project, a folder of a worktree of the project's repo", { cwd: "<cwd>/wt/web/src" }, { replies: { "projects.list": listed(WSP) } }, { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n", "wt/web/src/x.ts": "x" } });
both("no project, a worktree of a repo no project holds", { cwd: "<cwd>/wt" }, { replies: { "projects.list": listed(OTHER) } }, { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n" } });
both("no project, a folder with no repository", { cwd: "<cwd>/plain" }, { replies: { "projects.list": listed(OTHER) } }, { files: { "plain/x.txt": "x" } });

// ---- projectOfFolder
const pof = (case_: string, projects: Obj[], folder: string, extra: Partial<Case> = {}): void => add({ fn: "projectOfFolder", case: case_, args: { projects, folder }, ...extra });
pof("no projects", [], "<cwd>/a");
pof("the folder is the project's", [WSP], "<cwd>/wsp");
pof("a folder inside it", [WSP], "<cwd>/wsp/web/src");
pof("a folder beside it", [WSP], "<cwd>/wsp2");
pof("a folder above it", [WSP], "<cwd>");
pof("the project's path with a trailing slash", [{ ...WSP, path: "<cwd>/wsp/" }], "<cwd>/wsp/web");
pof("the project's path with a trailing slash, the folder is the path without it", [{ ...WSP, path: "<cwd>/wsp/" }], "<cwd>/wsp");
pof("the deepest of two holding projects wins", [WSP, { ...OTHER, path: "<cwd>/wsp/web" }], "<cwd>/wsp/web/x");
pof("the deepest wins whichever is listed first", [{ ...OTHER, path: "<cwd>/wsp/web" }, WSP], "<cwd>/wsp/web/x");
pof("two at one path: the first listed", [{ ...WSP, id: "first" }, { ...WSP, id: "second" }], "<cwd>/wsp/x");
pof("a project on another computer never holds a folder", [{ ...SITE, path: "<cwd>/wsp" }], "<cwd>/wsp/x");
pof("a project on another computer beside one here", [{ ...SITE, path: "<cwd>/wsp/deep" }, WSP], "<cwd>/wsp/deep/x");
pof("a worktree of the repo", [WSP], "<cwd>/wt", { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n" } });
pof("a worktree whose git file carries other lines", [WSP], "<cwd>/wt", { files: { "wt/.git": "# note\ngitdir:   <cwd>/wsp/.git/worktrees/wt   \nmore\n" } });
pof("a folder deep in a worktree", [WSP], "<cwd>/wt/a/b", { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n", "wt/a/b/f": "x" } });
pof("a worktree of a repo whose project is on another computer", [{ ...SITE, git: { top: "<cwd>/wsp" } }], "<cwd>/wt", { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n" } });
pof("a worktree of a repo no project's top is", [OTHER], "<cwd>/wt", { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n" } });
pof("a worktree and a project with no git record", [{ ...OTHER, path: "<cwd>/o" }], "<cwd>/wt", { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/worktrees/wt\n" } });
pof("a worktree whose git file has no gitdir line", [WSP], "<cwd>/wt", { files: { "wt/.git": "nothing here\n" } });
pof("a worktree whose git dir is not under .git/worktrees", [WSP], "<cwd>/wt", { files: { "wt/.git": "gitdir: <cwd>/wsp/.git/modules/wt\n" } });
pof("a worktree whose git dir is a plain folder named worktrees", [WSP], "<cwd>/wt", { files: { "wt/.git": "gitdir: <cwd>/wsp/worktrees/wt\n" } });
pof("a worktree with a relative gitdir", [WSP], "<cwd>/wt", { files: { "wt/.git": "gitdir: ../wsp/.git/worktrees/wt\n" } });
pof("the repository's own folder above a project in it", [{ ...OTHER, path: "<cwd>/repo/sub", git: { top: "<cwd>/repo" } }], "<cwd>/repo", { files: { "repo/.git/HEAD": "ref: refs/heads/main\n" } });
pof("a folder in the repository beside a project in it", [{ ...OTHER, path: "<cwd>/repo/sub", git: { top: "<cwd>/repo" } }], "<cwd>/repo/other", { files: { "repo/.git/HEAD": "ref: refs/heads/main\n", "repo/other/f": "x" } });
pof("a folder in a repository no project is in", [OTHER], "<cwd>/repo", { files: { "repo/.git/HEAD": "ref: refs/heads/main\n" } });
pof("a folder with no repository above it", [OTHER], "<cwd>/plain", { files: { "plain/f": "x" } });

// ---- forkFor
const NAMES = (...n: string[]): string => wsList(...n.map((name, i) => wsOf(`ws-${i + 1}`, name, "cloud", CLOUDY)));
const CREATED = (name: string, extra: Obj = {}): string => frame({ workspace: wsOf("ws-9", name, "cloud", CLOUDY), ...extra });
const stage = (name: string, stage_: string, message: string, extra: Obj = {}): string => JSON.stringify({ type: "workspace.creating", workspaceId: "ws-9", name, stage: stage_, message, elapsedMs: 120, ...extra });
const forkRun = (case_: string, args: Obj, script: Obj): void => add({ fn: "forkFor", case: case_, args: { project: CLOUDY, ...args }, script });
const forkScript = (names: string[], name: string, over: Obj = {}): Obj => ({ replies: { "workspaces.list": NAMES(...names), "workspaces.landing": landing("cloud"), "workspaces.create": CREATED(name), ...(over["replies"] as Obj | undefined) }, ...Object.fromEntries(Object.entries(over).filter(([k]) => k !== "replies")) });
forkRun("a machine named off the task", { task: "fix the login page before friday" }, forkScript([], "fix the login page before"));
forkRun("the same task again is numbered", { task: "fix the login page before friday" }, forkScript(["fix the login page before"], "fix the login page before 2"));
forkRun("the number goes up past the taken ones", { task: "fix the login page before friday" }, forkScript(["fix the login page before", "fix the login page before 2", "fix the login page before 4"], "fix the login page before 3"));
forkRun("a name taken in other words is not taken", { task: "fix the login page before friday" }, forkScript(["Fix the login page before", "fix the login page"], "fix the login page before"));
forkRun("the name is cut at forty characters, where the cut lands on a space", { task: `${"a".repeat(39)} and the rest of it` }, forkScript([], "a".repeat(39)));
forkRun("the cut name taken is numbered", { task: `${"a".repeat(39)} and the rest of it` }, forkScript(["a".repeat(39)], `${"a".repeat(39)} 2`));
forkRun("a name cut at exactly forty characters", { task: "b".repeat(60) }, forkScript([], "b".repeat(40)));
forkRun("a task of five words and a sixth", { task: "one two three four five six" }, forkScript([], "one two three four five"));
forkRun("only the first line names it", { task: "first line here\nsecond line is ignored" }, forkScript([], "first line here"));
forkRun("blank lines before the first are skipped by the trim", { task: "\n\n  spaced   out   words \n" }, forkScript([], "spaced out words"));
forkRun("a single word", { task: "go" }, forkScript([], "go"));
forkRun("an empty task names an empty machine", { task: "" }, forkScript([], ""));
forkRun("an empty task beside a taken empty name", { task: "" }, forkScript([""], " 2"));
forkRun("an accented task", { task: "répare la page d'accueil très vite maintenant" }, forkScript([], "répare la page d'accueil très"));
forkRun("tabs between the words", { task: "fix\tthe\tpage" }, forkScript([], "fix the page"));
forkRun("the stages go to say as they arrive", { task: "fix it" }, forkScript([], "fix it", {
  pushed: { "workspaces.create": [stage("fix it", "fork-requested", "asking for a machine"), stage("fix it", "daemon-answering", "waiting for the machine\n"), stage("fix it", "ready", "ready  \n\n")] },
}));
forkRun("a stage of another machine is not said", { task: "fix it" }, forkScript([], "fix it", {
  pushed: { "workspaces.create": [stage("someone else", "fork-requested", "not ours"), stage("fix it", "ready", "ours")] },
}));
forkRun("a frame that is no stage is not said", { task: "fix it" }, forkScript([], "fix it", {
  pushed: { "workspaces.create": [JSON.stringify({ type: "workspace.napped", workspaceId: "ws-9" }), stage("fix it", "ready", "ours")] },
}));
forkRun("a stage heard while the events are subscribed is said when the follow starts", { task: "fix it" }, forkScript([], "fix it", {
  pushed: { "events.subscribe": [stage("fix it", "fork-requested", "early")] },
}));
forkRun("a stage heard after the answer is not said", { task: "fix it" }, forkScript([], "fix it", {
  after: { "workspaces.create": [stage("fix it", "ready", "late")] },
}));
forkRun("a notice rides the created line", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.landing": landing("cloud"), "workspaces.create": CREATED("fix it", { notice: "no git-ignored files were sent" }) } });
forkRun("a notice of two lines stays one said line", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.landing": landing("cloud"), "workspaces.create": CREATED("fix it", { notice: "first\nsecond" }) } });
forkRun("a created machine with a folder says that folder", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.landing": landing("cloud"), "workspaces.create": frame({ workspace: wsOf("ws-9", "fix it", "cloud", CLOUDY, { folder: "/root/cloudy/work" }) }) } });
forkRun("a project on this computer forks nothing and is asked no size", { project: WSP, task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.landing": landing("local"), "workspaces.create": frame({ workspace: wsOf("ws-9", "fix it", "local", WSP) }) } });
forkRun("the landing refused costs no stage and no machine", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.landing": refused("this computer forks nothing", "usage") } });
forkRun("the machine list refused", { task: "fix it" }, { replies: { "workspaces.list": refused("workspaces.list failed", "provider") } });
forkRun("the create refused", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.landing": landing("cloud"), "workspaces.create": refused("the cloud has no room for another machine", "provider") } });
forkRun("the create refused after a stage", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.landing": landing("cloud"), "workspaces.create": refused("the cloud said no", "provider") }, pushed: { "workspaces.create": [stage("fix it", "fork-requested", "asking")] } });

// ---- subFor
const SUBSTARTED = (J(recorded("run", "a sub-thread on Boat, detached")["workspaces.create"]!)["sub"]) as Obj;
const subCreated = (name: string, extra: Obj = {}): string => frame({ workspace: wsOf("ws-sub", name, "cloud", WSP), sub: SUBSTARTED, ...extra });
const subRun = (case_: string, args: Obj, script: Obj, extra: Partial<Case> = {}): void => add({ fn: "subFor", case: case_, args: { ask: { on: "boat" }, task: "fix it", ...args }, script, ...extra });
const subScript = (names: string[], name: string, over: Obj = {}): Obj => ({ replies: { "workspaces.list": NAMES(...names), "workspaces.create": subCreated(name), ...(over["replies"] as Obj | undefined) }, ...Object.fromEntries(Object.entries(over).filter(([k]) => k !== "replies")) });
subRun("named off the task", { task: "fix the login page before friday" }, subScript([], "fix the login page before"));
subRun("named by its title", { task: "fix it", title: "Login fix" }, subScript([], "Login fix"));
subRun("the title is not cut", { task: "fix it", title: "t".repeat(60) }, subScript([], "t".repeat(60)));
subRun("an empty title is a title", { task: "fix it", title: "" }, subScript([], ""));
subRun("a title taken is numbered", { task: "fix it", title: "Login fix" }, subScript(["Login fix"], "Login fix 2"));
subRun("a name taken is numbered", { task: "fix it" }, subScript(["fix it"], "fix it 2"));
subRun("the numbers up to a gap", { task: "fix it" }, subScript(["fix it", "fix it 2", "fix it 3", "fix it 5"], "fix it 4"));
subRun("the project goes as its own id", { ask: { on: "boat", project: "p-1" }, task: "fix it" }, subScript([], "fix it"));
subRun("no project goes as an empty word", { ask: { on: "boat" }, task: "fix it" }, subScript([], "fix it"));
subRun("every field of the ask, in the frame's order whatever the ask's", { ask: { harness: "codex", branch: "feat/x", from: "feat/up", size: "big", on: "boat", project: "p-1" }, task: "fix it" }, subScript([], "fix it"));
subRun("a size alone", { ask: { on: "boat", size: "big" }, task: "fix it" }, subScript([], "fix it"));
subRun("a base alone", { ask: { on: "boat", from: "feat/up" }, task: "fix it" }, subScript([], "fix it"));
subRun("a branch alone", { ask: { on: "boat", branch: "feat/x" }, task: "fix it" }, subScript([], "fix it"));
subRun("a harness alone", { ask: { on: "boat", harness: "codex" }, task: "fix it" }, subScript([], "fix it"));
subRun("empty words are carried as they are", { ask: { on: "", size: "", from: "", branch: "", harness: "", project: "" }, task: "fix it" }, subScript([], "fix it"));
subRun("the stages go to say before the answer", { task: "fix it" }, subScript([], "fix it", { pushed: { "workspaces.create": [stage("fix it", "fork-requested", "asking"), stage("fix it", "ready", "ready")] } }));
subRun("a stage of another machine is not said", { task: "fix it" }, subScript([], "fix it", { pushed: { "workspaces.create": [stage("someone else", "ready", "not ours"), stage("fix it", "ready", "ours")] } }));
subRun("a frame that is no stage is not said", { task: "fix it" }, subScript([], "fix it", { pushed: { "workspaces.create": [JSON.stringify({ type: "workspace.napped", workspaceId: "ws-9", name: "fix it" })] } }));
subRun("a stage's own message is said whole, not trimmed", { task: "fix it" }, subScript([], "fix it", { pushed: { "workspaces.create": [stage("fix it", "ready", "ready  \n")] } }));
subRun("a stage heard while the events are subscribed is said", { task: "fix it" }, subScript([], "fix it", { pushed: { "events.subscribe": [stage("fix it", "fork-requested", "early")] } }));
subRun("a stage after the answer is said until stop", { task: "fix it", wait: true }, subScript([], "fix it", { after: { "workspaces.create": [stage("fix it", "ready", "late")] } }));
subRun("a stage after the answer is not said once stop is called", { task: "fix it", stopAtOnce: true }, subScript([], "fix it", { after: { "workspaces.create": [stage("fix it", "ready", "late")] } }));
subRun("the notice is said a line at a time", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.create": subCreated("fix it", { notice: "no git-ignored files were sent\nthe cloud woke slowly" }) } });
subRun("a notice of one line", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.create": subCreated("fix it", { notice: "one line" }) } });
subRun("an empty notice says one empty line", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.create": subCreated("fix it", { notice: "" }) } });
subRun("a notice with a trailing newline says its empty last line", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.create": subCreated("fix it", { notice: "a\nb\n" }) } });
subRun("the stages come before the notice", { task: "fix it" }, { replies: { "workspaces.list": NAMES(), "workspaces.create": subCreated("fix it", { notice: "note" }) }, pushed: { "workspaces.create": [stage("fix it", "ready", "stage")] } });
subRun("the create refused", { task: "fix it" }, subScript([], "fix it", { replies: { "workspaces.create": refused("the cloud has no room for another machine", "provider") } }));
subRun("the create refused after a stage", { task: "fix it" }, subScript([], "fix it", { replies: { "workspaces.create": refused("the cloud said no", "usage") }, pushed: { "workspaces.create": [stage("fix it", "fork-requested", "asking")] } }));
subRun("the machine list refused", { task: "fix it" }, { replies: { "workspaces.list": refused("workspaces.list failed", "provider") } });

// ---- threadHere
const hereRun = (case_: string, args: Obj, script: Obj): void => add({ fn: "threadHere", case: case_, args, script });
const S2 = sessions(session("s-1", "ws-1", T), session("s-2", "ws-2", "thread-b0b0b0b0c1c1"), session("s-3", "ws-7", "thread-cccc33334444"), session("s-4", "ws-9", "thread-dddd55556666"), session("s-5", "ws-1", undefined));
const hereScript = (...ws: Obj[]): Obj => ({ replies: { "sessions.list": S2, "workspaces.list": wsList(...ws) } });
hereRun("a thread in this computer's folder", { ref: T }, hereScript(LOCAL, PLACE, MACHINE));
hereRun("by a prefix", { ref: "thread-7f3a" }, hereScript(LOCAL, PLACE, MACHINE));
hereRun("a thread in a folder on a computer the person joined", { ref: "thread-b0b0" }, hereScript(LOCAL, PLACE, MACHINE));
hereRun("a thread on a machine is none", { ref: "thread-cccc" }, hereScript(LOCAL, PLACE, MACHINE));
hereRun("a thread whose workspace is not listed is none", { ref: "thread-dddd" }, hereScript(LOCAL, PLACE, MACHINE));
hereRun("a workspace of no kind reads as a machine", { ref: T }, hereScript({ ...LOCAL, kind: undefined }));
hereRun("a row with no thread id is named by its own", { ref: "s-5" }, hereScript(LOCAL));
hereRun("a word naming no thread is none", { ref: "zzz" }, hereScript(LOCAL));
hereRun("a word naming two threads is none", { ref: "thread-" }, hereScript(LOCAL));
hereRun("no word asks nothing", {}, {});
hereRun("a thread already read asks for no listing", { ref: thread(T, "ws-1", { threadId: T }) }, hereScript(LOCAL));
hereRun("a thread already read, with no threadId, is named by its id", { ref: thread("row-1", "ws-1") }, hereScript(LOCAL));
hereRun("a thread already read, on a machine", { ref: thread("row-7", "ws-7") }, hereScript(LOCAL, MACHINE));
hereRun("the made worktree it runs in", { ref: T }, hereScript(wsOf("ws-1", "wt", "local", WSP, { worktree: { path: "/Users/me/.wsp/worktrees/p-1/feat-x", branch: "feat/x", made: true } })));
hereRun("a worktree wsp did not make is not named", { ref: T }, hereScript(wsOf("ws-1", "wt", "local", WSP, { worktree: { path: "/Users/me/wsp", branch: "main", made: false } })));
hereRun("a made worktree that is gone is not named", { ref: T }, hereScript(wsOf("ws-1", "wt", "local", WSP, { worktree: { path: "/Users/me/.wsp/worktrees/p-1/feat-x", branch: "feat/x", made: true, gone: true } })));
hereRun("a made worktree of no branch", { ref: T }, hereScript(wsOf("ws-1", "wt", "place", SITE, { worktree: { path: "/root/.wsp/worktrees/x", made: true } })));
hereRun("the listing of threads refused is none", { ref: T }, { replies: { "sessions.list": refused("sessions.list failed", "provider"), "workspaces.list": wsList(LOCAL) } });
hereRun("the listing of workspaces refused is the answer", { ref: T }, { replies: { "sessions.list": S2, "workspaces.list": refused("workspaces.list failed", "provider") } });

// ---- subDeleting
const subd = (case_: string, args: Obj, script: Obj = {}): void => add({ fn: "subDeleting", case: case_, args, script });
const SUBROW = { state: "napping", on: "boat", machine: "Fix it", size: { cpu: 2, memMb: 4096 }, rateUsdPerHour: 0.018, branch: "wsp/fix-it", deleteAt: 1790000000000 };
const QUESTION = frame({ workspaceId: "ws-9", threads: 1, on: "Boat", question: "Delete sub-thread t-9 (Fix it) and its machine on Boat?\n2 commits on wsp/fix-it are not pushed and go with it." });
subd("no thread", {});
subd("a thread that is no sub-thread", { thread: thread("t-1", "ws-1", { threadId: "t-1" }) });
subd("a sub-thread asks the host first, as a check", { thread: thread("t-9", "ws-9", { threadId: "t-9", sub: SUBROW }) }, { replies: { "sessions.delete": QUESTION } });
subd("a sub-thread with no thread id is named by its row", { thread: thread("row-9", "ws-9", { sub: SUBROW }) }, { replies: { "sessions.delete": QUESTION } });
subd("the question as the host words it, whole", { thread: thread("t-9", "ws-9", { threadId: "t-9", sub: SUBROW }) }, { replies: { "sessions.delete": frame({ question: "Delete it?\n" }) } });
subd("an empty question", { thread: thread("t-9", "ws-9", { threadId: "t-9", sub: SUBROW }) }, { replies: { "sessions.delete": frame({ question: "" }) } });
subd("the host refusing: work not pushed", { thread: thread("t-9", "ws-9", { threadId: "t-9", sub: SUBROW }) }, { replies: { "sessions.delete": refused("sub-thread t-9 holds 2 commits not pushed on wsp/fix-it. Send it a push, or ask the person to delete it.", "usage") } });
subd("a sub-thread with a result", { thread: thread("t-9", "ws-9", { threadId: "t-9", sub: { ...SUBROW, state: "released", releasedAt: 5, cloudGone: true } }) }, { replies: { "sessions.delete": QUESTION } });

// ---- refuseMachineThread
const rmt = (case_: string, args: Obj, script: Obj): void => add({ fn: "refuseMachineThread", case: case_, args, script });
rmt("a thread on a machine is refused with the machine's name", { ref: "thread-cccc" }, hereScript(LOCAL, PLACE, MACHINE));
rmt("a thread in this computer's folder is left alone", { ref: T }, hereScript(LOCAL, PLACE, MACHINE));
rmt("a thread in a folder on a joined computer is left alone", { ref: "thread-b0b0" }, hereScript(LOCAL, PLACE, MACHINE));
rmt("a thread whose workspace is not listed is left alone", { ref: "thread-dddd" }, hereScript(LOCAL, PLACE, MACHINE));
rmt("a workspace of no kind reads as a machine and is refused", { ref: T }, hereScript({ ...LOCAL, kind: undefined, name: "old-one" }));
rmt("a word naming no thread is left to the caller's own refusal", { ref: "zzz" }, hereScript(MACHINE));
rmt("a word naming two threads is left to the caller's own refusal", { ref: "thread-" }, hereScript(MACHINE));
rmt("a thread by a prefix on a machine", { ref: "thread-cc" }, hereScript(MACHINE));
rmt("the thread listing refused leaves the refusal to the caller", { ref: T }, { replies: { "sessions.list": refused("sessions.list failed", "provider"), "workspaces.list": wsList(MACHINE) } });
rmt("the workspace listing refused is the answer", { ref: T }, { replies: { "sessions.list": S2, "workspaces.list": refused("workspaces.list failed", "provider") } });

// ---- threadAt
const at = (case_: string, args: Obj, script: Obj, extra: Partial<Case> = {}): void => add({ fn: "threadAt", case: case_, args, script, ...extra });
const atBoth = (case_: string, args: Obj, script: Obj, extra: Partial<Case> = {}): void => {
  at(case_, args, script, extra);
  at(`${case_}, cloud on`, args, script, { ...extra, cloud: true });
};
const GOT = wsOne(wsOf("ws-1", "attic-work", "cloud", WSP));
const base = { replies: { "sessions.list": S2, "workspaces.get": GOT } };
const missing = (extra: Obj): Obj => ({ replies: { "sessions.list": S2, ...extra } });
atBoth("a thread by id", { ref: T, line: "wsp stop" }, base);
atBoth("a thread by a prefix", { ref: "thread-7f3", line: "wsp stop" }, base);
atBoth("the thread's own workspace is asked for", { ref: "thread-dddd", line: "wsp send" }, base);
atBoth("a row with no thread id asks with its own id", { ref: "s-5", line: "wsp send" }, base);
atBoth("a workspace the host answers in another version", { ref: T, line: "wsp stop" }, { replies: { "sessions.list": S2, "workspaces.get": frame({ workspace: { id: 1 } }) } });
atBoth("no workspace in the answer", { ref: T, line: "wsp stop" }, { replies: { "sessions.list": S2, "workspaces.get": frame({}) } });
atBoth("a workspace the host refuses", { ref: T, line: "wsp stop" }, { replies: { "sessions.list": S2, "workspaces.get": refused("that thread is not yours to drive", "auth") } });
atBoth("the thread listing refused", { ref: T, line: "wsp stop" }, { replies: { "sessions.list": refused("sessions.list failed", "provider") } });
atBoth("a word naming two threads is not a refusal of the rest", { ref: "thread-", line: "wsp stop" }, missing({ "projects.list": listed(WSP) }));
atBoth("a word naming nothing and no project", { ref: "zzz", line: "wsp stop" }, missing({ "projects.list": listed(WSP, SITE), "workspaces.list": wsList(MACHINE, LOCAL) }));
atBoth("a project's name", { ref: "wsp", line: "wsp stop" }, missing({ "projects.list": listed(WSP, SITE), "workspaces.list": wsList(MACHINE) }));
atBoth("a project's id", { ref: "p-2", line: "wsp send" }, missing({ "projects.list": listed(WSP, SITE), "workspaces.list": wsList(MACHINE) }));
atBoth("a project's name, said for a line that also takes a computer", { ref: "wsp", line: "wsp thread read", orComputer: true }, missing({ "projects.list": listed(WSP), "workspaces.list": wsList(MACHINE) }));
atBoth("a machine's name", { ref: "attic-work", line: "wsp stop" }, missing({ "projects.list": listed(WSP), "workspaces.list": wsList(MACHINE, LOCAL, PLACE) }));
atBoth("a machine's id", { ref: "ws-7", line: "wsp stop" }, missing({ "projects.list": listed(WSP), "workspaces.list": wsList(MACHINE, LOCAL, PLACE) }));
atBoth("a machine's name, said for a line that also takes a computer", { ref: "attic-work", line: "wsp thread read", orComputer: true }, missing({ "projects.list": listed(WSP), "workspaces.list": wsList(MACHINE) }));
atBoth("a folder workspace's name is no machine", { ref: "local-one", line: "wsp stop" }, missing({ "projects.list": listed(WSP), "workspaces.list": wsList(MACHINE, LOCAL, PLACE) }));
atBoth("a joined computer's workspace name is no machine", { ref: "place-one", line: "wsp stop" }, missing({ "projects.list": listed(WSP), "workspaces.list": wsList(MACHINE, LOCAL, PLACE) }));
atBoth("a workspace of no kind is a machine", { ref: "old-one", line: "wsp stop" }, missing({ "projects.list": listed(WSP), "workspaces.list": wsList({ ...MACHINE, kind: undefined, name: "old-one" }) }));
atBoth("a word that is both a project and a machine is a project", { ref: "both", line: "wsp stop" }, missing({ "projects.list": listed({ ...WSP, name: "both" }), "workspaces.list": wsList({ ...MACHINE, name: "both" }) }));
atBoth("the project list refused: the machines are still read", { ref: "attic-work", line: "wsp stop" }, missing({ "projects.list": refused("projects.list is not a thread's to ask for", "usage"), "workspaces.list": wsList(MACHINE) }));
atBoth("the project list refused and the workspaces not listing a machine", { ref: "zzz", line: "wsp stop" }, missing({ "projects.list": refused("no", "usage"), "workspaces.list": wsList(LOCAL), "workspaces.get": GOT }));
atBoth("the project list refused: a project of the thread's own workspaces", { ref: "wsp", line: "wsp stop" }, missing({ "projects.list": refused("no", "usage"), "workspaces.list": wsList(MACHINE) }));
atBoth("the machine list refused: no machine is named", { ref: "attic-work", line: "wsp stop" }, missing({ "projects.list": listed(WSP), "workspaces.list": refused("workspaces.list failed", "provider") }));
atBoth("no projects and no machines", { ref: "zzz", line: "wsp stop" }, missing({ "projects.list": listed(), "workspaces.list": wsList() }));
atBoth("the empty word names every thread", { ref: "", line: "wsp stop" }, missing({ "projects.list": listed(WSP) }));
atBoth("a line named with its own words", { ref: "wsp", line: "wsp merge --into" }, missing({ "projects.list": listed(WSP) }));

// ---- childOf
const leadView = thread(T, "ws-1", { threadId: T });
const leadWs = wsOf("ws-1", "attic-work", "cloud", WSP);
const kid = (case_: string, args: Obj, script: Obj, extra: Partial<Case> = {}): void => {
  add({ fn: "childOf", case: case_, args: { lead: { thread: leadView, workspace: leadWs }, line: "wsp merge", ...args }, script, ...extra });
};
const kidWs = (id: string): string => wsOne(wsOf(id, `kid-${id}`, "cloud", WSP));
kid("a child in a folder of its own", { ref: "thread-b0b0" }, { replies: { "sessions.list": S2, "workspaces.get": kidWs("ws-2") } });
kid("a child by a prefix", { ref: "thread-b0" }, { replies: { "sessions.list": S2, "workspaces.get": kidWs("ws-2") } });
kid("a child beside its lead is refused", { ref: "thread-cccc" }, { replies: { "sessions.list": S2, "workspaces.get": kidWs("ws-1") } });
kid("a child beside its lead named by a row with no thread id", { ref: "s-5" }, { replies: { "sessions.list": S2, "workspaces.get": kidWs("ws-1") } });
kid("the lead's own thread named as its child", { ref: T }, { replies: { "sessions.list": S2, "workspaces.get": kidWs("ws-1") } });
kid("the lead with no thread id is named by its row", { ref: "thread-cccc", lead: { thread: thread("row-lead", "ws-1"), workspace: leadWs } }, { replies: { "sessions.list": S2, "workspaces.get": kidWs("ws-1") } });
kid("a child naming no thread", { ref: "zzz" }, { replies: { "sessions.list": S2, "projects.list": listed(WSP) } });
kid("a child that is a project", { ref: "wsp" }, { replies: { "sessions.list": S2, "projects.list": listed(WSP) } });
kid("a child that is a project, with the cloud", { ref: "wsp" }, { replies: { "sessions.list": S2, "projects.list": listed(WSP) } }, { cloud: true });
kid("a child that is a machine, with the cloud", { ref: "attic-work" }, { replies: { "sessions.list": S2, "projects.list": listed(), "workspaces.list": wsList(MACHINE) } }, { cloud: true });
kid("a child that is a machine, with no cloud", { ref: "attic-work" }, { replies: { "sessions.list": S2, "projects.list": listed(), "workspaces.list": wsList(MACHINE) } });
kid("a child the host refuses", { ref: "thread-b0b0" }, { replies: { "sessions.list": S2, "workspaces.get": refused("that thread is not yours to drive", "auth") } });
kid("a child's workspace of another version", { ref: "thread-b0b0" }, { replies: { "sessions.list": S2, "workspaces.get": frame({ workspace: 3 }) } });

// ---- sharingWith
const share = (case_: string, args: Obj, script: Obj): void => add({ fn: "sharingWith", case: case_, args, script });
const mine = { thread: thread(T, "ws-1", { threadId: T }), workspace: leadWs };
share("the others of the folder, in the host's order", { at: mine }, { replies: { "sessions.list": sessions(session("a", "ws-1", "thread-a"), session("b", "ws-1", T), session("c", "ws-1", "thread-c")) } });
share("none besides itself", { at: mine }, { replies: { "sessions.list": sessions(session("b", "ws-1", T)) } });
share("none at all", { at: mine }, { replies: { "sessions.list": sessions() } });
share("a thread of another folder is left out", { at: mine }, { replies: { "sessions.list": sessions(session("a", "ws-2", "thread-a"), session("b", "ws-1", "thread-b")) } });
share("a thread's turns are one thread", { at: mine }, { replies: { "sessions.list": sessions(session("a1", "ws-1", "thread-a"), session("a2", "ws-1", "thread-a"), session("b", "ws-1", T), session("b2", "ws-1", T)) } });
share("a row with no thread id is a thread of its own id", { at: mine }, { replies: { "sessions.list": sessions(session("loose-1", "ws-1", undefined), session("b", "ws-1", T)) } });
share("the thread asking has no thread id: its row id is the one left out", { at: { thread: thread("row-1", "ws-1"), workspace: leadWs } }, { replies: { "sessions.list": sessions(session("row-1", "ws-1", undefined), session("c", "ws-1", "thread-c")) } });
share("the listing is asked for the workspace", { at: { thread: mine.thread, workspace: wsOf("ws-9", "other", "cloud", WSP) } }, { replies: { "sessions.list": sessions(session("a", "ws-9", "thread-a")) } });
share("the listing refused", { at: mine }, { replies: { "sessions.list": refused("sessions.list failed", "provider") } });

// ---- threadDeleted
const DEL = recorded("delete", "thread unconfirmed")["sessions.delete"]!;
const del = (case_: string, args: Obj, reply: string): void => add({ fn: "threadDeleted", case: case_, args, script: { replies: { "sessions.delete": reply } } });
del("with its worktree and two threads", { threadId: "t-3" }, DEL);
del("a thread in the folder, nothing else", { threadId: "t-3" }, recorded("delete", "thread in the folder unconfirmed")["sessions.delete"]!);
del("the host's order is not the answer's", { threadId: "t-3" }, frame({ threads: 2, worktree: "/w/x", workspaceId: "ws-2" }));
del("extra fields are dropped", { threadId: "t-3" }, recorded("delete", "thread deleted with its worktree")["sessions.delete"]!);
del("the processes ended", { threadId: "t-3", o: { processes: "stop" } }, recorded("delete", "thread deleted with its processes stopped")["sessions.delete"]!);
del("processes kept", { threadId: "t-3", o: { processes: "keep" } }, frame({ workspaceId: "ws-2", threads: 1 }));
del("processes asked", { threadId: "t-3", o: { processes: "ask" } }, frame({ workspaceId: "ws-2", threads: 1 }));
del("a check", { threadId: "t-3", o: { check: true } }, frame({ workspaceId: "ws-2", threads: 1 }));
del("processes and a check", { threadId: "t-3", o: { processes: "stop", check: false } }, frame({ workspaceId: "ws-2", threads: 1 }));
del("an empty list of ended is kept", { threadId: "t-3" }, frame({ workspaceId: "ws-2", threads: 1, ended: [] }));
del("a sub-thread's machine deleted on its cloud", { threadId: "t-9" }, recorded("delete", "sub-thread deleted")["sessions.delete"]!);
del("a machine word that is not deleted is dropped", { threadId: "t-9" }, frame({ workspaceId: "ws-9", threads: 1, machine: "kept", on: "Boat" }));
del("a cloud named without a machine", { threadId: "t-9" }, frame({ workspaceId: "ws-9", threads: 1, on: "Boat" }));
del("no threads is zero", { threadId: "t-3" }, frame({ workspaceId: "ws-2", threads: 0 }));
del("refused for processes", { threadId: "t-3" }, recorded("delete", "thread refused while processes run")["sessions.delete"]!);
del("refused for a worktree with changes", { threadId: "t-3" }, recorded("delete", "thread refused")["sessions.delete"]!);
del("a sub-thread refused over work not pushed", { threadId: "t-9" }, recorded("delete", "sub-thread refused over work not pushed")["sessions.delete"]!);

// ---- the runs
const summaryOf = (target: any): unknown => {
  if (target === undefined || target === null) return null;
  const { opened, ...rest } = target as { opened?: (id: string, folder?: string) => string } & Obj;
  return { ...rest, ...(opened !== undefined ? { opened: { bare: opened("thread-1"), folder: opened("thread-1", "/root/work/lab") } } : {}) };
};
const T_ = (mod: Obj): any => mod["turns"];
const runs: Record<string, Run> = {
  runTarget: async ({ mod, client, args, c }) => summaryOf(await T_(mod).runTarget(client, args["ref"], args["callerCwd"], c.env ?? {}, args["elsewhere"], args["where"] ?? {})),
  projectOfFolder: async ({ mod, args }) => T_(mod).projectOfFolder(args["projects"], args["folder"]) ?? null,
  forkFor: async ({ mod, client, args, sinks }) => T_(mod).forkFor(client, args["project"], args["task"], (line: string) => sinks.said.push(line)),
  subFor: async ({ mod, client, args, sinks }) => {
    const made = await T_(mod).subFor(client, args["ask"], args["task"], args["title"], (line: string) => sinks.said.push(line));
    if (args["stopAtOnce"] === true) made.stop();
    if (args["wait"] === true || args["stopAtOnce"] === true) await new Promise(r => setTimeout(r, 20));
    made.stop();
    return { workspace: made.workspace, sub: made.sub };
  },
  threadHere: async ({ mod, client, args }) => (await T_(mod).threadHere(client, args["ref"])) ?? null,
  subDeleting: async ({ mod, client, args }) => (await T_(mod).subDeleting(client, args["thread"])) ?? null,
  refuseMachineThread: async ({ mod, client, args }) => {
    await T_(mod).refuseMachineThread(client, args["ref"]);
    return null;
  },
  threadAt: ({ mod, client, args }) => T_(mod).threadAt(client, args["ref"], args["line"], args["orComputer"]),
  childOf: ({ mod, client, args }) => T_(mod).childOf(client, args["lead"], args["ref"], args["line"]),
  sharingWith: ({ mod, client, args }) => T_(mod).sharingWith(client, args["at"]),
  threadDeleted: ({ mod, client, args }) => T_(mod).threadDeleted(client, args["threadId"], args["o"]),
};

const scrubbed = (run: Run): Run => async ctx => {
  try {
    const value = await run(ctx);
    return value === undefined ? value : (JSON.parse(JSON.stringify(value).split(ctx.cwd).join("<cwd>")) as unknown);
  } catch (e) {
    if (e instanceof Error) e.message = e.message.split(ctx.cwd).join("<cwd>");
    throw e;
  }
};

it("turns_target", async () => {
  await record("turns_target", cases, Object.fromEntries(Object.entries(runs).map(([k, v]) => [k, scrubbed(v)])));
});
void BOTH;
void defaultsOf;
void agentIs;
