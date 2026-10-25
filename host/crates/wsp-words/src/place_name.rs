// SPDX-License-Identifier: AGPL-3.0-only
//! The name a person reads a computer by, and which computer a workspace runs on. Port of
//! packages/protocol/src/place-name.ts at a3b870a6e. See spec/place_word.md.

use crate::exit::{refusal, HostError, Kind};
use crate::place_state::workspace_place_id;
use crate::place_word::{place_said, provider_key_name, PlaceRef};
use crate::workspace_state::{is_local_workspace, where_word, workspace_kind, WhereRecord};
use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::place::PlaceView;
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::workspace::WorkspaceView;
use wsp_wire::views::workspace_kind::WorkspaceKind;

/// Whether this row is the computer the host runs on: the one predicate, read by id and never by position.
// place-name.ts:13
// export const isHere = (place: PlaceView | undefined): boolean => place?.id === HERE_PLACE_ID;
pub fn is_here(place: Option<&PlaceView>) -> bool {
    place.is_some_and(|p| p.id == HERE_PLACE_ID)
}

/// Whether this row is the provider this host forks on rather than a computer somebody owns.
// place-name.ts:16
// export const isProviderPlace = (place: Pick<PlaceView, "kind">): boolean => place.kind === "provider";
pub fn is_provider_place(place: &PlaceView) -> bool {
    place.kind == PlaceKind::Provider
}

/// Whether the host keeps a workspace's credentials and adds them to each request on the way out, rather than its
/// machine holding them: a fork at a provider, the host's own included, which is a fork whose record names no place.
/// This computer and a computer the person joined hold their own, and a fork on a joined computer is a cloud record
/// too, so `place` is the row its record's place names. The one reading every credential road asks.
// place-name.ts:22
// export const isBrokered = (record: Pick<WorkspaceView, "kind" | "place">, place?: Pick<PlaceView, "kind">): boolean =>
//   workspaceKind(record) === "cloud" && (record.place === undefined || (place !== undefined && isProviderPlace(place)));
pub fn is_brokered(record: &WorkspaceView, place: Option<&PlaceView>) -> bool {
    workspace_kind(record.kind.as_ref()) == WorkspaceKind::Cloud && (record.place.is_none() || place.is_some_and(is_provider_place))
}

/// An agent other than Claude Code and Codex asked to launch where the host keeps the credentials: the broker adds no
/// key of its own, and none may sit on the machine.
// place-name.ts:27
// export const brokeredAgentRefusal = (agent: string, roads: readonly string[]): Error => refusal(`${agent} does not run on a cloud machine yet: wsp keeps no key there`, `Use ${roads.join(" or ")}.`, "usage");
/// `refusal(..., "usage")`.
pub fn brokered_agent_refusal(agent: &str, roads: &[impl AsRef<str>]) -> HostError {
    let roads: Vec<&str> = roads.iter().map(AsRef::as_ref).collect();
    refusal(
        &format!("{agent} does not run on a cloud machine yet: wsp keeps no key there"),
        &format!("Use {}.", roads.join(" or ")),
        Some(Kind::Usage),
    )
}

/// Codex asked to launch where the host keeps the credentials while the host's broker named no Codex login it holds:
/// the read failed, ran late, or found none. Codex there runs on the person's own login and never signs in itself.
// place-name.ts:31
// export const codexLoginUnreadRefusal = (): Error => refusal("Codex on a cloud machine runs on the Codex login on your computer, and wsp read none just now", "Check that Codex is signed in on your computer (codex login status), then try again.", "auth");
/// `refusal(..., "auth")`.
pub fn codex_login_unread_refusal() -> HostError {
    refusal(
        "Codex on a cloud machine runs on the Codex login on your computer, and wsp read none just now",
        "Check that Codex is signed in on your computer (codex login status), then try again.",
        Some(Kind::Auth),
    )
}

/// What a table calls a computer: the name a person types for it, and a provider by the name the app gives it, never
/// the id stored state holds; either one names it after --on.
// place-name.ts:35
// export const tableName = (place: { kind?: string; name: string }): string => placeSaid(place);
pub fn table_name(place: &PlaceRef) -> String {
    place_said(place)
}

/// What a person reads a row as: the name its owner gave the computer where it keeps one, a Mac's own "zingzy's
/// MacBook Pro"; a provider carries the name its own row in the provider table gives it, since the host words it by
/// the id WSP_PROVIDER holds and nobody types that; every other computer carries the name it reported.
// place-name.ts:40
// export function placeName(place: PlaceView): string {
//   if (place.label !== undefined) return place.label;
//   if (!isProviderPlace(place)) return place.name;
//   return providerKeyName(place.name);
// }
pub fn place_name(place: &PlaceView) -> String {
    if let Some(label) = &place.label {
        return label.clone();
    }
    if !is_provider_place(place) {
        return place.name.clone();
    }
    provider_key_name(&place.name)
}

/// The computer the host runs on, by its own name; empty until the places list holds that row.
// place-name.ts:47
// export function hereName(places: readonly PlaceView[]): string {
//   const here = places.find(isHere);
//   return here === undefined ? "" : placeName(here);
// }
pub fn here_name(places: &[PlaceView]) -> String {
    places.iter().find(|p| is_here(Some(p))).map(place_name).unwrap_or_default()
}

/// Which row a workspace stands on, or nothing for one this list cannot place.
// place-name.ts:53
// export function placeOf(places: readonly PlaceView[], workspace: Pick<WorkspaceView, "kind" | "machineId" | "place" | "provider">): PlaceView | undefined {
//   const at = workspacePlaceId(workspace, places);
//   return at === undefined ? undefined : places.find(p => p.id === at);
// }
pub fn place_of<'a>(places: &'a [PlaceView], workspace: &WorkspaceView) -> Option<&'a PlaceView> {
    let at = workspace_place_id(workspace, places)?;
    places.iter().find(|p| p.id == at)
}

/// The computer or the provider a workspace runs on, by the name its own row in the places list carries, this
/// computer's included; one no row holds is named by the protocol's one word for where it is.
// place-name.ts:60
// export function workspaceComputerName(places: readonly PlaceView[], workspace: Parameters<typeof whereWord>[0] & Pick<WorkspaceView, "kind" | "machineId" | "place" | "provider">): string {
//   const at = placeOf(places, workspace);
//   if (at !== undefined) return placeName(at);
//   if (isLocalWorkspace(workspace)) return hereName(places);
//   // A provider's rows arrive a moment after a host restarts: until then it goes by the name its words row gives it,
//   // never by the id its record holds.
//   return whereWord(workspace, workspace.provider === undefined ? undefined : providerKeyName(workspace.provider));
// }
pub fn workspace_computer_name(places: &[PlaceView], workspace: &WorkspaceView) -> String {
    if let Some(at) = place_of(places, workspace) {
        return place_name(at);
    }
    if is_local_workspace(workspace.kind.as_ref()) {
        return here_name(places);
    }
    let record =
        WhereRecord { machine_id: workspace.machine_id.clone(), kind: workspace.kind, provider: workspace.provider.clone(), facts: None };
    where_word(&record, workspace.provider.as_deref().map(provider_key_name).as_deref())
}
