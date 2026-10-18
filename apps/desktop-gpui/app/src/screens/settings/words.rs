// SPDX-License-Identifier: AGPL-3.0-only
//! The words more than one Settings page says, each written once: the computer the host runs on and its name, a word
//! at the head of a slot, a stamp and a size. The figures the Usage page and a transcript already say are theirs and
//! are taken from there, so a page reads every one of these through here.

use chrono::{Local, TimeZone};
use wsp_host_client::protocol::PlaceView;

pub use crate::screens::usage::words::{fmt_tokens, list_words, offline_for, read_when};
pub use crate::words::fmt_duration;

/// `HERE_PLACE_ID`: the computer the host runs on.
pub const HERE: &str = "here";

pub fn is_here(place: &PlaceView) -> bool {
    place.id == HERE
}

/// `hereName`, the sidebar's one reading, off the typed rows.
pub fn here_name(places: &[PlaceView]) -> String {
    crate::screens::sidebar::menu::here_name(&crate::screens::sidebar::host::values(places))
}

/// `capitalised`: a word as it stands alone at the head of a slot.
pub fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        None => String::new(),
    }
}

/// An ISO stamp as epoch ms, as `new Date(at)` reads one; none for one it does not.
pub fn stamp_ms(at: &str) -> Option<i64> {
    chrono::DateTime::parse_from_rfc3339(at).ok().map(|t| t.timestamp_millis())
}

/// `builtWhen`: the clock alone with today or yesterday, the day and the clock before that, in local time; a stamp
/// that does not read as one reads as itself.
pub fn built_when(at: &str, now: i64) -> String {
    let Some(ms) = stamp_ms(at) else { return at.to_owned() };
    let (Some(when), Some(today)) = (Local.timestamp_millis_opt(ms).single(), Local.timestamp_millis_opt(now).single()) else {
        return at.to_owned();
    };
    let clock = when.format("%H:%M");
    match (today.date_naive() - when.date_naive()).num_days() {
        i64::MIN..=0 => format!("today {clock}"),
        1 => format!("yesterday {clock}"),
        _ => format!("{} {clock}", when.format("%b %-d")),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    /// What `tests/catalog.mjs` reads off the built catalog and protocol. Needs node and the packages built (pnpm build).
    pub(crate) fn catalog_read() -> serde_json::Value {
        let here = Path::new(env!("CARGO_MANIFEST_DIR"));
        let packages = here.join("../../../packages").canonicalize().unwrap();
        let built = |name: &str| {
            let module = packages.join(name).join("dist/index.js");
            assert!(module.exists(), "{} is not built: run pnpm install && pnpm build", module.display());
            format!("file://{}", module.display())
        };
        let out = Command::new("node")
            .arg(here.join("tests/catalog.mjs"))
            .args([built("catalog"), built("protocol")])
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .output()
            .expect("node on PATH");
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
        serde_json::from_slice(&out.stdout).unwrap()
    }

    /// Each figure against what the protocol's own formatter writes for it, read with node.
    #[test]
    fn the_figures_are_what_the_protocol_writes() {
        let read = catalog_read();
        let figures = &read["figures"];
        let each = |key: &str, ours: &dyn Fn(f64) -> String| {
            for pair in figures[key].as_array().unwrap() {
                let n = pair[0].as_f64().unwrap();
                assert_eq!(ours(n), pair[1].as_str().unwrap(), "{key}({n})");
            }
        };
        each("bytes", &|n| crate::ui::fmt_bytes(n as u64));
        each("tokens", &fmt_tokens);
        each("duration", &fmt_duration);
        each("offline", &offline_for);
        for pair in figures["capitalised"].as_array().unwrap() {
            assert_eq!(capitalised(pair[0].as_str().unwrap()), pair[1].as_str().unwrap());
        }
        for pair in figures["listWords"].as_array().unwrap() {
            let words: Vec<String> = serde_json::from_value(pair[0].clone()).unwrap();
            assert_eq!(list_words(&words), pair[1].as_str().unwrap());
        }
    }

    #[test]
    fn a_stamp_reads_by_the_day_it_fell_on() {
        let now = Local.with_ymd_and_hms(2026, 10, 5, 12, 0, 0).unwrap().timestamp_millis();
        let at = |d: u32, h: u32| Local.with_ymd_and_hms(2026, 10, d, h, 0, 0).unwrap().to_rfc3339();
        assert_eq!(built_when(&at(5, 8), now), "today 08:00");
        assert_eq!(built_when(&at(4, 8), now), "yesterday 08:00");
        assert_eq!(built_when(&at(3, 8), now), "Oct 3 08:00");
        assert_eq!(built_when("not a stamp", now), "not a stamp");
    }

    #[test]
    fn this_computer_goes_by_its_own_rows_name() {
        let row = |v: serde_json::Value| -> PlaceView { serde_json::from_value(v).unwrap() };
        let boat = row(serde_json::json!({ "id": "box", "kind": "provider", "name": "box", "default": false }));
        assert_eq!(here_name(std::slice::from_ref(&boat)), "", "none until the list holds this computer");
        let mac = row(serde_json::json!({ "id": "here", "kind": "computer", "name": "mac", "default": true }));
        assert_eq!(here_name(&[boat.clone(), mac.clone()]), "mac");
        let named = PlaceView { label: Some("Studio".into()), ..mac };
        assert_eq!(here_name(&[boat, named]), "Studio", "the name its owner gave it");
    }
}
