// SPDX-License-Identifier: AGPL-3.0-only
use super::tests::{committed, git_in, here, paths_of, CAP};
use super::*;

/// A turn that checks out another branch and edits one file: the card is that one file and the checkout line, not
/// the whole branch the checkout brought.
#[tokio::test]
async fn a_turn_that_checks_out_a_branch_and_edits_one_file_lists_that_file_and_the_checkout() {
    let dir = committed(&["a.txt"]);
    let p = dir.path();
    git_in(p, &["checkout", "-q", "-b", "other"]);
    std::fs::write(p.join("b.txt"), "b\n").unwrap();
    std::fs::write(p.join("c.txt"), "c\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "other work"]);
    git_in(p, &["checkout", "-q", "main"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["checkout", "-q", "other"]);
    std::fs::write(p.join("a.txt"), "edited\n").unwrap();
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["a.txt"]);
    assert_eq!(reply.moved, vec!["Checked out other".to_owned()]);
}

/// A turn that pulls and edits nothing: the pull is one line and no files are its own.
#[tokio::test]
async fn a_turn_that_pulls_and_edits_nothing_shows_the_pull_line_and_no_files() {
    let up = committed(&["a.txt"]);
    let work = tempfile::tempdir().unwrap();
    let dir = work.path().join("clone");
    git_in(work.path(), &["clone", "-q", up.path().to_str().unwrap(), dir.to_str().unwrap()]);
    let p = dir.as_path();
    std::fs::write(up.path().join("b.txt"), "b\n").unwrap();
    git_in(up.path(), &["add", "-A"]);
    git_in(up.path(), &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "more"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["pull", "-q"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert!(reply.files.is_empty(), "{:?}", paths_of(&reply));
    assert_eq!(reply.moved, vec!["Pulled into main".to_owned()]);
}

/// A turn that commits two edits and moves HEAD no other way: both files are its own and it names no move.
#[tokio::test]
async fn a_turn_that_commits_two_edits_lists_both_and_names_no_move() {
    let dir = committed(&["a.txt", "b.txt"]);
    let p = dir.path();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    std::fs::write(p.join("a.txt"), "edited a\n").unwrap();
    std::fs::write(p.join("b.txt"), "edited b\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "two edits"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["a.txt", "b.txt"]);
    assert!(reply.moved.is_empty());
}

/// A merge the turn resolved by hand lists every file it brought and the merge line: the file both sides changed
/// and the file only the other side added.
#[tokio::test]
async fn a_turn_that_resolves_a_merge_lists_every_file_it_brought_and_the_merge_line() {
    let dir = committed(&["a.txt", "shared.txt"]);
    let p = dir.path();
    git_in(p, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(p.join("shared.txt"), "feature side\n").unwrap();
    std::fs::write(p.join("feat.txt"), "feat\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "feature"]);
    git_in(p, &["checkout", "-q", "main"]);
    std::fs::write(p.join("shared.txt"), "main side\n").unwrap();
    std::fs::write(p.join("other.txt"), "other\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "main moves"]);
    git_in(p, &["checkout", "-q", "feature"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    // The merge conflicts on shared.txt, so it exits non-zero and git_in cannot run it.
    let merge = std::process::Command::new("git")
        .args(["-c", "commit.gpgsign=false", "merge", "main"])
        .current_dir(p)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@x")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@x")
        .output()
        .unwrap();
    assert!(!merge.status.success(), "the merge was expected to conflict");
    std::fs::write(p.join("shared.txt"), "resolved\n").unwrap();
    git_in(p, &["add", "shared.txt"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "--no-edit"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["other.txt", "shared.txt"]);
    assert_eq!(reply.moved, vec!["Merged main into feature".to_owned()]);
}

/// A lead that merges two branches into a new one, the first by fast-forward and the second by a merge commit, and
/// then renames the branch: the card is every file the two merges brought, and the rename writes no commit.
#[tokio::test]
async fn a_turn_that_merges_two_branches_lists_every_file_the_merges_brought() {
    let dir = committed(&["server.js", "tip.js"]);
    let p = dir.path();
    git_in(p, &["checkout", "-q", "-b", "split"]);
    std::fs::write(p.join("tip.js"), "split\n").unwrap();
    std::fs::write(p.join("tip.test.js"), "split test\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "split a bill"]);
    git_in(p, &["checkout", "-q", "-b", "health", "main"]);
    std::fs::write(p.join("server.js"), "health\n").unwrap();
    std::fs::write(p.join("server.test.js"), "health test\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "health"]);
    git_in(p, &["checkout", "-q", "main"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["checkout", "-q", "-b", "walk/merge"]);
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "split"]);
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "health"]);
    git_in(p, &["branch", "-m", "walk/merge", "walk/merge-2"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["server.js", "server.test.js", "tip.js", "tip.test.js"]);
    assert_eq!(
        reply.moved,
        vec![
            "Checked out walk/merge".to_owned(),
            "Merged split into walk/merge".to_owned(),
            "Merged health into walk/merge".to_owned(),
            "Renamed walk/merge to walk/merge-2".to_owned(),
        ]
    );
}

/// A clone on a branch of its own with main moved on upstream, the commit on the branch and the fetch done.
fn ahead_and_behind() -> (tempfile::TempDir, tempfile::TempDir, PathBuf) {
    let up = committed(&["a.txt"]);
    let work = tempfile::tempdir().unwrap();
    let dir = work.path().join("clone");
    git_in(work.path(), &["clone", "-q", up.path().to_str().unwrap(), dir.to_str().unwrap()]);
    git_in(&dir, &["checkout", "-q", "-b", "fix/x"]);
    std::fs::write(dir.join("own.txt"), "own\n").unwrap();
    git_in(&dir, &["add", "-A"]);
    git_in(&dir, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "own"]);
    std::fs::write(up.path().join("up.txt"), "up\n").unwrap();
    git_in(up.path(), &["add", "-A"]);
    git_in(up.path(), &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "up"]);
    git_in(&dir, &["fetch", "-q"]);
    (up, work, dir)
}

/// A builder's own merge of main into its branch and its soft-reset squash name the branch HEAD was on, so neither
/// reads as a merge into main while the pull request is still open.
#[tokio::test]
async fn a_merge_of_main_and_a_reset_onto_it_name_the_branch_they_moved() {
    let (_up, _work, dir) = ahead_and_behind();
    let p = dir.as_path();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "origin/main"]);
    git_in(p, &["reset", "-q", "--soft", "origin/main"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "squash"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(reply.moved, vec!["Merged origin/main into fix/x".to_owned(), "Reset fix/x to origin/main".to_owned()]);
}

/// A turn whose end snapshot an older daemon took, with no record of HEAD on it, names no branch rather than one
/// it cannot know.
#[tokio::test]
async fn an_end_snapshot_with_no_record_of_head_names_no_branch() {
    let (_up, _work, dir) = ahead_and_behind();
    let p = dir.as_path();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "origin/main"]);
    let taken = git_snapshot(&here(), p).await.unwrap().commit;
    let message = git_in(p, &["show", "-s", "--format=%B", &taken]);
    let older = message.lines().filter(|l| !l.starts_with("head ")).collect::<Vec<_>>().join("\n");
    let tree = format!("{taken}^{{tree}}");
    let parent = format!("{taken}^");
    let to = git_in(p, &["commit-tree", &tree, "-p", &parent, "-m", &older]).trim().to_owned();
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(reply.moved, vec!["Merged origin/main".to_owned()]);
}

/// A move made before a checkout in the same turn names the branch it was made on, not the one the turn ended on.
#[tokio::test]
async fn a_merge_before_a_checkout_names_the_branch_it_was_made_on() {
    let (_up, _work, dir) = ahead_and_behind();
    let p = dir.as_path();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "origin/main"]);
    git_in(p, &["checkout", "-q", "main"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(reply.moved, vec!["Merged origin/main into fix/x".to_owned(), "Checked out main".to_owned()]);
}

/// On a detached HEAD a merge and a reset say so rather than naming a branch nothing moved.
#[tokio::test]
async fn a_merge_and_a_reset_on_a_detached_head_say_a_detached_head() {
    let (_up, _work, dir) = ahead_and_behind();
    let p = dir.as_path();
    git_in(p, &["checkout", "-q", "--detach"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "origin/main"]);
    git_in(p, &["reset", "-q", "--hard", "origin/main"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(reply.moved, vec!["Merged origin/main into a detached HEAD".to_owned(), "Reset a detached HEAD to origin/main".to_owned()]);
}

/// A pull that rebases is one line naming the branch it returned to, though HEAD was detached for its steps.
#[tokio::test]
async fn a_pull_that_rebases_is_one_line_naming_the_branch() {
    let (_up, _work, dir) = ahead_and_behind();
    let p = dir.as_path();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "pull", "-q", "--rebase", "origin", "main"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(reply.moved, vec!["Pulled into fix/x".to_owned()]);
}

/// A turn that ends with its rebase stopped on a conflict names the branch being rebased, which git keeps beside
/// the rebase while HEAD is detached under it.
#[tokio::test]
async fn a_rebase_stopped_on_a_conflict_names_the_branch_it_rebases() {
    let (up, _work, dir) = ahead_and_behind();
    let p = dir.as_path();
    std::fs::write(up.path().join("a.txt"), "theirs\n").unwrap();
    git_in(up.path(), &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "theirs"]);
    std::fs::write(p.join("a.txt"), "mine\n").unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "mine"]);
    git_in(p, &["fetch", "-q"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    // The rebase stops on a.txt, so it exits non-zero and git_in cannot run it.
    let rebase = std::process::Command::new("git")
        .args(["-c", "commit.gpgsign=false", "rebase", "origin/main"])
        .current_dir(p)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@x")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@x")
        .output()
        .unwrap();
    assert!(!rebase.status.success(), "the rebase was expected to stop on a conflict");
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(reply.moved, vec!["Rebased fix/x onto origin/main".to_owned()]);
}

/// A rebase of a detached HEAD has no branch to return to and says so.
#[tokio::test]
async fn a_rebase_of_a_detached_head_says_a_detached_head() {
    let (_up, _work, dir) = ahead_and_behind();
    let p = dir.as_path();
    git_in(p, &["checkout", "-q", "--detach"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "rebase", "-q", "origin/main"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(reply.moved, vec!["Rebased a detached HEAD onto origin/main".to_owned()]);
}

/// A turn that leaves its start HEAD and comes back to it keeps every move line: the window is the entries the two
/// snapshots bound by count, not a match on the start sha that would end at the first row.
#[tokio::test]
async fn a_turn_that_leaves_and_returns_to_its_start_keeps_the_move_lines() {
    let dir = committed(&["a.txt"]);
    let p = dir.path();
    git_in(p, &["branch", "other"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["checkout", "-q", "other"]);
    git_in(p, &["checkout", "-q", "main"]);
    std::fs::write(p.join("a.txt"), "edited\n").unwrap();
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["a.txt"]);
    assert_eq!(reply.moved, vec!["Checked out other".to_owned(), "Checked out main".to_owned()]);
}

/// A turn that commits a file and then moves HEAD keeps the file it committed: each commit contributes its own
/// files, collected one by one, whatever moved HEAD after it, and the move is only a line.
#[tokio::test]
async fn a_turn_that_commits_then_checks_out_keeps_the_commit_and_names_the_checkout() {
    let dir = committed(&["a.txt"]);
    let p = dir.path();
    git_in(p, &["branch", "other"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    std::fs::write(p.join("own.txt"), "own\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "own work"]);
    git_in(p, &["checkout", "-q", "other"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["own.txt"]);
    assert_eq!(reply.moved, vec!["Checked out other".to_owned()]);
}

/// A HEAD move made right before the start snapshot is not the turn's: the window is the reflog length the snapshot
/// stamped, so an entry below it is left out even in the same wall-clock second the turn starts in.
#[tokio::test]
async fn a_move_just_before_the_start_snapshot_is_not_the_turns() {
    let dir = committed(&["a.txt"]);
    let p = dir.path();
    git_in(p, &["branch", "other"]);
    git_in(p, &["checkout", "-q", "other"]);
    git_in(p, &["checkout", "-q", "main"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    std::fs::write(p.join("a.txt"), "edited\n").unwrap();
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["a.txt"]);
    assert!(reply.moved.is_empty(), "{:?}", reply.moved);
}

/// A rebase is one line and the edits standing at the turn's end, not the commits it replayed: the turn rebases
/// onto main and then edits a file, and the card is that file and "Rebased feature onto main".
#[tokio::test]
async fn a_turn_that_rebases_names_it_once_and_lists_only_the_end_edits() {
    let dir = committed(&["a.txt"]);
    let p = dir.path();
    git_in(p, &["checkout", "-q", "-b", "feature"]);
    std::fs::write(p.join("feat.txt"), "feat\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "feature work"]);
    git_in(p, &["checkout", "-q", "main"]);
    std::fs::write(p.join("main.txt"), "main\n").unwrap();
    git_in(p, &["add", "-A"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "main work"]);
    git_in(p, &["checkout", "-q", "feature"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "rebase", "-q", "main"]);
    std::fs::write(p.join("after.txt"), "after\n").unwrap();
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["after.txt"]);
    assert_eq!(reply.moved, vec!["Rebased feature onto main".to_owned()]);
}

/// Two merges that change one file count every line both brought, as the branch's own diff does.
#[tokio::test]
async fn two_merges_that_change_one_file_count_every_line_both_brought() {
    let dir = committed(&["server.js"]);
    let p = dir.path();
    let base = std::fs::read_to_string(p.join("server.js")).unwrap();
    git_in(p, &["checkout", "-q", "-b", "a"]);
    std::fs::write(p.join("server.js"), format!("a1\na2\n{base}")).unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "a"]);
    git_in(p, &["checkout", "-q", "-b", "b", "main"]);
    std::fs::write(p.join("server.js"), format!("{base}b1\nb2\nb3\n")).unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "b"]);
    git_in(p, &["checkout", "-q", "main"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "a"]);
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "b"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    let counts: Vec<_> = reply.files.iter().map(|f| (f.path.as_str(), f.additions, f.deletions)).collect();
    assert_eq!(counts, vec![("server.js", 5, 0)]);
}

/// A pull that stops on a conflict and is finished by `git commit` is a pull: one line, and none of the files
/// upstream brought.
#[tokio::test]
async fn a_pull_finished_by_hand_after_a_conflict_is_one_pull_line() {
    let up = committed(&["a.txt"]);
    let work = tempfile::tempdir().unwrap();
    let dir = work.path().join("clone");
    git_in(work.path(), &["clone", "-q", up.path().to_str().unwrap(), dir.to_str().unwrap()]);
    let p = dir.as_path();
    std::fs::write(up.path().join("a.txt"), "theirs\n").unwrap();
    std::fs::write(up.path().join("up.txt"), "up\n").unwrap();
    git_in(up.path(), &["add", "-A"]);
    git_in(up.path(), &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "theirs"]);
    std::fs::write(p.join("a.txt"), "mine\n").unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "mine"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    // The pull conflicts on a.txt, so it exits non-zero and git_in cannot run it.
    let pull = std::process::Command::new("git")
        .args(["-c", "commit.gpgsign=false", "pull", "--no-rebase"])
        .current_dir(p)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@x")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@x")
        .output()
        .unwrap();
    assert!(!pull.status.success(), "the pull was expected to conflict");
    std::fs::write(p.join("a.txt"), "resolved\n").unwrap();
    git_in(p, &["add", "a.txt"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "--no-edit"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert!(reply.files.is_empty(), "{:?}", paths_of(&reply));
    assert_eq!(reply.moved, vec!["Pulled into main".to_owned()]);
}

/// A checkout on main whose server.js has three lines added at its bottom on branch b.
fn bottom_on_b() -> (tempfile::TempDir, String) {
    let dir = committed(&["server.js"]);
    let p = dir.path();
    let base = std::fs::read_to_string(p.join("server.js")).unwrap();
    git_in(p, &["checkout", "-q", "-b", "b"]);
    std::fs::write(p.join("server.js"), format!("{base}b1\nb2\nb3\n")).unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "b"]);
    git_in(p, &["checkout", "-q", "main"]);
    (dir, base)
}

fn counts_of(reply: &GitDiffReply) -> Vec<(&str, u64, u64)> {
    reply.files.iter().map(|f| (f.path.as_str(), f.additions as u64, f.deletions as u64)).collect()
}

/// A commit and a later merge that both change one file count every line both brought, as the branch's own diff does.
#[tokio::test]
async fn a_commit_then_a_merge_on_one_file_count_every_line_both_brought() {
    let (dir, base) = bottom_on_b();
    let p = dir.path();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    std::fs::write(p.join("server.js"), format!("a1\na2\n{base}")).unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "top"]);
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "b"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(counts_of(&reply), vec![("server.js", 5, 0)]);
    assert_eq!(reply.moved, vec!["Merged b into main".to_owned()]);
}

/// A merge, a commit, its amend and an edit left uncommitted, all on one file: the card's lines are the turn's whole
/// range, its start HEAD against its end worktree, as `git diff --numstat` reads them.
#[tokio::test]
async fn any_mix_of_commits_merges_and_edits_on_one_file_counts_the_turns_whole_range() {
    let (dir, base) = bottom_on_b();
    let p = dir.path();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    let start = git_in(p, &["rev-parse", "HEAD"]);
    git_in(p, &["-c", "commit.gpgsign=false", "merge", "-q", "--no-edit", "b"]);
    let merged = std::fs::read_to_string(p.join("server.js")).unwrap();
    std::fs::write(p.join("server.js"), format!("a1\na2\n{merged}")).unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "top"]);
    std::fs::write(p.join("server.js"), format!("a1\na2\na3\n{merged}")).unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-a", "--amend", "--no-edit"]);
    std::fs::write(p.join("server.js"), format!("a1\na2\na3\n{}b1\nb2\nb3\nc1\n", base.replacen("server.js 1\n", "", 1))).unwrap();
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    let numstat = git_in(p, &["diff", "--numstat", start.trim(), &to]);
    let whole: Vec<(&str, u64, u64)> = numstat
        .lines()
        .map(|l| {
            let mut cols = l.split('\t');
            let added = cols.next().unwrap().parse().unwrap();
            let deleted = cols.next().unwrap().parse().unwrap();
            (cols.next().unwrap(), added, deleted)
        })
        .collect();
    assert_eq!(whole, vec![("server.js", 7, 1)]);
    assert_eq!(counts_of(&reply), whole);
}

/// An edit that stood uncommitted before the turn and that it never touched is not the turn's, even when the turn's
/// commit and its own uncommitted edit run on into its end worktree as one range.
#[tokio::test]
async fn an_edit_left_uncommitted_before_the_turn_stays_off_its_card() {
    let dir = committed(&["a.txt", "b.txt", "c.txt"]);
    let p = dir.path();
    std::fs::write(p.join("a.txt"), "the person's own edit\n").unwrap();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    std::fs::write(p.join("b.txt"), "committed\n").unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "b", "b.txt"]);
    std::fs::write(p.join("c.txt"), "left uncommitted\n").unwrap();
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(paths_of(&reply), vec!["b.txt", "c.txt"]);
}

/// A turn that starts an orphan branch and commits one file lists that file as new, never the files of the branch it
/// left, which git's reflog puts just below the orphan's first commit.
#[tokio::test]
async fn an_orphan_branchs_first_commit_counts_against_the_empty_tree() {
    let dir = committed(&["a.txt"]);
    let p = dir.path();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["checkout", "-q", "--orphan", "fresh"]);
    git_in(p, &["rm", "-q", "-r", "--cached", "."]);
    std::fs::remove_file(p.join("a.txt")).unwrap();
    std::fs::write(p.join("b.txt"), "b\n").unwrap();
    git_in(p, &["add", "b.txt"]);
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "fresh"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(counts_of(&reply), vec![("b.txt", 1, 0)]);
}

/// A commit after HEAD's reflog was emptied has no entry below it and counts against the HEAD the turn started on,
/// never the empty tree.
#[tokio::test]
async fn a_commit_after_the_reflog_was_emptied_counts_against_the_start_head() {
    let dir = committed(&["a.txt", "b.txt"]);
    let p = dir.path();
    git_in(p, &["reflog", "expire", "--expire=now", "--all"]);
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    std::fs::write(p.join("a.txt"), "edited\n").unwrap();
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-am", "edit a"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(counts_of(&reply), vec![("a.txt", 1, 30)]);
}

/// A turn that commits the staged part of an edit left from before it keeps that file, counted over its whole range.
#[tokio::test]
async fn a_commit_of_part_of_an_edit_left_before_the_turn_keeps_its_file() {
    let dir = committed(&["f.txt"]);
    let p = dir.path();
    let base = std::fs::read_to_string(p.join("f.txt")).unwrap();
    std::fs::write(p.join("f.txt"), format!("s1\ns2\n{base}")).unwrap();
    git_in(p, &["add", "f.txt"]);
    std::fs::write(p.join("f.txt"), format!("s1\ns2\n{base}u1\n")).unwrap();
    let from = git_snapshot(&here(), p).await.unwrap().commit;
    git_in(p, &["-c", "commit.gpgsign=false", "commit", "-q", "-m", "staged"]);
    let to = git_snapshot(&here(), p).await.unwrap().commit;
    let reply = git_turn(&here(), p, Path::new("/"), &from, &to, None, CAP).await.unwrap();
    assert_eq!(counts_of(&reply), vec![("f.txt", 3, 0)]);
}
