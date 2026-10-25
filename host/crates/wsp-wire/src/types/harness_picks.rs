// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/harness-picks.ts at a3b870a6e: one harness catalog's marked defaults, the efforts a
//! model takes, every model a start may name, and the picks a thread's rows carry. See spec/harness.md.

use serde::{Deserialize, Serialize};

use crate::types::thread_defaults::Fields;
use crate::views::harness::{HarnessCatalog, HarnessModel, HarnessOption};
use crate::views::session::SessionView;

/// An option of a picker list: HarnessOption, and HarnessModel, which is one with more beside it.
pub trait PickOption {
    fn value(&self) -> &str;
    fn is_default(&self) -> Option<bool>;
}

impl PickOption for HarnessOption {
    fn value(&self) -> &str {
        &self.value
    }
    fn is_default(&self) -> Option<bool> {
        self.is_default
    }
}

impl<T: PickOption> PickOption for Fields<T> {
    fn value(&self) -> &str {
        (**self).value()
    }
    fn is_default(&self) -> Option<bool> {
        (**self).is_default()
    }
}

impl PickOption for HarnessModel {
    fn value(&self) -> &str {
        &self.value
    }
    fn is_default(&self) -> Option<bool> {
        self.is_default
    }
}

/// `narrowed`: the catalog's options a subset names, in the catalog's order; the whole list where no subset is named.
fn narrowed(all: &[HarnessOption], subset: Option<&[String]>) -> Vec<HarnessOption> {
    match subset {
        None => all.to_vec(),
        Some(names) => all.iter().filter(|o| names.contains(&o.value)).cloned().collect(),
    }
}

const ONE_M: &str = "[1m]";

// harness-picks.ts:9 at a3b870a6e
// export function markedDefault<T extends HarnessOption>(options: ReadonlyArray<T>): T | undefined {
//   return options.find(o => o.isDefault === true);
// }
/// The first option whose `is_default` is `Some(true)`.
pub fn marked_default<T: PickOption>(options: &[T]) -> Option<&T> {
    options.iter().find(|o| o.is_default() == Some(true))
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
/// gets `Some(true)`, written last as `{ ...rest, isDefault: true }` writes it.
pub fn efforts_for(catalog: &HarnessCatalog, model: Option<&HarnessModel>) -> Vec<Fields<HarnessOption>> {
    let options = narrowed(&catalog.efforts, model.and_then(|m| m.efforts.as_deref()));
    let Some(own) = model.and_then(|m| m.default_effort.as_deref()) else {
        return options.into_iter().map(Fields::new).collect();
    };
    options
        .into_iter()
        .map(|o| match o.value == own {
            true => Fields::last(HarnessOption { is_default: Some(true), ..o }, "isDefault"),
            false => Fields::new(HarnessOption { is_default: None, ..o }),
        })
        .collect()
}

// harness-picks.ts:30 at a3b870a6e
// export function everyModel(catalog: HarnessCatalog): HarnessModel[] {
//   return [...catalog.models, ...(catalog.legacyModels ?? []), ...(catalog.hiddenModels ?? [])];
// }
pub fn every_model(catalog: &HarnessCatalog) -> Vec<HarnessModel> {
    let legacy = catalog.legacy_models.iter().flatten();
    let hidden = catalog.hidden_models.iter().flatten();
    catalog.models.iter().chain(legacy).chain(hidden).cloned().collect()
}

// harness-picks.ts:35 at a3b870a6e
// export function modelOf(catalog: HarnessCatalog, value: string | undefined): HarnessModel | null {
//   if (value === undefined) return null;
//   return everyModel(catalog).find(m => m.value === value) ?? { value, label: value };
// }
/// None for no value; the first model of `every_model` with that value; else a model of that value whose label is
/// the value and nothing else set.
pub fn model_of(catalog: &HarnessCatalog, value: Option<&str>) -> Option<HarnessModel> {
    let value = value?;
    Some(every_model(catalog).into_iter().find(|m| m.value == value).unwrap_or_else(|| HarnessModel {
        value: value.to_string(),
        label: value.to_string(),
        description: None,
        is_default: None,
        short: None,
        efforts: None,
        default_effort: None,
        context_windows: None,
        fast: None,
        added: None,
    }))
}

// harness-picks.ts:41 at a3b870a6e
// export function contextWindowsFor(catalog: HarnessCatalog, model: HarnessModel | null): HarnessOption[] {
//   return model === null ? [] : narrowed(catalog.contextWindows, model.contextWindows);
// }
pub fn context_windows_for(catalog: &HarnessCatalog, model: Option<&HarnessModel>) -> Vec<HarnessOption> {
    model.map_or_else(Vec::new, |m| narrowed(&catalog.context_windows, m.context_windows.as_deref()))
}

// harness-picks.ts:53 at a3b870a6e
// export function listedPick(options: ReadonlyArray<HarnessOption>, value: string | undefined): string | undefined {
//   return value !== undefined && options.some(o => o.value === value) ? value : undefined;
// }
pub fn listed_pick<T: PickOption>(options: &[T], value: Option<&str>) -> Option<String> {
    value.filter(|v| options.iter().any(|o| o.value() == *v)).map(String::from)
}

// harness-picks.ts:58 at a3b870a6e
// export type RanPicks = Partial<Record<"model" | "effort" | "contextWindow" | "permissionMode", string>>;
/// The picks a thread's turns ran with. Keys in the order `recorded_picks` writes them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RanPicks {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub context_window: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub permission_mode: Option<String>,
}

// harness-picks.ts:65 at a3b870a6e
// export function modelPicks(model: string, contextWindow?: string): RanPicks {
//   const window = contextWindow ?? (ONE_M.test(model) ? "1m" : undefined);
//   return { model: model.replace(ONE_M, ""), ...(window !== undefined ? { contextWindow: window } : {}) };
// }
/// The model with a trailing `[1m]` taken off, and the window: the one given, else `1m` where that suffix was there.
pub fn model_picks(model: &str, context_window: Option<&str>) -> RanPicks {
    let window = context_window.map(String::from).or_else(|| model.ends_with(ONE_M).then(|| "1m".to_string()));
    RanPicks { model: Some(model.strip_suffix(ONE_M).unwrap_or(model).to_string()), context_window: window, ..RanPicks::default() }
}

/// `Object.assign(picks, named)`: each key the reader names overwrites the one held, and the rest stay.
fn assign_picks(into: &mut RanPicks, from: RanPicks) {
    if from.model.is_some() {
        into.model = from.model;
    }
    if from.context_window.is_some() {
        into.context_window = from.context_window;
    }
    if from.effort.is_some() {
        into.effort = from.effort;
    }
    if from.permission_mode.is_some() {
        into.permission_mode = from.permission_mode;
    }
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
    type Reader = fn(&SessionView) -> Option<RanPicks>;
    let readers: [Reader; 3] = [
        |row| row.model.as_deref().map(|m| model_picks(m, row.context_window.as_deref())),
        |row| row.effort.as_ref().map(|effort| RanPicks { effort: Some(effort.clone()), ..RanPicks::default() }),
        |row| row.permission_mode.as_ref().map(|mode| RanPicks { permission_mode: Some(mode.clone()), ..RanPicks::default() }),
    ];
    let mut picks = RanPicks::default();
    for read in readers {
        if let Some(named) = rows.iter().rev().find_map(read) {
            assign_picks(&mut picks, named);
        }
    }
    picks
}

/// What `kept_picks` answers: RanPicks less the access, keys in the order it writes them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptPicks {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub effort: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
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
    let Some(catalog) = catalog else {
        return KeptPicks {
            model: named.model.clone().or_else(|| ran.model.clone()),
            effort: named.effort.clone().or_else(|| ran.effort.clone()),
            context_window: named.context_window.clone().or_else(|| ran.context_window.clone()),
        };
    };
    let takes = |model: Option<&HarnessModel>| {
        let effort_ok = named.effort.is_none()
            || catalog.efforts.is_empty()
            || listed_pick(&efforts_for(catalog, model), named.effort.as_deref()).is_some();
        let window_ok =
            named.context_window.is_none() || listed_pick(&context_windows_for(catalog, model), named.context_window.as_deref()).is_some();
        effort_ok && window_ok
    };
    let own = model_of(catalog, listed_pick(&every_model(catalog), ran.model.as_deref()).as_deref());
    let chosen = if named.model.is_some() { model_of(catalog, named.model.as_deref()) } else { own.filter(|m| takes(Some(m))) };
    let model = chosen.as_ref();
    KeptPicks {
        model: model.map(|m| m.value.clone()),
        effort: named.effort.clone().or_else(|| listed_pick(&efforts_for(catalog, model), ran.effort.as_deref())),
        context_window: named
            .context_window
            .clone()
            .or_else(|| listed_pick(&context_windows_for(catalog, model), ran.context_window.as_deref())),
    }
}
