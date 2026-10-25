# preferences

Source: the functions of `packages/protocol/src/views/preferences.ts` (359 lines: the lines :65-74, `labsFromEnv`
:174, `cloudFromEnv` :177, `DEFAULT_PREFERENCES` :206, `preferencesFrom` :209-214, `applyPreferencesPatch` :218-282,
`serverIconsLeftLine` :285), `patchedGeneral` and `GENERAL_DEFAULTS` of `general-prefs.ts` (:59-91), `patchedFields`
of `thread-defaults.ts` (:48-55), at a3b870a6e. Tests: `preferences.test.ts` (31 cases; 29 here, :193 is the wire
writer's, :284 `notifyBy` the words writer's), `thread-defaults.test.ts` :230 and :241, `broker-contract.test.ts` :96,
:104, :113, :122, :129, all in `tests/views_preferences.rs` (33); hard cases in `tests/views_preferences_rules.rs` (8,
compared as exact JSON text). Rust: `src/views/preferences.rs`, `src/types/general_prefs.rs`,
`src/types/thread_defaults.rs`.

## What it does

The one preferences record on the host's state: its defaults, a stored record read over them, and the one merge rule
a patch lands by, which the host and a painting client share. What the host stores and sends is exactly what
`apply_preferences_patch` writes, keys in its order.

## Behaviour

1. `default_preferences()` is, in this order: theme `system`, lightTheme `paper`, darkTheme `graphite`, sidebarMode
   `list`, terminalSize `app`, terminalZoom, access, projectLook, projectIcon, computerLook `{}`, serverIcons,
   agentVersions, usageLogs, productUsage, keepAwake, transparency `true`, projectOrder `[]`, keybindings `{}`, appFont
   and codeFont `""`, agentDefaults and projectDefaults `{}`, the nine GENERAL_DEFAULTS (enter, queue, notify-sound,
   notify, true, 2h, true, ask, current), labs `false` (:206).
2. `preferences_from(stored)`: None or JSON null reads as `{}` (:210); a value that does not parse as
   `PreferencesStored` (a bad enum, a string, an array, a broken strict key) gives `default_preferences()` whole (:211).
   Otherwise the stored record less `projectIcon` lands as a patch over the defaults, and `projectIcon` is set to the
   stored one or `{}` (:212-213). The stored `labs` never lands (rule 3). Unknown top-level keys are dropped.
3. `apply_preferences_patch(current, patch)` (:218-282) writes, in this order: theme, lightTheme, darkTheme,
   sidebarMode, terminalSize (each the patch's else the current), terminalZoom, access, projectLook (per-key maps),
   projectIcon (always the current), computerLook (per-key), serverIcons, agentVersions, usageLogs, productUsage,
   keepAwake, transparency, projectOrder (the patch's whole list else the current), keybindings (per-key), appFont,
   codeFont, agentDefaults, projectDefaults (per-id field patches), the nine general choices (`patched_general`), labs
   (always the current); then, only where they end up set: sidebarWidth, target, editor, updateDismissed,
   cloudCardDismissed, textSize, codeSize, defaultAgent, recipeLook, threadsShut.
4. Per-key maps (`perWorkspace`, :220): a copy of the current map; each patch entry in order: null removes the key, a
   value sets it (an existing key keeps its place; a new one, or one removed and set again, goes last).
5. sidebarWidth, target, textSize, codeSize, defaultAgent: an absent patch key keeps the current value; null clears
   it. editor, updateDismissed, cloudCardDismissed: the patch's value else the current (they cannot be cleared).
   recipeLook and threadsShut: absent keeps the current (which may be absent); present applies rule 4 over the current
   or `{}`.
6. agentDefaults and projectDefaults (`fieldsById`, :236): for each id in patch order, null removes it; a field patch
   lands through `Fields::patched` and an id left with no field is removed.
7. `patched_fields(kept, patch)` (thread-defaults.ts:48): the record's keys; each patch key null removes, absent keeps,
   a value sets, a new key after the kept ones; None when no key is left. `Fields::patched` is the same on a
   `Fields<T>`, which writes its keys in that order where a struct would write its own. `patched_general`: each choice the patch names, else the current.
8. `labs_from_env` / `cloud_from_env`: the variable is exactly `"1"` (no trim, no `true`).

## Edge cases

- `preferences_from(Some(json!("corrupt")))`, `Some(json!([]))` and `{"sidebarMode":"nope"}` are the defaults whole.
- A stored `labs: true` reads as `labs: false`; a stored `projectIcon` survives; `{"theme":"dark","unknownKey":1}`
  keeps the theme and drops the key.
- A patch `{ terminalZoom: { a: null } }` then `{ terminalZoom: { a: 4 } }` leaves `a` after `b`.

## Behaviour nobody wrote down

- What survives a host restart: the record as stored is read back through `preferences_from`, so a stored key the
  record no longer has is dropped on the next write, and a record that fails to parse is replaced by the defaults on
  the next write.
- Rule 4.26: a per-key map whose keys read as array indexes (`"2"`, `"10"`) is ordered with those first by JavaScript;
  the port keeps insertion order and applies the wire writer's `js_key_order` where it writes such a map.

## Decisions

1. `PreferencesStored` (the generator's name for `Preferences.partial()`) is converted to a `PreferencesPatch` for rule
   2: every value the stored record has becomes a set, never a null.

## Rules pinned → tests (`tests/views_preferences_rules.rs` unless named)

| Rule | Test |
|---|---|
| 1 | `the_defaults_are_the_ts_record_key_for_key_in_its_order`; `views_preferences.rs` :8, :288 |
| 2 | `a_stored_record_reads_over_the_defaults_and_absent_null_or_corrupt_reads_as_them` (9 stored records) |
| 3, 5, 6 | `a_patch_lands_field_by_field_in_the_order_apply_preferences_patch_writes_them` (11 patches); `views_preferences.rs` (29 TS cases) |
| 4 | `a_zoom_a_null_took_away_and_a_later_patch_sets_again_goes_after_the_others` |
| 7 | `a_record_patched_field_by_field_drops_a_null_keeps_an_absent_and_is_none_once_empty`, `the_general_choices_start_at_their_defaults_and_a_patch_lands_each_it_names` |
| 8 | `labs_and_the_cloud_are_on_only_for_exactly_1_in_the_host_s_environment` |
| lines | `each_line_reads_word_for_word_as_the_ts_writes_it` |
