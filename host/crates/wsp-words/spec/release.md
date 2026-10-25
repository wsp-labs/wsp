# release
Source: the functions of `packages/protocol/src/release.ts` (58 lines) at a3b870a6e; its zod shapes (`ReleaseLatest`, `ReleaseState`, `HostShape`, `ReleaseView`, `ReleaseChangedEvent`) and flat constants (`UP_RESTART_LINE`, `HOST_NO_RESTART_LINE`, `HOST_RESTART_TICKET_REFUSAL`) are `wsp_wire::types::release`. Tests: the one TS case on these functions is `packages/protocol/test/protocol.test.ts:2076` (`releaseAbove`, five asserts), ported in `tests/protocol_words.rs`; `pinned` in `tests/release.rs` holds the rest.

## What it does
Whether the newest release the host read off GitHub is above a version that runs, and the one word a reading shows: the version number once one was read, else the state of the check.

## Behaviour
1. `release_above(view, running)`: false where the view carries no `latest`; else true where `compare_versions(latest.version, version)` is Greater for at least one of the running versions; an empty `running` list is false (release.ts:46). The order is semver.mjs's, so a running prerelease or a build ahead of the release reads level or above.
2. `release_word(view)`: `latest.version` whenever `latest` is present, else the state word: "checking", "read", "unreached" or "off" (release.ts:49). The fallback is `??`, so a present version of "" is "" and never the state.

## Edge cases
- Latest 0.3.0 against running "0.2.0" is true; against "0.3.0" and "0.2.0" together is true; against "0.3.0" alone is false; against "0.4.0-rc.1" is false (protocol.test.ts:2078-2082).
- A view of `{ state: "checking" }` with no latest is false against "0.2.0" (protocol.test.ts:2083).

## Behaviour nobody wrote down
- Rule 8.8 through `semver`: version identifiers that are not numbers compare in UTF-16 order.

## Tapes
The `release.changed` event and the `release.get` answer carry the view; the host's version line ("<word>; this is <running>, <fix> gets it", packages/host/src/release.ts:99-100); the app's update notice and its shell version check (apps/web/src/shell/update.ts:68, shellVersion.ts:27).

## Decisions
1. Both functions take the whole `ReleaseView` for the TS `Pick<ReleaseView, ...>`.
2. The TS rest parameter `...running: string[]` is a slice, `&[impl AsRef<str>]`.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `pinned::a_release_is_ahead_only_of_a_version_that_runs`, `protocol_words.rs: the_newest_release_as_the_host_read_it::*` |
| 2 | `pinned::a_release_reads_its_version_else_its_state` |
