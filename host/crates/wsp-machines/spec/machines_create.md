# machines_create
Source: `packages/runtime/src/machines/machines.ts:15-362` (348 of 906 lines) at `65f6b89c7`. Tests: integration cases in
`lost-create.test.ts`, `local-workspace.test.ts`, `strays.ts` and the sweep suites (under "Rules against tests"), and 29
scenes in `oracle/machines.oracle.ts` ("machines: creates"), written to `tests/fixtures/oracle/machines_creates.json`.
Rust: `src/machines/machines/create.rs`.

## What it does
The machines area's road to a new machine: the spec a fork asks for, the idempotency key every create carries and the
`creates` row that holds it before the provider hears of it, the replay of an attempt nothing answered, the sweep of
machines made under this host's image names that nothing names, the claim that keeps a create in flight out of the
sweep's hands, the exec log listener, and `fork` and `unfork` of a workspace's machine.

## Behaviour
1. `fork_spec(r, kind, image, override, npm_bin)` (:23): keys in order: the image spec's own keys, `kind`, `binds` (only
   non-empty), `copy`, `cpu: override.cpu ?? r.shape.cpu ?? r.size.cpu`, `memMb` likewise, `envs: {...login_env_on(r.place,
   npm_bin), ...r.spec.envs, ...override.envs}`, `labels: {...r.spec.labels, wsp: "1", "wsp-owner": state.owner,
   "wsp-workspace": r.id, "wsp-name": r.name, "wsp-golden": r.golden, createdAt: ISO(now)}`, `onIdle: "pause"`,
   `idleTimeoutMs: backstop_ms(idle_window_of(r))` (twice the window; 6 h when the window is off).
2. `image_mark()` (:41): `template_host(hostId) + template_host(state.owner)`, where `template_host` keeps what follows
   the last `:`, lowercased, with every character outside `[a-z0-9]` removed (fixture: `host:ABC-1` and `h_0a993e4c`
   give `abc1h0a993e4c`).
3. `hold(purpose)` (:45): waits while the purpose is held, re-checking after each wake (not FIFO, 03 rule 3.9), then holds
   it until the returned release runs. Release removes the key and wakes the waiters.
4. `keyed_create(at, purpose, spec, waiting, after_corpse = false, place)` (:61):
   a. `body = fingerprint(spec)`: sha256 hex of the JSON of the spec without `idempotencyKey` and without the
      `createdAt` label, `labels` re-added last (`types/internal.ts:867`).
   b. `held = store.get("creates", purpose)`; it is *theirs* when its host is another, or its pid is not this
      process's and that pid is alive.
   c. `name` = the purpose when its UTF-16 length is at most `KEY_PURPOSE_MAX` (238), else the sha256 hex of it.
   d. *Replay* when `held.body == body`, not theirs, and (`held.request` is set or the backend has no `create_request`).
   e. A replay keeps `held` with `host` and `pid` set to this host and process; a fresh attempt is `{key: name + ":" +
      Ids::random_hex(8), createdAt: ISO(now), body, host, pid, request?: at.create_request(stamped), image?:
      spec.fromSnapshot ?? spec.template, place?}` in that key order. `stamped` replaces the `createdAt` label with the
      attempt's `createdAt` only where the spec carries that label.
   f. The attempt is put under `creates/<purpose>` **before** the provider is asked; then `at.create({...stamped,
      idempotencyKey: key}, waiting, attempt.request)`.
   g. A failure carrying a numeric `status` deletes the row; any other failure keeps it; either is rethrown. Success
      deletes the row.
   h. A `replayed` machine whose `state()` reads `gone`: after a corpse already, throws `create for <purpose>: the
      provider replayed <id>, which is gone, under a key it had never seen (<key>)`; else warns `create for <purpose>:
      the replay under <key> named <id>, which is gone; creating anew` and calls itself once with `after_corpse`
      (a fresh key, since the row is gone). A replayed machine not gone warns `create for <purpose>: <id> replayed from
      an earlier attempt under <key>` and is answered.
5. `replay_creates(known, failed)` (:106): for each key of `creates` in store order, skips the row when it has no
   `request`, is another host's, its purpose is held now, its pid is another live process, it is under
   `LOST_CREATE_MS` (120,000) old (`now - createdAt < 120000` skips; exactly 120,000 is asked), or its place (absent
   or `places.wired` means the wired backend) has no backend or no `replay_create`. Else, under `hold(purpose)`:
   `replay_create(key, request, createdAt)`; a `missing` failure deletes the row; another pushes `{message: "the
   create under <key> was asked again and not answered: <why>; asked again next sweep"}` and keeps it; `None` deletes
   it. A machine: when its `seen.state` is not `gone`, its id is not in `known()`, and no kept name waits for it
   (`purpose` is `workspace/<id>`, that id is not live, and `workspace-names/<id>` names a project in `projects_held`),
   `gone.stop(at, machine)`; a stop failure other than `missing` pushes `{id, message: "could not stop: <why>"}` and
   keeps the row (no delete); otherwise it pushes `{id, builder: false, reason: "lost", ageMs: now - createdAt,
   labels?, name?: row.image}`. Then the row is deleted.
6. `sweep_strays(known, failed, at)` (:159): only backends with `named_after`; none answers `[]` with no store read.
   `asked` = the `image` of every `creates` row; `names` = `recorded_images()` and `asked`, kept where
   `name_owner(name) == image_mark()`; none answers `[]`. For each backend, `strays(named_after(names), {known, asked,
   now})` (engine: not gone, not known, name not asked, `createdAt` parses, age at least `STRAY_GRACE_MS` 600,000), each
   then `gone.stop(b, b.get(id))`: a failure not `missing` pushes `{id, message: "could not stop: <why>"}`; `missing`
   is skipped silently; a stop pushes `{id, builder: false, reason: "stray", name, ageMs}`.
7. `claiming(purpose, run, at = backend, place)` (:185): `run` gets a handle with `at`'s capabilities, pricing,
   `baseTemplates`, `namesWorkspace`, `get`, `list`, `delete_snapshot`, every optional call `at` has (bound), and a
   `create` that holds the purpose, runs `keyed_create`, and claims into `inflight` every box id a `waiting` line
   names and the machine it answers (each once). Every id it claimed leaves `inflight` when `run` settles.
8. `observed(machine)` (:232): with no `goldenRecipe.onExec` the machine as it is. Otherwise `exec` and `run` report
   `{machineId, cmd, ms, ...result}` (a run with `unlogged` reports only `exitCode`) or `{machineId, cmd, ms, error}`
   and answer as the machine did; a listener that throws is warned once per machine (`exec log for <id> failed, its
   execs go on unlogged: <why>`) and never changes a result.
9. `observing(b)` (:302): `create` and `get` answer `observed` machines.
10. `fork(record, bind, override, report, reads_memory = true)` (:307), inside `claiming("workspace/<id>",
    ..., backend_for(record), record.place)`: the image (`image_of(record.golden)` where the backend keeps images),
    the spec (rule 1), `b.create(spec, report "fork-requested" with `{waiting: true, box?}`)`. A `missing` refusal on a
    fork from an image at a place throws `place_holds_no_image_line(name, fromSnapshot ?? template ?? golden)` with
    kind `invalid`. Then `bind(machine)` before the claim is released; unless the backend names the workspace,
    `set_hostname` and report `hostname-set` (`hostname_set_line(host)`, or `HOSTNAME_KEPT` with `{detail}`); then
    `apply_machine_context` (a failure only warns `machine context for <id> on <machine> <summary>`); `npmBin`,
    `shape`, `size = size_built(shape, spec)`; with `reads_memory` and no place, `read_memory`; `screen` from
    `streamUrl`. Absent values delete their keys.
11. `unfork(entry)` (:356): `kill_until_gone(backend_for(record), machine, opts.killConfirm)`, then put
    `dropped-machines/<id>` `{machineId, at: ISO(now)}`; a failed put only warns.

## Edge cases
- A purpose of exactly 238 UTF-16 units is kept as the key's name; 239 is hashed (scenes "a purpose ...").
- A replayed attempt keeps its old `createdAt` and stamps it into the label (scene "a held attempt of a dead pid").
- `strays` rows with no `createdAt` are never deleted (engine: `NaN` age).

## Behaviour nobody wrote down
- Rule 3.4: the `creates` row is written before the provider call; `hold` is taken before `keyed_create` and released
  in a `finally`; the claim into `inflight` happens synchronously in the waiting callback.
- Rule 3.16: the key's nonce is `Ids::random_hex(8)`; the test `Ids` answers byte `k` repeated on its `k`-th call, as
  the oracle mocks `randomBytes`.
- Restart: the `creates` rows and `dropped-machines` survive; `holding` and `inflight` start empty.

## Tapes
None: creates are reached through `workspaces.create` and the sweep in other units.

## Decisions
1. Pids: "this process" is `std::process::id()`; liveness is `kill(pid, 0)` as `pidAlive` reads it.

## After the port
- `hold` is not FIFO: a create can starve behind repeated replays of one purpose.

## Rules against tests
| Rule | Tests (`tests/machines_create_scenes.rs` replays a scene by name; `tests/machines_create.rs`) |
|---|---|
| 1 | `fork_spec_keys_and_precedence_match_the_ts`, `fork_spec_backstop_is_twice_the_window_or_six_hours_off` |
| 2 | scene `strays_only_this_hosts_image_names_never_known_or_asked` (the mark line) |
| 3 | `hold_waits_for_the_holder_and_rechecks_after_waking` |
| 4 | scenes `a_keyed_create_*`, `a_create_with_a_request_*`, `a_held_attempt_*` (2), `an_attempt_*` (3), `a_changed_request_*`, `a_refusal_with_a_status_*`, `a_failure_with_no_status_*`, `a_replay_*` (2), `a_second_gone_replay_is_thrown`, `a_purpose_*` (2), `a_spec_with_no_created_at_label_*` |
| 5 | scenes `replay_*` (11) |
| 6 | scenes `strays_*` (2) |
| 7 | `claiming_holds_every_claimed_id_until_run_settles`, `claiming_forwards_the_optional_calls_bound` |
| 8, 9 | `observed_reports_each_exec_and_run_once_and_a_throwing_listener_is_warned_once` |
| 10 | `fork_binds_before_the_claim_is_released`, `fork_at_a_place_without_the_image_says_where_it_would_land`, `fork_writes_shape_size_and_screen_and_deletes_what_is_absent` |
| 11 | `unfork_marks_the_machine_dropped_after_it_is_gone_and_a_failed_mark_only_warns` |
