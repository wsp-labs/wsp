# pipe_area
Source: packages/runtime/src/pipe.ts:394-466 (73 lines) at 65f6b89c7. Tests: packages/runtime/test/pipe.test.ts,
"the pipe pool in the runtime" (6 cases). Rust: `src/pipe/area.rs`, `tests/pipe_area.rs`.

## What it does
Runs the pool for the runtime: a brokered workspace's pipes stand while it runs, follow the bus's workspace events,
and are dialled again on a new machine. `cut_off` asks the broker to revoke a machine before its delete.

## Behaviour
1. `pipe_area(ctx, brokered)`: no broker wired answers `None` and dials nothing (pipe.ts:402-404).
2. The pool's `reach(id)`: no live entry `None`; else the entry's daemon reach url and `daemon_token_of(machine)`, `None`
   when the token is. `version(id)`: no entry `None`, else the kind's daemon version. `handler()`: the broker's pipe
   handler when it has one. `ready()`: the broker's. `rewrite(id)`: forget the machine's daemon token, so the next
   reach writes the host's token again. The clock is the context's.
3. `up_if(id)`: up only when the entry exists, its phase is `running` and `brokered(record)`.
4. One `"*"` listener (pipe.ts:429-453): `workspace.creating` with stage `failed` downs, any other stage `up_if`;
   `workspace.created` whose workspace phase is `running` `up_if`; `workspace.woken` and `workspace.upgraded` down then
   `up_if`; `workspace.napped`, `workspace.gone`, `workspace.deleted` down; every other event nothing.
5. Once `ctx.ready()` resolves, `up_if` every live id in order; a rejection does nothing.
6. `close()` detaches the listener, then closes the pool.
7. `cut_off(ctx, entry)` awaits the broker's `revoke(record id)` when both exist; a failure logs `<name> was not cut
   off from your computer before its delete: <message>` and returns normally.

## Behaviour nobody wrote down
- A refusal's rewrite works by forgetting the token entry: the next `daemon_token_of` writes the host's token again,
  so a second writer's token is overwritten a minute later with no wake.

## Decisions
1. The context is the trait `PipeAreaContext`, with the methods the TS reads off `RuntimeContext`, so the fake of the
   TS test is a plain struct here; the wiring implements it over the real context. `brokered` is asked by
   workspace id rather than handed the record, since the record type is the wiring's; the wiring's predicate reads
   the live record of that id.

## Decisions, continued
2. Single-threaded as the pool (`Rc`, `LocalBoxFuture`); `cut_off` warns through `crate::console::warn`.

## Rules against tests
1 `dials_nothing_on_a_host_that_wired_no_broker`; 2, 3 `keeps_4_pipes_to_each_running_brokered_workspace_none_to_a_joined_computer_s`;
4 `closes_a_workspace_s_pipes_as_it_naps_dials_4_again_as_it_wakes_and_none_after`,
`dials_a_brokered_workspace_s_pipes_while_its_create_still_runs_so_its_clone_rea`; 2, 9 of pipe_pool
`writes_the_host_s_token_again_over_another_writer_s_and_has_4_pipes_parked_aga`,
`says_once_that_another_writer_keeps_taking_the_file_back_and_waits_twice_as_lo`.
