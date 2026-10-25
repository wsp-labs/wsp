// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/protocol/src/projects.ts at a3b870a6e: its zod shapes and plain constants, written by the generator.

/// TS: packages/protocol/src/projects.ts:58 at a3b870a6e.
pub const ADD_FORMS_LINE: &str = "wsp add takes user@host or an ssh alias for a computer of yours, a folder on this computer for a project here, or a repo with --on <computer> for a project there: its url, owner/repo on github, or gitlab.com/owner/repo";

/// TS: packages/protocol/src/projects.ts:125 at a3b870a6e.
pub const INTO_TAKES_A_REPO_LINE: &str = "--into is where a repo is cloned; a folder is a project where it already is";

/// TS: packages/protocol/src/projects.ts:132 at a3b870a6e.
/// A JavaScript RegExp, flags "": the pattern source as the TS writes it.
pub const OWNER_REPO: &str = "^[\\w.][\\w.-]*\\/[\\w.][\\w.-]*$";

/// TS: packages/protocol/src/projects.ts:193 at a3b870a6e.
pub const NAME_A_PROJECT_LINE: &str = "name the project the thread runs in";

/// TS: packages/protocol/src/projects.ts:196 at a3b870a6e.
pub const BRANCH_ON_A_THREAD_LINE: &str = "a thread keeps its folder; start a new thread for another branch";

/// TS: packages/protocol/src/projects.ts:199 at a3b870a6e.
pub const BRANCH_HERE_ONLY_LINE: &str = "--branch takes a project on this computer, where wsp makes worktrees";

/// TS: packages/protocol/src/projects.ts:202 at a3b870a6e.
pub const BRANCH_OR_CWD_LINE: &str = "name a branch or a folder, not both";

/// TS: packages/protocol/src/projects.ts:243 at a3b870a6e.
pub const THREAD_WORKING_LINE: &str = "this thread is working; stop its turn first";

/// TS: packages/protocol/src/projects.ts:249 at a3b870a6e.
pub const WORKTREE_BUSY_LINE: &str = "a thread is working in that worktree; let its turn end or stop it first";

/// TS: packages/protocol/src/projects.ts:252 at a3b870a6e.
pub const WORKTREE_FORCE_LINE: &str = "only the person removes a worktree over files no commit holds; commit them, or ask them to remove it";

/// TS: packages/protocol/src/projects.ts:281 at a3b870a6e.
pub const KEPT_RUNNING_LINE: &str = "a thread is running";

/// TS: packages/protocol/src/projects.ts:282 at a3b870a6e.
pub const KEPT_ABANDONED_LINE: &str = "kept 7 days; remove it by hand";

/// TS: packages/protocol/src/projects.ts:349 at a3b870a6e.
pub const MEMORY_KEPT_CLAUSE: &str = "the memory its agent keeps on that computer stays";

/// TS: packages/protocol/src/projects.ts:390 at a3b870a6e.
pub const REGISTERING_LINE: &str = "already on this computer, registering";
