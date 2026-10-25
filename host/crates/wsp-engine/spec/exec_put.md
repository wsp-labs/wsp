# exec_put: bytes onto a guest through exec, and many reads in few execs

Source: exec-detached.ts:1-227 at `a4033dc34`. Module `exec_detached::put`, re-exported from `exec_detached`.
Shared rules: `roads-README.md`. Oracle: `tests/fixtures/roads/exec_put.json`, `exec_paged.json`.

## Constants

`INLINE_EXEC_MS = 20_000`; `DEADLINE_EXIT = EXEC_DEADLINE_EXIT` (124); `CANCELLED_EXIT = 130`; `GUEST_TMP = "/tmp"`;
`RUN_DIR = "/tmp/wsp-run"`; `HANDSHAKE` piece `WSP_PIECE`, launched `WSP_LAUNCHED`, written `WSP_OK`, gone
`WSP_GONE`, run `WSP_RUN`; `DAEMON_ENV_FILE = "/etc/wsp/daemon.env"`; `EXEC_ENVELOPE_BYTES = 512` (private);
`READS_PER_EXEC = 8`; `OLD_APPEND_MARKS = ".a[0-9a-f]*"`; `append_marker(path) = "{path}.appended"` (:16-159).

## Small readers

- `machine_answer(res)` (:42): stdout and stderr each JS-trimmed, the non-empty ones joined by one space; none gives
  `it printed nothing and exited {code}`, else `it exited {code} and said: {said}`.
- `exec_fits(command)` (:61): UTF-8 byte length + 512 <= `EXEC_BODY_MAX` (16,384): 15,872 ASCII bytes fit, 15,873 do
  not; 7,936 `é` fit, 7,937 do not.
- `markers_of(stdout, marker)` (:105): each `\n` line JS-trimmed and split on single spaces; where the first word is
  the marker and a second word exists, key = second word, value = the rest joined by one space (may be empty). A
  later line with the same key replaces the value and keeps the key's first place (Map order).
- `paged_reads(machine, rows, line, head)` (:82): pages of at most 8 rows; `line(row, at)` gets the row's place in its
  own page. A row whose line would push the page past 8 rows or past `exec_fits` of `head + "\n" + lines joined by
  "\n"` closes the page first and re-enters as place 0 of the next (its line is rebuilt with 0). A page never goes
  out empty; a lone row that does not fit still goes alone. Each page is one `exec(cmd, timeout 20,000)`, in order,
  one after another; an exec that fails fails the whole call (pages already sent are not answered). No rows sends
  nothing and answers an empty list.

## `put_files(machine, files, opts)` (:215)

Options: `before`, `after` (lines), `timeout_ms` (default 20,000), `stdin`. Files: `GuestWrite { path, text,
append }`.

1. `upload` = 12 lowercase hex chars from 6 random bytes. `framing = machine.framed()` where the machine offers it.
2. `upload_sequence(files, before, after, upload, framing)` (:169), public so a test fixes `upload`:
   - `head = "mkdir -p " + the distinct posix dirnames of the paths in first-seen order, each shell-quoted, joined by
     a space` (with no files the line is `mkdir -p ` and a trailing space).
   - Each file is planned with `b64` (base64 of its UTF-8 text), `pieces = 0`, `mark = append_marker(path)`, `key =
     "{upload}.{index}"`.
   - Landing a file: source is `printf %s '{b64}'` for 0 pieces, else `cat '{path}'.{0..N-1}` (quoted path, brace
     range unquoted); `{source} | base64 -d > '{path}'` (`>>` for an append). A plain file's line is
     `{decode} || exit 1`. An append's is `[ "$(cat '{mark}' 2>/dev/null)" = '{key}' ] || { {decode} && printf %s
     '{key}' > '{mark}'; } || exit 1`. A pieced file adds `rm -f '{path}'.{0..N-1}` after.
   - The last exec: `head`, the before lines, `set -o pipefail`, each file's landing lines in order, the after lines,
     joined by `\n`.
   - While `exec_fits(framing.wrap(last))` is false: take the file with the longest `b64` among those with 0
     pieces (first on a tie); none left refuses (below). Piece size = floor((16,384 - 512 - bytes of
     `framing.wrap(piece(path, b64.len(), ""))`) / 4) * 4; under 4 refuses with the piece measured as
     `piece(path, b64.len(), "AAAA")`. Pieces = max(1, ceil(b64.len() / size)); each piece exec is `head`, `umask
     077`, `printf %s '{part}' > '{path}'.{i} || exit 1`, `echo WSP_PIECE`, joined by `\n`, appended in order.
   - Refusal words: the framing's `no_room` where it has one and the bare command measured fits `exec_fits`
     unwrapped; else `the lines around the upload do not fit one exec body`. Nothing is sent before a refusal.
   - Answer: every piece exec, then the last.
3. Each piece exec, in order: `exec(cmd, { timeout_ms, idempotency_key: "{upload}/{i}" })`; exit not 0 or stdout
   without `WSP_PIECE` fails at once with `a piece did not land on {id}: nothing came back saying WSP_PIECE, the word
   the guest prints once a piece is written; {machine_answer}` and the last exec is not sent.
4. The last: `exec(cmd, { timeout_ms, idempotency_key: "{upload}/last", stdin })`. Its result is answered as is
   (whatever its exit) with `pieces` = the number of piece execs.

What it leaves on a guest: the files; one `.appended` marker per appended path holding the last append's key; a
piece the last exec never joined until the caller's cleanup. Every exec lands the same run twice: a piece is a whole
file, an append is held behind its marker, so two appends to one path in one call each land (two keys).

## Pins

| Rule | Tests |
|---|---|
| input over the cap in pieces beside a small script; last exec order | `an_input_file_over_the_cap_goes_up_in_pieces_beside_a_script_that_fits` |
| every exec keyed, one upload id | `names_every_exec_of_one_upload` |
| append in pieces / one exec / twice / piece join twice | `an_append_over_the_cap_goes_up_in_pieces`, `an_append_that_fits_goes_up_in_one_exec`, `an_append_exec_run_twice_lands_the_line_once`, `a_piece_form_append_whose_join_runs_twice_lands_one_copy` |
| two appends, two calls and one call | `two_appends_carry_two_keys_in_one_marker`, `two_appends_to_one_path_in_one_call_each_land` |
| pieces counted | `answers_the_pieces_the_upload_took` |
| piece not confirmed stops before last | `a_piece_that_does_not_confirm_fails_before_the_last_exec_goes` |
| exact sequences for ten cases and three framings | `upload_sequences_match_the_ts_byte_for_byte`, `put_files_sends_what_the_ts_sent` |
| framing refusal words | `a_framing_that_leaves_no_room_is_refused_in_its_own_words` |
| exec_fits boundary in bytes | `exec_fits_counts_utf8_bytes_and_the_envelope` |
| machine_answer, markers_of | `machine_answer_matches_the_ts`, `markers_of_reads_only_its_marker_lines` |
| paged reads: 8 a page, split on size, place rebuilt | `paged_reads_split_where_the_ts_split` |
| real shell: two files, appends, markers | exec_detached.rs real-bash cases `two_files_one_over_the_cap_and_appended_land_byte_for_byte`, `two_appends_to_one_path_leave_one_marker`, `an_append_exec_and_a_piece_form_join_each_run_twice` |

## Pins (wave 2)

Values from `oracle/unit_pins.oracle.ts` (`tests/fixtures/scenes/units.json`, section `exec_put`), held by
`tests/exec_put_pins.rs`.

- U-1 `markers_of`: each line JS-trimmed and split on single spaces; kept where word 0 is the marker and word 1
  exists, an empty word 1 included: `M  x y` is key `""` with `x y`, `M k a  b` keeps `a  b`, a tab is no separator,
  the last value of a key wins in the key's first place
  (`markers_split_on_single_spaces_and_an_empty_word_is_a_key`).
- U-2 Each file's folder is `posix.dirname(path)`, not `Path::parent`: `a.txt` is `.`, `a//b/c` is `a//b`, `/f` is `/`,
  `///a` is `//`, `a/./b` is `a/.`, `a/b/` is `a`; each sent exec is byte for byte the fixture's
  (`folders_are_posix_dirname`).
