// SPDX-License-Identifier: AGPL-3.0-only
//! The builders that make a piece's props, items and entry.
use super::*;

pub(super) fn spec(prop_type: SlatePropType, binds: SlateBinds) -> SlatePropSpec {
    SlatePropSpec {
        prop_type,
        binds,
        required: false,
        default: None,
        min: None,
        max: None,
        unseen: None,
        about: None,
        literal: false,
        of_number: false,
        most: None,
        time: false,
        names_run: false,
        marks: false,
        head: false,
    }
}

pub(super) fn str_prop() -> SlatePropSpec {
    spec(SlatePropType::String, SlateBinds::Yes)
}

pub(super) fn num_prop() -> SlatePropSpec {
    spec(SlatePropType::Number, SlateBinds::Yes)
}

pub(super) fn flag() -> SlatePropSpec {
    spec(SlatePropType::Boolean, SlateBinds::No)
}

pub(super) fn enm(values: &'static [&'static str], fallback: Option<&str>) -> SlatePropSpec {
    SlatePropSpec { default: fallback.map(|f| json!(f)), ..spec(SlatePropType::Enum(values), SlateBinds::No) }
}

pub(super) fn tone(values: &'static [&'static str]) -> SlatePropSpec {
    spec(SlatePropType::Enum(values), SlateBinds::Yes)
}

pub(super) fn req(prop: SlatePropSpec) -> SlatePropSpec {
    SlatePropSpec { required: true, ..prop }
}

pub(super) fn row_key() -> SlatePropSpec {
    SlatePropSpec { unseen: Some("names a row so it keeps its place as rows move"), ..spec(SlatePropType::Any, SlateBinds::Item) }
}

pub(super) fn props<const N: usize>(entries: [(&'static str, SlatePropSpec); N]) -> IndexMap<&'static str, SlatePropSpec> {
    entries.into_iter().collect()
}

/// How a group sits: inner space and the app's inset ground.
pub(super) fn with_box(mut entries: IndexMap<&'static str, SlatePropSpec>) -> IndexMap<&'static str, SlatePropSpec> {
    entries.insert("pad", enm(PAD, Some("none")));
    entries.insert("surface", enm(SURFACE, Some("plain")));
    entries
}

pub(super) fn row_action_item() -> SlateItemSpec {
    SlateItemSpec {
        prop: "rowActions",
        fields: props([("label", SlatePropSpec { binds: SlateBinds::Item, required: true, ..str_prop() })]),
        row: true,
        row_when: true,
        events: Some(&["press"]),
        min: None,
        max: Some(SLATE_LIMITS.row_actions),
    }
}

pub(super) fn piece(
    kind: &'static str,
    purpose: &'static str,
    fallback: &'static str,
    example: &'static str,
    sketch: SlateSketchFn,
) -> SlatePieceModule {
    SlatePieceModule {
        kind,
        level: "core",
        purpose,
        holds_children: false,
        child_limit: None,
        text_prop: None,
        raw_text: false,
        repeating: false,
        interactive: false,
        needs_handler: None,
        props: IndexMap::new(),
        items: IndexMap::new(),
        events: &[],
        sketch,
        collapsed: None,
        check: None,
        fallback,
        example,
    }
}
