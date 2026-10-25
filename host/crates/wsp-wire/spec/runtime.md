# runtime
Source: packages/protocol/src/wire/runtime.ts lines 1030-1431 (402 lines) at a3b870a6e, less threadOpRefusal, which went to wsp-words. Tests: protocol.test.ts (7 cases), exit.test.ts (1), redact.test.ts (2), slate/wire.test.ts (1), broker-contract.test.ts (1); 5 more pinned here against what the TS module printed. Rust: `src/wire/runtime.rs` (below RuntimeRequest); tests `tests/runtime.rs` (17).

## What it does
What the host answers and pushes beside the op switch: the reply envelope (ok with the op's own keys, or a refusal with kind and fix), the exec stream's two events, every session op's outcome shape, the provider key and broker status answers, and the readings the door and the event filter take off a raw frame or event: which values of a request are secrets, which workspace an event is about, and which thread it was asked for by.

## Behaviour
1. `request_secrets(frame)` (runtime.ts:1037-1043): a frame that is not an object (a string, null, an array, a number) has none. Otherwise, for each field of `SECRET_REQUEST_FIELDS` in its order (token, key, rows, code, passphrase, env, envs, headers, sudoPassword): a string value is one secret (the empty string included); an object or an array gives its string values in order, one level down only; anything else none.
2. `workspace_id_of(event)` (:1254-1258): the first string among `workspaceId`, `workspace.id`, `status.id`, `forward.workspaceId`; a non-object or none is None (the TS throws on null; the Rust answers None).
3. `asker_of(event)` (:1262-1266): `askedBy.threadId` and `askedBy.rootThreadId` both strings gives `EventAsker` with those two and nothing else; otherwise None.
4. `RuntimeResponse` (:1277-1288): `RuntimeOkResponse` keeps every key after `id` and `ok` in the order written (passthrough); `RuntimeErrorResponse` is `id` (or null), `ok: false`, `error`, optional `kind` and `fix`, written in that order.
5. `ExecEvent` (:1271-1274): `exec.output {execId, text}`; `exec.exit {execId, exitCode, error?}` where `exitCode` is an integer or null and null is written as null (rule 4.27).
6. The session results (:1300-1416): each outcome a closed word list; `SessionInterruptResult` with optional `under`, `error`, `left`; `SessionRenameResult.error` optional; `SessionSettleResult.ended` optional; `SessionSearchResult.unsearched` a positive integer when present; `SessionRewindResult.turns` an integer, `files` optional integer; `SessionStartResult` holds the views' `SessionView`, the outcome (started, steered, queued, held), `turnId`, optional `notice`; `SessionWarmResult.warm` started|standing|none.
7. `ProviderKeyWorks` (:1421): `works` the literal true, `running` and `limit` non-negative integers. `BrokerStatus` (:1426-1430) is copied as its zod reads it.
8. `task_stop_refused_line` and `task_stop_unsupported_line` are plain formats of their arguments.

## Edge cases
- `request_secrets({"key": {"z": {"w": "c"}}})` has none: two levels down is not read.
- `request_secrets({"passphrase": ""})` is `[""]`.
- `workspace_id_of({"workspaceId": 5, "workspace": {"id": "a"}})` is `a`: a non-string is passed over.

## Behaviour nobody wrote down
- Rule 4.27: `ExecEvent.exitCode` and `RuntimeErrorResponse.id` write null, never go missing.
- Chapter 05, 5.7: request_secrets is what the refusal log line redacts, read one level deep, and the log withholds the sentence past 256 secret values (the caller's rule).

## Tapes
Every reply envelope; `workspaces.exec` (exec.output, exec.exit); the session ops' answers. Held out: none counted here.

## Decisions
1. `request_secrets`, `workspace_id_of` and `asker_of` take `&serde_json::Value`, as the TS takes `unknown`.
2. `workspace_id_of(null)` answers None where the TS throws: a thrown reading in the event filter would drop the event from every socket, and no event is null.

## After the port
- `workspace_id_of` throws on a null event in the TS; no caller passes one.
