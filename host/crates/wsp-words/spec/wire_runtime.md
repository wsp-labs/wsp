# wire_runtime
Source: `threadOpRefusal` of `packages/protocol/src/wire/runtime.ts` (1431 lines) at a3b870a6e, :1247-1249; the rest of the file is wsp-wire's `wire::runtime`. Tests: no TS test; `pinned::a_joined_computer_that_holds_work_is_forgotten_in_its_own_words` in `tests/view_wire_words.rs`.

## What it does
The one sentence a thread's own token is refused with for an operation no thread may ask for.

## Behaviour
1. `thread_op_refusal(op, thread_id, on)`: "<op> is not a thread's to ask for; this request came in on the token of <threadAt(thread_id, on)>, and a thread's token starts work only under its own root and reads only that tree". `threadAt` (words/computer.ts:260) is "thread <threadWord(id)>" and " on <on>" where on is given; `threadWord` is the id's first 8 UTF-16 units.

## Edge cases
- `("send", "01HABCDEFGHJ", None)` names "thread 01HABCDE"; with on "spoo", "thread 01HABCDE on spoo" (pinned).

## Behaviour nobody wrote down
- Rules 8.4, 8.5: the id cut counts UTF-16 units and never leaves half a pair.

## Tapes
Every runtime op a thread token asks for outside its tree.

## Decisions
1. `on` is `Option<&str>` for the TS `string | undefined`.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `pinned::a_joined_computer_that_holds_work_is_forgotten_in_its_own_words` |
