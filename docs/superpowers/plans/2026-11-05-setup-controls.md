# Setup and computer controls implementation plan

**Goal:** Fix the bounded setup, removal and agents controls exposed after v0.3.10.

**Architecture:** Keep the stable Node host and existing operation contract. Reuse cloud init cancellation, agent-specific server switches, and the isolated runtime ownership checks. Only an owner-confirmed whole-computer removal can discard isolated work.

**Excluded:** SSH cancellation with installation rollback; accounting for inherited changes when retaining isolated checkouts; release or merge.

- [x] SSH Node: put the existing Homebrew bin on the same explicit command path for preflight, deploy, installed CLI and service. Reproduce with a fake Node outside ambient PATH in `packages/host/test/ssh-daemon-place.test.ts`.
- [x] Forced removal: carry the owner confirmation through `places/manage.ts`, recording `dropOn` and isolated release. Test ordinary removal still refuses, then whole-computer force removes dirty siblings and preserves source files in `isolated-computer.test.ts`.
- [x] Cleanup notices: keep the latest refusal in the workspace record, send one initial child-only notice, and stop appending changed retry reasons or notifying the lead. Keep successful release notices. Extend the existing idle-cleanup test.
- [x] Cloud wizard: expose existing `initCancel` with confirmation and the job's stoppable gate in `AddCloudDialog.tsx`. Test confirmation, failed cancellation and the uninterruptible seal stage in `add-cloud.test.tsx`.
- [x] Computers: keep agent sign-ins on Agents. Use compact removal wording and a bounded scroll area for retained-work details. Exercise `computers.test.tsx` and `places.test.ts`.
- [x] MCP detail: add per-agent switches for grouped servers; explain agents without support. Exercise grouped Codex and Claude rows in `agents-servers.test.tsx`.
- [x] Run focused tests and affected typechecks, inspect the entire diff, run the final pre-review gate against the maintained stable base, then open one PR for review.

## Area-note candidates

- Host: SSH preflight and the installed command use the same explicit tool path, including an existing Homebrew Node hidden by sudo.
- Runtime: An explicit owner force for whole-computer removal reaches isolated checkout release; automatic cleanup always retains its work checks.
- Web: Closing a setup runs it in the background; cancellation must call the host and respect its stoppable state.


## Verification and review

- Seven targeted regressions fail against v0.3.10 sources, including Node discovery, cloud cancellation, stale confirmation, mixed MCP controls, Computers sign-in state, forced removal, and retry notices. Sources were restored byte-for-byte after this check.
- Final focused run: 234 passed, one Linux-only case skipped, across seven files. This includes real Git checkouts, a held-transfer refusal, retry-reason changes without chat spam, cancellation rejection/retry, and a late successful cancellation response after a newer terminal event.
- Host and dependencies built successfully. Host, runtime and web typechecks passed separately. The built CLI/MCP contract and recorded-server checks passed 43 cases with one existing skip, using the released daemon 169 in the ignored local asset directory.
- Self-review and a cold review ran. Review caught stale cancellation confirmation, late cancellation response ordering, and force bypassing startup guards. All were fixed and checked with regressions. The cold recheck has no remaining findings.
- No live computer was removed or changed. No desktop app or host was restarted.
- Scope remains the stable v0.3.10 tree. The generic pre-review wrapper assumes origin/main, which contains the excluded Rust host port; direct tree laws and the stable target are checked separately.

## Behavior limits

- Cancelling cloud setup keeps the saved provider key. Sealing remains uninterruptible.
- Force removal still refuses while a child in its shared runtime is working or starting; stop it or wait for setup to finish first.
- A retention episode gets one initial notice in the child conversation. Its latest retry reason remains stored; it is not a new live UI status surface. Existing transcript notices are not deleted.
- Homebrew is a fallback on the recorded command path. An older Node already ahead of it remains the selected executable and still needs updating or a corrected path.
- SSH cancel-and-undo and inherited-work accounting remain separate follow-ups.
