# runs_secrets
Source: slate-runs.ts:499-611 (`secretForms`, `DefaultApprovals`, `createSecrets`), 197-208 (`SlateSecrets`) at
65f6b89c7. Tests: slate-runs.test.ts `secrets` (5 cases); oracle `oracle/helpers.out` (`secretForms.*`, `scrub.*`).

## What it does
Holds the plaintext of every secret a person typed into a slate, per thread and name, and nothing else ever sees
it: a handle answers set, length and time; `scrub` replaces every form a command could print it in. Secrets marked
`keep` are written to `slates.secrets.json` beside the state file and read back by the next host; the rest die with
the process. `DefaultApprovals` is the memory store of "Always" keys when the host gives none.

## Behaviour
1. `create_secrets(file, now)` (:545-556): with a file that exists, reads it as `{thread: {name: {text, at}}}` and
   holds each as kept; a file that does not read or parse holds nothing and is not rewritten.
2. `handle(thread, name)` (:565-568): unset is `{"secret":true,"set":false,"len":0,"at":null}`; set is
   `{"secret":true,"set":true,"len":<UTF-16 units of the text>,"at":<when set>}`, keys in that order.
3. `set(thread, name, text, keep)` (:578-590): an empty text is `forget(thread, name)` and answers the unset handle.
   Otherwise holds `{text, at: now, keep}` (an existing name keeps its place), and saves when this one is kept or the
   one it replaced was. A kept secret set again without `keep` leaves the file.
4. `clear` is `forget(thread, name)` then the handle (:591-594).
5. `forget(thread, name?)` (:569-576): nothing held for the thread does nothing. With a name drops that one, without
   drops the thread; saves when what went held a kept one.
6. `save` (:557-564): with no file does nothing. Otherwise writes, through `write_own` (folder 0700, file 0600),
   compact JSON with no newline: each thread with a kept secret, in the order threads were first held, and under it
   each kept name in held order as `{"text":..,"at":..}`. With none kept it writes `{}`.
7. `scrub(thread, text)` (:597-606): nothing held for the thread, or an empty text, answers the text. Otherwise
   every form of every held secret of this thread (names in held order, forms in `secret_forms` order) is paired
   with `[secret:<name>]`, sorted longest form first (a stable sort: ties keep that order), and each form in turn,
   once, replaces all its occurrences in the text as it stands. Other threads' secrets are not touched.
8. `secret_forms(text)` (:501-525), in this order, duplicates and the empty text dropped: the text; its UTF-8 bytes
   in base64 standard padded, unpadded, URL-safe padded, URL-safe unpadded; for shift 0, 1, 2 the base64 of `shift`
   zero bytes then the text's bytes, sliced from 0 (shift 0) or 4 to `floor((shift + n) / 3) * 4`, kept only when 8
   or more characters, then its URL-safe form; `encodeURIComponent(text)`, the same with each `%XX` lowercased, and
   with each `%20` as `+`.
9. `plaintext(thread, name)` answers the text or none (:607-609).
10. `DefaultApprovals` (:527-543): per thread a set of keys in insertion order; `has`, `allow`, `revoke`, `list`.

## Edge cases
- `set(t, "pin", "4321")` scrubs `pin 4321 elsewhere-secret` to `pin [secret:pin] elsewhere-secret` when
  `elsewhere-secret` is t2's; after `set(t, "pin", "")` the handle is unset and `4321` is no longer scrubbed.
- Forms of `vrcl_Ab+9/zz&q=1 ünï_0123456789` are the seven the oracle lists; a Basic header's base64 keeps only the
  bytes that share a group with `user:` (`dXNlcjp2[secret:token]ODk=`).
- A later, shorter form is matched inside a mark already written: secrets `s = "secret"` and `t = "cret"` scrub
  `a secret` to `a [se[secret:t]:s]` (oracle `scrub.nested`). Ported as is.
- A text with a space also scrubs `a+b` (oracle `scrub.plus`).
- A text holding a lone surrogate makes `encodeURIComponent` throw `URIError: URI malformed`, so every scrub of that
  thread fails (oracle `forms.lone.throws`).
- Kept file: after `set(kept, keep)`, `set(memory)`, the file is 0600 in a 0700 folder and holds no memory secret;
  a second `create_secrets` on it reads the kept one set and the memory one unset; `forget(thread)` rewrites it as
  `{}` (test `only kept ones reach the file`).

## Behaviour nobody wrote down
- Rule 2.22: `slates.secrets.json` is compact, no newline. Rule 2.17: unkept secrets are never persisted.
- Rule 8.2: base64 is over UTF-8 bytes; `len` counts UTF-16 units.

## Decisions
1. A lone surrogate cannot be held in a Rust `String`; the text arrives as JSON from the wire, where serde already
   refuses or replaces it (rule 8.26 reads the escape). The port takes the text as given and never fails a scrub.
2. `encodeURIComponent` is ported as a function keeping JS's unreserved set `A-Za-z0-9-_.!~*'()`, uppercase hex.

## After the port
- Nested marks (`[se[secret:t]:s]`) when one secret's form is inside another's mark.
- A lone surrogate in a secret makes every scrub of its thread throw in the TS.

## Tests

42 tests name this unit. Rule or TS case, then the test.

| Pins | Test |
|---|---|
| runs_secrets Edge case: slate-runs.test.ts:179 | `runs_secrets::secrets::an_empty_text_clears_one_a_short_one_is_still_scrubbed_and_other_threads_secrets` |
| runs_secrets Edge case: slate-runs.test.ts:188 | `runs_secrets::secrets::only_kept_ones_reach_the_file_at_0600_and_a_new_factory_reads_them_back` |
| runs_secrets Behaviour 1: a file that exists is read as {thread: {name: {text, at}}}, every secret held as kept | `runs_secrets::create_secrets_reads_the_kept_file_and_holds_each_as_kept` |
| runs_secrets Behaviour 1 / Edge case: oracle json.parse (padding around the JSON is fine) | `runs_secrets::create_secrets_reads_a_file_with_whitespace_around_its_json` |
| runs_secrets Behaviour 1 / Edge case: a kept file that does not parse holds nothing and is not rewritten | `runs_secrets::a_kept_file_that_does_not_parse_holds_nothing_and_is_left_as_it_is` |
| runs_secrets Behaviour 1: a file that does not exist holds nothing and is not made until something is saved | `runs_secrets::create_secrets_with_a_missing_file_holds_nothing_and_writes_nothing` |
| runs_secrets Behaviour 2: the handle's two shapes, keys in this order | `runs_secrets::handle_is_the_unset_shape_or_the_set_shape_with_the_time_it_was_set` |
| runs_secrets Behaviour 2 / Rule 8.2: len counts UTF-16 units | `runs_secrets::handle_len_counts_utf16_units` |
| runs_secrets Behaviour 3: an empty text forgets the secret and answers the unset handle | `runs_secrets::set_with_an_empty_text_forgets_the_secret_and_answers_the_unset_handle` |
| runs_secrets Behaviour 3: an empty text forgets a kept secret out of the file too | `runs_secrets::set_with_an_empty_text_takes_a_kept_secret_out_of_the_file` |
| runs_secrets Behaviour 3: a secret set again without keep leaves the file | `runs_secrets::a_kept_secret_set_again_without_keep_leaves_the_file` |
| runs_secrets Behaviour 3: a secret that is not kept, replacing one that was not kept, saves nothing | `runs_secrets::setting_a_secret_that_is_not_kept_writes_no_file` |
| runs_secrets Behaviour 3: an existing name keeps its place, and a kept one set again is saved with the new text | `runs_secrets::a_secret_set_again_keeps_its_place_in_the_file` |
| runs_secrets Behaviour 4: clear forgets, then answers the handle | `runs_secrets::clear_forgets_the_secret_and_answers_the_unset_handle` |
| runs_secrets Behaviour 5: a thread that holds nothing is left alone, and nothing is saved | `runs_secrets::forget_of_a_thread_that_holds_nothing_does_nothing` |
| runs_secrets Behaviour 5: with a name drops that one; without drops the thread's whole | `runs_secrets::forget_drops_one_secret_or_every_secret_of_the_thread` |
| runs_secrets Behaviour 5: it saves only when what went was a kept one | `runs_secrets::forget_saves_only_when_a_kept_secret_went` |
| runs_secrets Behaviour 5: a whole thread with a kept secret saves, one with none kept does not | `runs_secrets::forget_of_a_whole_thread_saves_only_when_it_held_a_kept_one` |
| runs_secrets Behaviour 6: with no file nothing is written and nothing breaks | `runs_secrets::save_with_no_file_does_nothing` |
| runs_secrets Behaviour 6: threads in the order they were first held, names in held order, compact, no newline | `runs_secrets::save_writes_threads_in_first_held_order_and_names_in_held_order_compact` |
| runs_secrets Behaviour 6: with none kept it writes {} | `runs_secrets::save_with_none_kept_writes_an_empty_object` |
| runs_secrets Behaviour 6: the file is written 0600 in a folder at 0700, whatever the umask | `runs_secrets::save_makes_the_folder_0700_and_the_file_0600` |
| runs_secrets Behaviour 9: plaintext answers the text or none | `runs_secrets::plaintext_answers_the_text_or_none` |
| runs_secrets Behaviour 7: nothing held for the thread, or an empty text, answers the text | `runs_secrets::scrub_of_a_thread_with_nothing_held_or_an_empty_text_answers_the_text` |
| runs_secrets Edge case: oracle scrub.nested | `runs_secrets::scrub_matches_a_later_shorter_form_inside_a_mark_already_written` |
| runs_secrets Edge case: oracle scrub.markword | `runs_secrets::scrub_replaces_every_occurrence_of_a_form` |
| runs_secrets Edge case: oracle scrub.plus | `runs_secrets::scrub_of_a_text_with_a_space_also_scrubs_the_plus_percent_and_base64_forms` |
| runs_secrets Edge case: oracle scrub.basic | `runs_secrets::scrub_of_a_basic_header_keeps_only_the_bytes_that_share_a_group_with_the_user` |
| runs_secrets Edge case: oracle scrub.tie | `runs_secrets::scrub_of_two_secrets_of_the_same_length_marks_each_by_its_name` |
| runs_secrets Behaviour 7: the longest form goes first, so a short secret inside a long one does not cut it | `runs_secrets::scrub_replaces_the_longest_form_first` |
| runs_secrets Behaviour 7: every form a command could print the secret in is scrubbed | `runs_secrets::scrub_removes_the_secret_in_every_form_it_could_be_printed` |
| runs_secrets Behaviour 8 / Edge case: oracle secretForms.token | `runs_secrets::secret_forms_of_a_token_are_the_seven_the_oracle_lists` |
| runs_secrets Behaviour 8 / Edge case: oracle secretForms.short | `runs_secrets::secret_forms_of_a_short_text_has_no_middle_forms` |
| runs_secrets Behaviour 8 / Edge case: oracle secretForms.space | `runs_secrets::secret_forms_of_a_text_with_a_space_has_the_percent_and_plus_forms` |
| runs_secrets Behaviour 8: oracle forms.middle | `runs_secrets::secret_forms_of_sixteen_bytes_has_the_three_middle_forms` |
| runs_secrets Behaviour 8: the empty text has no forms | `runs_secrets::secret_forms_of_the_empty_text_is_empty` |
| runs_secrets Decision 2: encodeURIComponent keeps `A-Za-z0-9-_.!~*'()` and writes the rest as uppercase %XX of UTF-8 | `runs_secrets::encode_uri_component_keeps_the_unreserved_set_and_encodes_the_rest` |
| runs_secrets Behaviour 10: has, allow, revoke and list, per thread | `runs_secrets::default_approvals_hold_keys_per_thread` |
| runs_secrets Behaviour 10: keys list in insertion order, a repeat is one, and a revoke keeps the rest in order | `runs_secrets::default_approvals_list_in_insertion_order_and_a_revoke_keeps_the_order` |
| runs_secrets Behaviour 10: a revoke of a key or a thread that is not there does nothing | `runs_secrets::default_approvals_revoke_of_a_missing_key_or_thread_does_nothing` |
| runs_secrets Behaviour 10: the store is shared by the trait object the table holds | `runs_secrets::default_approvals_work_through_the_trait_object` |
| runs_table Behaviour 6-8, runs_secrets 7: slate-runs.test.ts:127 | `runs_table::values_reach_a_command_as_data::secrets::reach_a_run_through_env_and_stdin_and_are_scrubbed_from_out_err_lines_and_the_record` |
