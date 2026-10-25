# semver
Source: `packages/protocol/src/semver.mjs` (47 lines) at a3b870a6e. Tests: none in TS; `tests/semver.rs` (2 cases, 21 rows, every one printed by node).

## What it does
The one version order: semver precedence over a loosely formed string, with no validation.

## Behaviour
1. Build metadata after the first `+` is dropped; the core is up to the first `-`, split on "."; the prerelease, where there is a `-`, is the rest split on "." (semver.mjs:11-18).
2. Core parts compare pairwise up to the longer length, a missing part reading "0" (semver.mjs:34-37).
3. An identifier pair: both `^\d+$` compare as `Number()` (f64, so 20-digit numbers may tie); a number is below a non-number; else UTF-16 order (`a < b`) (semver.mjs:22-27).
4. No prerelease is above a prerelease; two without are equal (semver.mjs:38).
5. Prereleases compare pairwise; the one that runs out first is below (semver.mjs:39-45).

## Edge cases
- `("", "0")` is Greater: "" is not numeric and a word is above a number. `("v1.0.0", "1.0.0")` is Greater for the same reason.
- `("1.0.0-", "1.0.0")` is Less: an empty prerelease is still a prerelease.
- `("1.0.0-01", "1.0.0-1")` is Equal; `("1.0.0-B", "1.0.0-a")` is Less (upper case first).
- `"99999999999999999999.0.0"` equals `"99999999999999999998.0.0"` (f64).
- A char above U+FFFF sorts below U+FFFF (rule 8.8).

## Behaviour nobody wrote down
- Rule 8.8: text order is `utf16::cmp`.

## Tapes
`release.get`, the shell notice: indirectly.

## Decisions
1. Answers `Ordering`, not a signed number: every caller reads only the sign (`compareVersions(...) < 0`, `=== 0`).

## After the port
- A leading `v` makes a version newer than its bare form.

## Rules pinned
| Rule | Test |
|---|---|
| 1 to 5 | `orders_cores_numerically_and_a_release_above_its_prereleases` |
| edge cases | `reads_malformed_versions_as_the_ts_does` |
