# places_link_reads: what a link and a place say, the forwards, and the place a word names

Source: `link.ts:58-77` (`land_sign_in`) and `:316-541`. Module `places::link` (`LinkDoor`, `name_of` to the end).
Shared rules: `places-a-README.md`. TS cases: `test/places-facts.test.ts`, `places.test.ts:188, :278`,
`places-agents.test.ts:87, :570`, `places-setup.test.ts:1651-1680, :1826-1922, :1705-1771`, `place-forward` via the
door, `copy-on-press.test.ts`, at this unit's seam (the fakes of `places_link_door.md`).

## Landing a sign-in from outside a run (`land_sign_in`, :61)

A run still writing the place's rows (its lander): the lander lands it, and where it did, the record is read back.
Else one `change`: no applied rows, or `landed_on` answers none: nothing. Where something landed, `place.changed {
place: with_cap(joined_row(record, mark_held() or HERE_PLACE_ID), row_ids()) }` is emitted. Nothing is emitted for a
row that did not land.

## The reads

- `name_of(place)`: `here` for `HERE_PLACE_ID`; the kept record's name; else `provider_key_name(place)`.
- `settings_at(place)`: `settings_held[place]` or empty.
- `turn_limit_at(place)`: `computer` for here or a kept place, `provider` for a provider id, else none;
  `place_turn_limit(kind, settings)`.
- `threads_at(place)`: none unless here or kept; `place_cap_of({ computer, shape? }, settings).threads` where the cap
  has `threads`. The shape is `wiring.here().shape` for here, else the report's.
- `machines_at(place)`: only a provider id; `place_cap_of({ provider }, settings).machines`.
- `sign_ins_at` = `ctx.sign_ins_here`. `vault_at(place, vault)` = `picked_vault(vault, kept picks)`.
- `github_from_vault(place)`: the kept picks have a GitHub config whose `signin` is `vault` (or unset) and an applied
  `github` row reads `present`.
- `login_landed(place, agent)`: the login listed on the record, then a landed row `{ id: github for gh else
  signins/<agent>, outcome: installed, note: SIGNED_IN_THERE }`.
- `logins_again(place)` (:356): nothing without kept facts' `logins` or a live link. Else
  `shared_logins_script(logins)` through `door().exec` with timeout 10,000; a failure or a non-zero exit: nothing.
  The non-empty stdout lines are `found`; one `change` puts `relisted_logins(report.logins, found)` on the report
  where it answers one (a report with no logins list is left alone).
- `key_landed(agent)` (:368): nothing unless the vault now reads `vault-key` for the agent; else for every record in
  order a landed row `{ id: signins/<agent>, outcome: present, note: vault_held_line(agent, vault, the record's picked
  way, here) }` (a row that stood installed or present is left: `landed_on`).
- `gh_there(place, stage)` (:373): nothing with no provisioner, record or `HOME`. Else the record's picks (or a recipe
  named after the place) with no CLIs and `configs: { github: { signin: machine } }`, planned for the `github` step
  alone, and that step run on the place machine; each `installed` row lands with `step: github`. Answers the rows.
- `offer_of(place)`: the kept facts' `offer`, else the id where it is a provider, else none.
- `backend_of(place)`: a built backend; else built from kept facts (`backend_from`); else none for a kept place; else
  the provider's backend.
- `joined(place)`: kept. `folder_computer(place)` (:402): none without a record or `HOME`; one `PlaceFolderMachine`
  per place while `HOME` and `PATH` stand (keyed `"{home}\0{path or empty}"`), built afresh when either moves, `path`
  passed only where non-empty; `{ machine, home, shape, tools: report.wspDoor == true }`.

## `forking_backend(place)` (:415)

1. The record (read, else kept). No record: the provider's backend where one; else `backends` loses it and
   `Err(PlaceForksNowhere(place_forks_nowhere_line(name or id)))`.
2. `setting_now(record)` refuses with `PlaceProvisioning(line)`, except where the call runs under that place's own
   folder add (`folders_of`).
3. `door().backend_of(place)` where built.
4. One read in flight per place (`asking`): a second caller gets the same answer. The read: `machine.backend` over
   `link_to(place)` bounded 10,000 ms (`"machine.backend on {name}"`), parsed as `BackendFacts` (a bad answer is the
   parse's error); then one `change` writing `backendFacts` only where the record's daemon version is still the one
   the read began under. Written: `backend_from` (kept in `backends`). Not written (a newer daemon dialled back):
   a backend over the facts that is not kept. `asking` loses the place however the read ends.

## Forwards (:452)

- `forward(place, port, o)`: key `"{place}:open:{port}"` for a pane, else `"{place}:{port}"`. A pane: `standing`
  where `o.standing`, else the pane ports' `reach` (`places_pane_ports.md`). Else a forward under the key answers its
  port; else a listener on `127.0.0.1:0` kept under the key with its port.
- Each connection: on a place with no live link, destroyed at once (the listener stays). Else marked used, kept under
  `"p{n}"` (one counter per door, from 1), paused until `tunnel.open { tunnelId, port }` answers: then each chunk goes
  as `tunnel.write { tunnelId, data: base64 }` (a failure destroys the connection) and the connection resumes; a
  refused open drops it and destroys it. Its close drops it, marks used, and sends `tunnel.close { tunnelId }`.
- `pane_forwards`: the pane ports.

## The place a word names (:504)

- `place_for(word)`: the records `names_place` matches; two or more: `Err(two_places_refusal(word, their ids))`. One:
  `{ placeId }`. Else here (`HERE_PLACE_ID` or the host's name) or the wired provider named: `{}`; another provider
  named: `{ placeId }`; else `Err(no_such_place_refusal(word, [here, record names..., provider key names...]))`.
- `default_place()`: no mark: `{}`; a record by that id: `{ placeId }`; a provider id that is not the wired one: `{
  placeId }`; else `{}`.
- `mark_used(place?)`: marks `place`, else the wired provider, else `HERE_PLACE_ID`.
- `mark_default_if_none(place)`: in the records' turn, writes the mark only where none is held.

## Pins

| Rule | Test | TS case |
|---|---|---|
| facts asked once, again on a new daemon | `the_facts_are_asked_once_and_again_on_a_new_daemon` | places-facts:15 |
| two roads asking at once send one frame | `two_roads_asking_at_once_send_one_frame` | |
| an answer from an older daemon is not kept | `facts_from_a_daemon_that_was_replaced_are_not_kept` | |
| backend facts land on the record as it stands | `backend_facts_land_on_the_record_as_it_stands` | invariant 6 |
| forks nowhere; a provider row forks by its own backend | `a_word_with_no_record_and_no_provider_forks_nowhere` | |
| busy setup refuses a fork except under its own folder add | `a_fork_is_refused_while_the_setup_runs_except_its_own_folder_add` | places-setup:1127 |
| name_of, turn limit, threads, machines | `the_place_reads_match_the_ts` | places-facts:199, :213 |
| github_from_vault | `github_from_vault_reads_the_pick_and_the_row` | |
| login_landed lists and lands; emits changed once | `a_login_landed_at_a_terminal_reads_signed_in`, `a_row_that_did_not_land_emits_nothing` | places-facts:228, places-setup:1651 |
| a run still writing takes the landing | `a_landing_during_a_run_goes_through_its_lander` | places-setup:1840 |
| logins_again relists | `logins_are_read_again_off_the_computer` | places-agents:87 |
| key_landed lands on every record | `a_key_put_in_the_vault_lands_on_every_computer` | places-setup:1826, :1888 |
| gh_there plans github alone and lands installed rows | `gh_there_puts_gh_on_alone_and_lands_it` | places-setup:1705, :1747 |
| folder computer is one per home and PATH | `the_folder_computer_is_one_while_home_and_path_stand` | |
| forward: tunnel frames per connection; refused open drops; away destroys | `a_forward_carries_each_connection_as_a_tunnel`, `a_refused_tunnel_drops_its_connection`, `a_connection_while_away_is_destroyed` | place-forward:45, :77 |
| place_for: two by one word; here; providers; unknown | `place_for_names_the_place_a_word_means` | places:278, :188 |
| default_place and the marks | `the_default_place_follows_the_mark`, `mark_default_if_none_marks_only_once` | places-facts:213, copy-on-press |
