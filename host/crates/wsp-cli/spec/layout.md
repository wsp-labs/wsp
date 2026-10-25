# layout

Source: `packages/host/src/init-layout.ts` (232 lines) over `@clack/core` 1.4.3 and `@clack/prompts` 1.7.0, and
`packages/host/src/watch.ts` (103 lines), at 65f6b89c7. TS tests: `test/init-layout.test.ts` (25), `test/watch.test.ts`
(8). Rust tests: `tests/init_layout.rs` (66), `tests/watch.rs` (17). Oracle: `tests/fixtures/layout_oracle.json`,
every value and every prompt's raw bytes printed by the TS on node 22.23.

## What it does

The text rules the init screens and the listings share (cells, cuts, columns, wrap, rewind), the colour rules, a
card, the two clack prompts drawn in wsp's frame, and the block a `--watch` line redraws in place.

## Text rules

1. `cell_width` (:24-28) sums grapheme clusters (Unicode extended graphemes, as `Intl.Segmenter`): 2 when the
   cluster opens with a code point in the `WIDE` ranges (:20) or holds `Emoji_Presentation` or U+FE0F; 0 when it
   opens with a mark (Mn, Me) or a format character (Cf); else 1. `👩‍💻` is 2, `é` is 1.
2. `ellipsize(text, width)` (:32-44): width ≤ 0 is `""`; text that fits is itself; else whole clusters while
   `used + n ≤ width - 1`, then `…`. A cluster is never torn.
3. `pad_cells` pads with spaces to `width` cells (none when wider), on the right or the left (:47-50).
4. `table` (:55-62) pads each column to its widest cell by UTF-16 length (`padStart`/`padEnd`), right where asked,
   joins with two spaces and trims the end of each row.
5. `width_of(columns, cap)` is `min(columns or 80, cap)`, cap 100 by default; `rows_of` is rows or 20 (:65-73).
6. `viewport(total, cursor, height)`: h = max(1, height); start = max(0, min(cursor - floor(h/2), total - h));
   end = min(total, start + h) (:76-80).
7. `summarize(labels, width, named = 3)` (:83-90): for n from min(named, len) down to 2, the first n joined `, ` plus
   ` +<rest> more` when it fits the width in UTF-16 units; else the first label ellipsized to `width - len(more)`
   plus ` +<len-1> more`; `""` for no labels.
8. `wrap(text, width, indent = "  ")` (:94-101), in UTF-16 indexes: text that fits is itself; `lead` = leading
   spaces; cut at the last space at or before `width`, or the first space after `lead` when that is before `lead`;
   no space at all is `[ellipsize(text, width)]`; else the head trimmed at its end and ellipsized, then the wrap of
   `indent + rest trimmed at its start`. A word wider than the width is cut alone and the words after it go on.
9. `cells(row)` is the UTF-16 length with VT sequences stripped (:104). `rewind(drawn, columns)` is
   `ESC[<rows>A ESC[G ESC[J` with rows = Σ max(1, ceil(w / columns)), `""` when there are none (:110-113).

## Colour rules

10. `node_color_depth(env)` is node 22's `getColorDepth` (lib/internal/tty.js), held to the oracle's 29 rows:
    `FORCE_COLOR` `""`/`1`/`true` 4, `2` 8, `3` 24, `0`/`false` and any other value 1; without it `NODE_DISABLE_COLORS`,
    any `NO_COLOR` (empty too) and `TERM=dumb` 1; `TMUX` 24; CI tables (`CI` with `GITHUB_ACTIONS` 24, `CI` alone
    1); `TERM_PROGRAM` iTerm.app ≥ 3 24, Apple_Terminal 8; `COLORTERM` truecolor or 24bit 24; `TERM` by node's
    tables (`xterm-256color` 8, `xterm-kitty` 24, `xterm`, `screen`, `vt100`, `linux`, `rxvt-unicode-256color` 4);
    nothing set 1. `colour_depth(tty, env)` is 1 off a terminal without `FORCE_COLOR`, else that (:119-122).
11. `styles_on(env, stdout_tty)`: with `FORCE_COLOR` set, depth > 2; else stdout a terminal and depth > 2.
    `style_text` wraps in each format's open code, closes in reverse order; plain when off.
12. `muted(s, depth, on)`: depth ≤ 1 plain; < 8 `style_text(dim)`; ≥ 8 `grey(243)` = `ESC[38;5;243m s ESC[39m`,
    painted whatever `on` says (:141). `help_line` (:144-148): depth ≤ 1 `key does` joined by three spaces; else keys
    plain (< 8) or grey 247, `does` muted, the three-space gap muted too. `accent` is cyan through `style_text`.

## Card and prompts

13. `card` (:156-159; clack `log.message`, prompts index.mjs:726-748): one write of `BAR`, `\n`, green
    `STEP_SUBMIT` + two spaces + bold title, then each line as `BAR` + two spaces + line, an empty line as the bare
    `BAR`, joined by `\n` with a closing `\n`. `BAR` is gray (`ESC[90m`) through `style_text`. A line wider than
    `width_of(columns) - 3` once stripped is wrapped by rule 8; a narrower coloured line passes whole.
14. Glyphs (clack, prompts index.mjs:27, init-layout.ts:15-16): unicode unless `TERM=linux`: bar `│`/`|`, focus
    `┃`/`|`, focus end `┗`/`+`, active `◆`/`*`, cancel `■`/`x`, submit `◇`/`o`, radio `●`/`>` and `○`/` `, mask
    `▪`/`*`.
15. A prompt's frame (`promptFrame`, :180-191): the message split on `\n`, each wrapped to `width_of(output) - 3`,
    the first on the glyph (cyan active, green submit, red cancel) and the rest on the bar, cyan while open; the
    hint's lines wrapped and dim; the body rows; while open, the dim focus end and the help line at
    `paint.depth`. The bar is the dim focus bar while open, the dim thin bar after.
16. Confirm (:196-218): body `○ Yes / ● No` (marked radio cyan, unmarked radio and label dim, `/` dim) while open,
    the dim picked label on submit, struck-through dim on cancel. Keys: left and right flip the marker (other
    cursor keys as clack's `ConfirmPrompt` reads them, unpinned); `y` and `n` answer at once; enter answers the marker; a lone escape (after readline's pause) and
    ctrl-c cancel with the marker as it was (clack's escape alias must not flip it). Initial marker No.
17. Password (:221-232): body the masked text plus the cursor (`_` inverse and hidden, `ESC[7m ESC[8m _ ESC[28m
    ESC[27m`, a plain `_` with colour off) while open; on submit the dim mask, on cancel the struck-through dim
    mask, nothing when empty. Backspace (`\x7f`) removes one character. The text itself is never written.
18. Render (`@clack/core` index.mjs:306-350): the first frame is `ESC[?25l` + frame. A changed frame moves to the
    top (`ESC[999D` and `ESC[<lines-1>A`); one changed line: down to it (`ESC[<r>B` when r > 0), `ESC[2K ESC[G`, the
    line, down to the end (`ESC[<rest>B` when > 0); several: down to the first changed line when > 0, `ESC[J`, the
    frame from that line. Frames are hard-wrapped at `paint.stdout_columns` (none when `None`). On submit or cancel
    the last frame, then `\n`, then `ESC[?25h`. The oracle's `raw` bytes are the authority.
19. A prompt answers `Answer::Value` or `Answer::Cancel`; `signal` settling cancels and hands the terminal back.

## Watch

20. `watch_block` (watch.ts:53-94): hang one listener on SIGINT, SIGTERM and SIGHUP in that order, write
    `ESC[?25l`; loop: read the rows (an error ends the watch and is returned); rows non-empty: one write of
    `rewind(drawn, columns now) + rows joined "\n" + "\n"`, then `drawn` = each row's `cells`; empty rows write
    nothing and keep `drawn`; stopped: break; wait for the tick (`every_ms`, 1 000) raced with `until()` when given;
    a stop cuts the wait at once. Every road out takes the three listeners off in order and writes `ESC[?25h`. A
    stop that arrives while a frame is being read still draws that frame, then ends.
21. `watch_on(words, json, redraw)` (:99-103): `--json` first: usage `<words> --watch redraws a table and --json
    answers with objects.` fix `Take one of the two: <words> --watch at a terminal, or <words> --json for the
    objects.`; no terminal: usage `<words> --watch redraws where it stands, and this run has no terminal to redraw
    on.` fix `Run <words> without --watch to print the list once.`; else the redraw.

## Decisions

1. The process-wide `styleText`, `unicode` and `process.stdout.columns` readings come in as `Paint` so tests set
   them; `PromptOptions.input` is a channel of chunks.

## After the port

1. `wrap` and `table` count UTF-16 units where `ellipsize` counts cells, so a wide-character label pads short.
2. `summarize` measures `line.length`, not cells.

## Rules to tests

| Rule | Test |
|---|---|
| 1, 2 | cell_width_matches_every_recorded_text, ellipsize_matches_every_recorded_cut, cell_width_counts_east_asian_wide_characters_and_emoji_as_two_and_combining_marks_as_none, ellipsize_cuts_to_cells_and_never_through_a_character, ellipsize_keeps_text_that_fits_and_ends_cut_text_with_one_ellipsis_inside_the_width |
| 3, 4 | pad_cells_matches_every_recorded_pad, table_matches_every_recorded_table, table_pads_each_column_to_its_widest_cell_right_aligns_where_asked_and_leaves_no_trailing_space |
| 5 | width_of_reads_the_streams_columns_falls_back_to_80_and_caps_at_100, width_of_caps_at_the_cap_it_is_given, rows_of_reads_the_streams_rows_and_falls_back_to_20 |
| 6 | viewport_matches_every_recorded_window, viewport_keeps_the_cursor_mid_window_pins_the_window_at_both_ends_and_never_goes_under_one_row |
| 7 | summarize_matches_every_recorded_summary, summarize_names_up_to_three_labels_then_n_more_drops_labels_when_the_width_is_short_and_cuts_the_last_one_kept, summarize_names_as_many_labels_as_it_is_told_to |
| 8 | wrap_matches_every_recorded_break_with_the_default_and_a_given_indent, wrap_breaks_a_line_at_word_ends_inside_the_width_indents_the_rest_by_two_and_cuts_a_word_longer_than_the_width |
| 9 | cells_counts_a_row_without_its_escape_codes_in_utf16_units, rewind_matches_every_recorded_block |
| 10 | colour_depth_matches_node_for_every_recorded_env_on_and_off_a_terminal, node_color_depth_matches_every_recorded_terminal_depth, colour_depth_is_1_off_a_terminal_and_when_the_env_says_no_colour_nodes_depth_for_term_and_colorterm_otherwise_and_force_color_wins |
| 11 | styles_are_on_for_force_color_above_two_bits_and_otherwise_only_on_a_deep_enough_terminal, style_text_wraps_the_text_in_each_formats_codes_and_closes_them_in_reverse_and_is_the_text_alone_when_off |
| 12 | muted_matches_every_recorded_depth, help_line_matches_every_recorded_depth, help_line_with_no_keys_is_empty_at_every_depth, muted_paints_a_grey_from_256_colours_up_dim_under_them_and_nothing_off_a_terminal, help_line_is_plain_at_depth_1_and_sets_keys_and_what_they_do_three_spaces_apart, help_line_under_256_colours_leaves_keys_plain_and_dims_the_rest_with_no_other_code, help_line_from_256_colours_up_paints_two_greys_the_key_brighter, grey_paints_a_256_colour_code_and_the_accent_is_cyan_only_where_styles_are_on |
| 13 | card_matches_the_recorded_bytes_at_forty_columns_without_colour, card_matches_the_recorded_bytes_with_colour, card_prints_a_bold_title_on_the_step_glyph_and_its_lines_down_the_bar_an_empty_line_as_a_bare_bar_and_long_lines_wrapped_to_the_width |
| 14 | the_focused_bar_and_its_end_are_one_cell_wide_and_ascii_off_a_unicode_terminal, unicode_is_on_unless_the_term_is_linux |
| 15, 16, 18 | confirm_prompts_write_the_recorded_bytes_and_answer_as_recorded_without_colour, confirm_prompts_write_the_recorded_bytes_and_answer_as_recorded_at_16_colours, confirm_prompts_write_the_recorded_bytes_and_answer_as_recorded_at_truecolor, prompts_under_no_color_write_the_recorded_bytes_and_answer_as_recorded, confirm_draws_the_question_on_the_step_glyph_the_hint_and_two_answers_down_the_thick_bar_and_enter_takes_the_marked_answer, confirm_left_and_right_move_the_marker_and_enter_takes_it, confirm_y_answers_yes_at_once, confirm_n_answers_no_at_once_even_when_the_marker_started_on_yes, confirm_esc_cancels_with_the_marked_answer_struck_through_and_clacks_escape_alias_does_not_flip_it_first, confirm_ctrl_c_cancels, confirm_wraps_a_long_question_and_hint_at_the_width_the_rest_under_the_bar_with_the_two_space_indent, confirm_takes_a_newline_in_the_question_as_a_line_break, confirm_with_colour_on_paints_cyan_glyph_and_question_dim_hint_and_bar_cyan_marker_and_two_help_greys_and_no_other_colour, confirm_at_16_colours_dims_the_help_words_and_no_256_colour_code_leaves_the_frame, confirm_under_no_color_carries_no_sgr_at_all_and_keeps_its_glyphs_and_dots |
| 17 | password_prompts_write_the_recorded_bytes_and_answer_as_recorded_without_colour, password_prompts_write_the_recorded_bytes_and_answer_as_recorded_at_16_colours, password_prompts_write_the_recorded_bytes_and_answer_as_recorded_at_truecolor, password_masks_every_character_never_echoes_the_text_and_returns_it_on_enter_and_the_finished_screen_shows_the_mask_on_the_thin_bar, password_enter_on_nothing_returns_the_empty_string_with_no_mask_line, password_esc_cancels_with_the_mask_struck_through_and_the_text_never_written, password_ctrl_c_cancels, password_with_colour_on_paints_cyan_glyph_and_question_dim_hint_and_bar_the_inverse_cursor_and_two_help_greys_and_no_other_colour, password_at_16_colours_dims_the_help_words_and_no_256_colour_code_leaves_the_frame, password_under_no_color_carries_no_sgr_at_all_and_keeps_its_glyphs_and_dots |
| 19 | confirm_ends_as_a_cancel_when_its_signal_fires_before_an_answer, password_ends_as_a_cancel_when_its_signal_fires_before_an_answer, is_tty_is_true_only_for_a_stream_that_is_a_terminal |
| 20 | watch_draws_one_write_per_frame_rewinds_the_rows_the_last_frame_took_and_gives_the_cursor_back_on_the_stop, watch_counts_the_rows_a_frame_wrapped_onto_at_the_width_the_terminal_has_now, watch_reads_the_width_again_for_every_frame, watch_ends_with_the_cursor_back_and_the_refusal_carried_out_to_the_caller_when_a_draw_fails, watch_keeps_the_frames_it_drew_when_a_later_draw_fails, watch_hangs_one_listener_per_signal_and_takes_them_all_back_off_when_it_ends, watch_hangs_the_signals_in_order_before_it_draws_and_takes_them_off_in_the_same_order, watch_draws_nothing_for_empty_rows_and_leaves_the_block_where_it_is, watch_draws_as_soon_as_the_source_pushes_rather_than_waiting_out_the_tick, watch_keeps_drawing_on_the_tick_when_the_source_stops_pushing, watch_ends_a_wait_the_moment_a_stop_arrives_instead_of_drawing_another_frame, watch_waits_a_second_between_frames_when_it_is_given_no_interval |
| 21 | watch_on_refuses_where_there_is_no_terminal_to_redraw_on_in_two_halves, watch_on_refuses_beside_json_in_two_halves, watch_on_refuses_json_before_it_looks_for_a_terminal, watch_on_hands_back_the_same_terminal_to_draw_on_when_there_is_one_and_no_json, watch_on_refusals_carry_the_recorded_message_usage_kind_and_fix |
