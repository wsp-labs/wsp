// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/words/init.ts at the cut (383708c78): every word of wsp init's job, its screens, its build
//! rows and sign-ins, the counts and lines the sidebar, the toast and the terminal read off one job, and the agent
//! road's brief. Each TS statement stands as a comment above its stub. See spec/init.md.

use crate::js::{js_trim, number_string, to_fixed};
use crate::words::computer::{this_computer, Platform};
use crate::words::units::{fmt_bytes, fmt_size};
use regex::Regex;
use wsp_wire::types::init_job::{LoginChoice, LoginState};
use wsp_wire::views::golden_image::GoldenStage;
use wsp_wire::wire::capabilities::{InitSetup, WorkspaceSize};

/// Every word of the steps that choose and build the image, drawn under a computer's Image card and, where a word is
/// the host's, in the terminal too. Headlines are a step's title, tops the one sentence under it, keycaps the one
/// primary button a step has. Nothing here asks the person to run a command, and nothing here is a word for a
/// computer or a provider: those are PLACES_WORDS.
// init.ts:12
// export const CLOUD_SETUP_WORDS = {
//   choice: {
//     headline: "What goes on your image",
//     top: "What your agents need goes on one image, built once and copied for every task",
//     manual: "Choose what goes on the image",
//     agent: "Let an agent choose from your usage",
//     agentWith: "with",
//     /** An agent here whose thread wsp cannot hand the recipe tools at launch: shown, never offered. */
//     noTools: "no wsp tools yet",
//     /** What the agent row says with no agent on this computer at all. */
//     none: "no agent here",
//     keycap: "Continue",
//   },
//   keys: {
//     saved: "saved",
//     unset: "not set",
//     /** Under the field when the provider answered and refused what was typed; its own status and word follow. */
//     refused: (provider: string): string => `${providerKeyName(provider)} refused this key`,
//     /** The build's line for a key already saved that the provider refuses, read before the first stage, naming that
//      * provider by PROVIDER_KEY_WORDS. */
//     refusedSaved: (provider: string): string => `${providerKeyName(provider)} refused the saved key`,
//     /** Under the field when nothing came back about the key at all; what this computer saw follows. */
//     unchecked: (provider: string): string => `${providerKeyName(provider)} could not be reached to check the key`,
//     /** What the build offers when the saved key was refused: that provider's key field, not another build. */
//     changeKey: "Change the key",
//   },
//   screen: {
//     /** A row wsp puts on every image, so its tick cannot be taken off. */
//     alwaysOn: "always on",
//     /** What that row says in place of always on once it is pressed. */
//     alwaysOnWhy: "wsp needs it on every image, so it stays on.",
//     keycap: "Continue",
//     back: "Back",
//     again: "Start over",
//     /** A sign-in row whose tool is off the image: its picker is fixed on skip. */
//     notOnImage: "not on the image",
//     /** A sign-in row the catalog locked out. */
//     leftAlone: "left alone",
//     /** A sign-in row whose tool stops on a question nobody but the person can answer, so the machine is no road for it. */
//     asksYou: "asks questions only you can answer",
//   },
//   build: {
//     headline: "Building your image",
//     top: "The computer starts, installs what you ticked and is saved as the image every task starts from",
//     /** The one stage row the sign-ins fold into, its sub-rows one per sign-in. */
//     signingIn: "Signing in on the computer",
//     /** The slide the build becomes while that stage runs: room to act on each sign-in. */
//     slideHeadline: "Sign in on the computer",
//     slideTop: "Each one opens a page on this computer, and the image keeps the sign-in",
//     open: "Open sign-in",
//     retry: "Retry",
//     codeAsk: "Paste the code from the page",
//     codeSubmit: "Submit",
//     cancel: "Cancel the build",
//     /** Why Cancel is held while the seal runs: the host's refusal and the line under the build, one sentence. */
//     cannotStop: "The image is being saved. The snapshot and the save cannot be stopped.",
//     cancelSure: "Stop the build",
//     cancelWhy: "The computer it was building on is thrown away and nothing is saved",
//     cancelKeep: "Keep building",
//     done: "Your image is ready",
//     failed: "The build stopped",
//     /** The headline of a build the person stopped, so the screen never reads as the machine's doing. */
//     stopped: "You stopped the build",
//     again: "Start over",
//   },
//   agent: {
//     headline: "Reading what your agents used",
//     top: "Your agent reads this computer and writes the recipe the next screens start from",
//     /** What the thread this road opens is called, which is what the sidebar's row for it reads. */
//     title: "Build your image",
//     /** The link to the thread doing the work, which the sidebar focuses. */
//     open: "Open the thread",
//     /** The headline once the turn ended without the recipe, and the two ways on from there. */
//     failed: "Your agent stopped",
//     /** The sentence under it when nothing named a reason, which is a job stopped from another client. */
//     stopped: "The thread ended before the recipe was written",
//     retry: "Retry",
//     again: "Start over",
//     /** What the block under the title says before the thread's first line. */
//     waiting: "waiting for the thread's first line",
//   },
//   reading: {
//     headline: "Reading this computer",
//     top: "What is installed here and what your agents used decides what the image starts with",
//     /** The one row the card shows until the first fact lands, so the work reads as started. */
//     first: "This computer",
//   },
// } as const;
pub mod cloud_setup_words {
    pub mod choice {
        pub const HEADLINE: &str = "What goes on your image";
        pub const TOP: &str = "What your agents need goes on one image, built once and copied for every task";
        pub const MANUAL: &str = "Choose what goes on the image";
        pub const AGENT: &str = "Let an agent choose from your usage";
        pub const AGENT_WITH: &str = "with";
        pub const NO_TOOLS: &str = "no wsp tools yet";
        pub const NONE: &str = "no agent here";
        pub const KEYCAP: &str = "Continue";
    }
    pub mod keys {
        pub const SAVED: &str = "saved";
        pub const UNSET: &str = "not set";
        /// TS: (provider ) => `${providerKeyName(provider)} refused this key`
        pub fn refused(provider: &str) -> String {
            format!("{} refused this key", crate::place_word::provider_key_name(provider))
        }
        /// TS: (provider ) => `${providerKeyName(provider)} refused the saved key`
        pub fn refused_saved(provider: &str) -> String {
            format!("{} refused the saved key", crate::place_word::provider_key_name(provider))
        }
        /// TS: (provider ) => `${providerKeyName(provider)} could not be reached to check the key`
        pub fn unchecked(provider: &str) -> String {
            format!("{} could not be reached to check the key", crate::place_word::provider_key_name(provider))
        }
        pub const CHANGE_KEY: &str = "Change the key";
    }
    pub mod screen {
        pub const ALWAYS_ON: &str = "always on";
        pub const ALWAYS_ON_WHY: &str = "wsp needs it on every image, so it stays on.";
        pub const KEYCAP: &str = "Continue";
        pub const BACK: &str = "Back";
        pub const AGAIN: &str = "Start over";
        pub const NOT_ON_IMAGE: &str = "not on the image";
        pub const LEFT_ALONE: &str = "left alone";
        pub const ASKS_YOU: &str = "asks questions only you can answer";
    }
    pub mod build {
        pub const HEADLINE: &str = "Building your image";
        pub const TOP: &str = "The computer starts, installs what you ticked and is saved as the image every task starts from";
        pub const SIGNING_IN: &str = "Signing in on the computer";
        pub const SLIDE_HEADLINE: &str = "Sign in on the computer";
        pub const SLIDE_TOP: &str = "Each one opens a page on this computer, and the image keeps the sign-in";
        pub const OPEN: &str = "Open sign-in";
        pub const RETRY: &str = "Retry";
        pub const CODE_ASK: &str = "Paste the code from the page";
        pub const CODE_SUBMIT: &str = "Submit";
        pub const CANCEL: &str = "Cancel the build";
        pub const CANNOT_STOP: &str = "The image is being saved. The snapshot and the save cannot be stopped.";
        pub const CANCEL_SURE: &str = "Stop the build";
        pub const CANCEL_WHY: &str = "The computer it was building on is thrown away and nothing is saved";
        pub const CANCEL_KEEP: &str = "Keep building";
        pub const DONE: &str = "Your image is ready";
        pub const FAILED: &str = "The build stopped";
        pub const STOPPED: &str = "You stopped the build";
        pub const AGAIN: &str = "Start over";
    }
    pub mod agent {
        pub const HEADLINE: &str = "Reading what your agents used";
        pub const TOP: &str = "Your agent reads this computer and writes the recipe the next screens start from";
        pub const TITLE: &str = "Build your image";
        pub const OPEN: &str = "Open the thread";
        pub const FAILED: &str = "Your agent stopped";
        pub const STOPPED: &str = "The thread ended before the recipe was written";
        pub const RETRY: &str = "Retry";
        pub const AGAIN: &str = "Start over";
        pub const WAITING: &str = "waiting for the thread's first line";
    }
    pub mod reading {
        pub const HEADLINE: &str = "Reading this computer";
        pub const TOP: &str = "What is installed here and what your agents used decides what the image starts with";
        pub const FIRST: &str = "This computer";
    }
}

/// What a login nobody signed in during the build is left to, and the one place those words are written: the answer
/// that defers a browser sign-in to the workspace, and the second half of the word a sign-in that hit the build's cap
/// ends on.
// init.ts:104
pub const SIGN_IN_LATER: &str = "sign in when you first need it";

/// The word a sign-in that never landed during the build ends on, deferred at the picker or stopped by the cap: it
/// says what is true of the machine and what the person does about it, in that order.
// init.ts:108
// export const SIGN_IN_DEFERRED_WORD = `not signed in, ${SIGN_IN_LATER}`;
pub const SIGN_IN_DEFERRED_WORD: &str = "not signed in, sign in when you first need it";

// One answer the sign-ins step can give, in the words its row and the counts beside it print. One entry per answer,
// keyed by the union itself, so a fifth cannot be offered before it is named here; the order the arrows walk them is
// LOGIN_CHOICES' own. This is the one home for those words: the terminal's screens and a client that draws the step
// itself both read them from here.
// init.ts:130
// export interface SignInAnswer {
//   /** The row's own column, for the computer the run is reading: the copy answer names it. */
//   label(platform: "darwin" | "linux"): string;
//   /** The word a count is made of ("2 copy  1 during the build"). */
//   short: string;
// }

// init.ts:137
// export const SIGN_IN_ANSWERS: Record<LoginChoice, SignInAnswer> = {
//   copy: { label: platform => `copy from ${thisComputer(platform)}`, short: "copy" },
//   machine: { label: () => "sign in during the build", short: "during the build" },
//   later: { label: () => SIGN_IN_LATER, short: "when you need it" },
//   key: { label: () => "API key", short: "API key" },
//   skip: { label: () => "skip", short: "skip" },
//   token: { label: () => "token from this computer", short: "token" },
// };
/// `SIGN_IN_ANSWERS[choice].label(platform)`, word for word.
pub fn sign_in_answer_label(choice: LoginChoice, platform: Platform) -> String {
    match choice {
        LoginChoice::Copy => format!("copy from {}", this_computer(platform)),
        LoginChoice::Machine => "sign in during the build".to_string(),
        LoginChoice::Later => SIGN_IN_LATER.to_string(),
        LoginChoice::Key => "API key".to_string(),
        LoginChoice::Skip => "skip".to_string(),
        LoginChoice::Token => "token from this computer".to_string(),
    }
}

/// `SIGN_IN_ANSWERS[choice].short`, word for word.
pub fn sign_in_answer_short(choice: LoginChoice) -> &'static str {
    match choice {
        LoginChoice::Copy => "copy",
        LoginChoice::Machine => "during the build",
        LoginChoice::Later => "when you need it",
        LoginChoice::Key => "API key",
        LoginChoice::Skip => "skip",
        LoginChoice::Token => "token",
    }
}

/// The state of a sign-in as a word, the one spelling the terminal's rows and the modal's rows print.
// init.ts:153
// export const LOGIN_STATE_WORDS: Record<LoginState, string> = {
//   "signed-in": "signed in",
//   "not-signed-in": "not signed in",
//   copied: "copied",
//   "not-verified": "not verified",
//   skipped: "skipped",
//   deferred: SIGN_IN_DEFERRED_WORD,
// };
/// `LOGIN_STATE_WORDS[state]`, word for word.
pub fn login_state_word(state: LoginState) -> &'static str {
    match state {
        LoginState::SignedIn => "signed in",
        LoginState::NotSignedIn => "not signed in",
        LoginState::Copied => "copied",
        LoginState::NotVerified => "not verified",
        LoginState::Skipped => "skipped",
        LoginState::Deferred => SIGN_IN_DEFERRED_WORD,
    }
}

/// Every build stage in plain words, the one table the app's rows and the terminal's lines read.
// init.ts:244
// export const GOLDEN_STAGE_WORDS: Record<Exclude<GoldenStage, "failed">, string> = {
//   creating: "Creating the machine",
//   "deploying-daemon": "Installing the base tools",
//   "applying-setup": "Applying your setup",
//   "uploading-files": "Copying your files",
//   "installing-harness": "Installing agents",
//   "installing-tools": "Installing tools",
//   "installing-mcp": "Installing MCP servers",
//   ready: "Checking the machine answers",
//   snapshotting: "Taking the snapshot",
//   promoting: "Saving the image",
//   "smoke-forking": "Checking a fork boots",
//   sealed: "Finishing",
// };
/// `GOLDEN_STAGE_WORDS[stage]`, word for word; None for Failed, which has no row word.
pub fn golden_stage_word(stage: GoldenStage) -> Option<&'static str> {
    match stage {
        GoldenStage::Creating => Some("Creating the machine"),
        GoldenStage::DeployingDaemon => Some("Installing the base tools"),
        GoldenStage::ApplyingSetup => Some("Applying your setup"),
        GoldenStage::UploadingFiles => Some("Copying your files"),
        GoldenStage::InstallingHarness => Some("Installing agents"),
        GoldenStage::InstallingTools => Some("Installing tools"),
        GoldenStage::InstallingMcp => Some("Installing MCP servers"),
        GoldenStage::Ready => Some("Checking the machine answers"),
        GoldenStage::Snapshotting => Some("Taking the snapshot"),
        GoldenStage::Promoting => Some("Saving the image"),
        GoldenStage::SmokeForking => Some("Checking a fork boots"),
        GoldenStage::Sealed => Some("Finishing"),
        GoldenStage::Failed => None,
    }
}

/// The snapshot stage's first line, while the guest's dirty pages are written to its disk ahead of the copy.
// init.ts:273
pub const DISK_SYNC_LINE: &str = "syncing the disk";

/// The snapshot stage's line when the guest's counters read non-zero once the sync returned, in kB as /proc/meminfo
/// counts them. The snapshot goes ahead; nothing waits on a writer.
// init.ts:277
// export const diskUnsettledLine = (reading: { dirtyKb: number; writebackKb: number }): string =>
//   `synced, Dirty ${fmtBytes(reading.dirtyKb * KIB)} and Writeback ${fmtBytes(reading.writebackKb * KIB)} remain; a writer is still running`;
pub fn disk_unsettled_line(dirty_kb: f64, writeback_kb: f64) -> String {
    format!(
        "synced, Dirty {} and Writeback {} remain; a writer is still running",
        fmt_bytes(dirty_kb * 1024.0),
        fmt_bytes(writeback_kb * 1024.0)
    )
}

/// A snapshot refused because the guest's sync did not succeed, with the machine's own answer.
// init.ts:281
// export const diskSyncFailedLine = (answer: string): string => `the disk could not be synced (${answer}); nothing was snapshotted`;
pub fn disk_sync_failed_line(answer: &str) -> String {
    format!("the disk could not be synced ({answer}); nothing was snapshotted")
}

/// The snapshot stage's one line: what is being snapshotted in words a person can use, never the snapshot's name,
/// and the size when the builder's disk could be read.
// init.ts:285
// export const snapshotStageLine = (bytes: number | undefined): string => `snapshotting${bytes === undefined ? "" : ` about ${fmtBytes(bytes)}`}, usually under a minute`;
pub fn snapshot_stage_line(bytes: Option<f64>) -> String {
    match bytes {
        None => "snapshotting, usually under a minute".to_string(),
        Some(bytes) => format!("snapshotting about {}, usually under a minute", fmt_bytes(bytes)),
    }
}

/// The snapshot stage's line while the layer is written: the bytes so far, over what the machine had written once
/// the backend has counted it. The stage's own clock says how long it has taken.
// init.ts:289
// export const snapshotProgressLine = (bytes: number, total: number | undefined): string =>
//   total === undefined ? `snapshotting, ${fmtBytes(bytes)} written` : `snapshotting, ${fmtBytes(bytes)} of about ${fmtBytes(total)} written`;
pub fn snapshot_progress_line(bytes: f64, total: Option<f64>) -> String {
    match total {
        None => format!("snapshotting, {} written", fmt_bytes(bytes)),
        Some(total) => format!("snapshotting, {} of about {} written", fmt_bytes(bytes), fmt_bytes(total)),
    }
}

/// The save stage's one line.
// init.ts:293
pub const SAVING_IMAGE_LINE: &str = "saving the image";

/// A stage's line for a machine its rollback could not remove, with the provider's own refusal: it goes in the stage's
/// block, never in the headline, which stays the failure's own sentence; the machine's row carries the retries.
// init.ts:360
// export const machineLeftLine = (reason: string): string => `the machine could not be removed and bills on: ${reason}`;
pub fn machine_left_line(reason: &str) -> String {
    format!("the machine could not be removed and bills on: {reason}")
}

/// What a stopped build says when every part of the error is the network going away.
// init.ts:187
const NETWORK_LOST_LINE: &str = "This computer lost its network";

// The failures a provider client raises when the network is gone rather than when the provider refused.
// init.ts:370
// private in the TS: port it as a private helper
// const OFFLINE_FAILURE = /^fetch failed$|\b(ENOTFOUND|EAI_AGAIN|ECONNREFUSED|ENETDOWN|ENETUNREACH|EHOSTUNREACH)\b/;

/// The failures a provider client raises when the network is gone, not when the provider refused.
fn is_offline(part: &str) -> bool {
    let offline = Regex::new(r"^fetch failed$|(?-u:\b)(ENOTFOUND|EAI_AGAIN|ECONNREFUSED|ENETDOWN|ENETUNREACH|EHOSTUNREACH)(?-u:\b)")
        .expect("a fixed pattern");
    offline.is_match(part)
}

/// The sentence a stopped build shows for what happened: this computer's own word when every part of the error is
/// the network going away, else the error's parts said once. A run that failed the same way twice reports it twice;
/// the person reads one reason, not a list.
// init.ts:385
// export function initStoppedLine(error: string): string {
//   const parts = [...new Set(error.split(";").map(p => p.trim()).filter(p => p !== ""))];
//   if (parts.length === 0) return error;
//   return parts.every(p => OFFLINE_FAILURE.test(p)) ? NETWORK_LOST_LINE : parts.join("; ");
// }
pub fn init_stopped_line(error: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in error.split(';').map(js_trim).filter(|part| !part.is_empty()) {
        if !parts.contains(&part) {
            parts.push(part);
        }
    }
    if parts.is_empty() {
        return error.to_string();
    }
    if parts.iter().all(|part| is_offline(part)) {
        NETWORK_LOST_LINE.to_string()
    } else {
        parts.join("; ")
    }
}

/// What the app says when it needs the person: the toast's opening and a system notification's title.
// init.ts:440
pub const NEEDS_YOU: &str = "wsp needs you";

/// What a window or tab title leads with while a need stands, so a person reading only the title sees it.
// init.ts:446
pub const NEEDS_YOU_MARK: &str = "• ";

/// The title with the mark on it while a need stands and without it otherwise, from a title that may already carry
/// one: the same title goes through this on every change, so the mark can never double or stick.
// init.ts:450
// export function titleWithNeed(title: string, needed: boolean): string {
//   const plain = title.startsWith(NEEDS_YOU_MARK) ? title.slice(NEEDS_YOU_MARK.length) : title;
//   return needed ? `${NEEDS_YOU_MARK}${plain}` : plain;
// }
pub fn title_with_need(title: &str, needed: bool) -> String {
    let plain = title.strip_prefix(NEEDS_YOU_MARK).unwrap_or(title);
    if needed {
        format!("{NEEDS_YOU_MARK}{plain}")
    } else {
        plain.to_string()
    }
}

/// The provider's own answer about a key: the status it replied with and the word it used, so a person reads whose
/// refusal they are looking at rather than ours.
// init.ts:528
// export function providerSaidLine(status: number, said: string): string {
//   return said === "" ? String(status) : `${status} ${said}`;
// }
pub fn provider_said_line(status: f64, said: &str) -> String {
    if said.is_empty() {
        number_string(status)
    } else {
        format!("{} {said}", number_string(status))
    }
}

/// Under the keys field when the provider answered and refused the key typed there; `provider` is its id.
// init.ts:533
// export function keyRefusedLine(said: string, provider: string): string {
//   return `${CLOUD_SETUP_WORDS.keys.refused(provider)}: ${said}`;
// }
pub fn key_refused_line(said: &str, provider: &str) -> String {
    format!("{}: {said}", cloud_setup_words::keys::refused(provider))
}

/// The same for a key already saved, which is what the build reads before its first stage; `provider` is the id of
/// the provider that refused it.
// init.ts:539
// export function savedKeyRefusedLine(said: string, provider: string): string {
//   return `${CLOUD_SETUP_WORDS.keys.refusedSaved(provider)}: ${said}`;
// }
pub fn saved_key_refused_line(said: &str, provider: &str) -> String {
    format!("{}: {said}", cloud_setup_words::keys::refused_saved(provider))
}

/// Either place when nothing came back about the key at all: what this computer saw instead, with Try again beside it.
// init.ts:544
// export function keyUncheckedLine(said: string, provider: string): string {
//   return `${CLOUD_SETUP_WORDS.keys.unchecked(provider)}: ${said}`;
// }
pub fn key_unchecked_line(said: &str, provider: &str) -> String {
    format!("{}: {said}", cloud_setup_words::keys::unchecked(provider))
}

/// What the machine the build boots costs, said once under the key screen's title from the backend's own rate.
// init.ts:554
// export function initCostLine(size: WorkspaceSize, rateUsdPerHour: number): string {
//   return `A ${fmtSize(size)} workspace costs about $${rateUsdPerHour.toFixed(2)} an hour while it runs and naps when idle`;
// }
pub fn init_cost_line(size: &WorkspaceSize, rate_usd_per_hour: f64) -> String {
    format!("A {} workspace costs about ${} an hour while it runs and naps when idle", fmt_size(size, None), to_fixed(rate_usd_per_hour, 2))
}

/// The cloud setup as wsp setup prints it: which keys are held and the price of the machine the build boots.
// init.ts:239
// export function initSetupLines(setup: InitSetup): string[] {
//   const held = (yes: boolean): string => (yes ? CLOUD_SETUP_WORDS.keys.saved : CLOUD_SETUP_WORDS.keys.unset);
//   const lines = Object.entries(setup.keys).map(([provider, yes]) => `${PROVIDER_KEY_WORDS[provider]?.keyName ?? provider}: ${held(yes)}`);
//   if (setup.pricing !== null) lines.push(initCostLine(setup.pricing.size, setup.pricing.rateUsdPerHour));
//   return lines;
// }
pub fn init_setup_lines(setup: &InitSetup) -> Vec<String> {
    let held = |yes: bool| if yes { cloud_setup_words::keys::SAVED } else { cloud_setup_words::keys::UNSET };
    let mut lines: Vec<String> = setup
        .keys
        .0
        .iter()
        .map(|(provider, yes)| {
            let name =
                crate::place_word::provider_key_words(provider).map_or_else(|| provider.to_string(), |words| words.key_name.to_string());
            format!("{name}: {}", held(*yes))
        })
        .collect();
    if let Some(pricing) = &setup.pricing {
        lines.push(init_cost_line(&pricing.size, pricing.rate_usd_per_hour.0));
    }
    lines
}
