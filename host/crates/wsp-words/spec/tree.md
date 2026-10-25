# tree
Source: the functions of `packages/protocol/src/tree.ts` (166 lines) at a3b870a6e; its zod shapes (`GitStartOnReply`, `GitBranchCompareReply`, `GitMergeInReply`, `TreeRecord`, `TreeChild`, `TreeFact`, `MergeInResult`) and flat constant (`FIX_CHECK_OR_CHILD`) are `wsp_wire::types::tree`. Tests: no TS test file of its own in `packages/protocol/test`, and no `tests/tree.rs` yet. Its words are read by `packages/runtime/test/tree.test.ts`, `packages/runtime/test/sub-results.test.ts:159-169` and `apps/web/src/sidebar/tileCheckout.test.ts`, which compare against the function itself.

## What it does
The words of a lead's tree of children, each on a branch cut from the lead's: what a child's card says of its branch, what a fork says as it makes a child, the lines and refusals of `wsp merge in` and `wsp fix --child`, and the message a lead's thread is handed when a merge stopped on conflicts.

## Behaviour
1. `tree_words`: `ahead_of(n, lead)` is "<n> ahead of <lead>"; `NOT_PUSHED` "not pushed"; `NOT_COUNTED` "not counted"; `MERGED_INTO_LEAD` "merged into lead"; `conflicts_in(n)` is "conflicts in <n> file" for exactly 1, "files" otherwise (tree.ts:68-74).
2. The fork's lines: `child_started_line` "<child> starts on <branch> as pushed" (:77-79); `pushed_for_child_line` "pushed <branch> so <child> starts from your work" (:82-84); `uncommitted_stayed(n, lead)` "<n> uncommitted change in <lead> did not travel", "changes" for any n but 1 (:87-89); `fork_needs_push_line` "the child was not made, since <lead>'s branch could not reach it: <why>" (:92-94).
3. The merge's lines: `merged_in_line` "<lead>: merged <branch> from <child>, <n> commit", "commits" for any n but 1 (:97-99); `merge_conflicts_line` "<lead>: conflicts with <branch> in <paths joined by ", ">" (:102-104); `nothing_to_merge_line` "<lead>: nothing to merge from <child>" (:123-125); `fix_merge_child_line` "<lead>: asked <agent> to merge <child>" (:107-109).
4. `not_in_merge_line(thread_id, n)`: the id cut to its first 8 UTF-16 units, then "holds <n> commit not pushed on its machine; it is not in this merge." for exactly 1, "commits ... they are" otherwise (:112-113).
5. `merge_in_line(done, child_thread_id)`, in order: not merged reads `merge_conflicts_line(lead, branch, conflicts)`; merged with commits above 0 reads `merged_in_line`; merged with 0 commits reads `nothing_to_merge_line`. Where `unpushed` is present and above 0, a "\n" and `not_in_merge_line(child_thread_id, else child, unpushed)` follow, whichever line came first (:117-120).
6. Refusals: `lead_busy_refusal(lead, ids)` names each id by `thread_word` (its first 8 units) joined by ", " in "<lead> has a turn running in thread <ids>; merge once it ends, or from inside that turn" (:128-131); `not_the_leads_child_refusal` (:134-136); `merge_into_own_refusal` "thread <thread_word(id)> may merge a child only into its own folder" (:139-141); `child_on_no_branch_refusal` (:144-146); `no_remote_for_tree_line` (:152-154); `cannot_push_from_line` "can't push from <computer>: <why>" (:157-159).
7. `merge_child_prompt(o)`: "Fetch <remote> <childBranch> and merge it into <leadBranch> with a merge commit; resolve the conflicts in <files joined by ", ">; run the tests; commit.", and "; resolve the conflicts" alone where the list is empty (:163-166).

## Edge cases
- `merge_in_line` of a merge of 3 commits with 2 unpushed, lead "thread <lead8>", child "thread <kid8>" and the kid's id is two lines: "thread <lead8>: merged <branch> from thread <kid8>, 3 commits" and "<kid8> holds 2 commits not pushed on its machine; they are not in this merge." (sub-results.test.ts:165).
- `merge_in_line` with `unpushed: 0` is one line.
- `merge_conflicts_line("l", "b", &[])` is "l: conflicts with b in " (a trailing space).
- `lead_busy_refusal("l", &[])` is "l has a turn running in thread ; merge once it ends, or from inside that turn".
- `uncommitted_stayed(0.0, "l")` is "0 uncommitted changes in l did not travel"; `tree_words::conflicts_in(0.0)` is "conflicts in 0 files".
- `tree_words::ahead_of(1.5, "m")` is "1.5 ahead of m".

## Behaviour nobody wrote down
- Rules 8.1 and 8.4: the 8-unit id cut in `not_in_merge_line` and `thread_word` counts UTF-16 units.
- Rule 8.5: that cut never leaves half a surrogate pair.
- Rule 8.11: every count in a sentence is `String(n)`, so `number_string`.

## Tapes
`wsp merge in` and its tool's answer; the notice of a child made on its lead's branch (the fork's `notice`, printed by the verb that made it and in its tool's answer); `wsp fix --child`; a child's card on the lead's tile (`TREE_WORDS`); the message a lead's thread is handed after a merge stops on conflicts.

## Decisions
1. TREE_WORDS is the module `tree_words`: fixed words are constants, sentences functions taking `n: f64`.
2. `merge_child_prompt` takes `MergeChild { lead_branch, child_branch, remote, conflicts }` for the TS inline object.
3. `readonly string[]` parameters are `&[impl AsRef<str>]`; `childThreadId?: string` is `Option<&str>`.

## After the port
- `merge_in_line` with no `child_thread_id` passes the child's workspace name to `not_in_merge_line`, which cuts it to 8 units as if it were a thread id: a child named "child-long-name" reads "child-lo holds 1 commit not pushed ...".
- A merge that stopped on conflicts still adds the unpushed line, "not in this merge", under a merge that took nothing.

## Rules pinned
| Rule | Test |
|---|---|
| 1 to 7 | not pinned: `tests/tree.rs` is not written; the TS reaches these words only through the runtime's and the app's tests named above |
