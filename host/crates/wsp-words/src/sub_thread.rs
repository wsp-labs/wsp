// SPDX-License-Identifier: AGPL-3.0-only
//! A sub-thread on a machine of its own: the sizes it may take, the preamble that tells its agent how to get the
//! code and hand it back, the strip and spawn words a client shows, and the lines and refusals of its end. Port of
//! the functions of packages/protocol/src/sub-thread.ts at a3b870a6e; its shapes and flat constants are
//! wsp_wire::types::sub_thread. See spec/sub_thread.md.

use wsp_wire::views::sub_row::SubResult;

use serde::Serialize;

use crate::exit::HostError;

// The most a sub-thread's picked files may come to, packed: the files worth picking are a few KB of `.env` style
// configs, so anything past this is a data folder picked by mistake.
// sub-thread.ts:19
// in wsp_wire::types::sub_thread: SUB_SEED_MAX_BYTES

// sub-thread.ts:21
// private in the TS: port it as a private helper
// const count = z.number().int().nonnegative();

// What a sub-thread's start answers beside the thread: the machine, the branches and the files sent.
// sub-thread.ts:24
// in wsp_wire::types::sub_thread: SubOut

// sub-thread.ts:36
// in wsp_wire::types::sub_thread: SubOut

// What the host answers a sub-thread's machine with, for the start that opens the thread on it: the answer's `sub`,
// the folder the thread opens in, the preamble above its task and the agent the checks read.
// sub-thread.ts:40
// in wsp_wire::types::sub_thread: SubStarted

// sub-thread.ts:41
// in wsp_wire::types::sub_thread: SubStarted

/// Why a word beside --on is refused, the word a flag or a tool input: the sub-thread runs in a folder of its own on the
/// cloud, under the asking thread.
// sub-thread.ts:45
// export const subBesideRefusal = (word: string): Error =>
//   refusal(`--on starts a sub-thread of the thread asking, in a folder of its own on its own machine, so it cannot also take ${word}`, `Drop ${word}, or run the work without --on.`, "usage");
pub fn sub_beside_refusal(word: &str) -> crate::exit::HostError {
    todo!()
}

/// A project named beside --on that is not the asking thread's: the sub-thread carries that thread's project alone.
// sub-thread.ts:49
// export const subOtherProjectRefusal = (on: string, word: string, carried: string): Error =>
//   refusal(`run --on ${on} starts a sub-thread of the thread asking, on that thread's project ${carried}, and ${word} is another project`, `Leave the project out, or run the work on ${word} without --on.`, "usage");
pub fn sub_other_project_refusal(on: &str, word: &str, carried: &str) -> crate::exit::HostError {
    todo!()
}

/// The line ahead of the next message of a thread that started before a cloud was added: its agent read the computers
/// and its tool words at its start and keeps them.
// sub-thread.ts:54
// export const cloudAddedLine = (name: string): string => `${name} was added since this thread started: run with on ${name.toLowerCase()} starts a sub-thread there.`;
pub fn cloud_added_line(name: &str) -> String {
    todo!()
}

/// The same for a cloud taken away.
// sub-thread.ts:57
// export const cloudRemovedLine = (name: string): string => `${name} was removed since this thread started: run with on ${name.toLowerCase()} starts nothing there now.`;
pub fn cloud_removed_line(name: &str) -> String {
    todo!()
}

/// One size a cloud sells a sub-thread, by its class's own name.
// sub-thread.ts:60
// export interface SubSize {
//   name: string;
//   cpu: number;
//   memMb: number;
//   rateUsdPerHour: number;
// }
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubSize {
    pub name: String,
    pub cpu: f64,
    pub mem_mb: f64,
    pub rate_usd_per_hour: f64,
}

/// What every size function reads of a size: its cpus and memory, and the name a plan gave it where it has one.
pub trait SizeFacts {
    fn cpu(&self) -> f64;
    fn mem_mb(&self) -> f64;
    fn name(&self) -> Option<&str> {
        None
    }
}

impl SizeFacts for SubSize {
    fn cpu(&self) -> f64 {
        self.cpu
    }
    fn mem_mb(&self) -> f64 {
        self.mem_mb
    }
    fn name(&self) -> Option<&str> {
        Some(&self.name)
    }
}

/// The TS `{ cpu, memMb }`.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuMem {
    pub cpu: f64,
    pub mem_mb: f64,
}

impl SizeFacts for CpuMem {
    fn cpu(&self) -> f64 {
        self.cpu
    }
    fn mem_mb(&self) -> f64 {
        self.mem_mb
    }
}

/// A size as a sub-thread names it.
// sub-thread.ts:68
// export const subSizeWord = (size: { cpu: number; memMb: number }): string => sizeWord(size);
pub fn sub_size_word(size: &CpuMem) -> String {
    todo!()
}

/// The size a word names, or nothing where it is not `<cpu>x<memGb>`.
// sub-thread.ts:71
// export const subSizeOf = (word: string): { cpu: number; memMb: number } | undefined => sizeFromWord(word);
pub fn sub_size_of(word: &str) -> Option<CpuMem> {
    todo!()
}

/// A size in the words the preamble and the card say it in: `small, 2 vCPU, 4 GB`.
// sub-thread.ts:74
// export const subSizeWords = (size: Pick<SubSize, "name" | "cpu" | "memMb">): string => `${size.name}, ${size.cpu} vCPU, ${fmtMemGb(size.memMb)}`;
pub fn sub_size_words(size: &SubSize) -> String {
    todo!()
}

// sub-thread.ts:76
// private in the TS: port it as a private helper
// const rateWord = (usdPerHour: number): string => `$${usdPerHour.toFixed(3)}/h`;

/// A size by the class name its row carries, or by its figures where it carries none.
// sub-thread.ts:79
// export const sizeNameOf = (size: { cpu: number; memMb: number; name?: string | undefined }): string => size.name ?? sizeWord(size);
pub fn size_name_of(size: &impl SizeFacts) -> String {
    todo!()
}

/// The size a sub-thread killed for memory starts again at: the smallest its cloud sells with more memory than it had,
/// or none where the cloud sells nothing bigger. What the kill's line names and the strip's Start again asks.
// sub-thread.ts:83
// export function subNextSize<S extends { cpu: number; memMb: number }>(size: { memMb: number }, sold: readonly S[]): S | undefined {
//   return [...sold].filter(s => s.memMb > size.memMb).sort((a, b) => a.memMb - b.memMb)[0];
// }
/// `mem_mb` is the TS `size.memMb`.
pub fn sub_next_size<S: SizeFacts + Clone>(mem_mb: f64, sold: &[S]) -> Option<S> {
    todo!()
}

/// The sizes a cloud's row says it sells a sub-thread now: those its plan names, or every size it lists before a
/// limits read has answered.
// sub-thread.ts:89
// export const subSizesSold = <S extends { cpu: number; memMb: number }>(sizes: readonly S[], plan: { sizes: readonly string[] } | undefined): S[] =>
//   sizes.filter(s => plan === undefined || plan.sizes.includes(sizeWord(s)));
/// `plan_sizes` is the TS `plan.sizes`, None for no plan.
pub fn sub_sizes_sold<S: SizeFacts + Clone>(sizes: &[S], plan_sizes: Option<&[&str]>) -> Vec<S> {
    todo!()
}

/// The folder a sub-thread works in on its machine: the repository's own name off its remote, under root's home.
// sub-thread.ts:93
// export function subFolderOf(remote: string): string {
//   const name = (ownerRepoOf(remote) ?? remote).split("/").at(-1)!.replace(/\.git$/, "");
//   return `/root/${name}`;
// }
pub fn sub_folder_of(remote: &str) -> String {
    todo!()
}

/// The branch a sub-thread pushes when its start names none: `wsp/` and the machine's name as a slug.
// sub-thread.ts:99
// export function subBranchOf(machine: string): string {
//   const slug = machine.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 48).replace(/-+$/, "");
//   return `wsp/${slug === "" ? "sub-thread" : slug}`;
// }
pub fn sub_branch_of(machine: &str) -> String {
    todo!()
}

/// Whether git takes a name for a branch, by `git check-ref-format --branch`'s rules: never HEAD, no leading `-` or
/// `/`, no trailing `/` or `.`, no `//`, `..` or `@{`, no control character, space or any of `~^:?*[\`, and no part
/// that starts with `.` or ends with `.lock`.
// sub-thread.ts:107
// export function subBranchNameOk(name: string): boolean {
//   if (name === "" || name === "HEAD" || name.startsWith("-") || name.startsWith("/") || name.endsWith("/") || name.endsWith(".")) return false;
//   if (/[\x00-\x20\x7f~^:?*[\\]/.test(name) || name.includes("..") || name.includes("//") || name.includes("@{")) return false;
//   return name.split("/").every(part => part !== "" && !part.startsWith(".") && !part.endsWith(".lock"));
// }
pub fn sub_branch_name_ok(name: &str) -> bool {
    todo!()
}

// The git lines that put a sub-thread's folder on its branch. Where the host keeps the machine's credentials, the
// remote tracks the branch alone and the one commit to start from is fetched at depth 1 by its id, which serves a
// branch, a tag and a bare commit alike. The refspec is a pattern on the branch's name, since a plain fetch refuses a
// refspec naming a branch the remote does not hold yet, and a wider one would pull every branch. Elsewhere the
// image's login is pointed at git and the start's branch fetched without blobs.
// sub-thread.ts:118
// private in the TS: port it as a private helper
// function subGitLines(o: { setupGit: string; repo: string; branch: string; brokered?: true; fetch: { ref: string; blobless: string; checkout: string } }): string[] {
//   if (o.brokered === true) return ["git init -q", `git remote add -t ${shellQuote(`${o.branch}*`)} origin ${o.repo}`, `git fetch -q --depth 1 origin ${o.fetch.ref}`, `git checkout -q -b ${o.branch} FETCH_HEAD`];
//   return [o.setupGit, "git init -q", `git remote add origin ${o.repo}`, `git fetch -q --filter=blob:none origin ${o.fetch.blobless}`, `git checkout -q -b ${o.branch} ${o.fetch.checkout}`];
// }

// sub-thread.ts:124
// in wsp_wire::types::sub_thread: SUB_SHALLOW_LINE

/// The first message's head on a sub-thread: where it runs, the git lines that fetch the lead's pushed commit onto a
/// branch of its own, and the rule to push that branch before the turn ends. Names the files sent, never their
/// contents. The task follows it after a blank line.
// sub-thread.ts:129
// export function subPreamble(o: {
//   on: string;
//   size: SubSize;
//   lead: string;
//   folder: string;
//   sent: readonly string[];
//   setupGit: string;
//   repo: string;
//   base: { branch: string; sha: string };
//   branch: string;
//   deleteAfterMinutes: number;
//   /** The host keeps the machine's credentials, so its clone is one commit deep. */
//   brokered?: true;
// }): string {
//   const lead = o.lead.slice(0, 8);
//   const folder = folderWords(o.folder, o.sent);
//   const deleted = deletedWords(o.deleteAfterMinutes);
//   return [
//     `This thread runs on a machine of its own on ${o.on} (${subSizeWords(o.size)}), started by thread ${lead}. The code is not here yet. ${folder} Get the code first, in this folder:`,
//     "",
//     ...subGitLines({ ...o, fetch: { ref: o.base.sha, blobless: o.base.branch, checkout: o.base.sha } }),
//     "",
//     `Work on ${o.branch}, then commit and run git push -u origin ${o.branch} before your turn ends. Thread ${lead} merges that branch. This machine is deleted ${deleted}, and anything not pushed is lost with it.${o.brokered === true ? ` ${SUB_SHALLOW_LINE}` : ""}`,
//     "",
//     "The task:",
//   ].join("\n");
// }
/// The TS `{ branch, sha }` a sub-thread starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BaseAt {
    pub branch: String,
    pub sha: String,
}

/// The TS object `subPreamble` reads; `brokered` is the TS `true` or absent.
#[derive(Clone, Debug, PartialEq)]
pub struct SubPreamble {
    pub on: String,
    pub size: SubSize,
    pub lead: String,
    pub folder: String,
    pub sent: Vec<String>,
    pub setup_git: String,
    pub repo: String,
    pub base: BaseAt,
    pub branch: String,
    pub delete_after_minutes: f64,
    pub brokered: Option<bool>,
}

pub fn sub_preamble(o: &SubPreamble) -> String {
    todo!()
}

// The folder a sub-thread's machine opens in, as its preamble says it: empty, or holding the files picked.
// sub-thread.ts:158
// private in the TS: port it as a private helper
// const folderWords = (folder: string, sent: readonly string[]): string =>
//   sent.length === 0 ? `The folder you are in, ${folder}, is empty.` : `The folder you are in, ${folder}, holds only the git-ignored files the person picked: ${sent.join(", ")}.`;

// When a sub-thread's machine goes, as its preamble says it.
// sub-thread.ts:162
// private in the TS: port it as a private helper
// const deletedWords = (minutes: number): string => (minutes === 0 ? "as soon as your last turn ends" : `${minutes} minutes after your last turn ends`);

/// The head of a send into a sub-thread whose machine was deleted, on the new machine it runs on now: the conversation
/// came and the code did not, so the git lines get its pushed branch back, or start it again from the lead's commit
/// where it pushed nothing. The message follows it after a blank line.
// sub-thread.ts:167
// export function subResumePreamble(o: {
//   on: string;
//   size: Pick<SubSize, "name" | "cpu" | "memMb">;
//   folder: string;
//   sent: readonly string[];
//   setupGit: string;
//   repo: string;
//   base: { branch: string; sha: string };
//   branch: string;
//   pushed: boolean;
//   deleteAfterMinutes: number;
//   brokered?: true;
// }): string {
//   const fetch = o.pushed ? { ref: o.branch, blobless: o.branch, checkout: "FETCH_HEAD" } : { ref: o.base.sha, blobless: o.base.branch, checkout: o.base.sha };
//   const push = o.pushed ? `Push to ${o.branch} as before.` : `Work on ${o.branch}, then commit and run git push -u origin ${o.branch} before your turn ends.`;
//   return [
//     `This thread's machine was deleted after its last turn, and the thread goes on here, on a new machine on ${o.on} (${subSizeWords(o.size)}). Your conversation came with it; the code did not. ${folderWords(o.folder, o.sent)} Get your branch back first, in this folder:`,
//     "",
//     ...subGitLines({ ...o, fetch }),
//     "",
//     `${push} This machine is deleted ${deletedWords(o.deleteAfterMinutes)}.${o.brokered === true ? ` ${SUB_SHALLOW_LINE}` : ""}`,
//     "",
//     "The message:",
//   ].join("\n");
// }
/// The TS object `subResumePreamble` reads; the size's rate is not read.
#[derive(Clone, Debug, PartialEq)]
pub struct SubResumePreamble {
    pub on: String,
    pub size: SubSize,
    pub folder: String,
    pub sent: Vec<String>,
    pub setup_git: String,
    pub repo: String,
    pub base: BaseAt,
    pub branch: String,
    pub pushed: bool,
    pub delete_after_minutes: f64,
    pub brokered: Option<bool>,
}

pub fn sub_resume_preamble(o: &SubResumePreamble) -> String {
    todo!()
}

/// A sub-thread's session past what the host keeps, as its row says it.
// sub-thread.ts:194
// export const subSessionTooLargeLine = (size: string): string => `the agent's session was too large to keep (${size}); a send continues from the transcript`;
pub fn sub_session_too_large_line(size: &str) -> String {
    todo!()
}

/// A sub-thread's session the host could not keep, as its row says it.
// sub-thread.ts:196
// export const subSessionNotKeptLine = (why: string): string => `the agent's session was not kept (${why}); a send continues from the transcript`;
pub fn sub_session_not_kept_line(why: &str) -> String {
    todo!()
}

// sub-thread.ts:199
// in wsp_wire::types::sub_thread: SUB_TURN_GONE_LINE

/// The first message whole: the preamble, a blank line, and the lead's task as given.
// sub-thread.ts:202
// export const subOpening = (preamble: string, task: string): string => `${preamble}\n\n${task}`;
pub fn sub_opening(preamble: &str, task: &str) -> String {
    todo!()
}

// sub-thread.ts:204
// private in the TS: port it as a private helper
// const usage = (happened: string, fix: string): Error => refusal(happened, fix, "usage");

// sub-thread.ts:205
// private in the TS: port it as a private helper
// const provider = (happened: string, fix: string): Error => refusal(happened, fix);

/// S1: a sub-thread is a child, so a line out of no thread has no lead to notify or merge into.
// sub-thread.ts:208
// export const subNoThreadRefusal = (on: string): Error =>
//   usage(`run --on ${on} starts a sub-thread of the thread that asks, and this line came from no thread`, "Start it from a thread, or run the work on this computer with wsp run <project>.");
pub fn sub_no_thread_refusal(on: &str) -> crate::exit::HostError {
    todo!()
}

/// `size` from a terminal with no cloud named.
// sub-thread.ts:211
// export const subSizeNoCloudRefusal = (): Error => usage("--size sizes a sub-thread's machine, and this line starts none", "Add --on boat.");
pub fn sub_size_no_cloud_refusal() -> crate::exit::HostError {
    todo!()
}

/// `from` from a terminal with no cloud named.
// sub-thread.ts:213
// export const subFromNoCloudRefusal = (): Error => usage("--from names the commit a sub-thread's machine starts from, and this line starts none", "Add --on boat.");
pub fn sub_from_no_cloud_refusal() -> crate::exit::HostError {
    todo!()
}

/// A lead runs on this computer or on a computer the person added, never on a machine wsp forked.
// sub-thread.ts:215
// export const subLeadOnMachineRefusal = (on: string, thread: string, machine: string): Error =>
//   usage(`run --on ${on} starts a sub-thread of a thread on this computer or on a computer you added, and thread ${thread.slice(0, 8)} runs on ${machine}, a machine wsp forked`, "Start it from a thread on this computer.");
pub fn sub_lead_on_machine_refusal(on: &str, thread: &str, machine: &str) -> crate::exit::HostError {
    todo!()
}

/// A sub-thread asking to start a thread: it starts none of its own, and its lead can.
// sub-thread.ts:218
// export const subStartsNothingRefusal = (thread: string, lead: string): Error =>
//   usage(`sub-thread ${thread.slice(0, 8)} starts no threads of its own`, `Send the work to thread ${lead.slice(0, 8)}, which started it, with wsp send ${lead.slice(0, 8)} "<message>".`);
pub fn sub_starts_nothing_refusal(thread: &str, lead: &str) -> crate::exit::HostError {
    todo!()
}

/// S3: the word names no cloud this host holds a key for.
// sub-thread.ts:221
// export const subNoCloudRefusal = (word: string, held: readonly string[]): Error =>
//   usage(`${word} is not a cloud this host holds`, `wsp add boat adds it. It holds: ${held.length === 0 ? "none" : held.join(", ")}.`);
pub fn sub_no_cloud_refusal(word: &str, held: &[impl AsRef<str>]) -> crate::exit::HostError {
    todo!()
}

/// S4: no remote, so nothing for the machine to fetch.
// sub-thread.ts:224
// export const subNoRemoteRefusal = (folder: string, on: string): Error =>
//   usage(`${folder} has no remote, so a machine on ${on} has nothing to clone`, "Push it to GitHub or GitLab, add the remote with git remote add origin <url>, then start the sub-thread again.");
pub fn sub_no_remote_refusal(folder: &str, on: &str) -> crate::exit::HostError {
    todo!()
}

/// S4: a remote on a host the image signs in to no command for.
// sub-thread.ts:227
// export const subOtherHostRefusal = (folder: string, host: string, on: string): Error =>
//   usage(`${folder}'s remote is on ${host}, and a machine on ${on} signs in to GitHub and GitLab only`, "Run this work on this computer.");
pub fn sub_other_host_refusal(folder: &str, host: &str, on: &str) -> crate::exit::HostError {
    todo!()
}

// The other road past a lead branch the remote cannot give: a ref it holds already, the default branch for one.
// sub-thread.ts:230
// private in the TS: port it as a private helper
// const fromRoad = (pushed: string): string => `or start it from a ref the remote holds with --from ${pushed}`;

/// A start on a machine whose credentials the host keeps, for a repo on a host other than GitHub: the host adds a
/// credential for GitHub alone.
// sub-thread.ts:233
// export const brokeredGitHubOnlyRefusal = (project: string, host: string, on: string): Error =>
//   usage(`${project}'s remote is on ${host}, and a machine on ${on} reaches GitHub alone, through this host`, "Run this work on this computer.");
pub fn brokered_git_hub_only_refusal(project: &str, host: &str, on: &str) -> crate::exit::HostError {
    todo!()
}

/// GitHub could not say which repository a start pushes to, read by gh on this computer: the push rule names the
/// repository by GitHub's id, so nothing starts without it.
// sub-thread.ts:237
// export const branchOfRecordUnreadRefusal = (repo: string, why: string): Error =>
//   provider(`gh on this computer could not read ${repo} (${why}), and a machine whose credentials this host keeps pushes only to a repository GitHub names`, "Run gh auth status on this computer, then start it again.");
pub fn branch_of_record_unread_refusal(repo: &str, why: &str) -> crate::exit::HostError {
    todo!()
}

/// A pull request whose fork GitHub no longer holds: there is no branch left to push to.
// sub-thread.ts:240
// export const forkHeadGoneRefusal = (number: number): Error => usage(`pull request #${number}'s fork was deleted, so there is no branch to push to`, "Start the work from an issue or a branch instead.");
pub fn fork_head_gone_refusal(number: f64) -> crate::exit::HostError {
    todo!()
}

/// A command on a machine whose credentials the host keeps, for a workspace with no thread to scope its token to: the
/// same sentence every op token refuses with.
// sub-thread.ts:243
// export const cloneNoThreadRefusal = (name: string): Error => usage(`${name} has no thread yet, so wsp cannot reach GitHub for it`, "Start a thread on it first.");
pub fn clone_no_thread_refusal(name: &str) -> crate::exit::HostError {
    todo!()
}

/// A start that would push to a fork whose author allowed no edits from maintainers.
// sub-thread.ts:245
// export const forkNotPushableRefusal = (owner: string, number: number): Error =>
//   usage(START_WORDS.forkNotPushable(owner), `Ask ${owner} to allow edits from maintainers on pull request #${number}, or review it with wsp review instead.`);
pub fn fork_not_pushable_refusal(owner: &str, number: f64) -> crate::exit::HostError {
    todo!()
}

/// S5: the lead is on no branch.
// sub-thread.ts:248
// export const subDetachedRefusal = (folder: string, pushed: string): Error => usage(`${folder} is on no branch`, `Check one out and push it, ${fromRoad(pushed)}.`);
pub fn sub_detached_refusal(folder: &str, pushed: &str) -> crate::exit::HostError {
    todo!()
}

/// S5: the lead's branch is not on the remote.
// sub-thread.ts:250
// export const subNoUpstreamRefusal = (branch: string, pushed: string): Error =>
//   usage(`${branch} is not on the remote yet`, `Run git push -u origin ${branch} and start the sub-thread again, ${fromRoad(pushed)}.`);
pub fn sub_no_upstream_refusal(branch: &str, pushed: &str) -> crate::exit::HostError {
    todo!()
}

/// S5: the lead holds commits the remote lacks, which the sub-thread could not fetch.
// sub-thread.ts:253
// export const subAheadRefusal = (branch: string, ahead: number, pushed: string): Error =>
//   usage(`${branch} has ${ahead} ${ahead === 1 ? "commit" : "commits"} the remote does not`, `Run git push and start the sub-thread again, ${fromRoad(pushed)}.`);
pub fn sub_ahead_refusal(branch: &str, ahead: f64, pushed: &str) -> crate::exit::HostError {
    todo!()
}

/// S5: git could not count the lead's commits against the remote, so some may not be on it.
// sub-thread.ts:256
// export const subAheadUnknownRefusal = (branch: string, pushed: string): Error =>
//   usage(`git could not count ${branch}'s commits against the remote, so some of them may not be on it`, `Run git fetch, then git push, and start the sub-thread again, ${fromRoad(pushed)}.`);
pub fn sub_ahead_unknown_refusal(branch: &str, pushed: &str) -> crate::exit::HostError {
    todo!()
}

/// S5: --from names a ref the remote does not hold: a branch or tag it lacks, pushed by its name with origin/ dropped,
/// or a commit no branch of it holds, which has no name to push.
// sub-thread.ts:260
// export const subFromNotOnRemoteRefusal = (ref: string, name: string | undefined): Error =>
//   usage(
//     `${ref} is not on the remote`,
//     name === undefined ? "Run git fetch if a branch holding it was pushed since, else push one with git push -u origin <branch>, then start the sub-thread again." : `Run git push -u origin ${name}, then start the sub-thread again.`,
//   );
pub fn sub_from_not_on_remote_refusal(r#ref: &str, name: Option<&str>) -> crate::exit::HostError {
    todo!()
}

/// S5: the remote did not answer the read of the ref --from names.
// sub-thread.ts:266
// export const subFromUnreadRefusal = (ref: string, said: string): Error => provider(`the remote did not say where ${ref} stands (${said})`, "Check the remote answers with git ls-remote origin, then start the sub-thread again.");
pub fn sub_from_unread_refusal(r#ref: &str, said: &str) -> crate::exit::HostError {
    todo!()
}

/// S6: no image on the cloud.
// sub-thread.ts:268
// export const subNoImageRefusal = (on: string): Error => usage(`${on} holds no image of yours yet`, `Build one in Settings > Computers > ${on}, or run wsp init --on ${on.toLowerCase()}.`);
pub fn sub_no_image_refusal(on: &str) -> crate::exit::HostError {
    todo!()
}

/// S6: the image holds no sign-in for the remote's git host, so the sub-thread could fetch nothing private and push nothing.
// sub-thread.ts:270
// export const subNoSignInRefusal = (on: string, host: { label: string; cli: string; login: string }, repo: string): Error =>
//   usage(
//     `the image on ${on} holds no ${host.label} sign-in (${host.cli}), so a sub-thread could neither clone ${repo} nor push its branch`,
//     `Run wsp init --on ${on.toLowerCase()} again with the ${host.login} set to copy.`,
//   );
/// The TS `{ label, cli, login }` of the git host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GitHostWords {
    pub label: String,
    pub cli: String,
    pub login: String,
}

pub fn sub_no_sign_in_refusal(on: &str, host: &GitHostWords, repo: &str) -> HostError {
    todo!()
}

/// S6, on a cloud whose credentials the host keeps: gh on this computer, whose login the host adds to every git and gh
/// request a sub-thread sends, holds none.
// sub-thread.ts:277
// export const subHostNoSignInRefusal = (on: string, repo: string): Error =>
//   usage(`gh on this computer is not signed in to GitHub, so a sub-thread on ${on} could neither clone ${repo} nor push its branch`, "Run gh auth login in a terminal on this computer, then start the sub-thread again.");
pub fn sub_host_no_sign_in_refusal(on: &str, repo: &str) -> crate::exit::HostError {
    todo!()
}

/// S6, on a cloud whose credentials the host keeps: a remote on a git host other than GitHub, which the host adds no
/// credential for.
// sub-thread.ts:281
// export const subHostOtherGitRefusal = (on: string, host: string, repo: string): Error =>
//   usage(`a sub-thread on ${on} reaches GitHub alone through this computer, so it could not clone ${repo} on ${host}`, "Run this work on this computer.");
pub fn sub_host_other_git_refusal(on: &str, host: &str, repo: &str) -> crate::exit::HostError {
    todo!()
}

/// S7: a size the plan does not sell, or no class's figures at all.
// sub-thread.ts:284
// export const subSizeRefusal = (on: string, word: string, sold: readonly SubSize[]): Error => {
//   const named = sold.map(s => `${s.name} ${subSizeWord(s)} (${rateWord(s.rateUsdPerHour)})`);
//   const list = named.length <= 1 ? (named[0] ?? "nothing") : `${named.slice(0, -1).join(", ")} and ${named.at(-1)!}`;
//   return usage(`${on} sells ${list} on this plan; ${word} is not one of them`, `Name one of them, or leave the size out for ${on}'s own.`);
// };
pub fn sub_size_refusal(on: &str, word: &str, sold: &[SubSize]) -> crate::exit::HostError {
    todo!()
}

/// S8: a name git refuses for a branch.
// sub-thread.ts:290
// export const subBranchNameRefusal = (name: string): Error => usage(`${name} is not a name git takes for a branch`, "Name another, or leave --branch out for wsp/<slug>.");
pub fn sub_branch_name_refusal(name: &str) -> crate::exit::HostError {
    todo!()
}

/// S8: the result branch is the lead's own.
// sub-thread.ts:292
// export const subLeadBranchRefusal = (name: string): Error => usage(`the sub-thread pushes ${name}, and that is the lead's own branch`, "Name another, or leave --branch out for wsp/<slug>.");
pub fn sub_lead_branch_refusal(name: &str) -> crate::exit::HostError {
    todo!()
}

/// S9: a sub-machine runs its sub-thread alone.
// sub-thread.ts:294
// export const subMachineTakenRefusal = (machine: string, thread: string): Error =>
//   usage(`${machine} is the machine of sub-thread ${thread.slice(0, 8)} and runs that thread alone`, `Send to ${thread.slice(0, 8)} to continue it.`);
pub fn sub_machine_taken_refusal(machine: &str, thread: &str) -> crate::exit::HostError {
    todo!()
}

/// A second message into a sub-thread whose first is still waiting for its machine.
// sub-thread.ts:297
// export const subWaitingRefusal = (machine: string): Error =>
//   usage(`${machine} is waiting for a machine, and its sub-thread's first message waits with it`, "Send again once it starts.");
pub fn sub_waiting_refusal(machine: &str) -> crate::exit::HostError {
    todo!()
}

// sub-thread.ts:300
// in wsp_wire::types::sub_thread: SUB_DROPPED_LINE

/// S11: the agent named is not on the image.
// sub-thread.ts:302
// export const subNoAgentRefusal = (on: string, agent: string, carried: readonly string[]): Error =>
//   usage(`the image on ${on} carries no ${agent}`, `Add it to the recipe and build the image again, or name one it carries: ${carried.join(", ")}.`);
pub fn sub_no_agent_refusal(on: &str, agent: &str, carried: &[impl AsRef<str>]) -> crate::exit::HostError {
    todo!()
}

/// S12: the picked files come to more than a sub-thread carries.
// sub-thread.ts:305
// export const subSeedTooBigRefusal = (project: string, bytes: string, max: string, largest: readonly string[]): Error =>
//   usage(`the files picked for ${project} come to ${bytes}, over the ${max} a sub-thread carries; the largest are ${largest.join(", ")}`, `Pick again with wsp projects seed ${project}.`);
pub fn sub_seed_too_big_refusal(project: &str, bytes: &str, max: &str, largest: &[impl AsRef<str>]) -> crate::exit::HostError {
    todo!()
}

/// Why a machine failed before its sub-thread's first turn, the three a fresh machine cures.
// sub-thread.ts:308
// export type SubFault = { kind: "unusable"; box: string; why: string; requestId?: string } | { kind: "env"; box: string } | { kind: "cancelled"; box: string; why: string };
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SubFault {
    Unusable { r#box: String, why: String, request_id: Option<String> },
    Env { r#box: String },
    Cancelled { r#box: String, why: String },
}

/// The line a first failure says once its machine is deleted and another started.
// sub-thread.ts:311
// export function subReplacedLine(f: SubFault, machine: string, on: string): string {
//   if (f.kind === "unusable") return `${on} gave ${f.box} with nothing able to run on it (${f.why}); wsp deleted it and started another.`;
//   if (f.kind === "env") return `${on}'s machine ${f.box} did not take wsp's setup within 2 minutes (exit 124); wsp deleted it and started another.`;
//   return `${on} could not get a machine for ${machine} and cancelled the create (${f.why}); wsp started another.`;
// }
pub fn sub_replaced_line(f: &SubFault, machine: &str, on: &str) -> String {
    todo!()
}

/// A sub-thread's turn the kernel ended for memory, as its finished line says it: the machine's size, that the
/// cloud's own processes hold part of it, and the next size up to start the work again at, where the cloud sells one.
// sub-thread.ts:319
// export function subOomLine(machine: string, on: string, size: Pick<SubSize, "name" | "cpu" | "memMb">, bigger: string | undefined): string {
//   const again = bigger !== undefined ? `Start the work again with size ${bigger}.` : `${on} sells no bigger size, so the work has to use less memory.`;
//   return `the agent on ${machine} was killed for memory: its machine is ${subSizeWords(size)}, and ${on}'s own processes hold part of that. ${again}`;
// }
pub fn sub_oom_line(machine: &str, on: &str, size: &SubSize, bigger: Option<&str>) -> String {
    todo!()
}

/// The refusal a second failure in a row fails the spawn with, naming both machines.
// sub-thread.ts:325
// export function subTwiceRefusal(first: SubFault, second: SubFault, machine: string, on: string): Error {
//   const fix = "Start it again in a few minutes.";
//   if (first.kind === "unusable" && second.kind === "unusable") {
//     const request = second.requestId ?? first.requestId;
//     return provider(`${on} gave two machines in a row that could not run commands (${first.box}, ${second.box}${request === undefined ? "" : `; request ${request}`}); ${on} is restoring badly right now`, fix);
//   }
//   if (first.kind === "env" && second.kind === "env") return provider(`${on}'s machines are slow to take a setup right now (${first.box}, ${second.box}, exit 124 on both)`, fix);
//   if (first.kind === "cancelled" && second.kind === "cancelled") return provider(`${on} cancelled two creates in a row for ${machine} (${first.why}; ${second.why})`, fix);
//   const said = (f: SubFault): string => (f.kind === "env" ? `${f.box}: setup not taken, exit 124` : `${f.box}: ${f.why}`);
//   return provider(`${on} gave two machines in a row that failed before the sub-thread started (${said(first)}; ${said(second)})`, fix);
// }
pub fn sub_twice_refusal(first: &SubFault, second: &SubFault, machine: &str, on: &str) -> crate::exit::HostError {
    todo!()
}

/// The line a start passes with when the lead's folder holds changes nobody committed.
// sub-thread.ts:338
// export const subUncommittedLine = (n: number, folder: string, branch: string, sha: string): string =>
//   `${n} uncommitted ${n === 1 ? "change" : "changes"} in ${folder} did not travel; the sub-thread starts at ${branch} as pushed (${sha.slice(0, 7)}).`;
pub fn sub_uncommitted_line(n: f64, folder: &str, branch: &str, sha: &str) -> String {
    todo!()
}

/// Why nothing ignored went: nothing is picked for the project.
// sub-thread.ts:341
// export const subNothingPickedWhy = (project: string): string => `nothing is picked for ${project}`;
pub fn sub_nothing_picked_why(project: &str) -> String {
    todo!()
}

/// Why nothing ignored went: the project's folder is on another computer.
// sub-thread.ts:343
// export const subPickedElsewhereWhy = (project: string, computer: string): string => refusalParts(seedNotHereRefusal(project, computer)).said.replace(/\.$/, "");
pub fn sub_picked_elsewhere_why(project: &str, computer: &str) -> String {
    todo!()
}

/// The line a start passes with when no git-ignored file was sent.
// sub-thread.ts:345
// export const subNotSentLine = (why: string, project: string): string =>
//   `no git-ignored files were sent: ${why}.${why.startsWith("nothing is picked") ? ` wsp projects seed ${project} shows what could go.` : ""}`;
pub fn sub_not_sent_line(why: &str, project: &str) -> String {
    todo!()
}

/// The line a start passes with when picked files stayed here: a sub-thread's machine is a provider's, which gets
/// no login, no file of values and no key, picked by name or found inside a picked folder.
// sub-thread.ts:349
// export const subStayedLine = (paths: readonly string[]): string =>
//   `${paths.join(", ")} stayed on this computer: a sub-thread's machine is a provider's, which gets no login, file of values or key.`;
pub fn sub_stayed_line(paths: &[impl AsRef<str>]) -> String {
    todo!()
}

/// The stage line for the picked files landing.
// sub-thread.ts:352
// export const subCopyingLine = (files: number): string => `copying ${files} ${files === 1 ? "file" : "files"}`;
pub fn sub_copying_line(files: f64) -> String {
    todo!()
}

/// The first line a terminal prints: where the sub-thread runs.
// sub-thread.ts:354
// export const subOpenedLine = (threadId: string, on: string, folder: string): string => `thread ${threadId.slice(0, 8)} on ${on.toLowerCase()} in ${folder}`;
pub fn sub_opened_line(thread_id: &str, on: &str, folder: &str) -> String {
    todo!()
}

// The work a done read found not pushed, in the words the delete's question and the lead's refusal give it.
// sub-thread.ts:357
// private in the TS: port it as a private helper
// function heldWords(held: { files: number; commits: number }): string {
//   const parts = [held.commits > 0 ? `${held.commits} ${held.commits === 1 ? "commit" : "commits"}` : "", held.files > 0 ? `${held.files} changed ${held.files === 1 ? "file" : "files"}` : ""].filter(Boolean);
//   return parts.length === 0 ? "work" : parts.join(" and ");
// }

// The work a done read found not pushed and the branch it is on, as the open thread's notice and a tile's card say it.
// sub-thread.ts:363
// private in the TS: port it as a private helper
// const heldOn = (held: { files: number; commits: number }, branch: string): string => `${heldWords(held)} on ${branch} ${held.files + held.commits === 1 ? "is" : "are"} not pushed`;

/// A held machine's cloud may delete its disk from `day`: Boat says it deletes at day 14, and a box stopped 18 days still
/// resumed whole (2026-10-15), so wsp warns and deletes nothing itself.
// sub-thread.ts:367
// export const diskMayGoLine = (on: string, day: string): string => `${on} may delete its disk from ${day}`;
pub fn disk_may_go_line(on: &str, day: &str) -> String {
    todo!()
}

/// A not pushed sub-thread's reason on its tile's card, with the day its cloud may delete the disk from where it naps,
/// said first, since the card cuts the reason at three lines and a branch name can be long.
// sub-thread.ts:371
// export const subNotPushedReason = (held: { files: number; commits: number }, branch: string, kept?: { on: string; day: string }): string =>
//   kept === undefined ? `${capitalisedFirst(heldOn(held, branch))}, so its machine stays.` : `${diskMayGoLine(kept.on, kept.day)}, with ${heldWords(held)} on ${branch} not pushed.`;
pub fn sub_not_pushed_reason(held: &Held, branch: &str, kept: Option<&KeptOn>) -> String {
    todo!()
}

/// One strip over a sub-thread's composer: what holds or blocks it as the title, the why and what follows under it.
// sub-thread.ts:375
// export interface SubStripWords {
//   title: string;
//   note: string;
// }
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SubStripWords {
    pub title: String,
    pub note: String,
}

/// The strip's words for every state that holds or blocks a sub-thread, each said there and nowhere else in the
/// thread. A not pushed strip names the push's refusal where the done read caught one, since asking the agent to push
/// again cannot mend a token that cannot write.
// sub-thread.ts:383
// export const SUB_STRIP = {
//   waitingSlot: (on: string, running: number, atOnce: number): SubStripWords => ({ title: `Waiting for a machine on ${on}`, note: `${running} of ${atOnce} machines are running. It starts on its own when one frees.` }),
//   waitingMinute: (on: string, used: number, limit: number): SubStripWords => ({ title: `Waiting for ${on}'s next minute of starts`, note: `${used} of ${limit} starts this minute are used. It starts on its own within the minute.` }),
//   notPushed: (held: { files: number; commits: number }, branch: string, refused?: string, kept?: { on: string; day: string }): SubStripWords => ({
//     title: `${capitalisedFirst(heldWords(held))} not pushed`,
//     note: `${refused === undefined ? `They are on ${branch} on this machine.` : `The push to ${branch} was refused: ${refused}.`} This machine stays until the work is pushed or copied to your computer${kept === undefined ? "" : `, but ${diskMayGoLine(kept.on, kept.day)}`}.`,
//   }),
//   deleting: (minutes: string, branch: string): SubStripWords => ({ title: `Its machine is deleted in ${minutes}`, note: `Everything is pushed to ${branch}. The transcript stays, and a message after the delete starts a new machine.` }),
//   released: (on: string): SubStripWords => ({ title: "This thread's machine was deleted", note: `A message starts a new one on ${on}, and the conversation goes on.` }),
//   oom: (line: string): SubStripWords => ({ title: "Killed for memory", note: capitalisedFirst(line) }),
//   cloudGone: (on: string, here: string): SubStripWords => ({ title: `${on} was removed from ${here}`, note: `This thread has no machine to start until ${on} is added again. Its transcript stays.` }),
//   raise: "Raise machines at once",
//   pushAgain: "Push again",
//   /** Copies the sub-thread's commits back to the computer the host runs on, by its name. */
//   copyTo: (computer: string): string => `Copy to ${computer}`,
//   startAgain: (size: string): string => `Start again at ${size}`,
//   addAgain: (on: string): string => `Add ${on} again`,
// } as const;
/// The TS SUB_STRIP: fixed words as constants, the rest as functions.
pub mod sub_strip {
    use super::{Held, KeptOn, SubStripWords};
    pub const RAISE: &str = "Raise machines at once";
    pub const PUSH_AGAIN: &str = "Push again";
    pub fn waiting_slot(on: &str, running: f64, at_once: f64) -> SubStripWords {
        todo!()
    }
    pub fn waiting_minute(on: &str, used: f64, limit: f64) -> SubStripWords {
        todo!()
    }
    pub fn not_pushed(held: &Held, branch: &str, refused: Option<&str>, kept: Option<&KeptOn>) -> SubStripWords {
        todo!()
    }
    pub fn deleting(minutes: &str, branch: &str) -> SubStripWords {
        todo!()
    }
    pub fn released(on: &str) -> SubStripWords {
        todo!()
    }
    pub fn oom(line: &str) -> SubStripWords {
        todo!()
    }
    pub fn cloud_gone(on: &str, here: &str) -> SubStripWords {
        todo!()
    }
    pub fn copy_to(computer: &str) -> String {
        todo!()
    }
    pub fn start_again(size: &str) -> String {
        todo!()
    }
    pub fn add_again(on: &str) -> String {
        todo!()
    }
}

/// The TS `{ files, commits }` of work a machine holds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held {
    pub files: f64,
    pub commits: f64,
}

/// The TS `{ on, day }`: the computer a kept disk is on and the day it may go.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeptOn {
    pub on: String,
    pub day: String,
}

// The command a shell call ran, off its tool input: the `command` field where the input is the agent's JSON (Claude
// Code's carries a description beside it), else the input itself.
// sub-thread.ts:404
// private in the TS: port it as a private helper
// function shellCommandOf(input: string): string {
//   try {
//     const parsed: unknown = JSON.parse(input);
//     const command = typeof parsed === "object" && parsed !== null ? (parsed as { command?: unknown }).command : undefined;
//     return typeof command === "string" ? command : input;
//   } catch {
//     return input;
//   }
// }

/// Whether a shell call an agent made runs a git push.
// sub-thread.ts:415
// export const isGitPush = (input: string): boolean => /\bgit\b(?:\s+-\S+(?:\s+\S+)?)*\s+push\b/.test(shellCommandOf(input));
pub fn is_git_push(input: &str) -> bool {
    todo!()
}

// Progress lines a remote says on any push, refused or not.
// sub-thread.ts:418
// private in the TS: port it as a private helper
// const PUSH_PROGRESS = /^remote:\s*(?:$|Enumerating|Counting|Compressing|Total|Resolving|Writing|Delta|To create|Create a pull request|\s*https?:\/\/)/;

/// Why a push was refused, off the output of a push that failed: the remote's own words, else git's fatal line, the
/// rejected ref's line, then its error line, cut to one line of 200 characters; nothing where none of them is there.
/// The lines are git's (2.43): `remote: Permission to <repo> denied to <user>.` with `fatal: ... returned error: 403`,
/// and ` ! [remote rejected] HEAD -> main (pre-receive hook declined)` under the hook's own `remote:` line.
// sub-thread.ts:424
// export function pushRefusedOf(output: string): string | undefined {
//   const lines = output.split("\n").map(l => l.trimEnd());
//   const first = (match: (l: string) => boolean): string | undefined => lines.find(match);
//   const remote = first(l => l.startsWith("remote:") && !PUSH_PROGRESS.test(l));
//   const line = remote ?? first(l => l.startsWith("fatal:")) ?? first(l => /^\s*! \[/.test(l)) ?? first(l => l.startsWith("error:"));
//   if (line === undefined) return undefined;
//   const said = line.replace(/^remote:\s*/, "").replace(/^\s*! /, "").replace(/\s+/g, " ").trim().replace(/\.$/, "");
//   return said === "" ? undefined : said.slice(0, 200);
// }
pub fn push_refused_of(output: &str) -> Option<String> {
    todo!()
}

/// The message Start again sends the lead: the sub-thread by its title, started again at the size its kill named.
// sub-thread.ts:435
// export const subStartAgainPrompt = (title: string, on: string, size: string): string => `Start the work of the sub-thread "${title}" again on ${on} at size ${size}: its agent was killed for memory.`;
pub fn sub_start_again_prompt(title: &str, on: &str, size: &str) -> String {
    todo!()
}

/// The message Push again sends the sub-thread.
// sub-thread.ts:438
// export const subPushAgainPrompt = (branch: string): string => `Push ${branch} again, and say what the remote answers.`;
pub fn sub_push_again_prompt(branch: &str) -> String {
    todo!()
}

/// Whether a turn's failure is subOomLine's, which the strip over the composer says in its place.
// sub-thread.ts:441
// export const isSubOomLine = (line: string): boolean => /^the agent on .+ was killed for memory: /i.test(line);
pub fn is_sub_oom_line(line: &str) -> bool {
    todo!()
}

/// The line under a sub-thread's brief and the steps it opens to, the stages the spawn reports with the facts it read:
/// done in the past tense, under way in the present, so the thread and the command line name the same steps.
// sub-thread.ts:445
// export const SUB_SPAWN_WORDS = {
//   branch: (done: boolean): string => (done ? "Checked the branch" : "Checking the branch"),
//   branchFact: (branch: string, sha: string): string => `${branch} is pushed at ${sha.slice(0, 7)}`,
//   machine: (on: string, done: boolean): string => (done ? `Asked ${on} for a machine` : `Asking ${on} for a machine`),
//   files: (n: number, done: boolean): string => `${done ? "Copied" : "Copying"} ${n} picked ${n === 1 ? "file" : "files"}`,
//   agent: (agent: string): string => `Starting ${agent}`,
//   agentFact: (model: string | undefined, effort: string | undefined): string | undefined => ([model, effort].filter(Boolean).join(", ") || undefined),
//   /** The cloud's machine by its id and the cloud's own states it passed through. */
//   machineFact: (id: string | undefined, states: readonly string[]): string | undefined => (id === undefined ? undefined : [id, ...states].join(", ")),
//   filesFact: (paths: readonly string[], bytes: string): string => `${paths.join(", ")}, ${bytes}`,
//   notSent: (why: string): string => `No git-ignored files were sent: ${why}.`,
//   starting: (on: string): string => `Starting a machine on ${on}`,
//   started: (on: string, took: string): string => `Started on ${on} in ${took}`,
//   failed: (on: string): string => `Could not start a machine on ${on}`,
//   details: "Show details",
//   hideDetails: "Hide details",
//   tryAgain: "Try again",
// } as const;
/// The TS SUB_SPAWN_WORDS: fixed words as constants, the rest as functions.
pub mod sub_spawn_words {
    pub const DETAILS: &str = "Show details";
    pub const HIDE_DETAILS: &str = "Hide details";
    pub const TRY_AGAIN: &str = "Try again";
    pub fn branch(done: bool) -> String {
        todo!()
    }
    pub fn branch_fact(branch: &str, sha: &str) -> String {
        todo!()
    }
    pub fn machine(on: &str, done: bool) -> String {
        todo!()
    }
    pub fn files(n: f64, done: bool) -> String {
        todo!()
    }
    pub fn agent(agent: &str) -> String {
        todo!()
    }
    pub fn agent_fact(model: Option<&str>, effort: Option<&str>) -> Option<String> {
        todo!()
    }
    pub fn machine_fact(id: Option<&str>, states: &[&str]) -> Option<String> {
        todo!()
    }
    pub fn files_fact(paths: &[&str], bytes: &str) -> String {
        todo!()
    }
    pub fn not_sent(why: &str) -> String {
        todo!()
    }
    pub fn starting(on: &str) -> String {
        todo!()
    }
    pub fn started(on: &str, took: &str) -> String {
        todo!()
    }
    pub fn failed(on: &str) -> String {
        todo!()
    }
}

// sub-thread.ts:464
// private in the TS: port it as a private helper
// const capitalisedFirst = (line: string): string => `${line.charAt(0).toUpperCase()}${line.slice(1)}`;

/// What a sub-thread holds that no remote has: the counts its last read found, or that the read did not answer.
// sub-thread.ts:467
// export interface SubUnpushed {
//   files: number;
//   commits: number;
//   readFailed?: string;
//   /** A turn ran since the last read, so what it left is not known yet. */
//   unread?: true;
// }
/// `unread` is the TS `true` or absent.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SubUnpushed {
    pub files: f64,
    pub commits: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_failed: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread: Option<bool>,
}

/// The second line of the delete's question, and the half of the lead's refusal that says what is not pushed.
// sub-thread.ts:476
// export function subUnpushedLine(held: SubUnpushed, branch: string): string {
//   if (held.unread === true) return `its machine has not been read since its last turn, so work on ${branch} may not be pushed and would go with it.`;
//   if (held.readFailed !== undefined) return `its machine could not be read (${held.readFailed}), so work on ${branch} may not be pushed and would go with it.`;
//   return `${heldWords(held)} on ${branch} ${held.files + held.commits === 1 ? "is" : "are"} not pushed and go with it.`;
// }
pub fn sub_unpushed_line(held: &SubUnpushed, branch: &str) -> String {
    todo!()
}

/// The question a person answers before a sub-thread and its machine go.
// sub-thread.ts:483
// export function subDeleteQuestion(o: { threadId: string; title: string; on: string; branch: string; held?: SubUnpushed }): string {
//   const first = `Delete sub-thread ${o.threadId.slice(0, 8)} (${o.title}) and its machine on ${o.on}?`;
//   const second = o.held === undefined ? `Its turns leave this computer and its machine is deleted; what it pushed to ${o.branch} stays on the remote.` : subUnpushedLine(o.held, o.branch);
//   return `${first}\n${second.charAt(0).toUpperCase()}${second.slice(1)}`;
// }
/// The TS object `subDeleteQuestion` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct SubDeleteQuestion {
    pub thread_id: String,
    pub title: String,
    pub on: String,
    pub branch: String,
    pub held: Option<SubUnpushed>,
}

pub fn sub_delete_question(o: &SubDeleteQuestion) -> String {
    todo!()
}

/// What a sub-thread's machine holds that no remote has, or may: the line a remove of its cloud lists, and the half
/// of the lead's refusal that says what happened.
// sub-thread.ts:491
// export function subHeldLine(threadId: string, held: SubUnpushed, branch: string): string {
//   const id8 = threadId.slice(0, 8);
//   if (held.unread === true) return `sub-thread ${id8}'s machine has not been read since its last turn, so work on ${branch} may not be pushed`;
//   if (held.readFailed !== undefined) return `sub-thread ${id8}'s machine could not be read (${held.readFailed}), so work on ${branch} may not be pushed`;
//   return `sub-thread ${id8} holds ${heldWords(held)} not pushed on ${branch}`;
// }
pub fn sub_held_line(thread_id: &str, held: &SubUnpushed, branch: &str) -> String {
    todo!()
}

/// A lead's own token asking the delete of a sub-thread whose work is not pushed: losing it is the person's call.
// sub-thread.ts:499
// export const subHeldDeleteRefusal = (threadId: string, held: SubUnpushed, branch: string): Error =>
//   usage(subHeldLine(threadId, held, branch), "Send it a push, or ask the person to delete it.");
pub fn sub_held_delete_refusal(thread_id: &str, held: &SubUnpushed, branch: &str) -> crate::exit::HostError {
    todo!()
}

/// A sub-thread whose cloud this host no longer holds, as its row says it.
// sub-thread.ts:503
// export const subCloudGoneLine = (on: string): string => `${on} is no longer on this host`;
pub fn sub_cloud_gone_line(on: &str) -> String {
    todo!()
}

/// A send into a sub-thread whose cloud this host no longer holds.
// sub-thread.ts:506
// export const subCloudGoneRefusal = (on: string): Error => usage(subCloudGoneLine(on), `Add it with wsp add ${on.toLowerCase()} and send again.`);
pub fn sub_cloud_gone_refusal(on: &str) -> crate::exit::HostError {
    todo!()
}

/// A wake of a sub-thread whose machine was deleted after it was done: there is none to wake, and a send makes one.
// sub-thread.ts:509
// export const subReleasedRefusal = (threadId: string, on: string): Error =>
//   usage(`sub-thread ${threadId.slice(0, 8)}'s machine on ${on} was deleted after it was done, so there is none to wake`, `Send to it with wsp send ${threadId.slice(0, 8)} "<message>", which starts a new one.`);
pub fn sub_released_refusal(thread_id: &str, on: &str) -> crate::exit::HostError {
    todo!()
}

/// What a delete Boat refused says, and when it is asked again.
// sub-thread.ts:513
// export const subDeleteRefusedLine = (on: string, machine: string, said: string, retryMinutes: number): string =>
//   `${on} would not delete ${machine} (${said}); it is tried again in ${retryMinutes} minutes`;
pub fn sub_delete_refused_line(on: &str, machine: &str, said: &str, retry_minutes: f64) -> String {
    todo!()
}

/// The line a cloud's remove adds to its question for sub-threads whose machines hold work no remote has: it goes
/// with them. Names five and counts the rest.
// sub-thread.ts:518
// export function subHeldOnRemoveLine(held: readonly string[]): string | undefined {
//   if (held.length === 0) return undefined;
//   const rest = held.length > 5 ? `; and ${held.length - 5} more` : "";
//   return `Work not pushed goes with ${held.length === 1 ? "its machine" : "their machines"}: ${held.slice(0, 5).join("; ")}${rest}.`;
// }
pub fn sub_held_on_remove_line(held: &[impl AsRef<str>]) -> Option<String> {
    todo!()
}

/// A sub-thread's cloud as a line names it, off the word its record keeps: `boat` is Boat.
// sub-thread.ts:525
// export function cloudSaid(on: string): string {
//   const id = cloudNamed(Object.keys(PROVIDER_KEY_WORDS), on);
//   return id === undefined ? on : providerKeyName(id);
// }
pub fn cloud_said(on: &str) -> String {
    todo!()
}

/// What a sub-thread's delete did.
// sub-thread.ts:531
// export const subDeletedLine = (threadId: string, on: string): string => `deleted sub-thread ${threadId.slice(0, 8)} and its machine on ${on}`;
pub fn sub_deleted_line(thread_id: &str, on: &str) -> String {
    todo!()
}

// The UTC day of a moment, for a line about a cloud's keep of a disk: the keep is counted from the cloud's own stop,
// so the time of day would claim more than wsp knows.
// sub-thread.ts:535
// private in the TS: port it as a private helper
// const utcDay = (at: number): string => `${new Date(at).toISOString().slice(0, 10)} UTC`;

/// The line a sub-thread's finished line to its lead ends with, off the done's read: the branch pushed and the merge
/// that takes it, the work not pushed that keeps the machine, or nothing to merge. `deleteInMs` is how long the machine
/// stays, absent where it goes at once; a span, never a clock time, since an agent and a person read it in any zone.
/// `kept` is the day from which a cloud that deletes a paused machine's disk may delete this one's, a UTC day for the
/// same reason.
// sub-thread.ts:542
// export function subResultLine(o: { threadId: string; lead: string; result: SubResult; base: string; deleteInMs?: number; kept?: { on: string; until: number } }): string {
//   const { result: r, kept } = o;
//   const id8 = o.threadId.slice(0, 8);
//   const minutes = o.deleteInMs === undefined ? 0 : Math.round(o.deleteInMs / 60_000);
//   const goes = minutes > 0 ? `Its machine naps now and is deleted in ${plural(minutes, "minute")} unless you send to ${id8} first.` : `Its machine is deleted now; a send to ${id8} starts a new one with the conversation kept.`;
//   const naps = kept === undefined ? "its machine is kept, napping at no cost" : `its machine naps at no cost, and ${diskMayGoLine(kept.on, utcDay(kept.until))}`;
//   if (r.readFailed !== undefined) return `its machine could not be read (${r.readFailed}), so work on ${r.branch} may not be pushed and ${naps}. Send to ${id8} to have it pushed, or ask the person to delete it.`;
//   if (!subAllPushed(r)) {
//     const parts = [r.unpushed > 0 ? plural(r.unpushed, "commit") : "", r.changed > 0 ? plural(r.changed, "changed file") : ""].filter(Boolean);
//     const one = r.unpushed + r.changed === 1;
//     return `${parts.join(" and ")} on ${r.branch} ${one ? "is" : "are"} not pushed, so ${naps}. Send to ${id8} to have ${one ? "it" : "them"} pushed, or ask the person to delete it.`;
//   }
//   if (r.pushedSha === null || r.commits === 0) return `no commits on ${r.branch}, so there is nothing to merge. ${goes}`;
//   return `branch ${r.branch} pushed at ${r.pushedSha.slice(0, 7)}, ${plural(r.commits, "commit")} on ${o.base}; merge it with wsp merge in ${o.lead.slice(0, 8)} ${id8}. ${goes}`;
// }
/// The TS `{ on, until }` of a kept disk, `until` ms epoch.
#[derive(Clone, Debug, PartialEq)]
pub struct KeptUntil {
    pub on: String,
    pub until: f64,
}

/// The TS object `subResultLine` reads.
#[derive(Clone, Debug, PartialEq)]
pub struct SubResultFacts {
    pub thread_id: String,
    pub lead: String,
    pub result: SubResult,
    pub base: String,
    pub delete_in_ms: Option<f64>,
    pub kept: Option<KeptUntil>,
}

pub fn sub_result_line(o: &SubResultFacts) -> String {
    todo!()
}

