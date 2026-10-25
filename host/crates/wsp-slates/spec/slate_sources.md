# slate_sources
Source: packages/runtime/src/slate-sources/ (index.ts 58, context.ts 38, thread.ts 57, usage.ts 26, pr.ts 35, git.ts 21, cost.ts 10, time.ts 11 lines) at 65f6b89c7. TS tests: none of their own; `test/slates.test.ts` reads `$thread.*` and `$pr.*` through the runtime. Every expected value below was printed by the TS under node (probe `pmisc.ts`). Rust: `src/slate_sources.rs` (module list, then index.ts below it) and `src/slate_sources/*.rs`; tests `tests/slate_sources.rs`.

## What it does
The host's own sources a slate reads by path (`$thread.status`, `$pr.checks[0].name`): six named views built from what the runtime fills per read, walked by name and index.

## Behaviour
1. `SlateSourceContext` (`context.ts:7-18`): `thread_id`, `workspace_id`, `now` (ms), `rows()` (the thread's rows, oldest first), `results()` (its ended turns' results, oldest first), `account()` (async, the account row or none), `workspace()` (the facts or none). `SlateWorkspaceFacts`: `computer, project, folder, checkout, pr, rate_usd_per_hour, accrued_usd`, each optional.
2. `HostSlateSource { name, view(ctx) }`; a view may be async and may answer none ("not there yet").
3. `json(value)` (`context.ts:38`): the value through JSON, so absent fields are dropped and none becomes `null`.
4. `HOST_SLATE_SOURCES` in this order: `thread, usage, cost, time, git, pr`.
5. `path_segments(path)` (`index.ts:18-22`): every match of `([^.[\]]+)|\[(-?\d+)\]`: a name, or a bracketed integer as a number. `pr.checks[0].name` gives `pr, checks, 0, name`; `a[-1]` gives `a, -1`; `a..b[x]` gives `a, b, x`; `""` gives nothing.
6. `walk(value, segs)` (`index.ts:27-40`): none on none; at each step a non-container gives none; an array takes only a number, negative counting from the end, out of range none; an object takes only a string that is its own key and not `__proto__`, `constructor` or `prototype`. `walk(null, [])` is `null`.
7. `view_sources(names, ctx)` (`index.ts:43-51`): each distinct name once, in first-seen order, one after another (each awaited before the next); an unknown name is skipped (no entry); a view that fails gives an entry of none.
8. `resolve_in(views, path)`: the first segment must be a string; walk the rest in that source's view.
9. `thread` (`thread.ts`): `latest` = the row with the greatest `startedAt` (none counts 0), ties to the later row. Status: none `starting`; running with `asking` or `waitingOn` `needs-you`, else `working`; failed `failed`; interrupted `resting`; anything else `done`. The view, keys in this order: `id`; `title` = latest's `harnessTitle`, else its prompt's first line cut to 80 UTF-16 units, else null; `agent` (harness), `model`, `effort`, `status`, `access` (permissionMode); `computer`, `project` (facts); `folder` = latest's cwd, else facts folder; `turns` = row count; `lastTurn {status, durationMs, waitedMs}` of the last result and `{startedAt, endedAt, model}` of the latest row; `cost {usd}` = sum of rows' costUsd; `tokens {input, output, cached, cacheWrite, reasoning}` summed over results with tokens; `context {used, window, free, percent}` from the newest result whose tokens carry `context` (free and percent only with both, percent only when window > 0, `used / window * 100`); `waitingOn` = latest's `asking`. Every absent value is `null`.
10. `usage` (`usage.ts`): none when there is no account. `account {key, label, agent, plan, address, computers}`, `windows` (each `{kind, usedPercent, resetsAt}`), `session` and `week` (`{percent, resetsAt}` of the first window of that kind), `status`, `note`, `burn {tokensPerMinute, threads}`, `credits {count, nextExpiresAt}`, `readAt`; absent values null.
11. `cost`: `{rateUsdPerHour, accruedUsd}`, each 0 when absent.
12. `time`: `{now, today, zone}`: `zone` the host's IANA zone name, `today` the date of `now` in that zone as `YYYY-MM-DD`.
13. `git`: none without a checkout; else `{branch, head (null), ahead, behind, changed, stashes (0), editsUnread (false), countsUnknown (false), readAt}`.
14. `pr`: no pull request seen: `{number: null, word: null, unread: null, checks: []}`; a seen with `why`: `{number: null, word: pull_request_word(seen), unread: why, checks: [], readAt}`; else the fact's fields (`number, url, state, base, draft, branch, headOid, headSubject, mergeable, review, checks, additions, deletions, changedFiles, commits, behindBase, author, readAt, word, unread: null`), the fact-only ones null when the seen is not a full fact (`is_pull_request_fact`), each check `{name, workflow, state, link, description, startedAt, completedAt}` with absent values null.
15. Nothing here holds state; nothing survives or needs to survive a restart.

## Edge cases
- Empty thread: `title null, status "starting", turns 0, cost {usd: 0}`, all tokens 0, context all null.
- A view that throws (an account read that fails) reads as none but its name stays in the map.
- `resolve_in` with a path starting with a bracket (`[0]`) is none.

## Decisions
1. Views are `serde_json::Value` with `preserve_order`, built in the TS's key order (ruling 6402177374 rule 1).
2. `HostSlateSource::view` answers `LocalBoxFuture<Option<Value>>`; a failure is `None` inside `view_sources`.

## After the port
- Node's ICU names `Asia/Kolkata` as `Asia/Calcutta` (probe, TZ=Asia/Kolkata); the Rust host names the zone as the system's tz database does. An accepted difference for the parity list.
- `latest` picks by `startedAt` with ties to the later row, but `lastTurn.status` comes from the last result: they can name two different turns. Kept.

## Tests (`tests/slate_sources.rs`)
| Rule | Test |
|---|---|
| 4 | `the_six_sources_in_order` |
| 5 | `path_segments_reads_names_and_indexes` |
| 6 | `walk_takes_own_keys_and_indexes_and_refuses_unsafe_names` |
| 7, 8 | `view_sources_views_each_name_once_and_skips_unknown`, `a_view_that_fails_reads_as_none`, `resolve_in_reads_a_path_out_of_the_views` |
| 9 | `thread_view_matches_the_ts`, `thread_view_of_an_empty_thread` |
| 10 | `usage_view_matches_the_ts`, `usage_view_with_no_account_is_none` |
| 11, 13 | `cost_and_git_views_match_the_ts`, `git_view_without_a_checkout_is_none` |
| 12 | `time_view_names_the_zone_and_todays_date_in_it` |
| 14 | `pr_view_of_a_fact`, `pr_view_unread_and_unseen` |
