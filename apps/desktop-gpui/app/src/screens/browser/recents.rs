// SPDX-License-Identifier: AGPL-3.0-only
//! Recently opened places, one list per workspace (`browser/recents.ts`), and what the servers list shows of them and
//! of the listening ports for what is typed in the bar (`PreviewEmptyState.tsx`).

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::ports::KnownPort;
use super::url::loopback_url;

/// `BrowserHistoryEntry`, in the JSON shape today's app keeps.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub url: String,
    pub last_visited_at: i64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

pub const RECENTS_MAX: usize = 50;

/// How many recents the servers list shows.
pub const SHOWN_MAX: usize = 8;

pub fn recents_key(workspace: &str) -> String {
    format!("wsp:browser-recents:v1:{workspace}")
}

/// The codec's decode: an array of entries, dropping any that is not one; anything else is no entries.
pub fn decode(raw: &Value) -> Vec<Entry> {
    let Some(rows) = raw.as_array() else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| {
            let row = row.as_object()?;
            let title = match row.get("title") {
                None => None,
                Some(Value::String(title)) => Some(title.clone()),
                Some(_) => return None,
            };
            Some(Entry { url: row.get("url")?.as_str()?.to_owned(), last_visited_at: row.get("lastVisitedAt")?.as_f64()? as i64, title })
        })
        .collect()
}

/// `recordVisit`: the url moves to the front stamped `at`, keeping its title, and the list stops at [`RECENTS_MAX`].
pub fn record_visit(entries: &[Entry], url: &str, at: i64) -> Vec<Entry> {
    let title = entries.iter().find(|e| e.url == url).and_then(|e| e.title.clone());
    let entry = Entry { url: url.to_owned(), last_visited_at: at, title };
    std::iter::once(entry).chain(entries.iter().filter(|e| e.url != url).cloned()).take(RECENTS_MAX).collect()
}

pub fn remove_visit(entries: &[Entry], url: &str) -> Vec<Entry> {
    entries.iter().filter(|e| e.url != url).cloned().collect()
}

/// `relativeLabel`: "just now", "4m ago", "2h ago", "3d ago".
pub fn relative_label(at_ms: i64, now_ms: i64) -> String {
    let seconds = (now_ms - at_ms).div_euclid(1000);
    if seconds < 60 {
        return "just now".into();
    }
    let minutes = seconds / 60;
    if minutes < 60 {
        return format!("{minutes}m ago");
    }
    let hours = minutes / 60;
    if hours < 24 {
        return format!("{hours}h ago");
    }
    format!("{}d ago", hours / 24)
}

/// The recents the servers list shows: those whose url parses, the first eight, then those matching what is typed.
pub fn shown<'a>(entries: &'a [Entry], query: &str) -> Vec<&'a Entry> {
    let needle = needle(query);
    entries
        .iter()
        .filter(|e| url::Url::parse(&e.url).is_ok())
        .take(SHOWN_MAX)
        .filter(|e| hit(&needle, &[Some(&e.url), e.title.as_deref()]))
        .collect()
}

/// The listening ports the servers list shows for what is typed: matched on "localhost:<port>", the loopback url and
/// the process.
pub fn servers_shown<'a>(ports: &'a [KnownPort], query: &str) -> Vec<&'a KnownPort> {
    let needle = needle(query);
    ports
        .iter()
        .filter(|p| hit(&needle, &[Some(&format!("localhost:{}", p.port)), Some(&loopback_url(p.port, "/")), p.process.as_deref()]))
        .collect()
}

fn needle(query: &str) -> String {
    query.trim().to_lowercase()
}

fn hit(needle: &str, words: &[Option<&str>]) -> bool {
    needle.is_empty() || words.iter().flatten().any(|word| word.to_lowercase().contains(needle))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn entry(url: &str, at: i64) -> Entry {
        Entry { url: url.into(), last_visited_at: at, title: None }
    }

    #[test]
    fn the_key_is_per_workspace() {
        assert_eq!(recents_key("ws_1"), "wsp:browser-recents:v1:ws_1");
    }

    #[test]
    fn a_visit_moves_the_url_to_the_front_keeping_its_title() {
        let titled = Entry { title: Some("Home".into()), ..entry("http://localhost:3000", 1) };
        let entries = [entry("http://localhost:5173", 2), titled];
        let next = record_visit(&entries, "http://localhost:3000", 9);
        assert_eq!(
            next,
            [
                Entry { url: "http://localhost:3000".into(), last_visited_at: 9, title: Some("Home".into()) },
                entry("http://localhost:5173", 2)
            ]
        );
        assert_eq!(record_visit(&next, "https://github.com/", 10)[0], entry("https://github.com/", 10));
    }

    #[test]
    fn the_list_stops_at_fifty() {
        let entries: Vec<Entry> = (0..RECENTS_MAX).map(|i| entry(&format!("http://localhost:{}", 3000 + i), 0)).collect();
        let next = record_visit(&entries, "https://github.com/", 1);
        assert_eq!(next.len(), RECENTS_MAX);
        assert_eq!(next[0].url, "https://github.com/");
        assert_eq!(next.last().unwrap().url, "http://localhost:3048");
    }

    #[test]
    fn removing_a_visit_drops_only_that_url() {
        let entries = [entry("a", 1), entry("b", 2)];
        assert_eq!(remove_visit(&entries, "a"), [entry("b", 2)]);
    }

    #[test]
    fn the_label_reads_how_long_ago_in_the_largest_whole_unit() {
        let now = 10_000_000_000;
        for (ago, label) in [
            (0, "just now"),
            (59_999, "just now"),
            (60_000, "1m ago"),
            (3_599_999, "59m ago"),
            (7_200_000, "2h ago"),
            (3 * 86_400_000, "3d ago"),
        ] {
            assert_eq!(relative_label(now - ago, now), label, "{ago}");
        }
        assert_eq!(relative_label(now + 5_000, now), "just now");
    }

    #[test]
    fn decoding_keeps_entries_and_drops_anything_malformed() {
        let raw = json!([
            {"url": "http://localhost:3000", "lastVisitedAt": 5},
            {"url": "https://github.com/", "lastVisitedAt": 6, "title": "GitHub"},
            {"url": "x", "lastVisitedAt": "6"},
            {"url": 1, "lastVisitedAt": 6},
            {"url": "y", "lastVisitedAt": 6, "title": null},
            {"lastVisitedAt": 6},
            "z",
            null
        ]);
        assert_eq!(
            decode(&raw),
            [entry("http://localhost:3000", 5), Entry { title: Some("GitHub".into()), ..entry("https://github.com/", 6) }]
        );
        assert_eq!(decode(&json!({"url": "a"})), []);
    }

    #[test]
    fn an_entry_writes_the_json_shape_todays_app_keeps() {
        let titled = Entry { title: Some("Home".into()), ..entry("http://localhost:3000", 5) };
        assert_eq!(serde_json::to_value(&titled).unwrap(), json!({"url": "http://localhost:3000", "lastVisitedAt": 5, "title": "Home"}));
        assert_eq!(serde_json::to_value(entry("a", 1)).unwrap(), json!({"url": "a", "lastVisitedAt": 1}));
    }

    #[test]
    fn the_list_shows_the_first_eight_parsable_recents_filtered_by_what_is_typed() {
        let mut entries = vec![entry("not a url", 0)];
        entries.extend((0..10).map(|i| entry(&format!("http://localhost:{}", 3000 + i), 0)));
        entries[3].title = Some("Docs".into());
        let urls = |shown: Vec<&Entry>| shown.into_iter().map(|e| e.url.clone()).collect::<Vec<_>>();
        let all = urls(shown(&entries, ""));
        assert_eq!(all.len(), 8);
        assert_eq!((all[0].as_str(), all[7].as_str()), ("http://localhost:3000", "http://localhost:3007"));
        assert_eq!(urls(shown(&entries, " docs ")), ["http://localhost:3002"]);
        assert_eq!(urls(shown(&entries, "3009")), Vec::<String>::new());
    }

    #[test]
    fn servers_match_on_their_address_their_loopback_url_or_their_process() {
        let port = |port: u16, process: Option<&str>| KnownPort { port, pid: None, process: process.map(str::to_owned), loopback: None };
        let ports = [port(3000, Some("node")), port(5173, Some("Vite")), port(8000, None)];
        let shown = |query: &str| servers_shown(&ports, query).into_iter().map(|p| p.port).collect::<Vec<_>>();
        assert_eq!(shown(""), [3000, 5173, 8000]);
        assert_eq!(shown("vite"), [5173]);
        assert_eq!(shown("localhost:8000"), [8000]);
        assert_eq!(shown("http://localhost:30"), [3000]);
        assert_eq!(shown("python"), Vec::<u16>::new());
    }
}
