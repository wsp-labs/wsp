// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/harness-picks.ts at a3b870a6e: one harness catalog's marked defaults, the efforts a
//! model takes, every model a start may name, and the picks a thread's rows carry. See spec/harness.md.

use serde::{Deserialize, Serialize};

use crate::views::harness::{HarnessCatalog, HarnessModel, HarnessOption};
use crate::views::session::SessionView;

/// An option of a picker list: HarnessOption, and HarnessModel, which is one with more beside it.
pub trait PickOption {
    fn value(&self) -> &str;
    fn is_default(&self) -> Option<bool>;
}

impl PickOption for HarnessOption {
    fn value(&self) -> &str {
        todo!()
    }
    fn is_default(&self) -> Option<bool> {
        todo!()
    }
}

impl PickOption for HarnessModel {
    fn value(&self) -> &str {
        todo!()
    }
    fn is_default(&self) -> Option<bool> {
        todo!()
    }
}

// harness-picks.ts:9 at a3b870a6e
// export function markedDefault<T extends HarnessOption>(options: ReadonlyArray<T>): T | undefined {
//   return options.find(o => o.isDefault === true);
// }
/// The first option whose `is_default` is `Some(true)`.
pub fn marked_default<T: PickOption>(options: &[T]) -> Option<&T> {
    todo!()
}

// harness-picks.ts:22 at a3b870a6e
// export function effortsFor(catalog: HarnessCatalog, model: HarnessModel | null): HarnessOption[] {
//   const options = narrowed(catalog.efforts, model?.efforts);
//   const own = model?.defaultEffort;
//   if (own === undefined) return options;
//   return options.map(({ isDefault: _harness, ...rest }) => (rest.value === own ? { ...rest, isDefault: true } : rest));
// }
/// The model's own efforts in the catalog's order (an effort the catalog lacks is dropped), else all of them. Where
/// the model names `default_effort`, every option's `is_default` is cleared and the one whose value is that effort
/// gets `Some(true)`.
pub fn efforts_for(catalog: &HarnessCatalog, model: Option<&HarnessModel>) -> Vec<HarnessOption> {
    todo!()
}

// harness-picks.ts:30 at a3b870a6e
// export function everyModel(catalog: HarnessCatalog): HarnessModel[] {
//   return [...catalog.models, ...(catalog.legacyModels ?? []), ...(catalog.hiddenModels ?? [])];
// }
pub fn every_model(catalog: &HarnessCatalog) -> Vec<HarnessModel> {
    todo!()
}

// harness-picks.ts:35 at a3b870a6e
// export function modelOf(catalog: HarnessCatalog, value: string | undefined): HarnessModel | null {
//   if (value === undefined) return null;
//   return everyModel(catalog).find(m => m.value === value) ?? { value, label: value };
// }
/// None for no value; the first model of `every_model` with that value; else a model of that value whose label is
/// the value and nothing else set.
pub fn model_of(catalog: &HarnessCatalog, value: Option<&str>) -> Option<HarnessModel> {
    todo!()
}

// harness-picks.ts:41 at a3b870a6e
// export function contextWindowsFor(catalog: HarnessCatalog, model: HarnessModel | null): HarnessOption[] {
//   return model === null ? [] : narrowed(catalog.contextWindows, model.contextWindows);
// }
pub fn context_windows_for(catalog: &HarnessCatalog, model: Option<&HarnessModel>) -> Vec<HarnessOption> {
    todo!()
}

// harness-picks.ts:53 at a3b870a6e
// export function listedPick(options: ReadonlyArray<HarnessOption>, value: string | undefined): string | undefined {
//   return value !== undefined && options.some(o => o.value === value) ? value : undefined;
// }
pub fn listed_pick<T: PickOption>(options: &[T], value: Option<&str>) -> Option<String> {
    todo!()
}

// harness-picks.ts:58 at a3b870a6e
// export type RanPicks = Partial<Record<"model" | "effort" | "contextWindow" | "permissionMode", string>>;
/// The picks a thread's turns ran with. Keys in the order `recorded_picks` writes them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RanPicks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub permission_mode: Option<String>,
}

// harness-picks.ts:65 at a3b870a6e
// export function modelPicks(model: string, contextWindow?: string): RanPicks {
//   const window = contextWindow ?? (ONE_M.test(model) ? "1m" : undefined);
//   return { model: model.replace(ONE_M, ""), ...(window !== undefined ? { contextWindow: window } : {}) };
// }
/// The model with a trailing `[1m]` taken off, and the window: the one given, else `1m` where that suffix was there.
pub fn model_picks(model: &str, context_window: Option<&str>) -> RanPicks {
    todo!()
}

// harness-picks.ts:80 at a3b870a6e
// export function recordedPicks(rows: ReadonlyArray<SessionView>): RanPicks {
//   const picks: RanPicks = {};
//   for (const read of ROW_READERS) {
//     for (let i = rows.length - 1; i >= 0; i--) {
//       const named = read(rows[i]!);
//       if (named !== null) {
//         Object.assign(picks, named);
//         break;
//       }
//     }
//   }
//   return picks;
// }
pub fn recorded_picks(rows: &[SessionView]) -> RanPicks {
    todo!()
}

/// What `kept_picks` answers: RanPicks less the access, keys in the order it writes them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptPicks {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context_window: Option<String>,
}

// harness-picks.ts:98 at a3b870a6e
// export function keptPicks(catalog: HarnessCatalog | undefined, ran: RanPicks, named: Omit<RanPicks, "permissionMode">): Omit<RanPicks, "permissionMode"> {
//   const picked = (model: string | undefined, effort: string | undefined, contextWindow: string | undefined): Omit<RanPicks, "permissionMode"> => ({
//     ...(model !== undefined ? { model } : {}),
//     ...(effort !== undefined ? { effort } : {}),
//     ...(contextWindow !== undefined ? { contextWindow } : {}),
//   });
//   if (catalog === undefined) return picked(named.model ?? ran.model, named.effort ?? ran.effort, named.contextWindow ?? ran.contextWindow);
//   // The thread's model only where it takes the effort and the window the start names beside it, which win over it.
//   const takes = (model: HarnessModel | null): boolean =>
//     (named.effort === undefined || catalog.efforts.length === 0 || listedPick(effortsFor(catalog, model), named.effort) !== undefined) &&
//     (named.contextWindow === undefined || listedPick(contextWindowsFor(catalog, model), named.contextWindow) !== undefined);
//   const own = modelOf(catalog, listedPick(everyModel(catalog), ran.model));
//   const chosen = named.model !== undefined ? modelOf(catalog, named.model) : own !== null && takes(own) ? own : null;
//   return picked(chosen?.value, named.effort ?? listedPick(effortsFor(catalog, chosen), ran.effort), named.contextWindow ?? listedPick(contextWindowsFor(catalog, chosen), ran.contextWindow));
// }
/// `named` is read for its model, effort and context window only.
pub fn kept_picks(catalog: Option<&HarnessCatalog>, ran: &RanPicks, named: &KeptPicks) -> KeptPicks {
    todo!()
}
