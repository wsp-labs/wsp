# Boat startup implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** A pending read cannot break a Boat child's first turn, and a refused first turn leaves an accurate failed row and tracked cleanup.

**Architecture:** Preserve immutable tree identity when marks first create a thread record. Keep the queued-to-real-turn handoff's failure reporting alive after its temporary row is removed. Use the existing provider-confirmed cleanup operation; retain machine identity if cleanup fails. Keep permission rules intact.

**Tech Stack:** TypeScript runtime, Vitest, fake Boat backend; Classic Electron preview built from v0.3.9.

### Task 1: Regressions

Files: packages/runtime/test/sub-startup.test.ts, existing sub-queue/thread-marks/spawn-guard tests.

- [x] Reproduce pending read/mark and compare with unread control.
- [x] Refuse first start after provisioning; assert failed row, ancestry, parent notification and confirmed cleanup.
- [x] Refuse cleanup; assert no orphaned/untracked computer.
- [x] Cover queued restart and cancellation through focused existing suites; add a case if a changed boundary lacks coverage.

### Task 2: Runtime fix

Files: packages/runtime/src/machines/boot.ts, packages/runtime/src/machines/sub-queue.ts, packages/runtime/src/threads/sessions.ts.

- [x] Preserve pending ancestry in mark-created records.
- [x] Publish failed startup state even after queue ownership has handed off.
- [x] Clean up provisioned compute without losing identity when cleanup refuses.
- [x] Replace the undefined thread diagnostic with the actual resolved thread id.

### Task 3: Verify and hand over

- [x] Prove focused regressions fail on release sources and pass on changes.
- [x] Run affected queue/lifecycle/permission tests and relevant repository checks using Node 22.
- [x] Cold review, fix findings, report one proposed runtime note on the ticket.
- [x] Build a separate Classic preview app; do not publish a release or replace/restart the installed host silently.

## Validation

- Original release sources: six of the first seven regressions fail; unread control passes.
- Fixed sources: all ten startup regressions pass, including old partial marks, queued restart, cleanup refusal, parent failure delivery, concurrent retry and deletion.
- Runtime regression suites: 199 tests pass across startup, queue, lifecycle, marks, spawn guards and remote folder threads.
- Law suite: 32 files pass, 555 tests pass, 22 skip by their existing gates.
- Runtime typecheck and repository file-size check pass.
- Cold review identified old marks and cleanup ownership races; both fixed and re-reviewed without remaining findings.
- Packaged Classic app: four selected success/failure flow checks pass across shipped and development pages, with temporary homes and fake agents. No real provider or model calls.
- Preview packaging uses generated version 0.3.9+boatfix.2506 to trigger the app's existing guarded host replacement flow; source version manifests remain 0.3.9. A fresh lead in the new preview uses its matching MCP/CLI.
- Live Boat verification remains pending the owner's test. The installed host has not been restarted by this work.
- Release-branch integration gate is separate: the repository pre-review script assumes origin/main, whereas this hotfix intentionally starts at the shipped v0.3.9 tag.

Proposed runtime note: A pending thread record keeps the queued start's parent/root identity; a failed handoff publishes terminal failure and releases its computer only after provider-confirmed cleanup, without reclaiming a newer turn or deleted workspace.
