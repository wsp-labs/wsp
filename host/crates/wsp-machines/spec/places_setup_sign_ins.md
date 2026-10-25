# places_setup_sign_ins: the sign-ins step and the GitHub step of a run

Source: `setup.ts:374-631`. Module `places::setup::sign_ins` (methods of `SetupRun`). Shared rules:
`places-a-README.md`. TS cases: `test/places-setup.test.ts` :748-858, :894-904, :1084-1126, :1290-1396,
:1438-1508, :1571-1593, :1632-1997, :2188, :2349, :2381, at the run's seam (`places_setup_run.md`).

## The recording roads it uses (slice b's `PlaceRecording`)

`sign_in(place, ask, emit) -> handle { leave(), stop()? }`, `sign_in_line(place, agent) -> { status?, env? }`,
`mint_here(agent, emit)`, `login_here(agent) -> bytes?`, `stores_on(place, home)`. Every one optional; a missing
road is the TS `undefined`.

## A wait on the person (`sign_in_there`, :377)

`sign_in_there(ask, row, label, row_step, settled?, how { start?, note?, fix? })`:
- The road is `how.start`, else `recording.sign_in(place, ask, ..)`; none: the row lands at once `failed` with
  `NO_SIGN_IN_ROAD`, and nothing waits.
- Else a wait `{ row, label, expiresAt: ISO(now + SIGN_IN_WAIT_MS), state: waiting }` goes on the setup (replacing
  any wait under that row) and is said (`{ addId, placeId, wait }`, after the write).
- Under `"{place}/{ask}"` an earlier run following the same sign-in is muted at once, and let go (its `leave`) only
  once this run holds the new handle: a relay still waiting keeps its page meanwhile.
- A skipper under `"{place}/{row}"`: once, it stops and leaves the login, lands the row `skipped` with
  `SKIPPED_FOR_NOW`, and answers once that write is on the record. A second skip, or one after the row landed, does
  nothing.
- The road's events, ignored once muted, skipped or landed: `waiting` or `running` with a url or code update the
  wait (url and code each kept where the event leaves one out) and say it; with neither, nothing. `signed-in` lands
  `installed` with `how.note` or `SIGNED_IN_THERE` and `ms = round(now - begun)`. Any other end at or past
  `begun + SIGN_IN_WAIT_MS - SIGN_IN_SLACK_MS` marks the wait `expired` (its row does not land) and settles false;
  earlier, it lands `failed` with the event's `said` or "the sign-in did not finish". A road that fails to start
  lands `failed` with its first line, unless muted or skipped. Any end that is not waiting or running takes this run
  out of `signing_in` where it is still the one there.
- Landing: the skipper goes, the wait goes from the setup, `settled(outcome == installed)`, and `land_row` with
  `step: row_step` and, on a failure, `how.fix` where given.

## Agents

- `agent_signed_in(agent, plan) -> bool` (:453): where the catalog has a status check and the recording a line with
  a status, that status runs on the machine as `with_env_from_input("{path_line}\n{status} 2>&1")`, timeout 30,000,
  the line's env on stdin. An exit of `EXEC_DEADLINE_EXIT` or a failed exec reads as unread; else the check's own
  `signed_in(stdout, exit)` is the answer, and a signed-in one puts its login on the record's list
  (`login_listed`). Unread or no status: `sign_ins_here(place)[agent] == signed-in`.
- `agent_sign_in(agent, plan)` (:469): signed in there already lands `present` with `SIGNED_IN_THERE`; an agent whose
  sign-in asks the person to pick lands `skipped` with `AT_ITS_TERMINAL`; one whose page hands back a code lands
  `skipped` with `CODE_FROM_ROW` and the ways fix (machine first) else `sign_in_there_fix(record name)`; else a wait.
- `ways_fix(agent, label, first?)` (:483): none for an agent with no ways; else `sign_in_ways_fix(ways with first
  first, { name: label, here, box: record name, mint?, keyEnv? })`. `here` is the host computer's label, else name.
- `vault_way(agent, label, way)` (:494): the env named by the way (token: the minted token's env; key: the key env)
  held in the vault lands `present` with `token_held_line(here)` or `key_held_line(here)`. A key not held: `failed`,
  `no_key_line(env or "its key", here)`, fix. A token not held and no `mint_here`: `failed`, `NO_SIGN_IN_ROAD`, fix.
  Else a wait started by `mint_here`, noting `token_held_line(here)` when it lands, and no row now.
- `again_sign_in(agent, plan)`: the token way where the row picked `token`, else `agent_sign_in`.
- `not_installed(agent)`: some row of that agent reads `failed`.
- **The step** (`sign_ins`, :522), for each picked agent in order: a sync passes over an agent its changes did not add
  or change; an agent `signs_in_again` refuses is passed over. Way `token` or `key`: `vault_way`. Way `machine`:
  `skipped` with `waits_for_install_line(label)` where not installed, else `agent_sign_in`. Way `vault` (the
  default): `present` with `vault_held_line` where the vault holds it; else the copy below; else `failed` with
  `no_copy_line(label, here)` and the ways fix or `sign_in_there_fix`.
- **The copy** (`login_copied`, :559): none with no shared login, no login here, or no store folder there. Not
  installed: `skipped`, waits for install. Signed in there: `present`, `SIGNED_IN_THERE`, nothing copied over it.
  Else `umask 077 && mkdir -p '{store}' && cat > '{store}/{file}.wsp' && mv -f ... ` with the login on stdin alone
  (never in the command), timeout 30,000; a non-zero exit `failed` with the last stderr line or "the copy exited
  {n}". Then signed in: `installed`, `copied_from_line(here)`; else `failed`, `copied_not_signed_in_line`.

## GitHub (:577-631)

- `github_ready`: settles once. No GitHub pick: true at once. A GitHub step that will not run this time (ended, or
  not in a sync) and no GitHub wait to rerun: from the row (`present` or `installed`). `github_known` is the settled
  value once in.
- `gh_status(plan, token?)`: `{path_line}\ngh auth status --hostname github.com 2>&1`, timeout 30,000; with a token,
  wrapped for stdin with `GH_TOKEN` on stdin alone.
- `github_sign_in(plan)`: gh's own status exit 0 settles true and lands `present`, `SIGNED_IN_THERE`, step github;
  else a wait for `gh`'s login on row `github`, settling with its end.
- **The step** (`github`): no pick: no rows. Word `skip`: what an earlier run put on in the github step stays (rows
  installed, and the GitHub row if present); with no GitHub row among them a `skipped` row with
  `GITHUB_SKIPPED_LINE`; settled by whether the row stood. Else the engine's `github` step; any failed row settles
  false and answers. Word `machine`: `github_sign_in`. Word `vault`: no `GH_TOKEN` held is `failed` with
  `no_copy_line("GitHub", here)` and `sign_in_there_fix`; else the status over the token: exit 0 is `present` with
  `copied_from_line(here)` and `"; token scopes: a, b"` where gh names any; else `failed` with gh's last line or
  "gh auth status exited {n}". The token never appears in any command.

## Pins

| Rule | Test | TS case |
|---|---|---|
| a machine sign-in waits with page and code; the vault's is a row | `a_machine_sign_in_waits_with_its_page_and_code` | :647 |
| status says signed in: no sign-in, present, listed | `an_agent_signed_in_there_starts_no_sign_in` | :773 |
| status says signed out over a listed login: starts | `a_signed_out_status_starts_the_sign_in` | :791 |
| no status: the daemon's list | `with_no_status_the_listed_login_counts` | :799 |
| a status cut at its deadline is unread | `a_status_cut_at_its_deadline_is_unread` | :808 |
| no shared file, status signed in | `an_agent_with_no_shared_file_reads_its_status` | :817 |
| a run again after the person signed in starts no second | `a_run_again_starts_no_second_sign_in` | :827 |
| gh signed in there: no GitHub sign-in | `gh_signed_in_there_starts_no_github_sign_in` | :844 |
| vault token on stdin alone, scopes on the row | `github_from_the_vault_names_the_token_scopes_and_never_sends_it_in_a_command` | :894 |
| GitHub on the box through the relay | `github_signs_in_on_the_box_through_the_relay` | :1084 |
| a resumed waiting sign-in asked afresh; one finished by hand reads there | `a_resume_asks_a_waiting_sign_in_afresh`, `a_sign_in_finished_between_runs_reads_there` | :1290, :1318 |
| a retry takes a waiting sign-in over; the old run writes nothing | `a_retry_takes_a_waiting_sign_in_over` | :1338 |
| a page that ran out reads expired; slack of 5 s | `a_sign_in_whose_page_ran_out_reads_expired`, `an_end_inside_the_slack_reads_expired` | :1357, :1959 |
| a failure well before the page ran out lands failed | `an_early_failure_lands_failed` | |
| the copy on stdin alone, signed in with no code | `a_login_from_here_is_copied_on_stdin_alone` | :1439 |
| a login there already is not copied over | `a_login_there_already_is_left_as_it_is` | :1472 |
| no copy where the agent did not install | `no_copy_where_the_agent_did_not_install` | :1490 |
| a way change runs only that sign-in | `a_way_change_runs_one_sign_in` | :1571 |
| skip for now: skipped, login stopped, Ready | `skip_for_now_lands_skipped_and_stops_the_login` | :1633 |
| skip is once | `a_second_skip_does_nothing` | :1973 |
| pick-at-terminal and code-from-row skip with no login run | `a_sign_in_that_asks_the_person_is_left_to_its_terminal`, `a_page_that_hands_back_a_code_is_left_to_its_row` | :1789, :1946 |
| waits for install where the agent failed | `a_sign_in_waits_on_its_agent_that_did_not_install` | :1816 |
| key way: present, or failed with where the key goes | `a_key_way_reads_the_vault_or_says_where_the_key_goes` | :1872 |
| token way minted here waits on the person | `a_token_way_is_minted_here_and_waits` | :1923 |
| no sign-in road lands failed at once | `no_sign_in_road_lands_failed_with_nothing_waiting` | |
| GitHub skip keeps what an earlier run put on | `a_github_skip_keeps_what_an_earlier_run_put_on` | :1693, :2259 |
| GitHub vault word with no token fails with the fix | `a_github_vault_word_with_no_token_fails_with_its_fix` | :904 |
| a sync starts no sign-in for an agent signed in there | `a_sync_lands_an_added_agent_signed_in_there_as_there` | :2349 |
| a sign-in row stands as its own step reads it | `a_sign_in_row_stands_as_its_step_reads_it` | :2188 |
