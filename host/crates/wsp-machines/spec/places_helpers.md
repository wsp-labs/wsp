# places_helpers: the readers and constants every area shares

Source: `helpers.ts` (431). Module `places::helpers`. Shared rules: `places-a-README.md`. Oracle:
`tests/fixtures/oracle/helpers.json` (written by `oracle/helpers.oracle.ts`); every expected value of a pure reader
below is read off it, never retyped.

## Constants (helpers.ts:91-431), each exact

`JOIN_PROVE_MS = 120_000`, `LOGINS_READ_MS = 10_000`, `SEEN_EVERY_MS = 60_000`, `REPLACED = "replaced by a newer
link"`, `LINK_FRAME_MS = 300_000`, `RELINK_WAIT_MS = 120_000`, `BACKEND_FACTS_MS = 10_000`, `ADD_FACTS_MS = 2_000`,
`CAPACITY_MS = 5_000`, `DIAL_MS = 20_000`, `JOIN_WAIT_MS = 90_000`, `ADDS_KEPT = 20`, `ADD_RUNNING_LINE`,
`ADD_RUNNING_FIX`, `PENDING = "pending-computers"`, `ADD_STOPPED_LINE`, `ADD_STOPPED_FIX`, `ADD_NOT_TAKEN_BACK_LINE`,
`NO_ESTIMATE_LINE`, `LOG_READ_MS = 10_000`, `UNDO_MS = 300_000`, `SIGN_IN_SLACK_MS = 5_000`, `GITHUB_MS = 30_000`,
`INSTALLS = "installs"`, `FILES = "files"`, `PROBE_MS = 30_000`, `SIGNIN_STATUS_MS = 30_000`,
`PROVISION_LOG_EVERY_MS = 2_000`, `PROVISION_LOG_LINES = 50`, `UPDATE_WAIT_MS = 60_000`, `UPDATE_POLL_MS = 500`,
`UNMERGE_MS = 60_000`, `PLACE_BAD_KEY_REFUSAL`. The fixture's `consts` holds every one; the test compares all.
`NEAR_PORTS = 20` is private (pane_ports spec).

## Readers

- `landed_on(rows, row) -> Option<Vec<PlaceProvisionRow>>` (:47). `LandedRow` is a row whose `label` is optional.
  - No row under `row.id`: `None` when `row.label` is `None`; else `rows` with `row` appended (its own label).
  - A row under the id whose outcome is neither `skipped` nor `failed`: `None` (a landed row never replaces an
    installed or present one).
  - Else the rows with that one replaced, in its place, by `{ ...row, label: stood.label, step: stood.step if any }`
    (fixture `landed[0]`); a `PlaceProvisionRow` is a wsp-wire struct, so its JSON keys come in the struct's order
    and the test compares values. The landed row's own `label` is dropped for the stood one (`landed[1]`). The stood row's `fix`, `ms` and `note` do not carry over
    (`landed[6]`).
- `bounded(work, ms, what)` (:56): `work`'s answer, or after `ms` on the clock `HostError("{what} was not answered
  in {round(ms / 1000)}s")`, JS `Math.round` (half up): 500 is `1s`, 499 is `0s`, 2,500 is `3s`. An error from
  `work` passes through with its message (a non-Error becomes its string). The timer is cancelled once `work`
  settles: a settled `bounded` leaves no timer on the fake clock.
- `reads_as_ed25519(b64) -> bool` (:81): true only for an SPKI DER ed25519 key in base64; empty, junk and a P-256
  SPKI are false; never an error (fixture `ed`).
- `place_home_refusal(said: Option<&str>)` (:99): `None` and `""` read "no home folder"; else the JSON quoting of
  the string then ", which is not a plain path" (fixture `homeRefusal`).
- `taken_report(report) -> Result<PlaceReport, HostError>` (:110), the one door every report passes:
  - `login.HOME` missing or not `is_plain_path`: `Err(place_home_refusal(home))`.
  - Else `login = { ...login, HOME, PATH: plain_path(login.PATH) }`: `HOME` and `PATH` keep their place where
    present and `PATH` is appended at the end where absent (fixture `taken[7]`); an empty PATH becomes the engine's
    default PATH (`taken[8]`).
  - Each of `SSH_STORE_VARS` (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`) present and not a plain path is removed; every
    other login key stays as given, plain or not (`OTHER: "kept;x"` in `taken[7]`).
  - Every other report field is unchanged.
- `agents_on(report, rows)` (:123): the report's agents in order, then each catalog agent (catalog order) whose
  `agents/<id>` row is `installed` or `present` and not already named; duplicates drop (a Set). A row for an agent
  the catalog does not know adds nothing; `failed` adds nothing (fixture `agentsOn`).
- `sign_ins_of(report, vault, rows, agents = agents_on(report, rows)) -> Option<IndexMap<String, AgentSignInState>>`
  (:137):
  - `report.logins` absent: `None` (unknown, not none). Present and empty is a map.
  - Per agent, in order: `signed-in` where the agent has a shared login file and `logins` lists it; or, with no
    shared file, where a row `signins/<agent>` with note `SIGNED_IN_THERE` reads `installed` or `present` (a
    `failed` one does not count). Else `vault_sign_in`.
  - Codex is the only agent with a shared file (`codex/auth.json`); a listed file for an agent not asked about adds
    nothing. Fixture `signInsCases`, all nine.
- `shared_login_file(agent)` (:160): `codex/auth.json` for codex, `None` for the rest (fixture `facts`).
- `shared_logins_script(logins)` (:173): byte for byte the fixture's `scripts` (shell quoting of the folder and of
  every shared file).
- `relisted_logins(listed, found)` (:179): `listed` less every shared file, plus `found`'s shared files, sorted by
  code unit; `None` where that equals `listed` element for element (an unsorted list that sorts differently is
  `Some`, fixture `relisted[4]`).
- `vault_sign_in(agent, vault)` (:186): `vault-key` where the agent's sign-in mints a token held in the vault under
  its `tokenEnv`, or its key env is held; else `none`. An agent the catalog does not know is `none`.
- `vault_held_line(agent, vault, way, here)` (:195): no sign-in or one that mints no token: `copied_from_line(here)`;
  else `key_held_line(here)` when `way == key` and the key is held, or when no token is held; else
  `token_held_line(here)`. So a key way with nothing held reads the key line (fixture `vaultHeld`).
- `picked_vault(vault, picks)` (:205): drops each `tokenEnv` of an agent picked `signin: key` whose sign-in mints a
  token and whose key is held; the same map (`vault` itself) where nothing drops, every other key in its order.
- `servable_options(options, vault, held)` (:217): each agent offering `vault` keeps it only where
  `vault_sign_in == vault-key` or the agent is in `held`; the rest of each agent and of the options unchanged.
- `login_here(homes, agent) -> Option<Vec<u8>>` (:224): no shared login, `homes()` failing, no home for the agent,
  the file `<home>/<shared.file>` missing, or empty: `None`. Else its bytes.
- `box_said(wiring, installed)` (:318): `[]` with no `wiring.log` or no `installed.ssh`; else `wiring.log({ ssh,
  keyPath? })`, `[]` where it fails. `keyPath` is present only where `installed.ssh_key_path` is.
- `pending_view(p)` (:333): the record less `keyPath`, `hostKey`, `undo`, `login`, other keys in order.
- `setup_outcome(setup, applied)` (:359): `said` is every wait's `wait_line` then every important failure as
  `"{label}: {note or failed}"`, joined `"; "`; any said: `needs-you` with it; else `ready`, with
  `"{plural(n, row)} did not install"` where n failed rows (any step) exist (fixture `outcome`).
- `cli_first(plan, ids)` (:382): steps after `plan.agents` are the CLIs. For each id in order, the chain of that
  step and its `after` steps (walking `after` until a step not found, already first, or already in this chain) goes
  first, earliest dependency first. An id not among the CLIs (an agent step, an unknown id) adds nothing. No id
  found: the same plan, unchanged (`same: true` in the fixture). A cycle stops at the repeat (`cliCycle`).
- `engine_row(r)`: a step set and not `signins` or `folders`, and id not `github`.
- `first_line_of(e)`: the message up to the first `\n`; a non-Error is its string (`42`, `null`).
- `picks_hash(picks)`: hex sha256 of `recipe_canon({ file: picks, items: {} })` (fixture `hashes`).

## Shapes

`Live { socket: Rc<dyn LinkSocket>, reach: Rc<dyn DaemonReach>, seen: Cancel, forward: Option<shared future of
PlaceForward> }`; `LinkSocket { is_open, close(code, reason), on_close(f) -> id, off_close(id) }`. `Forward { server,
local_port, conns: IndexMap<String, conn>, stop: Option<..> }`. `SyncJob`, `PendingRecord` as the TS (:330, :345).

## Pins

| Rule | Test |
|---|---|
| every constant | `constants_match_the_ts` |
| landed_on, every branch | `landed_on_matches_the_ts` |
| bounded message and rounding, error passthrough, no timer left | `bounded_says_what_was_not_answered`, `bounded_rounds_half_up`, `bounded_passes_the_works_own_error`, `bounded_leaves_no_timer_once_settled` |
| ed25519 read | `reads_as_ed25519_matches_the_ts` |
| home refusal and taken report, key order, store vars | `place_home_refusal_matches_the_ts`, `taken_report_matches_the_ts` |
| agents_on | `agents_on_matches_the_ts` |
| sign_ins_of, None on no logins | `sign_ins_of_matches_the_ts`, `sign_ins_of_is_none_where_the_daemon_lists_no_logins` |
| shared file, script, relist | `shared_login_file_matches_the_ts`, `shared_logins_script_matches_the_ts`, `relisted_logins_matches_the_ts` |
| vault words | `vault_sign_in_matches_the_ts`, `vault_held_line_matches_the_ts`, `picked_vault_matches_the_ts`, `servable_options_matches_the_ts` |
| login_here | `login_here_reads_the_shared_file_and_nothing_else` |
| box_said | `box_said_is_empty_without_a_log_road_or_an_ssh_login`, `box_said_swallows_a_failed_read` |
| pending_view | `pending_view_drops_the_door_only_fields` |
| setup_outcome | `setup_outcome_matches_the_ts` |
| cli_first | `cli_first_matches_the_ts`, `cli_first_answers_the_same_plan_when_no_id_is_a_cli`, `cli_first_stops_at_a_cycle` |
| engine_row, first_line_of, picks_hash | `engine_row_matches_the_ts`, `first_line_of_matches_the_ts`, `picks_hash_matches_the_ts` |
| box_said reads the log, keyPath only where present | `box_said_reads_the_log_with_the_key_path_only_where_there_is_one` |
