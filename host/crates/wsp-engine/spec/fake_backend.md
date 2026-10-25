# fake_backend: the stand-in provider a harness serves fixtures through

Source: fake-backend.ts at `a4033dc34`. Module `fake_backend`. Shared rules: `roads-README.md`.
Oracle: `tests/fixtures/roads/fake_backend.json` (the records file byte for byte, the clock pinned).

## Fixed values

- Sizes (:49): 2/4096, 4/8192, 8/16384. `rate_usd_per_hour(size) = Number((cpu * 0.02 + memMb / 1024 * 0.01)
  .toFixed(4))`: 0.08, 0.16, 0.32; 3/1000 is 0.0698; 1/1 is 0.02. Round half as `toFixed` does on the binary value.
- Pricing: default size 4/8192; snapshot storage `{ freeGb: 10, usdPerGbMonth: 0.15, billedFrom: "2026-01-01" }`;
  builder disk 20.
- Lifecycle: budgets `{ wakeAttempts: 1, daemonAnswersMs: 1000 }`, nothing else.
- `FAKE_NO_GUEST` (:75): exact sentence; it holds `stand-in provider for testing` and `nothing runs on them` and
  none of `out of memory`, `no guest`, `capability`, `landsBytes`.
- `id(prefix) = "{prefix}_{12 hex}"`.
- Capabilities (:273), key order as the oracle: liveCloneForks true, pauseMode memory, replacesMachine true,
  previewUrls = a guest is wired, signedUrls false, callbackRelay false, diskSnapshots true, images true,
  snapshotsAnyLife true, snapshotListing true, templates false, sizes (with rates), kept false, copies true,
  ownNetwork true.

## Records

- Shape: `{ machines: { id: { state, shape, labels } }, snapshots: [row] }`. Machine order is insertion order; a
  move keeps a machine's place.
- In memory (no file named): one per backend, starting empty.
- In a file (:128):
  - Read at every call; nothing is cached between calls, so a second backend on the same file sees every change.
  - A file that cannot be read (absent, a folder, no permission) is an empty fleet.
  - A file that is there and does not parse as JSON fails every call that reads it with
    `"{path} is not this stand-in provider's records: {why}"`; it is never taken for empty and never written over.
    `why` is the parser's message (serde's words differ from V8's; a test matches the sentence up to the colon).
  - Parsed JSON is read as `{ machines: {}, snapshots: [] }` with the file's own fields over it (the TS spread): any
    other top-level key survives every rewrite, after `machines` and `snapshots`, whatever order the file had; JSON
    that is not an object (`[]`, `null`, `5`) reads as an empty fleet. A seeded snapshot row keeps its own keys and
    their order on a rewrite.
  - Every change writes the whole records as JSON indented by two spaces plus `\n` to `{path}.next` (parents made),
    then renames it over `{path}`; no `.next` is left. Key orders: machine `state, shape, labels`; shape `cpu,
    memMb, diskGb, createdAt`; snapshot row `id, name, sizeBytes, restoredBytes, createdAt`; a seeded row keeps the
    keys it was seeded with.
  - The file is the one thing here that survives a restart: a new backend on it finds every machine in its state and
    every snapshot.

## `FakeBackend`

- `new(size, records, guest)`. `create(spec)` mints `fk_{12 hex}`; `get(id)` mints the id as given (`fk_c0ffee` and
  `fk_c0ffee.paused` both read running: no word in an id means anything).
- Mint (:297): a known machine is used as recorded and nothing is written. An unknown one is recorded running with
  shape `{ cpu: spec.cpu or size.cpu, memMb: spec.memMb or size.memMb, diskGb: spec.diskGb or 20, createdAt: now as
  ISO with milliseconds }` and labels `spec.labels or {}`, and the records are written.
- `list()`: every machine `{ id, state, labels }` in record order. `list_snapshots()`: the rows as stored. Offers
  `list_snapshots` and nothing else.
- `delete_snapshot(id)`: drops every row with that id and writes, even when none matched.

## `FakeMachine`

- Kind sandbox; labels and shape as minted; `seen` is the state read at mint and `createdAt` where the shape has one.
- `state()`: the records' state now, or the state seen at mint where the machine is no longer recorded.
- `pause` / `resume` / `kill` move the recorded state to paused / running / gone and write; a machine no longer
  recorded is left alone and nothing is written.
- `snapshot(name, ..)`: appends `{ id: fksnap_{12 hex}, name, sizeBytes: 8 GiB, restoredBytes: 8 GiB, createdAt:
  now }`, writes, answers the id. `describe()`: the shape. `facts()`: `{ os: "Debian GNU/Linux 12", uptimeMs:
  10800000, folder: "/root" }`. `download_url` / `upload_url` fail with `this provider mints no signed {download|
  upload} URL`.
- Without a guest: `exec` and `run` fail with `FAKE_NO_GUEST`; no preview, no token path, no bytes.
- With a guest (:83): offers `preview_url` (the guest's `reach(id, port)`), `daemon_token_path = guest.token_path(id)`,
  and `put_bytes` only where the guest has one. A command runs as `bash -c {cmd}` through `run_child`: where the
  guest has `shell(id, cmd)` its `{ cmd, cwd, env }`, else `{ cmd, cwd: folder(id), env: { HOME: folder } }`; the
  cwd is made first; env is the process env with the guest's over it. `exec` passes its timeout; `run` passes the
  deadline as the timeout and its line listener.

## Pins

| Rule | Tests |
|---|---|
| refusal sentence without a guest | `names_itself_when_a_road_reaches_for_the_guest` |
| any id answers running | `still_answers_for_a_machine_a_fixture_names` |
| a second backend on the file sees the first's changes | `holds_its_machines_in_the_file_it_was_given` |
| seeded and taken snapshots, delete | `lists_the_snapshots_a_harness_seeded_and_the_ones_its_machines_take` |
| a torn file refused and untouched, rename leaves no .next, empty folder is empty | `never_takes_a_file_it_cannot_read_for_an_empty_fleet` |
| guest folder, preview, token path | `runs_a_machines_commands_in_that_machines_own_folder_once_a_guest_is_wired` |
| records byte for byte, key orders | `the_records_file_is_what_the_ts_writes` |
| capabilities, rates | `capabilities_and_rates_match_the_ts` |
| a move on a machine no longer recorded writes nothing | `a_machine_no_longer_recorded_is_left_alone` |
| facts, describe, seen | `facts_describe_and_seen_match_the_ts` |
