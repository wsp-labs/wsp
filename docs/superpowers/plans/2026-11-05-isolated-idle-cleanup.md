# Isolated child cleanup implementation plan

> Execute inline under the owner's approved cleanup policy, using the existing hotfix worktree and PR.

Goal: release safe connected-computer checkouts on explicit settlement or after their shared runtime has been idle for 30 minutes, independently of sidebar auto-settlement and lead activity.

Architecture: keep the shared group's idleSince timestamp in isolated-groups. One host-clock timer calls the existing sweep; a busy member clears that group's idle clock. Cleanup continues through clean/release and its group lock, so active, dirty, ignored-data and unreturned-commit guards remain authoritative. Each eligible safe child may release without requiring an unsafe sibling to release. Cancel and drain the timer at host close.

Files and verification:

- [x] Extend packages/runtime/test/isolated-computer.test.ts's fixture with an injected Clock. Use jumpClock for the 30-minute boundary, reset on a new turn, explicit child settlement with a sibling, dirty/unreturned retention, restart with the original deadline and a new message after automatic release. Run the file before implementation and retain the failing evidence.
- [x] Update packages/runtime/src/machines/isolated.ts: persist idleSince on Group; record activity before agent starts; arm the earliest idle/retry deadline; select settled children or idle groups in sweep; run clean/release separately for eligible checkouts; record retention notices on the child and its lead. A failed cleanup retries after one minute. Preserve interrupted-release recovery and orphan allocation recovery.
- [x] Add isolated.close to cancel/drain scheduled work from packages/runtime/src/runtime.ts before daemon teardown. A returned branch in packages/runtime/src/projects/workspaces.ts asks sweep to reconsider a previously held settled/idle child.
- [x] Update skills/wsp/SKILL.md and its generated agent page with the 30-minute policy and filesystem recreation semantics. Record proposed area-note lines in the report rather than changing AGENTS.md.
- [x] Run the isolated/transfer/startup tests, CLI/MCP/skill contract checks, strict runtime/host/web typechecks and stable-tree laws. Build a separate local Electron preview, run packaged smoke checks against a matching throwaway host, push the same PR, and run scripts/pre-review.sh --changed 2508 last. Keep the established main-base and baseline replay/size limitations explicit.

Implementation notes: idle-clock writes use the same group lock as allocation/release, and release rechecks idle eligibility inside that lock. New starts wait for host hydration before claiming activity. A new turn makes an older settlement mark ineffective, so resuming a settled child receives the full grace period. Release timestamps feed the existing lost-checkpoint guards in rewind, fork and the chat view. The transcript gets a release notice or a retention reason; no provider countdown or new Settings toggle is introduced.

Area-note proposal: connected child resources follow a durable group idle clock and explicit effective settlement, independently of sidebar auto-folding; timer/ownership writes share the allocation lock, host close drains cleanup, and released checkout checkpoints remain unavailable after recreation.

A source checkout is retained while an unreleased or starting descendant needs it for branch return. A real-Git nested-child regression returns the descendant to its source first, then returns that source to the root; cleanup preserves both merge bases and releases the last runtime only after both returns.

Verification report: the first two policy regressions failed against the old root-only cleanup. The final focused run passes 120 tests across linked-computer lifecycle, real-Git transfer/return, Boat resumption/startup, CLI/MCP starts/deletes and shared web startup/transcript behavior. Ten added lifecycle tests cover idle expiry, explicit child settlement with an active sibling, new-turn grace/reset, restart deadlines, ignored/unreturned work, plain-folder retention, delete retry, resuming a settled child, allocation at expiry and nested return dependencies. Stable-tree laws pass 575 tests with 22 existing gated skips; the docs generator passes 20 cases and the runtime strict typecheck passes. The new local Electron preview is 0.3.9+boatfix.2506.8, built separately and signed ad hoc. Packaged smoke checks use a matching throwaway host version; the temporary source-package version is restored afterward. No installed app or production host is restarted.

The established release-gate limits remain: the main-based wrapper rejects the pinned stable hotfix's ancestry/merge assumptions, and prior-head Rust replay and standalone file-size failures are documented in the earlier startup report. This policy change does not claim those gates pass or release the app.

The final packaged smoke run passes eight cases with 18 gated skips on the shipped and development pages. The signed preview CLI reports wsp 0.3.9+boatfix.2506.8.


## Greptile follow-up: saved work and failure visibility

Three review findings were reproduced before the fixes (`/tmp/wsp-greptile3-red2.log`). A released child's failed restart persisted its startup failure but threw before emitting a changed row. Revive now pushes each retained thread head and drains the row queue before returning the error; the existing restart regression observes the pushed failed startup row. A pending child has no thread yet, so the inbox predicate skipped its creation failure. The existing inbox projection now includes failed creations, with a sidebar regression exercising selection under a folded opener.

The cleanup guard previously examined the worktree and HEAD alone, which can be clean while stashes and other saved branches hold unreturned work. It now refuses nonempty stashes and commits reachable from saved refs outside the base/returned history. WSP-owned checkpoint refs are excluded because their deliberate expiry follows checkout release. A real-Git lifecycle regression verifies retained stash contents and another branch's commit, safe cleanup with checkpoint refs, and successful cleanup after the saved work is returned. The skill and generated page document the retention rule.

Verification: 130 focused tests pass, including the expanded restart test and two new regression cases; 44 generated-doc, budget and contract-support checks pass. Strict runtime and web typechecks pass. Separate Electron preview .9 is built from these changes and its signature verifies; eight packaged-app smoke cases pass on shipped and development pages (18 unrelated gated cases skipped). The earlier main-base/replay/size limitations remain unchanged. No installed host was restarted, and nothing was merged or released.

Proposed runtime note: cleanup must check stashes and saved refs beyond the accepted base/returned history, excluding owned checkpoint refs; a persisted restart failure reaches windows through pushHead and rowsSent before start rejects. Proposed web note: a failed pending creation has no thread row but still belongs in Needs you, including under a folded opener.
