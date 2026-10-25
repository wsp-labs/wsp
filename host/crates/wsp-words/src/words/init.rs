// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/words/init.ts at a3b870a6e: every word of wsp init's job, its screens, its build
//! rows and sign-ins, the counts and lines the sidebar, the toast and the terminal read off one job, and the agent
//! road's brief. Each TS statement stands as a comment above its stub. See spec/init.md.

use crate::js::{js_trim, number_string, to_fixed};
use crate::words::base::lower_first;
use crate::words::computer::{this_computer, Platform};
use crate::words::units::{fmt_bytes, fmt_duration, fmt_size};
use regex::Regex;
use serde::Serialize;
use wsp_wire::types::init_job::{InitJob, InitPhase, InitRoad, InitRow, InitRowKind, LoginChoice, LoginState, SignInFinish};
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

/// The sign-ins a build at a provider is checked against before anything boots: the agents' and GitHub's, held on
/// this computer, which the provider's machines use through wsp. Nothing of them is on a machine, so nothing after
/// the build question waits on the person. One home for the head, each state and the line that stops the build.
// init.ts:113
// export const HOST_SIGN_INS = {
//   head: `Sign-ins on ${THIS_COMPUTER}, which the machines use through wsp:`,
//   login: "signed in",
//   token: "signed in with a token",
//   key: "signed in with an API key",
//   as: (account: string): string => `signed in as ${account}`,
//   missing: (command: string): string => `not signed in: run ${command}`,
//   stopped: `Nothing was booted. Run the line beside each sign-in that is not ready, on ${THIS_COMPUTER}, then run wsp init again; the recipe is kept.`,
// } as const;
pub mod host_sign_ins {
    pub const HEAD: &str = "Sign-ins on this computer, which the machines use through wsp:";
    pub const LOGIN: &str = "signed in";
    pub const TOKEN: &str = "signed in with a token";
    pub const KEY: &str = "signed in with an API key";
    /// TS: (account ) => `signed in as ${account}`
    pub fn r#as(account: &str) -> String {
        format!("signed in as {account}")
    }
    /// TS: (command ) => `not signed in: run ${command}`
    pub fn missing(command: &str) -> String {
        format!("not signed in: run {command}")
    }
    pub const STOPPED: &str = "Nothing was booted. Run the line beside each sign-in that is not ready, on this computer, then run wsp init again; the recipe is kept.";
}

/// The command a person runs on this computer to sign each in, by the row the lines name.
// init.ts:124
// export const HOST_SIGN_IN_COMMANDS = { claude: "claude auth login", codex: "codex login", github: "gh auth login" } as const;
pub mod host_sign_in_commands {
    pub const CLAUDE: &str = "claude auth login";
    pub const CODEX: &str = "codex login";
    pub const GITHUB: &str = "gh auth login";
}

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

/// One answer with its words, keys value then label.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SignInChoice {
    pub value: LoginChoice,
    pub label: String,
}

/// The answers a row can be walked through, in order, with the words each shows.
// init.ts:147
// export const signInChoices = (platform: "darwin" | "linux"): readonly { value: LoginChoice; label: string }[] => LOGIN_CHOICES.map(value => signInChoice(value, platform));
/// In LOGIN_CHOICES' order: copy, machine, later, key, skip, token.
pub fn sign_in_choices(platform: Platform) -> Vec<SignInChoice> {
    [LoginChoice::Copy, LoginChoice::Machine, LoginChoice::Later, LoginChoice::Key, LoginChoice::Skip, LoginChoice::Token]
        .into_iter()
        .map(|choice| sign_in_choice(choice, platform))
        .collect()
}

/// One answer by its own name, so nothing depends on where it sits in the list.
// init.ts:150
// export const signInChoice = (value: LoginChoice, platform: "darwin" | "linux"): { value: LoginChoice; label: string } => ({ value, label: SIGN_IN_ANSWERS[value].label(platform) });
pub fn sign_in_choice(value: LoginChoice, platform: Platform) -> SignInChoice {
    SignInChoice { value, label: sign_in_answer_label(value, platform) }
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

/// The word for a job that waits on the person rather than the machine: the answers, or a sign-in's page.
// init.ts:163
const WAITING_FOR_YOU: &str = "waiting for you";

/// The job's phase as the muted mono word a row or a footer prints.
// init.ts:166
// export function initPhaseWord(phase: InitPhase): string {
//   switch (phase) {
//     case "agent":
//       return "agent writing the recipe";
//     case "reading":
//       return "reading this computer";
//     case "answering":
//       return WAITING_FOR_YOU;
//     case "building":
//       return "building";
//     case "signing-in":
//       return "signing in";
//     case "sealing":
//       return "sealing";
//     case "finishing":
//       return "finishing";
//     case "done":
//       return "done";
//     case "failed":
//       return "failed";
//     case "cancelled":
//       return "cancelled";
//     default: {
//       const _exhaustive: never = phase;
//       return _exhaustive;
//     }
//   }
// }
pub fn init_phase_word(phase: InitPhase) -> String {
    match phase {
        InitPhase::Agent => "agent writing the recipe".to_string(),
        InitPhase::Reading => "reading this computer".to_string(),
        InitPhase::Answering => WAITING_FOR_YOU.to_string(),
        InitPhase::Building => "building".to_string(),
        InitPhase::SigningIn => "signing in".to_string(),
        InitPhase::Sealing => "sealing".to_string(),
        InitPhase::Finishing => "finishing".to_string(),
        InitPhase::Done => "done".to_string(),
        InitPhase::Failed => "failed".to_string(),
        InitPhase::Cancelled => "cancelled".to_string(),
    }
}

/// The words a build row's state is written in, one table for the host that sets them and the client that reads them;
/// a sign-in row's other words are LOGIN_STATE_WORDS.
// init.ts:197
// export const INIT_ROW_STATES = {
//   waiting: "waiting",
//   running: "running",
//   done: "done",
//   failed: "failed",
//   forking: "forking",
//   forked: "forked",
//   importing: "importing",
//   imported: "imported",
//   /** A sign-in whose page waits for the person. */
//   open: "waiting for you",
//   /** A stage running only in the sense that the provider has no room yet: the account is at its machine cap. */
//   slot: "waiting for a machine slot",
//   /** A first workspace or its project the build ended before reaching. */
//   notMade: "not made",
//   /** A machine a stop could not reach the provider to kill, being killed again until it is. */
//   retrying: "machine still running, retrying",
//   /** That machine once the provider took the kill: nothing is billing. */
//   gone: "gone",
//   /** The stage a person's stop ended: over, but nothing failed, so the row never wears the failure's cross. */
//   stopped: "stopped",
//   /** A sign-in answered with an API key the home held, so the machine has it and nothing is asked. */
//   keySet: "key set",
//   /** A step the build left out: a sign-in it never reached, or a first workspace it carried no name for. */
//   skipped: "skipped",
//   /** An agent on this computer whose config carries the wsp tools. */
//   mcpAdded: "MCP added",
// } as const;
pub mod init_row_states {
    pub const WAITING: &str = "waiting";
    pub const RUNNING: &str = "running";
    pub const DONE: &str = "done";
    pub const FAILED: &str = "failed";
    pub const FORKING: &str = "forking";
    pub const FORKED: &str = "forked";
    pub const IMPORTING: &str = "importing";
    pub const IMPORTED: &str = "imported";
    pub const OPEN: &str = "waiting for you";
    pub const SLOT: &str = "waiting for a machine slot";
    pub const NOT_MADE: &str = "not made";
    pub const RETRYING: &str = "machine still running, retrying";
    pub const GONE: &str = "gone";
    pub const STOPPED: &str = "stopped";
    pub const KEY_SET: &str = "key set";
    pub const SKIPPED: &str = "skipped";
    pub const MCP_ADDED: &str = "MCP added";
}

/// A sign-in row's word once its outcome is in, the app's and wsp setup's spelling, drawn for the computer the run
/// reads; the terminal's own table of the same outcomes keeps LOGIN_STATE_WORDS. The word is what a row prints and
/// never what a client reads: the outcome travels beside it as the row's login.
// init.ts:229
// export const INIT_SIGN_IN_WORDS: Record<LoginState, (platform: "darwin" | "linux") => string> = {
//   "signed-in": () => INIT_ROW_STATES.done,
//   "not-signed-in": () => "not signed in",
//   copied: platform => `copied from ${thisComputer(platform)}`,
//   "not-verified": () => "not verified",
//   skipped: () => INIT_ROW_STATES.skipped,
//   deferred: () => SIGN_IN_DEFERRED_WORD,
// };
/// `INIT_SIGN_IN_WORDS[login](platform)`.
pub fn init_sign_in_word(login: LoginState, platform: Platform) -> String {
    match login {
        LoginState::SignedIn => init_row_states::DONE.to_string(),
        LoginState::NotSignedIn => "not signed in".to_string(),
        LoginState::Copied => format!("copied from {}", this_computer(platform)),
        LoginState::NotVerified => "not verified".to_string(),
        LoginState::Skipped => init_row_states::SKIPPED.to_string(),
        LoginState::Deferred => SIGN_IN_DEFERRED_WORD.to_string(),
    }
}

/// A sign-in row's outcome as it travels: the name every client reads and, beside it, the word drawn for the
/// computer that ran the sign-in. The two are set together, so no row can carry one computer's word under another's
/// outcome.
// init.ts:241
// export const initSignInOutcome = (login: LoginState, platform: "darwin" | "linux"): Pick<InitRow, "state" | "login"> => ({ state: INIT_SIGN_IN_WORDS[login](platform), login });
/// A row's state word and outcome, set together.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct RowState {
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<LoginState>,
}

pub fn init_sign_in_outcome(login: LoginState, platform: Platform) -> RowState {
    RowState { state: init_sign_in_word(login, platform), login: Some(login) }
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

/// The stages the provider gives no progress for: the snapshot and the save go to their end with nothing to say in
/// between, so the row counts its own seconds while one runs.
// init.ts:261
// export const GOLDEN_STAGE_TIMED: ReadonlySet<GoldenStage> = new Set<GoldenStage>(["snapshotting", "promoting"]);
pub const GOLDEN_STAGE_TIMED: [GoldenStage; 2] = [GoldenStage::Snapshotting, GoldenStage::Promoting];

/// A stage's wire word, the one the row id and the stage's frame both carry.
fn stage_wire(stage: GoldenStage) -> &'static str {
    match stage {
        GoldenStage::Creating => "creating",
        GoldenStage::DeployingDaemon => "deploying-daemon",
        GoldenStage::ApplyingSetup => "applying-setup",
        GoldenStage::UploadingFiles => "uploading-files",
        GoldenStage::InstallingHarness => "installing-harness",
        GoldenStage::InstallingTools => "installing-tools",
        GoldenStage::InstallingMcp => "installing-mcp",
        GoldenStage::Ready => "ready",
        GoldenStage::Snapshotting => "snapshotting",
        GoldenStage::Promoting => "promoting",
        GoldenStage::SmokeForking => "smoke-forking",
        GoldenStage::Sealed => "sealed",
        GoldenStage::Failed => "failed",
    }
}

/// A stage's row id, the one spelling the host writes and a client reads.
// init.ts:264
// export const initStageRowId = (stage: GoldenStage): string => `stage/${stage}`;
pub fn init_stage_row_id(stage: GoldenStage) -> String {
    format!("stage/{}", stage_wire(stage))
}

/// Whether a row is one of the stages that count their own seconds.
// init.ts:267
// export const initRowTimed = (row: Pick<InitRow, "id">): boolean => [...GOLDEN_STAGE_TIMED].some(stage => row.id === initStageRowId(stage));
pub fn init_row_timed(row: &InitRow) -> bool {
    GOLDEN_STAGE_TIMED.iter().any(|stage| row.id == init_stage_row_id(*stage))
}

/// A row's clock while it runs: whole seconds under a minute, then minutes and seconds.
// init.ts:270
// export const initElapsedLine = (ms: number): string => (ms < 60_000 ? `${Math.max(0, Math.floor(ms / 1_000))}s` : fmtDuration(ms));
pub fn init_elapsed_line(ms: f64) -> String {
    if ms < 60_000.0 {
        format!("{}s", number_string((ms / 1_000.0).floor().max(0.0)))
    } else {
        fmt_duration(ms, None)
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

/// The one sentence on a sign-in the person is waited on for, beside the way to act: what the machine waits on, never
/// the command it ran, and short enough to share the action line with the code and the keycap uncut. A page that shows
/// a code or hands one back has the machine waiting for that code; any other page is open on this computer. A row
/// nobody is waited on for has no sentence.
// init.ts:299
// export function initSignInLine(row: Pick<InitRow, "state" | "code" | "finish">): string | undefined {
//   if (row.state !== INIT_ROW_STATES.open) return undefined;
//   return row.code !== undefined || row.finish === "code" ? "waiting for the code" : "the page is open on this computer";
// }
pub fn init_sign_in_line(row: &InitRow) -> Option<String> {
    if row.state != SIGN_IN_OPEN_STATE {
        return None;
    }
    if row.code.is_some() || matches!(row.finish, Some(SignInFinish::Code)) {
        Some("waiting for the code".to_string())
    } else {
        Some("the page is open on this computer".to_string())
    }
}

/// The id of the wsp tools screen's row for an agent, the one the app answers for it from the first launch's own answer.
// init.ts:305
// export const wspToolsRowId = (agent: string): string => `wsp-tools/${agent}`;
pub fn wsp_tools_row_id(agent: &str) -> String {
    format!("wsp-tools/{agent}")
}

/// The name a first workspace takes when nobody names one: what the terminal falls back to and what the app's name
/// field opens on, so the two roads cannot drift apart.
// init.ts:309
pub const FIRST_WORKSPACE: &str = "first";

/// What a sign-in row says when the run ended without reaching it.
// init.ts:312
pub const SIGN_IN_NEVER_REACHED: &str = "the build never reached this sign-in";

/// What any other row says when the build ended with it unfinished, whether or not it had started.
// init.ts:314
pub const NEVER_REACHED: &str = "the build ended before this step";

/// The state word of a sign-in row while its page waits for the person.
// init.ts:317
// export const SIGN_IN_OPEN_STATE = INIT_ROW_STATES.open;
pub const SIGN_IN_OPEN_STATE: &str = init_row_states::OPEN;

/// The state word of an agent on this computer whose config carries the wsp tools.
// init.ts:320
// export const MCP_ADDED_WORD = INIT_ROW_STATES.mcpAdded;
pub const MCP_ADDED_WORD: &str = init_row_states::MCP_ADDED;

// init.ts:322
// private in the TS: port it as a private helper
// const ROW_OVER: ReadonlySet<string> = new Set([INIT_ROW_STATES.done, INIT_ROW_STATES.failed, INIT_ROW_STATES.stopped, INIT_ROW_STATES.forked, INIT_ROW_STATES.imported, INIT_ROW_STATES.keySet, INIT_ROW_STATES.mcpAdded, INIT_ROW_STATES.skipped, INIT_ROW_STATES.notMade, INIT_ROW_STATES.gone]);

// init.ts:324
// private in the TS: port it as a private helper
// const ROW_UNRUN: ReadonlySet<string> = new Set([INIT_ROW_STATES.skipped, INIT_ROW_STATES.notMade, INIT_ROW_STATES.stopped]);

/// Whether a row's state is one it ended on without anything having run: the build never reached it, or a person's
/// stop ended it. The count leaves these out of its done, and a glyph gives them the ring they waited with rather
/// than a check, which would read as work that happened.
// init.ts:329
// export const initRowUnrun = (state: string): boolean => ROW_UNRUN.has(state);
pub fn init_row_unrun(state: &str) -> bool {
    [init_row_states::SKIPPED, init_row_states::NOT_MADE, init_row_states::STOPPED].contains(&state)
}

/// Whether a row is a failure to show as one: the stage that failed, and the sign-in or the stage it folds into whose
/// sign-in ran out, which must never wear a tick. The one answer to "this sign-in ran out", so the cross, the
/// attention mark and the Retry keycap all read it and none of them spells it again.
// init.ts:334
// export const initRowFailed = (row: Pick<InitRow, "state" | "login">): boolean => row.state === INIT_ROW_STATES.failed || row.login === "not-signed-in";
pub fn init_row_failed(row: &InitRow) -> bool {
    row.state == init_row_states::FAILED || row.login == Some(LoginState::NotSignedIn)
}

/// Whether a row has ended: what the progress count and a section's count read. A sign-in answers from its outcome,
/// so a row a Linux host worded for itself ends the same as a Mac's; every other row from the word it prints.
// init.ts:338
// export const initRowOver = (row: Pick<InitRow, "state" | "login">): boolean => row.login !== undefined || ROW_OVER.has(row.state);
pub fn init_row_over(row: &InitRow) -> bool {
    row.login.is_some()
        || [
            init_row_states::DONE,
            init_row_states::FAILED,
            init_row_states::STOPPED,
            init_row_states::FORKED,
            init_row_states::IMPORTED,
            init_row_states::KEY_SET,
            init_row_states::MCP_ADDED,
            init_row_states::SKIPPED,
            init_row_states::NOT_MADE,
            init_row_states::GONE,
        ]
        .contains(&row.state.as_str())
}

/// Where a stopped build was, from the stage that was running: one spelling for the terminal's stop line and the
/// app's sentence, since the stage names read as sentence openings ("Creating the machine").
// init.ts:342
// export const initStageWhile = (stage: string): string => `while ${lowerFirst(stage)}`;
pub fn init_stage_while(stage: &str) -> String {
    format!("while {}", lower_first(stage))
}

/// The opening of every stop line, terminal and app alike: where the run was when it stopped. What follows it is
/// what became of the machine, which differs by who is reading.
// init.ts:346
// export const initStoppedAt = (where: string): string => `Stopped ${where}.`;
pub fn init_stopped_at(r#where: &str) -> String {
    format!("Stopped {}.", r#where)
}

/// What the app says after that opening when the provider would not take the kill: the terminal tells the person
/// how to finish it themselves, the app does not, because the host is already trying again and its row says so.
// init.ts:350
pub const STOP_LEFT_MACHINE_LINE: &str = "The machine did not stop yet; wsp keeps trying.";

/// A machine row's name: the builder, never its provider id, which is the host's to hold and no sentence's to say.
// init.ts:353
pub const MACHINE_ROW_LABEL: &str = "The builder";

/// What a stopped build says of its machine once the provider took the kill: the same words in the terminal and the app.
// init.ts:356
pub const MACHINE_GONE_LINE: &str = "The machine is gone; nothing is billing.";

/// A stage's line for a machine its rollback could not remove, with the provider's own refusal: it goes in the stage's
/// block, never in the headline, which stays the failure's own sentence; the machine's row carries the retries.
// init.ts:360
// export const machineLeftLine = (reason: string): string => `the machine could not be removed and bills on: ${reason}`;
pub fn machine_left_line(reason: &str) -> String {
    format!("the machine could not be removed and bills on: {reason}")
}

/// What the sidebar's keycap says while a machine an earlier build left is still being removed: the one line that
/// keeps a machine from billing unseen once the setup has moved on to another job.
// init.ts:364
pub const MACHINE_SWEEP_LINE: &str = "a machine from the last build is still being removed";

/// What a build says stopped it when nothing on this computer could reach anything.
// init.ts:367
pub const NETWORK_LOST_LINE: &str = "This computer lost its network";

// The failures a provider client raises when the network is gone rather than when the provider refused.
// init.ts:370
// private in the TS: port it as a private helper
// const OFFLINE_FAILURE = /^fetch failed$|\b(ENOTFOUND|EAI_AGAIN|ECONNREFUSED|ENETDOWN|ENETUNREACH|EHOSTUNREACH)\b/;

/// What to do about a file the build would put on the machine and this computer has not got, which is read before
/// the confirm so nothing is booted and nothing bills for a file the deploy was always going to ask for. The file
/// itself is no part of this: a terminal says it once above this line, the way every other refusal there names the
/// fault and then the fix. A shipped install carries every one, so the fix names the install first and the
/// checkout's own build after it.
// init.ts:377
pub const BUILD_NEEDS_FILE_FIX: &str = "Nothing was booted. An installed wsp carries it; a checkout builds the command and places the daemon binaries with packages/wspx/scripts/daemon-binary.mjs.";

/// The same refusal with the file in front of it, for the one line a client has to draw a stage that failed.
// init.ts:380
// export const buildNeedsFileLine = (missing: string): string => `${missing}. ${BUILD_NEEDS_FILE_FIX}`;
pub fn build_needs_file_line(missing: &str) -> String {
    format!("{missing}. {BUILD_NEEDS_FILE_FIX}")
}

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

/// The sentence a build that stopped on its own ends on: what stopped it, then what became of the machine it had
/// booted, in the same words a stop the person asked for uses. A build that never booted one says nothing of a
/// machine: the stage it refused at already says nothing was booted.
// init.ts:394
// export function initFailedLine(error: string, machine?: "gone" | "left"): string {
//   const said = initStoppedLine(error);
//   if (machine === undefined) return said;
//   return `${said} ${machine === "gone" ? MACHINE_GONE_LINE : STOP_LEFT_MACHINE_LINE}`;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MachineEnd {
    Gone,
    Left,
}

pub fn init_failed_line(error: &str, machine: Option<MachineEnd>) -> String {
    let said = init_stopped_line(error);
    match machine {
        None => said,
        Some(MachineEnd::Gone) => format!("{said} {MACHINE_GONE_LINE}"),
        Some(MachineEnd::Left) => format!("{said} {STOP_LEFT_MACHINE_LINE}"),
    }
}

/// Whether the job's phase is one it ends on.
// init.ts:401
// export const initJobOver = (phase: InitPhase): boolean => phase === "done" || phase === "failed" || phase === "cancelled";
pub fn init_job_over(phase: InitPhase) -> bool {
    matches!(phase, InitPhase::Done | InitPhase::Failed | InitPhase::Cancelled)
}

/// Whether the job is on the build: from the machine booting to the first workspace, the stretch the count is over.
// init.ts:404
// export const initJobBuilding = (phase: InitPhase): boolean => phase === "building" || phase === "signing-in" || phase === "sealing" || phase === "finishing";
pub fn init_job_building(phase: InitPhase) -> bool {
    matches!(phase, InitPhase::Building | InitPhase::SigningIn | InitPhase::Sealing | InitPhase::Finishing)
}

/// Whether the job's step is the agent's own: the agent road while its thread writes the recipe, and a job that
/// ended there, which is the step that carries Retry. A job that ended with screens ended past this step, on the
/// build. One rule, so the app's sheet and the terminal pick the same step for one job.
// init.ts:409
// export const initAgentStep = (job: Pick<InitJob, "road" | "phase" | "screens">): boolean =>
//   job.road === "agent" && (job.phase === "agent" || (initJobOver(job.phase) && job.screens.length === 0));
pub fn init_agent_step(job: &InitJob) -> bool {
    matches!(job.road, InitRoad::Agent) && (matches!(job.phase, InitPhase::Agent) || (init_job_over(job.phase) && job.screens.is_empty()))
}

// The sentence a sign-in whose page is open makes: one spelling for the sidebar's line, the toast and a system
// notification.
// init.ts:414
// private in the TS: port it as a private helper
// const signInTo = (label: string): string => `sign in to ${label}`;

/// What a row of the vault step waits on: a value the person holds on their own computer, pasted into the client
/// rather than typed on a machine. `mint`, where the tool has one, is the command that prints it there.
// init.ts:418
// export const pasteHereLine = (word: string, mint?: string): string =>
//   mint === undefined ? `paste the ${word}; it stays on this computer` : `run ${mint} on this computer and paste the ${word} it prints; it stays there`;
pub fn paste_here_line(word: &str, mint: Option<&str>) -> String {
    match mint {
        None => format!("paste the {word}; it stays on this computer"),
        Some(mint) => format!("run {mint} on this computer and paste the {word} it prints; it stays there"),
    }
}

// The sign-in row whose page waits for the person, if one does.
// init.ts:422
// private in the TS: port it as a private helper
// const openSignIn = (rows: InitJob["rows"]): InitJob["rows"][number] | undefined => rows.find(r => r.kind === "sign-in" && r.state === SIGN_IN_OPEN_STATE);

/// The sign-in row whose page waits for the person, if one does.
fn open_sign_in(rows: &[InitRow]) -> Option<&InitRow> {
    rows.iter().find(|row| row.kind == InitRowKind::SignIn && row.state == SIGN_IN_OPEN_STATE)
}

/// What the job waits on the person for, or nothing: a sign-in whose page is open on the machine and nothing else
/// today. Only a wait the person is not already looking at counts, so the screens they just opened are not one; the
/// phase word says where those stand. The host writes the job's needsYou from this and every surface reads that
/// field, so nothing derives the wait twice.
// init.ts:428
// export function initNeedWhat(job: Pick<InitJob, "rows">): string | undefined {
//   const label = initSignInWaitedOn(job);
//   return label === undefined ? undefined : signInTo(label);
// }
pub fn init_need_what(job: &InitJob) -> Option<String> {
    init_sign_in_waited_on(job).map(|label| format!("sign in to {label}"))
}

/// The sign-in the job waits on the person for, by its row's label, or nothing.
// init.ts:434
// export const initSignInWaitedOn = (job: Pick<InitJob, "rows">): string | undefined => openSignIn(job.rows)?.label;
pub fn init_sign_in_waited_on(job: &InitJob) -> Option<String> {
    open_sign_in(&job.rows).map(|row| row.label.clone())
}

/// What a card that started the build says while that sign-in waits.
// init.ts:437
// export const signInWaitLine = (label: string): string => `Waiting for your sign-in to ${label}`;
pub fn sign_in_wait_line(label: &str) -> String {
    format!("Waiting for your sign-in to {label}")
}

/// What the app says when it needs the person: the toast's opening and a system notification's title.
// init.ts:440
pub const NEEDS_YOU: &str = "wsp needs you";

/// The one line the toast and a system notification say for a need.
// init.ts:443
// export const initNeedsYouLine = (what: string): string => `${NEEDS_YOU}: ${what}`;
pub fn init_needs_you_line(what: &str) -> String {
    format!("{NEEDS_YOU}: {what}")
}

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

/// Whether a machine an earlier build left is still being removed: the row the host's sweep rides on whatever job
/// is current.
// init.ts:457
// export const initSweeping = (rows: readonly InitRow[]): boolean => rows.some(r => r.kind === "machine" && r.state === INIT_ROW_STATES.retrying);
pub fn init_sweeping(rows: &[InitRow]) -> bool {
    rows.iter().any(|row| row.kind == InitRowKind::Machine && row.state == init_row_states::RETRYING)
}

/// The one line the collapsed sidebar row shows for a running job: the sign-in waited on while one is open, then a
/// machine still being removed, since that one bills while nobody looks, then the phase with the count of stages
/// done while it builds, the phase word alone otherwise.
// init.ts:462
// export function initProgressLine(job: Pick<InitJob, "phase" | "rows" | "progress">): string {
//   const open = openSignIn(job.rows);
//   if (open !== undefined) return signInTo(open.label);
//   if (initSweeping(job.rows)) return MACHINE_SWEEP_LINE;
//   const word = initPhaseWord(job.phase);
//   return initJobBuilding(job.phase) && job.progress.total > 0 ? `${word} ${job.progress.done}/${job.progress.total}` : word;
// }
pub fn init_progress_line(job: &InitJob) -> String {
    if let Some(open) = open_sign_in(&job.rows) {
        return format!("sign in to {}", open.label);
    }
    if init_sweeping(&job.rows) {
        return MACHINE_SWEEP_LINE.to_string();
    }
    let word = init_phase_word(job.phase);
    if init_job_building(job.phase) && job.progress.total.as_f64() > 0.0 {
        format!("{word} {}/{}", number_string(job.progress.done.as_f64()), number_string(job.progress.total.as_f64()))
    } else {
        word
    }
}

/// The id of the stage row the build's sign-ins fold into.
// init.ts:471
pub const SIGN_IN_STAGE_ID: &str = "stage/sign-ins";

// What the stage the sign-ins fold into stands at, from the sign-ins themselves. A run-out hands the stage its own
// outcome and its own word, so the stage says what the row under it says on whichever computer wrote it.
// init.ts:475
// private in the TS: port it as a private helper
// function signInStageState(signIns: readonly InitRow[]): Pick<InitRow, "state" | "login"> {
//   if (signIns.some(r => r.state === SIGN_IN_OPEN_STATE)) return { state: SIGN_IN_OPEN_STATE };
//   if (signIns.some(r => r.state === INIT_ROW_STATES.running)) return { state: INIT_ROW_STATES.running };
//   if (!signIns.every(r => initRowOver(r))) return { state: signIns.every(r => r.state === INIT_ROW_STATES.waiting) ? INIT_ROW_STATES.waiting : INIT_ROW_STATES.running };
//   const ranOut = signIns.find(r => initRowFailed(r));
//   if (ranOut !== undefined) return { state: ranOut.state, login: ranOut.login };
//   // A build that ended before it reached any of them signed none in, so the fold says so rather than done.
//   if (signIns.every(r => r.state === INIT_ROW_STATES.skipped)) return { state: INIT_ROW_STATES.skipped };
//   return { state: INIT_ROW_STATES.done };
// }

/// The state a folded sign-in stage stands at, and the outcome a run-out hands it.
fn sign_in_stage_state(sign_ins: &[InitRow]) -> (String, Option<LoginState>) {
    if sign_ins.iter().any(|row| row.state == SIGN_IN_OPEN_STATE) {
        return (SIGN_IN_OPEN_STATE.to_string(), None);
    }
    if sign_ins.iter().any(|row| row.state == init_row_states::RUNNING) {
        return (init_row_states::RUNNING.to_string(), None);
    }
    if !sign_ins.iter().all(init_row_over) {
        let waiting = sign_ins.iter().all(|row| row.state == init_row_states::WAITING);
        let state = if waiting { init_row_states::WAITING } else { init_row_states::RUNNING };
        return (state.to_string(), None);
    }
    if let Some(ran_out) = sign_ins.iter().find(|row| init_row_failed(row)) {
        return (ran_out.state.clone(), ran_out.login);
    }
    if sign_ins.iter().all(|row| row.state == init_row_states::SKIPPED) {
        return (init_row_states::SKIPPED.to_string(), None);
    }
    (init_row_states::DONE.to_string(), None)
}

/// The build's list as the app draws it: stages only, in the job's order, the sign-ins folded into one stage row where
/// the first of them sits, whose state is the sign-ins' own (waiting for you while a page waits on the person, running
/// while one runs, not signed in once every one is over and one ran out, done once every one is over well, waiting
/// before any starts); the agent rows are not build stages and leave. The sign-ins come back beside it, for the stage's sub-rows.
// init.ts:490
// export function initBuildRows(rows: readonly InitRow[]): { rows: InitRow[]; signIns: InitRow[] } {
//   const signIns = rows.filter(r => r.kind === "sign-in");
//   const out: InitRow[] = [];
//   let folded = false;
//   for (const row of rows) {
//     if (row.kind === "agent") continue;
//     if (row.kind !== "sign-in") {
//       out.push(row);
//       continue;
//     }
//     if (folded) continue;
//     folded = true;
//     out.push({ id: SIGN_IN_STAGE_ID, kind: "stage", label: CLOUD_SETUP_WORDS.build.signingIn, ...signInStageState(signIns) });
//   }
//   return { rows: out, signIns };
// }
/// The build's stage rows with the sign-ins folded, and the sign-ins beside them.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildRows {
    pub rows: Vec<InitRow>,
    pub sign_ins: Vec<InitRow>,
}

pub fn init_build_rows(rows: &[InitRow]) -> BuildRows {
    let sign_ins: Vec<InitRow> = rows.iter().filter(|row| row.kind == InitRowKind::SignIn).cloned().collect();
    let mut out = Vec::new();
    let mut folded = false;
    for row in rows {
        if row.kind == InitRowKind::Agent {
            continue;
        }
        if row.kind != InitRowKind::SignIn {
            out.push(row.clone());
            continue;
        }
        if folded {
            continue;
        }
        folded = true;
        let (state, login) = sign_in_stage_state(&sign_ins);
        out.push(InitRow {
            id: SIGN_IN_STAGE_ID.to_string(),
            kind: InitRowKind::Stage,
            tool: None,
            label: cloud_setup_words::build::SIGNING_IN.to_string(),
            state,
            login,
            detail: None,
            page: None,
            code: None,
            finish: None,
            ms: None,
            since: None,
            lines: None,
        });
    }
    BuildRows { rows: out, sign_ins }
}

/// How many of the build's stages ended well, of all of them: the one count a build has, read by the line along the
/// card's top edge, the count beside the title and the host's own progress field, so the sidebar and the sheet can
/// never say two things. Stages alone, so the bar measures the build and not the agents given the tools, the first
/// workspace or its project; it takes the rows initBuildRows hands over, with the sign-ins folded into their stage.
/// A stage the glyph draws a cross on, that a stop ended, or that the build never reached is over without having
/// ended well, so a build that stopped never reads complete however early it stopped, and the folded sign-in stage
/// whose row ran out is counted the way its own cross reads.
// init.ts:514
// export function initStageCount(rows: readonly InitRow[]): { done: number; total: number } {
//   const stages = rows.filter(r => r.kind === "stage");
//   return { done: stages.filter(r => initRowOver(r) && !initRowFailed(r) && !initRowUnrun(r.state)).length, total: stages.length };
// }
/// A count of stages done of all of them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct StageCount {
    pub done: usize,
    pub total: usize,
}

pub fn init_stage_count(rows: &[InitRow]) -> StageCount {
    let stages = || rows.iter().filter(|row| row.kind == InitRowKind::Stage);
    StageCount {
        done: stages().filter(|row| init_row_over(row) && !init_row_failed(row) && !init_row_unrun(&row.state)).count(),
        total: stages().count(),
    }
}

/// Where a step sits in the steps its run shows, over the title ("2/4"): the steps the person sees, the build or the
/// first workspace counted as the last, a step with nothing to pick not counted since it is not shown.
// init.ts:521
// export const initStepCounter = (at: number, total: number): string => `${at}/${total}`;
pub fn init_step_counter(at: f64, total: f64) -> String {
    format!("{}/{}", number_string(at), number_string(total))
}

/// The count as words: `3 of 12`.
// init.ts:524
// export const initStageCountLine = (count: { done: number; total: number }): string => `${count.done} of ${count.total}`;
pub fn init_stage_count_line(count: &StageCount) -> String {
    format!("{} of {}", count.done, count.total)
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

/// How a terminal run closes when the check stopped it before the first stage. The refusal itself is said above this
/// line, so this one carries the way on alone; wsp init asks for a key it can use, so running it again is that road.
/// `provider` is the id of the provider that refused it.
// init.ts:551
// export const savedKeyStoppedLine = (provider: string): string => `Save a key ${providerKeyName(provider)} takes and run wsp init again; nothing booted, and the recipe is kept.`;
pub fn saved_key_stopped_line(provider: &str) -> String {
    format!(
        "Save a key {} takes and run wsp init again; nothing booted, and the recipe is kept.",
        crate::place_word::provider_key_name(provider)
    )
}

/// What the machine the build boots costs, said once under the key screen's title from the backend's own rate.
// init.ts:554
// export function initCostLine(size: WorkspaceSize, rateUsdPerHour: number): string {
//   return `A ${fmtSize(size)} workspace costs about $${rateUsdPerHour.toFixed(2)} an hour while it runs and naps when idle`;
// }
pub fn init_cost_line(size: &WorkspaceSize, rate_usd_per_hour: f64) -> String {
    format!("A {} workspace costs about ${} an hour while it runs and naps when idle", fmt_size(size, None), to_fixed(rate_usd_per_hour, 2))
}

/// The cloud setup as wsp setup prints it: which keys are held, the agents here with their tools, the price, and the
/// job's phase with each of its rows when one runs or ran.
// init.ts:560
// export function initSetupLines(setup: InitSetup): string[] {
//   const held = (yes: boolean): string => (yes ? CLOUD_SETUP_WORDS.keys.saved : CLOUD_SETUP_WORDS.keys.unset);
//   const agents = setup.agents.length === 0 ? "none found" : setup.agents.map(a => (a.configured ? `${a.name} (${MCP_ADDED_WORD})` : a.name)).join(", ");
//   const keys = Object.entries(setup.keys).map(([provider, yes]) => `${PROVIDER_KEY_WORDS[provider]?.keyName ?? provider}: ${held(yes)}`);
//   const lines = [...keys, `Agents here: ${agents}`];
//   if (setup.pricing !== null) lines.push(initCostLine(setup.pricing.size, setup.pricing.rateUsdPerHour));
//   const job = setup.job;
//   if (job === null) {
//     lines.push("No setup is running; the app's Image section starts one.");
//     return lines;
//   }
//   lines.push(`Setup on the ${job.road} road: ${initProgressLine(job)}${job.error !== undefined ? ` (${job.error})` : ""}`);
//   for (const r of job.rows) lines.push(`  ${r.label}: ${r.state}${r.page !== undefined ? ` ${r.page}` : ""}${r.code !== undefined ? ` code ${r.code}` : ""}${r.detail !== undefined ? ` (${r.detail})` : ""}`);
//   if (job.workspace !== undefined) lines.push(`Workspace ${job.workspace.name} (${job.workspace.id}) is up.`);
//   return lines;
// }
pub fn init_setup_lines(setup: &InitSetup) -> Vec<String> {
    let held = |yes: bool| if yes { cloud_setup_words::keys::SAVED } else { cloud_setup_words::keys::UNSET };
    let agents = if setup.agents.is_empty() {
        "none found".to_string()
    } else {
        setup
            .agents
            .iter()
            .map(|agent| if agent.configured { format!("{} ({MCP_ADDED_WORD})", agent.name) } else { agent.name.clone() })
            .collect::<Vec<_>>()
            .join(", ")
    };
    let mut lines: Vec<String> = setup
        .keys
        .iter()
        .map(|(provider, yes)| {
            let name =
                crate::place_word::provider_key_words(provider).map_or_else(|| provider.to_string(), |words| words.key_name.to_string());
            format!("{name}: {}", held(*yes))
        })
        .collect();
    lines.push(format!("Agents here: {agents}"));
    if let Some(pricing) = &setup.pricing {
        lines.push(init_cost_line(&pricing.size, pricing.rate_usd_per_hour.0));
    }
    let Some(job) = &setup.job else {
        lines.push("No setup is running; the app's Image section starts one.".to_string());
        return lines;
    };
    let road = match job.road {
        InitRoad::Manual => "manual",
        InitRoad::Agent => "agent",
        InitRoad::Terminal => "terminal",
    };
    let error = job.error.as_ref().map_or(String::new(), |error| format!(" ({error})"));
    lines.push(format!("Setup on the {road} road: {}{error}", init_progress_line(job)));
    for row in &job.rows {
        let page = row.page.as_ref().map_or(String::new(), |page| format!(" {page}"));
        let code = row.code.as_ref().map_or(String::new(), |code| format!(" code {code}"));
        let detail = row.detail.as_ref().map_or(String::new(), |detail| format!(" ({detail})"));
        lines.push(format!("  {}: {}{page}{code}{detail}", row.label, row.state));
    }
    if let Some(workspace) = &job.workspace {
        lines.push(format!("Workspace {} ({}) is up.", workspace.name, workspace.id));
    }
    lines
}

/// The first message of the thread the agent road opens on this computer: read this computer with recipe_scan, write
/// the recipe with recipe to the path the job reads it from, and ask the person nothing, since they review every row
/// on the screens that follow. The two tools are named as the only road on purpose: a thread that reached for the
/// command line instead spent its turn on one permission prompt per `sed` over its own tool results (seen 2026-09-10),
/// and the launch carries the wsp server so both tools are there to call.
// init.ts:582
// export function initAgentPrompt(recipePath: string): string {
//   return [
//     "Write the recipe for this person's wsp machine image from what their agents actually used on this computer.",
//     "Use the wsp tools recipe_scan and recipe, and nothing else: run no commands and read no files.",
//     "Call recipe_scan first and read every row's recommended value and its reason.",
//     `Then call recipe once, with tick set to used, set for every row whose reason says it is worth changing, signin for every sign-in row at its recommended choice, and out set to ${recipePath}.`,
//     "Ask them nothing: they review every row in the app once the file is written.",
//     "Reply with one line saying the recipe is written.",
//   ].join(" ");
// }
pub fn init_agent_prompt(recipe_path: &str) -> String {
    let then = format!(
        "Then call recipe once, with tick set to used, set for every row whose reason says it is worth changing, signin for every sign-in row at its recommended choice, and out set to {recipe_path}."
    );
    [
        "Write the recipe for this person's wsp machine image from what their agents actually used on this computer.",
        "Use the wsp tools recipe_scan and recipe, and nothing else: run no commands and read no files.",
        "Call recipe_scan first and read every row's recommended value and its reason.",
        then.as_str(),
        "Ask them nothing: they review every row in the app once the file is written.",
        "Reply with one line saying the recipe is written.",
    ]
    .join(" ")
}

/// What the agent step says when the thread's turn ended and no recipe arrived at the path the brief named: the
/// turn's own reason where it had one, and what the file was waited on for. A recipe that was already beside the
/// state is not this thread's, so this is the line even when a file is sitting there.
// init.ts:596
// export function initAgentNoRecipeLine(recipePath: string, reason?: string): string {
//   return `the thread ended without writing ${recipePath}${reason === undefined || reason === "" ? "" : `: ${reason}`}`;
// }
pub fn init_agent_no_recipe_line(recipe_path: &str, reason: Option<&str>) -> String {
    let tail = match reason {
        None | Some("") => String::new(),
        Some(reason) => format!(": {reason}"),
    };
    format!("the thread ended without writing {recipe_path}{tail}")
}
