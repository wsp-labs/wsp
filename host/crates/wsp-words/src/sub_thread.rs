// SPDX-License-Identifier: AGPL-3.0-only
//! A sub-thread on a machine of its own: the sizes it may take, the preamble that tells its agent how to get the
//! code and hand it back, the strip and spawn words a client shows, and the lines and refusals of its end. Port of
//! the functions of packages/protocol/src/sub-thread.ts at a3b870a6e; its shapes and flat constants are
//! wsp_wire::types::sub_thread. See spec/sub_thread.md.

use std::sync::LazyLock;

use regex::Regex;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::types::sub_thread::SUB_SHALLOW_LINE;
use wsp_wire::views::sub_row::{sub_all_pushed, SubResult};

use serde::Serialize;

use crate::exit::{refusal, refusal_parts, HostError, Kind};
use crate::js::{
    js_collapse_whitespace, js_round, js_trim, js_trim_end, js_trim_start, number_string, to_fixed, upper_first, utf16, JS_WS,
};
use crate::place_word::{cloud_named, provider_key_name, PROVIDER_KEY_IDS};
use crate::project_seed::seed_not_here_refusal;
use crate::start::owner_repo_of;
use crate::start::start_words;
use crate::words::base::plural;
use crate::words::units::fmt_mem_gb;

mod end;

pub use end::*;

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

// sub-thread.ts:418
static PUSH_PROGRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(
        r"^remote:[{JS_WS}]*(?:$|Enumerating|Counting|Compressing|Total|Resolving|Writing|Delta|To create|Create a pull request|[{JS_WS}]*https?://)"
    ))
    .expect("PUSH_PROGRESS is a valid pattern")
});

// sub-thread.ts:424
static PUSH_REJECTED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(&format!(r"^[{JS_WS}]*! \[")).expect("PUSH_REJECTED is a valid pattern"));

// sub-thread.ts:415
static GIT_PUSH: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(&format!(r"(?-u:\b)git(?-u:\b)(?:[{JS_WS}]+-[^{JS_WS}]+(?:[{JS_WS}]+[^{JS_WS}]+)?)*[{JS_WS}]+push(?-u:\b)"))
        .expect("GIT_PUSH is a valid pattern")
});

/// Why a word beside --on is refused, the word a flag or a tool input: the sub-thread runs in a folder of its own on the
/// cloud, under the asking thread.
// sub-thread.ts:45
// export const subBesideRefusal = (word: string): Error =>
//   refusal(`--on starts a sub-thread of the thread asking, in a folder of its own on its own machine, so it cannot also take ${word}`, `Drop ${word}, or run the work without --on.`, "usage");
pub fn sub_beside_refusal(word: &str) -> crate::exit::HostError {
    usage(
        &format!("--on starts a sub-thread of the thread asking, in a folder of its own on its own machine, so it cannot also take {word}"),
        &format!("Drop {word}, or run the work without --on."),
    )
}

/// A project named beside --on that is not the asking thread's: the sub-thread carries that thread's project alone.
// sub-thread.ts:49
// export const subOtherProjectRefusal = (on: string, word: string, carried: string): Error =>
//   refusal(`run --on ${on} starts a sub-thread of the thread asking, on that thread's project ${carried}, and ${word} is another project`, `Leave the project out, or run the work on ${word} without --on.`, "usage");
pub fn sub_other_project_refusal(on: &str, word: &str, carried: &str) -> crate::exit::HostError {
    usage(
        &format!(
            "run --on {on} starts a sub-thread of the thread asking, on that thread's project {carried}, and {word} is another project"
        ),
        &format!("Leave the project out, or run the work on {word} without --on."),
    )
}

/// The line ahead of the next message of a thread that started before a cloud was added: its agent read the computers
/// and its tool words at its start and keeps them.
// sub-thread.ts:54
// export const cloudAddedLine = (name: string): string => `${name} was added since this thread started: run with on ${name.toLowerCase()} starts a sub-thread there.`;
pub fn cloud_added_line(name: &str) -> String {
    format!("{name} was added since this thread started: run with on {} starts a sub-thread there.", name.to_lowercase())
}

/// The same for a cloud taken away.
// sub-thread.ts:57
// export const cloudRemovedLine = (name: string): string => `${name} was removed since this thread started: run with on ${name.toLowerCase()} starts nothing there now.`;
pub fn cloud_removed_line(name: &str) -> String {
    format!("{name} was removed since this thread started: run with on {} starts nothing there now.", name.to_lowercase())
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

// units.ts:123, the memory a size word spells in GB: whole when whole, else one decimal, read back as a number.
fn mem_gb(mem_mb: f64) -> f64 {
    to_fixed(mem_mb / 1024.0, 1).parse().unwrap_or(f64::NAN)
}

// units.ts:239, a size as the words name it: `<cpu>x<memGb>`.
fn size_word_of(size: &impl SizeFacts) -> String {
    format!("{}x{}", number_string(size.cpu()), number_string(mem_gb(size.mem_mb())))
}

/// A size as a sub-thread names it.
// sub-thread.ts:68
// export const subSizeWord = (size: { cpu: number; memMb: number }): string => sizeWord(size);
pub fn sub_size_word(size: &CpuMem) -> String {
    size_word_of(size)
}

/// The size a word names, or nothing where it is not `<cpu>x<memGb>`.
// sub-thread.ts:71
// export const subSizeOf = (word: string): { cpu: number; memMb: number } | undefined => sizeFromWord(word);
pub fn sub_size_of(word: &str) -> Option<CpuMem> {
    let (cpu, gb) = js_trim(word).split_once('x')?;
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    let gb_ok = match gb.split_once('.') {
        Some((whole, frac)) => digits(whole) && digits(frac),
        None => digits(gb),
    };
    if !digits(cpu) || !gb_ok {
        return None;
    }
    let cpu: f64 = cpu.parse().ok()?;
    let mem_mb = js_round(gb.parse::<f64>().ok()? * 1024.0);
    (cpu > 0.0 && mem_mb > 0.0).then_some(CpuMem { cpu, mem_mb })
}

/// A size in the words the preamble and the card say it in: `small, 2 vCPU, 4 GB`.
// sub-thread.ts:74
// export const subSizeWords = (size: Pick<SubSize, "name" | "cpu" | "memMb">): string => `${size.name}, ${size.cpu} vCPU, ${fmtMemGb(size.memMb)}`;
pub fn sub_size_words(size: &SubSize) -> String {
    format!("{}, {} vCPU, {}", size.name, number_string(size.cpu), fmt_mem_gb(size.mem_mb))
}

// sub-thread.ts:76
// private in the TS: port it as a private helper
// const rateWord = (usdPerHour: number): string => `$${usdPerHour.toFixed(3)}/h`;
fn rate_word(usd_per_hour: f64) -> String {
    format!("${}/h", to_fixed(usd_per_hour, 3))
}

/// A size by the class name its row carries, or by its figures where it carries none.
// sub-thread.ts:79
// export const sizeNameOf = (size: { cpu: number; memMb: number; name?: string | undefined }): string => size.name ?? sizeWord(size);
pub fn size_name_of(size: &impl SizeFacts) -> String {
    size.name().map_or_else(|| size_word_of(size), str::to_string)
}

/// The size a sub-thread killed for memory starts again at: the smallest its cloud sells with more memory than it had,
/// or none where the cloud sells nothing bigger. What the kill's line names and the strip's Start again asks.
// sub-thread.ts:83
// export function subNextSize<S extends { cpu: number; memMb: number }>(size: { memMb: number }, sold: readonly S[]): S | undefined {
//   return [...sold].filter(s => s.memMb > size.memMb).sort((a, b) => a.memMb - b.memMb)[0];
// }
/// `mem_mb` is the TS `size.memMb`.
pub fn sub_next_size<S: SizeFacts + Clone>(mem_mb: f64, sold: &[S]) -> Option<S> {
    sold.iter().filter(|s| s.mem_mb() > mem_mb).min_by(|a, b| a.mem_mb().total_cmp(&b.mem_mb())).cloned()
}

/// The sizes a cloud's row says it sells a sub-thread now: those its plan names, or every size it lists before a
/// limits read has answered.
// sub-thread.ts:89
// export const subSizesSold = <S extends { cpu: number; memMb: number }>(sizes: readonly S[], plan: { sizes: readonly string[] } | undefined): S[] =>
//   sizes.filter(s => plan === undefined || plan.sizes.includes(sizeWord(s)));
/// `plan_sizes` is the TS `plan.sizes`, None for no plan.
pub fn sub_sizes_sold<S: SizeFacts + Clone>(sizes: &[S], plan_sizes: Option<&[&str]>) -> Vec<S> {
    sizes.iter().filter(|s| plan_sizes.is_none_or(|plan| plan.contains(&size_word_of(*s).as_str()))).cloned().collect()
}

/// The folder a sub-thread works in on its machine: the repository's own name off its remote, under root's home.
// sub-thread.ts:93
// export function subFolderOf(remote: string): string {
//   const name = (ownerRepoOf(remote) ?? remote).split("/").at(-1)!.replace(/\.git$/, "");
//   return `/root/${name}`;
// }
pub fn sub_folder_of(remote: &str) -> String {
    let path = owner_repo_of(remote).unwrap_or_else(|| remote.to_string());
    let last = path.split('/').next_back().unwrap_or("");
    format!("/root/{}", last.strip_suffix(".git").unwrap_or(last))
}

/// The branch a sub-thread pushes when its start names none: `wsp/` and the machine's name as a slug.
// sub-thread.ts:99
// export function subBranchOf(machine: string): string {
//   const slug = machine.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-+|-+$/g, "").slice(0, 48).replace(/-+$/, "");
//   return `wsp/${slug === "" ? "sub-thread" : slug}`;
// }
pub fn sub_branch_of(machine: &str) -> String {
    let mut runs = String::new();
    let mut in_run = false;
    for c in machine.to_lowercase().chars() {
        if c.is_ascii_lowercase() || c.is_ascii_digit() {
            runs.push(c);
            in_run = false;
        } else if !in_run {
            runs.push('-');
            in_run = true;
        }
    }
    let trimmed = runs.trim_matches('-');
    let slug: String = trimmed.chars().take(48).collect();
    let slug = slug.trim_end_matches('-');
    format!("wsp/{}", if slug.is_empty() { "sub-thread" } else { slug })
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
    if name.is_empty() || name == "HEAD" || name.starts_with('-') || name.starts_with('/') || name.ends_with('/') || name.ends_with('.') {
        return false;
    }
    if name.chars().any(|c| c <= ' ' || c == '\x7f' || "~^:?*[\\".contains(c))
        || name.contains("..")
        || name.contains("//")
        || name.contains("@{")
    {
        return false;
    }
    name.split('/').all(|part| !part.is_empty() && !part.starts_with('.') && !part.ends_with(".lock"))
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
/// The TS `fetch` of `subGitLines`: what the clone fetches and what it checks out.
struct GitFetch<'a> {
    fetch_ref: &'a str,
    blobless: &'a str,
    checkout: &'a str,
}

// sub-thread.ts:118, with the shell quote of the brokered refspec from shell-quote.ts:4.
fn sub_git_lines(setup_git: &str, repo: &str, branch: &str, brokered: bool, fetch: &GitFetch) -> Vec<String> {
    if brokered {
        return vec![
            "git init -q".into(),
            format!("git remote add -t {} origin {repo}", shell_quote(&format!("{branch}*"))),
            format!("git fetch -q --depth 1 origin {}", fetch.fetch_ref),
            format!("git checkout -q -b {branch} FETCH_HEAD"),
        ];
    }
    vec![
        setup_git.to_string(),
        "git init -q".into(),
        format!("git remote add origin {repo}"),
        format!("git fetch -q --filter=blob:none origin {}", fetch.blobless),
        format!("git checkout -q -b {branch} {}", fetch.checkout),
    ]
}

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
    let lead = utf16::cut(&o.lead, 8);
    let brokered = o.brokered == Some(true);
    let fetch = GitFetch { fetch_ref: &o.base.sha, blobless: &o.base.branch, checkout: &o.base.sha };
    let mut lines = vec![
        format!(
            "This thread runs on a machine of its own on {} ({}), started by thread {lead}. The code is not here yet. {} Get the code first, in this folder:",
            o.on,
            sub_size_words(&o.size),
            folder_words(&o.folder, &o.sent)
        ),
        String::new(),
    ];
    lines.extend(sub_git_lines(&o.setup_git, &o.repo, &o.branch, brokered, &fetch));
    lines.push(String::new());
    lines.push(format!(
        "Work on {b}, then commit and run git push -u origin {b} before your turn ends. Thread {lead} merges that branch. This machine is deleted {}, and anything not pushed is lost with it.{}",
        deleted_words(o.delete_after_minutes),
        if brokered { format!(" {SUB_SHALLOW_LINE}") } else { String::new() },
        b = o.branch
    ));
    lines.push(String::new());
    lines.push("The task:".into());
    lines.join("\n")
}

// The folder a sub-thread's machine opens in, as its preamble says it: empty, or holding the files picked.
// sub-thread.ts:158
// private in the TS: port it as a private helper
// const folderWords = (folder: string, sent: readonly string[]): string =>
//   sent.length === 0 ? `The folder you are in, ${folder}, is empty.` : `The folder you are in, ${folder}, holds only the git-ignored files the person picked: ${sent.join(", ")}.`;
fn folder_words(folder: &str, sent: &[impl AsRef<str>]) -> String {
    if sent.is_empty() {
        return format!("The folder you are in, {folder}, is empty.");
    }
    format!("The folder you are in, {folder}, holds only the git-ignored files the person picked: {}.", join_all(sent, ", "))
}

// When a sub-thread's machine goes, as its preamble says it.
// sub-thread.ts:162
// private in the TS: port it as a private helper
// const deletedWords = (minutes: number): string => (minutes === 0 ? "as soon as your last turn ends" : `${minutes} minutes after your last turn ends`);
fn deleted_words(minutes: f64) -> String {
    if minutes == 0.0 {
        return "as soon as your last turn ends".into();
    }
    format!("{} minutes after your last turn ends", number_string(minutes))
}

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
    let brokered = o.brokered == Some(true);
    let (fetch, push) = if o.pushed {
        (GitFetch { fetch_ref: &o.branch, blobless: &o.branch, checkout: "FETCH_HEAD" }, format!("Push to {} as before.", o.branch))
    } else {
        (
            GitFetch { fetch_ref: &o.base.sha, blobless: &o.base.branch, checkout: &o.base.sha },
            format!("Work on {b}, then commit and run git push -u origin {b} before your turn ends.", b = o.branch),
        )
    };
    let mut lines = vec![
        format!(
            "This thread's machine was deleted after its last turn, and the thread goes on here, on a new machine on {} ({}). Your conversation came with it; the code did not. {} Get your branch back first, in this folder:",
            o.on,
            sub_size_words(&o.size),
            folder_words(&o.folder, &o.sent)
        ),
        String::new(),
    ];
    lines.extend(sub_git_lines(&o.setup_git, &o.repo, &o.branch, brokered, &fetch));
    lines.push(String::new());
    lines.push(format!(
        "{push} This machine is deleted {}.{}",
        deleted_words(o.delete_after_minutes),
        if brokered { format!(" {SUB_SHALLOW_LINE}") } else { String::new() }
    ));
    lines.push(String::new());
    lines.push("The message:".into());
    lines.join("\n")
}

/// A sub-thread's session past what the host keeps, as its row says it.
// sub-thread.ts:194
// export const subSessionTooLargeLine = (size: string): string => `the agent's session was too large to keep (${size}); a send continues from the transcript`;
pub fn sub_session_too_large_line(size: &str) -> String {
    format!("the agent's session was too large to keep ({size}); a send continues from the transcript")
}

/// A sub-thread's session the host could not keep, as its row says it.
// sub-thread.ts:196
// export const subSessionNotKeptLine = (why: string): string => `the agent's session was not kept (${why}); a send continues from the transcript`;
pub fn sub_session_not_kept_line(why: &str) -> String {
    format!("the agent's session was not kept ({why}); a send continues from the transcript")
}

// sub-thread.ts:199
// in wsp_wire::types::sub_thread: SUB_TURN_GONE_LINE

/// The first message whole: the preamble, a blank line, and the lead's task as given.
// sub-thread.ts:202
// export const subOpening = (preamble: string, task: string): string => `${preamble}\n\n${task}`;
pub fn sub_opening(preamble: &str, task: &str) -> String {
    format!("{preamble}\n\n{task}")
}

// sub-thread.ts:204
// private in the TS: port it as a private helper
// const usage = (happened: string, fix: string): Error => refusal(happened, fix, "usage");
fn usage(happened: &str, fix: &str) -> HostError {
    refusal(happened, fix, Some(Kind::Usage))
}

// sub-thread.ts:205
// private in the TS: port it as a private helper
// const provider = (happened: string, fix: string): Error => refusal(happened, fix);
fn provider(happened: &str, fix: &str) -> HostError {
    refusal(happened, fix, None)
}

/// S1: a sub-thread is a child, so a line out of no thread has no lead to notify or merge into.
// sub-thread.ts:208
// export const subNoThreadRefusal = (on: string): Error =>
//   usage(`run --on ${on} starts a sub-thread of the thread that asks, and this line came from no thread`, "Start it from a thread, or run the work on this computer with wsp run <project>.");
pub fn sub_no_thread_refusal(on: &str) -> crate::exit::HostError {
    usage(
        &format!("run --on {on} starts a sub-thread of the thread that asks, and this line came from no thread"),
        "Start it from a thread, or run the work on this computer with wsp run <project>.",
    )
}

/// `size` from a terminal with no cloud named.
// sub-thread.ts:211
// export const subSizeNoCloudRefusal = (): Error => usage("--size sizes a sub-thread's machine, and this line starts none", "Add --on boat.");
pub fn sub_size_no_cloud_refusal() -> crate::exit::HostError {
    usage("--size sizes a sub-thread's machine, and this line starts none", "Add --on boat.")
}

/// `from` from a terminal with no cloud named.
// sub-thread.ts:213
// export const subFromNoCloudRefusal = (): Error => usage("--from names the commit a sub-thread's machine starts from, and this line starts none", "Add --on boat.");
pub fn sub_from_no_cloud_refusal() -> crate::exit::HostError {
    usage("--from names the commit a sub-thread's machine starts from, and this line starts none", "Add --on boat.")
}

/// A lead runs on this computer or on a computer the person added, never on a machine wsp forked.
// sub-thread.ts:215
// export const subLeadOnMachineRefusal = (on: string, thread: string, machine: string): Error =>
//   usage(`run --on ${on} starts a sub-thread of a thread on this computer or on a computer you added, and thread ${thread.slice(0, 8)} runs on ${machine}, a machine wsp forked`, "Start it from a thread on this computer.");
pub fn sub_lead_on_machine_refusal(on: &str, thread: &str, machine: &str) -> crate::exit::HostError {
    usage(
        &format!(
            "run --on {on} starts a sub-thread of a thread on this computer or on a computer you added, and thread {} runs on {machine}, a machine wsp forked",
            utf16::cut(thread, 8)
        ),
        "Start it from a thread on this computer.",
    )
}

/// A sub-thread asking to start a thread: it starts none of its own, and its lead can.
// sub-thread.ts:218
// export const subStartsNothingRefusal = (thread: string, lead: string): Error =>
//   usage(`sub-thread ${thread.slice(0, 8)} starts no threads of its own`, `Send the work to thread ${lead.slice(0, 8)}, which started it, with wsp send ${lead.slice(0, 8)} "<message>".`);
pub fn sub_starts_nothing_refusal(thread: &str, lead: &str) -> crate::exit::HostError {
    let lead8 = utf16::cut(lead, 8);
    usage(
        &format!("sub-thread {} starts no threads of its own", utf16::cut(thread, 8)),
        &format!("Send the work to thread {lead8}, which started it, with wsp send {lead8} \"<message>\"."),
    )
}

/// S3: the word names no cloud this host holds a key for.
// sub-thread.ts:221
// export const subNoCloudRefusal = (word: string, held: readonly string[]): Error =>
//   usage(`${word} is not a cloud this host holds`, `wsp add boat adds it. It holds: ${held.length === 0 ? "none" : held.join(", ")}.`);
pub fn sub_no_cloud_refusal(word: &str, held: &[impl AsRef<str>]) -> crate::exit::HostError {
    let holds = if held.is_empty() { "none".to_string() } else { join_all(held, ", ") };
    usage(&format!("{word} is not a cloud this host holds"), &format!("wsp add boat adds it. It holds: {holds}."))
}

/// S4: no remote, so nothing for the machine to fetch.
// sub-thread.ts:224
// export const subNoRemoteRefusal = (folder: string, on: string): Error =>
//   usage(`${folder} has no remote, so a machine on ${on} has nothing to clone`, "Push it to GitHub or GitLab, add the remote with git remote add origin <url>, then start the sub-thread again.");
pub fn sub_no_remote_refusal(folder: &str, on: &str) -> crate::exit::HostError {
    usage(
        &format!("{folder} has no remote, so a machine on {on} has nothing to clone"),
        "Push it to GitHub or GitLab, add the remote with git remote add origin <url>, then start the sub-thread again.",
    )
}

/// S4: a remote on a host the image signs in to no command for.
// sub-thread.ts:227
// export const subOtherHostRefusal = (folder: string, host: string, on: string): Error =>
//   usage(`${folder}'s remote is on ${host}, and a machine on ${on} signs in to GitHub and GitLab only`, "Run this work on this computer.");
pub fn sub_other_host_refusal(folder: &str, host: &str, on: &str) -> crate::exit::HostError {
    usage(
        &format!("{folder}'s remote is on {host}, and a machine on {on} signs in to GitHub and GitLab only"),
        "Run this work on this computer.",
    )
}

// The other road past a lead branch the remote cannot give: a ref it holds already, the default branch for one.
// sub-thread.ts:230
// private in the TS: port it as a private helper
// const fromRoad = (pushed: string): string => `or start it from a ref the remote holds with --from ${pushed}`;
fn from_road(pushed: &str) -> String {
    format!("or start it from a ref the remote holds with --from {pushed}")
}

/// A start on a machine whose credentials the host keeps, for a repo on a host other than GitHub: the host adds a
/// credential for GitHub alone.
// sub-thread.ts:233
// export const brokeredGitHubOnlyRefusal = (project: string, host: string, on: string): Error =>
//   usage(`${project}'s remote is on ${host}, and a machine on ${on} reaches GitHub alone, through this host`, "Run this work on this computer.");
pub fn brokered_git_hub_only_refusal(project: &str, host: &str, on: &str) -> crate::exit::HostError {
    usage(
        &format!("{project}'s remote is on {host}, and a machine on {on} reaches GitHub alone, through this host"),
        "Run this work on this computer.",
    )
}

/// GitHub could not say which repository a start pushes to, read by gh on this computer: the push rule names the
/// repository by GitHub's id, so nothing starts without it.
// sub-thread.ts:237
// export const branchOfRecordUnreadRefusal = (repo: string, why: string): Error =>
//   provider(`gh on this computer could not read ${repo} (${why}), and a machine whose credentials this host keeps pushes only to a repository GitHub names`, "Run gh auth status on this computer, then start it again.");
pub fn branch_of_record_unread_refusal(repo: &str, why: &str) -> crate::exit::HostError {
    provider(
        &format!("gh on this computer could not read {repo} ({why}), and a machine whose credentials this host keeps pushes only to a repository GitHub names"),
        "Run gh auth status on this computer, then start it again.",
    )
}

/// A pull request whose fork GitHub no longer holds: there is no branch left to push to.
// sub-thread.ts:240
// export const forkHeadGoneRefusal = (number: number): Error => usage(`pull request #${number}'s fork was deleted, so there is no branch to push to`, "Start the work from an issue or a branch instead.");
pub fn fork_head_gone_refusal(number: f64) -> crate::exit::HostError {
    usage(
        &format!("pull request #{}'s fork was deleted, so there is no branch to push to", number_string(number)),
        "Start the work from an issue or a branch instead.",
    )
}

/// A command on a machine whose credentials the host keeps, for a workspace with no thread to scope its token to: the
/// same sentence every op token refuses with.
// sub-thread.ts:243
// export const cloneNoThreadRefusal = (name: string): Error => usage(`${name} has no thread yet, so wsp cannot reach GitHub for it`, "Start a thread on it first.");
pub fn clone_no_thread_refusal(name: &str) -> crate::exit::HostError {
    usage(&format!("{name} has no thread yet, so wsp cannot reach GitHub for it"), "Start a thread on it first.")
}

/// A start that would push to a fork whose author allowed no edits from maintainers.
// sub-thread.ts:245
// export const forkNotPushableRefusal = (owner: string, number: number): Error =>
//   usage(START_WORDS.forkNotPushable(owner), `Ask ${owner} to allow edits from maintainers on pull request #${number}, or review it with wsp review instead.`);
pub fn fork_not_pushable_refusal(owner: &str, number: f64) -> crate::exit::HostError {
    usage(
        &start_words::fork_not_pushable(owner),
        &format!(
            "Ask {owner} to allow edits from maintainers on pull request #{}, or review it with wsp review instead.",
            number_string(number)
        ),
    )
}

/// S5: the lead is on no branch.
// sub-thread.ts:248
// export const subDetachedRefusal = (folder: string, pushed: string): Error => usage(`${folder} is on no branch`, `Check one out and push it, ${fromRoad(pushed)}.`);
pub fn sub_detached_refusal(folder: &str, pushed: &str) -> crate::exit::HostError {
    usage(&format!("{folder} is on no branch"), &format!("Check one out and push it, {}.", from_road(pushed)))
}

/// S5: the lead's branch is not on the remote.
// sub-thread.ts:250
// export const subNoUpstreamRefusal = (branch: string, pushed: string): Error =>
//   usage(`${branch} is not on the remote yet`, `Run git push -u origin ${branch} and start the sub-thread again, ${fromRoad(pushed)}.`);
pub fn sub_no_upstream_refusal(branch: &str, pushed: &str) -> crate::exit::HostError {
    usage(
        &format!("{branch} is not on the remote yet"),
        &format!("Run git push -u origin {branch} and start the sub-thread again, {}.", from_road(pushed)),
    )
}

/// S5: the lead holds commits the remote lacks, which the sub-thread could not fetch.
// sub-thread.ts:253
// export const subAheadRefusal = (branch: string, ahead: number, pushed: string): Error =>
//   usage(`${branch} has ${ahead} ${ahead === 1 ? "commit" : "commits"} the remote does not`, `Run git push and start the sub-thread again, ${fromRoad(pushed)}.`);
pub fn sub_ahead_refusal(branch: &str, ahead: f64, pushed: &str) -> crate::exit::HostError {
    usage(
        &format!("{branch} has {} {} the remote does not", number_string(ahead), if ahead == 1.0 { "commit" } else { "commits" }),
        &format!("Run git push and start the sub-thread again, {}.", from_road(pushed)),
    )
}

/// S5: git could not count the lead's commits against the remote, so some may not be on it.
// sub-thread.ts:256
// export const subAheadUnknownRefusal = (branch: string, pushed: string): Error =>
//   usage(`git could not count ${branch}'s commits against the remote, so some of them may not be on it`, `Run git fetch, then git push, and start the sub-thread again, ${fromRoad(pushed)}.`);
pub fn sub_ahead_unknown_refusal(branch: &str, pushed: &str) -> crate::exit::HostError {
    usage(
        &format!("git could not count {branch}'s commits against the remote, so some of them may not be on it"),
        &format!("Run git fetch, then git push, and start the sub-thread again, {}.", from_road(pushed)),
    )
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
    let fix = match name {
        None => "Run git fetch if a branch holding it was pushed since, else push one with git push -u origin <branch>, then start the sub-thread again.".to_string(),
        Some(name) => format!("Run git push -u origin {name}, then start the sub-thread again."),
    };
    usage(&format!("{} is not on the remote", r#ref), &fix)
}

/// S5: the remote did not answer the read of the ref --from names.
// sub-thread.ts:266
// export const subFromUnreadRefusal = (ref: string, said: string): Error => provider(`the remote did not say where ${ref} stands (${said})`, "Check the remote answers with git ls-remote origin, then start the sub-thread again.");
pub fn sub_from_unread_refusal(r#ref: &str, said: &str) -> crate::exit::HostError {
    provider(
        &format!("the remote did not say where {} stands ({said})", r#ref),
        "Check the remote answers with git ls-remote origin, then start the sub-thread again.",
    )
}

/// S6: no image on the cloud.
// sub-thread.ts:268
// export const subNoImageRefusal = (on: string): Error => usage(`${on} holds no image of yours yet`, `Build one in Settings > Computers > ${on}, or run wsp init --on ${on.toLowerCase()}.`);
pub fn sub_no_image_refusal(on: &str) -> crate::exit::HostError {
    usage(
        &format!("{on} holds no image of yours yet"),
        &format!("Build one in Settings > Computers > {on}, or run wsp init --on {}.", on.to_lowercase()),
    )
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
    usage(
        &format!(
            "the image on {on} holds no {} sign-in ({}), so a sub-thread could neither clone {repo} nor push its branch",
            host.label, host.cli
        ),
        &format!("Run wsp init --on {} again with the {} set to copy.", on.to_lowercase(), host.login),
    )
}

/// S6, on a cloud whose credentials the host keeps: gh on this computer, whose login the host adds to every git and gh
/// request a sub-thread sends, holds none.
// sub-thread.ts:277
// export const subHostNoSignInRefusal = (on: string, repo: string): Error =>
//   usage(`gh on this computer is not signed in to GitHub, so a sub-thread on ${on} could neither clone ${repo} nor push its branch`, "Run gh auth login in a terminal on this computer, then start the sub-thread again.");
pub fn sub_host_no_sign_in_refusal(on: &str, repo: &str) -> crate::exit::HostError {
    usage(
        &format!("gh on this computer is not signed in to GitHub, so a sub-thread on {on} could neither clone {repo} nor push its branch"),
        "Run gh auth login in a terminal on this computer, then start the sub-thread again.",
    )
}

/// S6, on a cloud whose credentials the host keeps: a remote on a git host other than GitHub, which the host adds no
/// credential for.
// sub-thread.ts:281
// export const subHostOtherGitRefusal = (on: string, host: string, repo: string): Error =>
//   usage(`a sub-thread on ${on} reaches GitHub alone through this computer, so it could not clone ${repo} on ${host}`, "Run this work on this computer.");
pub fn sub_host_other_git_refusal(on: &str, host: &str, repo: &str) -> crate::exit::HostError {
    usage(
        &format!("a sub-thread on {on} reaches GitHub alone through this computer, so it could not clone {repo} on {host}"),
        "Run this work on this computer.",
    )
}

/// S7: a size the plan does not sell, or no class's figures at all.
// sub-thread.ts:284
// export const subSizeRefusal = (on: string, word: string, sold: readonly SubSize[]): Error => {
//   const named = sold.map(s => `${s.name} ${subSizeWord(s)} (${rateWord(s.rateUsdPerHour)})`);
//   const list = named.length <= 1 ? (named[0] ?? "nothing") : `${named.slice(0, -1).join(", ")} and ${named.at(-1)!}`;
//   return usage(`${on} sells ${list} on this plan; ${word} is not one of them`, `Name one of them, or leave the size out for ${on}'s own.`);
// };
pub fn sub_size_refusal(on: &str, word: &str, sold: &[SubSize]) -> crate::exit::HostError {
    let named: Vec<String> = sold.iter().map(|s| format!("{} {} ({})", s.name, size_word_of(s), rate_word(s.rate_usd_per_hour))).collect();
    let list = match named.split_last() {
        None => "nothing".to_string(),
        Some((last, [])) => last.clone(),
        Some((last, rest)) => format!("{} and {last}", join_all(rest, ", ")),
    };
    usage(
        &format!("{on} sells {list} on this plan; {word} is not one of them"),
        &format!("Name one of them, or leave the size out for {on}'s own."),
    )
}

/// S8: a name git refuses for a branch.
// sub-thread.ts:290
// export const subBranchNameRefusal = (name: string): Error => usage(`${name} is not a name git takes for a branch`, "Name another, or leave --branch out for wsp/<slug>.");
pub fn sub_branch_name_refusal(name: &str) -> crate::exit::HostError {
    usage(&format!("{name} is not a name git takes for a branch"), "Name another, or leave --branch out for wsp/<slug>.")
}

/// S8: the result branch is the lead's own.
// sub-thread.ts:292
// export const subLeadBranchRefusal = (name: string): Error => usage(`the sub-thread pushes ${name}, and that is the lead's own branch`, "Name another, or leave --branch out for wsp/<slug>.");
pub fn sub_lead_branch_refusal(name: &str) -> crate::exit::HostError {
    usage(
        &format!("the sub-thread pushes {name}, and that is the lead's own branch"),
        "Name another, or leave --branch out for wsp/<slug>.",
    )
}

/// S9: a sub-machine runs its sub-thread alone.
// sub-thread.ts:294
// export const subMachineTakenRefusal = (machine: string, thread: string): Error =>
//   usage(`${machine} is the machine of sub-thread ${thread.slice(0, 8)} and runs that thread alone`, `Send to ${thread.slice(0, 8)} to continue it.`);
pub fn sub_machine_taken_refusal(machine: &str, thread: &str) -> crate::exit::HostError {
    let thread8 = utf16::cut(thread, 8);
    usage(
        &format!("{machine} is the machine of sub-thread {thread8} and runs that thread alone"),
        &format!("Send to {thread8} to continue it."),
    )
}

/// A second message into a sub-thread whose first is still waiting for its machine.
// sub-thread.ts:297
// export const subWaitingRefusal = (machine: string): Error =>
//   usage(`${machine} is waiting for a machine, and its sub-thread's first message waits with it`, "Send again once it starts.");
pub fn sub_waiting_refusal(machine: &str) -> crate::exit::HostError {
    usage(&format!("{machine} is waiting for a machine, and its sub-thread's first message waits with it"), "Send again once it starts.")
}

// sub-thread.ts:300
// in wsp_wire::types::sub_thread: SUB_DROPPED_LINE

/// S11: the agent named is not on the image.
// sub-thread.ts:302
// export const subNoAgentRefusal = (on: string, agent: string, carried: readonly string[]): Error =>
//   usage(`the image on ${on} carries no ${agent}`, `Add it to the recipe and build the image again, or name one it carries: ${carried.join(", ")}.`);
pub fn sub_no_agent_refusal(on: &str, agent: &str, carried: &[impl AsRef<str>]) -> crate::exit::HostError {
    usage(
        &format!("the image on {on} carries no {agent}"),
        &format!("Add it to the recipe and build the image again, or name one it carries: {}.", join_all(carried, ", ")),
    )
}

/// S12: the picked files come to more than a sub-thread carries.
// sub-thread.ts:305
// export const subSeedTooBigRefusal = (project: string, bytes: string, max: string, largest: readonly string[]): Error =>
//   usage(`the files picked for ${project} come to ${bytes}, over the ${max} a sub-thread carries; the largest are ${largest.join(", ")}`, `Pick again with wsp projects seed ${project}.`);
pub fn sub_seed_too_big_refusal(project: &str, bytes: &str, max: &str, largest: &[impl AsRef<str>]) -> crate::exit::HostError {
    usage(
        &format!(
            "the files picked for {project} come to {bytes}, over the {max} a sub-thread carries; the largest are {}",
            join_all(largest, ", ")
        ),
        &format!("Pick again with wsp projects seed {project}."),
    )
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
    match f {
        SubFault::Unusable { r#box, why, .. } => {
            format!("{on} gave {} with nothing able to run on it ({why}); wsp deleted it and started another.", r#box)
        }
        SubFault::Env { r#box } => {
            format!("{on}'s machine {} did not take wsp's setup within 2 minutes (exit 124); wsp deleted it and started another.", r#box)
        }
        SubFault::Cancelled { why, .. } => {
            format!("{on} could not get a machine for {machine} and cancelled the create ({why}); wsp started another.")
        }
    }
}

/// A sub-thread's turn the kernel ended for memory, as its finished line says it: the machine's size, that the
/// cloud's own processes hold part of it, and the next size up to start the work again at, where the cloud sells one.
// sub-thread.ts:319
// export function subOomLine(machine: string, on: string, size: Pick<SubSize, "name" | "cpu" | "memMb">, bigger: string | undefined): string {
//   const again = bigger !== undefined ? `Start the work again with size ${bigger}.` : `${on} sells no bigger size, so the work has to use less memory.`;
//   return `the agent on ${machine} was killed for memory: its machine is ${subSizeWords(size)}, and ${on}'s own processes hold part of that. ${again}`;
// }
pub fn sub_oom_line(machine: &str, on: &str, size: &SubSize, bigger: Option<&str>) -> String {
    let again = match bigger {
        Some(bigger) => format!("Start the work again with size {bigger}."),
        None => format!("{on} sells no bigger size, so the work has to use less memory."),
    };
    format!(
        "the agent on {machine} was killed for memory: its machine is {}, and {on}'s own processes hold part of that. {again}",
        sub_size_words(size)
    )
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
    let fix = "Start it again in a few minutes.";
    match (first, second) {
        (
            SubFault::Unusable { r#box: first_box, request_id: first_request, .. },
            SubFault::Unusable { r#box: second_box, request_id: second_request, .. },
        ) => {
            let request = second_request.clone().or_else(|| first_request.clone());
            let request = request.map_or(String::new(), |r| format!("; request {r}"));
            provider(
                &format!("{on} gave two machines in a row that could not run commands ({first_box}, {second_box}{request}); {on} is restoring badly right now"),
                fix,
            )
        }
        (SubFault::Env { r#box: first_box }, SubFault::Env { r#box: second_box }) => {
            provider(&format!("{on}'s machines are slow to take a setup right now ({first_box}, {second_box}, exit 124 on both)"), fix)
        }
        (SubFault::Cancelled { why: first_why, .. }, SubFault::Cancelled { why: second_why, .. }) => {
            provider(&format!("{on} cancelled two creates in a row for {machine} ({first_why}; {second_why})"), fix)
        }
        _ => {
            let said = |f: &SubFault| match f {
                SubFault::Env { r#box } => format!("{}: setup not taken, exit 124", r#box),
                SubFault::Unusable { r#box, why, .. } | SubFault::Cancelled { r#box, why } => format!("{}: {why}", r#box),
            };
            provider(
                &format!("{on} gave two machines in a row that failed before the sub-thread started ({}; {})", said(first), said(second)),
                fix,
            )
        }
    }
}

/// The line a start passes with when the lead's folder holds changes nobody committed.
// sub-thread.ts:338
// export const subUncommittedLine = (n: number, folder: string, branch: string, sha: string): string =>
//   `${n} uncommitted ${n === 1 ? "change" : "changes"} in ${folder} did not travel; the sub-thread starts at ${branch} as pushed (${sha.slice(0, 7)}).`;
pub fn sub_uncommitted_line(n: f64, folder: &str, branch: &str, sha: &str) -> String {
    format!(
        "{} uncommitted {} in {folder} did not travel; the sub-thread starts at {branch} as pushed ({}).",
        number_string(n),
        if n == 1.0 { "change" } else { "changes" },
        utf16::cut(sha, 7)
    )
}

/// Why nothing ignored went: nothing is picked for the project.
// sub-thread.ts:341
// export const subNothingPickedWhy = (project: string): string => `nothing is picked for ${project}`;
pub fn sub_nothing_picked_why(project: &str) -> String {
    format!("nothing is picked for {project}")
}

/// Why nothing ignored went: the project's folder is on another computer.
// sub-thread.ts:343
// export const subPickedElsewhereWhy = (project: string, computer: string): string => refusalParts(seedNotHereRefusal(project, computer)).said.replace(/\.$/, "");
pub fn sub_picked_elsewhere_why(project: &str, computer: &str) -> String {
    let said = refusal_parts(&seed_not_here_refusal(project, computer)).said;
    said.strip_suffix('.').unwrap_or(&said).to_string()
}

/// The line a start passes with when no git-ignored file was sent.
// sub-thread.ts:345
// export const subNotSentLine = (why: string, project: string): string =>
//   `no git-ignored files were sent: ${why}.${why.startsWith("nothing is picked") ? ` wsp projects seed ${project} shows what could go.` : ""}`;
pub fn sub_not_sent_line(why: &str, project: &str) -> String {
    let shows =
        if why.starts_with("nothing is picked") { format!(" wsp projects seed {project} shows what could go.") } else { String::new() };
    format!("no git-ignored files were sent: {why}.{shows}")
}

/// The line a start passes with when picked files stayed here: a sub-thread's machine is a provider's, which gets
/// no login, no file of values and no key, picked by name or found inside a picked folder.
// sub-thread.ts:349
// export const subStayedLine = (paths: readonly string[]): string =>
//   `${paths.join(", ")} stayed on this computer: a sub-thread's machine is a provider's, which gets no login, file of values or key.`;
pub fn sub_stayed_line(paths: &[impl AsRef<str>]) -> String {
    format!(
        "{} stayed on this computer: a sub-thread's machine is a provider's, which gets no login, file of values or key.",
        join_all(paths, ", ")
    )
}

/// The stage line for the picked files landing.
// sub-thread.ts:352
// export const subCopyingLine = (files: number): string => `copying ${files} ${files === 1 ? "file" : "files"}`;
pub fn sub_copying_line(files: f64) -> String {
    format!("copying {} {}", number_string(files), if files == 1.0 { "file" } else { "files" })
}

/// The first line a terminal prints: where the sub-thread runs.
// sub-thread.ts:354
// export const subOpenedLine = (threadId: string, on: string, folder: string): string => `thread ${threadId.slice(0, 8)} on ${on.toLowerCase()} in ${folder}`;
pub fn sub_opened_line(thread_id: &str, on: &str, folder: &str) -> String {
    format!("thread {} on {} in {folder}", utf16::cut(thread_id, 8), on.to_lowercase())
}

// The work a done read found not pushed, in the words the delete's question and the lead's refusal give it.
// sub-thread.ts:357
// private in the TS: port it as a private helper
// function heldWords(held: { files: number; commits: number }): string {
//   const parts = [held.commits > 0 ? `${held.commits} ${held.commits === 1 ? "commit" : "commits"}` : "", held.files > 0 ? `${held.files} changed ${held.files === 1 ? "file" : "files"}` : ""].filter(Boolean);
//   return parts.length === 0 ? "work" : parts.join(" and ");
// }
fn held_words(files: f64, commits: f64) -> String {
    let mut parts = Vec::new();
    if commits > 0.0 {
        parts.push(plural(commits, "commit"));
    }
    if files > 0.0 {
        parts.push(plural(files, "changed file"));
    }
    if parts.is_empty() {
        return "work".into();
    }
    parts.join(" and ")
}

// The work a done read found not pushed and the branch it is on, as the open thread's notice and a tile's card say it.
// sub-thread.ts:363
// private in the TS: port it as a private helper
// const heldOn = (held: { files: number; commits: number }, branch: string): string => `${heldWords(held)} on ${branch} ${held.files + held.commits === 1 ? "is" : "are"} not pushed`;
fn held_on(files: f64, commits: f64, branch: &str) -> String {
    format!("{} on {branch} {} not pushed", held_words(files, commits), if files + commits == 1.0 { "is" } else { "are" })
}

/// A held machine's cloud may delete its disk from `day`: Boat says it deletes at day 14, and a box stopped 18 days still
/// resumed whole (2026-10-15), so wsp warns and deletes nothing itself.
// sub-thread.ts:367
// export const diskMayGoLine = (on: string, day: string): string => `${on} may delete its disk from ${day}`;
pub fn disk_may_go_line(on: &str, day: &str) -> String {
    format!("{on} may delete its disk from {day}")
}

/// A not pushed sub-thread's reason on its tile's card, with the day its cloud may delete the disk from where it naps,
/// said first, since the card cuts the reason at three lines and a branch name can be long.
// sub-thread.ts:371
// export const subNotPushedReason = (held: { files: number; commits: number }, branch: string, kept?: { on: string; day: string }): string =>
//   kept === undefined ? `${capitalisedFirst(heldOn(held, branch))}, so its machine stays.` : `${diskMayGoLine(kept.on, kept.day)}, with ${heldWords(held)} on ${branch} not pushed.`;
pub fn sub_not_pushed_reason(held: &Held, branch: &str, kept: Option<&KeptOn>) -> String {
    match kept {
        None => format!("{}, so its machine stays.", upper_first(&held_on(held.files, held.commits, branch))),
        Some(kept) => {
            format!("{}, with {} on {branch} not pushed.", disk_may_go_line(&kept.on, &kept.day), held_words(held.files, held.commits))
        }
    }
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
    use super::{disk_may_go_line, held_words, upper_first, Held, KeptOn, SubStripWords};
    use crate::js::number_string;
    pub const RAISE: &str = "Raise machines at once";
    pub const PUSH_AGAIN: &str = "Push again";
    pub fn waiting_slot(on: &str, running: f64, at_once: f64) -> SubStripWords {
        SubStripWords {
            title: format!("Waiting for a machine on {on}"),
            note: format!(
                "{} of {} machines are running. It starts on its own when one frees.",
                number_string(running),
                number_string(at_once)
            ),
        }
    }
    pub fn waiting_minute(on: &str, used: f64, limit: f64) -> SubStripWords {
        SubStripWords {
            title: format!("Waiting for {on}'s next minute of starts"),
            note: format!(
                "{} of {} starts this minute are used. It starts on its own within the minute.",
                number_string(used),
                number_string(limit)
            ),
        }
    }
    pub fn not_pushed(held: &Held, branch: &str, refused: Option<&str>, kept: Option<&KeptOn>) -> SubStripWords {
        let why = match refused {
            None => format!("They are on {branch} on this machine."),
            Some(refused) => format!("The push to {branch} was refused: {refused}."),
        };
        let kept = kept.map_or(String::new(), |k| format!(", but {}", disk_may_go_line(&k.on, &k.day)));
        SubStripWords {
            title: format!("{} not pushed", upper_first(&held_words(held.files, held.commits))),
            note: format!("{why} This machine stays until the work is pushed or copied to your computer{kept}."),
        }
    }
    pub fn deleting(minutes: &str, branch: &str) -> SubStripWords {
        SubStripWords {
            title: format!("Its machine is deleted in {minutes}"),
            note: format!("Everything is pushed to {branch}. The transcript stays, and a message after the delete starts a new machine."),
        }
    }
    pub fn released(on: &str) -> SubStripWords {
        SubStripWords {
            title: "This thread's machine was deleted".into(),
            note: format!("A message starts a new one on {on}, and the conversation goes on."),
        }
    }
    pub fn oom(line: &str) -> SubStripWords {
        SubStripWords { title: "Killed for memory".into(), note: upper_first(line) }
    }
    pub fn cloud_gone(on: &str, here: &str) -> SubStripWords {
        SubStripWords {
            title: format!("{on} was removed from {here}"),
            note: format!("This thread has no machine to start until {on} is added again. Its transcript stays."),
        }
    }
    pub fn copy_to(computer: &str) -> String {
        format!("Copy to {computer}")
    }
    pub fn start_again(size: &str) -> String {
        format!("Start again at {size}")
    }
    pub fn add_again(on: &str) -> String {
        format!("Add {on} again")
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
fn shell_command_of(input: &str) -> String {
    match serde_json::from_str::<serde_json::Value>(input) {
        Ok(serde_json::Value::Object(fields)) => match fields.get("command") {
            Some(serde_json::Value::String(command)) => command.clone(),
            _ => input.to_string(),
        },
        _ => input.to_string(),
    }
}

/// Whether a shell call an agent made runs a git push.
// sub-thread.ts:415
// export const isGitPush = (input: string): boolean => /\bgit\b(?:\s+-\S+(?:\s+\S+)?)*\s+push\b/.test(shellCommandOf(input));
pub fn is_git_push(input: &str) -> bool {
    GIT_PUSH.is_match(&shell_command_of(input))
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
    let lines: Vec<&str> = output.split('\n').map(js_trim_end).collect();
    let line = lines
        .iter()
        .copied()
        .find(|l| l.starts_with("remote:") && !PUSH_PROGRESS.is_match(l))
        .or_else(|| lines.iter().copied().find(|l| l.starts_with("fatal:")))
        .or_else(|| lines.iter().copied().find(|l| PUSH_REJECTED.is_match(l)))
        .or_else(|| lines.iter().copied().find(|l| l.starts_with("error:")))?;
    let after_remote = line.strip_prefix("remote:").map_or(line, js_trim_start);
    let after_bang = js_trim_start(after_remote).strip_prefix("! ").unwrap_or(after_remote);
    let collapsed = js_collapse_whitespace(after_bang);
    let trimmed = js_trim(&collapsed);
    let said = trimmed.strip_suffix('.').unwrap_or(trimmed);
    if said.is_empty() {
        return None;
    }
    Some(utf16::cut(said, 200))
}

/// The message Start again sends the lead: the sub-thread by its title, started again at the size its kill named.
// sub-thread.ts:435
// export const subStartAgainPrompt = (title: string, on: string, size: string): string => `Start the work of the sub-thread "${title}" again on ${on} at size ${size}: its agent was killed for memory.`;
pub fn sub_start_again_prompt(title: &str, on: &str, size: &str) -> String {
    format!("Start the work of the sub-thread \"{title}\" again on {on} at size {size}: its agent was killed for memory.")
}

/// The message Push again sends the sub-thread.
// sub-thread.ts:438
// export const subPushAgainPrompt = (branch: string): string => `Push ${branch} again, and say what the remote answers.`;
pub fn sub_push_again_prompt(branch: &str) -> String {
    format!("Push {branch} again, and say what the remote answers.")
}

/// Whether a turn's failure is subOomLine's, which the strip over the composer says in its place.
// sub-thread.ts:441
// export const isSubOomLine = (line: string): boolean => /^the agent on .+ was killed for memory: /i.test(line);
pub fn is_sub_oom_line(line: &str) -> bool {
    // The `i` flag folds ASCII letters alone, so ASCII lowering of the whole line is the same test.
    let lower = line.to_ascii_lowercase();
    let Some(rest) = lower.strip_prefix("the agent on ") else {
        return false;
    };
    let head = rest.find(['\n', '\r', '\u{2028}', '\u{2029}']).unwrap_or(rest.len());
    rest[..head].match_indices(" was killed for memory: ").any(|(at, _)| at >= 1)
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
    use crate::js::utf16;
    use crate::words::base::plural;

    pub const DETAILS: &str = "Show details";
    pub const HIDE_DETAILS: &str = "Hide details";
    pub const TRY_AGAIN: &str = "Try again";
    pub fn branch(done: bool) -> String {
        String::from(if done { "Checked the branch" } else { "Checking the branch" })
    }
    pub fn branch_fact(branch: &str, sha: &str) -> String {
        format!("{branch} is pushed at {}", utf16::cut(sha, 7))
    }
    pub fn machine(on: &str, done: bool) -> String {
        if done {
            format!("Asked {on} for a machine")
        } else {
            format!("Asking {on} for a machine")
        }
    }
    pub fn files(n: f64, done: bool) -> String {
        format!("{} {}", if done { "Copied" } else { "Copying" }, plural(n, "picked file"))
    }
    pub fn agent(agent: &str) -> String {
        format!("Starting {agent}")
    }
    pub fn agent_fact(model: Option<&str>, effort: Option<&str>) -> Option<String> {
        let parts: Vec<&str> = [model, effort].into_iter().flatten().filter(|s| !s.is_empty()).collect();
        (!parts.is_empty()).then(|| parts.join(", "))
    }
    pub fn machine_fact(id: Option<&str>, states: &[&str]) -> Option<String> {
        id.map(|id| std::iter::once(id).chain(states.iter().copied()).collect::<Vec<&str>>().join(", "))
    }
    pub fn files_fact(paths: &[&str], bytes: &str) -> String {
        format!("{}, {bytes}", paths.join(", "))
    }
    pub fn not_sent(why: &str) -> String {
        format!("No git-ignored files were sent: {why}.")
    }
    pub fn starting(on: &str) -> String {
        format!("Starting a machine on {on}")
    }
    pub fn started(on: &str, took: &str) -> String {
        format!("Started on {on} in {took}")
    }
    pub fn failed(on: &str) -> String {
        format!("Could not start a machine on {on}")
    }
}
