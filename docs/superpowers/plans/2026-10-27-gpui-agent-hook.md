# GPUI Agent Hook Implementation Plan

> **For maintainers:** Execute this plan in the GPUI worktree and keep the driver out of release builds.

**Goal:** Let `gpui-agent` inspect and drive a development build of WSP's GPUI app for a short recorded demo.

**Architecture:** Add an optional Cargo feature that initializes `gpui-agent` during app startup and registers narrow probes for the visible thread and composer. Preserve the accessibility tree as the main control surface, including an accessible send button, and keep the driver dependency absent from normal binaries. Carry the upstream GPUI driver hooks as separately named vendor patches.

**Tech stack:** Rust, GPUI/GPUI Kit, `gpui-agent`, Cargo integration tests, shell patch tooling, Vitest.

---

### Task 1: Gate the driver integration

**Files:**
- Modify: `apps/desktop-gpui/app/Cargo.toml`
- Create: `apps/desktop-gpui/app/src/agent.rs`
- Modify: `apps/desktop-gpui/app/src/lib.rs`
- Modify: `apps/desktop-gpui/app/src/main.rs`
- Test: `apps/desktop-gpui/app/tests/agent_absent.rs`

1. Add optional driver dependencies and an `agent` feature.
2. Initialize the driver only when that feature is enabled.
3. Register read-only `thread` and `composer` probes.
4. Prove normal binaries contain no `gpui_agent` symbols and agent builds do.

### Task 2: Expose stable controls

**Files:**
- Modify: `apps/desktop-gpui/app/src/screens/composer/parts.rs`
- Test: `apps/desktop-gpui/app/tests/agent_drive.rs`

1. Give composer actions button roles and accessible names.
2. Launch a throwaway host and the app through `gpui-agent --headless`.
3. Click a thread, fill the composer, send the message, and save an annotated screenshot.

### Task 3: Carry and validate upstream hooks

**Files:**
- Create: `apps/desktop-gpui/patches/*.driver.patch`
- Modify: `apps/desktop-gpui/scripts/gpui-patches.sh`

1. Keep driver changes in topic patches applied after the base GPUI patches.
2. Teach patch save/check to preserve patch ordering.
3. Run `apps/desktop-gpui/scripts/gpui-patches.sh check`.

### Task 4: Make the tool reproducible

**Files:**
- Create: `.claude/skills/gpui-agent/SKILL.md`
- Modify: `.claude/skills/wsp-gpui/SKILL.md`
- Modify: `packages/host/test/gpui-skills.test.ts`
- Modify: `scripts/box-warm.sh`

1. Record when to use the tool and its pinned install command.
2. Install that revision idempotently on a warmed Linux box.
3. Validate the skill inventory and links.

### Task 5: Verify the handoff

1. Run Cargo formatting and Clippy with and without `--features agent`.
2. Run `agent_absent` both ways and `agent_drive` with the feature.
3. Run the focused GPUI skill test and patch check.
4. Run the GPUI hand-in check when the lane has enough disk and memory.
