// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/words/image.ts at the cut (383708c78): the image record and its copies as `wsp image` and
//! Settings > Image read them, project images, pins and the recipe's install lines, the lineage marks, and every line
//! a snapshot, a template and a seal end on. Each TS statement stands as a comment above its stub. See spec/image.md.

use crate::js::{number_string, utf16};
use crate::words::base::{fmt_threads, lower_first, plural};
use crate::words::init::{cloud_setup_words, golden_stage_word, init_stopped_line, SAVING_IMAGE_LINE};
use crate::words::units::{fmt_bytes, fmt_duration};
use serde::Serialize;
use wsp_wire::types::init_job::LoginState;
use wsp_wire::views::golden_image::{GoldenStage, SealedImage, SealedImageCopy};

/// --- the image and its copies, as `wsp image` and Settings > Image read them ---
/// What a record with no vault says: its copies ask for every sign-in again until the next version holds them.
// image.ts:10
pub const IMAGE_NO_VAULT: &str = "no sign-ins held; cut the next version to hold them";

/// What a record sealed at a provider says: it holds no sign-in by design, so there is nothing to cut again for.
// image.ts:12
pub const IMAGE_NO_SECRETS: &str = "no sign-ins on it";

/// Whether a copy of this record asks for every sign-in again: it was sealed holding none, and not at a provider,
/// whose seal holds none by design. The refusal that wants force and the card that offers to copy it anyway read
/// this one rule.
// image.ts:16
// export const copyAsksSignIns = (image: Pick<SealedImage, "vault" | "noSecrets">): boolean => image.vault === undefined && image.noSecrets !== true;
pub fn copy_asks_sign_ins(image: &SealedImage) -> bool {
    image.vault.is_none() && image.no_secrets != Some(true)
}

/// Why a fork at a provider is refused: the image was sealed before a provider's seal refused every sign-in, so its
/// disk may still hold them. imageRebuildFix is what to do.
// image.ts:19
// export const imageHeldSignInsLine = (where: string): string => `Your image on ${where} was sealed with sign-ins on it`;
pub fn image_held_sign_ins_line(r#where: &str) -> String {
    format!("Your image on {} was sealed with sign-ins on it", r#where)
}

// image.ts:20
// export const imageRebuildFix = (place: string): string => `Seal your image again for ${place}.`;
pub fn image_rebuild_fix(place: &str) -> String {
    format!("Seal your image again for {place}.")
}

/// What to do about each refusal a copy build meets before anything boots: said after it on a terminal, and in the
/// fix ink beside the Image card's press in the app. Only the no-sign-ins one names a flag: the card sends force on
/// such a record before the host could refuse it, so that fix is read on a terminal alone.
// image.ts:24
// export const COPY_BUILD_FIX = {
//   noCopy: "Start its tasks on a cloud or a computer that holds your image.",
//   noImage: "Build your image first; a copy is made from it.",
//   noRecipe: "Build the next version, then copy that one.",
//   noVault: "Build the next version to hold them, or run it again with --force to copy this one anyway.",
// } as const;
pub mod copy_build_fix {
    pub const NO_COPY: &str = "Start its tasks on a cloud or a computer that holds your image.";
    pub const NO_IMAGE: &str = "Build your image first; a copy is made from it.";
    pub const NO_RECIPE: &str = "Build the next version, then copy that one.";
    pub const NO_VAULT: &str = "Build the next version to hold them, or run it again with --force to copy this one anyway.";
}

/// What a host that has sealed nothing says. Named apart from the attachment lines beside it: those are pictures.
// image.ts:34
pub const NO_SEALED_IMAGE: &str = "no image yet";

/// The same where a road needed the image to fork and there is none.
// image.ts:36
pub const NO_IMAGE_YET: &str = "no image yet";

/// Whether a copy stands on the record as it is now: built at the record's hash, which is the whole of it. The
/// version is the place's own manifest number and says nothing about which record the copy came from, so a second
/// place's v1 built from the record's v2 is current. A copy with no hash was built before hashes were recorded and
/// no record matches it. The one rule: the refusal that will not rebuild a current copy reads it too.
// image.ts:44
// export function copyIsCurrent(image: Pick<SealedImage, "hash">, copy: Pick<SealedImageCopy, "hash">): boolean {
//   return copy.hash === image.hash;
// }
/// A copy with no hash is never current.
pub fn copy_is_current(image: &SealedImage, copy: &SealedImageCopy) -> bool {
    copy.hash.as_deref() == Some(image.hash.as_str())
}

// The states a login ends in with something of it on the machine, which is what the vault then carries: signed in
// there, copied from this computer, or there with no status command to prove it. The rest left nothing behind.
// image.ts:50
// private in the TS: port it as a private helper
// const LOGIN_ON_MACHINE: readonly LoginState[] = ["signed-in", "copied", "not-verified"];

/// How many of this image's logins ended with something on the machine for the vault to hold.
// image.ts:53
// export function sealedLoginsHeld(image: Pick<SealedImage, "logins">): number {
//   return image.logins.filter(l => LOGIN_ON_MACHINE.includes(l.state)).length;
// }
pub fn sealed_logins_held(image: &SealedImage) -> usize {
    image.logins.iter().filter(|login| matches!(login.state, LoginState::SignedIn | LoginState::Copied | LoginState::NotVerified)).count()
}

/// The record in one line: the version, its hash, how many sign-ins it holds and what its disk came to.
// image.ts:58
// export function sealedImageLine(image: SealedImage): string {
//   const held = image.noSecrets === true ? IMAGE_NO_SECRETS : image.vault === undefined ? IMAGE_NO_VAULT : `${plural(sealedLoginsHeld(image), "sign-in")} held, ${plural(image.vault.paths, "path")}`;
//   const size = image.usedBytes === undefined ? [] : [fmtBytes(image.usedBytes)];
//   return [`${image.name} v${image.version}`, image.hash, held, ...size, `sealed on ${image.sealedFrom}`].join("  ");
// }
pub fn sealed_image_line(image: &SealedImage) -> String {
    let held = if image.no_secrets == Some(true) {
        IMAGE_NO_SECRETS.to_string()
    } else {
        match &image.vault {
            None => IMAGE_NO_VAULT.to_string(),
            Some(vault) => {
                format!("{} held, {}", plural(sealed_logins_held(image) as f64, "sign-in"), plural(vault.paths.as_f64(), "path"))
            }
        }
    };
    let mut parts = vec![format!("{} v{}", image.name, number_string(image.version.as_f64())), image.hash.clone(), held];
    if let Some(bytes) = image.used_bytes {
        parts.push(fmt_bytes(bytes.as_f64()));
    }
    parts.push(format!("sealed on {}", image.sealed_from));
    parts.join("  ")
}

/// What a place's row says while a copy of the image is built there: the stage in the seal's own words, so the row
/// and the init sheet name one stage one way.
// image.ts:66
// export const copyBuildingLine = (stage: Exclude<GoldenStage, "failed">): string => `copying your image: ${lowerFirst(GOLDEN_STAGE_WORDS[stage])}`;
/// `stage` is never Failed (the TS type excludes it).
pub fn copy_building_line(stage: GoldenStage) -> String {
    format!("copying your image: {}", golden_stage_word(stage).map(lower_first).unwrap_or_default())
}

/// What the row says after a build there stopped: the seal's own headline, and the reason the way the sheet reads it.
/// It stands until the next build there starts: wsp image build, the next version cut, or the computer connecting
/// again.
// image.ts:71
// export const copyStoppedLine = (reason?: string): string => {
//   const head = lowerFirst(CLOUD_SETUP_WORDS.build.failed);
//   const said = reason === undefined ? "" : initStoppedLine(reason);
//   return said === "" ? head : `${head}: ${said}`;
// };
pub fn copy_stopped_line(reason: Option<&str>) -> String {
    let head = lower_first(cloud_setup_words::build::FAILED);
    let said = reason.map(init_stopped_line).unwrap_or_default();
    if said.is_empty() {
        head
    } else {
        format!("{head}: {said}")
    }
}

/// What one golden.stage frame says of the copy at its place: the stage while a build runs, the reason once one
/// stopped, and nothing once one sealed, when what stands is the copies' to say. The host's row, the app's store and
/// its image card all read a frame through this one rule.
// image.ts:80
// export type CopyBuild = { line: string; stopped: boolean };
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CopyBuild {
    pub line: String,
    pub stopped: bool,
}

// image.ts:81
// export const copyBuildOf = (frame: Pick<GoldenStageEvent, "stage" | "detail">): CopyBuild | undefined => {
//   if (frame.stage === "sealed") return undefined;
//   return frame.stage === "failed" ? { line: copyStoppedLine(frame.detail), stopped: true } : { line: copyBuildingLine(frame.stage), stopped: false };
// };
/// A golden.stage frame's stage and detail.
pub fn copy_build_of(stage: GoldenStage, detail: Option<&str>) -> Option<CopyBuild> {
    if matches!(stage, GoldenStage::Sealed) {
        return None;
    }
    if matches!(stage, GoldenStage::Failed) {
        return Some(CopyBuild { line: copy_stopped_line(detail), stopped: true });
    }
    Some(CopyBuild { line: copy_building_line(stage), stopped: false })
}

/// The two words a copy's standing is said in, either of which fits the slot the longer one needs.
// image.ts:87
pub const COPY_CURRENT: &str = "current";

// image.ts:88
pub const COPY_STALE: &str = "stale";

/// How a copy stands against the record, as the one word a person reads: current when it was built from the record
/// as it is now, stale when it was built from an older one. Nothing at all where the record holds no vault and was not
/// sealed at a provider: such a record was read back off its own copies rather than written at a seal, so it has
/// nothing to judge them by. The one home for the word and for that gate; `wsp image` and Settings > Image both read it.
// image.ts:94
// export function copyStanding(image: Pick<SealedImage, "hash" | "vault" | "noSecrets">, copy: Pick<SealedImageCopy, "hash">): string | undefined {
//   if (image.vault === undefined && image.noSecrets !== true) return undefined;
//   return copyIsCurrent(image, copy) ? COPY_CURRENT : COPY_STALE;
// }
pub fn copy_standing(image: &SealedImage, copy: &SealedImageCopy) -> Option<String> {
    if image.vault.is_none() && image.no_secrets != Some(true) {
        return None;
    }
    let word = if copy_is_current(image, copy) { COPY_CURRENT } else { COPY_STALE };
    Some(word.to_string())
}

/// One place's copy in one line: the place, the version it holds, its size where the provider reports one, and
/// whether it stands on the record as it is now.
// image.ts:101
// export function sealedCopyLine(image: SealedImage, copy: SealedImageCopy): string {
//   const size = copy.sizeBytes === undefined ? [] : [fmtBytes(copy.sizeBytes)];
//   const standing = copyStanding(image, copy);
//   return [copy.place, `v${copy.version}`, ...size, ...(standing === undefined ? [] : [standing])].join("  ");
// }
pub fn sealed_copy_line(image: &SealedImage, copy: &SealedImageCopy) -> String {
    let mut parts = vec![copy.place.clone(), format!("v{}", number_string(copy.version.as_f64()))];
    if let Some(bytes) = copy.size_bytes {
        parts.push(fmt_bytes(bytes.as_f64()));
    }
    if let Some(standing) = copy_standing(image, copy) {
        parts.push(standing);
    }
    parts.join("  ")
}

/// What a build at a place came to, as the line a person reads after it: the copy that place now holds, and, when
/// the place already stood on the record, that nothing was built.
// image.ts:109
// export function sealedBuiltLine(image: SealedImage, built: { copy: SealedImageCopy; built: boolean }): string {
//   const line = sealedCopyLine(image, built.copy);
//   return built.built ? line : `${line}  already built from this image; nothing was built`;
// }
pub fn sealed_built_line(image: &SealedImage, copy: &SealedImageCopy, built: bool) -> String {
    let line = sealed_copy_line(image, copy);
    if built {
        line
    } else {
        format!("{line}  already built from this image; nothing was built")
    }
}

/// What forgetting a workspace takes off this computer, the one sentence every client's confirmation shows.
// image.ts:161
// export function forgetNotice(threads: number): string {
//   return `Its record and ${fmtThreads(threads)} leave this computer; the computer it ran on is already gone.`;
// }
pub fn forget_notice(threads: f64) -> String {
    format!("Its record and {} leave this computer; the computer it ran on is already gone.", fmt_threads(threads))
}

/// One noun's change in a golden build line: "2 tools added".
// image.ts:166
// export interface GoldenChange {
//   count: number;
//   /** Singular; the line pluralises it. */
//   noun: string;
//   word: string;
// }
#[derive(Clone, Debug, PartialEq)]
pub struct GoldenChange {
    pub count: f64,
    /// Singular; the line pluralises it.
    pub noun: String,
    pub word: String,
}

/// How much of a checksum a line shows: enough to tell two apart, short enough that a reason line with both fits its cut.
// image.ts:183
pub const SUM_SHOWN: usize = 12;

// image.ts:184
// export const shortSum = (sha256: string): string => sha256.slice(0, SUM_SHOWN);
pub fn short_sum(sha256: &str) -> String {
    utf16::slice(sha256, 0, Some(SUM_SHOWN as i64))
}

/// The install step's failure when a download the recipe pinned does not hash to the recorded sum: what came down,
/// at which tag, and both sums, so a moved asset reads as a moved asset and never as a broken download.
// image.ts:188
// export function pinMismatchLine(what: string, tag: string, recorded: string, served: string): string {
//   return `${what} at ${tag} does not match the checksum recorded on its first install: recorded ${recorded}, served ${served}`;
// }
pub fn pin_mismatch_line(what: &str, tag: &str, recorded: &str, served: &str) -> String {
    format!("{what} at {tag} does not match the checksum recorded on its first install: recorded {recorded}, served {served}")
}

/// Why a tool in the recipe installs differently now: the road it takes moved. Both sides in the roads' own words.
// image.ts:193
// export function roadMovedLine(from: string, to: string): string {
//   return `now ${to}, was ${from}`;
// }
pub fn road_moved_line(from: &str, to: &str) -> String {
    format!("now {to}, was {from}")
}

/// Why a tool installs differently now: the version it installs at moved; a side with none reads as unpinned.
// image.ts:198
// export function versionMovedLine(from: string | undefined, to: string | undefined): string {
//   return `${from ?? "unpinned"} to ${to ?? "unpinned"}`;
// }
pub fn version_moved_line(from: Option<&str>, to: Option<&str>) -> String {
    format!("{} to {}", from.unwrap_or("unpinned"), to.unwrap_or("unpinned"))
}

/// The words for a tools row no road installs, where a road's own words would stand.
// image.ts:203
pub const NO_ROAD_WORDS: &str = "by no road";

/// What a row marked latest does on every place: the words `wsp recipe`, `wsp image` and the seal's stage all use.
// image.ts:206
pub const INSTALLS_LATEST: &str = "installs latest";

/// Why a tool installs differently now when its road stands: the lines the road runs are not the golden's.
// image.ts:234
pub const INSTALLER_MOVED_LINE: &str = "its install lines changed";

/// The app's line for a workspace still forked from an older golden version, offered the way the helper update is:
/// a state in words, never a badge. The move is the person's; nothing replaces a machine they are working on.
// image.ts:274
// export function behindGoldenLine(on: number, head: number): string {
//   return `on image v${on}, v${head} available`;
// }
pub fn behind_golden_line(on: f64, head: f64) -> String {
    format!("on image v{}, v{} available", number_string(on), number_string(head))
}

// The states a version row can be in, each as the muted mono word the row's marks column shows: state is text there,
// never a badge, and a missing tool's outcome indexes this table as it is. The keys are the wire's; the words are
// the person's, so a row says what the version is rather than the name the code holds it under. The volatile key's
// word says what such a version has behind it, a snapshot at the provider and no durable template, since that is
// the whole of what it is and what the provider can drop.
// image.ts:283
// export const LINEAGE_MARKS = { now: "now", head: "newest", fork: "this one", failed: "failed", skipped: "skipped", volatile: "snapshot only" } as const;

// image.ts:284
// export type LineageMark = keyof typeof LINEAGE_MARKS;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LineageMark {
    Now,
    Head,
    Fork,
    Failed,
    Skipped,
    Volatile,
}

impl LineageMark {
    /// `LINEAGE_MARKS[mark]`: now, newest, this one, failed, skipped, snapshot only.
    pub fn words(self) -> &'static str {
        match self {
            Self::Now => "now",
            Self::Head => "newest",
            Self::Fork => "this one",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
            Self::Volatile => "snapshot only",
        }
    }
}

// What is said for each folder git named no branch for: nothing in a branch slot or the diff pane's git mark, which
// show the branch or nothing until it is known, whatever kept it unknown; and the one line an empty diff pane says in
// place of a diff, only for a folder outside any repository, since an empty pane with no reason reads as broken while
// a refused read shows the cause git gave.
// image.ts:290
// export const REPO_STATE_WORDS = {
//   unknown: { pane: "" },
//   none: { pane: "This folder is not inside a git repository, so there is nothing to diff." },
//   refused: { pane: "" },
// } as const;

// image.ts:295
// export type RepoStateWord = keyof typeof REPO_STATE_WORDS;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepoStateWord {
    Unknown,
    None,
    Refused,
}

impl RepoStateWord {
    /// `REPO_STATE_WORDS[word].pane`: "" for unknown and refused, the not-a-repository sentence for none.
    pub fn pane(self) -> &'static str {
        match self {
            Self::Unknown => "",
            Self::None => "This folder is not inside a git repository, so there is nothing to diff.",
            Self::Refused => "",
        }
    }
}

/// A missing tool's row as the lineage shows it. A record sealed before the name and outcome were recorded still
/// carries its id, so it reads by that and as failed rather than as a blank row.
// image.ts:299
// export function missingToolRow(t: { id: string; name?: string; outcome?: GoldenMissingTool["outcome"]; note: string }): { name: string; note: string; mark: LineageMark } {
//   return { name: t.name === undefined || t.name === "" ? t.id : t.name, note: t.note, mark: t.outcome ?? "failed" };
// }
/// A missing tool as a record holds it: `outcome` is "skipped" or "failed" where recorded.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MissingTool {
    pub id: String,
    pub name: Option<String>,
    pub outcome: Option<LineageMark>,
    pub note: String,
}

/// Keys name, note, mark.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct MissingToolRow {
    pub name: String,
    pub note: String,
    pub mark: LineageMark,
}

pub fn missing_tool_row(t: &MissingTool) -> MissingToolRow {
    let name = match t.name.as_deref() {
        None | Some("") => t.id.clone(),
        Some(name) => name.to_string(),
    };
    MissingToolRow { name, note: t.note.clone(), mark: t.outcome.unwrap_or(LineageMark::Failed) }
}

/// What the provider answered one call with: the status, its message, the request id its reply carried when it
/// carried one (measured 2026-09-07: Solari's replies carry none), and the UTC time the reply landed.
// image.ts:305
// export interface ProviderAnswer {
//   status: number;
//   message: string;
//   requestId?: string;
//   at: string;
// }
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProviderAnswer {
    pub status: f64,
    pub message: String,
    pub request_id: Option<String>,
    pub at: String,
}

/// What the builder read at the provider after the last attempt: a state, or unread when the GET itself failed.
// image.ts:313
// export type BuilderReading = MachineState | "unread";
/// A MachineState's wire word, or Unread when the GET itself failed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuilderReading {
    State(String),
    Unread,
}

/// The provider's answer as a report to the provider needs it: status, message, and the request id; without one,
/// that the reply carried none and when it landed, never an empty id.
// image.ts:317
// export function providerAnswerLine(a: ProviderAnswer): string {
//   return `${a.status} ${a.message} (${a.requestId !== undefined ? `request ${a.requestId}` : `no request id from the provider, at ${a.at}`})`;
// }
pub fn provider_answer_line(a: &ProviderAnswer) -> String {
    let reply = match &a.request_id {
        Some(id) => format!("request {id}"),
        None => format!("no request id from the provider, at {}", a.at),
    };
    format!("{} {} ({reply})", number_string(a.status), a.message)
}

/// What df read of a machine's root disk before a snapshot was asked for, or why it read nothing.
// image.ts:322
// export type DiskUse = { kind: "use"; usedBytes: number; sizeBytes: number } | { kind: "unknown"; reason: string };
#[derive(Clone, Debug, PartialEq)]
pub enum DiskUse {
    Use { used_bytes: f64, size_bytes: f64 },
    Unknown { reason: String },
}

/// The snapshotting stage's line after one refused attempt with another to come: which attempt, what the provider
/// said, what the builder reads at the provider, and when the next attempt is.
// image.ts:361
// export function snapshotAttemptLine(attempt: number, attempts: number, answer: ProviderAnswer, builderState: BuilderReading, retryMs: number): string {
//   return `attempt ${attempt} of ${attempts} answered ${providerAnswerLine(answer)}; the builder reads ${builderState}, next attempt in ${fmtDuration(retryMs)}`;
// }
pub fn snapshot_attempt_line(
    attempt: f64,
    attempts: f64,
    answer: &ProviderAnswer,
    builder_state: &BuilderReading,
    retry_ms: f64,
) -> String {
    let reading = match builder_state {
        BuilderReading::State(state) => state.as_str(),
        BuilderReading::Unread => "unread",
    };
    format!(
        "attempt {} of {} answered {}; the builder reads {reading}, next attempt in {}",
        number_string(attempt),
        number_string(attempts),
        provider_answer_line(answer),
        fmt_duration(retry_ms, None)
    )
}

/// The seal's failure line once the snapshot is given up on: how many attempts, the last answer, and whether the
/// provider still has the builder. A builder the provider answers 404 for is named gone at the provider's hand.
// image.ts:367
// export function snapshotFailedLine(attempts: number, answer: ProviderAnswer, builderState: BuilderReading, readError?: string): string {
//   const head = `the snapshot failed ${plural(attempts, "time")}: the provider answered ${providerAnswerLine(answer)}`;
//   if (builderState === "gone") return `${head} and no longer has the builder (404)`;
//   if (builderState === "unread") return `${head} and could not be read about the builder (${readError ?? "no reason given"})`;
//   return `${head} while the builder read ${builderState}`;
// }
pub fn snapshot_failed_line(attempts: f64, answer: &ProviderAnswer, builder_state: &BuilderReading, read_error: Option<&str>) -> String {
    let head = format!("the snapshot failed {}: the provider answered {}", plural(attempts, "time"), provider_answer_line(answer));
    match builder_state {
        BuilderReading::State(state) if state.as_str() == "gone" => format!("{head} and no longer has the builder (404)"),
        BuilderReading::Unread => format!("{head} and could not be read about the builder ({})", read_error.unwrap_or("no reason given")),
        BuilderReading::State(state) => format!("{head} while the builder read {state}"),
    }
}

/// The promoting stage's line for one read of the saved image: what the provider says of it and whether the seal asks
/// again. Ready is the last line the stage writes; the template's id is the provider's and reaches no screen.
// image.ts:376
// export function templateStatusLine(status: string): string {
//   return status === "ready" ? "the image is saved" : `${SAVING_IMAGE_LINE}, the provider says ${status}; asking again`;
// }
pub fn template_status_line(status: &str) -> String {
    if status == "ready" {
        "the image is saved".to_string()
    } else {
        format!("{SAVING_IMAGE_LINE}, the provider says {status}; asking again")
    }
}

/// The seal's failure line when the provider marks the template failed: forks would have nothing to boot from.
// image.ts:381
// export function templateFailedLine(templateId: string, reason: string | undefined): string {
//   return `the provider failed the template ${templateId}: ${reason ?? "no reason given"}`;
// }
pub fn template_failed_line(template_id: &str, reason: Option<&str>) -> String {
    format!("the provider failed the template {template_id}: {}", reason.unwrap_or("no reason given"))
}

/// The seal's failure line when the template never read ready inside the wait.
// image.ts:386
// export function templateWaitedLine(templateId: string, status: string, waitedMs: number): string {
//   return `the template ${templateId} still reads ${status} after ${fmtDuration(waitedMs)}`;
// }
pub fn template_waited_line(template_id: &str, status: &str, waited_ms: f64) -> String {
    format!("the template {template_id} still reads {status} after {}", fmt_duration(waited_ms, None))
}

/// The doctor's line per version it made durable: the image and version, the template it promoted, and when other
/// templates already carry the name (another host's, or a run that recorded nothing), how many; none when the count
/// is zero or the listing was not given.
// image.ts:393
// export function templateRecordedLine(image: string, version: number, templateId: string, sharing: number | undefined): string {
//   const head = `image ${image} v${version}: template ${templateId} promoted and recorded`;
//   return sharing === undefined || sharing === 0 ? head : `${head}; ${sharing} other ${sharing === 1 ? "template carries" : "templates carry"} its name`;
// }
pub fn template_recorded_line(image: &str, version: f64, template_id: &str, sharing: Option<f64>) -> String {
    let head = format!("image {image} v{}: template {template_id} promoted and recorded", number_string(version));
    match sharing {
        Some(count) if count != 0.0 => {
            let carry = if count == 1.0 { "template carries" } else { "templates carry" };
            format!("{head}; {} other {carry} its name", number_string(count))
        }
        _ => head,
    }
}

/// The doctor's line per version it could not make durable and why: a lost snapshot in the provider's own words is
/// left to the doctor's rebuild road below it.
// image.ts:400
// export function templateSkippedLine(image: string, version: number, reason: string): string {
//   return `image ${image} v${version}: no template recorded, ${reason}`;
// }
pub fn template_skipped_line(image: &str, version: f64, reason: &str) -> String {
    format!("image {image} v{}: no template recorded, {reason}", number_string(version))
}

/// What a version's row says when the provider answers 404 for its snapshot: the vanish the templates exist to outlive.
// image.ts:405
pub const SNAPSHOT_GONE_REASON: &str = "its snapshot is gone at the provider";

/// The doctor's line on a backend whose capabilities lack templates: nothing to promote, nothing wrong.
// image.ts:408
pub const NO_TEMPLATES_LINE: &str = "this backend has no templates; image versions stay as snapshots";

/// The doctor's line on a backend that cannot list snapshots: nothing to split, nothing to clean.
// image.ts:411
pub const NO_SNAPSHOT_LISTING: &str = "this backend lists no snapshots; nothing to split by owner";

/// The doctor's line for the machines its teardown check found on the account that this host did not make: each
/// named with the host that did, left alone and never a failure, since two computers on one account each stand
/// their own.
// image.ts:416
// export function otherHostsMachinesLine(machines: readonly { id: string; owner?: string }[]): string {
//   const named = machines.map(m => `${m.id} (${m.owner === undefined ? "no owner" : `owner ${m.owner}`})`);
//   return `left alone ${plural(machines.length, "machine")} this host did not make: ${named.join(", ")}`;
// }
/// A machine on the account and the host that made it, where named.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OwnedMachine {
    pub id: String,
    pub owner: Option<String>,
}

pub fn other_hosts_machines_line(machines: &[OwnedMachine]) -> String {
    let named: Vec<String> = machines
        .iter()
        .map(|m| {
            let owner = match &m.owner {
                None => "no owner".to_string(),
                Some(owner) => format!("owner {owner}"),
            };
            format!("{} ({owner})", m.id)
        })
        .collect();
    format!("left alone {} this host did not make: {}", plural(machines.len() as f64, "machine"), named.join(", "))
}

/// Why a seal was refused before it took the snapshot: the builder holds a file a sign-in leaves behind. The paths
/// are the guest's own, so whoever reads the line can go and look at them.
// image.ts:440
// export function credentialOnBuilderLine(paths: readonly string[]): string {
//   return `the builder holds a sign-in file at ${paths.join(", ")}; sign-ins never sit in an image, so nothing was sealed`;
// }
pub fn credential_on_builder_line(paths: &[impl AsRef<str>]) -> String {
    let paths: Vec<&str> = paths.iter().map(|p| p.as_ref()).collect();
    format!("the builder holds a sign-in file at {}; sign-ins never sit in an image, so nothing was sealed", paths.join(", "))
}
