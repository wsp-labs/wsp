# machine_link
Source: packages/protocol/src/wire/machine-link.ts lines 1-509 (509 of its 702 lines) at a3b870a6e, less placeForgetLine, placeHoldsLine and localRunsOneLine, which went to wsp-words. Lines 510-702 are the proc_frames unit. Tests: machine-link.test.ts (8 cases), places.test.ts (14), protocol.test.ts (3), hosts-menu.test.ts (1), place-state.test.ts (1); 5 more pinned here against what the TS module printed. Rust: `src/wire/machine_link.rs`; tests `tests/machine_link.rs` (33).

## What it does
One computer drives another's machines over the place link: every call of the engine's MachineBackend and Machine interfaces as a frame (`MachineLinkRequest`, 19 ops), the shapes they answer (facts, capacity, handle, list rows, exec results, readings, preview routes), the engine's error kinds that ride a refusal, the kernel check a computer runs before it may host workspaces, and the sentences of removing, forgetting and refusing on such a computer. No state.

## Behaviour
1. `parse_machine_link_request` (machine-link.ts:203-241) reads the union on `op` by the rule written on the function: an unknown or absent op is one `invalid_union_discriminator` on ["op"] with `options` = `MACHINE_LINK_OPS` in order (machine.backend first, machine.putBytes last). The eight layer-store ops a place answered once (machine.snapshot and the rest) are unknown ops now (machine-link.test.ts:45).
2. `machine.exec`: `cmd` at most `EXEC_BODY_MAX` = 16384 UTF-16 units (too_big at ["cmd"]); `timeoutMs` a positive integer. `machine.daemonAnswers.timeoutMs` the same. `machine.previewUrl.port` 1..=65535.
3. `machine.putBytes`: `uploadId` 1..=32 of `[a-z0-9]`; `seq` an integer >= 0; `last` required; `data` any string (the base64 of at most `MACHINE_PUT_PART_BYTES` = 4194304 raw bytes is the sender's rule, not the reader's); `timeoutMs` positive.
4. `MachineSpec` (:36-66): `kind` the views' MachineKind; every other field optional; `onIdle` pause|kill; `envs` and `labels` string maps kept in order; `copy`, `shares`, `binds` are the views' WorkspaceCopy, MachineShare, MachineBind, which check their own paths.
5. `BackendFacts` (:162-177): `logins` and `projects`, when present, are non-empty and `is_plain_path` (helpers_limits 2); a relative or dotted path is a `custom` issue at that field. `pricing.builderDiskGb` optional; `lifecycle.budgets.wakeAttempts` an integer >= 1 and `daemonAnswersMs` positive.
6. `SnapshotRow.parent` keeps absent, `null` and a string apart (double_option): a root snapshot writes `"parent": null`.
7. `MachineHandle.roads` holds six required booleans. `MachineReading.cgroup` and `.upper` required; every live figure optional and a non-negative integer where the TS says `.int().nonnegative()`.
8. `workspaces_blocked_by(at)` (:410-445), in this order, the first failure answered: platform not `linux` gives "wsp runs workspaces on a Linux computer" before any file is read; `read(CGROUP_CONTROLLERS_PATH)` None gives the cgroup v1 sentence; the controllers split on white space must hold `memory` then `cpu`, each missing one named in turn; then the overlay (below); then `euid != Some(0)` gives the root sentence; else None.
9. The overlay check (:431-442): `read(PROC_FILESYSTEMS_PATH)` split on white space holds `overlay`: done, no modprobe. Else `modprobe(["overlay"])`: `Failed{error}` answers the would-not-load sentence with `/sbin/modprobe could not run: <error>`; `Ran{ok: true}` and the list read again holds overlay: done. Else `modprobe(["-n", "overlay"])`: `Failed` or `ok: false` answers the no-module sentence; else the would-not-load sentence with the LAST line of the first load's stderr (trailing white space trimmed first, then the line trimmed), or "modprobe loaded it and the kernel still lists no overlay" where that is empty. The files are read again after a load: `read` is called twice in that road.
10. `place_cannot_boot_line(place, reason)` (:456): no reason is the "so it cannot be a place" sentence; any reason, the empty string included, follows a colon with every `this computer's` turned to `its` and every `this computer` to `it` (lower case only, left to right, `'s` form first where both match).
11. `place_blocked(place, runs, blocked)` is None while `runs`, else the line above. `place_no_tools_line(place, door, blocked, unit)` (:469) is None unless `door == Some(false)`; the reason defaults to "its daemon opened no door for them"; the fix names `systemctl restart <unit>` where a unit is given, else remove and add again.
12. `is_joined_computer(id, kind)`: kind `computer` and id not `here`.
13. Every other sentence is a plain format of its arguments, copied byte for byte; lists join with `, ` (or `; ` in place_unsaved_refusal).

## Edge cases
- `place_drop_stopped_line(why, [])` is `why` alone.
- `no_such_place_refusal("x", [])` ends `you have ` with nothing after it.
- `place_cannot_boot_line("box", Some("This computer x"))` keeps the capital T.

## Behaviour nobody wrote down
- Rule 4.26: `MachineSpec.envs`/`labels`, `MachineListRow.labels` and `MachineHandle.labels` keep their keys in the order read; a label map the host writes back to a place is that order.
- Rule 4.25: `ExecResult.exitCode`, `MachineReading` counters and the budget integers are i64; sizes, rates and `cpu`/`memMb` are plain numbers.

## Tapes
The machine ops ride the place link only (chapter 09); the contract fixtures under daemon/fixtures/contract/frames/machine.*.json are their accept and reject frames (`tests/contract_fixtures.rs`). Held out: none counted here.

## Decisions
1. `workspaces_blocked_by` takes a `KernelAt` of borrowed closures (`read`, `modprobe`) and `euid: Option<u32>`; the TS takes the same three as an object.
2. `MachineBackendReply` and `MachineCapacityReply` are type aliases of `BackendFacts` and `PlaceCapacity`, as the TS binds the same schema twice.
3. `place_unsaved_refusal` and the other two-part refusals return a `SaidFix { said, fix }`.

## After the port
- The overlay check reads `/proc/filesystems` twice on the load road and never re-reads the controllers; a kernel that changed between the reads is not considered.
