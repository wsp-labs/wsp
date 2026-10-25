# mcp_launch (with config_write)

Source: packages/catalog/src/mcp-launch.ts (327) and config-write.ts (120) at a3b870a6e. Tests: turn-servers.test.ts
(12), mcp-launch.test.ts (10), mcp-launch-codex.test.ts (3), config-write.test.ts (11). Rust: tests/turn_servers.rs,
tests/mcp_launch.rs, tests/mcp_launch_codex.rs, tests/config_write.rs (36), tests/oracle_text.rs (cksum).

## What it does

Which MCP servers a turn of an agent gets in a folder, by scope and in the order the agent reads them; the values
the vault hands one launch (whole entries for Claude Code, config keys for Codex); where each agent's own servers
live; and the one sh line that writes an agent's config file on any computer by a rename that never half-writes it.

## Behaviour

1. Paths are joined by hand (mcp-launch.ts:84-86): `normal` drops empty and `.` parts and pops on `..`; `up_from`
   lists the folder then each parent to `/`, stopping at `top` where `top` holds the folder, else the folder alone.
2. Claude Code (mcp-launch.ts:119): `key` = configs.key or the folder; servers in order: the user file's entry for
   `projects[key]` (scope local), each project file's root servers (scope project) unless named in
   `disabledMcpjsonServers`, then the user file's root servers (scope user); a name in `disabledMcpServers` reads
   disabled. Project files are `.mcp.json` from the folder up to `/`.
3. Codex (mcp-launch.ts:138): no user file means none; project files (`.codex/config.toml` from the folder up to the
   checkout top) count only where `codex_trusts(user, key)`, ahead of the user file's.
4. An agent with no road reads only the first of its project files that is there (scope user servers only), then its
   own file (mcp-launch.ts:296-303).
5. `started_servers`: the first of each name in reading order, skipped where disabled (mcp-launch.ts:148).
6. `claude_launch_servers`: nothing with no values; per started server, its entry (canonical, from its own file;
   `projects[key]` for local) with every `${N}` and `${N:-d}` of a held name replaced by the value; only entries that
   changed are returned, keyed by name (mcp-launch.ts:161).
7. `codex_launch_config`: nothing with no values; per started server with `reads` and a plain name, `mcp_servers.<s>.
   env.<n>` for each held env var with a plain name and `mcp_servers.<s>.http_headers.<h>` for each held header
   variable with a plain header name; the bearer variable never (mcp-launch.ts:194).
8. `unset_toml_keys`: the keys a TOML text sets no top-level value of; none where it does not parse (mcp-launch.ts:180).
9. Roads (mcp-launch.ts:258): Claude's user file `<store or home>/.claude.json`, carry keeps the folder, switches are
   its two lists, `hands` is always true; Codex's `<store or home/.codex>/config.toml`, carry is the folder's own
   `.codex/config.toml` with trust, `hands` only an env var or header variable the server passes through.
10. `own_server_config`: a store with a road gives the road's user file and `store_base(store, home)`; else the
    catalog files with `~/` under home and base home. `turn_server_files` drops project files that are the agent's own.
11. `launch_misses`: the names `resolve` asks for that are held and the road cannot hand, sorted (UTF-16 order).
12. `checkout_of`: the first line must be absolute to count; the key is the parent of a `--git-common-dir` ending in
    `/.git`, else the top; no answer keys by the folder.
13. `config_write_line` (config-write.ts:389) is the TS line byte for byte: umask 077; the landing trap; the base's
    realpath; an existing file must be expected (else exit 5 naming it), resolve inside the base (else exit 4 with
    the path and its link target, each NUL-ended), and have one link (else exit 6); a new file must not exist, its
    nearest existing folder inside the base, folders made; stale temp files of the three minted shapes older than 10
    minutes swept in both folders; mktemp beside the real path; `cp -p` the old file for its mode; bytes from stdin or
    the landing; the byte count checked; the cksum checked (or absence); `mv -f` over the real path.
14. `config_refusal` reads the first NUL-ended path off stdout (else the file) and gives the link, changed or hard
    link sentence for exits 4, 5, 6; nothing for any other exit. `config_landing` refuses an id that is not twelve
    lowercase hex digits. `config_sum` is POSIX cksum: CRC-32/CKSUM over the bytes then the length's bytes, low
    first, printed `<crc> <length>`.

## Edge cases

- `checkout_of` of a relative first line keys by the folder.
- A killed write leaves the old file whole and its temp file swept by the next write (config-write.test.ts).
- The real-codex test (mcp-launch-codex.test.ts) is skipped where no `codex` is on PATH, as the TS's `skipIf`.

## Behaviour nobody wrote down

- Rule 4.26: `claude_launch_servers` and `codex_launch_config` keep started-server order; IndexMaps throughout.
- Nothing here survives a restart; the write's temp files are swept by age, not by state.

## Tapes

sessions.start on a computer the person owns with a server that reads a held value (#2443).

## Decisions

1. Synchronous; `TurnAgent` is a flat struct that callers build from an `AgentEntry` with an `mcp`.
2. `config_refusal` takes the exit code and stdout as two arguments for the TS `res` object.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1, 2, 5 | turn_servers: a_claude_code_turns_servers::* (5) |
| 3, 8 | turn_servers: a_codex_turns_servers::* (4); mcp_launch: a_codex_thread_starts_config::reads_a_folders_trust_however_the_toml_spells_it_a_dotted_key_and_an_inline_tabl |
| 4 | turn_servers: an_agent_with_no_road_of_its_own::reads_the_first_of_its_project_files_in_the_folder_before_its_own_file |
| 6 | mcp_launch: a_claude_code_launchs_servers::* (4) |
| 7 | mcp_launch: a_codex_thread_starts_config::sets_only_the_held_names_on_each_server_that_passes_them_through_so_codex_lays_t, ::reads_the_projects_file_only_where_codexs_own_marks_the_folder_trusted; mcp_launch_codex: * (3) |
| 9, 10 | turn_servers: what_a_projects_servers_travel_in::* (2), a_codex_turns_servers::reads_the_folders_own_codex_config_toml_under_the_store_its_turns_read_and_each |
| 11 | mcp_launch: the_held_names_a_launch_on_a_computer_the_person_owns_cannot_hand_a_server::* (3) |
| 12 | turn_servers: a_claude_code_turns_servers::keys_a_worktrees_local_entry_by_the_projects_folder_as_claude_code_keys_it_by_th, ::keys_a_subfolders_local_entry_by_its_checkouts_top_as_git_answers_it_there |
| 13, 14 | config_write: config_write_line::* (9), under_base::holds_every_path_inside_a_base_of_and_a_sibling_that_shares_a_bases_name_out_of, config_sum::is_what_cksum_prints_for_the_same_bytes; oracle_text: the_checksum_is_what_posix_cksum_prints |
