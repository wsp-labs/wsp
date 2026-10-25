# skill

Source: `packages/host/src/skill.ts` (66 lines) and `packages/host/src/cloud-text.ts` (51 lines) at 65f6b89c7. TS
tests: `test/skill.test.ts` (27) and the skill cases of `test/cloud.test.ts` (6). Rust tests: `tests/skill.rs` (42).
Oracle: `tests/fixtures/skill_oracle.json` (instructions in four scopes with the cloud on and off, the agents line,
`cloudText` and `skillFor` cases) and `skill_public.md` / `skill_cloud.md` (the whole skill as each process reads it),
printed by the TS.

## What it does

The wsp skill is `skills/wsp/SKILL.md`, inlined at build time (`include_str!`), read through the cloud marks for the
process's cloud reading at each ask. The MCP server's instructions are built from fixed words and the agents list.

## Rules

1. `cloud_text(text, cloud)` (cloud-text.ts:22-27), in order: remove every `<!-- boat -->` and `<!-- /boat -->`;
   replace each `<!-- cloud -->(.*?)<!-- /cloud -->` (lazy, `.` not crossing a newline) with its text when the cloud
   is on, nothing when off; then each `<!-- no cloud -->(.*?)<!-- /no cloud -->` the other way. An unclosed mark
   stays as written.
2. `skill_for(skill, cloud)` (:31-51), line by line on `split("\n")`:
   1. a line whose start-trimmed text opens with three backticks toggles fenced; a heading is `^(#+) ` outside a
      fence only;
   2. while dropping a section, skip every line until a heading of the same or fewer `#`s;
   3. a line ending ` <!-- cloud -->` with the cloud off is dropped, and a heading so marked drops its section;
   4. otherwise the trailing ` <!-- cloud -->` or else ` <!-- boat -->` is cut, and the line goes through
      `cloud_text`. Lines are joined with `\n`, so a trailing newline survives.
3. `wsp_skill()` is `skill_for(SKILL_TEXT, cloud_on())`, worked out at each call; `wsp_skill_for(cloud)` for a
   given reading. With the cloud off it equals `skill_public.md`, with it on `skill_cloud.md`, byte for byte.
4. `agents_line(agents)` (:38-40): `The agents this host runs threads on, the only values ` + (`run and fork take`
   with the cloud on, `run takes` off) + ` as agent: ` + agents joined `, ` + `.`.
5. `instructions_of(agents, scoped, slate)` (:59-62): the first line is `ANOTHER_AGENT_WORDS.`, then `SLATE_WORDS.`
   unless scoped or no slate, then `ESSENTIALS`, then the agents line, joined with one space; then three lines
   `- NOTIFY_CALLER, so nothing is polled.`, `- COORDINATOR_HANDOFF.`, `- BACKGROUND_WORK_WORDS.`; joined with `\n`.
   Scoped with a slate puts `THREAD_SLATE_WORDS` and a blank line first. `instructions` uses `THREAD_AGENTS`
   (`claude, codex, opencode, cursor`).
6. Every instruction text stays at most `INSTRUCTIONS_KEPT` (2 048) characters, with 60 to spare, cloud on and off;
   `THREAD_SLATE_WORDS` is under half of it.
7. Invariant 7 of `packages/host/AGENTS.md`: with the cloud off no skill line or instruction names Solari or a cloud
   verb; Boat's text is kept in both readings, its marks gone.
8. Constants byte for byte: `SKILL_NAME`, the four headings, `ESSENTIALS`, `SLATE_WORDS`, `THREAD_SLATE_WORDS`, the
   six marks.

## Decisions

1. `CLOUD_ON` is read once per process in the TS; `wsp_skill_for` and `*_for` take the reading so both are tested
   in one process. The skill text cases that need the verb table (`ALL_VERBS`, `VERBS`, `hasTool`, `toolName`),
   `SERVICE_MANAGERS`, `BOX_CLASSES`, the MCP server or `slate-reach.json` stay with the crates that own those.

## After the port

1. `cloud_text`'s spans do not cross a newline, so a span opened on one line and closed on the next is left with
   both marks showing (oracle case `multi<!-- cloud -->line\nspan<!-- /cloud -->end`).

## Rules to tests

| Rule | Test |
|---|---|
| 1 | cloud_text_matches_the_ts_oracle_for_every_recorded_input_with_the_cloud_on, cloud_text_matches_the_ts_oracle_for_every_recorded_input_with_the_cloud_off |
| 2 | skill_for_matches_the_ts_oracle_for_every_recorded_input_with_the_cloud_on, skill_for_matches_the_ts_oracle_for_every_recorded_input_with_the_cloud_off, cloud_marks_keep_the_text_and_drop_the_marks_with_the_cloud_on, cloud_marks_drop_a_marked_line_span_and_headings_whole_section_with_the_cloud_off, cloud_marks_keep_boats_line_span_and_headings_section_with_the_cloud_on_and_off |
| 3 | wsp_skill_for_false_is_the_public_skill_file_byte_for_byte, wsp_skill_for_true_is_the_cloud_skill_file_byte_for_byte, wsp_skill_for_is_skill_for_of_the_inlined_text_in_both_readings, wsp_skill_is_the_repos_skill_file_as_this_process_reads_it_with_a_one_line_description |
| 4 | agents_line_for_claude_and_codex_matches_the_ts_oracle_with_the_cloud_off, agents_line_for_claude_and_codex_matches_the_ts_oracle_with_the_cloud_on, the_instructions_name_every_agent_the_host_has_an_adapter_for_and_no_other_catalog_agent |
| 5 | instructions_match_the_ts_oracle_for_all_four_scopes_with_the_cloud_off, instructions_match_the_ts_oracle_for_all_four_scopes_with_the_cloud_on, a_threads_own_instructions_open_with_its_slate_and_the_others_are_left_as_they_are, the_instructions_are_what_another_agent_is_the_slate_what_wsp_is_the_agents_and_the_roads_to_a_childs_end, the_skill_hands_the_blocking_wait_to_a_shell_script_and_the_instructions_carry_both_roads_to_a_childs_end |
| 6 | the_instructions_open_with_what_another_agent_is_and_stay_whole_inside_what_claude_code_keeps, the_instructions_leave_room_under_the_cut_for_a_thread_agent_or_two_more_with_the_cloud_on_or_off |
| 7 | no_skill_line_or_instruction_names_solari_with_the_cloud_off, the_public_skill_carries_boats_spans_and_no_word_of_solari_and_both_are_kept_with_the_cloud_on, the_public_skill_says_what_wsp_remove_boat_does_with_the_cloud_off, the_cloud_skill_keeps_its_text_and_has_no_mark_left, the_public_skill_teaches_sub_threads_on_boat_right_after_add_and_prices_boat_in_the_costs |
| 8 and the skill's text | the_slate_words_and_the_skills_slate_section_say_a_request_to_see_lands_on_the_slate, the remaining 19 skill-text tests ported from skill.test.ts |
