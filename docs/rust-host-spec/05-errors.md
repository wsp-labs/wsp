# 05 Errors and refusals

How a failure is stamped, classed, worded and sent. One rule runs through all of it: the class is read off the kind stamped on the error where it was born, never off its words (`packages/protocol/src/exit.ts:1-7`; protocol invariant 4). Every `file:line` is at `a3b870a6e`. The second pass (2026-10-25) re-checked every `file:line` and count, recorded the tapes from rule 5.9 on against a build of `a3b870a6e`, and added rules 5.9 to 5.20, each in the section it belongs to.

## Kinds and classes

`ExitClass` is `ok | provider | auth | usage`, with exit codes 0, 1, 2, 3 (exit.ts:12-16). `KIND_CLASS` maps five kinds; every other kind, and an error with none, is the provider's (exit.ts:34-40):

| Kind | Class | Exit | Stamped by | Example |
|---|---|---|---|---|
| `usage` | usage | 3 | `usageRefusal(happened, fix)` (exit.ts:106, 50 calls in `runtime/src`), and by hand at 53 `kind: "usage"` sites in `runtime/src` (61 across `packages/*/src`) | `packages/host/src/verbs.ts:724` |
| `invalid` | usage | 3 | by hand, 31 sites in `runtime/src` (34 across `packages/*/src`) | `packages/runtime/src/project-landing.ts:603` |
| `not-found` | usage | 3 | `notFoundRefusal(message)` (exit.ts:116) | `packages/host/src/recipes.ts:103` |
| `processes-left` | usage | 3 | a settle or delete refused while a thread still runs processes; carries `left` | `protocol/src/views/activity.ts:568` |
| `auth` | auth | 2 | `authRefusal(message)` (exit.ts:112), the door, the engine's 401 | `packages/host/src/hosts.ts:352` |
| `ticket` | provider | 1 | the `*_TICKET_REFUSAL` family: ops only the owner's own road may ask; two sites send the same sentence with no kind (serve.ts:1718,1726) | serve.ts:874,1104,1138,1150,1159,1169,1739,1811,1844,1901; `runtime/src/serve-broker.ts:28` |
| `conflict` | provider | 1 | by hand, 27 sites in `runtime/src` (30 across `packages/*/src`); the engine's 409 | `runtime/src/threads/forks.ts:101` |
| `concurrency`, `plan`, `missing`, `snapshotUnavailable`, `transient`, `unknown` | provider | 1 | the engine's `classify` by HTTP status: 429, 402 or 403, 404, 502 "Failed to snapshot sandbox", 502 to 504, else (`packages/engine/src/errors.ts:13-26`) | `engine/src/errors.ts:16-25` |
| `startLimit`, and `transient` or `concurrency` by provider code | provider | 1 | `boxRefusal` reads the provider's code before the status: `REQUEST_LIMIT_CODE` is `transient`, `START_CODES` is `startLimit` (with `window: "day"` for `daily_limit_reached`), `CAP_CODES` or `CAP_WORDS` is `concurrency` | `engine/src/box-backend.ts:459-466` |
| `notFirstLife`, `machineAlive`, `snapshotFailed`, `credentialOnBuilder`, `noProvider`, `vaultTooLarge`, `exists` | provider | 1 | the engine | `engine/src/errors.ts:70`; `engine/src/golden.ts:46-83`; `engine/src/vault.ts:218,505`; `engine/src/no-provider-backend.ts:22` |
| `sshInclude`, `host-key`, `sshBehind` | provider | 1 | places and ssh | serve.ts:1826; `protocol/src/views/place.ts:65`; `views/preferences.ts:73` |
| `unreachable`, `refused`, `reauth` | provider | 1 | client side only, never on the wire from the host | `hosts.ts:241`; `runtime/src/daemon-channel.ts:39,52` |

So a schema failure (no kind) exits 1, a thread-door refusal (no kind) exits 1, and `ticket` and `conflict` exit 1. These look odd and are kept (14 lists the schema failure's class as an after-port question).

5.1. Every error the Rust host raises MUST carry the kind the TS code stamps at the same site, or none where it stamps none. The class and exit code MUST come from one table equal to `KIND_CLASS`, with every unlisted kind the provider's.

## The failure reply

One catch wraps the whole op switch (serve.ts:1932-1942) and answers:

```
{ id, ok: false, error: failureWords(e), kind?, fix?, errors?, warnings?, left? }
```

5.2. `error` MUST be `failureWords(e)` (`packages/engine/src/errors.ts:335-342`): the error's message; for a `fetch failed` TypeError with a cause, `fetch failed: <cause message>` plus `; <message>` for each error of an AggregateError cause; for a thrown non-Error, its string. `kind` and `fix` are present only when they are strings on the error; `errors` and `warnings` only when arrays (a slate write's validator lists, exit.ts:63-67); `left` only when the kind is `processes-left` (exit.ts:44-47). No `code`, `status` or stack ever reaches a client; `code` and `status` exist only on daemon error frames (`protocol/src/wire/machine-link.ts:531-544`). The wire's own reply schema declares only `id, ok, error, kind?, fix?` with no passthrough (`RuntimeErrorResponse`, `protocol/src/wire/runtime.ts:1279-1287`), so a client that parses a failure through it drops `errors`, `warnings` and `left`; the command line reads them off the raw frame instead (`verbs/client.ts:267`).

5.3. Internal messages DO reach the client today: whatever a handler throws becomes `error`, node's errno sentences included (`ENOENT: no such file or directory, open '<path>'`). The Rust host MUST carry its own error's message in the same place; where a TS errno sentence reaches a tape, the Rust host's equivalent sentence is an accepted difference with this reason, since node's wording cannot be reproduced exactly.

5.4. Replies written outside the catch keep their own shapes: `{id: null, ok: false, error: "invalid json"}` and the non-object sentence (rule 4.8), no kind; the schema refusal, no kind (below); the thread door and paired door refusals, no kind (rule 4.17); the door's `unauthorized`, kind `auth`; `places.readings` naming neither id, kind `usage` with no `fix` (serve.ts:1611-1619).

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"usage.reset","account":"nobody"}}
{"out":{"id":2,"ok":false,"error":"no account nobody; wsp usage lists every account","kind":"not-found"}}
{"in":{"id":3,"op":"sessions.head","threadId":"nope"}}
{"out":{"id":3,"ok":false,"error":"no thread nope","kind":"not-found"}}
{"in":{"id":4,"op":"slates.get","threadId":"nope"}}
{"out":{"id":4,"ok":true,"slate":null}}
{"in":{"id":5,"op":"sessions.read","threadId":"nope"}}
{"out":{"id":5,"ok":false,"error":"no thread nope","kind":"not-found"}}
```

`sessions.head` and `sessions.read` of an unknown thread refuse with kind `not-found`; `slates.get` answers `slate: null`; `sessions.rename` answers `ok: true` with `outcome: "not-found"` (04's tapes). Which of these an op does is per op and is behaviour.

5.9. The failure reply from the catch MUST be built as `{id, ok:false, error, kind?, fix?, errors?, warnings?, left?}` in that key order (serve.ts:1932-1942), where presence is decided by type and nothing else: `kind` and `fix` ride whenever they are strings on the error, the empty string included; `errors` and `warnings` whenever they are arrays, the empty array included; `left` only when `kind` is exactly `processes-left` and `left` is an array (`refusalFields`, exit.ts:44-47; `problemListsOf`, exit.ts:63-67). A `kind` or `fix` that is not a string is left out, never coerced. After the send the host hands the op and the error to its `failed` hook, which writes an `op.failed` analytics record (serve.ts:1941; `packages/host/src/cli/serve.ts:353`); nothing about that reaches the client.

5.10. An unknown id in a lookup MUST answer as its op answers it, and the four ways in use are behaviour: `not-found` with the op's own sentence, `ok: true` with a null value, `ok: true` with `outcome: "not-found"`, or a usage refusal. The sentences differ per op and are not shared:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"workspaces.get","workspaceId":"nope"}}
{"out":{"id":2,"ok":false,"error":"no such workspace: nope","kind":"not-found"}}
{"in":{"id":3,"op":"recipes.get","name":"nope"}}
{"out":{"id":3,"ok":false,"error":"no recipe named nope; none is saved yet","kind":"not-found"}}
{"in":{"id":4,"op":"workspaces.delete","workspaceId":"nope"}}
{"out":{"id":4,"ok":false,"error":"no such workspace: nope","kind":"not-found"}}
{"in":{"id":5,"op":"sessions.delete","threadId":"nope"}}
{"out":{"id":5,"ok":false,"error":"no thread nope","kind":"not-found"}}
{"in":{"id":6,"op":"slates.write","threadId":"nope","text":""}}
{"out":{"id":6,"ok":false,"error":"no thread nope","kind":"not-found"}}
{"in":{"id":7,"op":"sessions.rename","sessionId":"nope","title":"x"}}
{"out":{"id":7,"ok":true,"outcome":"not-found"}}
```

5.11. An empty string where an id or name goes MUST be treated as any other unknown value: no trim, no "empty" sentence, and the empty value lands in the sentence as it is, which leaves `no thread ` with its trailing space. The schema allows the empty string in every one of these fields. `conversations.list` with no project is a `usage` refusal written by hand with no `fix`, so the two-halves rule (5.5) does not hold for every usage site:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"sessions.head","threadId":""}}
{"out":{"id":2,"ok":false,"error":"no thread ","kind":"not-found"}}
{"in":{"id":3,"op":"usage.reset","account":""}}
{"out":{"id":3,"ok":false,"error":"no account ; wsp usage lists every account","kind":"not-found"}}
{"in":{"id":4,"op":"workspaces.get","workspaceId":""}}
{"out":{"id":4,"ok":false,"error":"no such workspace: ","kind":"not-found"}}
{"in":{"id":5,"op":"workspaces.resolve","ref":""}}
{"out":{"id":5,"ok":false,"error":"no workspace ","kind":"not-found"}}
{"in":{"id":6,"op":"projects.resolve","ref":""}}
{"out":{"id":6,"ok":false,"error":"no project \"\"; wsp add <folder> records one","kind":"not-found"}}
{"in":{"id":7,"op":"slates.get","threadId":""}}
{"out":{"id":7,"ok":true,"slate":null}}
{"in":{"id":8,"op":"sessions.search","query":""}}
{"out":{"id":8,"ok":true,"hits":[]}}
{"in":{"id":9,"op":"conversations.list"}}
{"out":{"id":9,"ok":false,"error":"name the project the thread runs in","kind":"usage"}}
```

## Refusals in two halves

5.5. A refusal MUST be built by `refusal(happened, fix, kind?)` (exit.ts:72-73): the message is `refusalLine(happened, fix)` and `fix` rides beside it. `refusalLine` (`protocol/src/words/thread.ts:393-396`) trims the end of the first half, adds `.` unless it already ends in `.`, `!`, `?` or `:`, then a space and the fix. `refusalSaid` takes the fix back off the end (exit.ts:76). Every `usageRefusal` call has both halves (`packages/protocol/test/refusal-shape.test.ts`).

5.12. The edges of `refusalLine` and `refusal` MUST match (`words/thread.ts:393-396`; exit.ts:72-88):
- only the end of `happened` is trimmed, with JavaScript's `trimEnd` (every Unicode space and line end); its start and the whole of `fix` are kept as given;
- the period is added unless the trimmed first half ends in `.`, `!`, `?` or `:`, so `x)` and `x"` get one and `x .` keeps its own;
- an empty `fix` gives `<happened>. ` with its trailing space; an empty `happened` gives `. <fix>`;
- `refusal` always sets `fix`, even to `""`, and sets `kind` whenever it is not `undefined`, even to `""` (which then classes as provider, since `""` is not in `KIND_CLASS`);
- `refusalSaid(message, fix)` takes off `" " + fix` only when the message ends with exactly that, so an empty fix takes off the one trailing space;
- `refusalParts(e)` of a thrown non-Error is `{said: String(e)}` with no fix and no kind.

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"sessions.history","workspaceId":"w","limit":3}}
{"out":{"id":2,"ok":false,"error":"before and limit page one thread's events. Name the thread with threadId.","kind":"usage","fix":"Name the thread with threadId."}}
{"in":{"id":3,"op":"places.readings","range":"day"}}
{"out":{"id":3,"ok":false,"error":"places.readings names a placeId or a workspaceId","kind":"usage"}}
{"in":{"id":4,"op":"daemon.open"}}
{"out":{"id":4,"ok":false,"error":"daemon.open opens a channel to one daemon: name workspaceId or placeId, not both"}}
```

`daemon.open` with neither id is a plain error with no kind, so it exits 1 though it reads like a usage refusal; so are `sys.subscribe` with both or neither id and `forwards.stop` of nothing (serve.ts:1439,1448,1675; `runtime/src/sys-socket.ts:22`).

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"sys.subscribe"}}
{"out":{"id":2,"ok":false,"error":"sys.subscribe and sys.unsubscribe read one daemon: name workspaceId or placeId, not both"}}
{"in":{"id":3,"op":"forwards.stop"}}
{"out":{"id":3,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"undefined\",\n    \"path\": [\n      \"workspaceId\"\n    ],\n    \"message\": \"Required\"\n  },\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"number\",\n    \"received\": \"undefined\",\n    \"path\": [\n      \"port\"\n    ],\n    \"message\": \"Required\"\n  }\n]"}}
```

5.16. These replies MUST keep the shape they are written with outside the catch (each `{id, ok:false, error}` plus what the table says; serve.ts line numbers):

| Where | Sentence | Kind | Close |
|---|---|---|---|
| 647 | `invalid json` (id `null`) | none | 4401 if not authed |
| 654 | the non-object sentence (id `null`) | none | 4401 if not authed |
| 665 | the thread door (raw id) | none | no |
| 674 | the paired door (raw id) | none | no |
| 679 | the schema refusal (raw id) | none | 4401 if not authed |
| 687 | the door's `refuse`, plus `hostPublicKey, signature` on a place refusal | `auth` | 4401 |
| 895, 908, 912, 917, 924, 987, 1018, 1022, 1033, 1037, 1049, 1061, 1142, 1748, 1760 | a constant: `PAIR_ISSUE_REFUSAL`, `PAIR_CODE_REFUSAL`, `DEVICE_AUTH_REFUSAL`, `UNAUTHORIZED`, `PLACE_UNKNOWN_REFUSAL`, `DOCTOR_UNSERVED`, `PLACE_DOOR_REFUSAL`, `ACCOUNT_UNSERVED`, `AGENTS_KEY_REFUSAL`, `ENV_VALUE_REFUSAL` and the like | none | no |
| 1718, 1726 | `PLACES_TICKET_REFUSAL` | none | no |
| 1739 | `SIGN_IN_LINE_REFUSAL` on the own road, else `PLACES_TICKET_REFUSAL` | none, else `ticket` | no |
| 993, 1000, 1615; `sign-in-socket.ts:89` | `noSuchPlaceRefusal` and `doctorRowRefusal` (places.doctor); `places.readings` naming neither id; `noSignInRefusal` | `usage`, no `fix` | no |
| 1004 | `doctorRunningLine` | `conflict` | no |
| 1826 | `sshIncludeLine` | `sshInclude` | no |
| 874, 1104, 1138, 1150, 1159, 1169, 1811, 1844, 1901 | the ticket family | `ticket` | no |

5.6. A refusal that outlives its answer (kept on a job, in a log, on a computer's row) MUST be cut by `keptSaid`: at most 11 lines, each cut at `SAID_LINE_CHARS` = 400 with ` (cut N characters)` appended by `markedCut`, and `(cut N lines)` past the eleventh (exit.ts:92-103). The 400 counts UTF-16 units (rule 8.4).

5.13. `markedCut` and `keptSaid` MUST cut exactly so (exit.ts:92-103):
- `markedCut(line, max = 400)` keeps a line of 400 units or fewer whole; a longer one keeps its first 400 units and gains ` (cut N characters)`, N the units dropped, with no singular form (`(cut 1 characters)`), and a cut may split a surrogate pair;
- `keptSaid` splits on `\n` alone, so a `\r` stays in its line and counts toward the 400, and a trailing `\n` makes a last empty line that counts as one;
- it keeps the first 11 lines, each through `markedCut`, and past 11 adds one more line, `(cut N lines)`, again with no singular form; exactly 11 lines are kept whole;
- the empty string stays the empty string.

## The refusal log line

5.7. Every refused op MUST leave one line in the host's log (`refusedLine`, serve.ts:356-374): `refused <op> kind=<kind or none>: <the first line of the sentence>`, with request values of the secret fields (`token key rows code passphrase env envs headers sudoPassword`, read one level deep, `protocol/src/wire/runtime.ts:1033`) replaced by `<redacted>`, cut at 400 characters as 5.6 says, and the sentence withheld past 256 secret values. An op the protocol does not declare is logged as `frame`, never by the stranger's own word. A schema refusal's sentence is `issuesLine`: `<path>: <message>; ...` (`protocol/src/words/thread.ts:411-413`; `packages/runtime/test/serve-refusal.test.ts:102-108` pins `refused init.keys kind=none: provider: Expected string, received number`).

5.14. The refusal log line MUST be written for every frame the host answers with `ok: false`, the replies outside the catch included (invalid JSON, the non-object sentence, the doors, the schema refusal), and for a reply the closed socket dropped (rule 4.39) (`answering`, serve.ts:629-634). Its exact parts (serve.ts:364-374):
- the op is the frame's `op` when the frame is an object and the op is a string the protocol declares, else `frame`, which covers a missing op, a non-string op, an unknown op and a non-object frame;
- the kind is the reply's `kind` when it is a string, else `none`;
- the secrets are the distinct string values of the nine fields of `SECRET_REQUEST_FIELDS`, read as the field's own string or, for an object field, its string values one level down (`requestSecrets`, `runtime.ts:1037-1044`); past 256 distinct values the line is `refused <op> kind=<kind>: (sentence withheld, <N> secret values)`;
- otherwise the sentence is the schema refusal's `issuesLine` or the reply's `error`, cut at its first `\n`, then redacted, then cut by `markedCut`; `redacted` skips values whose trimmed length is under 4 and replaces each other value and its trimmed form, longest first, with `<redacted>` (`words/thread.ts:402`).

The host writes it with `console.log`, so it goes to standard output, beside the start lines, not to standard error (`packages/host/src/cli/serve.ts:305,331,349`; `cli/io.ts:61`). It is not on the wire, so it has no tape; recorded from the throwaway host's standard output for the frames of rules 4.32 and 5.10:

```text
refused projects.add kind=none: source: Required
refused workspaces.get kind=not-found: no such workspace: nope
refused sessions.settle kind=none: threadIds: Required
refused forwards.stop kind=none: workspaceId: Required; port: Required
refused sys.subscribe kind=none: sys.subscribe and sys.unsubscribe read one daemon: name workspaceId or placeId, not both
refused pair.redeem kind=none: that pairing code is not one this host is waiting for; run wsp host pair on the host for a fresh one
```

5.15. An error the TS code throws that is not an object MUST be handled as the catch handles it today: a thrown string or number gives `{id, ok:false, error: String(e)}` with nothing else; `failureWords` of a thrown `undefined` or `null` would give `"undefined"` and `"null"`, but the catch destructures `kind` and `fix` off the thrown value first (serve.ts:1933), which throws on `null` and `undefined`, so no reply is sent, the `failed` hook does not run, the client's request waits forever, and the host logs `unhandled rejection, kept serving: <message>` on standard error and keeps serving (`stayOnUncaught`, `packages/host/src/cli/serve.ts:168-171`). After-port note: a Rust error is always a value, so the port has no way to reproduce the hang and does not try; every TS site that can throw `null` or `undefined` is a bug to ticket after the port. No tape: no op reachable from a fresh host throws one.

## The schema refusal

The TS host validates every frame against its zod union before any handler runs, and a failure's `error` is zod 3's `ZodError.message`: `JSON.stringify(issues, null, 2)` (serve.ts:677-683). Clients read it: `validatorRefusal` parses the text as JSON and, when some issue is `invalid_union_discriminator` and every issue's first path element that is a string is `op`, tells the person the host runs another version of wsp (`protocol/src/words/thread.ts:79-97`; `hostSchemaRefusal`, `packages/host/src/verbs/client.ts:1106-1109`).

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"sessions.rename","threadId":"t","title":"x"}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"undefined\",\n    \"path\": [\n      \"sessionId\"\n    ],\n    \"message\": \"Required\"\n  }\n]"}}
{"in":{"id":3,"op":"preferences.set","patch":{"theme":"purple"}}}
{"out":{"id":3,"ok":false,"error":"[\n  {\n    \"received\": \"purple\",\n    \"code\": \"invalid_enum_value\",\n    \"options\": [\n      \"system\",\n      \"light\",\n      \"dark\"\n    ],\n    \"path\": [\n      \"patch\",\n      \"theme\"\n    ],\n    \"message\": \"Invalid enum value. Expected 'system' | 'light' | 'dark', received 'purple'\"\n  }\n]"}}
{"in":{"id":4,"op":"auth","token":5}}
{"out":{"id":4,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_type\",\n    \"expected\": \"string\",\n    \"received\": \"number\",\n    \"path\": [\n      \"token\"\n    ],\n    \"message\": \"Expected string, received number\"\n  }\n]"}}
```

An unknown op answers one `invalid_union_discriminator` issue whose `options` and `message` list every op the union holds, 211 of them, in union order:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"no.such.op"}}
{"out":{"id":2,"ok":false,"error":"[\n  {\n    \"code\": \"invalid_union_discriminator\",\n    \"options\": [\n      \"auth\",\n      \"ticket.issue\",\n      \"pair.issue\",\n      \"pair.redeem\",\n      \"seal.open\",\n      \"device.auth\",\n      \"account.get\",\n      \"devices.list\",\n      \"devices.revoke\",\n      \"place.join\",\n      \"place.auth\",\n      \"place.prove\",\n      \"places.list\",\n      \"places.update\",\n      \"places.holds\",\n      \"places.remove\",\n      \"places.doctor\",\n      \"places.dial\",\n      \"places.set\",\n      \"places.follow\",\n      \"places.skip\",\n      \"places.setupLog\",\n      \"places.estimate\",\n      \"places.loginLanded\",\n      \"places.door\",\n      \"places.mint\",\n      \"places.sshHosts\",\n      \"places.add\",\n      \"places.setup\",\n      \"places.choose\",\n      \"recipes.list\",\n      \"recipes.get\",\n      \"recipes.save\",\n      \"recipes.remove\",\n      \"recipes.options\",\n      \"events.subscribe\",\n      \"status.subscribe\",\n      \"status.list\",\n      \"workspaces.create\",\n      \"workspaces.landing\",\n      \"workspaces.list\",\n      \"workspaces.resolve\",\n      \"workspaces.get\",\n      \"workspaces.nap\",\n      \"workspaces.resize\",\n      \"workspaces.wake\",\n      \"workspaces.restartDaemon\",\n      \"workspaces.stopWake\",\n      \"workspaces.upgrade\",\n      \"workspaces.rename\",\n      \"workspaces.look\",\n      \"workspaces.bringBack\",\n      \"worktree.make\",\n      \"folder.make\",\n      \"worktree.remove\",\n      \"workspaces.checkout\",\n      \"workspaces.discard\",\n      \"workspaces.commit\",\n      \"workspaces.commitDraft\",\n      \"workspaces.viewed\",\n      \"workspaces.pullRequestView\",\n      \"workspaces.pullRequestDiff\",\n      \"workspaces.pullRequestSend\",\n      \"workspaces.pullRequestReply\",\n      \"workspaces.pullRequestResolve\",\n      \"workspaces.pullRequestReact\",\n      \"workspaces.fix\",\n      \"workspaces.merge\",\n      \"workspaces.start\",\n      \"workspaces.review\",\n      \"workspaces.reviewDraft\",\n      \"workspaces.reviewPost\",\n      \"workspaces.update\",\n      \"workspaces.mergeIn\",\n      \"workspaces.delete\",\n      \"workspaces.forget\",\n      \"workspaces.snapshot\",\n      \"projectGoldens.list\",\n      \"projectGoldens.remove\",\n      \"workspaces.touch\",\n      \"daemon.open\",\n      \"sys.subscribe\",\n      \"sys.unsubscribe\",\n      \"appProcs.watch\",\n      \"appProcs.unwatch\",\n      \"appProcs.offer\",\n      \"appProcs.set\",\n      \"activity.read\",\n      \"activity.end\",\n      \"daemon.send\",\n      \"daemon.close\",\n      \"sessions.start\",\n      \"harnesses.list\",\n      \"sessions.list\",\n      \"sessions.history\",\n      \"sessions.head\",\n      \"sessions.attachment\",\n      \"sessions.interrupt\",\n      \"sessions.steer\",\n      \"sessions.answer\",\n      \"sessions.access\",\n      \"sessions.rename\",\n      \"sessions.forget\",\n      \"sessions.delete\",\n      \"sessions.read\",\n      \"sessions.replaceable\",\n      \"sessions.warm\",\n      \"sessions.settle\",\n      \"sessions.mark\",\n      \"sessions.restore\",\n      \"sessions.search\",\n      \"sessions.aside\",\n      \"sessions.run\",\n      \"sessions.rewind\",\n      \"golden.get\",\n      \"capabilities.get\",\n      \"golden.prepare\",\n      \"golden.seal\",\n      \"snapshots.list\",\n      \"snapshots.storage\",\n      \"cost.history\",\n      \"cost.spend\",\n      \"usage.used\",\n      \"usage.accounts\",\n      \"usage.reset\",\n      \"places.readings\",\n      \"snapshots.rollback\",\n      \"workspaces.portReach\",\n      \"workspaces.portProbe\",\n      \"workspaces.rebuild\",\n      \"forwards.list\",\n      \"forwards.stop\",\n      \"workspaces.exec\",\n      \"host.folders\",\n      \"host.terminalConfig\",\n      \"editor.list\",\n      \"editor.open\",\n      \"ssh.port\",\n      \"ssh.include\",\n      \"agents.read\",\n      \"servers.tools\",\n      \"servers.icon\",\n      \"agents.signIn\",\n      \"servers.signIn\",\n      \"agents.signInCode\",\n      \"agents.signIns\",\n      \"agents.signInStop\",\n      \"agents.signInLine\",\n      \"agents.key\",\n      \"agents.cloudSignIn\",\n      \"providers.checkKey\",\n      \"broker.status\",\n      \"agents.addTools\",\n      \"agents.setup\",\n      \"skills.search\",\n      \"skills.get\",\n      \"skills.preview\",\n      \"skills.add\",\n      \"skills.remove\",\n      \"skills.toggle\",\n      \"servers.add\",\n      \"servers.remove\",\n      \"servers.toggle\",\n      \"plugins.toggle\",\n      \"init.get\",\n      \"init.keys\",\n      \"init.start\",\n      \"init.answer\",\n      \"init.step\",\n      \"init.draft\",\n      \"init.retry\",\n      \"init.build\",\n      \"init.signInCode\",\n      \"init.cancel\",\n      \"preferences.get\",\n      \"preferences.set\",\n      \"release.get\",\n      \"release.check\",\n      \"host.restart\",\n      \"projects.add\",\n      \"project.seed.plan\",\n      \"projects.seed\",\n      \"projects.list\",\n      \"conversations.list\",\n      \"conversations.read\",\n      \"projects.defaults\",\n      \"projects.resolve\",\n      \"projects.branch\",\n      \"projects.remove\",\n      \"projects.icon\",\n      \"projects.icons\",\n      \"project.plan\",\n      \"project.import\",\n      \"project.export\",\n      \"image.get\",\n      \"image.build\",\n      \"image.export\",\n      \"slates.get\",\n      \"slates.write\",\n      \"slates.state\",\n      \"slates.read\",\n      \"slates.catalog\",\n      \"slates.event\",\n      \"slates.approve\",\n      \"slates.cancel\",\n      \"slates.revoke\",\n      \"slates.shown\",\n      \"slates.subscribe\",\n      \"slates.unsubscribe\",\n      \"slates.resolve\",\n      \"slates.image\"\n    ],\n    \"path\": [\n      \"op\"\n    ],\n    \"message\": \"Invalid discriminator value. Expected 'auth' | 'ticket.issue' | 'pair.issue' | 'pair.redeem' | 'seal.open' | 'device.auth' | 'account.get' | 'devices.list' | 'devices.revoke' | 'place.join' | 'place.auth' | 'place.prove' | 'places.list' | 'places.update' | 'places.holds' | 'places.remove' | 'places.doctor' | 'places.dial' | 'places.set' | 'places.follow' | 'places.skip' | 'places.setupLog' | 'places.estimate' | 'places.loginLanded' | 'places.door' | 'places.mint' | 'places.sshHosts' | 'places.add' | 'places.setup' | 'places.choose' | 'recipes.list' | 'recipes.get' | 'recipes.save' | 'recipes.remove' | 'recipes.options' | 'events.subscribe' | 'status.subscribe' | 'status.list' | 'workspaces.create' | 'workspaces.landing' | 'workspaces.list' | 'workspaces.resolve' | 'workspaces.get' | 'workspaces.nap' | 'workspaces.resize' | 'workspaces.wake' | 'workspaces.restartDaemon' | 'workspaces.stopWake' | 'workspaces.upgrade' | 'workspaces.rename' | 'workspaces.look' | 'workspaces.bringBack' | 'worktree.make' | 'folder.make' | 'worktree.remove' | 'workspaces.checkout' | 'workspaces.discard' | 'workspaces.commit' | 'workspaces.commitDraft' | 'workspaces.viewed' | 'workspaces.pullRequestView' | 'workspaces.pullRequestDiff' | 'workspaces.pullRequestSend' | 'workspaces.pullRequestReply' | 'workspaces.pullRequestResolve' | 'workspaces.pullRequestReact' | 'workspaces.fix' | 'workspaces.merge' | 'workspaces.start' | 'workspaces.review' | 'workspaces.reviewDraft' | 'workspaces.reviewPost' | 'workspaces.update' | 'workspaces.mergeIn' | 'workspaces.delete' | 'workspaces.forget' | 'workspaces.snapshot' | 'projectGoldens.list' | 'projectGoldens.remove' | 'workspaces.touch' | 'daemon.open' | 'sys.subscribe' | 'sys.unsubscribe' | 'appProcs.watch' | 'appProcs.unwatch' | 'appProcs.offer' | 'appProcs.set' | 'activity.read' | 'activity.end' | 'daemon.send' | 'daemon.close' | 'sessions.start' | 'harnesses.list' | 'sessions.list' | 'sessions.history' | 'sessions.head' | 'sessions.attachment' | 'sessions.interrupt' | 'sessions.steer' | 'sessions.answer' | 'sessions.access' | 'sessions.rename' | 'sessions.forget' | 'sessions.delete' | 'sessions.read' | 'sessions.replaceable' | 'sessions.warm' | 'sessions.settle' | 'sessions.mark' | 'sessions.restore' | 'sessions.search' | 'sessions.aside' | 'sessions.run' | 'sessions.rewind' | 'golden.get' | 'capabilities.get' | 'golden.prepare' | 'golden.seal' | 'snapshots.list' | 'snapshots.storage' | 'cost.history' | 'cost.spend' | 'usage.used' | 'usage.accounts' | 'usage.reset' | 'places.readings' | 'snapshots.rollback' | 'workspaces.portReach' | 'workspaces.portProbe' | 'workspaces.rebuild' | 'forwards.list' | 'forwards.stop' | 'workspaces.exec' | 'host.folders' | 'host.terminalConfig' | 'editor.list' | 'editor.open' | 'ssh.port' | 'ssh.include' | 'agents.read' | 'servers.tools' | 'servers.icon' | 'agents.signIn' | 'servers.signIn' | 'agents.signInCode' | 'agents.signIns' | 'agents.signInStop' | 'agents.signInLine' | 'agents.key' | 'agents.cloudSignIn' | 'providers.checkKey' | 'broker.status' | 'agents.addTools' | 'agents.setup' | 'skills.search' | 'skills.get' | 'skills.preview' | 'skills.add' | 'skills.remove' | 'skills.toggle' | 'servers.add' | 'servers.remove' | 'servers.toggle' | 'plugins.toggle' | 'init.get' | 'init.keys' | 'init.start' | 'init.answer' | 'init.step' | 'init.draft' | 'init.retry' | 'init.build' | 'init.signInCode' | 'init.cancel' | 'preferences.get' | 'preferences.set' | 'release.get' | 'release.check' | 'host.restart' | 'projects.add' | 'project.seed.plan' | 'projects.seed' | 'projects.list' | 'conversations.list' | 'conversations.read' | 'projects.defaults' | 'projects.resolve' | 'projects.branch' | 'projects.remove' | 'projects.icon' | 'projects.icons' | 'project.plan' | 'project.import' | 'project.export' | 'image.get' | 'image.build' | 'image.export' | 'slates.get' | 'slates.write' | 'slates.state' | 'slates.read' | 'slates.catalog' | 'slates.event' | 'slates.approve' | 'slates.cancel' | 'slates.revoke' | 'slates.shown' | 'slates.subscribe' | 'slates.unsubscribe' | 'slates.resolve' | 'slates.image'\"\n  }\n]"}}
```

## Proposal 4: one error type

> **Proposal, accepted by the owner (26 Oct 2026, #2442)** (ticket decision 4). Pick: one `HostError` in `wsp-words`, used by every crate, carrying exactly what the TS reply and the failure object carry.

```rust
pub struct HostError {
    /// The sentence: `refusal_line(happened, fix)` for a refusal, else the message.
    pub message: String,
    /// What to do, where the error was built as a refusal.
    pub fix: Option<String>,
    /// The kind stamped where the error was born; None classes as provider.
    pub kind: Option<Kind>,
    /// A slate write's validator lists.
    pub errors: Option<Vec<serde_json::Value>>,
    pub warnings: Option<Vec<serde_json::Value>>,
    /// The threads and processes a settle or delete was refused over (kind ProcessesLeft only).
    pub left: Option<Vec<serde_json::Value>>,
}

/// Every kind the TS host stamps, one variant each, serialized as the TS string ("not-found", "startLimit").
pub enum Kind { Usage, Invalid, NotFound, ProcessesLeft, Auth, Ticket, Conflict, /* ...the table above */ }

impl HostError {
    pub fn class(&self) -> ExitClass { todo!() } // KIND_CLASS, unlisted kinds and None are Provider
    pub fn exit_code(&self) -> i32 { todo!() }   // 0, 1, 2, 3
}
```

Why: the TS host's whole error contract is a sentence, an optional fix, an optional kind and three optional lists (exit.ts:27-67), and every door (the wire reply, the failure object, the MCP tool error, the log line) reads the same four things. A closed `Kind` enum makes a misspelt kind a compile error, where today it silently classes as provider; a new kind is one variant and one row of the class table. The constructors mirror the TS helpers one for one (`usage_refusal`, `auth_refusal`, `not_found_refusal`, `cloud_off_refusal`, `refusal(happened, fix, kind)`), so a port reads like the TS. Crates do not define error types of their own; a crate's internal error converts to `HostError` at its edge with the kind the TS site stamps.

5.17. `processes-left` MUST be raised only where the TS raises it, by `processesLeftRefusal(verb, left, theirs)`, `theirs` a set of pids (`protocol/src/views/activity.ts:567-568`) from `meet` when the caller asked `processes: "ask"` (`runtime/src/activity.ts:412`), on `sessions.settle` and `sessions.delete` (`runtime/src/threads/sessions.ts:1134,1420,1422`). The reply is `{id, ok:false, error, kind:"processes-left", fix, left}`. Each `left` item is a `LeftThread` (`activity.ts:62`): `{threadId, title, workspaceId, memory, cpu, processes, on?}`, each process `{pid, ppid, depth, startedAt, kind, name, command, memory, cpu, ports}` with `kind` one of `PROC_KINDS` (`activity.ts:24-45`). The sentence (`processesLeftLine`, `processesLeftFix`, `activity.ts:553-566`) names a thread by its first 8 characters, names at most 8 processes then ` and N more`, says `a process` for one, `a thread to settle` or `the thread to delete`, and ends its first half with `:` so no period is added: `a thread to settle still runs 2 processes: 4123 node (dev, :3000) of thread-i, 4130 vite (tool) of abcd1234: settle it again with processes stop to end them first, or processes keep to leave them running (--processes on the command line).` `theirs` is the set of pids the asking thread may not end; when it holds any, each of those processes gains `the person's to end` and the fix changes: `the thread to delete still runs a process: 4123 node (dev, :3000, the person's to end) of thread-i: a thread ends only what it and the threads under it started, so ask the person to end 4123, or delete it again with processes keep to leave them running (--processes on the command line).` Both sentences were printed by the built `processesLeftRefusal`. No tape: it needs a thread with live processes, which a fresh host with no agent cannot start; `packages/runtime/test/serve-refusal.test.ts` and `box-settle.test.ts` hold the shape.

5.18. A failure after an op's reply MUST travel as an event, never as a second reply. `workspaces.exec` answers `{id, ok:true, execId, cwd?}` and then ends with `{type:"exec.exit", execId, exitCode, error?}`, `exitCode` an integer or `null` and `error` the failure's message with no kind and no fix (serve.ts:1679-1702; `ExecEvent`, `runtime.ts:1273`); a turn's failure after `sessions.start` answered rides the session events. A stream whose `exited` itself rejects reaches only the host's standard error, as rule 5.15's log line.

5.19. `verbFailure` (exit.ts:56-59), which builds the command line's failure object and the MCP tool error, MUST take `error` as `e.message` for an Error and `String(e)` otherwise, not `failureWords`: an Error with an empty message gives `""`, and a `fetch failed` TypeError stays `fetch failed` with no cause. It carries `error, class, exit` then `errors` and `warnings` when arrays, never `kind`, `fix` or `left`.

5.20. `failureWords` (`engine/src/errors.ts:335-342`) MUST give: for a thrown non-Error, `String(e)`, so `"undefined"`, `"null"`, and `"[object Object]"` for a plain object even when it has a `message`; for an Error with an empty message, `""`; for a TypeError whose message is exactly `fetch failed` with an Error `cause`, `fetch failed: ` and the cause's message and, for an AggregateError cause, each inner error's message, empty ones dropped, joined with `; ` and not deduplicated, one level deep only; `fetch failed` alone when nothing is left; for every other Error, its message unchanged.

5.8. An MCP tool MUST return the failure object `{error, class, exit[, errors, warnings]}` as a tool error with `isError: true` (10, rule 10.13), built from the same `HostError`.

## Proposal 9: the schema refusal's text

> **Proposal, accepted by the owner (26 Oct 2026, #2442)** (new; not in the ticket). Pick: the Rust host answers a schema failure with the same envelope (`ok: false`, no kind, the request's id or `null`) and an `error` that is the 2-space JSON text of an issue list in zod 3's shape (`code`, `path`, `message`, and `options`, `keys`, `expected`, `received` where zod has them). An unknown op is exactly one `invalid_union_discriminator` issue on `["op"]` whose `options` are the ops the Rust host serves. Every other failure carries at least the first failing field's issue with zod's code and path. The harness compares `code` and `path` of the first issue and accepts the rest of the text as a difference.

Why not byte for byte: zod collects every issue, in the schema's field order, and words each in its own way (a missing id gives an `invalid_union` with nested `unionErrors`, the tape under rule 4.13 in 04; `forwards.stop` with neither field gives two issues, the tape under rule 5.16); serde stops at the first error and words nothing alike. Reproducing zod's walker for 211 schemas is a port of zod, not of wsp. Why not free text: clients parse the text (`validatorRefusal` above), and the version hint must keep working while TS and Rust hosts coexist on the owner's computers. The issue list's first code and path are what any client reads; the rest is for people, and a schema refusal is a bug in a client, not something a person meets.
