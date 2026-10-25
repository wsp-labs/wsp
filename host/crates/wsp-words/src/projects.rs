// SPDX-License-Identifier: AGPL-3.0-only
//! A project as wsp records it: what a source word names, where a checkout lands, the clone and worktree lines, and
//! the refusals of a project in the wrong place. Port of the functions of packages/protocol/src/projects.ts at
//! a3b870a6e; its shapes and flat constants are wsp_wire::types::projects. See spec/projects.md.

use wsp_wire::views::workspace::ProjectSource;
use wsp_wire::views::workspace::ProjectView;
use wsp_wire::views::workspace_kind::WorkspaceKind;
use wsp_wire::views::workspace::WorkspaceProject;

use serde::Serialize;

use crate::exit::HostError;

/// What a caller is told once a project is recorded: what it is called, where its code comes from, the computer it
/// lives on and where a workspace of it holds the checkout, then the line that makes one. The command line prints it
/// and the tool answers it, so both doors say the same thing about the same record. The computer is named the way
/// every table names it, off the same places reading and the same platform word; a caller with no map says the id.
// projects.ts:20
// export function addedProjectLine(project: ProjectView, named: ReadonlyMap<string, string> | undefined, platform: "darwin" | "linux"): string {
//   return addedProjectOn(project, computerNamed(project.computer, named, platform));
// }
pub fn added_project_line(project: &ProjectView, named: Option<&indexmap::IndexMap<String, String>>, platform: &crate::words::computer::Platform) -> String {
    todo!()
}

/// The same sentence where the computer's word is already in hand, which is the landing's road: the add says how
/// far it has got as it goes and its last stage is this line, so the terminal reads where the project is from the
/// add itself and no door says that fact a second time.
// projects.ts:27
// export function addedProjectOn(project: ProjectView, computer: string): string {
//   const here = copiesFolder(kindForComputer(project.computer));
//   return `${project.name} ${project.id}: ${sourceWord(project.source)} on ${computer}, at ${project.path}${here ? `\nopen a thread on it with: wsp run ${shellQuote(project.name)} "<message>"` : ""}`;
// }
pub fn added_project_on(project: &ProjectView, computer: &str) -> String {
    todo!()
}

/// What the add says before any work runs on that computer: what is being recorded and where its code comes from,
/// and, where nothing of the person's folder travels with it, that nothing is seeded. It counts nothing else: what
/// travels is the seeding's own line in the counts the menu showed, and a count of ticked files here read as the
/// whole of a seed that was also carrying a memory folder.
// projects.ts:36
// export function addingProjectLine(name: string, source: ProjectSource, seeding: boolean): string {
//   return `${name} from ${sourceWord(source)}${seeding ? "" : ", nothing seeded"}.`;
// }
pub fn adding_project_line(name: &str, source: &ProjectSource, seeding: bool) -> String {
    todo!()
}

/// What one word to `wsp add` names: a computer of the person's own over ssh, a repo a computer clones by its own
/// url, `owner/repo` on a host whose signed-in command line the image carries, or a folder a computer holds. Read
/// once here, so the command line, the tool and the runtime cannot each decide for themselves what somebody typed.
/// A word that is none of them throws with the forms.
// projects.ts:44
// export function sourceKind(word: string): "computer" | "git" | "github" | "gitlab" | "folder" {
//   // A path is a path first: /Users/me/repo.git is a folder somebody named that way, not a url.
//   if (word.startsWith("/") || word.startsWith("~") || word.startsWith(".")) return "folder";
//   // A host named in front of the path is that host's, which is how gitlab is named: the bare owner/repo form is
//   // github's, the same word `gh repo clone` itself takes.
//   const host = /^(github|gitlab)\.com\/[^/]+\/[^/]+$/.exec(word);
//   if (host !== null) return host[1] === "gitlab" ? "gitlab" : "github";
//   if (word.includes("://") || /^[\w.-]+@[\w.-]+:/.test(word) || word.endsWith(".git")) return "git";
//   if (/^[\w.-]+@[\w.-]+$/.test(word)) return "computer";
//   if (/^[\w.-]+\/[\w.-]+$/.test(word)) return "github";
//   throw new Error(ADD_FORMS_LINE);
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceKind {
    Computer,
    Git,
    Github,
    Gitlab,
    Folder,
}

/// The TS throw (ADD_FORMS_LINE) is the Err.
pub fn source_kind(word: &str) -> Result<SourceKind, HostError> {
    todo!()
}

// projects.ts:58
// in wsp_wire::types::projects: ADD_FORMS_LINE

/// The one word a source is written as, whichever kind it is: the folder's path, the repo's url, or the
/// `owner/repo` a host's own command line takes. Read by every line that says where a project's code comes from,
/// so no road spells one kind of source two ways.
// projects.ts:64
// export function sourceWord(source: ProjectSource): string {
//   if (source.kind === "folder") return source.path;
//   return source.kind === "git" ? source.url : source.repo;
// }
pub fn source_word(source: &ProjectSource) -> String {
    todo!()
}

/// What a project is called: the repo's last word without .git, or the folder's own name.
// projects.ts:70
// export function projectNameOf(source: ProjectSource): string {
//   if (source.kind === "folder") return folderName(source.path);
//   const word = sourceWord(source);
//   const last = word.replace(/\/+$/, "").split(/[/:]/).pop() ?? word;
//   return last.replace(/\.git$/, "");
// }
pub fn project_name_of(source: &ProjectSource) -> String {
    todo!()
}

/// Where a project's checkout sits inside a workspace of it: the folder itself where the source is one the
/// computer holding it already has, which every workspace there is a copy of, and `<home>/<name>` where the source
/// is a repo that computer cloned into a checkout of its own. Which home is the landing road's answer, since it is the road that knows
/// which folder a copy can be bound at inside a workspace there without leaving the mount point on the computer
/// itself. A road that clones nothing names none, and a repo is refused on such a computer before this is asked.
// projects.ts:82
// export function projectPathOn(source: ProjectSource, name: string, home?: string): string {
//   if (source.kind === "folder") return source.path;
//   if (home === undefined) throw new Error(`${name} is a repo, and that computer holds no checkout of its own to put one in`);
//   return `${home}/${name}`;
// }
/// The TS throw is the Err.
pub fn project_path_on(source: &ProjectSource, name: &str, home: Option<&str>) -> Result<String, HostError> {
    todo!()
}

/// The source one word names, off the kind that word is: the shape the record keeps. A word naming a computer is
/// no project's source and is refused by its caller before this.
// projects.ts:90
// export function projectSourceOf(word: string, kind: Exclude<ReturnType<typeof sourceKind>, "computer">, folderPath?: string): ProjectSource {
//   if (kind === "folder") return { kind, path: folderPath ?? word };
//   if (kind === "git") return { kind, url: word };
//   // owner/repo as the host's own command line takes it: the host in front of it and a trailing .git are the
//   // person's way of writing the same repo, and neither is part of the word that command reads.
//   return { kind, repo: word.replace(/^(github|gitlab)\.com\//, "").replace(/\.git$/, "") };
// }
/// `kind` is never `SourceKind::Computer` (the TS excludes it).
pub fn project_source_of(word: &str, kind: &SourceKind, folder_path: Option<&str>) -> ProjectSource {
    todo!()
}

/// Why a second project on one source on one computer is refused: one source per computer is one project, and a
/// second record of it would give two names to one checkout.
// projects.ts:100
// export const sameSourceRefusal = (name: string, computer: string): string =>
//   `that source is already a project on ${computer}, ${name}; one source on one computer is one project`;
pub fn same_source_refusal(name: &str, computer: &str) -> String {
    todo!()
}

/// Why a repo's url with no computer and no folder named records nothing: it is cloned into a folder here or by
/// another computer that clones, and which is the person's to say, with the other computers that clone.
// projects.ts:105
// export const noComputerForSourceLine = (word: string, computers: readonly string[]): string =>
//   `${word} is a repo: clone it here with --into <folder>${computers.length === 0 ? "" : `, or name the computer that clones it with --on ${computers.join(" | ")}`}`;
pub fn no_computer_for_source_line(word: &str, computers: &[impl AsRef<str>]) -> String {
    todo!()
}

/// Why a repo on this computer records nothing without a folder: the clone goes where the person says.
// projects.ts:109
// export const cloneIntoNeeded = (word: string): string => `${word} is a repo; name the folder to clone it in with --into <folder>`;
pub fn clone_into_needed(word: &str) -> String {
    todo!()
}

/// Why the repo's folder inside the one picked is no place to clone into: the clone would land among files that are
/// not its. `free` is the next folder of the repo's name with a number that does not stand yet.
// projects.ts:113
// export const cloneIntoTakenLine = (folder: string, free: string): string => `${folder} already holds files; clone into ${free}, or pick another folder`;
pub fn clone_into_taken_line(folder: &str, free: &str) -> String {
    todo!()
}

/// Why a file is no place to clone into, with the next free folder of the repo's name where it stands at the
/// repo's own path.
// projects.ts:117
// export const cloneIntoFileLine = (path: string, free?: string): string =>
//   `${path} is a file; ${free === undefined ? "pick a folder to clone the repo in" : `clone into ${free}, or pick another folder`}`;
pub fn clone_into_file_line(path: &str, free: Option<&str>) -> String {
    todo!()
}

/// Why --into beside a computer that is not this one is refused: the folder is on the computer named, and the other
/// computer clones where it keeps its own checkouts.
// projects.ts:122
// export const intoIsHereLine = (here: string): string => `--into is a folder on ${here}; drop --on to clone there, or drop --into to clone on the computer you named`;
pub fn into_is_here_line(here: &str) -> String {
    todo!()
}

// projects.ts:125
// in wsp_wire::types::projects: INTO_TAKES_A_REPO_LINE

// projects.ts:127
// private in the TS: port it as a private helper
// const CLONE_HOST = /^[A-Za-z0-9][A-Za-z0-9.-]*$/;

// A login ssh is handed: nothing it could read as an option, and nothing a shell or a url would have to escape.
// projects.ts:129
// private in the TS: port it as a private helper
// const CLONE_USER = /^[A-Za-z0-9_.][A-Za-z0-9_.-]*$/;

// projects.ts:130
// private in the TS: port it as a private helper
// const SCP_FORM = /^[A-Za-z0-9_.][A-Za-z0-9_.-]*@[A-Za-z0-9][A-Za-z0-9.-]*:[^\s-][^\s]*$/;

// `owner/name` as GitHub and GitLab spell a repo: nothing a shell, a url or a git config key would have to escape.
// projects.ts:132
// in wsp_wire::types::projects: OWNER_REPO

/// Why a repo word is not one this computer clones, or nothing where it is: an https url, an ssh url, git's scp form
/// or owner/repo, with no credentials in it, no login, host or path that git or ssh could read as an option, and no
/// transport of git's that runs a command or reads this computer's own files.
// projects.ts:137
// export function cloneUrlRefusal(word: string): string | undefined {
//   const refused = `${word} is not a repo address wsp clones; give its https or ssh url, or owner/repo`;
//   if (/\s/.test(word) || word.startsWith("-")) return refused;
//   let kind: ReturnType<typeof sourceKind>;
//   try {
//     kind = sourceKind(word);
//   } catch {
//     return refused;
//   }
//   if (kind === "github" || kind === "gitlab") return OWNER_REPO.test(word.replace(/^(github|gitlab)\.com\//, "")) ? undefined : refused;
//   if (kind !== "git") return refused;
//   if (SCP_FORM.test(word)) return undefined;
//   let url: URL;
//   try {
//     url = new URL(word);
//   } catch {
//     return refused;
//   }
//   if (url.protocol !== "https:" && url.protocol !== "ssh:") return refused;
//   if (url.password !== "" || (url.protocol === "https:" && url.username !== "")) return refused;
//   if (url.username !== "" && !CLONE_USER.test(url.username)) return refused;
//   if (!CLONE_HOST.test(url.hostname) || url.pathname.length <= 1) return refused;
//   return undefined;
// }
pub fn clone_url_refusal(word: &str) -> Option<String> {
    todo!()
}

// What a sign-in failure reads like from git, ssh and gh: a private repo answers "not found" to a clone with no
// login rather than admitting it exists.
// projects.ts:164
// private in the TS: port it as a private helper
// const SIGN_IN_FAILURE = /authentication failed|could not read username|terminal prompts disabled|permission denied \(publickey\)|not found|could not resolve to a repository|gh auth login|HTTP 40[13]/i;

/// What a failed clone says: git's own last line, and where it failed for want of a login, how to give it one.
// projects.ts:167
// export function cloneFailedLine(said: string): string {
//   const last = said.trimEnd().split("\n").at(-1)?.trim() ?? "";
//   if (last === "") return "the clone failed and said nothing";
//   return SIGN_IN_FAILURE.test(said) ? `${last}; sign in with gh auth login, or use the repo's ssh url` : last;
// }
pub fn clone_failed_line(said: &str) -> String {
    todo!()
}

/// Why a clone on a machine failed, in one sentence: the host's refusal where its broker refused git, else git's own
/// last line, or where git said nothing past the line it opens a clone with, that the clone ran out of its time or how
/// it ended. git prints the broker's sentence as a `remote: wsp: ` line above its own 403 line, which names no reason.
// projects.ts:176
// export function cloneOnMachineFailedLine(o: { repo: string; on: string; exitCode: number; said: string; limitMs: number }): string {
//   const clone = `the clone of ${o.repo} on ${o.on}`;
//   const host = /^remote: wsp: (.+)$/m.exec(o.said)?.[1]?.trim();
//   if (host !== undefined) return `${clone} failed: ${host}`;
//   const last = lastLine(o.said);
//   if (last !== undefined && !last.startsWith("Cloning into ")) return `${clone} failed: ${last}`;
//   if (o.exitCode === EXEC_DEADLINE_EXIT) return `${clone} did not finish in ${Math.round(o.limitMs / 60_000)} minutes, and git said nothing past its start`;
//   return `${clone} ended with exit ${o.exitCode}, and git said nothing past its start`;
// }
/// The TS object `cloneOnMachineFailedLine` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct CloneFailed {
    pub repo: String,
    pub on: String,
    pub exit_code: i32,
    pub said: String,
    pub limit_ms: f64,
}

pub fn clone_on_machine_failed_line(o: &CloneFailed) -> String {
    todo!()
}

/// Why a folder on this computer seeding a project on another computer needs the person's answer first: what git
/// ignores in that folder is theirs, and nothing of it leaves this computer until they have read the menu and
/// ticked what travels.
// projects.ts:189
// export const seedChoiceNeeded = (folder: string): string =>
//   `${folder} would seed the project on that computer, so what travels is yours to pick: read the menu and add it again with your ticks, or --yes for the ones the catalogue ticks itself`;
pub fn seed_choice_needed(folder: &str) -> String {
    todo!()
}

// projects.ts:193
// in wsp_wire::types::projects: NAME_A_PROJECT_LINE

// projects.ts:196
// in wsp_wire::types::projects: BRANCH_ON_A_THREAD_LINE

// projects.ts:199
// in wsp_wire::types::projects: BRANCH_HERE_ONLY_LINE

// projects.ts:202
// in wsp_wire::types::projects: BRANCH_OR_CWD_LINE

/// A start on this computer for a project another computer holds.
// projects.ts:205
// export const notOnThisComputerLine = (project: string): string => `${project} is not on this computer, so no thread runs in its folder here`;
pub fn not_on_this_computer_line(project: &str) -> String {
    todo!()
}

/// A thread on a computer the person joined naming a project on another computer that no lead of its reaches: a
/// thread elsewhere runs beside it on its own computer, nothing yet carries a thread's work from one computer to
/// another, and the person is the one who can start it there.
// projects.ts:210
// export const childOnAnotherComputerLine = (from: string, to: string): string => `a thread on ${from} cannot start one on ${to} yet; the person can start it there, or name a project on ${from}`;
pub fn child_on_another_computer_line(from: &str, to: &str) -> String {
    todo!()
}

/// A thread on a computer the person joined with a login that is not root: that computer's daemon runs a folder's git
/// and writes its roots file as root, which in a folder another login owns either stops at git's ownership check or
/// runs that login's hooks as root.
// projects.ts:215
// export const placeLoginNotRootLine = (computer: string, user: string): string =>
//   `threads on ${computer} would run as ${user}, and wsp runs threads on a computer you added only as root for now; remove ${computer} and add it again with a root login`;
pub fn place_login_not_root_line(computer: &str, user: &str) -> String {
    todo!()
}

/// A branch named for a thread on a computer the person joined, where wsp makes no worktree yet.
// projects.ts:219
// export const placeBranchLine = (computer: string): string => `wsp makes no worktrees on ${computer} yet; start the thread without a branch, or make one inside the project folder with git worktree add and name it as the folder`;
pub fn place_branch_line(computer: &str) -> String {
    todo!()
}

/// A folder a thread may not run in: only the project folder and the worktrees of its repo are a project's.
// projects.ts:222
// export const cwdOutsideLine = (path: string, project: string): string => `${path} is not in ${project} or a worktree of it; wsp add it first`;
pub fn cwd_outside_line(path: &str, project: &str) -> String {
    todo!()
}

/// A start's folder that is not there: refused before the agent is launched into it, which would only die.
// projects.ts:224
// export const noCwdLine = (path: string): string => `there is no folder at ${path}; name one that exists`;
pub fn no_cwd_line(path: &str) -> String {
    todo!()
}

/// What taking a thread away on this computer takes, the question a delete puts first: the thread's turns and
/// checkpoints, and the worktree wsp made it ran in once no other thread is in it.
// projects.ts:228
// export function threadDeleteQuestion(threadId: string, worktree: string | undefined): string {
//   return worktree === undefined
//     ? `Delete thread ${threadId}?\nIts turns and checkpoints leave this computer; the folder it worked in is left as it is.`
//     : `Delete thread ${threadId}?\nIts turns and checkpoints leave this computer, and the worktree at ${worktree} with it where no other thread is in it; the branch and the project folder are left as they are.`;
// }
pub fn thread_delete_question(thread_id: &str, worktree: Option<&str>) -> String {
    todo!()
}

/// What a thread delete did.
// projects.ts:235
// export function threadDeletedLine(threadId: string, gone: { worktree?: string; threads: number }): string {
//   return gone.worktree === undefined ? `deleted thread ${threadId}` : `deleted thread ${threadId}, the worktree at ${gone.worktree} and ${gone.threads === 1 ? "its one thread" : `its ${gone.threads} threads`}`;
// }
/// `worktree` and `threads` are the TS `gone` object's fields.
pub fn thread_deleted_line(thread_id: &str, worktree: Option<&str>, threads: f64) -> String {
    todo!()
}

/// What a worktree removal says once git took it away.
// projects.ts:240
// export const worktreeRemovedLine = (branch: string): string => `removed the worktree for ${branch}; the branch stays`;
pub fn worktree_removed_line(branch: &str) -> String {
    todo!()
}

// projects.ts:243
// in wsp_wire::types::projects: THREAD_WORKING_LINE

/// A thread on a box taken away alone: it goes with its machine.
// projects.ts:246
// export const threadOnMachineLine = (machine: string): string => `a thread on ${machine} goes with its machine; wsp delete ${machine} takes both`;
pub fn thread_on_machine_line(machine: &str) -> String {
    todo!()
}

// projects.ts:249
// in wsp_wire::types::projects: WORKTREE_BUSY_LINE

// projects.ts:252
// in wsp_wire::types::projects: WORKTREE_FORCE_LINE

/// A folder of a project on this computer named to delete or forget: the verbs take threads there, never the folder.
// projects.ts:255
// export const localFolderRefusal = (name: string): string => `${name} is a project's folder, which delete and forget leave alone`;
pub fn local_folder_refusal(name: &str) -> String {
    todo!()
}

/// A worktree's record on this computer named to delete or forget: a worktree goes by its own verb or with its thread.
// projects.ts:258
// export const localWorktreeRefusal = (name: string): string => `${name} is a worktree of a project on this computer, which delete and forget leave alone`;
pub fn local_worktree_refusal(name: &str) -> String {
    todo!()
}

/// The line a thread is told when the worktree it ran in is gone and its next turn runs in the project folder.
// projects.ts:261
// export const threadMovedLine = (from: string, to: string, fresh: boolean): string =>
//   `its worktree at ${from} is gone, so it goes on in ${to}${fresh ? " in a fresh session of its agent" : ""}`;
pub fn thread_moved_line(from: &str, to: &str, fresh: bool) -> String {
    todo!()
}

/// Why a start on a pull request runs in the worktree holding its branch as that worktree stands.
// projects.ts:265
// export const PR_BEHIND_WORDS = {
//   changed: (number: number, path: string): string => `this worktree is behind pull request #${number} and holds files no commit has, so it was not updated; commit them, then run git pull --ff-only in ${path}`,
//   diverged: (number: number, path: string): string => `this worktree's branch and pull request #${number} have gone different ways, so it was not updated; run git pull --rebase in ${path} to bring them together`,
//   folder: (number: number, path: string): string => `this project folder is behind pull request #${number}, and wsp never moves your own checkout; run git pull --ff-only in ${path} to update it`,
//   notMade: (number: number, path: string): string => `this worktree is behind pull request #${number}, and wsp moves only worktrees it made; run git pull --ff-only in ${path} to update it`,
//   byHead: (number: number, path: string, remote: string): string => `this worktree is behind pull request #${number}; run git pull --ff-only ${remote} pull/${number}/head in ${path} to update it`,
//   unread: (number: number, said: string): string => `pull request #${number} could not be read just now (${said}), so this worktree may be behind it`,
// } as const;
/// The TS PR_BEHIND_WORDS.
pub mod pr_behind_words {
    pub fn changed(number: f64, path: &str) -> String {
        todo!()
    }
    pub fn diverged(number: f64, path: &str) -> String {
        todo!()
    }
    pub fn folder(number: f64, path: &str) -> String {
        todo!()
    }
    pub fn not_made(number: f64, path: &str) -> String {
        todo!()
    }
    pub fn by_head(number: f64, path: &str, remote: &str) -> String {
        todo!()
    }
    pub fn unread(number: f64, said: &str) -> String {
        todo!()
    }
}

/// A worktree removal refused over files no commit holds: they would go with it.
// projects.ts:275
// export const worktreeChangedLine = (files: number): string =>
//   `that worktree has ${files} ${files === 1 ? "file" : "files"} not committed; commit ${files === 1 ? "it" : "them"}, or remove it with --force to lose ${files === 1 ? "it" : "them"}`;
pub fn worktree_changed_line(files: f64) -> String {
    todo!()
}

/// Why a settled worktree wsp made still stands: files no commit holds, a turn running in it, or a week of either,
/// after which it is the person's to remove.
// projects.ts:280
// export const keptChangedLine = (files: number): string => `${files} ${files === 1 ? "file" : "files"} not committed`;
pub fn kept_changed_line(files: f64) -> String {
    todo!()
}

// projects.ts:281
// in wsp_wire::types::projects: KEPT_RUNNING_LINE

// projects.ts:282
// in wsp_wire::types::projects: KEPT_ABANDONED_LINE

/// A command a new worktree runs once before its threads start, a module's install or the project's own, said as it
/// starts, since a thread waits on it.
// projects.ts:286
// export const worktreeCommandRunningLine = (command: string, path: string): string => `${command} runs in the new worktree at ${path} before its threads start`;
pub fn worktree_command_running_line(command: &str, path: &str) -> String {
    todo!()
}

/// How one such command ended where it did not end well: its exit code and its last line, or the minutes it ran
/// before it was stopped.
// projects.ts:290
// export type WorktreeCommandEnd = { exitCode: number; said: string } | { stoppedAfterMin: number };
#[derive(Clone, Debug, PartialEq)]
pub enum WorktreeCommandEnd {
    Exited { exit_code: i32, said: String },
    Stopped { stopped_after_min: f64 },
}

// projects.ts:291
// private in the TS: port it as a private helper
// const endWords = (end: WorktreeCommandEnd): string =>
//   "stoppedAfterMin" in end ? `was stopped after ${end.stoppedAfterMin} minutes` : `exited ${end.exitCode}${end.said === "" ? "" : `: ${end.said}`}`;

/// One that did not end well; the threads there start all the same.
// projects.ts:295
// export const worktreeCommandFailedLine = (command: string, path: string, end: WorktreeCommandEnd): string =>
//   `${command} in the new worktree at ${path} ${endWords(end)}; threads there start without it`;
pub fn worktree_command_failed_line(command: &str, path: &str, end: &WorktreeCommandEnd) -> String {
    todo!()
}

/// One command as the first thread in a new worktree reads it: where it ran when not at the top, and how it ended
/// when not well.
// projects.ts:300
// export const worktreeStepWords = (command: string, folder: string, end?: WorktreeCommandEnd): string =>
//   `${command}${folder === "." ? "" : ` in ${folder}`}${end === undefined ? "" : ` (${endWords(end)})`}`;
pub fn worktree_step_words(command: &str, folder: &str, end: Option<&WorktreeCommandEnd>) -> String {
    todo!()
}

/// The line the first thread in a new worktree opens with: what the worktree ran before the thread started, one
/// command to a line, since a command can hold any separator a joined sentence would use.
// projects.ts:305
// export const worktreeSetupLine = (steps: readonly string[]): string => ["Before this thread started, its new worktree ran:", ...steps.map(step => `- ${step}`)].join("\n");
pub fn worktree_setup_line(steps: &[impl AsRef<str>]) -> String {
    todo!()
}

/// A removal asked of a branch with no worktree wsp made: a worktree the person or an agent made is theirs to remove.
// projects.ts:308
// export const notMadeWorktreeLine = (branch: string): string => `wsp made no worktree for ${branch}; git worktree remove takes one wsp did not make`;
pub fn not_made_worktree_line(branch: &str) -> String {
    todo!()
}

/// Why an old copy of a project folder was left on disk when the host moved off copies; one line per copy in the
/// file the move writes. Nothing is removed that holds a change no commit has or a branch the project could not take.
// projects.ts:312
// export const OLD_COPY_WORDS = {
//   changed: "it has changes no commit holds",
//   stashed: "it holds stashed changes",
//   noProject: "its project is no longer recorded",
//   isProject: "it is the project folder itself",
//   noCopier: "this host has no daemon binary to remove it with",
//   unread: (said: string): string => `git could not read it: ${said}`,
//   fetchFailed: (said: string): string => `the fetch of its branches into the project failed: ${said}`,
//   notRemoved: (said: string): string => `it could not be removed: ${said}`,
//   /** Why a turn still running on an old copy ended at the move. */
//   ended: "the host moved off copies, so this copy's threads end here",
//   /** What the person is told once, with the file that lists each copy kept. */
//   kept: (copies: number, file: string): string => `${copies} old ${copies === 1 ? "copy" : "copies"} kept with work in ${copies === 1 ? "it" : "them"}; see ${file}`,
// } as const;
/// The TS OLD_COPY_WORDS: fixed words as constants, sentences as functions.
pub mod old_copy_words {
    pub const CHANGED: &str = "it has changes no commit holds";
    pub const STASHED: &str = "it holds stashed changes";
    pub const NO_PROJECT: &str = "its project is no longer recorded";
    pub const IS_PROJECT: &str = "it is the project folder itself";
    pub const NO_COPIER: &str = "this host has no daemon binary to remove it with";
    pub const ENDED: &str = "the host moved off copies, so this copy's threads end here";
    pub fn unread(said: &str) -> String {
        todo!()
    }
    pub fn fetch_failed(said: &str) -> String {
        todo!()
    }
    pub fn not_removed(said: &str) -> String {
        todo!()
    }
    pub fn kept(copies: f64, file: &str) -> String {
        todo!()
    }
}

/// A branch asked of a project whose folder git holds no repo in.
// projects.ts:328
// export const noBranchesLine = (project: string): string => `${project} is not a git repo, so it has no branches`;
pub fn no_branches_line(project: &str) -> String {
    todo!()
}

/// Why a folder on a computer that is not this one records nothing: nothing carries a folder there yet, so its
/// project is the repo that computer can clone.
// projects.ts:332
// export const folderOnCopyRefusal = (computer: string): string =>
//   `${computer} takes a repo it can clone, not a folder on this computer; give the repo's url, or add the folder here with no --on`;
pub fn folder_on_copy_refusal(computer: &str) -> String {
    todo!()
}

/// Why a cloud records no project with the cloud off: a project and its threads on a cloud's machines are the cloud's
/// top-level road, which only a host started with the cloud on takes.
// projects.ts:337
// export const cloudProjectRefusal = (cloud: string): { happened: string; fix: string } => ({
//   happened: `${cloud} holds no project of its own on this host`,
//   fix: "Record the project on this computer or on a computer you added.",
// });
pub fn cloud_project_refusal(cloud: &str) -> crate::place_word::RefusalHalves {
    todo!()
}

/// What a remove says on the computer the app runs on: the folder is the project, so nothing of it moves.
// projects.ts:343
// export const projectRemovedHereLine = (name: string): string => `${name} is no longer a project here; its code is where it was`;
pub fn project_removed_here_line(name: &str) -> String {
    todo!()
}

// projects.ts:349
// in wsp_wire::types::projects: MEMORY_KEPT_CLAUSE

/// What a remove says on a computer the person joined: the project is a folder in their home there, which they and
/// their threads work in, so it stays as it is and only the record goes.
// projects.ts:353
// export const projectLeftOnComputerLine = (name: string, computer: string, folder: string): string => `${name} is no longer a project on ${computer}; its folder ${folder} stays there as it is`;
pub fn project_left_on_computer_line(name: &str, computer: &str, folder: &str) -> String {
    todo!()
}

/// What a remove says for a project a provider keeps in an image: nothing runs on any machine, and the image
/// stays where it is, since no verb deletes one yet.
// projects.ts:357
// export const projectRemovedAtProviderLine = (name: string, computer: string, snapshotId?: string): string =>
//   `${name} is no longer a project on ${computer}; ${snapshotId === undefined ? "nothing of it was held there" : `its project image ${snapshotId} stays at the provider`}`;
pub fn project_removed_at_provider_line(name: &str, computer: &str, snapshot_id: Option<&str>) -> String {
    todo!()
}

/// What the seeding says where the computer already keeps this project's memory at the path its agent reads:
/// that memory is the agent's own work on that computer and stays, so the folder the seed carried is not landed
/// over it.
// projects.ts:363
// export const seedMemoryKeptLine = (computer: string): string =>
//   `${computer} already keeps this project's memory where its agent reads it, so what the agent has kept there stays and the memory from your folder was not landed over it`;
pub fn seed_memory_kept_line(computer: &str) -> String {
    todo!()
}

/// Why a project cannot be dropped yet, with the workspaces standing on it.
// projects.ts:367
// export const projectInUseRefusal = (name: string, workspaces: readonly string[]): string =>
//   `${name} has ${workspaces.length === 1 ? "a workspace" : "workspaces"} standing on it: ${workspaces.join(", ")}; delete ${workspaces.length === 1 ? "its threads" : "their threads"} first`;
pub fn project_in_use_refusal(name: &str, workspaces: &[impl AsRef<str>]) -> String {
    todo!()
}

/// The whole of what a caller held to one project reads about a word naming another: absence and nothing else,
/// since the names of what else this host holds are not a thread's to learn from a refusal.
// projects.ts:372
// export const bareNoSuchProjectLine = (ref: string): string => `no project ${JSON.stringify(ref)}`;
pub fn bare_no_such_project_line(r#ref: &str) -> String {
    todo!()
}

/// Why a word names no project here, with the ones it could have named.
// projects.ts:375
// export const noSuchProjectLine = (ref: string, names: readonly string[]): string =>
//   `${bareNoSuchProjectLine(ref)}; ${names.length === 0 ? "wsp add <folder> records one" : `this host holds ${names.join(", ")}`}`;
pub fn no_such_project_line(r#ref: &str, names: &[impl AsRef<str>]) -> String {
    todo!()
}

/// Why a workspace cannot be made without naming its project, with the projects to name.
// projects.ts:379
// export const nameTheProjectLine = (names: readonly string[]): string =>
//   `name the project this work is on: ${names.join(", ")}`;
pub fn name_the_project_line(names: &[impl AsRef<str>]) -> String {
    todo!()
}

/// A path as the machine's own shell would show it: `~` for its home and anything under it, the path as given
/// elsewhere or where the home is not known.
// projects.ts:384
// export function homeShortened(path: string, home: string | undefined): string {
//   if (home === undefined || !underProject(path, home)) return path;
//   return path === home ? "~" : `~${path.slice(home.length)}`;
// }
pub fn home_shortened(path: &str, home: Option<&str>) -> String {
    todo!()
}

// projects.ts:390
// in wsp_wire::types::projects: REGISTERING_LINE

/// What a register says when it has landed.
// projects.ts:393
// export function registeredLine(dest: string): string {
//   return `${folderName(dest)} registered at ${dest}; nothing was copied.`;
// }
pub fn registered_line(dest: &str) -> String {
    todo!()
}

/// Whether a project image already holds this project's checkout, so a fork of it clones and installs nothing: the
/// image records every project that was on the disk and the path each sat at, and a fork of the whole disk comes up
/// with all of them where they were. Read off the snapshot a fork names and never off the project's own record,
/// which says nothing about which image a given workspace was forked from.
// projects.ts:401
// export function imageCarriesCheckout(projects: readonly WorkspaceProject[] | undefined, path: string): boolean {
//   return projects !== undefined && projects.some(p => p.dest === path);
// }
pub fn image_carries_checkout(projects: Option<&[WorkspaceProject]>, path: &str) -> bool {
    todo!()
}

/// What a fork of a project golden starts with, after the created line; nothing for an image carrying none.
// projects.ts:406
// export function projectsInPlace(projects: readonly WorkspaceProject[]): string {
//   return projects.length === 0 ? "" : ` with ${projects.map(p => p.name).join(", ")} in place`;
// }
pub fn projects_in_place(projects: &[WorkspaceProject]) -> String {
    todo!()
}

/// The name a fork of a project golden goes by: the newest project it carries, which is also the name `new --from`
/// takes for it; the snapshot id where a manifest carries none.
// projects.ts:412
// export function goldenForkName(g: { projects: readonly WorkspaceProject[]; snapshotId: string }): string {
//   return g.projects.at(-1)?.name ?? g.snapshotId;
// }
/// `projects` and `snapshot_id` are the TS object's fields.
pub fn golden_fork_name(projects: &[WorkspaceProject], snapshot_id: &str) -> String {
    todo!()
}

/// Why an import onto this computer takes no consent flags: nothing is carried, cut, sent or replaced by a register.
// projects.ts:417
// export function registerTakesNoConsentLine(flags: readonly string[]): string {
//   return `${flags.join(", ")} ${flags.length === 1 ? "has" : "have"} no meaning on this computer: the folder is registered at its path and nothing is carried, cut or replaced`;
// }
pub fn register_takes_no_consent_line(flags: &[impl AsRef<str>]) -> String {
    todo!()
}

/// The first line of a thread opened with no folder named outright: the project, where the caller named one, and the
/// folder it went to.
// projects.ts:423
// export function threadOpenedLine(threadId: string, project: string | undefined, folder: string): string {
//   return project === undefined ? `thread ${threadId} in ${folder}` : `thread ${threadId} on ${project} in ${folder}`;
// }
pub fn thread_opened_line(thread_id: &str, project: Option<&str>, folder: &str) -> String {
    todo!()
}

/// Whether a workspace of this kind is a copy of a folder on this computer, made by directory, and forks no image:
/// the one reading of the kind table both the create and the folder rule take.
// projects.ts:429
// export function copiesFolder(kind: WorkspaceKind): boolean {
//   return kindWords(kind).copiesFolder;
// }
pub fn copies_folder(kind: &WorkspaceKind) -> bool {
    todo!()
}

/// Whether a thread on a workspace of this kind runs in its project's folder, so a start makes nothing: the reading
/// of the kind table the create, the folder rule and the tree rule take.
// projects.ts:435
// export function runsInFolder(kind: WorkspaceKind): boolean {
//   return kindWords(kind).inFolder;
// }
pub fn runs_in_folder(kind: &WorkspaceKind) -> bool {
    todo!()
}

/// Whether a workspace of this kind is a folder on a computer the person joined, reached over that computer's link:
/// its ports open here on demand, its threads' processes are told apart by cgroup, and nothing of a machine wsp
/// made, a pause, a bring back or an export, applies to it.
// projects.ts:442
// export function folderOnJoined(kind: WorkspaceKind): boolean {
//   return kindWords(kind).inFolder && !kindWords(kind).copiesFolder;
// }
pub fn folder_on_joined(kind: &WorkspaceKind) -> bool {
    todo!()
}

/// Whether a record is a folder's that no thread names: the host makes one before a folder's first thread and keeps
/// it past its last, so nothing shows it and nothing counts it as standing on its project.
// projects.ts:448
// export function bareFolder(record: { readonly kind?: WorkspaceKind | undefined }, holdsThread: boolean): boolean {
//   return !holdsThread && runsInFolder(workspaceKind(record));
// }
/// `kind` is the record's own field.
pub fn bare_folder(kind: Option<&WorkspaceKind>, holds_thread: bool) -> bool {
    todo!()
}

/// The kind of workspace a computer makes: the computer the app runs on and every computer the person joined run a
/// thread in the project's folder there, and a cloud account forks a copy of its image. `joined` says whether the
/// computer is one the person joined, which only a reader of the places can know; a caller asking only whether the
/// kind is this computer's leaves it out. The one place a computer's id is read for its kind, so no road anywhere
/// compares that id itself.
// projects.ts:457
// export function kindForComputer(computer: string, joined = false): WorkspaceKind {
//   return computer === HERE_PLACE_ID ? "local" : joined ? "place" : "cloud";
// }
pub fn kind_for_computer(computer: &str, joined: Option<bool>) -> WorkspaceKind {
    todo!()
}

/// Where a workspace of a project on this computer lands: a copy of a folder beside the app for a project here, a
/// fork on the host's own backend for a project on the provider it forks on (`wired`), and a fork on the place it
/// names for every other one. The one rule the host forks by and a computer's page reads its projects by.
// projects.ts:464
// export type WorkspaceLands = { at: "here" } | { at: "wired" } | { at: "place"; place: string };
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(tag = "at", rename_all = "lowercase")]
pub enum WorkspaceLands {
    Here,
    Wired,
    Place { place: String },
}

// projects.ts:466
// export function workspaceLands(computer: string, wired: string | undefined): WorkspaceLands {
//   if (copiesFolder(kindForComputer(computer))) return { at: "here" };
//   return computer === wired ? { at: "wired" } : { at: "place", place: computer };
// }
pub fn workspace_lands(computer: &str, wired: Option<&str>) -> WorkspaceLands {
    todo!()
}

/// Whether a workspace of a project on `computer` stands on this place's row. The provider the host forks on is a
/// row under its own word, so which provider that is changes nothing here.
// projects.ts:473
// export function landsOn(computer: string, place: { id: string; name: string }): boolean {
//   return workspaceLands(computer, undefined).at === "here" ? place.id === HERE_PLACE_ID : namesPlace(place, computer);
// }
/// `place_id` and `place_name` are the place's id and name.
pub fn lands_on(computer: &str, place_id: &str, place_name: &str) -> bool {
    todo!()
}

/// Why a project on this computer takes none of the words a fork takes: its threads run in its own folder, so there
/// is no image to start from, no machine to size and no engine to hand it. The words are named in the order the
/// command line lists them, so the sentence says exactly which to drop.
// projects.ts:480
// export const copyTakesNone = (project: string, words: readonly string[], computer: string = THIS_COMPUTER): string =>
//   `${project} is on ${computer}, where threads run in its folder and nothing forks, so it takes no ${words.join(", ")}`;
pub fn copy_takes_none(project: &str, words: &[impl AsRef<str>], computer: Option<&str>) -> String {
    todo!()
}

/// What a computer is called in a row or a line: the name this wsp holds for it, and this computer's own word
/// where the record names this one. `named` is the places table by id, which every caller already reads for its
/// other columns, and `platform` is the computer the host runs on, since its own word is Mac or computer and no
/// default can know which. The one reading, so a project's row, a workspace's row and the line a recorded project
/// answers with can never print one computer three ways.
// projects.ts:488
// export function computerNamed(computer: string, named: ReadonlyMap<string, string> | undefined, platform: "darwin" | "linux"): string {
//   return computer === HERE_PLACE_ID ? thisComputer(platform) : (named?.get(computer) ?? computer);
// }
pub fn computer_named(computer: &str, named: Option<&indexmap::IndexMap<String, String>>, platform: &crate::words::computer::Platform) -> String {
    todo!()
}

/// Why a thread opened with no project named, from a folder that is in no project, opens nothing; `named` is the
/// caller's word for naming one (`<project>` on the command line, `project` on the tool).
// projects.ts:494
// export function noThreadTargetLine(named: string): string {
//   return `${named} is needed outside your projects: run from inside a project's folder, or name the project`;
// }
pub fn no_thread_target_line(named: &str) -> String {
    todo!()
}

