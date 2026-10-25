# devices
Source: packages/runtime/src/devices.ts (266 lines) at 65f6b89c7. Tests: packages/runtime/test/pairing.test.ts "one writer over the device records" (3 cases, :454-489) and the store facts of "pairing codes" (:163-194); the rest of pairing.test.ts drives `serveRuntime` and belongs to `wsp-serve`. Rust: `src/devices.rs`, tests `tests/devices.rs`.

## What it does
Pairing codes and the device tokens they mint, kept in the store so a restart does not lock a paired computer out. A code is one use; a device token is handed over once and only its sha256 is kept; every reading of a token compares hashes in constant time. A thread's turn token is a device too, with a `scope`. A revoke of a device the account admitted remembers its key.

## Behaviour
1. Collections (:12-20): `devices` keyed by device id, `pairings` keyed by the code, `revoked-keys` keyed by key fingerprint. All survive a restart; nothing is held in memory but the write chain.
2. `safe_equal(a, b)` (:23-27): false when the UTF-8 byte lengths differ, else a constant-time compare of the bytes. `token_digest(t)` (:90): sha256 of the UTF-8 bytes, lowercase hex.
3. Guards on what the store holds (:80-86): a device needs string `id` and string `tokenHash`; a pairing string `code` and number `expiresAt`; a revoked key string `fingerprint`. A row failing its guard reads as absent (`get`) or is skipped (`list`).
4. `mint_code` (:94-99): `PAIR_CODE_LENGTH` (8) random bytes, each `byte & 31` indexing `PAIR_CODE_ALPHABET`.
5. The write chain (:171-177): `issue`, `redeem`, `spend`, `admit`, `admit_account`, `mint`, `seen` and `revoke` run one at a time in call order, each after the one before settled. A failure rejects its own caller and the chain goes on. `match`, `list`, `refuses` and the listing inside `revoke_asides` run outside it and never write.
6. Making a device (`admit`, :180-199), in this order: the token is 24 random bytes as base64url without padding (32 characters); the id is `d_` and 8 random bytes as hex (16 characters), token drawn first; `at` is `now` as `toISOString()` (`1970-01-01T00:00:01.000Z`). The record's keys in order: `id`, `name` (`js_trim(name)`, or `a paired computer` when that is empty), `tokenHash`, `createdAt = at`, `lastSeenAt = at`, then `scope` when given, `road` only beside a scope, `aside: true` only beside a scope, `here: true`, `via`. It is `put` to `devices`; the answer is `{device_id, device_token, device: view}`.
7. `view` (:101-109): `id, name, createdAt, lastSeenAt`, then `scope`, `here: true`, `via` where set; never `tokenHash`, `road` or `aside`. `held` is the view plus `road` where set (:111).
8. `issue({now, ttl_ms, here})` (:222-231): first every pairing with `now > expiresAt` is deleted, in list order; then a code, `expiresAt = now + ttl_ms`, and the record `{code, expiresAt, here?: true}` is put. A code equal to a held one overwrites it.
9. Spending a code (:203-208): `get`; absent or not a pairing gives nothing; otherwise the row is deleted **before** the expiry is read, and the record is the answer only when `now <= expiresAt` (a code exactly at its expiry still spends). So a guess is never tried twice.
10. `redeem(code, name, now)` (:232-236): a spent code makes a device with `here: true` where the code carried it; else `None`. `spend(code, now)` (:237): true when the code was held and unexpired.
11. `admit(name, now)` (:238) makes a plain device; `admit_account(name, via, now)` (:239) one with `via`; `mint(name, scope, now, more)` (:240) one with `scope`, and `road`, `aside` from `more`.
12. `revoke_asides()` (:241-243): lists devices outside the chain, then for each with `aside: true`, in list order, awaits `revoke(id)`.
13. `refuses(fingerprint)` (:244): a `revoked-keys` row passing its guard.
14. `match(token)` (:245-252): digests the token, compares it with **every** device's `tokenHash` by `safe_equal`, keeps the first match without leaving the loop early (so the time does not say where the token sat), and answers its held form; none, `None`. It writes nothing, not even `lastSeenAt`.
15. `seen(id, now)` (:255-262): in the chain, `get`; not a device gives `None` and writes nothing; else the record with `lastSeenAt = toISOString(now)`, other keys in place, is put and its held form answered.
16. `list()` (:263): devices sorted by `createdAt` with `wsp_words::collate` (`localeCompare`), a stable sort, then their views.
17. `revoke(id)` (:210-219): in the chain, `get`; not a device gives `false`; a device with `via` first puts `{fingerprint}` under its fingerprint in `revoked-keys` (key before record, so a failure between leaves the key refused), then the device row is deleted; `true`.
18. `thread_of(device)` (:40-42): no scope gives `None`; else `{by: scope, road: road or "relayed"}`. A scoped record with no road is one an older host wrote for a machine.
19. `NO_DEVICE_DOOR` (:163): `this runtime keeps no devices; the host that serves the app wires one`.

## Edge cases
- `issue` at `now = 1000`, ttl 60_000, redeemed at 61_000: spent; at 61_001: refused and the row is gone (`a_code_redeemed_at_its_expiry_spends_and_one_past_it_is_refused_and_forgotten`).
- Name `"  "` gives `a paired computer`; `"  laptop "` gives `laptop`.
- `mint` with `more.road` but the scope always set: road kept; `admit` never carries `road` or `aside`.
- Two redeems of one code at once: one device (`two_sockets_spending_one_code_get_one_device_between_them`).
- A revoke landing while a `seen` is queued: the device stays revoked and `seen` answers `None` (both orders).

## Behaviour nobody wrote down
- Rule 3.9, chain kind, one chain for the whole host, caller gets its own result (03 table: devices).
- Rule 2.15: `d_` with 16 hex, device tokens 24 bytes base64url.
- Rule 3.16: every random byte comes from the `Ids` seam, so a seeded replay mints the same codes, ids and tokens.
- Rule 2.27: the store parses every read afresh; a record written here comes back with its keys in the order of Behaviour 6.

## Decisions
1. The door is a trait `DeviceDoor` (one implementation, `make_devices`), since `serve` holds it as an option and its tests hand in none (`NO_DEVICE_DOOR`).
2. A `devices` row that passes the guard but does not deserialize as a record (a missing `createdAt`) is skipped as a guard failure; the TS would carry it with `undefined` fields.

## After the port
- `list` sorting a record with no `createdAt` throws in the TS (`localeCompare` of `undefined`); the port skips it (Decision 2).
- Expired codes are only cleared by the next `issue`; a host that never mints again keeps them.

## Tests
| Rule | Test (`tests/devices.rs`) |
|---|---|
| 5, 15, 17 | `a_revoke_that_lands_while_a_last_seen_write_is_in_flight_stays_revoked`, `holds_in_the_other_order_too_and_a_seen_after_a_revoke_writes_nothing_back` |
| 5, 9, 10 | `two_sockets_spending_one_code_get_one_device_between_them` |
| 4, 8 | `issue_mints_eight_symbols_of_the_alphabet_and_stores_the_code_with_its_expiry`, `clears_codes_that_ran_out_when_the_next_one_is_minted` |
| 9 | `a_code_redeemed_at_its_expiry_spends_and_one_past_it_is_refused_and_forgotten`, `spend_answers_true_once_and_false_after` |
| 2, 6, 14 | `keeps_only_the_tokens_hash_never_the_token`, `match_finds_the_device_a_token_names_and_writes_nothing` |
| 6, 7 | `a_device_record_keeps_its_keys_in_the_written_order`, `a_blank_name_reads_a_paired_computer` |
| 10 | `a_code_issued_here_admits_a_device_read_as_here` |
| 11, 18 | `mint_scopes_a_token_to_its_thread_with_its_road_and_admit_never_carries_one`, `thread_of_reads_relayed_for_a_scoped_device_with_no_road` |
| 12 | `revoke_asides_takes_away_only_the_side_question_tokens` |
| 13, 17 | `revoking_an_account_device_remembers_its_key_and_refuses_it_after` |
| 16 | `list_orders_devices_by_when_they_were_made` |
| 2 | `safe_equal_is_false_for_different_lengths_and_true_for_equal_bytes` |
