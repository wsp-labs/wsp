// SPDX-License-Identifier: AGPL-3.0-only
//! The repository a folder sits in, and its main folder when the folder is in a linked worktree. Port of
//! `packages/host/src/repo-root.ts` at 65f6b89c7; the spec is `spec/account.md`.

/// repo-root.ts:9-14: the nearest folder up from `folder`, itself included, holding a `.git` entry of any kind.
pub fn git_root_of(folder: &str) -> Option<String> {
    todo!()
}

/// repo-root.ts:18-28. A `.git` file with no `gitdir:` line, or pointing anywhere but `<main>/.git/worktrees/<name>`,
/// is `None`; an unreadable `.git` file is an error in the TS (it throws), `None` here.
pub fn main_worktree_of(folder: &str) -> Option<String> {
    todo!()
}
