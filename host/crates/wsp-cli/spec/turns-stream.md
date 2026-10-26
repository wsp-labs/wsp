# turns_help/stream: why a turn failed, what a watched turn prints, the answers to its prompts

Port of `packages/host/src/verbs/turns-help.ts:832-1173` at 65f6b89c7 (`turnFailure`, `turnRefusal`, the wait and join
lines, `turnStream`, `ANSWER_ROADS` and the answer lines, `answerOpenAsk`, `answering`, `followVerb`, `beforeSending`,
`detachVerb`, `turnView`). Rust: `src/verbs/turns_help/stream.rs`. Tests: `tests/cli_c/turns_stream.rs` (261 recorded cases
from `turns_stream.json`), `turns_stream_rules.rs` (28 named tests), `drivers_turns_stream.rs`. Recorded by running the TS
(`record/turns_stream.probe.ts`, `TZ=UTC`); every `followVerb` and `detachVerb` case is run as text and as `--json`. The TS
tests that name these helpers: `send-waits.test.ts` (6, restated as the four "send-waits:" cases plus the queued cases, on
the detached and followed roads), `verbs-turns.test.ts` and `verbs.test.ts` for the answer lines (`answerKeysLine`,
`answerVerbsLine`, `answeredLine`, `noOpenAskLine`, `noSuchAnswerLine`, `ANSWER_IN_THE_APP`: restated as pure cases; their
fake-runtime bodies are reached through the verbs' tapes).

## Behaviour

- `turnFailure` (:832): nothing when `result.status == completed` (whatever `error` says). Else `result.error` where
  present (an empty string counts), else `reason` (an empty string counts), else `turn <status>` where there is a result
  (`turn failed`, `turn interrupted`), else `turn ended without a result`.
- `turnRefusal` (:840-852): the failure as a thrown error. The class is the stamp the adapter put on the result
  (`refusal`): `sign-in` makes `auth_refusal(failure)` (kind auth, exit 2, no fix); anything else, or none, is a plain error
  of the failure's words (provider class, exit 1). The agent's own words never decide the class. A completed turn: nothing.
- Wait lines (:855-868). `WAITING` = `waiting behind the running turn`; a `session.queued` naming a computer
  (`waitsFor`) says `send_waits_for_line(name)` instead. Joined lines: `steered` says `joined the running turn` and, for
  each pick the line named, `; --model, --effort, --access, --fast dropped, it keeps its own model, effort and access` (flag
  order model, effort, access, then `fast` only where it is `true`; none named adds nothing); `queued` says
  `send_waited_for_line(computer)` where the **last** wait was on a computer, else `queued behind the running turn; it has
  ended and this turn started`; `started` and `held` say nothing. With a steer whose `waiting` is true, one more line:
  `the turn is waiting on a permission; wsp threads shows it as Needs you` (the thread table's own word).
- `turnStream` (:878-920), written through `out.stream` only (so nothing under `--json`): prose is written as it arrives,
  nothing for an empty piece; prose of another `messageId` after prose opens a paragraph (`\n` where a line start already
  stands, `\n\n` otherwise); a line of work between two messages already parts them, and a piece with no message id never
  parts (`said` keeps the id it last saw); `says(l)` closes a half line first (`\n` unless at a line start) and writes
  `l\n`; `line(l)` is `says` of `io.muted(l)` where the io has one; `reply(text)` closes a half line and answers the text for
  stdout **except** `None` where prose was streamed and `io.same_screen` is true.
- Answer lines (:921-980). `ANSWER_ROADS` is three rows in the order y (allow, "run it", verb `allow`, said `allowed`), n (deny,
  "refuse it", verb `deny`, said `denied`, reasons), a (mode, no words, no verb). A prompt's roads are those rows whose effect
  one of its options has, each beside the **first** option of that effect, in the rows' order, never the options' order.
  `answer_keys_line`: `answer here: type ` then `<key> to <does>` (a road with no words takes the option's label with its first
  UTF-16 unit lowered: `Édit` -> `édit`, an astral capital and a digit stay, `İ` -> `i̇`), joined by `, `; with no road the line
  ends `type `. `answer_verbs_line`: `answer from another terminal: wsp thread allow <thread_word>, or wsp thread deny
  <thread_word>`. `ANSWER_IN_THE_APP` = `answer it in the app: this prompt carries its own answers`. `answered_line` =
  `<said> on thread <thread_word>: <asking_line(ask)>`. `noOpenAskLine`, `noSuchAnswerLine` as the doc comments.
- `answer_open_ask` (:989): `thread_of`, `sessions.history` (whole), `open_ask` (a close removes a prompt; a prompt of another
  thread is not this thread's; with two open the first opened), then the first option of the road's effect, then
  `sessions.answer {sessionId, askId, optionId[, reason]}`: `reason` is sent exactly where one was given (an empty string is
  one). Order of refusals: no thread -> `no thread <ref>` (not-found) before any read; no open prompt -> `no_open_ask_line`
  (nothing sent); no option of that effect -> `no_such_answer_line` (nothing sent); then the host's outcome: `answered` is
  the answer, `gone`/`unsupported`/`not-found`/`no-option` fail with `ANSWER_WORDS[outcome]` (plain error), a refused op fails
  with the host's own error and kind.
- `answering` (:1004-1042): on a permission: `needs_you_line(ask)`, then **one** of: `ANSWER_IN_THE_APP` (no road),
  `answer_verbs_line` (no key offered: `--json`, or the io has no `answer_key`), `answer_keys_line` (keys offered). Keys are
  offered when the line is not `--json` and the io offers `answer_key`, and there is at least one road: the previous read is
  released, then `answer_key(roads' keys, until)` is read; a key that is a road's `answer_ask`s that road's option on
  `ctx.client()` (the socket as it stands then); a key that is no road of this prompt sends nothing; a failure goes to
  `io.error` as its message. A prompt closed (`session.permission.closed`) releases the read of that ask only; `stop()` (every
  road out of `followVerb`) releases whatever is open. All of this is said through the stream (stderr's copy of the screen).
- `followVerb` (:1059-1140): see the doc comment. Each event in order; the `done` event emits itself with the reply for
  stdout (`reply()` of the stream, or the text whole once the host was redialled, where the stream has a gap); every other
  event is emitted bare (text prints nothing, `--json` prints the frame). The failure comes **after** the reply is printed.
  Under `--json` the wait and starting lines are held back, the join lines are not.
- `beforeSending` (:1141): the read's answer; a failure on a socket the host stopped (4001) is `NOT_DELIVERED_LINE`, any other
  failure as it is. `detachVerb` (:1153): as the doc comment; the emitted object is `turn_view` + `sub` + `notice: said`, in that
  key order. `turnView` (:1165): `text` is present only where the turn has a result (`""` for a result with no text); `capped`
  only for a `held` turn that has one on its session; `afterCut: true` only where set; `outcome` is the turn's.

## Limits and caps

None numeric. UTF-16 rule 8.4 applies to `lowerFirst` (one UTF-16 unit, not one `char`). The stream is unbuffered (each call writes at once).

## Null and empty inputs

No options, an unknown effect-less prompt (`answer` effect only), an empty label, an empty text piece, an empty `reason`, a
`done` with no text, no spend word, no `opened`, no `sub`, `fast: false`, an empty thread id in `answerVerbsLine`
(`thread_word("")`), a call with no `toolUseId`, a result with no call open, no `onTurn`.

## Order

stderr/stream lines in event order; `onTurn` before the first event; notice lines then the join line(s), then (steer waiting)
the permission line; the reply (stdout) before the failure is thrown; `sessions.answer` is the only op after the start.

## Invariants held

The exit contract's class rule (`packages/protocol/src/exit.ts`): the class is read off the kind stamped where the error was born
(`auth_refusal`, `usage_refusal`...), never off its words: `the_class_is_read_off_the_stamp_never_off_the_words`,
`a_sign_in_refusal_is_the_auth_class_with_the_agents_words`. `--json` carries JSON alone on stdout and holds the wait lines
back: `json_writes_no_stream`, `the_wait_lines_are_held_back_under_json_but_the_join_lines_are_not`,
`a_failure_object_stays_the_one_line_on_stderr_under_json`. Host invariants 1-8 do not reach this file.

## What the TS leaves unclear

- A key typed after the turn ended is a read already released; a key typed for a prompt that the host has meanwhile closed
  is the host's refusal, said on stderr (`gone`).
- `answering` offers keys by presence of `io.answerKey` only; there is no tty check here (the io decides).
- `followVerb`'s `redialed` flag is set only by the follow's own redial; a prompt answered on the new socket after a redial is
  untested (no recorded case: the key and the redial together need two pushed phases).

## Rule -> test

| Rule | Recorded cases (helper / case) | Named test (`turns_stream_rules.rs`) |
|---|---|---|
| failure text order; empty strings count | turnFailure 14, turnRefusal 14 | `an_empty_error_is_still_the_failure_and_an_empty_reason_too`, `a_completed_turn_has_no_failure_whatever_it_says` |
| class off the stamp | turnRefusal "refused for a sign-in" (3), followVerb "refused for a sign-in", "a failed turn" | `a_sign_in_refusal_is_the_auth_class_with_the_agents_words`, `the_class_is_read_off_the_stamp_never_off_the_words` |
| reply before failure | followVerb "a failed turn", "an interrupted turn", "a turn the runtime ended" | `a_turn_that_failed_still_prints_its_reply_before_it_fails` |
| keys: roads in the rows' order, label lowered by one UTF-16 unit | answerKeysLine 14 | (whole-case compare) |
| key picks by effect, never by id; no reason | followVerb "keys pick by effect, y/n/a" | `a_key_answers_with_the_option_its_effect_names_never_by_the_options_id` |
| no key, no road, wrong key: nothing sent | followVerb "...a key no road has", "...option the prompt lacks", "...only its own answers", "...no options at a keyboard" | `a_key_no_road_has_sends_nothing`, `a_key_for_a_road_the_prompt_has_no_option_for_sends_nothing`, `a_prompt_with_its_own_answers_offers_no_key_and_points_to_the_app` |
| `--json` offers no keys, writes no stream | followVerb every "(--json)" variant | `json_offers_no_keys_and_sends_no_answer`, `json_writes_no_stream` |
| a second prompt releases the first read; a close releases that ask | followVerb "a second prompt replaces...", "a prompt closed by anyone...", "two prompts in a row" | `a_second_prompt_releases_the_read_of_the_first` |
| a failed answer goes to stderr, turn goes on | followVerb "a key that the host refuses/fails on..." | `a_failed_answer_goes_to_stderr_and_the_turn_goes_on` |
| wait lines held under `--json`, join lines not | followVerb "queued...", "the agent is starting", detachVerb "queued...", "held..." | `the_wait_lines_are_held_back_under_json_but_the_join_lines_are_not`, `a_failure_object_stays_the_one_line_on_stderr_under_json`, `a_held_turn_with_its_cap_says_the_wait_and_any_other_join_its_own_line` |
| dropped picks, flag order, `fast: false` | followVerb "joined with ...", detachVerb "steered with picks dropped" | `only_the_picks_a_steer_drops_are_named_in_flag_order` |
| paragraphs, half lines, calls forgotten | followVerb "a second message...", "a line of work...", "prose with no trailing newline", "a result forgets its call", ... (20) | `another_message_opens_its_own_paragraph_only_where_prose_would_run_into_prose`, `a_cut_stream_line_is_closed_before_a_line_of_work_and_before_the_reply`, `a_call_is_forgotten_once_its_result_is_read` |
| same screen / redial print the reply | followVerb "prose on the person's own screen", "no prose streamed...", "the host stops under the turn: the reply is printed whole" | `a_redialed_follow_prints_the_reply_whole_even_on_the_persons_own_screen`, `a_reply_on_the_same_screen_is_printed_only_where_nothing_was_streamed` |
| announce | followVerb "announce ..." (3) | `the_announce_is_emitted_before_the_reply_with_the_threads_own_line` |
| detach: key order sub then notice | detachVerb 36 cases | `a_detached_start_prints_the_thread_object_with_sub_then_notice_after_the_turn_views_keys` |
| beforeSending | beforeSending 4 | `a_read_that_failed_on_a_socket_the_host_stopped_is_a_message_not_delivered` |
| answer by the verb: order of refusals, reason, outcomes | answerOpenAsk 22 | `answering_by_the_verb_refuses_where_the_thread_waits_on_no_prompt_or_the_prompt_lacks_the_answer`, `an_answer_carries_the_persons_words_only_where_they_were_given`, `every_answer_that_is_not_answered_is_a_failure_in_the_hosts_words` |
| turn view | turnView 10 | (whole-case compare) |
