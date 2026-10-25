# wsp-serve: the op switch and the assembly of the areas

Ported from `packages/runtime/src/` at `65f6b89c7` (the dead-code cut, #2441): `serve.ts` (1,934),
`serve-app-procs.ts` (102), `serve-slates.ts` (74), `serve-start.ts` (52), `serve-projects.ts` (44),
`serve-broker.ts` (31), `serve-processes.ts` (24), `serve-conversations.ts` (17), `runtime.ts` (1,277) and
`index.ts` (67): 3,622 lines. Every `file:line` below is at `65f6b89c7`.

## Units

| Unit | TS | Rust | Spec | Tests |
|---|---|---|---|---|
| serve_wire | serve.ts:135-347, 375-492, 1899-1934 | `src/serve.rs` | `serve_wire.md` | `tests/serve_wire.rs` |
| serve_door | serve.ts:349-367, 444-449, 493-848 | `src/serve/door.rs` | `serve_door.md` | `tests/serve_door.rs` |
| serve_places | serve.ts:869-878 (gates), 883-1169 | `src/serve/places.rs` | `serve_places.md` | `tests/serve_places.rs` |
| serve_workspaces | serve.ts:1170-1473, 1634-1678 | `src/serve/workspaces.rs` | `serve_workspaces.md` | `tests/serve_workspaces.rs` |
| serve_ops | serve.ts:419-440, 857-868, 1474-1633, 1679-1895 | `src/serve/ops.rs` | `serve_ops.md` | `tests/serve_ops.rs` |
| serve_helpers | the seven `serve-*.ts` | `src/serve_*.rs` | `serve_helpers.md` | `tests/serve_helpers.rs` |
| runtime_core | runtime.ts:425-992 | `src/runtime.rs`, `src/runtime/core.rs` | `runtime_core.md` | `tests/runtime_core.rs` |
| runtime_of | runtime.ts:1017-1277, index.ts | `src/runtime/public.rs`, `src/lib.rs` | `runtime_of.md` | `tests/runtime_of.rs` |

Order for the implementers: serve_helpers (no other unit), serve_door with serve_wire (every socket test needs the
socket state), then serve_places, serve_workspaces and serve_ops in any order; runtime_of, then runtime_core.

## The seams

**The runtime a socket serves** is `ServedRuntime` (`src/serve/served.rs`): wsp-core's `Runtime` trait
(`wsp-core/src/types/api.rs`, which rule 1.5 moved down), plus the doors wsp-core still holds as empty handle
traits (ruling 6400658264, 2), read typed: `place_door()` answers wsp-machines' `PlaceDoor`
(`wsp-machines/src/places/types.rs`), `slate_door()` answers this crate's `SlateDoor` (the calls `answerSlate`
makes, serve-slates.ts), `activity_door()` this crate's `ActivityOps` (serve-processes.ts), and the three
per-socket roads wsp-core's empty `sys_socket.rs`, `sign_in_socket.rs` and `forwards.rs` will hold (ruling
6402177374, 8 gave them a writer): `sys_ops`, `sign_in_ops`, `forwards`. When wsp-slates lands and wsp-core's
pending traits get their methods, the join folds `ServedRuntime` into `Runtime`; no behaviour here changes.

**One socket** is `Socket` (`src/serve/socket.rs`): what the door decided about it (`Road`: `stamped`,
`paired_road`, `me`, `by`, serve.ts:505-555) and its transport, a `FrameSink` (send a text frame, close with a
code and a reason, whether it is open). The real sink is the websocket; the tests hand a recording sink, so every
unit but serve_wire is tested without a network. `Socket::detaches` is the list a close runs, in push order
(serve.ts:557, 569-573).

**The host's own doors** (`ServeOptions`, serve.ts:139-220) stay traits the host implements: `AccountDoor`,
`AdmittedDevices`, `PlaceDoorControl`, `PlaceDoctor`, `ReleaseDoor`, `RestartDoor`, `BrokerDoor`, and wsp-core's
`HostFolders`, `HostTerminalConfig`, `HostEditor`, `HostSsh`, `InitDoor`, `RecipeShelf`, `ProjectBundler`,
`ProjectLander`. Each absent door refuses its ops in its own sentence (each unit lists them).

## Rules every unit holds

- R1. **Key order is the TS's** (ruling 6402177374, 1). Every reply is `{ id, ok, ...rest }` with `id` first and
  `ok` second, then the fields in the order the TS literal writes them; a spread of an area's answer keeps the
  area's own order. A refusal is `{ id, ok: false, error, kind?, fix?, errors?, warnings?, left? }`
  (serve.ts:1887-1893). Tests compare the reply's JSON text, not a map, wherever order is pinned.
- R2. **Absent is absent.** `...(x !== undefined ? { x } : {})` writes no key, never `null`. A field the TS writes
  as `null` (`id: null`, `head: null`, `storage: null`) is written as `null`.
- R3. **One reply per frame**, except the streams (`workspaces.exec`, `daemon.open`, `events.subscribe`,
  `status.subscribe`, `appProcs.*`), whose pushes carry no `id`.
- R4. **Every refusal is logged once** through `opts.log`, after it is sent (serve.ts:621-626), as
  `refused <op> kind=<kind>: <first line>` (serve_door.md, D20-D24). A reply that is `ok: true` logs nothing.
- R5. **A failed op tells `opts.failed(op, e)`** after the refusal is sent (serve.ts:1894), only from the catch,
  never for a refusal the switch sends itself.
- R6. **Invariant 3 of `packages/runtime/AGENTS.md`**: a request's origin is stamped off its token, never the
  client's word. Pinned in serve_door.md (D30-D34) and by the gate tests of every op unit.
- R7. **Nothing survives a host restart here.** Tickets, the `doctoring` set, `restarting`, the held device sockets
  and the app-process hub live in memory and are gone with the process (serve.ts:390-395, 453;
  serve-app-procs.ts:92). The devices and the places they name are the store's (wsp-core, wsp-machines).

## Tests

The TS tests drive `createRuntime` over a stub backend and a real websocket. The areas behind `Runtime` are other
crates' and are not built yet, so each case is ported at this crate's seam: a scripted `FakeRuntime`
(`tests/support/fake_rt.rs`) that records every call with its arguments and answers what the case scripts, a
`RecordingSink` for one socket, and the real websocket only in `serve_wire`. Each test names its TS case
(`// serve.test.ts:37`). A case whose assertion is about an area's own behaviour (a create's stages, a transcript's
cap) is that area's and is listed, not ported, in the unit's spec.
