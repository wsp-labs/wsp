# api: the Runtime interface

TS: `packages/runtime/src/types/api.ts` (748 lines) at `65f6b89c7`. Rust: `src/types/api.rs`. Tests: none of its own.

`api.ts` declares the `Runtime` every door calls and holds no behaviour: every method's rules are its area's spec
(workspaces, projects, sessions, harnesses, the image roads, agents, conversations, preferences, status, usage,
slates, activity, devices), and its tests are that area's. So this unit has nothing to implement and no red tests;
its check is that the areas' implementations satisfy it.

## Shape

- Rule 1.5 moves `Runtime` down to wsp-core because `context.ts` and the area files name it; wsp-serve implements it.
  The TS's nested objects (`rt.workspaces.create`) are one trait per area reached through an accessor
  (`rt.workspaces().create`): `AgentsApi`, `WorkspacesApi`, `ProjectsApi`, `SessionsApi`, `HarnessesApi`,
  `GoldenApi`, `ImageApi`, `ConversationsApi`, `PreferencesApi`. `status`, `usage`, `slates`, `activity`, `devices`,
  `places`, `recipes` and `events` answer the area's own handle as an `Rc<dyn _>`.
- Every async method is `#[async_trait(?Send)]` and answers `Result<_, HostError>` (chapter 05). The methods the TS
  declares without a promise stay synchronous, because their callers cannot await: `seen_by` and `shown_to` (the
  event road, api.ts:328-337), `creating`, `thread_place`, `place_kills`, `own_process`, `note_retry`, `sign_ins`,
  `sign_in_follow`, `sign_in_stop`, `forwards`.
- The caller's origin is the last argument, `Option<Caller>`, None reading here (api.ts:141-143). Where the TS puts
  another optional after it (`get(id, origin, threadId)`, `checkout(id, origin, fresh)`, `head(id, origin, o)`,
  `interrupt(id, origin, task)`, `aside(id, q, origin, askId)`, `forget(id, origin, o)`, `delete(id, origin, o)`,
  `settle(ids, origin, o)`, `remove(id, origin, o)`), the Rust keeps that order.
- A TS options object with more than two fields is a struct (`StartOptions`, `FolderForAsk`, `MergeAsk`, ...); one
  of one or two fields is plain arguments. A TS answer object is a struct (`Landing`, `FolderFor`, `ThreadDeleted`,
  `Pruned`, ...), or the bare value where the TS answers `{ file }`, `{ removed }`, `{ paths }` or `{ plugin }`: the
  wire layer (wsp-serve) wraps it back, so the reply's JSON is the TS's.
- `shown_to` and `seen_by` take and give `serde_json::Value`, since they see every event type. `signal` options are a
  `CancellationToken` (the TS `AbortSignal`).
- `OutsideOpening.earlier` holds `SessionEarlierEvent`s; only `who`, `text` and `at` are read (api.ts:747).

## What the type cannot say

`sessions.fork`'s TS options are `Omit<start options, "fork" | "thread" | "cwd" | "replaces">`; the Rust
`ForkOptions` carries a whole `StartOptions` and the area refuses or ignores those four, as the TS type forbids them.
`workspaces.shownTo<E>` is generic in the TS and `Value` here.
