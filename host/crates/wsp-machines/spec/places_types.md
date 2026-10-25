# places/types: the record, the roads, the door's interface, its lines and its error marks

Port of `packages/runtime/src/places/types.ts` (808 lines) at `65f6b89c7`, into `src/places/types.rs`. Tests:
`tests/places_types.rs`, expected values from `tests/fixtures/oracle/places_b.json` (written by
`oracle/places_b.oracle.ts`). `file:line` is `types.ts` unless named.

About 700 of the 808 lines are interfaces with their contracts in doc comments; they become the Rust traits and
structs of `types.rs`, each naming its TS line. What carries behaviour is below.

## Interface rules

- **T1 One thread.** Every trait is `#[async_trait(?Send)]` and every road is `Rc<dyn ..>` (spec 03 rule 3.1). Engine
  traits stay `Arc<dyn MachineBackend>` as wsp-engine defines them.
- **T2 Optional roads.** A TS optional member of `PlaceWiring` (:157-208) is an `Option<Rc<dyn ..>>` field; a TS optional
  method of `PlaceRecording` (:391-454) answers `None` for "absent", and `has_images` stands for the pair
  `imagesOn`/`dropImageOn`. A door reads `None` exactly where the TS reads a missing method: the absent and the empty
  answer differ (`sub_holds_on` absent is `[]` at the call site, `spent_today` absent leaves `spentTodayUsd` off).
- **T3 Errors.** Every failure is `wsp_words::exit::HostError`. A TS error class is a mark in `HostError.marks`
  (`PLACE_*_MARK`), made by its constructor and read by its predicate:
  `place_host_key_changed` (:764, no kind), `place_login_refused` (:769-771, kind `login`), `place_forks_nowhere`
  (:775, no kind), `place_add_taken_back` (:779, no kind), `place_provisioning` (:785-787, kind `conflict`). The
  message is the one given, no fix. A predicate is true only for an error carrying its mark: a usage refusal with the
  same words is not a login refusal.

## The record

- **T4 `PlaceRecord`** (:69-100) reads and writes camelCase keys; every optional field is left out when absent, never
  written as `null`. A document the TS wrote reads back equal, whatever its key order; one with keys this port does not
  know still reads (they are dropped on the next write, see the report).
- **T5 `PlaceRecordRoad`** (:104-109): `ssh`, `from`, `back`, `keyPath`, `hostKey`. `view()` is the client's `PlaceRoad`:
  `ssh`, `from`, `back` alone.
- **T6 `is_place_record(v)`** (:133-136): true for an object (not null, not an array's missing fields) whose `id`,
  `publicKey` and `name` are all strings, empty strings included; nothing else is checked. `place_record_of(v)` is
  `Some` where that holds and the value deserializes as a `PlaceRecord`.
- **T7 `HeldRow`** (:119) is a `PlaceProvisionRow` with `pick` and `createdAt` after its own keys; `HeldApplied` (:120) is
  `{hash, at, rows, items?}` over held rows.

## The readers

- **T8 `made_by_setup(held, plugins)`** (:113-114): the keys of `held.picks.plugins` in the picks' order, each kept
  only where an applied row with id `plugins/<key>` reads outcome `installed`. No picks or no applied rows: empty. A
  row reading `present`, `failed` or `skipped` does not count.
- **T9 `applied_view(applied)`** (:126): the same `hash`, `at`, `items`, and each row with `pick` and `createdAt` taken
  off, every other key kept in its place.
- **T10 `projects_made(held)`** (:131): the `project.id` of each applied row that has a `project`, in row order, each
  once. No applied rows: empty.

## The lines (exact, from the oracle)

- **T11** `NO_PLACE_DOOR` (:729), `NO_PLACE_UPDATER` (:732).
- **T12** `place_update_slow_line(name, s)` (:736): `<name> took the daemon and had not dialled back on it within <s>s;
  its row reads the new version once it does`.
- **T13** `place_swept_over_ssh_line(name)` (:741): `wsp and the service that kept it running are removed from <name>`.
- **T14** `place_login_road_line(name, at, said)` (:747): `<name> did not answer the login at <at>` with no `said`; else
  `<name> ran the leave over the login at <at> and did not finish it (<said>)`. An empty `said` is a said.
- **T15** `place_swept_over_link_line(name, at, said)` (:753): T14's line, then `, so it swept itself over the link: its
  files came off and the service that starts the agent there did not`.
- **T16** `place_elsewhere_swept_over_link_line(name, away)` (:758): `<away>, so nothing was changed on the machine it
  reaches; <name> swept itself over the link: its files came off and the service that starts the agent there did not`.

## Constants

- **T17** Collections `places`, `caps`, `place-default` with the one id `default` (:60-65). They are table names other
  processes read (spec 02 rule 2.4).

## What survives a restart

Nothing in this file holds state. The record shape is what the store keeps across one.

## Tests

| Rule | Test |
|---|---|
| T3 | `error_marks_and_kinds`, `predicate_false_for_same_words_unmarked` |
| T4 | `record_reads_ts_document_and_omits_absent`, `record_round_trip_keeps_values` |
| T5 | `road_view_drops_key_path_and_host_key` |
| T6 | `is_place_record_cases` (oracle) |
| T7, T9 | `applied_view_drops_pick_and_created_at` (oracle) |
| T8 | `made_by_setup_only_installed_in_pick_order` (oracle) |
| T10 | `projects_made_in_row_order_once` (oracle) |
| T11-T16 | `lines_match_oracle` |
| T17 | `collection_names` |
