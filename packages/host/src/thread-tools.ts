// SPDX-License-Identifier: AGPL-3.0-only
// Discovery for a thread's tools, not authorization: the host still checks THREAD_OPS and the caller's tree.
// A tool with both a thread and an owner form (delete, projects_seed) keeps only the thread's existing rights.
// This allowlist is recorded for the Rust tool server; a newly added management tool is hidden by default.
export const THREAD_TOOLS: ReadonlySet<string> = new Set([
  "activity", "activity_end", "commit", "computers", "conversations", "delete", "exec", "fix", "fork",
  "merge_in", "projects", "projects_seed", "run", "send", "stop", "thread_head", "thread_read",
  "thread_rename", "thread_restore", "thread_settle", "threads", "threads_wait", "update", "wake",
  "worktree", "worktree_remove", "slate_catalog", "slate_read", "slate_state", "slate_write",
]);

/** A thread sees only the destinations its current project may reach, not every connected computer. */
export const THREAD_COMPUTERS_WORDS = "Inside a wsp thread, computers lists permitted destinations for that project. A missing computer may be connected but unavailable to this project. Use projects to find eligible registered folders.";
