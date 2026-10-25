# usage_reset

Source: `packages/runtime/src/usage-reset.ts` (183 lines) at 65f6b89c7. Tests: `packages/runtime/test/usage-reset.test.ts`
(19 cases, all ported). `packages/host/test/usage-reset.test.ts` tests the `wsp usage reset` verb, which is `wsp-cli`'s.

## What it does

`usage_resets(deps)` answers the function `usage.reset` calls: spend one banked plan reset of an account on a computer
of the person's that holds its login. Two scripts run there through `deps.run`: a read that checks the login and the
banked count, then the spend under an idempotency key. The key is stored under `usage-reset-pending` before the spend
and deleted once the agent answered, so a press after an unanswered one sends the same key. A second press for the same
account while one runs joins it.

## Behaviour

1. `running` is a map from account to the spend in flight; a call whose account is held answers the held future, else
   it starts `spend` and removes the account when the spend settles, ok or not (:176-182). Memory only.
2. `spend`, in order (:106-174): `limits()`; `limit` = the first whose `key` equals `ask.account`; `row(ask.account)`;
   `own` = `ask.account` matched against `^([^:@]+)@(.+)$` (agent, computer id).
3. `agent` = `limit.agent`, else `row.agent`, else `own[1]`; none: `with_kind(noSuchAccountLine(account), NotFound)` (:111-112).
4. `resets(agent)` none: `with_kind(resetNoneLine(agentName(agent)), Usage)` (:113-114).
5. `label` = `row.label`, else `limit.label`, else `ask.account` (:115).
6. Refused as keyed before anything runs, kind usage, `resetKeyedLine(label)`, when `limit.keyed === true`, or
   `limit.road === "vault"`, or `row.note === USAGE_WORDS.keyed` (:116).
7. `pick` (:86-104), on `places()` read once: `ids` = `[own[2]]` for an own login, else `limit.computers`, else `[]`;
   `mine` = those places whose kind is not `provider`, in `ids` order.
   - `mine` empty: no ids, `resetSignedOutLine(label, agentName)`; else `resetProviderOnlyLine(label,
     providerKeyName(ids[0]), agentName)`; both kind usage.
   - `holding` = `mine` for an own login, else `mine` whose `signed_in(agent)`; empty: usage `resetSignedOutLine`.
   - `ask.on` set: the holding place with that id; none: usage `resetNotOnLine(label, placeOf(on).name, holding names)`;
     not connected: `HostError::new(absentComputer(name, null).sentence)`, no kind.
   - else the first holding place that is connected; none: `absentComputer(holding[0].name, null).sentence`, no kind.
8. `who` = `resetWho({label, own: own matched, computer: place.name})`; `road(agent, place)` (:118-120).
9. Read: `parseRead(run(place, readCommand(road), agent))`. `read.keyed === true`: usage `resetKeyedLine`. No
   `read.limit`: no kind, `resetUnreadLine(name, place.name, label)` (:122-124).
10. Mismatch, only for `limit.road === "named"`: `expected` = `ask.account` from UTF-16 index `agent.length + 1`; refused
    (no kind) unless `signedIn.id == expected` or `signedIn.label == expected`; words `resetMismatchLine(name, place.name,
    signedIn.label ?? signedIn.id ?? "another account", limit.label)` (:125-129).
11. `filed` = `file({agent, place, limit: read.limit})`, always before the count is read (:130).
12. `banked` = `read.limit.credits.count`; absent: no kind, `"<name> on <place> <resets.tooOld>"` (:131-132).
13. `pending` = store `usage-reset-pending`/`account`; `before` = `pending.before` only when it is a number (:133-134).
14. A pending string `key` with a number `before` and `banked < before`: delete the pending row, answer `earlier` with
    `RESET_WORDS.earlier(who, banked)` and the row read under `filed`; nothing is spent (:139-142).
15. `banked == 0`: answer `noCredit` (:143); the pending row is left as it stands.
16. `key` = the pending string key, else `uuid()` (:145). `spendCommand({...road, idempotencyKey: key, creditId?})`; a
    throw becomes a usage error with its message (:146-151).
17. Store `{key, before: before ?? banked}` in that key order, then run the spend (:152-153). A run that fails leaves the
    row (the next press reuses its key and its first `before`).
18. `answered: false`: warn `the reset of <label> on <place> went unanswered; the same request goes again on the next
    press`, fail with no kind `resetSilentLine(name, place.name)`; the row stays (:154-157).
19. Answered: delete the row (:158). `refused` present: `tooOld` true gives `"<name> on <place> <tooOld>"`, else
    `resetRefusedLine(name, place.name, label, markedCut(refused))` (400 UTF-16 units), no kind (:159).
20. `after` = `filed` when the spend carries no limit, else `file({..., limit: spent.limit})` (:160); `left` =
    `spent.limit.credits.count`.
21. Outcome: one of `reset`, `nothingToReset`, `noCredit`, `alreadyRedeemed` as spent, else `unknown`; words per outcome,
    `unknown` with `RESET_WORDS.unknown(name, spent.outcome, who)` (:162-172).
22. The answer is `{outcome, said, account?}`, `account` = `row(key)` only where it is some (:135-138).

## Edge cases

- An own login (`codex@here`) with no limit: `ids = ["here"]`, every place is holding regardless of `signed_in`.
- `ask.on` naming a place the read does not know: `placeOf(on)` still answers a place, and its name is what is said.
- A press after the pending row's count fell (`earlier`) deletes the row; the next press mints a fresh key.
- `creditId` absent: the road carries no `creditId` key at all.

## Behaviour nobody wrote down

- Rule 3.9, join: a second press gets the first press's own answer or error; after it settles the next press runs anew.
- Rule 3.4: no await between `running.get` and `running.set` (the join is taken in the same step it is checked).
- Rule 13.6: `UsageResetDeps` is one trait with the TS method names; `places()` answers a lookup function.

## Decisions

1. `running` holds a `Shared` future; its error is cloned to every waiter (`HostError: Clone`).
2. `places()` answers `Rc<dyn Fn(&str) -> ResetPlace>`; `ResetPlace.signed_in` is `Rc<dyn Fn(&str) -> bool>`.

## After the port

- A spend run that fails (not unanswered) leaves the pending row, so a later press whose read finds fewer banked than
  `before` says `earlier` though nothing was sent (:152-153). Kept.
- `banked == 0` with a pending row keeps the row forever until a press finds `banked < before` (:143). Kept.

## Rules against tests

| Rule | Tests |
|---|---|
| 7 | `the_first_of_the_accounts_computers_that_holds_its_login_and_is_connected`, `the_one_the_person_named_refused_with_the_computers_own_sentence_when_away`, `a_box_that_is_away_when_it_is_the_only_one_refused_with_nothing_run_and_no_key`, `the_computer_an_own_login_is_kept_on`, `refuses_a_login_that_lives_only_on_a_providers_machines_naming_them`, `no_computers_at_all_reads_signed_out` |
| 3, 4, 6 | `an_account_signed_in_by_key_before_any_process_runs`, `the_vaults_key_which_no_turn_has_read_a_limit_for_yet`, `an_account_no_limit_names_and_an_agent_that_banks_none` |
| 9, 10, 12 | `a_computer_whose_login_reads_as_a_key_now`, `a_computer_signed_in_as_another_account_now`, `a_build_whose_read_carries_no_resets_at_all`, `a_read_with_no_limit_says_unread` |
| 15, 16 | `nothing_banked_on_the_read_with_no_spend`, `a_credit_id_the_agent_would_not_take_as_a_usage_refusal_with_nothing_kept` |
| 19, 20, 21, 22 | `says_each_answer_in_its_words_the_reading_after_it_filed`, `lets_go_of_the_key_on_a_refusal_and_says_a_build_with_no_such_request_is_too_old`, `a_refusal_past_400_units_is_marked_cut` |
| 1 | `joins_a_second_press_to_the_one_in_flight`, `a_press_after_a_failed_one_runs_anew` |
| 17, 18 | `keeps_the_key_when_the_spend_went_unanswered_and_lets_go_once_answered`, `keeps_the_key_across_a_run_that_failed_outright` |
| 14 | `says_the_count_fell_since_an_unanswered_press_and_spends_nothing` |
| restart | `a_new_resets_over_the_same_store_sends_the_kept_key` |
