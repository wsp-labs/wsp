// SPDX-License-Identifier: AGPL-3.0-only
//! What a computer's row calls its kind, and the count of what a recipe put on it. Port of the functions of packages/protocol/src/views/place.ts that wsp-wire left to this crate, at
//! the cut (383708c78); that file's shapes are wsp-wire's. See spec/views_place.md.

use wsp_wire::types::place_word::HERE_PLACE_ID;
use wsp_wire::views::place::{PlaceProvisionKind, PROVISION_KIND_WORDS};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::PlaceView;

use crate::words::base::plural;
use crate::words::computer::this_computer;

/// What the computers table calls one row's kind, which is finer than PlaceKind by one: the computer the host runs
/// on is named as its owner names it, a computer somebody joined is a box, and a provider account is a cloud. The
/// one table, so the command line's KIND column and the app read one word per row.
// place.ts:140
// export function computerKindWord(place: Pick<PlaceView, "id" | "kind">, platform: "darwin" | "linux"): string {
//   if (place.id === HERE_PLACE_ID) return thisComputer(platform);
//   return place.kind === "provider" ? "cloud" : "box";
// }
pub fn computer_kind_word(place: &PlaceView, platform: &crate::words::computer::Platform) -> String {
    if place.id == HERE_PLACE_ID {
        return this_computer(*platform);
    }
    match place.kind {
        PlaceKind::Provider => "cloud".to_string(),
        PlaceKind::Computer => "box".to_string(),
    }
}

/// A count of rows by kind as a person reads it, the kinds with none left out: `9 tools, 3 files, 5 MCP servers`.
/// Nothing but tools reads as it did before there was anything else.
// place.ts:222
// export function provisionCountWord(counts: Partial<Record<PlaceProvisionKind, number>>): string {
//   const said = PlaceProvisionKind.options.flatMap(kind => ((counts[kind] ?? 0) > 0 ? [plural(counts[kind]!, PROVISION_KIND_WORDS[kind])] : []));
//   return said.length === 0 ? plural(0, PROVISION_KIND_WORDS.tool) : said.join(", ");
// }
pub fn provision_count_word(counts: &indexmap::IndexMap<PlaceProvisionKind, f64>) -> String {
    let said: Vec<String> = [PlaceProvisionKind::Tool, PlaceProvisionKind::File, PlaceProvisionKind::Server]
        .into_iter()
        .filter_map(|kind| {
            let count = counts.get(&kind).copied().unwrap_or(0.0);
            (count > 0.0).then(|| plural(count, provision_word(&kind)))
        })
        .collect();
    if said.is_empty() {
        plural(0.0, provision_word(&PlaceProvisionKind::Tool))
    } else {
        said.join(", ")
    }
}

/// The noun a kind's rows are counted as, read off the wire's table.
fn provision_word(kind: &PlaceProvisionKind) -> &'static str {
    let key = match kind {
        PlaceProvisionKind::Tool => "tool",
        PlaceProvisionKind::File => "file",
        PlaceProvisionKind::Server => "server",
    };
    PROVISION_KIND_WORDS.iter().find(|&&(k, _)| k == key).map_or(key, |&(_, word)| word)
}
