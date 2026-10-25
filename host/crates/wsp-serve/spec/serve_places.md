# serve_places: identity re-asks, the person's computers, recipes, account, devices and tickets

serve.ts:396-418 (the doors these ops read), 863-868 (`refused_off_own_road`), 880-1169 at `65f6b89c7`. Rust:
`src/serve/places.rs` (`answer_places`, called from `answer_frame` for these ops). Tests: `tests/serve_places.rs`.

**The two gates.** `refused_off_own_road()` (serve.ts:864-868): a socket whose `own_road()` is false (stamped: a
ticket's or a thread's) is sent `{"id","ok":false,"error":PLACES_TICKET_REFUSAL,"kind":"ticket"}` and the op runs
nothing. **Host only**: `own_road() && me == host`, a socket opened by this host's own token (or a ticket it
minted; a ticket's socket is `me = host` but never its own road).

## Ops

- P1. `auth` on a socket already in: `{"id","ok":true,...released}` again (serve.ts:880-882).
- P2. `pair.issue`: host only, else `{"id","ok":false,"error":PAIR_ISSUE_REFUSAL}` (no kind). The code comes from
  `devices.issue(now, pair_ttl_ms, here)` (`here` only when the frame says `here: true`). Reply
  `{"id","ok":true,"code","expiresAt","hostKey"?}`, `hostKey` the place door's `host_key()` where a place door
  is wired; no device door refuses `NO_DEVICE_DOOR` through the catch (serve.ts:883-897, 396-401).
- P3. `pair.redeem` once in: `PAIR_CODE_REFUSAL`; `device.auth`: `DEVICE_AUTH_REFUSAL`; `seal.open`:
  `unauthorized`; `place.join`, `place.auth`, `place.prove`: `PLACE_UNKNOWN_REFUSAL`. None has a kind, none closes
  the socket (serve.ts:898-917).
- P4. `places.list`: a thread's socket reads `{"id","ok":true,"places":thread_places(..),"adds":[],"pending":[]}`
  (serve_helpers.md, H20) before the own-road gate. Otherwise the gate, then
  `{"id","ok":true,"places":list(now),"adds":adds(),"pending":pending()}`. No place door refuses
  `NO_PLACE_DOOR` (serve.ts:918-922).
- P5. Each of these is gated, then asks the place door and spreads or names its answer:
  `places.update` (`{sudoPassword}` only when sent), `places.remove` (`sudoPassword`, `force: true`, `forget: true`
  each only when sent and true), `places.holds`, `places.dial(now)`, `places.skip` (`place`), `places.estimate`
  (`estimate`), `places.setupLog` (`lines`), `places.loginLanded` (`{"id","ok":true}` after the await),
  `places.choose` (`pending`) (serve.ts:923-975, 1085-1089).
- P6. `places.set`: gated; the frame less `id`, `op`, `placeId`, `reset`, `name`, `ssh` and `recipe` is the set;
  `recipe` is passed as sent when absent or `NO_RECIPE` ("none"), else as the saved recipe's slug read by name
  (`recipes.read(word)`, whose refusal names the saved ones). Reply spreads the door's answer (serve.ts:943-948).
- P7. `places.follow`: gated; `NO_RECIPE` stays, any other word becomes its slug; reply `{"id","ok":true,"place"}`.
- P8. `places.doctor` (serve.ts:976-1007): gated; no doctor door sends `DOCTOR_UNSERVED` (no kind); a place id not
  in `list(now)` sends `no_such_place_refusal(id, rows' table names)` kind `usage`; a row that is not a joined
  computer sends `doctor_row_refusal(table name)` kind `usage`; a computer already in `doctoring` sends
  `doctor_running_line(name)` kind `conflict`. Otherwise the id goes in `doctoring`, `doctor.run({placeId,
  doctorId, project?})` is awaited, the reply is `{"id","ok":true,"code"}`, and the id leaves `doctoring` whether
  the run answered or failed. `doctoring` is one set per server, not per socket.
- P9. `places.door`: not own road sends `PLACE_DOOR_REFUSAL` (no kind); no door control sends
  `PLACE_DOOR_UNSERVED`; else `door.open()` less its `backPort`, with the place door's `hostKey` last:
  `{"id","ok":true,"door":{...view,"hostKey"}}` (serve.ts:1008-1022).
- P10. `places.mint`: host only, else `MINT_JOIN_REFUSAL`; no door control `PLACE_DOOR_UNSERVED`. The host key,
  then `door.open()`, then the code (`issue_code(false)`), so a host that cannot answer a join mints no code. Reply
  `{"id","ok":true,"joins":join_roads(join_token(code, hostKey), addresses, relay),"expiresAt":<ISO string>}`
  (serve.ts:1023-1038).
- P11. `places.sshHosts`: host only, else `SSH_HOSTS_REFUSAL`; `hosts` is `ssh_hosts(rows)` over the place rows
  (none with no place door), `[]` with no `ssh_hosts` door (serve.ts:1039-1047).
- P12. `places.add` (serve.ts:1048-1077): gated; no door control `PLACE_DOOR_UNSERVED`; the recipe is read by name
  **before** the door opens, so an unsaved name dials nothing; the add is asked with, in this order: `addId`?,
  `address`, `name`?, `sshPort`?, `keyPath`?, `hostKey`?, `sudoPassword`?, `hostUrls` (the door's addresses then
  its relay), `doorPort` (the door's `backPort`)?, `relay`?, then `choices` and `recipe` from the saved recipe?
  Reply spreads the add's answer.
- P13. `places.setup`: gated; a named recipe gives `{choices: file, recipe: slug}`, else the frame's `choices`, else
  nothing; then `addId`? and `row`? Reply spreads `set_up(ref, ..)` (serve.ts:1078-1084).
- P14. `recipes.*` (serve.ts:1090-1127): not own road sends `RECIPES_TICKET_REFUSAL` kind `ticket`. The shelf is
  `opts.recipes`, else the runtime's, else `NO_RECIPES` through the catch. A recipe's view is
  `{"name","slug","summary":recipe_summary(file),"machines":followers[slug] or [],"savedAt"?,"file"}`; followers
  are the place door's, or none with no place door.
  - `recipes.list`: followers read first, then every held recipe's view: `{"recipes":[..]}`.
  - `recipes.get`: `{"recipe","hash"}`.
  - `recipes.save`: with `from`, the place word's id (`place_for`); a word naming this computer refuses
    `recipe_from_here_refusal(word)` kind `usage`; no picks there refuses `no_picks_refusal(word)` kind `usage`.
    The picks with `name` set, else the frame's `file` with `name` set, are saved. With `from`, the computer then
    follows the new slug; without, every follower takes the change (`recipe_changed(slug)`) where a place door is
    wired. Reply `{"recipe"}` with followers read after the save.
  - `recipes.remove`: followers read **before**; `check: true` reads the recipe and removes nothing and unfollows
    nothing; otherwise removes, then unfollows its slug where a place door is wired. Reply `{"recipe"}` with the
    followers read before.
  - `recipes.options`: the projects on `HERE_PLACE_ID` as `{name, path}`, then `options(here)`: `{"options"}`.
- P15. `account.get`: not own road `ACCOUNT_TICKET_REFUSAL` kind `ticket`; no account door `ACCOUNT_UNSERVED`;
  else `{"id","ok":true,"account"}` (serve.ts:1128-1139).
- P16. `devices.list`: not own road `DEVICES_TICKET_REFUSAL` kind `ticket`; else `{"devices"}`.
- P27. `devices.revoke`: not own road `DEVICES_TICKET_REFUSAL` kind `ticket`; a paired computer may revoke any
  device, its own included. The record goes first, then the reply `{"id","ok":true,"revoked"}`, then the sockets
  that device held are cut, so a device that revoked itself reads its answer before its socket goes
  (serve.ts:1147-1158).
- P17. `ticket.issue`: a stamped socket (a ticket's or a thread's) sends `RELAY_TICKET_REFUSAL` kind `ticket`.
  Otherwise 24 random bytes in base64url, `expiresAt = now() + ticket_ttl_ms`, kept with the purpose and whether
  this socket is a paired computer's; reply `{"id","ok":true,"ticket","expiresAt"}` (serve.ts:1159-1169). The
  ticket never contains the host token.

## Rules against tests (`tests/serve_places.rs`)

| Rule | Tests | TS case |
|---|---|---|
| gates | `every_places_op_is_refused_kind_ticket_on_a_ticketed_socket` (one loop over the gated ops), `host_only_ops_refuse_a_device_socket_in_their_own_sentence` | serve-refusal.test.ts:164, pairing.test.ts |
| P1 | `auth_again_names_the_release_again` | serve.test.ts:54 |
| P2 | `pair_issue_answers_the_code_and_the_host_key`, `pair_issue_on_a_device_socket_is_refused` | pairing.test.ts |
| P3 | `identity_ops_once_in_are_refused_and_the_socket_stays` | serve.ts:898-917 |
| P4 | `places_list_answers_rows_adds_and_pending`, `places_list_for_a_thread_reads_its_places_and_no_adds` | places.test.ts, spawn-guard.test.ts |
| P5 | `places_remove_passes_only_the_flags_sent`, `places_login_landed_answers_after_the_door` | places-remove.test.ts |
| P6, P7 | `places_set_reads_a_recipe_word_as_its_slug_and_none_as_none`, `places_follow_none_stays_none` | places-settings.test.ts |
| P8 | `doctor_says_each_refusal_without_reaching_the_road`, `doctor_refuses_a_second_road_on_one_computer_until_the_first_answers`, `doctor_unserved_says_so`, `doctor_frees_the_computer_when_the_road_fails` | serve.test.ts:1289, 1296, 1310 |
| P9 | `places_door_answers_the_view_with_the_host_key_and_no_back_port` | places-door.test.ts |
| P10 | `places_mint_opens_the_door_before_minting_a_code` | places-door.test.ts |
| P11 | `places_ssh_hosts_answers_none_without_a_door` | places.test.ts |
| P12 | `places_add_reads_the_recipe_before_dialling_and_passes_fields_in_order` | places-install.test.ts |
| P13 | `places_setup_prefers_the_named_recipe_over_choices` | places-setup.test.ts |
| P14 | `recipes_list_get_save_remove_options`, `recipes_save_from_here_is_refused_usage`, `recipes_remove_check_unfollows_nothing`, `recipes_are_refused_kind_ticket_off_the_own_road` | places-settings.test.ts |
| P15, P16 | `account_get_unserved_and_ticketed`, `devices_list_on_a_ticket_is_refused` | serve-refusal.test.ts:164 |
| P27 | `devices_revoke_answers_before_cutting_its_own_socket` | pairing.test.ts |
| P17 | `ticket_issue_answers_a_ticket_without_the_token_and_ttl_five_minutes`, `a_ticketed_socket_cannot_mint_a_ticket`, `a_ticket_minted_by_a_paired_socket_carries_the_paired_road` | serve.test.ts:311, serve-refusal.test.ts:164 |

Not ported here, each its area's: what `places.list` rows hold, the add's install, the setup's run, the doctor's road.
