// SPDX-License-Identifier: AGPL-3.0-only
//! What a place's cap is and what it counts, read off the rows the host lists: which place a workspace runs on, the
//! settings each kind of place takes and how a row says them, the room left, and the lines a thread waiting for a
//! slot is told. Port of the functions of packages/protocol/src/place-state.ts at a3b870a6e; its constants are
//! wsp_wire::types::place_state. See spec/place_state.md.

use wsp_wire::types::js_number::{JsInt, JsNumber};
use wsp_wire::types::place_state::{CAP_RAISE_ACT, CLOUD_CAP_DEFAULT, DELETE_AFTER_MS, THREAD_MEM_MB, TURN_WALL_MS};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::place::{CloudCap, ComputerCap, PlaceCap, PlaceCapSet, PlaceSettingWord, PlaceSettings, PlaceSettingsAsk, PlaceView};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::session::{SessionStatus, ThreadCapWait, ThreadCapWaitWhy, ThreadView};
use wsp_wire::views::session_events_more::{SessionUncappedEvent, SessionUncappedEventWhy};
use wsp_wire::views::workspace::{WorkspacePhase, WorkspaceView};
use wsp_wire::wire::capabilities::WorkspaceSize;

use crate::js::{js_round, number_string};
use crate::place_word::{place_said, PlaceRef};
use crate::words::base::plural;
use crate::words::computer::{agents_line, AgentsSwitch, SPEND_LIMIT_LINE};
use crate::words::thread::{fmt_limit, refusal_line};
use crate::words::units::{fmt_cost, fmt_duration, fmt_size, size_offer, size_refusal, size_word, SIZE_PICK_FIX};
use crate::workspace_state::{is_local_workspace, workspace_state, WorkspaceState, WorkspaceStateInput};

/// Which computer a workspace stands on, by place id: a workspace forked on a joined computer carries that
/// computer's id on its record. Undefined for everything on this computer or at a provider. Written once because
/// the host asks it to know whether anything can be asked of the machine, and the app asks it to know which row of
/// the places table a workspace belongs to.
// place-state.ts:16
// export function workspacePlace(view: Pick<WorkspaceView, "place">): string | undefined {
//   return view.place;
// }
pub fn workspace_place(view: &WorkspaceView) -> Option<String> {
    view.place.clone()
}

/// Which row of the places list a workspace stands on, by id: the computer its record names, this computer for a
/// workspace that is this computer, and for a fork the provider its record was stamped with. A fork written before
/// records carried that word stands at the first provider row, which is where a host that forks at one provider put
/// it. Nothing for a workspace whose row this list does not hold, a stamped fork included: a provider that has been
/// removed takes its money off the list with it, and standing its workspaces on whatever provider is left would put
/// one provider's spend on another's row.
///
/// The one reading, so the settings table, the sidebar's rows and the host's own spend fold cannot disagree about
/// which row a workspace belongs to.
// place-state.ts:29
// export function workspacePlaceId(view: Pick<WorkspaceView, "kind" | "machineId" | "place" | "provider">, places: readonly Pick<PlaceView, "id" | "kind">[]): string | undefined {
//   const named = workspacePlace(view);
//   if (named !== undefined) return places.find(p => p.id === named)?.id;
//   if (isLocalWorkspace(view)) return places.find(p => p.id === HERE_PLACE_ID)?.id ?? places[0]?.id;
//   const providers = places.filter(p => p.kind === "provider");
//   return view.provider === undefined ? providers[0]?.id : providers.find(p => p.id === view.provider)?.id;
// }
pub fn workspace_place_id(view: &WorkspaceView, places: &[PlaceView]) -> Option<String> {
    if let Some(named) = workspace_place(view) {
        return places.iter().find(|place| place.id == named).map(|place| place.id.clone());
    }
    if is_local_workspace(view.kind.as_ref()) {
        return places.iter().find(|place| place.id == HERE_PLACE_ID).or(places.first()).map(|place| place.id.clone());
    }
    let providers: Vec<&PlaceView> = places.iter().filter(|place| matches!(place.kind, PlaceKind::Provider)).collect();
    match &view.provider {
        None => providers.first().map(|place| place.id.clone()),
        Some(provider) => providers.iter().find(|place| place.id == *provider).map(|place| place.id.clone()),
    }
}

// place-state.ts:40
// in wsp_wire::types::place_state: THREAD_MEM_MB

/// Threads at once on a computer of this shape until the person sets a number: one per THREAD_MEM_MB, rounded,
/// at least one, and never more than twice its cores.
// place-state.ts:44
// export const threadsAtOnce = (s: WorkspaceSize): number => Math.max(1, Math.min(2 * Math.floor(s.cpu), Math.round(s.memMb / THREAD_MEM_MB)));
pub fn threads_at_once(s: &WorkspaceSize) -> f64 {
    let by_memory = js_round(s.mem_mb.0 / THREAD_MEM_MB as f64);
    (2.0 * s.cpu.0.floor()).min(by_memory).max(1.0)
}

// place-state.ts:46
// export const CLOUD_CAP_DEFAULT: CloudCap = { machines: 3, spendPerDayUsd: 10 };
// in wsp_wire::types::place_state: CLOUD_CAP_DEFAULT

/// What a workspace's switch reads as when neither it nor its place sets one, and what it takes when a person names
/// no numbers. Three machines is what one root thread's builders need and few enough that a runaway is a bill a
/// person notices, and two levels let a lead hand work to a sub-lead; threads at once and the machines cap bound cost.
// place-state.ts:51
// export const AGENTS_ON: WorkspaceAgents = { spawn: true, maxMachines: 3, maxDepth: 2 };
// in wsp_wire::types::place_state: AGENTS_ON

/// How long a quiet workspace runs before it naps until the person sets another window.
// place-state.ts:54
// export const NAP_AFTER_MS = 20 * 60_000;
// in wsp_wire::types::place_state: NAP_AFTER_MS

/// The longest nap window a person may set: the provider's own timer is set behind ours at twice the window, and
/// six hours is the longest it has accepted.
// place-state.ts:57
// export const NAP_AFTER_MAX_MS = 3 * 60 * 60_000;
// in wsp_wire::types::place_state: NAP_AFTER_MAX_MS

/// The nap window a person names in minutes: none of them never naps.
// place-state.ts:60
// export const napMsOf = (minutes: number): number | null => (minutes === 0 ? null : minutes * 60_000);
pub fn nap_ms_of(minutes: f64) -> Option<f64> {
    if minutes == 0.0 {
        None
    } else {
        Some(minutes * 60_000.0)
    }
}

/// How long a finished sub-thread's machine stays before it is deleted until the person sets another delay.
// place-state.ts:63
// export const DELETE_AFTER_MS = 30 * 60_000;
// in wsp_wire::types::place_state: DELETE_AFTER_MS

/// The longest delete delay a person may set: a day.
// place-state.ts:65
// export const DELETE_AFTER_MAX_MS = 24 * 60 * 60_000;
// in wsp_wire::types::place_state: DELETE_AFTER_MAX_MS

/// The turn limit on a cloud account until the person sets one: what a forgotten turn there can bill by the hour. A
/// computer the person owns has none by default, since TURN_IDLE_MS already ends a stuck turn and nothing there bills.
// place-state.ts:69
// export const TURN_WALL_MS = 6 * 60 * 60_000;
// in wsp_wire::types::place_state: TURN_WALL_MS

/// The longest turn limit a person may set: past a day the number is more likely minutes typed as hours than a turn
/// anyone means to run, and off is there for one that should never stop.
// place-state.ts:73
// export const TURN_LIMIT_MAX_MS = 24 * 60 * 60_000;
// in wsp_wire::types::place_state: TURN_LIMIT_MAX_MS

/// The turn limit a person names in hours: none of them is no limit.
// place-state.ts:76
// export const turnLimitMsOf = (hours: number): number | null => (hours === 0 ? null : hours * 3_600_000);
pub fn turn_limit_ms_of(hours: f64) -> Option<f64> {
    if hours == 0.0 {
        None
    } else {
        Some(hours * 3_600_000.0)
    }
}

/// What a workspace runs under for one setting: its own, else its place's, else the default. Null is a value (a nap
/// that is off), so only an absent one falls through. The one chain the host's row and its idle and spawn rules read.
// place-state.ts:80
// export function settingFor<T>(own: T | undefined, placed: T | undefined, fallback: T): T {
//   return own !== undefined ? own : placed !== undefined ? placed : fallback;
// }
pub fn setting_for<T: Clone>(own: Option<&T>, placed: Option<&T>, fallback: &T) -> T {
    match (own, placed) {
        (Some(own), _) => own.clone(),
        (None, Some(placed)) => placed.clone(),
        (None, None) => fallback.clone(),
    }
}

/// Whether a machine in this phase holds one of its cloud's slots: a napping machine holds none, a gone or released
/// one is not there and a queued one waiting with `queue` has none yet, while one creating or waking already does.
/// Phase alone, so a refusal can read it without asking the provider about every machine.
// place-state.ts:87
// export const phaseHoldsSlot = (w: Pick<WorkspaceView, "phase" | "queue">): boolean => {
//   const state = workspaceState({ phase: w.phase });
//   return state !== "paused" && state !== "gone" && state !== "released" && !(w.phase === "queued" && w.queue !== undefined);
// };
pub fn phase_holds_slot(w: &WorkspaceView) -> bool {
    let state = workspace_state(&WorkspaceStateInput { phase: w.phase, machine_state: None, reach: None, wake_refused: None });
    state != WorkspaceState::Paused && state != WorkspaceState::Gone && state != WorkspaceState::Released && !(w.phase == WorkspacePhase::Queued && w.queue.is_some())
}

/// What the count reads of a workspace and of a thread.
// place-state.ts:93
// export type PlacedWorkspace = Pick<WorkspaceView, "id" | "kind" | "machineId" | "place" | "provider" | "phase" | "queue">;
/// The fields of a workspace the counts read.
pub type PlacedWorkspace = WorkspaceView;

// place-state.ts:94
// export type PlacedThread = Pick<ThreadView, "workspaceId" | "status">;
/// The fields of a thread the counts read.
pub type PlacedThread = ThreadView;

/// A dollar figure a person typed, in whole dollars where it has no cents.
// place-state.ts:97
// private in the TS: port it as a private helper
// const dollars = (usd: number): string => (Number.isInteger(usd) ? `$${usd}` : fmtCost(usd));

/// One number of a cap, off the cap that holds it.
// place-state.ts:100
// private in the TS: port it as a private helper
// const capNumber = (cap: PlaceCap | undefined, key: "threads" | "machines" | "spendPerDayUsd"): number | undefined =>
//   cap !== undefined && key in cap ? (cap as Record<string, number>)[key] : undefined;

// place-state.ts:103
// type SettingView = Pick<PlaceView, "cap" | "capDefault" | "napMs" | "napDefault" | "turnLimitMs" | "turnLimitDefault" | "spawn" | "spawnDefault" | "size" | "sizeDefault" | "deleteAfterMs">;

/// What a row says a setting runs at: the whole phrase, and the figure alone that a default beside it takes.
// place-state.ts:105
// type SettingSaid = { long: string; short: string };

/// Every setting a person can make on a place: the key it is stored under (and for a part of the agents switch, the
/// parts of it), the words a refusal names it by, and what a row says it runs at now or by default.
// place-state.ts:109
// private in the TS: port it as a private helper
// const SETTINGS: Record<PlaceSettingWord, { key: keyof PlaceSettings; parts?: readonly (keyof WorkspaceAgents)[]; words: string; reads(place: SettingView, fallback: boolean): SettingSaid | undefined }> = {
//   threads: { key: "threads", words: "threads at once", reads: (p, fallback) => atOnce(capNumber(fallback ? p.capDefault : p.cap, "threads"), "thread") },
//   machines: { key: "machines", words: "machines at once", reads: (p, fallback) => atOnce(capNumber(fallback ? p.capDefault : p.cap, "machines"), "machine") },
//   spend: {
//     key: "spendPerDayUsd",
//     words: "spend per day",
//     reads: (p, fallback) => {
//       const usd = capNumber(fallback ? p.capDefault : p.cap, "spendPerDayUsd");
//       return usd === undefined ? undefined : { long: `${dollars(usd)} a day`, short: dollars(usd) };
//     },
//   },
//   size: {
//     key: "size",
//     words: "size",
//     reads: (p, fallback) => {
//       const size = fallback ? p.sizeDefault : p.size;
//       return size === undefined ? undefined : { long: `size ${fmtSize(size)}`, short: sizeWord(size) };
//     },
//   },
//   "delete-after": {
//     key: "deleteAfterMs",
//     words: "delete after",
//     reads: (p, fallback) => {
//       const ms = fallback ? DELETE_AFTER_MS : p.deleteAfterMs;
//       if (ms === undefined) return undefined;
//       return ms === 0 ? { long: "deleted once done", short: "at once" } : { long: `deleted after ${ms / 60_000} min`, short: `${ms / 60_000} min` };
//     },
//   },
//   nap: {
//     key: "napMs",
//     words: "nap after",
//     reads: (p, fallback) => {
//       const ms = fallback ? p.napDefault : p.napMs;
//       if (ms === undefined) return undefined;
//       return ms === null ? { long: "never naps", short: "never" } : { long: `naps after ${fmtDuration(ms)}`, short: fmtDuration(ms) };
//     },
//   },
//   "turn-limit": {
//     key: "turnLimitMs",
//     words: "turn limit",
//     reads: (p, fallback) => {
//       const ms = fallback ? p.turnLimitDefault : p.turnLimitMs;
//       if (ms === undefined) return undefined;
//       return ms === null ? { long: "no turn limit", short: "off" } : { long: `stops a turn at ${fmtLimit(ms)}`, short: fmtLimit(ms) };
//     },
//   },
//   spawn: {
//     key: "spawn",
//     parts: ["spawn", "maxMachines"],
//     words: "agents may start agents",
//     reads: (p, fallback) => {
//       const agents = fallback ? p.spawnDefault : p.spawn;
//       if (agents === undefined) return undefined;
//       return { long: agentsLine(agents), short: agents.spawn ? `on, up to ${agents.maxMachines}` : "off" };
//     },
//   },
//   "max-depth": {
//     key: "spawn",
//     parts: ["maxDepth"],
//     words: "levels deep",
//     reads: (p, fallback) => {
//       const depth = (fallback ? p.spawnDefault : p.spawn)?.maxDepth;
//       return depth === undefined ? undefined : { long: `${depth} ${depth === 1 ? "level" : "levels"} deep`, short: String(depth) };
//     },
//   },
// };

// place-state.ts:176
// private in the TS: port it as a private helper
// function atOnce(n: number | undefined, noun: string): SettingSaid | undefined {
//   return n === undefined ? undefined : { long: `${plural(n, noun)} at once`, short: String(n) };
// }

/// Whether settings, held or asked for, name one setting: its key, or any of its parts of the agents switch.
// place-state.ts:181
// export function placeSettingNamed(settings: PlaceSettings, word: PlaceSettingWord): boolean {
//   const { key, parts } = SETTINGS[word];
//   return parts === undefined ? settings[key] !== undefined : parts.some(part => settings.spawn?.[part] !== undefined);
// }
pub fn place_setting_named(settings: &PlaceSettings, word: &PlaceSettingWord) -> bool {
    match word {
        PlaceSettingWord::Threads => settings.threads.is_some(),
        PlaceSettingWord::Machines => settings.machines.is_some(),
        PlaceSettingWord::Spend => settings.spend_per_day_usd.is_some(),
        PlaceSettingWord::Size => settings.size.is_some(),
        PlaceSettingWord::DeleteAfter => settings.delete_after_ms.is_some(),
        PlaceSettingWord::Nap => settings.nap_ms.is_some(),
        PlaceSettingWord::TurnLimit => settings.turn_limit_ms.is_some(),
        PlaceSettingWord::Spawn => settings.spawn.as_ref().is_some_and(|spawn| spawn.spawn.is_some() || spawn.max_machines.is_some()),
        PlaceSettingWord::MaxDepth => settings.spawn.as_ref().is_some_and(|spawn| spawn.max_depth.is_some()),
    }
}

/// Settings with one setting taken back to its default: its key dropped, or its parts of the agents switch, and the
/// switch with them once nothing of it is left.
// place-state.ts:188
// export function placeSettingDropped(settings: PlaceSettings, word: PlaceSettingWord): PlaceSettings {
//   const { key, parts } = SETTINGS[word];
//   const kept = Object.fromEntries(Object.entries(settings).filter(([k]) => k !== key)) as PlaceSettings;
//   if (parts === undefined || settings.spawn === undefined) return kept;
//   const spawn = Object.fromEntries(Object.entries(settings.spawn).filter(([part]) => !parts.includes(part as keyof WorkspaceAgents)));
//   return Object.keys(spawn).length === 0 ? kept : { ...kept, spawn };
// }
pub fn place_setting_dropped(settings: &PlaceSettings, word: &PlaceSettingWord) -> PlaceSettings {
    let mut kept = settings.clone();
    match word {
        PlaceSettingWord::Threads => kept.threads = None,
        PlaceSettingWord::Machines => kept.machines = None,
        PlaceSettingWord::Spend => kept.spend_per_day_usd = None,
        PlaceSettingWord::Size => kept.size = None,
        PlaceSettingWord::DeleteAfter => kept.delete_after_ms = None,
        PlaceSettingWord::Nap => kept.nap_ms = None,
        PlaceSettingWord::TurnLimit => kept.turn_limit_ms = None,
        PlaceSettingWord::Spawn | PlaceSettingWord::MaxDepth => {
            kept.spawn = settings.spawn.clone().and_then(|mut spawn| {
                match word {
                    PlaceSettingWord::Spawn => {
                        spawn.spawn = None;
                        spawn.max_machines = None;
                    }
                    _ => spawn.max_depth = None,
                }
                (spawn.spawn.is_some() || spawn.max_machines.is_some() || spawn.max_depth.is_some()).then_some(spawn)
            });
        }
    }
    kept
}

// place-state.ts:196
// interface PlaceCapRule {
//   /** The settings this kind takes. */
//   keys: readonly PlaceSettingWord[];
//   /** How long one turn runs before it is stopped where the person set no limit; null is none. */
//   turnLimit: number | null;
//   /** The set numbers over this kind's defaults; nothing where there is no default and nothing was set. */
//   capOf(place: Pick<PlaceView, "shape">, set: PlaceCapSet): PlaceCap | undefined;
//   /** How many at once the cap allows. */
//   atOnce(cap: PlaceCap): number;
//   /** What one of those is called in a sentence and a column. */
//   noun: string;
//   /** The spend a day stops new work at, where this kind has one. */
//   spendLimit(cap: PlaceCap): number | undefined;
//   /** How many of what the cap counts stand on the place, given the workspaces on it and every thread. */
//   count(on: readonly PlacedWorkspace[], threads: readonly PlacedThread[]): number;
// }

// place-state.ts:213
// private in the TS: port it as a private helper
// const PLACE_CAPS: Record<PlaceKind, PlaceCapRule> = {
//   computer: {
//     keys: ["threads", "nap", "turn-limit", "spawn", "max-depth"],
//     turnLimit: null,
//     capOf: (place, set) => {
//       const threads = set.threads ?? (place.shape === undefined ? undefined : threadsAtOnce(place.shape));
//       return threads === undefined ? undefined : { threads };
//     },
//     atOnce: cap => ("threads" in cap ? cap.threads : 0),
//     noun: "thread",
//     spendLimit: () => undefined,
//     count: (on, threads) => {
//       const ids = new Set(on.map(w => w.id));
//       return threads.filter(t => t.status === "running" && ids.has(t.workspaceId)).length;
//     },
//   },
//   provider: {
//     keys: ["machines", "spend", "size", "delete-after", "nap", "turn-limit", "spawn", "max-depth"],
//     turnLimit: TURN_WALL_MS,
//     capOf: (_place, set) => ({ machines: set.machines ?? CLOUD_CAP_DEFAULT.machines, spendPerDayUsd: set.spendPerDayUsd ?? CLOUD_CAP_DEFAULT.spendPerDayUsd }),
//     atOnce: cap => ("machines" in cap ? cap.machines : 0),
//     noun: "machine",
//     spendLimit: cap => ("spendPerDayUsd" in cap ? cap.spendPerDayUsd : undefined),
//     count: on => on.filter(phaseHoldsSlot).length,
//   },
// };

/// A place's cap: the numbers the person set over its kind's defaults.
// place-state.ts:241
// export function placeCapOf(place: Pick<PlaceView, "kind" | "shape">, set: PlaceCapSet = {}): PlaceCap | undefined {
//   return PLACE_CAPS[place.kind].capOf(place, set);
// }
/// `set` defaults to `{}`.
pub fn place_cap_of(place: &PlaceView, set: Option<&PlaceCapSet>) -> Option<PlaceCap> {
    match place.kind {
        PlaceKind::Computer => {
            let threads = set.and_then(|set| set.threads).or_else(|| place.shape.as_ref().map(|shape| JsInt::from(threads_at_once(shape) as i64)));
            threads.map(|threads| PlaceCap::ComputerCap(ComputerCap { threads }))
        }
        PlaceKind::Provider => Some(PlaceCap::CloudCap(CloudCap {
            machines: set.and_then(|set| set.machines).unwrap_or_else(|| JsInt::from(cloud_default("machines") as i64)),
            spend_per_day_usd: set
                .and_then(|set| set.spend_per_day_usd)
                .unwrap_or_else(|| JsNumber(cloud_default("spendPerDayUsd"))),
        })),
    }
}

/// How long one turn on a place of this kind runs before the runtime stops it: what the person set there, else the
/// kind's default; null is no limit. The one reading the row and the turn's own reader both take.
// place-state.ts:247
// export function placeTurnLimit(kind: PlaceKind, settings: PlaceSettings): number | null {
//   return settingFor(undefined, settings.turnLimitMs, PLACE_CAPS[kind].turnLimit);
// }
/// None is the TS null (no limit).
pub fn place_turn_limit(kind: &PlaceKind, settings: &PlaceSettings) -> Option<f64> {
    let fallback = match kind {
        PlaceKind::Computer => None,
        PlaceKind::Provider => Some(TURN_WALL_MS as f64),
    };
    let placed: Option<Option<f64>> = settings.turn_limit_ms.as_ref().map(|limit| limit.as_ref().map(|ms| ms.as_f64()));
    setting_for(None, placed.as_ref(), &fallback)
}

/// Why a set is refused on this place: nothing set or reset at all, a setting its kind does not take, or one both
/// set and reset. Nothing when every key fits.
// place-state.ts:253
// export function placeSetRefusal(place: Pick<PlaceView, "id" | "kind" | "name" | "takesForks" | "sizes">, set: PlaceSettingsAsk, reset: readonly PlaceSettingWord[] = []): string | undefined {
//   const takes = settingsOn(place);
//   const said = (words: readonly PlaceSettingWord[]): string => andList(words.map(word => SETTINGS[word].words));
//   const named = (Object.keys(SETTINGS) as PlaceSettingWord[]).filter(word => placeSettingNamed(set, word));
//   // A computer the person added also takes a new name and a new ssh login, which are no settings of its kind.
//   const added = place.kind === "computer" && place.id !== HERE_PLACE_ID ? ["a new name", "a new ssh login"] : [];
//   if (named.length === 0 && reset.length === 0) return `nothing to set on ${placeSaid(place)}: it takes ${andList([...takes.map(word => SETTINGS[word].words), ...added])}`;
//   const wrong = [...new Set([...named, ...reset])].filter(word => !takes.includes(word));
//   if (wrong.length > 0) return `${placeSaid(place)} takes ${said(takes)}, not ${said(wrong)}`;
//   const both = named.filter(word => reset.includes(word));
//   if (both.length > 0) return `${placeSaid(place)}: ${said(both)} is both set and reset; name it once`;
//   const size = set.size;
//   if (size !== undefined && sizeOffer(place.sizes ?? [], size) === undefined) return refusalLine(sizeRefusal(sizeWord(size), place.sizes ?? []), SIZE_PICK_FIX);
//   return undefined;
// }
/// `reset` defaults to none.
pub fn place_set_refusal(place: &PlaceView, set: &PlaceSettingsAsk, reset: &[PlaceSettingWord]) -> Option<String> {
    let takes = settings_on(place);
    let said_place = said_place(place);
    let said = |words: &[PlaceSettingWord]| and_list(&words.iter().map(|word| setting_words(*word).to_string()).collect::<Vec<_>>());
    let asked = settings_of_ask(set);
    let named: Vec<PlaceSettingWord> = SETTING_ORDER.iter().copied().filter(|word| place_setting_named(&asked, word)).collect();
    let added: Vec<&str> = if matches!(place.kind, PlaceKind::Computer) && place.id != HERE_PLACE_ID {
        vec!["a new name", "a new ssh login"]
    } else {
        Vec::new()
    };
    if named.is_empty() && reset.is_empty() {
        let mut takes_words: Vec<String> = takes.iter().map(|word| setting_words(*word).to_string()).collect();
        takes_words.extend(added.iter().map(|word| word.to_string()));
        return Some(format!("nothing to set on {said_place}: it takes {}", and_list(&takes_words)));
    }
    let mut wrong: Vec<PlaceSettingWord> = Vec::new();
    for word in named.iter().chain(reset.iter()) {
        if !takes.contains(word) && !wrong.contains(word) {
            wrong.push(*word);
        }
    }
    if !wrong.is_empty() {
        return Some(format!("{said_place} takes {}, not {}", said(&takes), said(&wrong)));
    }
    let both: Vec<PlaceSettingWord> = named.iter().copied().filter(|word| reset.contains(word)).collect();
    if !both.is_empty() {
        return Some(format!("{said_place}: {} is both set and reset; name it once", said(&both)));
    }
    if let Some(size) = &set.size {
        let wanted = WorkspaceSize { cpu: JsNumber(size.cpu.as_f64()), mem_mb: JsNumber(size.mem_mb.as_f64()) };
        let sizes = place.sizes.as_deref().unwrap_or(&[]);
        if size_offer(sizes, &wanted).is_none() {
            return Some(refusal_line(&size_refusal(&size_word(&wanted), sizes), SIZE_PICK_FIX));
        }
    }
    None
}

/// The settings a place takes: its kind's, less the nap on a place that forks nothing, whose workspaces are
/// folders on the computer itself and never nap.
// place-state.ts:271
// private in the TS: port it as a private helper
// function settingsOn(place: Pick<PlaceView, "kind" | "takesForks" | "sizes">): readonly PlaceSettingWord[] {
//   return PLACE_CAPS[place.kind].keys.filter(word => placeTakes(place, word));
// }

/// Whether a place takes one setting: its kind's, less the nap where it forks nothing and the size where its row
/// offers none.
// place-state.ts:277
// export function placeTakes(place: Pick<PlaceView, "kind" | "takesForks" | "sizes">, word: PlaceSettingWord): boolean {
//   if (!PLACE_CAPS[place.kind].keys.includes(word)) return false;
//   if (word === "nap") return place.takesForks === true;
//   return word !== "size" || (place.sizes ?? []).length > 0;
// }
pub fn place_takes(place: &PlaceView, word: &PlaceSettingWord) -> bool {
    if !kind_keys(&place.kind).contains(word) {
        return false;
    }
    if *word == PlaceSettingWord::Nap {
        return place.takes_forks == Some(true);
    }
    *word != PlaceSettingWord::Size || place.sizes.as_ref().is_some_and(|sizes| !sizes.is_empty())
}

// place-state.ts:283
// private in the TS: port it as a private helper
// const andList = (words: readonly string[]): string => (words.length < 2 ? words.join("") : `${words.slice(0, -1).join(", ")} and ${words.at(-1)}`);

/// One place's settings in a line: each its kind takes at the value it runs at, and beside one the person set what
/// it reads by default. What wsp computers set answers with.
// place-state.ts:287
// export function placeSettingsLine(place: Pick<PlaceView, "kind" | "name" | "settings" | "takesForks"> & SettingView): string {
//   const parts = settingsOn(place).flatMap(word => {
//     const { reads } = SETTINGS[word];
//     const now = reads(place, false);
//     if (now === undefined) return [];
//     if (!placeSettingNamed(place.settings ?? {}, word)) return [`${now.long} (the default)`];
//     const fallback = reads(place, true);
//     return [fallback === undefined ? now.long : `${now.long} (${fallback.short} by default)`];
//   });
//   return `${placeSaid(place)}: ${parts.join(", ")}`;
// }
pub fn place_settings_line(place: &PlaceView) -> String {
    let mut parts: Vec<String> = Vec::new();
    for word in settings_on(place) {
        let Some(now) = reads(place, &word, false) else {
            continue;
        };
        if !place.settings.as_ref().is_some_and(|settings| place_setting_named(settings, &word)) {
            parts.push(format!("{} (the default)", now.long));
            continue;
        }
        match reads(place, &word, true) {
            None => parts.push(now.long),
            Some(fallback) => parts.push(format!("{} ({} by default)", now.long, fallback.short)),
        }
    }
    format!("{}: {}", said_place(place), parts.join(", "))
}

/// How many of what a place's cap counts run there now: threads whose latest turn is running on a workspace that
/// stands on that computer, or on a cloud the machines holding a slot there.
// place-state.ts:301
// export function runningOn(placeId: string, places: readonly Pick<PlaceView, "id" | "kind">[], workspaces: readonly PlacedWorkspace[], threads: readonly PlacedThread[]): number {
//   const place = places.find(p => p.id === placeId);
//   if (place === undefined) return 0;
//   return PLACE_CAPS[place.kind].count(
//     workspaces.filter(w => workspacePlaceId(w, places) === placeId),
//     threads,
//   );
// }
pub fn running_on(place_id: &str, places: &[PlaceView], workspaces: &[PlacedWorkspace], threads: &[PlacedThread]) -> usize {
    let Some(place) = places.iter().find(|place| place.id == place_id) else {
        return 0;
    };
    let on: Vec<&WorkspaceView> = workspaces.iter().filter(|workspace| workspace_place_id(workspace, places).as_deref() == Some(place_id)).collect();
    match place.kind {
        PlaceKind::Computer => threads
            .iter()
            .filter(|thread| thread.status == SessionStatus::Running && on.iter().any(|workspace| workspace.id == thread.workspace_id))
            .count(),
        PlaceKind::Provider => on.iter().filter(|workspace| phase_holds_slot(workspace)).count(),
    }
}

/// What runs on a place against its cap, and how many more it takes under it. Nothing for a place with no cap.
// place-state.ts:311
// export function placeRoom(place: Pick<PlaceView, "kind" | "cap" | "running">): { running: number; atOnce: number; room: number; noun: string } | undefined {
//   if (place.cap === undefined) return undefined;
//   const rule = PLACE_CAPS[place.kind];
//   const running = place.running ?? 0;
//   const atOnce = rule.atOnce(place.cap);
//   return { running, atOnce, room: Math.max(0, atOnce - running), noun: rule.noun };
// }
/// A place's room: what runs, what it takes at once, what is left, and the noun it counts.
#[derive(Clone, Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRoom {
    #[serde(serialize_with = "crate::js::serialize_f64")]
    pub running: f64,
    #[serde(serialize_with = "crate::js::serialize_f64")]
    pub at_once: f64,
    #[serde(serialize_with = "crate::js::serialize_f64")]
    pub room: f64,
    pub noun: String,
}

pub fn place_room(place: &PlaceView) -> Option<PlaceRoom> {
    let cap = place.cap.as_ref()?;
    let running = place.running.map_or(0.0, |running| running.as_f64());
    let (at_once, noun) = match place.kind {
        PlaceKind::Computer => (cap_number(Some(cap), "threads").unwrap_or(0.0), "thread"),
        PlaceKind::Provider => (cap_number(Some(cap), "machines").unwrap_or(0.0), "machine"),
    };
    Some(PlaceRoom {
        running,
        at_once,
        room: (at_once - running).max(0.0),
        noun: noun.to_string(),
    })
}

/// The sentence a full place says: the person's number reached, else a computer that has no room for one more
/// fork under it. Nothing while there is room.
// place-state.ts:321
// export function placeFullLine(place: Pick<PlaceView, "kind" | "name" | "cap" | "running" | "forks">): string | undefined {
//   const room = placeRoom(place);
//   if (room !== undefined && room.running >= room.atOnce) return `full: ${room.running} of ${plural(room.atOnce, room.noun)} running`;
//   return place.forks?.room === 0 ? `no room on ${place.name}` : undefined;
// }
pub fn place_full_line(place: &PlaceView) -> Option<String> {
    if let Some(room) = place_room(place) {
        if room.running >= room.at_once {
            return Some(format!("full: {} of {} running", number_string(room.running), plural(room.at_once, &room.noun)));
        }
    }
    if place.forks.as_ref().is_some_and(|forks| forks.room.as_f64() == 0.0) {
        Some(format!("no room on {}", place.name))
    } else {
        None
    }
}

/// What holds a turn back while its computer's threads at once is met: the computer and what runs there against its
/// cap; for a sub-thread's spawn, the cloud's machines running or its starts this minute used.
// place-state.ts:329
// export const capRunningLine = (w: ThreadCapWait): string =>
//   w.why === "minute" ? `${w.place}'s starts this minute are used: ${w.running} of ${w.atOnce}` : `${w.place} is running ${w.running} of ${plural(w.atOnce, w.why === "slot" ? "machine" : "thread")}`;
pub fn cap_running_line(w: &ThreadCapWait) -> String {
    let running = number_string(w.running.as_f64());
    let at_once = number_string(w.at_once.as_f64());
    match w.why {
        Some(ThreadCapWaitWhy::Minute) => format!("{}'s starts this minute are used: {running} of {at_once}", w.place),
        why => {
            let noun = if why == Some(ThreadCapWaitWhy::Slot) { "machine" } else { "thread" };
            format!("{} is running {running} of {}", w.place, plural(w.at_once.as_f64(), noun))
        }
    }
}

/// The line a held turn says where nothing beside it says it waits: the chat's working row and the command line. A
/// spawn says a machine slot and a minute of starts apart, since raising one does nothing for the other (05, Q18).
// place-state.ts:334
// export const capWaitLine = (w: ThreadCapWait): string =>
//   w.why === "slot" ? `waiting for a machine on ${w.place}: ${w.running} of ${w.atOnce} running` : w.why === "minute" ? `waiting for ${w.place}'s next minute of starts: ${w.running} of ${w.atOnce} used` : `waiting while ${capRunningLine(w)}`;
pub fn cap_wait_line(w: &ThreadCapWait) -> String {
    let running = number_string(w.running.as_f64());
    let at_once = number_string(w.at_once.as_f64());
    match w.why {
        Some(ThreadCapWaitWhy::Slot) => format!("waiting for a machine on {}: {running} of {at_once} running", w.place),
        Some(ThreadCapWaitWhy::Minute) => format!("waiting for {}'s next minute of starts: {running} of {at_once} used", w.place),
        None => format!("waiting while {}", cap_running_line(w)),
    }
}

/// The bar a held thread's drawer row opens in its composer's place: the computer it waits on as the title, what runs
/// there and how the wait ends under it.
// place-state.ts:339
// export const capBarWords = (w: ThreadCapWait): { title: string; note: string } => ({
//   title: `Waiting for a thread slot on ${w.place}`,
//   note: `${w.running} of ${plural(w.atOnce, "thread")} ${w.running === 1 ? "is" : "are"} running. It starts on its own when one ends.`,
// });
/// The bar's title and note.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CapBarWords {
    pub title: String,
    pub note: String,
}

pub fn cap_bar_words(w: &ThreadCapWait) -> CapBarWords {
    let running = w.running.as_f64();
    CapBarWords {
        title: format!("Waiting for a thread slot on {}", w.place),
        note: format!(
            "{} of {} {} running. It starts on its own when one ends.",
            number_string(running),
            plural(w.at_once.as_f64(), "thread"),
            if running == 1.0 { "is" } else { "are" }
        ),
    }
}

/// Why a held turn left its wait, as the command line's one line before its start.
// place-state.ts:345
// export const capStartedLine = (e: Pick<SessionUncappedEvent, "why" | "atOnce">): string =>
//   e.why === "raised" ? `started: threads raised to ${e.atOnce}` : e.why === "lent" ? "started: a thread waiting on it lent its slot" : "started: a thread slot freed";
pub fn cap_started_line(e: &SessionUncappedEvent) -> String {
    match e.why {
        SessionUncappedEventWhy::Raised => format!("started: threads raised to {}", number_string(e.at_once.as_f64())),
        SessionUncappedEventWhy::Lent => "started: a thread waiting on it lent its slot".to_string(),
        SessionUncappedEventWhy::Freed => "started: a thread slot freed".to_string(),
    }
}

/// Why a held turn ended with nothing started: the person or its lead stopped it while it waited, or before it had
/// looked at its computer's threads at once and had no count to say.
// place-state.ts:350
// export const capStoppedLine = (w: ThreadCapWait | undefined): string =>
//   w === undefined ? "stopped before it started" : `stopped before it started, while ${w.place} was running ${w.running} of ${plural(w.atOnce, "thread")}`;
pub fn cap_stopped_line(w: Option<&ThreadCapWait>) -> String {
    match w {
        None => "stopped before it started".to_string(),
        Some(w) => format!(
            "stopped before it started, while {} was running {} of {}",
            w.place,
            number_string(w.running.as_f64()),
            plural(w.at_once.as_f64(), "thread")
        ),
    }
}

/// Why a start ended with nothing started: its workspace was deleted while the start was on its way, held or not.
// place-state.ts:354
// export const deletedBeforeStartLine = (name: string): string => `${name} was deleted before this turn started`;
pub fn deleted_before_start_line(name: &str) -> String {
    format!("{name} was deleted before this turn started")
}

/// Why a held turn ended with nothing started: the host restarted while it waited, and a held start lives in the
/// host's memory alone.
// place-state.ts:358
// export const capRestartedLine = (place: string): string => `the host restarted while this turn waited for a slot on ${place}; nothing started, so send it again`;
pub fn cap_restarted_line(place: &str) -> String {
    format!("the host restarted while this turn waited for a slot on {place}; nothing started, so send it again")
}

// place-state.ts:361
// in wsp_wire::types::place_state: CAP_RAISE_ACT

/// The act a held wait offers beside its count: threads or machines at once, and none for a minute of starts, which
/// the cloud's plan sets and no setting here moves.
// place-state.ts:365
// export const capRaiseAct = (w: ThreadCapWait): string | undefined => (w.why === undefined ? CAP_RAISE_ACT : w.why === "slot" ? "Raise machines at once" : undefined);
pub fn cap_raise_act(w: &ThreadCapWait) -> Option<String> {
    match w.why {
        None => Some(CAP_RAISE_ACT.to_string()),
        Some(ThreadCapWaitWhy::Slot) => Some("Raise machines at once".to_string()),
        Some(ThreadCapWaitWhy::Minute) => None,
    }
}

// place-state.ts:368
// in wsp_wire::types::place_state: WAKE_ACT

/// The spend a day stops new work on this place at: nothing on a kind with no spend limit or a row with no cap.
// place-state.ts:371
// export function placeSpendLimit(place: Pick<PlaceView, "kind" | "cap">): number | undefined {
//   return place.cap === undefined ? undefined : PLACE_CAPS[place.kind].spendLimit(place.cap);
// }
pub fn place_spend_limit(place: &PlaceView) -> Option<f64> {
    let cap = place.cap.as_ref()?;
    match place.kind {
        PlaceKind::Computer => None,
        PlaceKind::Provider => cap_number(Some(cap), "spendPerDayUsd"),
    }
}

/// The sentence a cloud at its day's spend says. Nothing without a spend figure, which is never guessed.
// place-state.ts:376
// export function placeAtLimitLine(place: Pick<PlaceView, "kind" | "cap">, spentTodayUsd: number | undefined): string | undefined {
//   const limit = placeSpendLimit(place);
//   return limit !== undefined && spentTodayUsd !== undefined && spentTodayUsd >= limit ? SPEND_LIMIT_LINE : undefined;
// }
pub fn place_at_limit_line(place: &PlaceView, spent_today_usd: Option<f64>) -> Option<String> {
    let limit = place_spend_limit(place)?;
    let spent = spent_today_usd?;
    (spent >= limit).then(|| SPEND_LIMIT_LINE.to_string())
}
const SETTING_ORDER: [PlaceSettingWord; 9] = [
    PlaceSettingWord::Threads,
    PlaceSettingWord::Machines,
    PlaceSettingWord::Spend,
    PlaceSettingWord::Size,
    PlaceSettingWord::DeleteAfter,
    PlaceSettingWord::Nap,
    PlaceSettingWord::TurnLimit,
    PlaceSettingWord::Spawn,
    PlaceSettingWord::MaxDepth,
];

struct SettingSaid {
    long: String,
    short: String,
}

fn kind_keys(kind: &PlaceKind) -> &'static [PlaceSettingWord] {
    use PlaceSettingWord::*;
    match kind {
        PlaceKind::Computer => &[Threads, Nap, TurnLimit, Spawn, MaxDepth],
        PlaceKind::Provider => &[Machines, Spend, Size, DeleteAfter, Nap, TurnLimit, Spawn, MaxDepth],
    }
}

fn settings_on(place: &PlaceView) -> Vec<PlaceSettingWord> {
    kind_keys(&place.kind).iter().copied().filter(|word| place_takes(place, word)).collect()
}

fn setting_words(word: PlaceSettingWord) -> &'static str {
    match word {
        PlaceSettingWord::Threads => "threads at once",
        PlaceSettingWord::Machines => "machines at once",
        PlaceSettingWord::Spend => "spend per day",
        PlaceSettingWord::Size => "size",
        PlaceSettingWord::DeleteAfter => "delete after",
        PlaceSettingWord::Nap => "nap after",
        PlaceSettingWord::TurnLimit => "turn limit",
        PlaceSettingWord::Spawn => "agents may start agents",
        PlaceSettingWord::MaxDepth => "levels deep",
    }
}

fn and_list(words: &[String]) -> String {
    match words.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => words.join(""),
    }
}

fn said_place(place: &PlaceView) -> String {
    place_said(&PlaceRef {
        id: place.id.clone(),
        name: place.name.clone(),
        kind: Some(wire_word(&place.kind)),
    })
}

fn wire_word<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value).ok().and_then(|word| word.as_str().map(str::to_string)).unwrap_or_default()
}

fn cloud_default(key: &str) -> f64 {
    CLOUD_CAP_DEFAULT.iter().find(|(name, _)| *name == key).map(|(_, value)| *value).unwrap_or_default()
}

fn cap_number(cap: Option<&PlaceCap>, key: &str) -> Option<f64> {
    match (cap?, key) {
        (PlaceCap::ComputerCap(cap), "threads") => Some(cap.threads.as_f64()),
        (PlaceCap::CloudCap(cap), "machines") => Some(cap.machines.as_f64()),
        (PlaceCap::CloudCap(cap), "spendPerDayUsd") => Some(cap.spend_per_day_usd.0),
        _ => None,
    }
}

fn dollars(usd: f64) -> String {
    if usd.fract() == 0.0 {
        format!("${}", number_string(usd))
    } else {
        fmt_cost(usd)
    }
}

fn at_once(n: Option<f64>, noun: &str) -> Option<SettingSaid> {
    let n = n?;
    Some(SettingSaid {
        long: format!("{} at once", plural(n, noun)),
        short: number_string(n),
    })
}

fn workspace_size(cpu: JsInt, mem_mb: JsInt) -> WorkspaceSize {
    WorkspaceSize {
        cpu: cpu.0,
        mem_mb: mem_mb.0,
    }
}

fn reads(place: &PlaceView, word: &PlaceSettingWord, fallback: bool) -> Option<SettingSaid> {
    let cap = if fallback { place.cap_default.as_ref() } else { place.cap.as_ref() };
    match word {
        PlaceSettingWord::Threads => at_once(cap_number(cap, "threads"), "thread"),
        PlaceSettingWord::Machines => at_once(cap_number(cap, "machines"), "machine"),
        PlaceSettingWord::Spend => {
            let usd = cap_number(cap, "spendPerDayUsd")?;
            Some(SettingSaid {
                long: format!("{} a day", dollars(usd)),
                short: dollars(usd),
            })
        }
        PlaceSettingWord::Size => {
            let size = if fallback {
                place.size_default.as_ref().map(|size| workspace_size(size.cpu, size.mem_mb))
            } else {
                place.size.as_ref().map(|size| workspace_size(size.cpu, size.mem_mb))
            }?;
            Some(SettingSaid {
                long: format!("size {}", fmt_size(&size, None)),
                short: size_word(&size),
            })
        }
        PlaceSettingWord::DeleteAfter => {
            let ms = if fallback {
                Some(DELETE_AFTER_MS as f64)
            } else {
                place.delete_after_ms.map(|ms| ms.as_f64())
            }?;
            if ms == 0.0 {
                return Some(SettingSaid {
                    long: "deleted once done".to_string(),
                    short: "at once".to_string(),
                });
            }
            let minutes = number_string(ms / 60_000.0);
            Some(SettingSaid {
                long: format!("deleted after {minutes} min"),
                short: format!("{minutes} min"),
            })
        }
        PlaceSettingWord::Nap => {
            let ms: Option<Option<f64>> = if fallback {
                place.nap_default.map(|ms| Some(ms.as_f64()))
            } else {
                place.nap_ms.map(|ms| ms.map(|ms| ms.as_f64()))
            };
            match ms? {
                None => Some(SettingSaid {
                    long: "never naps".to_string(),
                    short: "never".to_string(),
                }),
                Some(ms) => Some(SettingSaid {
                    long: format!("naps after {}", fmt_duration(ms, None)),
                    short: fmt_duration(ms, None),
                }),
            }
        }
        PlaceSettingWord::TurnLimit => {
            let limit: Option<Option<f64>> = if fallback {
                place.turn_limit_default.map(|ms| ms.map(|ms| ms.as_f64()))
            } else {
                place.turn_limit_ms.map(|ms| ms.map(|ms| ms.as_f64()))
            };
            match limit? {
                None => Some(SettingSaid {
                    long: "no turn limit".to_string(),
                    short: "off".to_string(),
                }),
                Some(ms) => Some(SettingSaid {
                    long: format!("stops a turn at {}", fmt_limit(ms)),
                    short: fmt_limit(ms),
                }),
            }
        }
        PlaceSettingWord::Spawn => {
            let agents = if fallback { place.spawn_default.as_ref() } else { place.spawn.as_ref() }?;
            let switch = AgentsSwitch {
                spawn: agents.spawn,
                max_machines: agents.max_machines.as_f64(),
                max_depth: agents.max_depth.as_f64(),
            };
            Some(SettingSaid {
                long: agents_line(Some(&switch)),
                short: if agents.spawn {
                    format!("on, up to {}", number_string(switch.max_machines))
                } else {
                    "off".to_string()
                },
            })
        }
        PlaceSettingWord::MaxDepth => {
            let depth = if fallback { place.spawn_default.as_ref() } else { place.spawn.as_ref() }?.max_depth.as_f64();
            Some(SettingSaid {
                long: format!("{} {} deep", number_string(depth), if depth == 1.0 { "level" } else { "levels" }),
                short: number_string(depth),
            })
        }
    }
}

fn settings_of_ask(set: &PlaceSettingsAsk) -> PlaceSettings {
    PlaceSettings {
        threads: set.threads,
        machines: set.machines,
        spend_per_day_usd: set.spend_per_day_usd,
        nap_ms: set.nap_ms,
        turn_limit_ms: set.turn_limit_ms,
        spawn: set.spawn.clone(),
        size: set.size.clone(),
        delete_after_ms: set.delete_after_ms,
    }
}
