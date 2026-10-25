# pull_request
Source: the functions of `packages/protocol/src/pull-request.ts` (582 lines) at a3b870a6e; its zod shapes (`PullRequestCheck`, `PullRequest`, `PullRequestFact`, `PullRequestKept`, `PullRequestSeen`, `MergeResult`, `FixResult`, `GitPrViewReply`, `PullRequestPage`, ...) and flat constants (`PR_POLL_MS`, `PR_POLL_IDLE_MS`, `CHECK_STATE_WORDS`, `CHECK_LOG_BYTES`, `SENT_HUNK_LINES`, `FIX_RESULT_FIELDS`, `AUTO_MERGE_OFF_LINE`, `REPLY_EMPTY_LINE`, `PR_REPLY_BODY_MAX`) are `wsp_wire::types::pull_request`. Tests: `packages/protocol/test/pull-request.test.ts` (18 cases, lines 29-305, all ported, none skipped) in `tests/pull_request.rs`; there is no `pinned` module.

## What it does
The one word a workspace's pull request reads as on a tile, a row, the pane and the command line, off a fact the host read or a record it kept of a settled one. The three messages that hand the agent a failed check, a conflict with the base, or comments off the pull request's page, each with the words of others folded onto one line or fenced. The lines `wsp fix`, `wsp merge` and `wsp update` print, and the refusals around them.

## Behaviour
1. `is_pull_request_fact(seen)`: false for None; true only for a fact (the TS reads `"headOid" in seen`). `is_pull_request_named(seen)`: false for None; true for a fact or a kept record, false for the unread sentence (`"number" in seen`) (pull-request.ts:106-113).
2. `pull_request_key(seen)`, the first that holds: not named is Unread; state merged is Merged; closed is Closed; a kept record (no fact) is Open; mergeable conflicting is Conflicts; any check `fail` is Failed; any check `pending` is Running; review `changes_asked` is ChangesAsked; review `approved` is Approved; draft is Draft; else Open (:145-157). A kept record never reads its kept checks, so a merged one reads merged after a restart without a read. Review `required`, and checks skipped or cancelled, change nothing.
3. `pull_request_word(seen)`: the PULL_REQUEST_WORDS row of the key; Conflicts is "conflicts with <base>" off the record's own base (:124-135, :160-164).
4. `capitalised(word)`: "" stays ""; else the first UTF-16 unit upper-cased, the rest as it is (:167-169).
5. `pull_request_mergeable(fact)`: state open, mergeable `mergeable`, and no check `fail`; a pending check does not stop it (:172-174).
6. `pull_request_unread_line(host)`, `pull_request_stopped_line(host, workspace)` (:178-185) and `pull_request_post_line(host)` (:580-582) are fixed sentences.
7. `tail_within(text, max)`: the text whole where its UTF-8 bytes are at most `max`; else the last `max` bytes, from just after their first newline where a byte follows that newline, else all of them; decoded, with every leading U+FFFD taken off (:191-197).
8. `check_failed_prompt(check, commit, log)` (:202-230), lines in order: `The check "<name>"[ in the <workflow> workflow] failed on commit <oid's first 7>[ (<subject>)][, and it says: "<description>"].`, each part through `one_line`; the subject only where present and not "", the description only where its trim is not "". Then, where the log has lines: "", the intro (", its last lines" where `truncated` or the tail was cut), the fence + "text", the tail of the lines joined by "\n" within CHECK_LOG_BYTES, the fence (`fence_for` of the tail). Then "", "The check: <link>" where a link is present. Then "", the ask. Joined by "\n".
9. `conflicts_prompt(base, branch, files)`: ", and resolve the conflicts in <files joined ", ">", or ", and resolve the conflicts" with no files (:234-237).
10. `no_such_item_refusal(item)`: comment, review, or "comment on a line" for a review comment, then the id (:243-246). `sent_lead_line(number)` (:250-251).
11. The author line of an item (private `sent_author`, :255-265): an author whose trim is "" is "a deleted account", else `one_line(author)`. The association with "_" made " " and folded; exactly owner, member or collaborator reads "<who> (<word>)"; anything else reads "<who>, from outside the repository", with " (<word>)" unless the word is absent, "" or "none".
12. `pull_request_send_prompt` (:268-302): items in the order asked; each found by kind and id among the page's comments, reviews or review comments, the first missing one an Err with `no_such_item_refusal`. A comment is its author line and its trimmed body fenced "text". A review adds ", a review marked <state with "_" made " ">". A review comment adds ", on <one_line(path)>[:<line>]", then the hunk (absent is "") split on "\n", lines starting "@@" dropped, the last SENT_HUNK_LINES kept, fenced "diff" unless that is "", then the body fenced "text". Each block is "[<i+1>. ]From <author line>:" (numbered only where there are two or more), its lines, then "". The whole: the lead, "", the blocks, "Make the changes that hold up, then commit and push.", joined by "\n".
13. `merged_line`: "merged #<n> (<method>)" where merged, else "#<n> merges when its checks pass" (:334-336). `updated_line`: 0 commits is "already has everything in <base>", 1 is "1 commit", else "<n> commits" (:339-342). `update_conflicts_line` joins files with ", " (:345-347). `child_pushed_line` (:351-353).
14. `fix_asked_line`, `fix_conflicts_line`, `fix_nothing_line` (:319-331), `no_pull_request_refusal` (:356-358), `no_such_check_refusal` (no "; it has ..." for an empty list, else the checks joined ", ") (:361-363), `check_not_failed_refusal` (:366-368), `merge_method_refusal` (allowed joined ", ") (:371-373), `not_open_refusal` (:379-381).

## Edge cases
- `tail_within("one\ntwo\nthree", 9)` is "three"; `tail_within("short", 100)` is "short" (test).
- `tail_within("abcdef", 3)` is "def" and `tail_within("ab\n", 2)` is "b\n": with no newline that has a byte after it, the half line stays.
- `tail_within("aé", 1)` is "": the half character decodes as U+FFFD and comes off. `tail_within("x\n\u{FFFD}y", 5)` is "y".
- `capitalised("ßa")` is "SSa"; `capitalised("\u{10428}x")` is unchanged; `capitalised("")` is "" (the last is a test).
- A log of no lines adds no log block, even with `truncated` true.
- A kept record with state open reads "open"; a fact with only skipped and cancelled checks reads "open" (test).
- A review the page carries with no id (one before the newest 100) never matches an item.
- A review comment with no hunk, or one of "@@" lines alone, carries no diff fence (the README.md comment in the test).
- Association "first_time_contributor" reads "(first time contributor)"; "none" reads no parenthesis; an author of "" is "a deleted account" (test).
- `merge_method_refusal(m, [])` ends "it allows " with nothing after.
- No items gives the lead, "", and the closing line.

## Behaviour nobody wrote down
- Rules 8.4 and 8.5: `commit.oid.slice(0, 7)` cuts 7 UTF-16 units (an oid is hex, so it never meets a pair); `capitalised` upper-cases the first UTF-16 unit, so a first character outside the BMP stays as it is.
- Rule 8.6: the trims of the description, the author and each body, and `one_line`'s `\s`, are JavaScript's whitespace.
- Rule 8.7: `/_/g` and `/^\uFFFD+/` are ported by hand; `one_line` and `fence_for` are the quote unit's.
- Rule 8.9: `capitalised` uses full-mapping uppercase ("ß" is "SS").
- Rule 8.11: ids, `#<number>`, `:<line>` and the commit count print through `js::number_string`.
- `tail_within` and CHECK_LOG_BYTES count UTF-8 bytes (TextEncoder), not UTF-16 units.

## Tapes
The pull request word on a thread's tile and row, in the Pull request pane and the `pr` slate source; the lines of `wsp fix`, `wsp merge` and `wsp update`; the bring back line for a thread under a lead; the messages the runtime sends the agent (runtime projects/workspaces.ts:981, :1026).

## Decisions
1. PULL_REQUEST_WORDS is the module `pull_request_words`: fixed words are constants, `conflicts` a function. `PullRequestKey` is an enum serialised camelCase as the TS keys.
2. The type guards answer bool over `Option<&PullRequestSeen>`; the enum's variant replaces the TS `in` checks.
3. The TS object parameters become arguments or structs: `check_failed_prompt` takes the whole `PullRequestCheck` (reading only name, workflow, link, description), a `CheckedCommit` and `Option<&CheckLog>`; `conflicts_prompt(base, branch, files)`; `pull_request_send_prompt` takes the page's three lists apart; `merged_line` takes the whole `MergeResult` and does not read `auto_armed`.
4. A send's item is this unit's `PageItem { kind: PageItemKind, id: f64 }`; it matches a page item by numeric value of the id.
5. The TS throw is `Err(HostError)` carrying the refusal sentence; the TS stamps no kind, so neither does the Rust.
6. A TS number parameter is f64 (`tail_within`'s max, `sent_lead_line`, `updated_line`, `not_open_refusal`); a list of names is `&[impl AsRef<str>]` or `&[&str]`.

## After the port
- `tail_within` says no line arrives half, yet it keeps a half line where the kept bytes hold no newline, or their only newline is the last byte ("abcdef" at 3 is "def").
- `tail_within` also takes off a real U+FFFD at the start of the kept line, not only one its own cut made.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `the_one_word_a_pull_request_reads_as::is_not_read_where_the_host_could_not_read_it_and_a_fact_is_told_from_the_sentenc`, `the_one_word_a_pull_request_reads_as::reads_merged_or_closed_off_what_the_record_kept_of_a_settled_one_which_is_no_fac` |
| 2, 3 | `the_one_word_a_pull_request_reads_as::is_open_draft_approved_or_changes_asked_for_off_the_review_when_nothing_outranks`, `the_one_word_a_pull_request_reads_as::takes_merged_then_closed_then_conflicts_then_a_failed_check_then_a_running_one_b`, `the_one_word_a_pull_request_reads_as::reads_its_word_off_one_key_which_the_pane_s_ink_reads_too`, `the_one_word_a_pull_request_reads_as::is_not_read_where_the_host_could_not_read_it_and_a_fact_is_told_from_the_sentenc`, `the_one_word_a_pull_request_reads_as::reads_merged_or_closed_off_what_the_record_kept_of_a_settled_one_which_is_no_fac` |
| 4 | `the_one_word_a_pull_request_reads_as::stands_capitalised_alone_in_the_settled_fold_s_slot` |
| 5 | `what_it_offers::offers_a_merge_only_on_an_open_one_the_host_says_merges_with_no_failed_check` |
| 6 | no test pins it |
| 7 | `the_message_that_asks_the_agent_to_fix_a_failed_check::carries_at_most_check_log_bytes_of_the_log_s_end_cut_at_a_line_s_start_and_says` |
| 8 | `the_message_that_asks_the_agent_to_fix_a_failed_check::*` |
| 9 | `the_message_that_asks_the_agent_to_fix_a_conflict_and_the_verbs_lines::asks_for_the_latest_base_merged_in_the_files_named_resolved_the_tests_run_and_a` |
| 10 | `the_message_that_sends_a_pull_request_s_comments_to_the_agent::keeps_every_fence_one_its_words_cannot_close_folds_a_name_onto_its_line_and_refu`, `the_message_that_sends_a_pull_request_s_comments_to_the_agent::quotes_one_comment_on_a_line_under_the_fixed_lead_with_its_author_their_associat` (the lead) |
| 11 | `the_message_that_sends_a_pull_request_s_comments_to_the_agent::names_a_deleted_account_as_one_and_keeps_github_s_word_for_every_author_outside`, `the_message_that_sends_a_pull_request_s_comments_to_the_agent::numbers_several_in_the_order_asked_reads_only_an_owner_a_member_or_a_collaborato` |
| 12 | `the_message_that_sends_a_pull_request_s_comments_to_the_agent::*` |
| 13 | `the_message_that_asks_the_agent_to_fix_a_conflict_and_the_verbs_lines::says_what_each_verb_did_in_one_line` |
| 14 | no test pins it |
