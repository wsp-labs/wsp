// SPDX-License-Identifier: AGPL-3.0-only
//! The marks on text that only means something on a cloud, and the two readings of them. Port of
//! `packages/host/src/cloud-text.ts` at 65f6b89c7; the spec is `spec/skill.md`.
use std::sync::LazyLock;

use regex::Regex;
use wsp_words::js::js_trim_start;

pub const CLOUD_MARK: &str = "<!-- cloud -->";
pub const CLOUD_SPAN_END: &str = "<!-- /cloud -->";
pub const BOAT_MARK: &str = "<!-- boat -->";
pub const BOAT_SPAN_END: &str = "<!-- /boat -->";
pub const NO_CLOUD_MARK: &str = "<!-- no cloud -->";
pub const NO_CLOUD_SPAN_END: &str = "<!-- /no cloud -->";

static BOAT_MARKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!-- /?boat -->").unwrap());
static CLOUD_SPANS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<!-- cloud -->([^\r\n\u{2028}\u{2029}]*?)<!-- /cloud -->").unwrap());
static NO_CLOUD_SPANS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"<!-- no cloud -->([^\r\n\u{2028}\u{2029}]*?)<!-- /no cloud -->").unwrap());

/// cloud-text.ts:22-27.
pub fn cloud_text(text: &str, cloud: bool) -> String {
    let no_boat = BOAT_MARKS.replace_all(text, "");
    let kept = CLOUD_SPANS.replace_all(&no_boat, |caps: &regex::Captures| if cloud { caps[1].to_string() } else { String::new() });
    NO_CLOUD_SPANS.replace_all(&kept, |caps: &regex::Captures| if cloud { String::new() } else { caps[1].to_string() }).into_owned()
}

/// The level of a markdown heading line: `^(#+) ` (cloud-text.ts:37).
fn heading_level(line: &str) -> Option<usize> {
    let level = line.bytes().take_while(|&b| b == b'#').count();
    (level > 0 && line.as_bytes().get(level) == Some(&b' ')).then_some(level)
}

/// cloud-text.ts:31-51.
pub fn skill_for(skill: &str, cloud: bool) -> String {
    let mut out: Vec<String> = Vec::new();
    let mut dropping: Option<usize> = None;
    let mut fenced = false;
    for line in skill.split('\n') {
        if js_trim_start(line).starts_with("```") {
            fenced = !fenced;
        }
        let heading = if fenced { None } else { heading_level(line) };
        if let Some(level) = dropping {
            match heading {
                Some(h) if h <= level => dropping = None,
                _ => continue,
            }
        }
        let marked = line.ends_with(&format!(" {CLOUD_MARK}"));
        if marked && !cloud {
            if let Some(h) = heading {
                dropping = Some(h);
            }
            continue;
        }
        let bare = if marked {
            &line[..line.len() - CLOUD_MARK.len() - 1]
        } else if line.ends_with(&format!(" {BOAT_MARK}")) {
            &line[..line.len() - BOAT_MARK.len() - 1]
        } else {
            line
        };
        out.push(cloud_text(bare, cloud));
    }
    out.join("\n")
}
