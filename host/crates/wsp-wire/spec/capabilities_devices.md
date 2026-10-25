# capabilities_devices
Source: packages/protocol/src/wire/capabilities.ts (165 lines) and devices.ts (258 lines) at a3b870a6e. Tests: protocol.test.ts (6 cases), copy.test.ts (1), init-job.test.ts (1), places.test.ts (6), machine-link.test.ts (3); 8 more pinned here against what the TS module printed. Rust: `src/wire/capabilities.rs`, `src/wire/devices.rs`; tests `tests/capabilities_devices.rs` (25).

## What it does
What a backend says it can do (`Capabilities`, `PlaceCapacity`, the sizes it sells) and the four rules read off them; the shapes of a paired device, an account and its admissions; the pairing code's alphabet and grouping and the join token that carries a code and a host key; the daemon channel frames a page sends through the host; and the refusal sentences of the device, account and pairing roads. No state; the host's device records live in the runtime.

## Behaviour
1. `Capabilities` (capabilities.ts:53-107): fourteen fields, every boolean required (a backend that declares neither copies nor ownNetwork is refused, copy.test.ts:52), `pauseMode` optional and one of `memory`, `disk` (a boolean refused, protocol.test.ts:801), `sizes` a list of `MachineSizeOffer` (`cpu`, `memMb`, `rateUsdPerHour`, optional `name`).
2. `forks_no_machines(c)` is `c.sizes` empty (:120). `builds_images(c)` is sizes non-empty and `disk_snapshots` (:127).
3. `names_size(cpu, mem_mb)` is either present, zero counts (:133).
4. `fork_room(capacity, mem_mb, image_bytes)` (:161-165): `by_memory = floor(mem_room_mb / mem_mb)`, `by_disk = floor(disk_free_bytes / image_bytes)` or +infinity when the image is absent or 0, answer `max(0, min(by_memory, by_disk))` with JavaScript's NaN rules: `0 / 0` is NaN and the answer is NaN, `x / 0` for x > 0 is +infinity. Rust's `f64::max`/`min` drop a NaN, so write the comparisons out.
5. `InitSetup` (:15-39): `pricing` and `job` are nullable and written as `null` when null (rule 4.27); `pricing.builderDiskGb` positive when present.
6. `PlaceCapacity` (:140-156): every number a plain number (JsNumber); `cpuTaken` and `memTakenMb` optional.
7. `ticket_origin` (devices.ts:19): connect is here, relay is relayed.
8. `DeviceView` (devices.ts:39-58): `scope` optional (views' ThreadScope), `here` only the literal `true`, `via` a `DeviceVia` whose `kind` is the literal `account`.
9. `shown_pair_code(code)` (:70-74): remove every `-`, upper-case with the full Unicode mapping (`ß` is `SS`), keep the first 8 UTF-16 units, and if more than 4 remain put a `-` after the 4th. `sent_pair_code` removes dashes and upper-cases, nothing more (spaces stay).
10. `is_pair_code` (:87): exactly 8 UTF-16 units, each in `0123456789ABCDEFGHJKMNPQRSTVWXYZ`.
11. `read_join_token(typed)` (:233-240): trim the whole text (JavaScript's trim: the same white space as rule 1 of helpers_limits); no `.` gives `{code: sent_pair_code(trimmed)}`; else split at the FIRST `.`: the key is the rest trimmed, kept as typed (case and all), None when it trims to empty; the code is `sent_pair_code` of the part before, untrimmed. `join_token(code, key)` is `shown_pair_code(code) + "." + key`; `pair_token` is the same function.
12. `device_admission_transcript(d, b, t)` is the UTF-8 of `wsp device admission v1\n<d>\n<b>\n<t>\n` (:141-143).
13. `DaemonFrame` (:201): `op` a string other than `auth` (a custom issue on ["op"], message `the host authenticates the channel`); every other key kept, in order (`rest`).
14. `DaemonChannelEvent` (:215-218): `daemon.event` with `channel` and an `event` object holding at least a string `type`, every key kept; `daemon.closed` with `channel`, an integer `code` and `reason`.
15. Every refusal constant and sentence function is copied byte for byte (the generator wrote the constants from the TS values).

## Edge cases
- `shown_pair_code("")` is `""`; `"abcdefghijk"` is `ABCD-EFGH`; `"abcde"` is `ABCD-E`.
- `read_join_token("abcd-efgh . key ")` gives code `"ABCDEFGH "` (the space before the mark stays) and key `key`; `".key"` gives code `""`.
- `fork_room` with negative room or disk is 0, never negative.

## Behaviour nobody wrote down
- Rule 4.27: `InitSetup.pricing` and `.job` write `null`, never go missing.
- Rule 4.26: `DaemonFrame` and `daemon.event` keep the page's and the daemon's keys in the order they came; the host never reorders a frame it relays.
- Rule 8.3: `PAIR_CODE_LENGTH` and the cut in `shown_pair_code` count UTF-16 units.

## Tapes
`pair.issue`, `pair.redeem`, `devices.list`, `devices.revoke`, `account.get`, `capabilities.get`, `workspaces.landing`, `daemon.open`/`daemon.send` (chapter 04 and 09's tapes). Held out: none counted here.

## Decisions
1. `forks_no_machines` and `builds_images` take the whole `Capabilities`; the TS takes any object with `sizes` (and `diskSnapshots`), and every caller has one.
2. `names_size` takes the two halves; the TS takes a partial `WorkspaceSize` or undefined.
3. `fork_room` answers f64, NaN and infinity included, as the TS's number does; the caller words it.
4. `TICKET_ORIGIN` is a function of the purpose, since a Rust const cannot hold the views enum before the merge.

## After the port
- `fork_room` with a zero machine size and zero room answers NaN; nothing upstream asks with a zero size today.
