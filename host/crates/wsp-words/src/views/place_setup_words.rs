// SPDX-License-Identifier: AGPL-3.0-only
//! The words of a computer's setup: its state word, the rows of what goes on it, the sign-in ways, and the lines a provision prints as it packs, ships and lands. Port of the functions of packages/protocol/src/views/place-setup-words.ts that wsp-wire left to this crate, at
//! the cut (383708c78); that file's shapes are wsp-wire's. See spec/views_place_setup_words.md.

use crate::workspace_state::AbsentComputer;
use wsp_wire::views::place::PendingComputer;
use wsp_wire::views::place::PendingStep;
use wsp_wire::views::place::PlaceApplied;
use wsp_wire::views::place::PlaceProvisionRow;
use wsp_wire::views::place::PlaceSetup;
use wsp_wire::views::place::PlaceSetupStep;
use wsp_wire::views::place::PlaceSync;
use wsp_wire::views::place::PlaceWait;
use wsp_wire::views::place_more::PlaceView;

use serde::Serialize;

use crate::js::number_string;
use crate::place_state::{place_at_limit_line, place_full_line};
use crate::views::place::provision_count_word;
use crate::words::base::{name_list, plural};
use crate::words::thread::iso_seconds;
use crate::words::units::{fmt_bytes, fmt_bytes_of_total};
use wsp_wire::views::place::{
    provision_counts, PlaceProvisionKind, PlaceProvisionRowOutcome, PlaceSetupLineState, PlaceSetupState, PlaceSyncState, PlaceWaitState,
    SETUP_STEP_CLASS, SETUP_STEP_WORDS,
};
use wsp_wire::wire::daemon_version::place_daemon_behind;
use wsp_wire::wire::machine_link::PLACE_BLOCKED_WORD;

/// What a computer whose recipe is still being put on says to whoever asked for a workspace there, or for a second
/// run of the job: the row under way where the job has reached one, and the two roads to the rest of the answer.
/// The refusal a setup gets on a computer while a sync to its recipe runs there: the two would install over each other.
// place-setup-words.ts:15
// export const placeSyncingLine = (name: string): string => `${name} is syncing to its recipe; set it up again once that ends`;
pub fn place_syncing_line(name: &str) -> String {
    format!("{name} is syncing to its recipe; set it up again once that ends")
}

// place-setup-words.ts:17
// export const placeProvisioningLine = (name: string, step?: PlaceSetupStep): string =>
//   `${name} is still being set up${step === undefined ? "" : ` (${SETUP_STEP_WORDS[step]})`}; wsp computers shows it, and a workspace there can be made once it is done`;
pub fn place_provisioning_line(name: &str, step: Option<&PlaceSetupStep>) -> String {
    let step = step.map_or(String::new(), |step| format!(" ({})", setup_step_words(step)));
    format!("{name} is still being set up{step}; wsp computers shows it, and a workspace there can be made once it is done")
}

/// What a join or an update says about a computer that carries no picks: nothing goes on it until it has some.
// place-setup-words.ts:21
// export const placeNoPicksLine = (name: string): string =>
//   `${name} has nothing picked to go on it: choose in the app's Add a computer, or run wsp add ${name} --resume --recipe <name>`;
pub fn place_no_picks_line(name: &str) -> String {
    format!("{name} has nothing picked to go on it: choose in the app's Add a computer, or run wsp add {name} --resume --recipe <name>")
}

/// What a computer that reported no home folder for its login gets instead of the recipe: every path the job would
/// build comes off that home, so there is nothing to build one from. Said where the no-recipe line is said.
// place-setup-words.ts:26
// export const placeNoHomeLine = (name: string): string =>
//   `${name} got no agents or tools: it reported no home folder for its login, so nothing on it could be reached`;
pub fn place_no_home_line(name: &str) -> String {
    format!("{name} got no agents or tools: it reported no home folder for its login, so nothing on it could be reached")
}

/// The rows of a setup whose failure reads Needs you: a folder that did not move, the GitHub sign-in.
// place-setup-words.ts:30
// export const importantFailures = (applied: PlaceApplied | undefined): PlaceProvisionRow[] =>
//   (applied?.rows ?? []).filter(r => r.outcome === "failed" && r.step !== undefined && SETUP_STEP_CLASS[r.step] === "important");
pub fn important_failures(applied: Option<&PlaceApplied>) -> Vec<PlaceProvisionRow> {
    let Some(applied) = applied else { return Vec::new() };
    applied
        .rows
        .iter()
        .filter(|r| r.outcome == PlaceProvisionRowOutcome::Failed && r.step.is_some_and(|step| setup_step_class(&step) == "important"))
        .cloned()
        .collect()
}

/// The setup on a computer as the TOOLS column says it: the step under way, where it stopped, or what stands.
/// Empty for a computer nothing has set up, which is every provider and this computer itself.
// place-setup-words.ts:35
// export function setupWord(setup: PlaceSetup | undefined, applied?: PlaceApplied): string {
//   if (setup === undefined) return "";
//   const under = setup.steps.find(l => l.state === "running");
//   if (setup.state === "running") return under === undefined ? "setting up" : `setting up ${SETUP_STEP_WORDS[under.step]}`;
//   if (setup.state === "failed") return `stopped: ${setup.said ?? "no reason recorded"}`;
//   const rows = applied?.rows ?? [];
//   const failed = rows.filter(r => r.outcome === "failed");
//   // What the rows put there, not the word row: a row is the recipe's own word and nobody reading this screen has
//   // seen a recipe.
//   return failed.length === 0 ? `${provisionCountWord(provisionCounts(rows))} ready` : `${failed.length} of ${rows.length} failed: ${nameList(failed.map(r => r.label))}`;
// }
pub fn setup_word(setup: Option<&PlaceSetup>, applied: Option<&PlaceApplied>) -> String {
    let Some(setup) = setup else { return String::new() };
    match setup.state {
        PlaceSetupState::Running => match setup.steps.iter().find(|line| line.state == PlaceSetupLineState::Running) {
            None => "setting up".to_string(),
            Some(under) => format!("setting up {}", setup_step_words(&under.step)),
        },
        PlaceSetupState::Failed => format!("stopped: {}", setup.said.as_deref().unwrap_or("no reason recorded")),
        PlaceSetupState::Done => {
            let rows: &[PlaceProvisionRow] = applied.map_or(&[], |applied| applied.rows.as_slice());
            let failed: Vec<&str> =
                rows.iter().filter(|r| r.outcome == PlaceProvisionRowOutcome::Failed).map(|r| r.label.as_str()).collect();
            if failed.is_empty() {
                let counts: indexmap::IndexMap<PlaceProvisionKind, f64> =
                    provision_counts(rows).into_iter().map(|(kind, n)| (kind, n as f64)).collect();
                format!("{} ready", provision_count_word(&counts))
            } else {
                format!("{} of {} failed: {}", failed.len(), rows.len(), name_list(failed.as_slice()))
            }
        }
    }
}

/// The words a computer's setup reads as, in the order they win.
// place-setup-words.ts:48
// export const SETUP_WORDS = { failed: "Setup failed", needsYou: "Needs you", pending: "Pending", settingUp: "Setting up", behind: "Behind", ready: "Ready" } as const;
/// The TS SETUP_WORDS.
pub mod setup_words {
    pub const FAILED: &str = "Setup failed";
    pub const NEEDS_YOU: &str = "Needs you";
    pub const PENDING: &str = "Pending";
    pub const SETTING_UP: &str = "Setting up";
    pub const BEHIND: &str = "Behind";
    pub const READY: &str = "Ready";
}

/// What a computer out of step with its recipe says: the rows on their way, or under way.
// place-setup-words.ts:51
// export const syncLine = (sync: PlaceSync): string =>
//   `${sync.state === "running" ? "putting on" : "waiting to put on"} the recipe's ${plural(sync.changes.length, "change")}: ${nameList(sync.changes)}`;
pub fn sync_line(sync: &PlaceSync) -> String {
    let verb = if sync.state == PlaceSyncState::Running { "putting on" } else { "waiting to put on" };
    format!("{verb} the recipe's {}: {}", plural(sync.changes.len() as f64, "change"), name_list(sync.changes.as_slice()))
}

/// What a sign-in waiting on the person says, and the same once its page ran out.
// place-setup-words.ts:55
// export const waitLine = (w: PlaceWait): string => (w.state === "expired" ? `${w.label}'s sign-in ran out; a retry asks for a fresh code` : `${w.label} waits on you to sign in${w.url === undefined ? "" : ` at ${w.url}`}${w.code === undefined ? "" : ` with code ${w.code}`}`);
pub fn wait_line(w: &PlaceWait) -> String {
    if w.state == PlaceWaitState::Expired {
        return format!("{}'s sign-in ran out; a retry asks for a fresh code", w.label);
    }
    let url = w.url.as_ref().map_or(String::new(), |url| format!(" at {url}"));
    let code = w.code.as_ref().map_or(String::new(), |code| format!(" with code {code}"));
    format!("{} waits on you to sign in{url}{code}", w.label)
}

/// One computer's word for its setup, the one rule the STATE column and the app's rows read: Setup failed, then
/// Needs you (a sign-in waits on the person, or a folder or the GitHub sign-in failed), then Pending (it joined and
/// waits on its picks, `pending` its add), then Offline (no link), then Setting up, then Behind (a change to its recipe on
/// its way, then its daemon behind this host's), then Ready. An add that never joined is a row of its own, read by pendingWord.
// place-setup-words.ts:61
// export function placeWord(place: Pick<PlaceView, "setup" | "applied" | "daemonVersion" | "sync">, absent: AbsentComputer | null, pending?: PendingComputer): PlaceState {
//   const setup = place.setup;
//   if (setup?.state === "failed") return { word: SETUP_WORDS.failed, ...(setup.said !== undefined ? { sentence: setup.said } : {}) };
//   if (pending?.failed !== undefined) return pendingWord(pending);
//   const wait = setup?.waiting[0];
//   if (wait !== undefined) return { word: SETUP_WORDS.needsYou, sentence: waitLine(wait) };
//   const important = importantFailures(place.applied);
//   if (important.length > 0) return { word: SETUP_WORDS.needsYou, sentence: important.map(r => `${r.label}: ${r.note ?? "failed"}`).join("; ") };
//   if (pending !== undefined) return pendingWord(pending);
//   if (absent !== null) return { word: absent.away, sentence: absent.sentence };
//   if (setup?.state === "running") return { word: SETUP_WORDS.settingUp, sentence: setupWord(setup) };
//   if (place.sync !== undefined) return { word: SETUP_WORDS.behind, sentence: syncLine(place.sync) };
//   const behind = placeDaemonBehind(place);
//   return behind === undefined ? { word: SETUP_WORDS.ready } : { word: behind };
// }
pub fn place_word(place: &PlaceView, absent: Option<&AbsentComputer>, pending: Option<&PendingComputer>) -> PlaceState {
    let setup = place.setup.as_ref();
    if setup.is_some_and(|setup| setup.state == PlaceSetupState::Failed) {
        return word_state(setup_words::FAILED, setup.and_then(|setup| setup.said.clone()));
    }
    if let Some(pending) = pending.filter(|p| p.failed.is_some()) {
        return pending_word(pending);
    }
    if let Some(wait) = setup.and_then(|setup| setup.waiting.first()) {
        return word_state(setup_words::NEEDS_YOU, Some(wait_line(wait)));
    }
    let important = important_failures(place.applied.as_ref());
    if !important.is_empty() {
        let sentence =
            important.iter().map(|r| format!("{}: {}", r.label, r.note.as_deref().unwrap_or("failed"))).collect::<Vec<_>>().join("; ");
        return word_state(setup_words::NEEDS_YOU, Some(sentence));
    }
    if let Some(pending) = pending {
        return pending_word(pending);
    }
    if let Some(absent) = absent {
        return word_state(absent.away.as_str(), Some(absent.sentence.clone()));
    }
    if setup.is_some_and(|setup| setup.state == PlaceSetupState::Running) {
        return word_state(setup_words::SETTING_UP, Some(setup_word(setup, None)));
    }
    if let Some(sync) = place.sync.as_ref() {
        return word_state(setup_words::BEHIND, Some(sync_line(sync)));
    }
    word_state(
        &place_daemon_behind(place.daemon_version.map(|version| version.get())).unwrap_or_else(|| setup_words::READY.to_string()),
        None,
    )
}

/// A pending add's word: Setup failed where a step stopped it, else Pending with how far it got.
// place-setup-words.ts:78
// export function pendingWord(p: PendingComputer): PlaceState {
//   if (p.failed !== undefined) return { word: SETUP_WORDS.failed, sentence: p.failed.said };
//   return { word: SETUP_WORDS.pending, sentence: PENDING_STEP_WORDS[p.step] };
// }
pub fn pending_word(p: &PendingComputer) -> PlaceState {
    match &p.failed {
        Some(failed) => word_state(setup_words::FAILED, Some(failed.said.clone())),
        None => word_state(setup_words::PENDING, Some(pending_step_words(p.step).to_string())),
    }
}

/// How far a pending add got, as its row says it.
// place-setup-words.ts:84
// export const PENDING_STEP_WORDS: Record<PendingStep, string> = {
//   connect: "connecting over ssh",
//   check: "checking it can run wsp",
//   wsp: "installing wsp",
//   floor: "joined; the base tools are going on while you choose",
//   choosing: "joined; waiting on what goes on it",
// };
/// `PENDING_STEP_WORDS[step]`.
pub fn pending_step_words(step: PendingStep) -> &'static str {
    match step {
        PendingStep::Connect => "connecting over ssh",
        PendingStep::Check => "checking it can run wsp",
        PendingStep::Wsp => "installing wsp",
        PendingStep::Floor => "joined; the base tools are going on while you choose",
        PendingStep::Choosing => "joined; waiting on what goes on it",
    }
}

/// What a place's row reads as: the word beside its name, the tone the two cap words take, and the sentence that
/// says why. Blocked first, since only a person fixes it; then the setup's own words that need the person; then not
/// answering, since nothing reaches a computer that is off; then the day's spend and the cap, which stop new work;
/// then the setup running, a daemon behind and Ready. `spentTodayUsd` is the cloud's spend since this computer's
/// midnight, and At limit is never read without it. The command line's STATE column and the app's row both read
/// this, so the two cannot word one place two ways.
// place-setup-words.ts:98
// export interface PlaceState {
//   word: string;
//   tone?: "warning";
//   sentence?: string;
// }
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaceTone {
    Warning,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlaceState {
    pub word: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tone: Option<PlaceTone>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
}

// place-setup-words.ts:104
// export function placeStateOf(place: PlaceView, absent: AbsentComputer | null, spentTodayUsd?: number, pending?: PendingComputer): PlaceState {
//   if (place.blocked !== undefined) return { word: PLACE_BLOCKED_WORD, sentence: place.blocked };
//   const setup = placeWord(place, absent, pending);
//   if (setup.word === SETUP_WORDS.failed || setup.word === SETUP_WORDS.needsYou || setup.word === SETUP_WORDS.pending || absent !== null) return setup;
//   const limit = placeAtLimitLine(place, spentTodayUsd);
//   if (limit !== undefined) return { word: "At limit", tone: "warning", sentence: limit };
//   const full = placeFullLine(place);
//   if (full !== undefined) return { word: "Full", tone: "warning", sentence: full };
//   return setup;
// }
pub fn place_state_of(
    place: &PlaceView,
    absent: Option<&AbsentComputer>,
    spent_today_usd: Option<f64>,
    pending: Option<&PendingComputer>,
) -> PlaceState {
    if let Some(blocked) = &place.blocked {
        return word_state(PLACE_BLOCKED_WORD, Some(blocked.clone()));
    }
    let setup = place_word(place, absent, pending);
    if [setup_words::FAILED, setup_words::NEEDS_YOU, setup_words::PENDING].contains(&setup.word.as_str()) || absent.is_some() {
        return setup;
    }
    if let Some(limit) = place_at_limit_line(place, spent_today_usd) {
        return PlaceState { word: "At limit".to_string(), tone: Some(PlaceTone::Warning), sentence: Some(limit) };
    }
    if let Some(full) = place_full_line(place) {
        return PlaceState { word: "Full".to_string(), tone: Some(PlaceTone::Warning), sentence: Some(full) };
    }
    setup
}

/// The lines a terminal prints once a setup is over: what this run installed by name, how many rows it found
/// already there, an earlier setup's included, then every row that failed or was set aside with its reason and what to
/// do about it, every sign-in still waiting, and what stopped the job where one did. `command` is the command line's
/// own line for a row, such as the sign-in it runs on that computer, which the app's words never carry.
// place-setup-words.ts:119
// export function setupLines(name: string, setup: PlaceSetup, applied: PlaceApplied | undefined, command: (row: PlaceProvisionRow) => string | undefined = () => undefined): string[] {
//   const rows = applied?.rows ?? [];
//   const of = (outcome: PlaceProvisionRow["outcome"]): PlaceProvisionRow[] => rows.filter(r => r.outcome === outcome);
//   const installed = of("installed").filter(r => r.earlier !== true);
//   const present = rows.filter(r => r.outcome === "present" || (r.outcome === "installed" && r.earlier === true));
//   const tally = [
//     installed.length === 0 ? "nothing installed" : `${installed.length} installed: ${nameList(installed.map(r => r.label))}`,
//     ...(present.length > 0 ? [`${present.length} already there`] : []),
//   ];
//   const toDo = (r: PlaceProvisionRow): string[] => [r.fix, command(r)].flatMap(line => (line === undefined ? [] : [`    ${line}`]));
//   return [
//     `${name}: ${tally.join(", ")}`,
//     ...of("failed").flatMap(r => [`  x ${r.label}: ${r.note ?? "no reason recorded"}`, ...toDo(r)]),
//     ...of("skipped").flatMap(r => [`  - ${r.label}: ${r.note ?? "set aside"}`, ...toDo(r)]),
//     ...setup.waiting.map(w => `  ? ${waitLine(w)}`),
//     ...(setup.said === undefined ? [] : [`${name}: ${setup.said}`]),
//   ];
// }
pub type LineCommand<'a> = &'a dyn Fn(&PlaceProvisionRow) -> Option<String>;

pub fn setup_lines(name: &str, setup: &PlaceSetup, applied: Option<&PlaceApplied>, command: Option<LineCommand>) -> Vec<String> {
    let rows: &[PlaceProvisionRow] = match applied {
        Some(applied) => &applied.rows,
        None => &[],
    };
    let of = |outcome: PlaceProvisionRowOutcome| rows.iter().filter(move |r| r.outcome == outcome);
    let installed: Vec<&str> = rows
        .iter()
        .filter(|r| r.outcome == PlaceProvisionRowOutcome::Installed && r.earlier != Some(true))
        .map(|r| r.label.as_str())
        .collect();
    let present = rows
        .iter()
        .filter(|r| {
            r.outcome == PlaceProvisionRowOutcome::Present || (r.outcome == PlaceProvisionRowOutcome::Installed && r.earlier == Some(true))
        })
        .count();
    let mut tally = vec![if installed.is_empty() {
        "nothing installed".to_string()
    } else {
        format!("{} installed: {}", installed.len(), name_list(installed.as_slice()))
    }];
    if present > 0 {
        tally.push(format!("{present} already there"));
    }
    let to_do = |r: &PlaceProvisionRow| -> Vec<String> {
        [r.fix.clone(), command.and_then(|command| command(r))].into_iter().flatten().map(|line| format!("    {line}")).collect()
    };
    let mut lines = vec![format!("{name}: {}", tally.join(", "))];
    for r in of(PlaceProvisionRowOutcome::Failed) {
        lines.push(format!("  x {}: {}", r.label, r.note.as_deref().unwrap_or("no reason recorded")));
        lines.extend(to_do(r));
    }
    for r in of(PlaceProvisionRowOutcome::Skipped) {
        lines.push(format!("  - {}: {}", r.label, r.note.as_deref().unwrap_or("set aside")));
        lines.extend(to_do(r));
    }
    lines.extend(setup.waiting.iter().map(|w| format!("  ? {}", wait_line(w))));
    if let Some(said) = &setup.said {
        lines.push(format!("{name}: {said}"));
    }
    lines
}

/// The refusal a second add to an address gets while the first stands, and what to do instead.
// place-setup-words.ts:139
// export const pendingHeldLine = (address: string, step: PendingStep): string => `${address} is already being added (${PENDING_STEP_WORDS[step]})`;
pub fn pending_held_line(address: &str, step: &PendingStep) -> String {
    format!("{address} is already being added ({})", pending_step_words(*step))
}

// place-setup-words.ts:140
// export const pendingHeldFix = (name: string): string => `Finish it with wsp add ${name} --resume --recipe <name>, or in the app.`;
pub fn pending_held_fix(name: &str) -> String {
    format!("Finish it with wsp add {name} --resume --recipe <name>, or in the app.")
}

/// The refusal picks kept for a word no pending add answers to get.
// place-setup-words.ts:142
// export const noPendingRefusal = (ref: string): string => `no add of ${ref} is pending`;
pub fn no_pending_refusal(r#ref: &str) -> String {
    format!("no add of {} is pending", r#ref)
}

/// The refusal a resume of an add that never joined gets: there is no computer to set up yet.
// place-setup-words.ts:144
// export const pendingNotJoinedLine = (name: string): string => `${name} never joined, so there is nothing to set up`;
pub fn pending_not_joined_line(name: &str) -> String {
    format!("{name} never joined, so there is nothing to set up")
}

// place-setup-words.ts:145
// export const pendingNotJoinedFix = (address: string): string => `Add it again with wsp add ${address}.`;
pub fn pending_not_joined_fix(address: &str) -> String {
    format!("Add it again with wsp add {address}.")
}

// place-setup-words.ts:147
/// The fix half of a resume that names nothing to set up with.
pub const CHOOSE_FIX: &str = "Choose what goes on it in the app's Add a computer, or name a saved recipe with --recipe <name>.";

/// How long a sign-in run on a computer you own waits on the person: a page opened and a code typed there is
/// minutes, and the codes the tools print run out in about fifteen.
// place-setup-words.ts:151
// export const SIGN_IN_WAIT_MS = 10 * 60_000;
pub const SIGN_IN_WAIT_MS: f64 = 600_000.0;

/// What a setup's rows say about where a sign-in came from, in the person's words: they never read how wsp keeps a
/// sign-in or which command it runs. `here` is the computer the host runs on, by its own name.
// place-setup-words.ts:155
// export const copiedFromLine = (here: string): string => `copied from ${here}`;
pub fn copied_from_line(here: &str) -> String {
    format!("copied from {here}")
}

// place-setup-words.ts:156
pub const SIGNED_IN_THERE: &str = "signed in on that computer";

// place-setup-words.ts:159
/// A sign-in that asks the person to pick, which the setup leaves for them: it runs at its own terminal on that
/// computer once they sign it in.
pub const AT_ITS_TERMINAL: &str = "signs in at its own terminal on that computer";

/// A sign-in on that computer whose agent did not install there, which has nothing to run until it does.
// place-setup-words.ts:161
// export const waitsForInstallLine = (name: string): string => `waits for ${name} to install`;
pub fn waits_for_install_line(name: &str) -> String {
    format!("waits for {name} to install")
}

// place-setup-words.ts:162
pub const NO_SIGN_IN_ROAD: &str = "this host cannot run a sign-in on that computer; sign in from its page in Settings";

// place-setup-words.ts:163
pub const NO_FOLDER_ROAD: &str = "this host cannot move a folder to that computer; add the project from its page in Settings";

// place-setup-words.ts:164
// export const noCopyLine = (name: string, here: string): string => `${here} has no ${name} sign-in to copy`;
pub fn no_copy_line(name: &str, here: &str) -> String {
    format!("{here} has no {name} sign-in to copy")
}

/// A sign-in copied from this computer that the agent there still reads as signed out: the login ran out here too.
// place-setup-words.ts:166
// export const copiedNotSignedInLine = (name: string, here: string): string => `${name} still reads signed out there with the sign-in copied from ${here}`;
pub fn copied_not_signed_in_line(name: &str, here: &str) -> String {
    format!("{name} still reads signed out there with the sign-in copied from {here}")
}

/// What to do about a sign-in that had nothing to copy: sign it in on the computer itself where it has a login
/// there, else hand it the token or key it takes.
// place-setup-words.ts:169
// export const signInThereFix = (computer: string): string => `Sign in on ${computer} instead.`;
pub fn sign_in_there_fix(computer: &str) -> String {
    format!("Sign in on {computer} instead.")
}

/// A way an agent that mints its token here signs in on a computer you own: each is a choice in the picker and a
/// button on its row once that row failed or waits.
// place-setup-words.ts:173
// export type SignInWay = Extract<RecipeSignIn, "token" | "key" | "machine">;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SignInWay {
    Token,
    Key,
    Machine,
}

/// Each way's name, the same on its button, in the picker and at the head of its sentence in the row's fix.
// place-setup-words.ts:175
// export const signInWayLabel = (way: SignInWay, box: string): string => ({ token: "Make a token", key: "Use an API key", machine: `Sign in on ${box}` })[way];
pub fn sign_in_way_label(way: &SignInWay, r#box: &str) -> String {
    match way {
        SignInWay::Token => "Make a token".to_string(),
        SignInWay::Key => "Use an API key".to_string(),
        SignInWay::Machine => format!("Sign in on {}", r#box),
    }
}

/// What a row of an agent with several ways says to do: a sentence per way, each opening with its button's name and
/// saying what pressing it does. `mint` is the command that makes the token, `keyEnv` the variable the key goes under.
// place-setup-words.ts:178
// export function signInWaysFix(ways: readonly SignInWay[], o: SignInWayFacts): string {
//   return ways.map(way => `${signInWayLabel(way, o.box)}: ${signInWayDoes(way, o)}.`).join(" ");
// }
pub fn sign_in_ways_fix(ways: &[SignInWay], o: &SignInWayFacts) -> String {
    ways.iter().map(|way| format!("{}: {}.", sign_in_way_label(way, &o.r#box), sign_in_way_does(way, o))).collect::<Vec<_>>().join(" ")
}

/// The facts a way's words name: the agent, the computer the host runs on, the box, the command that makes the token
/// and the variable the key goes under.
// place-setup-words.ts:183
// export interface SignInWayFacts {
//   name: string;
//   here: string;
//   box: string;
//   mint?: string;
//   keyEnv?: string;
//   /** The key is typed into a field drawn under the row, so nothing has to be put anywhere first. */
//   field?: boolean;
// }
/// `field` is the TS optional boolean.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SignInWayFacts {
    pub name: String,
    pub here: String,
    pub r#box: String,
    pub mint: Option<String>,
    pub key_env: Option<String>,
    pub field: Option<bool>,
}

/// What one way does, as its sentence in the fix and the picker's note under a row picked that way both say it.
// place-setup-words.ts:193
// export const signInWayDoes = (way: SignInWay, o: SignInWayFacts): string =>
//   ({
//     token: `${o.mint ?? `${o.name}'s own command`} runs on ${o.here} and asks your browser once`,
//     key: o.field === true ? `paste your ${o.keyEnv ?? "key"}; wsp saves it to ~/.wsp/.env without showing it` : `put ${o.keyEnv ?? "the key"} in ~/.wsp/.env on ${o.here} first, and wsp reads it from there without showing it`,
//     machine: `${o.name}'s own sign-in runs there`,
//   })[way];
pub fn sign_in_way_does(way: &SignInWay, o: &SignInWayFacts) -> String {
    match way {
        SignInWay::Token => {
            let mint = o.mint.clone().unwrap_or_else(|| format!("{}'s own command", o.name));
            format!("{mint} runs on {} and asks your browser once", o.here)
        }
        SignInWay::Key if o.field == Some(true) => {
            format!("paste your {}; wsp saves it to ~/.wsp/.env without showing it", o.key_env.as_deref().unwrap_or("key"))
        }
        SignInWay::Key => format!(
            "put {} in ~/.wsp/.env on {} first, and wsp reads it from there without showing it",
            o.key_env.as_deref().unwrap_or("the key"),
            o.here
        ),
        SignInWay::Machine => format!("{}'s own sign-in runs there", o.name),
    }
}

/// What a sign-in row says where the vault holds what its way takes.
// place-setup-words.ts:200
// export const tokenHeldLine = (here: string): string => `the token made on ${here}`;
pub fn token_held_line(here: &str) -> String {
    format!("the token made on {here}")
}

// place-setup-words.ts:201
// export const keyHeldLine = (here: string): string => `the key in ~/.wsp/.env on ${here}`;
pub fn key_held_line(here: &str) -> String {
    format!("the key in ~/.wsp/.env on {here}")
}

/// What a row picked to take a key says where the vault holds none.
// place-setup-words.ts:203
// export const noKeyLine = (keyEnv: string, here: string): string => `~/.wsp/.env on ${here} holds no ${keyEnv}`;
pub fn no_key_line(key_env: &str, here: &str) -> String {
    format!("~/.wsp/.env on {here} holds no {key_env}")
}

/// Whether a row's note is the one a row picked to take a key says where the vault holds none, on any computer.
// place-setup-words.ts:205
// export const saysNoKey = (note: string | undefined, keyEnv: string): boolean => note?.startsWith("~/.wsp/.env on ") === true && note.endsWith(` holds no ${keyEnv}`);
pub fn says_no_key(note: Option<&str>, key_env: &str) -> bool {
    note.is_some_and(|note| note.starts_with("~/.wsp/.env on ") && note.ends_with(&format!(" holds no {key_env}")))
}

// place-setup-words.ts:208
/// A sign-in on that computer whose page hands back a code: the setup has nowhere to take the code, so the row's own
/// Sign in runs it, with the code pasted under the row.
pub const CODE_FROM_ROW: &str = "its page hands back a code, so it signs in from this row";

/// What a row the recipe took out says where a file of it stays, since the person has written it there since, and
/// where its files could not be taken off at all.
// place-setup-words.ts:211
// export const editedThereLine = (name: string, kept: readonly string[]): string => `${nameList(kept)} ${kept.length === 1 ? "was" : "were"} edited on ${name}, so ${kept.length === 1 ? "it stays" : "they stay"} and wsp no longer manages ${kept.length === 1 ? "it" : "them"}`;
pub fn edited_there_line(name: &str, kept: &[impl AsRef<str>]) -> String {
    let one = kept.len() == 1;
    format!(
        "{} {} edited on {name}, so {} and wsp no longer manages {}",
        name_list(kept),
        if one { "was" } else { "were" },
        if one { "it stays" } else { "they stay" },
        if one { "it" } else { "them" },
    )
}

// place-setup-words.ts:212
pub const UNLAND_FAILED_LINE: &str = "its files could not be taken off there; wsp remove takes them";

/// What a row the recipe took out says where the box had it before wsp: it stays.
// place-setup-words.ts:214
// export const wasThereLine = (name: string): string => `${name} had it before wsp, so it stays`;
pub fn was_there_line(name: &str) -> String {
    format!("{name} had it before wsp, so it stays")
}

// What a computer's picks weigh against the room it has, before Set up: the bytes the picks need, the bytes a setup
// keeps free there past them, the bytes free there as it last said, and how many picked rows nobody measured.
// place-setup-words.ts:218
// in wsp_wire::views::place_setup_words: PlaceEstimate

// place-setup-words.ts:219
// in wsp_wire::views::place_setup_words: PlaceEstimate

/// How much of the end of a computer's setup log one read takes: a step's output for the running view, never the
/// whole log of a long job.
// place-setup-words.ts:223
// export const SETUP_LOG_TAIL_BYTES = 64 * 1024;
pub const SETUP_LOG_TAIL_BYTES: usize = 65_536;

// place-setup-words.ts:226
/// What a row the person set aside with Skip for now says: nothing of it waits, and Settings finishes it.
pub const SKIPPED_FOR_NOW: &str = "skipped for now; finish it from the computer's page in Settings";

/// The refusal a skip naming no row that waits or failed gets.
// place-setup-words.ts:228
// export const nothingToSkipLine = (row: string, name: string): string => `nothing waits or failed under ${row} on ${name}`;
pub fn nothing_to_skip_line(row: &str, name: &str) -> String {
    format!("nothing waits or failed under {row} on {name}")
}

/// What a run of one row of a setup says where the setup holds no such row: no step by that name, no row on the record, no wait.
// place-setup-words.ts:231
// export const noSetupRowRefusal = (row: string, name: string): string => `${name}'s setup holds no row ${row} to run again`;
pub fn no_setup_row_refusal(row: &str, name: &str) -> String {
    format!("{name}'s setup holds no row {row} to run again")
}

// place-setup-words.ts:234
/// What the GitHub row says where the person skipped it, and a folder whose repository needs it to clone.
pub const GITHUB_SKIPPED_LINE: &str = "skipped; gh is not signed in there";

// place-setup-words.ts:235
pub const NEEDS_GITHUB_LINE: &str = "private; needs GitHub to clone";

// place-setup-words.ts:236
pub const WAITS_ON_GITHUB_LINE: &str = "private; waits on the GitHub sign-in to clone";

/// What a folder's row says where the recipe moved its source or name and the project wsp made from it could not
/// go first, the reason being the project remove's own.
// place-setup-words.ts:239
// export const folderMoveHeldLine = (why: string): string => `${why}, then retry to move it to the recipe's folder`;
pub fn folder_move_held_line(why: &str) -> String {
    format!("{why}, then retry to move it to the recipe's folder")
}

/// What a folder's row says where the recipe moved its source or name and the folder the project wsp made from it
/// lives in still stands on that computer, which holds the person's work and is never taken.
// place-setup-words.ts:242
// export const folderMoveStandsLine = (folder: string, computer: string): string =>
//   `${folder} still stands on ${computer}; remove or rename it there, then sync again to move the project to the recipe's folder`;
pub fn folder_move_stands_line(folder: &str, computer: &str) -> String {
    format!("{folder} still stands on {computer}; remove or rename it there, then sync again to move the project to the recipe's folder")
}

// place-setup-words.ts:245
/// What a project's servers row says where the agents' own files did not land before it, which the servers merge into.
pub const FOLDER_SERVERS_WAIT_LINE: &str = "its servers wait for the agents' own files, which did not land this run; retry once they have";

/// What a folder's row says where the recipe names a folder this computer does not have.
// place-setup-words.ts:248
// export const recipeFolderGoneLine = (folder: string): string => `there is no folder at ${folder} on this computer; point the recipe at one that exists`;
pub fn recipe_folder_gone_line(folder: &str) -> String {
    format!("there is no folder at {folder} on this computer; point the recipe at one that exists")
}

/// What to do about a row of the setup that did not land, beside what happened: the reason's own fix where the
/// reason is one wsp knows, else its step's. `box` is the computer's name.
// place-setup-words.ts:252
// export function setupRowFix(row: Pick<PlaceProvisionRow, "step" | "note">, box: string): string | undefined {
//   if (row.note === NEEDS_GITHUB_LINE) return `Sign GitHub in on ${box}, then retry.`;
//   // A folder's own line is the add's, and already says what stopped it.
//   if (row.step === undefined || row.step === "folders") return undefined;
//   const fixes: Record<Exclude<PlaceSetupStep, "folders">, string> = {
//     floor: `Check that ${box} can reach its package mirrors, then retry.`,
//     agents: `Retry, or install it on ${box} yourself and skip it here.`,
//     signins: "Retry for a fresh code, or sign in later in Settings.",
//     clis: `Retry, or install it on ${box} yourself and skip it here.`,
//     skills: "Check the skill's folder here, then retry.",
//     mcp: "Check the server's entry in the agent's settings here, then retry.",
//     folderServers: "Check the server's entry in the project's settings here, then retry.",
//     plugins: `Retry, or add it on ${box} yourself and skip it here.`,
//     configs: "Check the file here, then retry.",
//     github: "Retry for a fresh sign-in, or sign in later in Settings.",
//     context: "Retry; nothing else waits on it.",
//   };
//   return fixes[row.step];
// }
pub fn setup_row_fix(row: &PlaceProvisionRow, r#box: &str) -> Option<String> {
    if row.note.as_deref() == Some(NEEDS_GITHUB_LINE) {
        return Some(format!("Sign GitHub in on {}, then retry.", r#box));
    }
    let box_name = r#box;
    match row.step? {
        PlaceSetupStep::Folders => None,
        PlaceSetupStep::Floor => Some(format!("Check that {box_name} can reach its package mirrors, then retry.")),
        PlaceSetupStep::Agents | PlaceSetupStep::Clis => Some(format!("Retry, or install it on {box_name} yourself and skip it here.")),
        PlaceSetupStep::Signins => Some("Retry for a fresh code, or sign in later in Settings.".to_string()),
        PlaceSetupStep::Skills => Some("Check the skill's folder here, then retry.".to_string()),
        PlaceSetupStep::Mcp => Some("Check the server's entry in the agent's settings here, then retry.".to_string()),
        PlaceSetupStep::FolderServers => Some("Check the server's entry in the project's settings here, then retry.".to_string()),
        PlaceSetupStep::Plugins => Some(format!("Retry, or add it on {box_name} yourself and skip it here.")),
        PlaceSetupStep::Configs => Some("Check the file here, then retry.".to_string()),
        PlaceSetupStep::Github => Some("Retry for a fresh sign-in, or sign in later in Settings.".to_string()),
        PlaceSetupStep::Context => Some("Retry; nothing else waits on it.".to_string()),
    }
}

// place-setup-words.ts:274
/// How many of the setup's steps after the base tools run at once on a computer of this much memory: one per 2 GB,
/// at least one and at most four, so a small box never runs two installs into each other's memory.
pub const SETUP_MB_PER_STEP: i64 = 2048;

// place-setup-words.ts:275
// export const setupWidth = (memMb: number): number => Math.max(1, Math.min(4, Math.floor(memMb / SETUP_MB_PER_STEP)));
pub fn setup_width(mem_mb: f64) -> f64 {
    let steps = (mem_mb / SETUP_MB_PER_STEP as f64).floor();
    if steps.is_nan() {
        return f64::NAN;
    }
    steps.clamp(1.0, 4.0)
}

/// Why a setup stopped at the floor, naming the base tools that did not install.
// place-setup-words.ts:278
// export const floorFailedLine = (rows: readonly Pick<PlaceProvisionRow, "label">[]): string => `the base tools did not install: ${nameList(rows.map(r => r.label))}`;
pub fn floor_failed_line(rows: &[PlaceProvisionRow]) -> String {
    let labels: Vec<&str> = rows.iter().map(|r| r.label.as_str()).collect();
    format!("the base tools did not install: {}", name_list(labels.as_slice()))
}

/// Why a setup stopped at the agents: every one picked failed.
// place-setup-words.ts:280
// export const noAgentLine = (rows: readonly Pick<PlaceProvisionRow, "label" | "note">[]): string =>
//   `no agent installed: ${rows.map(r => `${r.label}${r.note === undefined ? "" : ` (${r.note})`}`).join("; ")}`;
pub fn no_agent_line(rows: &[PlaceProvisionRow]) -> String {
    let said: Vec<String> = rows
        .iter()
        .map(|r| match &r.note {
            Some(note) => format!("{} ({note})", r.label),
            None => r.label.clone(),
        })
        .collect();
    format!("no agent installed: {}", said.join("; "))
}

/// One line of the job's own log on a computer you own: the time it was written, in UTC to the second, then the
/// line. Every line that log takes carries one, so what each part of a run took is read off the computer's own log
/// afterwards rather than timed while it happens.
// place-setup-words.ts:286
// export const provisionLogLine = (at: number, line: string): string => `${isoSeconds(at)} ${line}`;
pub fn provision_log_line(at: f64, line: &str) -> String {
    format!("{} {line}", iso_seconds(at))
}

/// What the round that lands the person's agent files packed on this computer: the paths the recipe planned, then
/// what the pack counted where it asked that computer first, the files already standing there at the same bytes and
/// the files the archive holds, and what that archive weighs. A pack that asked nothing says its bytes alone.
// place-setup-words.ts:291
// export const provisionPackedLine = (paths: number, p: { bytes: number; files?: number; stood?: number }): string => {
//   const counted = p.files === undefined || p.stood === undefined ? "" : `${plural(p.stood, "file")} ${p.stood === 1 ? "stands" : "stand"} there already, ${plural(p.files, "file")} packed, `;
//   return `packed on this computer: ${plural(paths, "path")}, ${counted}${fmtBytes(p.bytes)}`;
// };
/// The TS `{ bytes, files?, stood? }` of a packed bundle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Packed {
    pub bytes: f64,
    pub files: Option<f64>,
    pub stood: Option<f64>,
}

pub fn provision_packed_line(paths: f64, p: &Packed) -> String {
    let counted = match (p.files, p.stood) {
        (Some(files), Some(stood)) => format!(
            "{} {} there already, {} packed, ",
            plural(stood, "file"),
            if stood == 1.0 { "stands" } else { "stand" },
            plural(files, "file")
        ),
        _ => String::new(),
    };
    format!("packed on this computer: {}, {counted}{}", plural(paths, "path"), fmt_bytes(p.bytes))
}

/// What one part of that archive came to on the way over: the bytes it carried, and the pieces the road cut it
/// into where the bytes travel on one exec frame each, which is how a computer you own is reached.
// place-setup-words.ts:298
// export const provisionShippedLine = (p: { part: number; parts: number; bytes: number; total: number; pieces?: number }): string => {
//   const took = p.pieces === undefined ? "" : ` in ${plural(p.pieces, "piece")}`;
//   return p.parts === 1 ? `shipped: ${fmtBytes(p.total)}${took}` : `shipped part ${p.part} of ${p.parts}: ${fmtBytesOfTotal(p.bytes, p.total)}${took}`;
// };
/// The TS `{ part, parts, bytes, total, pieces? }` of a shipped part.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shipped {
    pub part: f64,
    pub parts: f64,
    pub bytes: f64,
    pub total: f64,
    pub pieces: Option<f64>,
}

pub fn provision_shipped_line(p: &Shipped) -> String {
    let took = p.pieces.map_or(String::new(), |pieces| format!(" in {}", plural(pieces, "piece")));
    if p.parts == 1.0 {
        return format!("shipped: {}{took}", fmt_bytes(p.total));
    }
    format!("shipped part {} of {}: {}{took}", number_string(p.part), number_string(p.parts), fmt_bytes_of_total(p.bytes, Some(p.total)))
}

/// What the run on that computer walked once the archive was there: every file of the person's copy, each read
/// against what stands at its path.
// place-setup-words.ts:305
// export const provisionLandedLine = (files: number): string => `landed: ${plural(files, "file")} walked on that computer`;
pub fn provision_landed_line(files: f64) -> String {
    format!("landed: {} walked on that computer", plural(files, "file"))
}

/// What the list beside the job holds for the servers round: one key per server wsp wrote into an agent's own
/// file there.
// place-setup-words.ts:309
// export const provisionListReadLine = (keys: number): string => `list read: ${plural(keys, "server key")}`;
pub fn provision_list_read_line(keys: f64) -> String {
    format!("list read: {}", plural(keys, "server key"))
}

/// What the servers round wrote into the agents' own files on that computer, of the servers the recipe names.
// place-setup-words.ts:312
// export const provisionServersLine = (written: number, servers: number): string =>
//   `servers merged: ${written} of ${plural(servers, "server")} written into the agents' own files`;
pub fn provision_servers_line(written: f64, servers: f64) -> String {
    format!("servers merged: {} of {} written into the agents' own files", number_string(written), plural(servers, "server"))
}

/// The refusal an update gets where this wsp holds no daemon built for the chip that computer said it is.
// place-setup-words.ts:316
// export const placeNoChipLine = (name: string, platform: string, arch: string): string =>
//   `${name} says it is ${platform} ${arch}, and this wsp carries no daemon built for it`;
pub fn place_no_chip_line(name: &str, platform: &str, arch: &str) -> String {
    format!("{name} says it is {platform} {arch}, and this wsp carries no daemon built for it")
}

fn word_state(word: &str, sentence: Option<String>) -> PlaceState {
    PlaceState { word: word.to_string(), tone: None, sentence }
}

fn setup_step_key(step: &PlaceSetupStep) -> &'static str {
    match step {
        PlaceSetupStep::Floor => "floor",
        PlaceSetupStep::Agents => "agents",
        PlaceSetupStep::Signins => "signins",
        PlaceSetupStep::Clis => "clis",
        PlaceSetupStep::Skills => "skills",
        PlaceSetupStep::Mcp => "mcp",
        PlaceSetupStep::Plugins => "plugins",
        PlaceSetupStep::Configs => "configs",
        PlaceSetupStep::Folders => "folders",
        PlaceSetupStep::FolderServers => "folderServers",
        PlaceSetupStep::Github => "github",
        PlaceSetupStep::Context => "context",
    }
}

/// `SETUP_STEP_WORDS[step]`.
fn setup_step_words(step: &PlaceSetupStep) -> &'static str {
    let key = setup_step_key(step);
    SETUP_STEP_WORDS.iter().find(|&&(k, _)| k == key).map_or(key, |&(_, word)| word)
}

/// `SETUP_STEP_CLASS[step]`.
fn setup_step_class(step: &PlaceSetupStep) -> &'static str {
    let key = setup_step_key(step);
    SETUP_STEP_CLASS.iter().find(|&&(k, _)| k == key).map_or("", |&(_, class)| class)
}
