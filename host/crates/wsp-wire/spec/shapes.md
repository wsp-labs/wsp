# shapes

Source: every exported zod schema of `packages/protocol/src/views/*.ts` (19 files) and of the top-level
`packages/protocol/src/*.ts` (all but `format.ts`, `exit.ts`, `index.ts`, `adapter-port.ts`, `bundles.mjs`,
`semver.mjs`), at a3b870a6e: 476 schemas, written as 669 Rust types (plus their nested anonymous ones) into
`src/views/<file>.rs` and `src/types/<file>.rs` by a generator that walks the live zod objects. Tests: `tests/shapes.rs`,
475 tests (one per exported schema plus `PreferencesStored`), 2,786 fixture cases zod answered (1,501 written back, 1,285 refused)
(`tests/fixtures/shapes/*.json`). The TS has no test of its own per schema; `tests/views_protocol.rs` ports the
protocol suite's 45 cases on these shapes.

## What it does

The views the host sends and the records it keeps, as serde types. The implementer writes only the `check` bodies
(each doc comment lists its rules) and the helpers of spec/check.md; the shapes themselves are the interface.

## Behaviour

1. Field order is the zod object's key order, which is the order a zod parse writes (rule 4.26). Exception, where the
   TS code writes another order and the host sends that: `Preferences` follows `applyPreferencesPatch`
   (views/preferences.ts:218); its fixtures compare as values.
2. `.optional()` is `Option` + `skip_serializing_if` (absent stays absent); `.nullable()` on a required key is `Option`
   with `deserialize_with = nullable` (null is written, absence refused); both is `Option<Option<T>>` with
   `double_option`; `.nullable()` inside a record value, array item or union is `Option<T>`. `.default(v)` is
   `#[serde(default = ...)]` from the TS default's JSON (32 fields), unless an `.optional()` wraps it, which zod answers
   before the default: then the key stays absent (`PreferencesStored`, views/preferences.ts:209).
3. `z.number()` is `JsNumber`; `.int()` is `i64`; `z.record` is `IndexMap` (input order kept); `z.unknown()` is
   `serde_json::Value`; `z.enum` is a Rust enum with each word as its serde name; `z.literal` a kept key's unit type.
4. `.strict()` objects (16) carry `deny_unknown_fields`; every other object drops unknown keys, as zod strips them.
5. A `discriminatedUnion` whose members each name one literal tag (5: `SessionEvent`, `PlanAlert`, `RecipeSource`,
   `ProjectSource`, `McpRowTransport`) is `#[serde(tag = ...)]`: its member structs leave the tag key out and carry
   `pub const TYPE` (or `KIND`). Every `*Event` struct whose first key is a `type` literal does the same; the wire
   writer's `EventUnion` tags them. Other unions (8) are `untagged`, variants in zod's order: the first that reads wins.
6. A zod schema is re-exported from wsp-frames only where everything matches zod: the keys and their order, each field's
   type (an integer where zod has `.int()`, a `JsNumber` where zod has `z.number()`, unsigned only where zod says so), every
   check (a `>= 0` rule must not be a serde refusal), the defaults and the nullable keys, and the nested schemas. Otherwise
   the type is a local struct with `JsNumber` fields and zod's rules, and wsp-frames is left alone. Re-exported today, after
   the audit: the unit enums (`PullRequestState`, `Mergeable`, `ReviewState`, `CheckState`, `MergeMethod`, `ReactionContent`,
   `CopyWord`, `MachineKind`, `MachineState`, `WorkspaceKind`, `Carried`) and the structs `GitPrCheckoutReply` (one key),
   `PullRequestLabel`, `CarryModule`. Local, because wsp-frames' derived `Deserialize` answers the first key the input holds
   that fails and zod answers the first key of the schema, and lists every issue (`src/types/frames_replies.rs`):
   `GitStartOnReply`, `GitPrDiffReply`, `GitPrReviewReply`, `PullRequestReviewRequest`, `GitRunLogReply`, `GitPrMergeReply`,
   `GitRepoReadReply`, `GitPrResolveReply`. Local, because they differ: `GitPushReply`, `GitPrReply`, `IssueRead`,
   `GitIssueReadReply`, `GitBranchCompareReply`, `PullRequestReaction`, `PullRequestCommit`, `GitPrReadReply`, `GitPrReactReply`,
   `GitPrReplyReply`, `UsageTokens`, `HostFolder`, `HostFolderListing`, `WorkspaceCopy`, and `RelayPort` (wire/limits.rs).
7. Hand rules beside the generated ones: `WorkspaceCopy.from/at`, `MachineShare.source/target`,
   `MachineBind.source/target` refine with `crate::wire::limits::is_plain_path` (custom, "an absolute path on the
   computer" / "an absolute path inside the workspace", views/place.ts:180-206); `textSize` and `codeSize` refine to
   `TEXT_SIZES` / `CODE_SIZES` (custom, "one of 13, 14, 15, 16, 17" / "one of 11, 12, 13, 14, 15", :86);
   `ReviewBlock.verdict` is read after lower-casing a string and turning spaces and dashes into `_` (start.ts:196),
   which its `Deserialize` does (a custom `deserialize_with` the implementer adds in start.rs's hand-written part).
8. Shapes named by `views/` but owned by the wire writer come from `crate::wire` (`PortForward` from `wire::limits`,
   `MachineSizeOffer` from `wire::capabilities`, `PermissionOption` and `PermissionEffect` from `wire::helpers`, `RelayPort`
   from `wire::limits`); the slate shapes (`SessionSlateEvent`, `SlateCause`, `SlateBy`) from `crate::types::slate_wire`.

## Edge cases

- The "nulls" fixture of each schema sets every nullable key null; the "minimal" one leaves every optional and
  defaulted key out (a default must then appear in the output); a "missing <key>" case removes the first required key
  and every required nullable one.
- Each rule mutation (one past a cap, one under a minimum, a fraction for an int, a word off an enum, an unknown key on
  a strict object, a refine's failure) must be refused with zod's code and path.

## Behaviour nobody wrote down

- Struct order is the schema's, but a few TS writers build objects in another order, and each is written as the TS
  builds it: `recipePins` (views/golden-image.ts:355) writes `id` first, so `recipe_pins` returns `RecipePin`, not
  `SealedPin`, which declares `id` after the pin's keys; `effortsFor` (harness-picks.ts:22) moves `isDefault` to an
  option's last key, and `patchedFields` (thread-defaults.ts:48) keeps the kept keys first and puts a new key last.
  A struct cannot hold either, so `efforts_for` returns `Fields<HarnessOption>` and the preferences' `agentDefaults` and
  `projectDefaults` hold `Fields<AgentDefaults>` and `Fields<ProjectOverrides>` (`types/thread_defaults.rs`), each
  keeping the order of its keys.

## Decisions

1. Where construction order and schema order differ, the order is the TS builder's (see the behaviour above), not the
   schema's, for `Preferences`, `recipe_pins` and `efforts_for`.
2. Web-only exports (no file under `packages/*/src` uses them) are not ported: `appProcLine`, `bootLineOf`,
   `placeAddSheetWord`, `needsYouCount`, `withPushedRow`, `creationAwaits`, the harness screen-command helpers, the
   desktop bridge's Electron interfaces.

## Rules pinned → tests

| Rule | Test |
|---|---|
| 1-6 per schema | `tests/shapes.rs::<file>::<schema>_reads_and_writes_as_zod_does` (475) |
| 7 refines | `tests/types_check.rs::the_first_issue::a_failed_regex_is_invalid_string_and_a_failed_refine_is_custom_with_the_refine_s_words`, the `WorkspaceCopy`, `MachineShare`, `MachineBind`, `Preferences*` fixtures |
| 1 Preferences order | `tests/views_preferences_rules.rs::a_patch_lands_field_by_field_in_the_order_apply_preferences_patch_writes_them` |
