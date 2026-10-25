# branch_of_record: the one branch a brokered thread may push

TS: `projects/branch-of-record.ts` (179 lines). Rust: `src/projects/branch_of_record.rs`, `BranchOfRecordArea` over
`Rc<dyn BranchCtx>` and the free function `free_branch_name`. TS tests: branch-of-record.test.ts (22 cases; the
project-landing lines it also checks are wsp-core's `project_landing`), sub-spawn, places-fork. Tests:
`tests/branch_of_record.rs`.

`ctx.gh_exec(cmd, timeout)` stands for TS `ctx.moduleOf("local").backend(..).get(LOCAL_MACHINE_ID).exec(..)`, this
computer's machine.

## Rules

B1. `brokered_start(provider)` (branch-of-record.ts:21): a provider place whose broker answers ready.
B2. One gh line (branch-of-record.ts:24-29) runs `GH_PROMPT_DISABLED=1 <shell_line(["gh", ...args])>` on this computer
with `timeoutMs` `INLINE_EXEC_MS` (20,000). A rejected exec reads as exit -1 with its message as stderr. A non-zero
exit refuses `branch_of_record_unread_refusal(repo, <last line of stderr, else "gh exited <n>">)`. The answer is
stdout less one trailing `\n` or `\r\n`, split on tabs.
B3. A commit (branch-of-record.ts:31-34) is exactly 40 lowercase hex; else `branch_of_record_unread_refusal(repo, "no
commit in <JSON of the word or "">")`. A repository id (branch-of-record.ts:36-40) is JavaScript `Number(word)`, a safe
integer above 0 (`" 12"`, `"12\r"`, `"12.0"`, `"0x10"` and `"1e3"` pass as 12, 12, 12, 16 and 1000; `""`, `"-1"`,
`"1.5"` and `"9007199254740993"` fail); else `... "no repository id in <JSON>"`.
B4. Only a GitHub remote has a repo of record (branch-of-record.ts:43-47): `git_host_of(remote)` must be `github` and
`owner_repo_of` answer; else `brokered_github_only_refusal(project, <remote_host(remote), else the remote>, on)`.
B5. `repo_of_record(remote, project, on)` (branch-of-record.ts:51-55): `gh api repos/<o/n> --jq "[.id, .owner.login,
.name, .private] | @tsv"`; answers `{repoId, owner, name, public?}`, `public: true` only where `.private` printed
`false`, missing fields `""`.
B6. `reread_public(entry)` (branch-of-record.ts:62-72): only a public record, at most once per workspace per
`PUBLIC_REREAD_MS` (300,000), the stamp taken before the read. `gh api repositories/<id> --jq .private`; a failure
keeps it public. Only `true`, with the record's branch of record unchanged since the read began, drops `public` and
persists.
B7. Marketplaces (branch-of-record.ts:74-107): read again at most every `MARKETPLACES_REREAD_MS` (600,000). A read
keeps the last repos until it lands; it reads each catalog agent's marketplace file under the home
(`ctx.home_dir()`, the path less its `~/`; an unreadable file reads none), takes the unique repos the agent's
`github_repos` names, and keeps those `gh api repos/<r> --jq .private` answers `false` for (a failed read drops the
repo). While the first read of this host is the one held, a call waits for it at most `MARKETPLACES_WAIT_MS` (3,000)
on the clock and the timer is cancelled once it lands; every later call answers at once. `public_marketplaces()` is
the repos last read, `[]` before any.
B8. `pull_request_of_record(remote, number, project, on, pushes)` (branch-of-record.ts:113-119): `gh api
repos/<o/n>/pulls/<n> --jq "[.head.repo.id, .head.repo.owner.login, .head.repo.name, .head.ref,
.maintainer_can_modify, .base.repo.id, .head.sha, .head.repo.private] | @tsv"`; no head repo id refuses
`fork_head_gone_refusal(number)`; a pushing start on a fork (head id != base id) whose `maintainer_can_modify` is not
`true` refuses `fork_not_pushable_refusal(owner, number)`; answers `{repoId, owner, name, branch, startSha, public?}`.
B9. `free_branch_name(live, wanted, heads)` (branch-of-record.ts:124-133): taken are every live record's
`branchOfRecord.branch` and `sub.branch`, and every `refs/heads/<b>` in the second tab field of the heads lines; the
answer is `wanted`, else `wanted-2`, `wanted-3`, ... the first not taken.
B10. `named_of_record(o)` (branch-of-record.ts:139-147): `repo_of_record`; unless carried, the base's tip `gh api
repos/<owner>/<name>/commits/<base or HEAD> --jq .sha`; then `GIT_TERMINAL_PROMPT=0 git ls-remote --heads <remote>`
on this computer (a failure or non-zero exit reads no heads); branch `free_branch_name(sub_branch_of(name), heads)`.
Answers `{repoId, owner, name, public?, branch, startSha?}` (this path's key order; B8's writes `public` last).
B11. `with_op_token(entry, op, run)` (branch-of-record.ts:152-162): the thread is the record's `parentThreadId`, else
the entry's `opensThread`; none refuses `clone_no_thread_refusal(name)` and mints nothing. The token is minted as `op
<op>` scoped `{kind: thread, threadId, workspaceId, rootThreadId (the record's, else the thread)}` at clock now with
the kind's `turnRoad`; `run` gets `brokered_git_env(token, branchOfRecord)`; the token is revoked after, whether `run`
held or failed, a failed revoke warning `the <op> token of <name> was not taken away: <message>`.
B12. `land_on_record(entry, project, clone)` (branch-of-record.ts:167-179): the lines are `clone` or `["set -e",
...on_record_lines({checkout: project path, ofRecord, origin: owner_repo_of(remote) or "", held: entry
ofRecordHeld})]`, run through `env_exec(machine, with_env_from_input(<lines joined by \n>), env, CLONE_MS)` under an
op token for `clone`. A failure without `clone` fails with git's last stderr line, else last stdout line, else `git
exited <n>`; with `clone` it fails `clone_on_machine_failed_line({repo, on: place_name(place or wired), exitCode, said:
stderr or stdout, limitMs: CLONE_MS})`. Success with no `startSha` on the record and a last stdout line of 40 hex
writes it and persists; else nothing is written.

Invariant 3: B11's scope is read off the workspace's own record and entry, never a caller
(`invariant_3_an_op_token_is_scoped_to_the_workspaces_thread_not_the_asker`).

## Restart

The public reread stamps and the marketplaces read are memory: a restarted host reads both afresh at its first launch.

## After the port

- B6 keys the stamp by the live entry (`WeakMap`), so a re-attached workspace may reread at once; the port keys it by
  workspace id.
- B6 tells a changed branch of record by object identity; the port compares values.
- B10 writes `public` before `branch`; B8 after `startSha`: two key orders for one record type (ruling 1 asks a parity
  line for it).

## Tests

| Rule | Test |
|---|---|
| B1 | `a_brokered_start_needs_a_provider_and_a_ready_broker` |
| B2 | `gh_runs_without_a_prompt_on_this_computer_with_the_inline_timeout`, `a_gh_failure_is_refused_naming_the_repo_with_its_last_line`, `a_rejected_gh_exec_reads_as_exit_minus_one` |
| B3 | `a_commit_is_forty_lowercase_hex`, `a_repository_id_reads_as_javascript_number_does` |
| B4 | `a_remote_off_github_is_refused_naming_its_host` |
| B5 | `the_repo_of_record_is_read_by_id_and_public_only_on_false` |
| B6 | `a_public_repo_turned_private_drops_public_and_persists`, `a_reread_waits_five_minutes`, `a_reread_that_fails_keeps_it_public`, `a_branch_changed_meanwhile_is_not_written` |
| B7 | `the_first_launch_waits_at_most_three_seconds_for_the_marketplaces`, `later_launches_answer_at_once_and_reread_after_ten_minutes`, `only_public_marketplace_repos_are_kept` |
| B8 | `a_pull_request_thread_records_the_head_of_its_fork`, `a_push_to_a_fork_without_maintainer_edits_is_refused`, `a_review_of_such_a_fork_passes`, `a_gone_fork_is_refused` |
| B9 | `names_wsp_slug_numbered_past_every_branch_taken` |
| B10 | `a_named_branch_starts_from_the_bases_tip`, `a_carried_checkout_leaves_the_start_to_the_machine` |
| B11 | `invariant_3_an_op_token_is_scoped_to_the_workspaces_thread_not_the_asker`, `an_op_token_is_revoked_after_a_failure_too`, `no_thread_refuses_before_minting`, `a_failed_revoke_warns` |
| B12 | `land_on_record_writes_the_start_commit_the_machine_says`, `a_failed_clone_says_why_in_one_sentence`, `a_failed_checkout_says_gits_last_line`, `a_start_sha_already_recorded_is_kept` |
