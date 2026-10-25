# wsp-words: what the spec notes follow

Each `<unit>.md` here describes one unit of the crate against the TypeScript it ports, `packages/protocol/src` of the wsp repository.

## The TS commit the crate matches

The crate was written from `a3b870a6e`. It now matches the dead-code cut, **383708c78** (on `refactor/2441-cut-archive`; that branch's head `65f6b89c7` holds the same `packages/protocol/src`, and its only later commit lays out the archive repository). The cut's fix round is still open, so a change the cut makes in its own round has to be read against this line again.

The `// file.ts:N` marker above each item in the sources is the line the item had at `a3b870a6e`; the copied TS beside it is the text at the cut. A marker is a pointer to read the TS by, not a promise that the line number still holds in the cut's file.

What the cut changed in `packages/protocol/src` and where the port stands:

| Part of the cut | State |
|---|---|
| The words the crate ports (`app-address`, `app-ports`, `projects`, `sub-thread`, `place-state`, `workspace-state`, `words/boat`, `words/computer`, `words/image`, `words/init`, `words/places`, `words/thread`, `words/tools`, `words/units`) | applied: the sentences, rules and signatures, and the items removed with their tests |
| `init-job.ts` deleted, `setup.ts` new | the shapes are wsp-wire's; this crate still imports `LoginState`, `LoginChoice`, `InitSetup`'s fields and the rest from `wsp_wire::types::init_job` and `wsp_wire::wire::capabilities` until the wire's drift pass moves them |
| `views/*`, `wire/*` (`WorkspaceAgents.spawn`, `PlaceSettingWord` `spawn` becoming `max-machines`, `PlaceView.engine`, `InitSetup.agents` and `job`, the twelve ops, `golden-image` and `snapshot-lineage` items) | wsp-wire's, left to its drift pass; the crate reads the wire as it stands and ignores the removed fields, and its tests keep the wire's still-required fields in their fixtures |

Items the cut removed were removed here too, unless a crate on `rust/2448-base` still called one (checked by name over `host/crates`, 25 Oct). None did: `PinRead` is the engine's own type, and no other crate reads `init`, `image`, `agents_line` or the rest of the list.

## Accepted differences

Each is a class a parity list names, with the case that pins it.

| Class | What differs | Pinned by |
|---|---|---|
| `idna-version` | the `url` crate's international-domain table is not node 22.23.4's | `tests/idna_version.rs` |
| `libm-pow` | `contrast_ratio` uses `powf` where V8 uses its own `pow`; 41 of 1,129 colour pairs differ in the 17th digit | none |
| `posix-tz` | a POSIX `TZ` string such as `IST-5:30` is honoured by jiff and read as UTC by node's ICU; the host's zones are IANA names | none |
| `json-depth` | a container nested past 128 levels is read and kept as an empty one; a number past the double range reads as a half (for zod) or null (for a tool call's text) | `tests/v8_json.rs` |
| `invalid-date-words` | where the TS throws RangeError for an instant no `Date` holds and the Rust function has no error to answer with, it says `Invalid Date`, `NaN` or `Invalid ` | `tests/js_date.rs`, `tests/review_agents.rs` |
| `key-order` | the order of the keys of `PlaceSettings` after a partial drop of the machines setting (the TS writes `spawn` last, the struct writes its own order); equal as a value | `tests/cut_drift.rs` |
| `lone-surrogate` | text a backspace cuts in half a character above U+FFFF (`terminal_text`) is written without the lone half JavaScript holds (rule 8.5) | none |
| `workspace-facts` | `workspace_computer_name` takes a `WorkspaceView`, which carries no machine facts; `workspace_computer_name_with` takes them beside it | `tests/review_places.rs` |

## What the review's oracle is

The expected values in the tests of the fix round were printed by node 22.23.4 running the TS at the cut as one esbuild bundle of `packages/protocol/src`; the generators that wrote the tables are not kept in the repository.
