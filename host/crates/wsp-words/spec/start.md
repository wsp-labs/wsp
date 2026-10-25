# start
Source: the functions of `packages/protocol/src/start.ts` (312 lines) at a3b870a6e; its zod shapes (`GithubLink`, `WorkspaceFrom`, `IssueRead`, `GitPrDiffReply`, `ReviewBlock`, `ReviewDraft`, ...) and its flat constant `REVIEW_DIFF_MAX_BYTES` are `wsp_wire::types::start`. Tests: `packages/protocol/test/start.test.ts` (18 of 19 cases ported in `tests/start.rs`; :216-223 skipped, since it parses `WorkspaceOut` and so tests wsp_wire). No pinned module.

## What it does
The words of a thread started on a GitHub issue or pull request and of a pull request reviewed by an agent: the link read out of what the person pasted, the `owner/repo` a remote names, the task each start opens its thread with, the review read off the reviewer's reply, the name a workspace takes, and the buttons, refusals and lines every road says them in.

## Behaviour
1. `github_link_of(text)`: trim; where the trimmed text opens with the picker's block (`Issue #N "..." (<link>):` or `Pull request #N ...`, then a newline), read the link it names, else the whole trimmed text; the link must be the whole line: http or https, an optional `www.`, `github.com` in lower case, an owner of letters, digits and inner dashes, a name of letters, digits, `.`, `_`, `-`, then `issues` or `pull` and digits, then nothing or `/`, `?` or `#` and non-space (start.ts:21-35). The number is `Number(digits)`; the url is rebuilt as `https://github.com/<owner>/<name>/<path>/<digits>`.
2. `owner_repo_of(remote)`: off, in order, a lower-case `scheme://`, a `user@`, every trailing `/`, one `.git`; then split on `/` and `:`, drop the first part (the host) and join the rest with `/`; "" is None (:39-42). `github_address(remote)`: None unless `github.com` follows the start, an `@` or a `/` and is followed by `:` or `/` (case counts); else `https://github.com/<owner_repo>.git`, None where `owner_repo_of` is None (:45-49).
3. `holds_repo(remote, repo)`: `owner_repo_of(remote)` lower-cased equals `repo` lower-cased; a remote naming no path holds nothing (:52). `project_for_repo(projects, repo, here)`: of the projects that hold the repo, in list order, the first on computer `here`, else the first (:56-59).
4. `from_task_prompt(from, read)`: the noun is "issue" for kind issue, "pull request" for pull_request and review. Lines: `Work on this <noun>: #<read.number> <one_line(title)>`, the url; the body trimmed and cut at 16 384 UTF-8 bytes, after a blank line, with "\n(the description is cut here)" where cut, and nothing where empty; the first 20 comments under a blank line and "Comments:", each `<one_line(author)>: <one_line(body) cut at 4 096 bytes>` with " (cut)" where cut, then `(<more> more comments on the <noun> are not shown)` where more than 20; a blank line; for an issue `... says Closes #<from.number>.`, else the branch sentence naming `head.branch` where a head is set (:122-144). Joined with "\n".
5. The byte cut (`headWithin`, :113-117): text within the bytes is kept whole and not cut; else the first `max` UTF-8 bytes decoded, any trailing run of U+FFFD dropped, and cut is true.
6. `with_closes(body, issue)`: the body unchanged where it already says, case aside, `close`, `closes`, `closed`, `fix`, `fixes`, `fixed`, `resolve`, `resolves` or `resolved`, one space and `#<issue>`, with a word boundary before the verb and after the number; else the body trimmed at its end, "" becoming `Closes #<issue>` and anything else `<body>\n\nCloses #<issue>` (:146-154).
7. `line_in_diff(diff, path, line, side)`: walk the rows split on "\n"; a `diff --git ` row makes the file the walk's own where it ends with ` b/<path>` or is exactly `diff --git a/<path> b/<path>`, and marks the path as held; in the walk's own file a row starting `@@ ` that reads as a hunk head gives start and length of the side (length 1 where omitted); true at the first hunk with length above 0 where `start <= line < start + length`. Past the end: Some(false) where the path was held, None where never (:158-174).
8. `start_name(kind, number, title)`: `Review ` for kind "review" (anything else reads as a start), then `#<number> <one_line(title)>`, cut to 40 UTF-16 units, trimmed at its end (:177-180). `name_of_task(task)`: the trimmed task's first line split on whitespace, empties dropped, the first 5 words joined by one space, cut to 40 UTF-16 units, trimmed at its end (:183-186). `taken_name_after(name, taken)`: the name where not taken, else `<name> <n>` for the first n from 2 not taken, with no cap (:189-192).
9. `review_from_reply(text)`: of every fenced block opening "```json" (anything up to the line's end may follow, so "```jsonc" counts), then a newline, then the shortest text up to "\n```", the last one's text is read; none is Err "the reply has no fenced json block"; text that is not JSON is Err `the json block does not parse: <JSON.parse's message>`; a value ReviewBlock refuses is Err `the json block's <path joined by "."> is wrong: <zod's message>`, path "" reading as "shape", for the first issue in key order (verdict, summary, comments; within a comment path, line, side, body) (:203-223). The verdict, where a string, is lower-cased and each whitespace character or dash becomes `_` before it is read (:196); unknown keys are dropped.
10. `review_task_prompt(from, read, diff)`: `Review pull request #<n>: <one_line(title)>`, the url, `It merges <head.branch, else "its head"> into <base, else "its base">.`; the body trimmed and cut at 16 384 bytes under a blank line and "Its description, as its author wrote it:" where not empty; the fixed rules lines; where the diff is truncated the "cut at 96 KB" line naming `left` joined by ", "; the "Below is the diff" line, `<fence>diff`, the diff, the fence, where the fence is `fence_for(diff)` (backticks one longer than the diff's longest run, at least 3); a blank line and the reply-shape sentence (:227-249).
11. `is_review_read(draft)`: false for None, true for a draft carrying a verdict (:281-283). `review_reask_prompt(why)` (:286-288).
12. `start_words` (:291-312): fixed words are constants; `verdicts` maps comment, approve, request_changes to "Comment", "Approve", "Request changes"; `from_issue` one-lines the title; `no_read_only` joins the others with " or "; `not_a_link` one-lines the text and cuts it to 80 UTF-16 units; `made` is `made <workspace>` without a from, else `... from issue #N` for kind issue and `... from pull request #N` for the other two; `posted` says "comment" for exactly 1, "comments" otherwise, and the folded count bare.

## Edge cases
- `github_link_of(".../a/b/pull/007")` has number 7 and url `https://github.com/a/b/pull/007`. `GitHub.com`, `/pulls/7`, an owner ending in `-`, and a link with a word after it are None.
- A picker block with nothing after its `):` is None: the block needs its newline, and the whole text is then no link.
- `owner_repo_of`: `ssh://git@host:2222/a/b.git` is "2222/a/b"; `/srv/repos/lab` is "srv/repos/lab"; `https://gitlab.com/a/b/c.git` is "a/b/c"; "" and `https://github.com/` are None.
- `github_address("https://notgithub.com/a/b")` is None.
- `with_closes`: "FIXED #5" and "closes #5." stay; "closes#5" and "closes #5a" take the line; "Closes #50" with issue 5 takes it (test); "   " is "Closes #5".
- `start_name("start", 5, "x" * 36 + "😀z")`: the cut lands inside the emoji; the TS keeps its high surrogate, the Rust drops it (8.5).
- `name_of_task("  one two\tthree four five six\nsecond")` is "one two three four five"; whitespace alone is "".
- `line_in_diff`: a `-0,0` side holds no line (length 0, Some(false)); path "x" in a diff of "sub/x" is None; a file header with no hunk is Some(false).
- `review_from_reply`: "Request-Changes" and "Request Changes" read as request_changes, "request  changes" is refused (two underscores); "```JSON" and a block with no newline before its closing fence are no block; an empty block is "does not parse: Unexpected end of JSON input"; an array is "the json block's shape is wrong: Expected object, received array"; side BOTH is "comments.0.side is wrong: Invalid enum value. Expected 'LEFT' | 'RIGHT', received 'BOTH'"; line 0 is "Number must be greater than 0", line 1.5 "Expected integer, received float", a missing summary "summary is wrong: Required".
- `from_task_prompt`: a body of exactly 16 384 bytes is not cut; 10 000 "é" (20 000 bytes) keep 8 192; 21 comments end "(1 more comments on the pull request are not shown)"; an empty comment reads "a: ".
- `review_task_prompt` with no head and no base says "It merges its head into its base."; truncated with `left` empty ends "read them in the copy: .".
- `taken_name_after("n", {"n", "n 3"})` is "n 2". `posted(.., 1, 0)` is "(1 comment, 0 in the summary)"; `no_read_only("a", [])` ends "review with ".

## Behaviour nobody wrote down
- Rules 8.1 and 8.4: `start_name` and `name_of_task` cut at 40, `not_a_link` at 80, UTF-16 units; the body and comment cuts count UTF-8 bytes, not UTF-16 units.
- Rule 8.5: the `start_name`, `name_of_task` and `not_a_link` cuts never leave half a surrogate pair.
- Rule 8.6: `trim`, `trimEnd`, `one_line`'s `\s+`, `name_of_task`'s split and the verdict's `[\s-]` are JavaScript's whitespace.
- Rule 8.7: LINK, PICKED, the CLOSES test (its `i` flag folds ASCII alone, and `\b` is ASCII word characters), JSON_BLOCK, the `owner_repo_of` chain and HUNK_HEAD are ported by hand; LINK's and PICKED's `\S` run per UTF-16 unit.
- Rule 8.9: `holds_repo` and the verdict lower-case with full mapping.
- Rule 8.11: every number in a sentence (`with_closes`, `start_name`, the `start_words` sentences) is `js::number_string`.
- The Err text of rule 9 carries V8's `JSON.parse` message and zod's issue message word for word; the Rust writes each by hand.

## Tapes
`wsp run` with a pasted link (the `made` line, the start's opening task), `wsp review` (the reviewer's task and its re-ask) and `wsp review post` (the `posted` line); the refusals of a link no project holds or a fork that cannot be pushed; ProjectHome's Start on and Review buttons, the review draft section and Post review.

## Decisions
1. `holds_repo` takes the project's remote string, not `{ remote }`; `project_for_repo` reads projects through the `RepoProject` trait (remote, computer) and answers `Option<&P>`.
2. `review_from_reply` answers `Result<ReviewBlock, String>`: the TS `{ ok: true, review }` is Ok, `{ ok: false, why }` is Err(why). The verdict's lower-casing lives in wsp_wire's `ReviewBlock` Deserialize (its shapes spec, rule 7).
3. `is_review_read` answers a bool for the TS type guard; `ReviewVerdict` is `ReviewBlockVerdict`.
4. The string unions stay strings: `line_in_diff`'s side is "LEFT" or "RIGHT", `start_name`'s kind "start" or "review". A JS number parameter is `f64` (`with_closes`, `line_in_diff`, `start_name`, the `start_words` sentences); `taken_name_after` takes an `IndexSet<String>` for the ReadonlySet.
5. START_WORDS is the module `start_words`: fixed words are constants, sentences and `verdicts` functions; `no_read_only` takes `&[&str]`.

## After the port
- One left-out comment reads "(1 more comments on the issue are not shown)": the plural is fixed.
- `github_link_of` keeps leading zeros in the url (`pull/007`) though its number is 7, so the link is not written the one way.
- `owner_repo_of` reads an ssh port as a path part: `ssh://git@host:2222/a/b.git` names "2222/a/b".
- `review_task_prompt` cuts the description at 16 KB without saying so, where `from_task_prompt` adds "(the description is cut here)".
- The byte cut drops a U+FFFD the text itself held where it sits at the cut's end, not only the one the cut made.
- A truncated diff with an empty `left` says "read them in the copy: ."

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `github_link_of::*` |
| 2 | `owner_repo_of::*`; `github_address` is pinned by no test |
| 3 | `the_project_a_link_s_repository_names::*` |
| 4, 5 | `from_task_prompt::*` |
| 6 | `with_closes::*` |
| 7 | `line_in_diff::*` |
| 8 | `the_names_a_workspace_takes::*`; `name_of_task` is pinned by no test |
| 9 | `review_from_reply::*` |
| 10 | `review_task_prompt::*` |
| 11 | none: no test pins `is_review_read` or `review_reask_prompt` |
| 12 | `the_words::names_the_buttons_and_the_refusals_as_ruled` (start_on, review, no_project_for_repo, fork_not_pushable, no_read_only); the other sentences are pinned by no test |
