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
