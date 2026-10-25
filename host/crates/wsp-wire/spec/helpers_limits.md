# helpers_limits
Source: packages/protocol/src/wire/helpers.ts (109 lines) and limits.ts (198 lines) at a3b870a6e. Tests: protocol/test/callback-relay.test.ts (6 cases, 2 of them `it.each` over 19 rows), port-forwards.test.ts (1), runtime/test/box-keep.test.ts (1 assertion), and 12 cases written here against values the TS module printed. Rust: `src/wire/helpers.rs`, `src/wire/limits.rs`; tests `tests/helpers_limits.rs` (20).

## What it does
The small rules every other wire schema reads: what a URL a guest hands the laptop may be, what a path or a name from a machine may be, the base64 and cgroup and watch-name shapes, the caps and timings the runtime shares, and the serde helpers that give zod's `.catch` and JavaScript's key order their Rust form. Nothing here holds state or touches a disk or a socket.

## Behaviour
1. `is_http_url(url)`: false past `HTTP_URL_MAX` = 8192 UTF-16 units (limits.ts:115); false unless it matches `^https?://[^\s\x00-\x1f\x7f]+$` case-insensitively, where `\s` is JavaScript's (it holds U+00A0, U+1680, U+2000..U+200A, U+2028, U+2029, U+202F, U+205F, U+3000, U+FEFF besides ASCII space, \t \n \v \f \r) (limits.ts:11); and false unless the WHATWG URL parser takes it (limits.ts:117-121). Use the `url` crate, which is WHATWG. wsp_frames::is_http_url is NOT this rule (no cap, no parse) and must not be called.
2. `is_plain_path(path)` (limits.ts:131-134): starts with `/`, every char in `A-Za-z0-9 ._+@:,/-`, no `//`, no part equal to `.` or `..`. `/` alone is plain.
3. `is_under_path(path)` (limits.ts:139-141): non-empty, no leading `/`, no part empty, `.` or `..`. A backslash is an ordinary character.
4. `host_of(url)` (limits.ts:144-150): the WHATWG `host`: lower-cased, punycoded, IPv4 shorthand expanded, `[..]` kept round IPv6, the port kept unless it is the scheme's default, userinfo dropped, a trailing root dot kept; `Some("")` for a URL the parser takes that has no host (`file:///etc`, `mailto:x@y`); `None` only where the parser refuses.
5. `redirects_to_machine(url)` / `callback_port_of(url)` (limits.ts:163-188): parse `url`, read the FIRST `redirect_uri` query value (form-decoded), parse that; the target's hostname must be one of `localhost`, `127.0.0.1`, `[::1]` after the parser normalised it (so `LOCALHOST` and `[0:0:0:0:0:0:0:1]` count, `0.0.0.0` and `127.0.0.2` do not). `callback_port_of` also needs an explicit, non-default port in 1024..=65535; any parse failure is false / None.
6. `agents_kept_on(mem_mb)` (limits.ts:59-61): `min(6, max(0, floor(mem_mb * 0.25 / 400)))`.
7. `link_backoff_ms(attempt, random)` (limits.ts:103-105): `min(10000, 500 * 2^max(0, attempt - 1)) + floor(random * 250)`; the TS reads `Math.random()` itself, the Rust takes it as `random` in [0, 1). A huge attempt saturates at 10000, never overflows.
8. `base64_length(n)` = `ceil(n / 3) * 4` (helpers.ts:99). `is_base64_of(text, n)` (helpers.ts:101-106): length at most `base64_length(n)`, matches `^[A-Za-z0-9+/]+={0,2}$`, length a multiple of 4, and `len / 4 * 3 - pad == n` where pad is 2 for a trailing `==`, 1 for `=`. Nothing is decoded: the last symbol's spare bits are not checked.
9. `is_cgroup_path` (helpers.ts:71-74): at most 512 UTF-16 units, starts with `/`, every `/`-part after the first char non-empty and not `.` or `..`; so `/` and `/a/` are refused.
10. `is_watch_name` (helpers.ts:68): `^[A-Za-z0-9_-]{1,64}$`, ASCII only.
11. `listed(subject, word, options, value, legacy, hidden)` (helpers.ts:45-50): Ok when `value` is None or equals the `value` of any option in the three lists; otherwise Err with `kind: "usage"`, `offered: options.len()` and the message `<subject> takes no <word>` when `options` is empty, else `<word> "<value>" is not one <subject> takes; one of: <label> (<value>), ...` followed by `; legacy: <label> (<value>), ...` when `legacy` is non-empty. Hidden options are never named.
12. `js_key_order(map)` (rule 4.26): keys that are canonical array indexes (decimal, no sign, no leading zero unless `0` itself, value at most 4294967294) move to the front in ascending numeric order; every other key keeps its relative place; inner maps are untouched.
13. `or_none` (zod `.catch(undefined)` on an optional field): absent, null and any value that fails to read as T all give None; the surrounding struct still reads.
14. `PortForward` (limits.ts:197) needs all five fields; `port` is a RelayPort (wsp_frames: an integer 1024..=65535), `kind` one of url, callback, editor.

## Edge cases
- `is_http_url("http://")` false (nothing after the scheme); `"http://é.test"` true (non-ASCII is not `\s`).
- An `https` redirect on `localhost:443` names the default port, so it returns to the machine but `callback_port_of` is None (printed by the TS module).
- `agents_kept_on` of a negative figure is 0. NaN is not reachable from the wire (a report's memory is an int); the TS answers NaN and the Rust answers 0 (Decisions).
- `listed` with `Some("")` and no options refuses: `cursor takes no effort`.

## Behaviour nobody wrote down
- Rule 4.25: every number constant here is an integer except `KEPT_MEMORY_SHARE` (0.25), which is f64.
- Rule 4.26: `js_key_order` is the crate's one helper for index-like keys; every other map keeps insertion order through serde_json's `preserve_order`.
- Rule 8.3 (text units): every length cap in this unit counts UTF-16 units, as a JavaScript string's `.length` does: `HTTP_URL_MAX`, the cgroup 512.

## Tapes
None of its own: these rules surface through `browser.open`, `callback.port`, `localhost.url` (daemon events), `forwards.list`/`forwards.stop` and every frame naming a path. Held out: none counted here.

## Decisions
1. `link_backoff_ms` takes the random draw as an argument rather than reading a global RNG, so the jitter is testable; the caller passes `rand::random::<f64>()` or its own.
2. `is_http_url` takes `&str`: the TS accepts `unknown` and answers false for a non-string, which is the caller's match in Rust.
3. `agents_kept_on(NaN)` answers 0 (the `as i64` of a NaN floor), where the TS answers NaN.
4. `PermissionPrompt` and `PortCloseDetail` are structs where the TS has field bags spread into other schemas; the views and events flatten them (`#[serde(flatten)]`).
5. `double_option` lives in crate::types::check (the views writer's); helpers keeps `or_none` alone.

## After the port
- `link_backoff_ms` for attempt 1 and attempt 0 is the same 500 ms; a first retry and a zeroth one cannot be told apart. Kept.
