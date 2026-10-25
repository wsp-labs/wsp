// SPDX-License-Identifier: AGPL-3.0-only
//! The sizes the web app's Settings pieces write, read from its own source at test time, so a change there turns these
//! tests red rather than leaving a copy here to drift.
// Each test crate that includes this reads only some of the files.
#![allow(dead_code)]

pub const LAYOUT: &str = include_str!("../../../../web/src/settings/layout.ts");
pub const ROWS: &str = include_str!("../../../../web/src/settings/rows.tsx");
pub const PICK_LISTS: &str = include_str!("../../../../web/src/settings/add/PickLists.tsx");
pub const PICK_ROW: &str = include_str!("../../../../web/src/settings/add/PickRow.tsx");
pub const SHEET_PARTS: &str = include_str!("../../../../web/src/settings/sheetParts.tsx");
pub const STEP_DIALOG: &str = include_str!("../../../../web/src/settings/add/StepDialog.tsx");
pub const DIALOG_STYLES: &str = include_str!("../../../../web/src/components/ui/dialog-styles.ts");
pub const KBD: &str = include_str!("../../../../web/src/components/ui/kbd.tsx");
pub const BUTTON: &str = include_str!("../../../../web/src/components/ui/button.tsx");
pub const INDEX_CSS: &str = include_str!("../../../../web/src/index.css");

/// The classes one `export const NAME = "..."` holds.
pub fn constant(src: &'static str, name: &str) -> &'static str {
    let start = src.find(&format!("const {name} =")).unwrap_or_else(|| panic!("no const {name}"));
    let rest = &src[start..];
    let open = rest.find(['"', '`']).unwrap();
    let quote = rest.as_bytes()[open] as char;
    let body = &rest[open + 1..];
    &body[..body.find(quote).unwrap()]
}

/// The quoted class string that starts with `prefix`, wherever it stands in the file.
pub fn literal(src: &'static str, prefix: &str) -> &'static str {
    let start = src.find(prefix).unwrap_or_else(|| panic!("no class string starting {prefix:?}"));
    let body = &src[start..];
    &body[..body.find(['"', '`']).unwrap()]
}

/// `--radius` from index.css, in px: Tailwind's `rounded-md` and the rest are steps off it.
fn radius() -> f32 {
    let line = INDEX_CSS.lines().find(|l| l.trim_start().starts_with("--radius:")).expect("--radius in index.css");
    let rem: f32 = line.split(':').nth(1).unwrap().trim().trim_end_matches(';').trim_end_matches("rem").parse().unwrap();
    rem * 16.
}

/// A Tailwind value in px: `[13px]`, `(--var,20px)`, a spacing step, or a named size.
fn value(key: &str, v: &str) -> Option<f32> {
    if let Some(inner) = v.strip_prefix('[').and_then(|v| v.strip_suffix(']')) {
        return inner.strip_suffix("px")?.parse().ok();
    }
    if let Some(inner) = v.strip_prefix('(').and_then(|v| v.strip_suffix(')')) {
        return inner.split(',').nth(1)?.strip_suffix("px")?.parse().ok();
    }
    match key {
        "text" => match v {
            "xs" => Some(12.),
            "sm" => Some(14.),
            "base" => Some(16.),
            _ => None,
        },
        "rounded" => match v {
            "sm" => Some(radius() - 4.),
            "md" => Some(radius() - 2.),
            "lg" => Some(radius()),
            "xl" => Some(radius() + 4.),
            _ => None,
        },
        _ => v.parse::<f32>().ok().map(|n| n * 4.),
    }
}

/// The px a class list gives `key` (`min-h`, `h`, `w`, `size`, `px`, `py`, `rounded`, `text`, `leading`, `gap`,
/// `max-w`) at the widest breakpoint, ignoring variants such as `max-sm:` and `group-data-[locked]/settings:`.
pub fn px(classes: &str, key: &str) -> f32 {
    classes
        .split_whitespace()
        .filter(|c| !c.contains(':'))
        .filter_map(|c| c.strip_prefix(key)?.strip_prefix('-').and_then(|v| value(key, v)))
        .next()
        .unwrap_or_else(|| panic!("no {key}-* size in {classes:?}"))
}
