# Unit claude-store: the binary's lists, session titles, the conversations store

TS at a3b870a6e: `packages/adapter-claude/src/{catalog,session-title,conversations}.ts` (438 lines). Rust:
`src/claude/{catalog,session_title,conversations}.rs`. Tests: `tests/claude_store.rs` (7 + 22 + 11 ported). Each
function builds one shell line or reads one stdout; the store is on the machine, so everything survives a host
restart by living there. Nothing is cached in the host.

## catalog (catalog.ts)

- `claude_screen_commands()`: login→sign-in, logout→sign-in, model→model, permissions→access, config→settings,
  help→docs, in that order (:20).
- `version_probe_command(launch)`: `cd ~ && unset ${!CLAUDE_CODE_@} CLAUDECODE FORCE_CODE_TERMINAL; <ENV_FROM_INPUT>; <program> --version`.
- `catalog_probe_command(launch)`: `<version probe>; echo __WSP_CATALOG_SEP__; <program> --help; echo __WSP_CATALOG_SEP__; printf '%s\n' "<INIT_REQUEST with every " escaped as \">" | <program> -p --bare --output-format stream-json --input-format stream-json --verbose`
  where INIT_REQUEST is `{"type":"control_request","request_id":"init","request":{"subtype":"initialize","hooks":{}}}`.
  `<program>` is `program_word("claude", launch)`: the person's program quoted, never the pid wrapper.
- `parse_version`: first `\d+\.\d+\.\d+` anywhere; `None` if none.
- `parse_catalog_probe(stdout)`: split on the separator; fewer than 3 parts → `None`. The third part's first line that
  parses as `{type:"control_response", response:{response:{...}}}` is the init (an array inner counts and stops the
  walk; a string or null inner is skipped); none → `None`. Models (`init.models`
  must be an array, else `None`): each entry needs a non-empty string `value`; slug = (non-empty `resolvedModel` or
  value) less a trailing `[1m]`; 1M where value or resolvedModel ends `[1m]`. `value == "default"` names the default
  slug and adds no model. A repeat slug merges efforts (new ones appended) and gains both windows if 1M and it had
  none. label = non-empty displayName or slug, less a trailing ` (1M context)` (case-insensitive); description where
  non-empty; efforts the string items of supportedEffortLevels (`[]` where absent); contextWindows `["200k","1m"]` or
  `[]`; isDefault true only for the default slug. No model at all → `None`. `efforts` comes from `--help`: the text
  after `--effort <level>` (a window of 300 UTF-16 units starting at the flag, its own 16 counted) up to the first
  `(...)` only, split on `,`, trimmed, kept where `^[a-z]+$`, no dedupe.
  permissionModes: `"default"` then the quoted choices after `--permission-mode <mode>` (600 UTF-16 units from the
  flag, `(choices: ...)`, no dedupe, case-sensitive)
  less default and plan. Version may be `None`; the probe still answers.

## session titles (session-title.ts)

- `session_title_command`: `f=$(ls -1td '<config>/projects'/*/'<id>.jsonl' 2>/dev/null | head -n 1); [ -n "$f" ] || exit 0; grep -F '"type":"custom-title"' "$f" | tail -n 1; grep -F '"type":"ai-title"' "$f" | tail -n 1; true`.
- `parse_session_title`: over trimmed lines that parse as objects, the last custom-title's `customTitle` and the last
  ai-title's `aiTitle` (strings only). The rename wins wherever it sits; a blank (trimmed empty) one gives way to the
  generated, even where an earlier rename was not blank; both blank or absent → `None`. Answers trimmed. Blank and
  trimmed are by JS trim (claude-reading rule R1): U+FEFF is blank, U+0085 is not.
- `title_for_command(prompt, model, launch, sign_in)`: `cd ~ && <CLEAN>; <sign_in or "">printf '%s' '<prompt>' | <program> -p --safe-mode --output-format json --tools '' --no-session-persistence[ --model '<m>']`,
  CLEAN = `unset ${!CLAUDE_CODE_@} CLAUDECODE FORCE_CODE_TERMINAL ANTHROPIC_API_KEY; <ENV_FROM_INPUT>`.
- `draft_for_command`: `cd ~ && <CLEAN>; <sign_in>` + the same question line + ` < '<promptFile>'`.
- `parse_title_for`: the first trimmed line parsing as an object with `type:"result"`, `is_error` not true and a
  string `result`: `generated_title(result)`. Lines before it that fail any test are skipped. None → `None`.
- `parse_draft_for`: same walk; answers the result whole (untrimmed), `None` where it trims empty.
- `rename_command`: `f=$(ls -1td <glob> 2>/dev/null | head -n 1); [ -n "$f" ] || { echo no-session; exit 0; }; [ -z "$(tail -c 1 "$f")" ] || printf '\n' >> "$f" 2>/dev/null; e=$({ printf '%s\n' '<record>' >> "$f"; } 2>&1) && echo wrote || printf '%s %s\n' failed "$e"`,
  record = `{"type":"custom-title","customTitle":<title>,"sessionId":<id>}` in that key order. Each `\n` shown
  here is a real line feed (the TS template's own), three in all; `present_command`'s is two characters.
- `parse_rename`: any trimmed line `wrote` → Written; else any `no-session` → NoSession; else Failed with the text
  after the first trimmed line starting `failed ` (`failed ` alone trims to `failed`, which is then a plain line), else the non-empty lines joined by `; `, else "the machine said nothing
  about the write". A stdout naming none of them is never NoSession.

## conversations (conversations.ts)

- `origin_of`: `script` for sdk-cli, sdk-ts, sdk-py; `terminal` for anything else and for none. Never `wsp`.
- `keys_for(cwds, pinned)`: `claude_project_key` of each cwd, then pinned, deduplicated first place.
- `present_command(root, keys, file)`: `cd '<root>' 2>/dev/null && for d in '<k1>' '<k2>'; do [ -d "$d" ] && printf '%s\n' "$d"; done; true`,
  the test `-f "$d"/'<file>'` where a file is named.
- `runs_of`: chunks of 64 (TRANSCRIPT_DIRS_MAX); `[]` for an empty list.
- `live_command`: `cd ~ && unset ${!CLAUDE_CODE_@} CLAUDECODE FORCE_CODE_TERMINAL; <ENV_FROM_INPUT>; <program> agents --json`.
- `parse_live`: the trimmed stdout must parse as a JSON array, else `None`; each row's non-empty string `sessionId`.
- Store, root = config dir less trailing slashes + `/projects`; env = `build_env({base: base_env})`:
  - `list(cwds, road)`: keys = keys_for(cwds, project_dir_name); over 64 keys, ask `present_command(root, keys)` on
    road.exec and keep the keys printed, in keys order. Then one `transcripts.list` frame per (run of dirs) x (run of
    cwds), frame keys `{op, root, dirs, cwds}` in that order, all asked at once. Rows from every reply: per id the row
    with the greatest lastAt wins (a tie keeps the first). Answer in first-seen order of ids, each `{id, title: title
    ?? firstPrompt ?? id, firstPrompt?, branch?, cwd, lastAt, bytes, origin: origin_of(entrypoint)}`. A reply that
    does not parse as TranscriptsListReply rejects the list.
  - `stamp(cwds, road)`: road.exec(folders_stamp_command(each key's `'<root>/<key>'` quoted)), no env.
  - `live(road)`: road.exec(live_command, env) then parse_live; an exec failure answers `None`, never an error.
  - `earlier(id, ask, road)`: keys as list, the present check with file `<id>.jsonl`; one `transcripts.read` frame per
    run, `{op, root, dirs, session, last[, tail]}`; the first reply with `found` answers `{cwd?, title?, lines: each
    message {who, text, tool?, at?}, earlier}`; none found → Gone. Never Open (the Claude store has no lock word).
    `probe` and `count` are not read.

## Rules added for the port (numbered; each value printed by the TS at a3b870a6e)

- **S1.** `rename_command("/c", "sid", "a \"q\"\nb")` is the TS bytes, three real line feeds.
- **S2.** `parse_rename` over eleven answers, among them `failed  two spaces` → ` two spaces`, `failed ` → `failed`,
  `failed x\nfailed y` → `x`, `\n\n` → the nothing-said line.
- **S3.** A custom title of U+FEFF is blank and the generated one stands; U+0085 is a title.
- **S4.** `keys_for` replaces each UTF-16 unit: `/w/😀` → `-w---`; `/a b` and `/a-b` share `-a-b`, kept once; a pinned
  `""` is kept.
- **S5.** `runs_of` sizes 0, 1, 64, 65, 128, 129 → `[]`, `[1]`, `[64]`, `[64,1]`, `[64,64]`, `[64,64,1]`.
- **S6.** `parse_live`: non-empty string ids once, first-seen; `""`, `null`, `{}` and trailing text → None; `  []  `
  and `[1,2]` → an empty set.
- **S7.** `present_command("/r", [a, b], None)` is the TS bytes, its `\n` two characters.
- **S8.** The first result that is no error decides the title even where it gives none: `Sure!\nHere` then a good
  title → None.

## Rules against tests

| Rule | Test |
|---|---|
| probe reads models, 1M variants, efforts, modes less plan | `claude_store::parse_catalog_reads_version_models_effort_levels_1m_variants_and_flag_choices` (ported) |
| recorded 2.1.296 probe | `claude_store::parse_catalog_reads_the_pinned_versions_recording` (ported) |
| probe `None` without init, tolerant of missing parts | `claude_store::parse_catalog_is_none_without_initialize_and_tolerates_missing_version_or_help` (ported) |
| rename beats generated wherever it sits, blank falls back | `claude_store` title cases 49-77 (ported) |
| title question shape, no tools, no session kept | `claude_store` cases at session-title.test.ts:95-178 (ported) |
| rename record, half-written last line, no-session vs failed | `claude_store` cases at session-title.test.ts:182-232 (ported) |
| list frames, keys, newest per id, runs of 64 | `claude_store` cases at conversations.test.ts:53-141 (ported) |
| live None on a failed or non-list answer | `claude_store::live_is_none_where_the_command_answers_no_list` (ported) |
| earlier gone where no folder holds it, tail passed | `claude_store` cases at conversations.test.ts:101-118 (ported) |
| S1 | `claude_store::the_rename_script_is_the_ts_bytes` |
| S2 | `claude_store::a_rename_reads_its_answer_line_by_line` |
| S3 | `claude_store::a_title_is_blank_by_javascript_trim` |
| S4 | `claude_store::folder_keys_count_utf16_units` |
| S5 | `claude_store::runs_hold_at_most_64` |
| S6 | `claude_store::the_live_list_reads_only_a_whole_array` |
| S7 | `claude_store::the_presence_check_is_the_ts_bytes` |
| S8 | `claude_store::the_first_good_result_decides_the_title` |
