# Isolated connected-computer children implementation plan

> **For agentic workers:** Execute inline with bounded cold reviews under agent-pipeline. The owner authorized implementation in the existing stable hotfix PR, outside the Rust host migration.

**Goal:** A thread can choose any connected computer with runtime support and start an isolated child from its current checkout without a registered destination project or a shared Git remote.

**Architecture:** One persisted runtime group per root thread and destination computer; each child has its own workspace record, checkout and branch on that compute. This preserves the existing workspace-based execution and Git resolver. A distinct workspace kind and group owner label keep older hosts from adopting or reaping shared compute. Keep the original project identity, authorize against the actual thread tree, and use the existing joined backend and machine byte roads. Capture the original commit and its tree as a shallow Git object pack plus separate staged/unstaged patches and nonignored files; transfer selected ignored paths only. Return branches through the daemon link. Completion of a turn is not completion of a tree: clean up only after explicit settling/deletion and no live/pending children, and preserve unreturned or dirty work.

**Tech stack:** Existing TypeScript host/runtime/protocol, Rust MCP record, joined daemon machine.create and byte transport. Electron stays default.

## Transfer and return

Files: create `packages/runtime/src/projects/checkout-transfer.ts` and `packages/runtime/test/checkout-transfer.test.ts`.

- [x] Capture the source checkout through its Machine, preserving HEAD identity without unrelated history, staged and unstaged binary changes, untracked files and explicitly selected ignored files. Read subfolder paths from Git; do not copy the host worktree's .git pointer.
- [x] Transfer a file in bounded chunks with size/hash verification; reject truncation, corruption and failed writes, and remove partial archives.
- [x] Materialize an independent checkout on the target with local bundle history; no push, remote clone or source commit.
- [x] Return the child's committed branch over the same road and fetch it into a validated commit on the lead, imported from a bounded descriptor snapshot through Git stdin. Let the existing merge guard refuse overwriting current work.
- [x] Tests use actual local Git repositories behind two Machine implementations, including no remote, worktrees, binary changes, staged/unstaged differences, ignored selections, corruption and failed transfer. Prove the current remote run refusal before changing routing.

## Runtime ownership and routing

Files: create `packages/runtime/src/machines/isolated.ts`; modify runtime context/wiring, machines boot/kinds, account channels/rules, sessions, workspaces and serve-projects.

- [x] Route `workspaces.create {sub:{on:<connected computer>}}` to the isolated road before provider checks. Refuse unsupported/offline destinations before transfer. Keep existing provider behavior and explicit existing-project execution.
- [x] Serialize creation per root/destination and persist the keyed group before allocating a runtime. Restart reuses its existing machine/create key, including failed-start cleanup.
- [x] Allocate one checkout per child; bind its folder to the thread at its first start, preserve it across sends/restarts/row trimming, and prevent a thread from claiming another child's checkout.
- [x] Keep source project identity and tree/depth/count permissions; no fake destination project or access to the destination's ordinary folders.
- [x] Merge the selected child's branch using its checkout metadata and source workspace, without a Git hosting service.
- [x] Delete a thread without deleting sibling compute. A settled root releases only clean, returned checkouts after every child and pending start finishes; retain dirty/unreturned work and failed deletes.

## Contract, regression checks and preview

Files: protocol sub-thread contract, host run help/schema/skill, Rust MCP sub input/output, generated records/docs, focused runtime/host tests.

- [x] Make `run --on <computer>` and the tool's `on` input describe connected isolation and provider spawning consistently; preserve owner-only project registration and existing shared-folder runs.
- [x] Add truthful isolated-start metadata, with no provider delete countdown. Update Rust serialization and regenerate records.
- [x] Exercise concurrent siblings and unrelated roots, restart, cancellation, transfer failure, merge without a remote, permissions, held work and cleanup.
- [ ] Run changed-package typechecks, focused suites, laws and cold review; update PR 1766 and the existing cross-computer ticket. Generic main-based ancestry gates do not apply to the pinned v0.3.9 base.
- [ ] Build and sign a new local Electron preview; test on a throwaway host and provide it for the owner's live verification. No release or production restart.

Proposed runtime note: An isolated destination workspace belongs to a root/computer pair; checkout and return state belong to each child, and a turn ending never authorizes deleting shared compute.

Implementation rulings: each child has a workspace record because every command, Git op and checkpoint resolves its checkout there. The group alone owns compute. A durable releasing state precedes checkout deletion, and startup/reconnect completes interrupted releases. The initial pack carries only HEAD and its tree, not repository history; returned bundles are incremental against that original commit. The daemon snapshots a regular bundle by descriptor under a 64 MB cap and feeds it to Git stdin. Older daemons refuse with an update instruction, never a remote fallback. Plain-folder children are copied but retained during automatic cleanup because they have no Git return proof. Full source transfer is capped at 1 GB.
