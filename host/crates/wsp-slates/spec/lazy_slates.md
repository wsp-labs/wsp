# lazy_slates
Source: lazy-slates.ts (64 lines) at 65f6b89c7. Tests: test/lazy-slates.test.ts (4 cases).

## What it does
Wraps the slate area so a host with no slate never builds it. The real area is made on the first call that needs
it, or at start where the store holds a slate, so its timed runs recover then. While no slate was stored at start
and none was used since, the turn-following calls answer nothing and make nothing. In Rust there is no code to load;
the factory that builds the area is what is deferred, and tests inject a fake one.

## Behaviour
1. `SLATES` = `"slates"`, the collection every record lives in (lazy-slates.ts:7).
2. `lazy_slates(deps, create)` holds `real` (none until made) and `loading` (the one shared start of making it)
   (:9-12). Making it calls `create(deps)` once, sets `real`, and logs `the slate's code loaded in N ms` (:13-20).
3. `used(f)`: with `real` set, `f(real)` at once; otherwise after `loading` resolves, in call order (:23).
4. `if_any(f, none)` (:26-30): once `loading` has started, `used(f)`. Otherwise `stored` is the store's
   `list(SLATES)` non-empty, read once and kept for the life of the wrapper; after it, `used(f)` when stored was true
   or `loading` started meanwhile, else `none`.
5. Through `if_any`: `ready` (none `()`), `get` (none `null`), `turn_ended`, `rewound`, `forget` (none `()`).
   Through `used`: `write`, `state`, `read`, `catalog`, `shown`, `revoke`, `event`, `approve`, `cancel`, `resolve`,
   `image` (:31-46).
6. `settled` awaits `loading` when there is one, then the real area's `settled`; with none it answers at once (:32).
7. `close`, `close_servers`, `napped` call the real area only when it is made; `watches_pr` answers false until then
   (:33, 59-61).
8. `subscribe(p)` (:47-57): with `real` set, the real area's own. Otherwise it starts making the area and answers a
   release at once; when the area is made, it subscribes `p` unless the release ran first. The release marks itself
   released and calls the inner release when there is one.

## Edge cases
- A host with nothing stored: `ready`, `get` (null), `turn_ended`, `rewound`, `forget`, `watches_pr` (false),
  `settled`, `close` make nothing (test 1).
- A stored record of any shape counts: `{schema: 2, threadId: "t1"}` makes the area at `ready` and calls its `ready`
  once (test 2).
- Two writes at once make one area; both writes and the next `turn_ended` reach it, in order (test 3).
- A subscribe released before the area is made never reaches it; one still held does, and its release reaches the
  inner release (test 4).
- A `list` that fails is kept: every later `if_any` call fails the same way until something has started making the
  area (`stored ??=` keeps the rejected promise, :28).

## Behaviour nobody wrote down
- Rule 3.4: `used` before the area is made awaits; after, it calls straight through with no extra await, so a turn's
  snapshot is taken before the next change lands (:21-22).

## Decisions
1. `create` is `Box<dyn FnOnce(Rc<dyn SlatesDeps>) -> Rc<dyn Slates>>`; the real wiring passes `create_slates`.
2. The log line goes to stdout, as `console.log` does, its ms off `std::time::Instant`, not the clock (it measures
   the host's work, not a time the replay controls).

## After the port
- A rejected `list` at start is remembered for the wrapper's life (:28).

## Tests

20 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| lazy_slates Edge case: lazy-slates.test.ts:30 (is not loaded by a host with no slate, whatever its threads do) | `lazy_slates::is_not_loaded_by_a_host_with_no_slate_whatever_its_threads_do` |
| lazy_slates Behaviour 1 / Edge case: lazy-slates.test.ts:43 (loads at start where a slate is stored) | `lazy_slates::loads_at_start_where_a_slate_is_stored_so_its_timed_runs_recover_then` |
| lazy_slates Behaviour 4 / Edge case: lazy-slates.test.ts:50 (loads on a thread's first write, once) | `lazy_slates::loads_on_a_threads_first_write_once_and_then_follows_the_threads_turns` |
| lazy_slates Behaviour 8 / Edge case: lazy-slates.test.ts:58 (holds a window's sources once it loads, unless the window let go first) | `lazy_slates::holds_a_windows_sources_once_it_loads_unless_the_window_let_go_first` |
| lazy_slates Behaviour 2: making the area calls create once, with the deps the wrapper was given | `lazy_slates::making_the_area_gives_the_factory_the_wrappers_deps_once` |
| lazy_slates Behaviour 3 / 5: each call of `used` makes the area and reaches it, answering what it answered | `lazy_slates::every_used_call_makes_the_area_and_passes_through_its_answer` |
| lazy_slates Behaviour 3: calls made before the area exists run in call order | `lazy_slates::calls_made_before_the_area_exists_reach_it_in_call_order` |
| lazy_slates Behaviour 3 / Rule 3.4: once the area is made a call goes straight through with no extra wait | `lazy_slates::a_call_after_the_area_is_made_runs_straight_through` |
| lazy_slates Behaviour 4: a slate stored of any shape is found by `get`, and the area's answer is passed on | `lazy_slates::get_where_a_slate_is_stored_reaches_the_area` |
| lazy_slates Behaviour 5: where a slate is stored the turn-following calls reach the area with their arguments | `lazy_slates::turn_following_calls_reach_the_area_where_a_slate_is_stored` |
| lazy_slates Behaviour 4: with nothing stored the store is read once and the answer kept | `lazy_slates::a_host_with_nothing_stored_reads_the_store_once` |
| lazy_slates Behaviour 4: once loading has started the store is not read at all | `lazy_slates::once_loading_has_started_an_if_any_call_goes_to_the_area_without_reading_the_store` |
| lazy_slates Edge case: the remembered failed list (lazy-slates.ts:28, `stored ??=` keeps the rejected promise) | `lazy_slates::a_failed_list_is_remembered_and_fails_every_later_call_until_the_area_is_made` |
| lazy_slates Edge case: close, close_servers, napped and watches_pr make nothing (lazy-slates.ts:33, 59-61) | `lazy_slates::close_close_servers_napped_and_watches_pr_make_no_area` |
| lazy_slates Behaviour 7: once the area is made, close, close_servers, napped and watches_pr reach it | `lazy_slates::close_close_servers_napped_and_watches_pr_reach_a_made_area` |
| lazy_slates Edge case: settled with nothing made answers at once and makes nothing | `lazy_slates::settled_with_nothing_made_answers_at_once_and_makes_nothing` |
| lazy_slates Behaviour 6: settled reaches a made area | `lazy_slates::settled_reaches_a_made_area` |
| lazy_slates Behaviour 6: settled waits for an area that is loading, then asks it | `lazy_slates::settled_waits_for_the_loading_area_and_then_asks_it` |
| lazy_slates Behaviour 8: with the area made, subscribe is the area's own and so is its release | `lazy_slates::subscribe_with_the_area_made_is_the_areas_own_at_once` |
| lazy_slates Behaviour 8: a subscribe starts making the area and answers a release at once | `lazy_slates::subscribe_starts_the_area_and_answers_a_release_at_once` |
