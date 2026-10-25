# runtime_ops
Source: packages/protocol/src/wire/runtime.ts lines 1-1029 (the RuntimeOp union, RuntimeRequest and RUNTIME_OPS) at a3b870a6e. The union is 211 ops of field lists, so the unit is large in lines and small in rules; its Rust was generated from the zod schemas by script and finished by hand. Tests: protocol.test.ts (29 cases), agents-report.test.ts (7), port-forwards, preferences, init-job, broker-contract (1 each), thread-defaults (2); 3 more pinned here against what the TS module printed. Rust: `src/wire/runtime_ops.rs` (RuntimeOp, RuntimeRequest, parse_runtime_request, RuntimeOp::name/id; re-exported from `wire::runtime`), its variants' inline shapes in `src/wire/runtime.rs`; tests `tests/runtime_ops.rs` (45).

## What it does
Every frame a client sends the host: the 211 ops of the op switch with their params, read before any handler runs. A frame that does not read is refused with zod's issue list (chapter 05, Proposal 9) and never reaches a handler. `origin` rides the envelope, beside the op.

## Behaviour
1. `parse_runtime_request(frame)` (runtime.ts:51-1025), in this order: a non-object is `invalid_type` at [] ("Expected object, received <type>"); an absent or unknown op is ONE `invalid_union_discriminator` at ["op"] with `detail.options` = `RUNTIME_OPS` (211 words in the union's order, `auth` first, `slates.image` last) and message `Invalid discriminator value. Expected 'auth' | ... | 'slates.image'`, and nothing else is read (an origin also wrong is not reported first); a known op reads its fields in the TS object's order: an absent id is `invalid_union` at ["id"] message `Invalid input`, a wrong field its own issue at its path; then `origin`, when present one of here, relayed, paired (`invalid_enum_value` at ["origin"], message `Invalid enum value. Expected 'here' | 'relayed' | 'paired', received 'x'`).
2. Unknown keys of the frame and of every params object are dropped (rule 4.14), except under the strict schemas the variant embeds (the views writer's `PreferencesPatch`, `AgentsTarget`, and `AgentSetupSet` in the top-level types), which answer `unrecognized_keys` at the object's path.
3. Five ops are members defined in place_link (`seal.open`, `device.auth`, `place.join`, `place.auth`, `place.prove`): newtype variants holding structs without an `op` field. The fourteen `slates.*` ops take their params from slate/wire.ts's SLATE_OPS (`extend(SLATE_OPS[name].shape)`), in that order.
4. `places.set` is its own fields merged with the views' `PlaceSettingsAsk` (zod `.merge`), flattened in that order. `places.estimate.choices` is the top-level RecipeFile (zod `z.lazy`).
5. The caps each variant carries are written as comments at the foot of runtime.rs (`// - RuntimeOp::<Variant>.<field>: <rule>`), generated from the zod checks: every `max`, `min`, `int`, `positive`, `nonnegative`, `regex`, `url` and `refine` there is an issue at that field with zod's code (too_big, too_small, invalid_type, invalid_string, custom).
6. `RuntimeOp::name()` is the op word the variant reads and writes; `RuntimeOp::id()` the id it carried. A written frame puts `op` first, then the variant's fields in the TS order, then `origin`.
7. `RUNTIME_OPS` is the union's options in order; `THREAD_OPS` (50) and `DEVICE_OPS` (87) are copied lists, each a subset of `RUNTIME_OPS` (runtime.ts:1056-1242). The door reads them by op name before the schema (rule 4.17), so they are consulted on the raw frame's `op`, never on a parsed request.

## Edge cases
- `{"id": 1, "op": "workspaces.list", "junk": 1}` reads, and writes back without `junk`.
- A fraction id and a string id both read and write back as sent (rule 4.13).
- `{"op": "nope", "origin": "x"}` answers the discriminator issue first.

## Behaviour nobody wrote down
- Rule 4.14 above; rule 4.15: `origin` on the frame is honoured only on the host-token socket, which is the door's rule, not this reader's: the reader keeps what was sent.
- Rule 4.26: the params' maps (env, envs, headers, rows) are IndexMaps in the order sent.

## Tapes
Every tape in docs/rust-host-spec reads through this reader; chapter 04's door tapes and chapter 05's schema-refusal tapes (`sessions.rename` missing sessionId, `preferences.set` with a bad theme, `auth` with a number token) pin its refusals. Held out: none counted here.

## Decisions
1. RuntimeOp and DaemonRequest carry `#[serde_with::skip_serializing_none]`: an absent optional field is written absent; the one nullable field a variant must write as null (`projects.icon`'s `png`) is `#[serialize_always]`. A double_option field keeps its own attributes.
2. `RuntimeRequest` is `{ #[serde(flatten)] op: RuntimeOp, origin }`; reading goes through `parse_runtime_request`, which dispatches on `op` by hand so the first issue is zod's, never serde's own enum error.
3. The views and top-level types a variant holds are the views writer's (`crate::views`, `crate::types`).
4. The slate shapes the slates.* variants name are `crate::types::slate_wire::*`, in wsp-wire, since Proposal 1 puts protocol/src/slate in wsp-slate, which depends on wsp-wire.

## After the port
- The union has no hello or version exchange (rule 4.11); a client of another version learns of an unknown op only from the discriminator refusal.
