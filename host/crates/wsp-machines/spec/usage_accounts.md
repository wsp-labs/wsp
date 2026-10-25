# usage_accounts: accounts, computer names, the burn, the price table and the account rows

TS: `usage.ts:111-160, 181-201, 457-618`. Rust: `src/usage/accounts.rs`. TS tests: usage-ledger.test.ts groups "an
account's banked resets" (2 of 6), "the rate table" (1), "a computer in the usage records" (3), "an account's key and
label" (2), "the account rows" (6), "what an account draws right now" (1); account/usage tests read them through the
runtime. Tests: `tests/usage_accounts.rs`.

## Rules

C1. `account_of(o)` (usage.ts:117-122), first match: a named account `{key: "<agent>:<id>", label: <label, else id>,
road: named}`; a vault token `{"<agent>:vault-token", account_words({agentName, vaulted: true}), vault}`; a vault key
`{"<agent>:vault-key", account_words({agentName, keyed: true}), vault}`; else `{"<agent>@<computer id>",
account_words({agentName, ownOn: computer name}), own}`.
C2. `account_on_computer(o)` (usage.ts:127-130): the first reading of the agent that ran on the computer on a road
other than the vault; else, where the vault holds a key, the first that ran there on the vault; else `account_of` with
nothing vaulted. Answers its key and label.
C3. `usage_computer_name(places, id, kept)` (usage.ts:135-140): a listed place's `place_name`; this computer's
`here_name`, else `THIS_COMPUTER`; a provider key's `provider_key_name`; a kept name; else
`USAGE_WORDS.removedComputer`. Never an id or a hostname.
C4. `kept_computer_names(store, places)` (usage.ts:146-160): reads every `usage-computers` document with a string id
and name; writes `{id, name}` for each listed place that is not this computer nor a provider key and whose name
changed; answers the kept names, the stored first in store order, new ones after.
C5. `RESET_DETAILS_MS` is 24 hours; `reset_details_due(held, now)` (usage.ts:194-195): never for a keyed account; due
where no full read is held or the last was at least 24 hours ago.
C6. `row_credits(held)` (usage.ts:198-201): the held credits with `nextExpiresAt`, the soonest `expiresAt` of an
`available` credit, where any.
C7. The burn (usage.ts:472-499), memory only. `BURN_WINDOW_MS` is 15 minutes. A call is filed at its receipt, or, for a
replayed run, at `min(receipt, newest receipt - (newest stamp - its stamp))` against that run's newest stamp (a stamp
older than the newest moves nothing). A call filed at or before `now - window` is gone; a run with no call left
forgets its newest stamp. `of(account)`: none without calls; else `{tokensPerMinute: sum / 15, threads: <distinct
threads>}`.
C8. The price table (usage.ts:503-537): fresh while `now - fetchedAt < PRICES_TTL_MS`. A fresh table answers at once.
Else one read at a time (a second caller joins). The first read loads the kept blob `prices/litellm` (`{fetchedAt,
table}`; a non-number `fetchedAt` or a non-object table is ignored) and answers it where fresh. Else it fetches;
a table with rows is kept with `fetchedAt` now and written back; an empty one is ignored silently; a failure warns
`the price table was not read (<message>); no row is priced` or `...; the last one read is used`. Answers the held
table or `{}`.
C9. `account_names(o)` (usage.ts:543-559): each reading's label by `account_words({agentName, keyed, plan, planBrand,
named (named road: its label), vaulted (vault road), ownOn (own road: its first computer's name)})`. Where two readings
read the same, a named one gets ` as <label>` (unless it already ends so) and an own one ` on <its first computer's
name>`; a vault one stays.
C10. `account_rows(o)` (usage.ts:564-618): one row per reading `{key, agent, label, computers (names, once), plan?,
windows? and readAt? (only where it has windows), status? (with windows), note? (without windows: USAGE_WORDS.keyed
for a keyed account, unread where the agent prints limits, else noLimit), address? (named road: the label), credits?
(C6)}`. Then every sign-in a place reports, `none` skipped: a `vault-key` state with the vault holding something is
that vault account (C1), else the account the computer runs on (C2); a new row is `{key, agent, label, computers: [],
note}` (note keyed only for a vault key); each computer name once. Then each row's burn where it has one. Sorted by
agent, then label, both `localeCompare`.

## Tests

| Rule | Test |
|---|---|
| C1 | `names_the_account_a_harness_named_else_the_vaults_token_or_key_else_the_computers_login` |
| C2 | `the_account_on_a_computer_is_the_one_its_turns_named_else_the_vault_key_else_its_login` |
| C3 | `a_computer_reads_as_its_name_never_a_hostname_or_id` |
| C4 | `keeps_a_joined_computers_name_and_writes_only_a_change` |
| C5 | `is_due_a_read_in_full_where_never_or_a_day_ago_and_never_on_a_key` |
| C6 | `puts_the_resets_on_the_row_with_the_soonest_available_lapse` |
| C7 | `draws_the_last_fifteen_minutes_per_minute_and_counts_threads`, `files_a_replayed_run_by_its_own_stamps`, `forgets_a_runs_newest_stamp_once_its_calls_are_gone` |
| C8 | `is_read_once_a_day_and_kept_for_when_the_network_is_gone`, `two_callers_share_one_read`, `an_empty_fetch_keeps_the_last_table_quietly` |
| C9 | `keeps_the_address_where_two_accounts_would_read_the_same`, `keeps_the_computer_where_two_own_logins_would_read_the_same` |
| C10 | `lists_the_vault_token_once_across_computers`, `carries_the_address_a_named_account_signed_in_as`, `reads_a_vault_key_as_a_sign_in_with_no_plan_window`, `folds_a_computers_login_into_the_vault_key_its_turns_ran_on`, `names_each_computer_once_two_removed_ones_too`, `sorts_by_agent_then_label` |
