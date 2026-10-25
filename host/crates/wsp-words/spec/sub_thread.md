# sub_thread
Source: the functions of `packages/protocol/src/sub-thread.ts` (557 lines) at a3b870a6e; its zod shapes (`SubOut`, `SubStarted`) and flat constants (`SUB_SEED_MAX_BYTES`, `SUB_SHALLOW_LINE`, `SUB_TURN_GONE_LINE`, `SUB_DROPPED_LINE`) are `wsp_wire::types::sub_thread`. Tests: sub-thread.ts has no test file of its own; `packages/protocol/test/push-refused.test.ts` (3 of 3 cases ported, as `push_refused_of` in `tests/sub_thread.rs`), plus `pinned` there (8 cases, every value printed by the TS under node).

## What it does
The words of a sub-thread, a thread a lead starts with `run --on` on a cloud machine of its own: the sizes it may take, the preamble that tells its agent how to fetch the lead's pushed commit and push its own branch, every refusal and line its start passes with, the strip and spawn words a client shows, and the lines of its end (what is not pushed, the delete, the finished line to the lead).

## Behaviour
1. `sub_size_word` is `<cpu>x<memGb>`, memGb `Number((memMb / 1024).toFixed(1))` (sub-thread.ts:68, units.ts:123,239). `sub_size_of`: the word trimmed must be `^(\d+)x(\d+(\.\d+)?)$`, memMb `js_round(gb * 1024)`, and both cpu and memMb above 0, else None (:71, units.ts:244-250). `sub_size_words`: "name, cpu vCPU, memGb GB" (:74). `size_name_of`: the name, else the size word (:79).
2. `sub_next_size`: among sizes with strictly more memory, the smallest by memMb, ties in the sold order (a stable sort); None where none (:83-85). `sub_sizes_sold`: every size where no plan is known, else those whose size word the plan's list names, in the sizes' order; an empty plan list sells none (:89-90).
3. `sub_folder_of`: `ownerRepoOf(remote)`, else the remote as given; its last "/" part, one trailing ".git" off, under "/root/" (:93-96, start.ts:39-42). `sub_branch_of`: the machine lowercased, each run outside `[a-z0-9]` one "-", dashes trimmed both ends, cut to 48, trailing dashes trimmed again; "wsp/" before it, "wsp/sub-thread" where the slug is empty (:99-102).
4. `sub_branch_name_ok`, in order: false for "", "HEAD" (that case alone), a leading "-" or "/", a trailing "/" or "."; then for any of `\x00-\x20`, `\x7f`, `~^:?*[\`, or "..", "//", "@{"; then every "/" part must be non-empty, not start with "." and not end with ".lock" (:107-111).
5. `sub_preamble` (:129-155): where it runs with `sub_size_words`, the lead cut to 8, the folder sentence (empty, or "holds only the git-ignored files the person picked: a, b") (:158-159); a blank line; the git lines (:118-121); a blank line; the push rule, the delete ("as soon as your last turn ends" at 0 minutes, else "<n> minutes after your last turn ends", :162), and SUB_SHALLOW_LINE where brokered; a blank line; "The task:". Brokered git lines: `git init -q`, `git remote add -t '<branch>*' origin <repo>` (the refspec `shellQuote`d), a depth-1 fetch of the ref, `git checkout -q -b <branch> FETCH_HEAD`. Otherwise: the setup line, init, a plain remote add, a blobless fetch of the base branch, a checkout of the base sha. `sub_resume_preamble` (:167-191) fetches the sub-thread's own branch and checks out FETCH_HEAD where it pushed, else the lead's base as the preamble does, and ends "The message:". `sub_opening`: preamble, a blank line, the task (:202).
6. Every refusal is `refusal(happened, fix, kind)`: the message is happened (trimmed at the end), a "." unless it ends in `.!?:`, a space, the fix; the fix rides beside it (exit.ts:72-73, words/thread.ts:393-396). Kind usage for all but `branch_of_record_unread_refusal`, `sub_from_unread_refusal` and `sub_twice_refusal`, which carry none (provider) (:204-205, :238, :266, :325-335). Ids are cut to 8 (:216, :219, :295, :510); the cloud is lowercased in `wsp init --on` and `wsp add` fixes (:268, :273, :506). `sub_no_cloud_refusal`: the held clouds joined ", ", "none" for none (:222). `sub_ahead_refusal`: "commit" at exactly 1 (:254). `sub_from_not_on_remote_refusal`: a fetch-or-push fix without a name, `git push -u origin <name>` with one (:260-264). `fork_not_pushable_refusal` opens with START_WORDS.forkNotPushable (start.ts:303). The four refusals of the lead's branch (:247-256) end with the `--from <pushed>` road (:230).
7. `sub_size_refusal`: each sold size as "name size ($rate/h)", the rate `to_fixed(3)` (:76); none is "nothing", one stands alone, more read "a, b and c" (:284-288).
8. `sub_replaced_line`: one sentence per fault kind; the request id is never read (:311-315). `sub_twice_refusal`: two unusable name both boxes and the second's request id, else the first's; two env and two cancelled have their own sentences; a mixed pair says each as "box: why" (env: "setup not taken, exit 124") (:325-335). `sub_oom_line`: the size, then "Start the work again with size <bigger>." or "<on> sells no bigger size, ..." (:319-322). `is_sub_oom_line`: `^the agent on .+ was killed for memory: ` without regard to ASCII case (:441).
9. Start lines (:54, :57, :194, :196, :338-354): `cloud_added_line` and `cloud_removed_line` lowercase the name in "run with on <name>"; `sub_uncommitted_line` says "change" at exactly 1 and the sha cut to 7; `sub_picked_elsewhere_why` is `seedNotHereRefusal`'s sentence before its fix with one trailing "." off (project-seed.ts:145-146); `sub_not_sent_line` adds the `wsp projects seed` sentence only where the why starts "nothing is picked"; `sub_stayed_line` joins the paths ", "; `sub_copying_line` says "file" at exactly 1; `sub_opened_line` lowercases the cloud and cuts the id to 8.
10. Work not pushed (:357-372, :476-522): the held words are "<n> commit(s)" then "<n> changed file(s)", each only above 0, joined " and ", "work" where both are 0; "is" where files plus commits is exactly 1, else "are". `sub_unpushed_line` and `sub_held_line` read unread first, then a read failure (an empty one included), then the counts. `sub_delete_question`: the fixed second line without held, else the unpushed line, its first UTF-16 unit upper-cased. `sub_not_pushed_reason`: the disk line first where kept. `sub_held_on_remove_line`: None for none; "its machine" for one, "their machines" else; the first five joined "; ", then "; and <n> more".
11. `sub_strip::*` (:383-400): `not_pushed` titles the held words with the first UTF-16 unit upper-cased plus " not pushed", its note names the refusal where given and the disk line where kept; `oom` capitalises the line the same way. `sub_spawn_words::*` (:445-462): past tense when done; `agent_fact` drops an empty or absent model and effort and is None where both drop; `machine_fact` is None without an id, else the id and the states joined ", "; `branch_fact` cuts the sha to 7.
12. `is_git_push` (:404-415): the input's `command` where the input parses as a JSON object whose `command` is a string, else the input itself, tested against `\bgit\b(?:\s+-\S+(?:\s+\S+)?)*\s+push\b`. `push_refused_of` (:418-432): lines split on "\n" and trimmed at the end; the first `remote:` line that is not progress, else the first `fatal:`, else the first `\s*! [`, else the first `error:`; None without one; the prefix off, whitespace runs one space, trimmed, one trailing "." off; None where that leaves "", else cut to 200. The two prompts are fixed sentences (:435, :438).
13. End lines (:503-531): `sub_cloud_gone_refusal` lowercases the cloud in its fix; `sub_released_refusal` cuts the id to 8; `sub_delete_refused_line` says "<n> minutes" at any n; `cloud_said`: the provider id the word names (its id or its key name, any case) as its key name, else the word as given. `sub_result_line` (:535-556), in order: a read failure; work not pushed (unpushed or changed above 0); no pushed sha or 0 commits is "nothing to merge"; else the branch, the sha cut to 7, the commits, and `wsp merge in <lead 8> <id 8>`. Minutes are `js_round(deleteInMs / 60000)`, 0 without one; above 0 the machine "naps now and is deleted in <n> minute(s)", else "is deleted now". The kept day is `toISOString` cut to 10, then " UTC".

## Edge cases
- `sub_size_of("16x32")` is `{cpu:16, memMb:32768}`; `"4 vCPU 8 GB"`, `"nope"` and `""` are None (pinned). `sub_size_word` of 1536 MB is "4x1.5", of 512 MB "1x0.5" (pinned).
- `sub_next_size(4096, ..)` skips the 4096 MB size: strictly more. `sub_sizes_sold(.., Some(&[]))` is empty (pinned).
- `sub_folder_of("/root/repo")` is "/root/repo", `("web")` "/root/web", `("https://gitlab.com/a/b/c")` "/root/c" (pinned). `sub_branch_of("")` is "wsp/sub-thread" (pinned).
- `sub_branch_name_ok("wsp/")`, `("x.lock")`, `("-x")`, `("a b")`, `("a..b")` are false (pinned).
- `sub_size_refusal(.., &[])` reads "sells nothing on this plan"; `sub_no_cloud_refusal(.., &[])` "It holds: none."; `sub_no_agent_refusal(.., &[])` ends "name one it carries: ." (pinned).
- `sub_unpushed_line` of 0 and 0 is "work on wsp/x are not pushed and go with it."; `sub_stayed_line(&[])` opens with a space; `sub_delete_refused_line(.., 1)` says "1 minutes" (pinned).
- `push_refused_of("")` and `("Everything up-to-date")` are None; progress-only `remote:` lines fall through to the `fatal:` line (pinned).
- `is_git_push("git  push")` is true, `("gitpush")` false, and a JSON input's description is never read (pinned).
- `agent_fact(None, None)` is None (pinned); `cloud_said("Boat")` is "Boat" (pinned).

## Behaviour nobody wrote down
- Rules 8.1, 8.4 and 8.5: ids cut to 8, shas to 7, the push refusal to 200 and the branch slug to 48 count UTF-16 units; a cut never leaves half a surrogate pair.
- Rule 8.6: `sub_size_of`'s trim, `push_refused_of`'s trimEnd and `\s`, `is_git_push`'s `\s` and the refusal join's trimEnd are JavaScript's whitespace.
- Rule 8.7: the `is_git_push`, `push_refused_of` and `is_sub_oom_line` patterns are ported by hand; `\b` is an ASCII word edge, `.` stops at "\n", "\r", U+2028 and U+2029, and the `i` flag folds ASCII letters alone.
- Rule 8.9: the cloud word and `sub_branch_of` lowercase with the full mapping; the capitalised first letter upper-cases one UTF-16 unit with the full mapping ("ß" becomes "SS").
- Rule 8.11: every count, cpu and minute in a sentence is `js::number_string`.
- Rules 8.12 and 8.13: `sub_size_of` and `sub_result_line`'s minutes use `js_round`; the rate is `to_fixed(3)`, memGb `to_fixed(1)` read back as a number.
- Rule 8.15: the kept day is the first 10 characters of `toISOString`.
- `is_git_push` reads a JSON input's `command` only where it is a string; a JSON array, a number, a non-string command or text that is not JSON is tested whole.

## Tapes
The first message and the resume head of a sub-thread (runtime sub-spawn); the finished line to the lead, the delete question and the not pushed reads (runtime sub-life); the line ahead of a thread's next message after a cloud is added or removed; `wsp run --on` refusals and its first terminal line; `wsp remove` of a cloud; the strip over a sub-thread's composer, the spawn line and its steps, and the tile card in the app.

## Decisions
1. SUB_STRIP and SUB_SPAWN_WORDS are modules `sub_strip` and `sub_spawn_words`: fixed words are constants, the rest functions.
2. The TS structural size types become the `SizeFacts` trait over `SubSize` and `CpuMem`; numbers are `f64`. `sub_next_size` takes the memory alone and `sub_sizes_sold` the plan's list (`None` for no plan). `sub_size_words`, `sub_oom_line` and `SubResumePreamble` take a whole `SubSize` where the TS picks name, cpu and memMb; the rate is not read.
3. The TS object parameters become structs: `SubPreamble`, `SubResumePreamble`, `BaseAt`, `GitHostWords`, `Held`, `KeptOn`, `KeptUntil` (`until` in ms), `SubDeleteQuestion`, `SubResultFacts`. A TS `true` or absent (`brokered`, `unread`) is `Option<bool>`, and only `Some(true)` counts.
4. `SubFault` is an enum for the TS union; refusals answer `HostError`; string lists are `&[impl AsRef<str>]` or `&[&str]`; `ref` is `r#ref`.

## After the port
- Zero counts read "work", and the unpushed and held-on lines then say "work ... are not pushed" (:359, :363, :479).
- `sub_delete_refused_line` at 1 and the preamble's delete at 1 minute say "1 minutes" (:162, :514).
- `sub_no_agent_refusal` with nothing carried ends "name one it carries: ."; `sub_stayed_line` with no paths opens with a space (:303, :350).
- `push_refused_of`: a `remote:` line that is only a "." (not progress, empty once cut) answers None even where a `fatal:` line follows (:427-431).
- The git lines write the branch and the repo unquoted, the brokered refspec aside, and `sub_branch_name_ok` lets `;`, `$`, a backquote, `&` and `|` through (:109, :119-120).

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `pinned::sizes_are_read_named_and_stepped_within_what_a_plan_sells` |
| 3, 4 | `pinned::folders_and_branches_are_named_off_the_remote_and_the_machine` |
| 5 | `pinned::the_lines_of_a_start_a_fault_and_a_copy` (`sub_opening` alone); no test pins `sub_preamble` or `sub_resume_preamble` |
| 6, 7 | `pinned::every_refusal_says_what_happened_what_to_do_and_its_kind` |
| 8 | `pinned::the_lines_of_a_start_a_fault_and_a_copy`, `pinned::every_refusal_says_what_happened_what_to_do_and_its_kind`, `pinned::a_push_and_an_oom_are_read_off_their_own_lines` (the false side of `is_sub_oom_line` alone) |
| 9 | `pinned::the_lines_of_a_start_a_fault_and_a_copy` |
| 10 | `pinned::work_not_pushed_holds_the_machine`, `pinned::every_refusal_says_what_happened_what_to_do_and_its_kind` |
| 11 | `pinned::strip_and_spawn_words` (`waiting_slot`, `not_pushed`, `oom`, `start_again`, `files`, `agent_fact`, `machine_fact`, `branch_fact`; the rest no test pins) |
| 12 | `push_refused_of::*`, `pinned::a_push_and_an_oom_are_read_off_their_own_lines` |
| 13 | `pinned::the_end_of_a_machine`, `pinned::every_refusal_says_what_happened_what_to_do_and_its_kind`; no test pins `sub_result_line` |
