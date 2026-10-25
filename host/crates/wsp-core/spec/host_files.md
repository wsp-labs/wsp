# host_files
Source at 65f6b89c7: packages/runtime/src/daemon-token.ts (69), daemon-roots.ts (18), host-id.ts (44),
github-cache.ts (25). Tests: daemon-token.test.ts (6), host-id.test.ts (7). Rust: `src/daemon_token.rs`,
`src/daemon_roots.rs`, `src/host_id.rs`, `src/github_cache.rs`; tests `tests/host_files.rs` (26). Fixture:
`tests/fixtures/core_rest/daemon_files.json`.

## What it does
The files the host writes for itself and for a daemon: the daemon's token (derived per machine, written whole), the
daemon's roots file, this install's id, and the in-memory cache of reads off a git host.

## Behaviour
1. **Token shape** (daemon-token.ts:19): `^[0-9a-f]+$` or `Err("a daemon token is lowercase hex")`. The empty string,
   uppercase, a newline at the end and any quote are refused.
2. **Token for a machine** (:26): checks the seed's shape first, then HMAC-SHA256 keyed by the seed's **UTF-8 text**
   (not the bytes the hex spells) over the machine id's UTF-8 bytes, as 64 lowercase hex. The empty machine id and a
   non-ASCII id are hashed like any other (fixture `tokenFor`).
3. **Write script** (:33): four lines joined by `\n`: the assignment `WSP_DAEMON_TOKEN='<token>'`, `umask 077`, the
   printf of that variable to `<path>.next`, `mv -f <path>.next <path>`. The path is DAEMON_TOKEN_PATH
   (`/root/.wsp/daemon-token`) where none is given and is **not quoted**. The token appears on line 1 alone.
4. **Rotate script** (:44): `test -f <path> || { echo WSP_DAEMON_TOKEN_NONE; exit 0; }`, the write script, `echo
   WSP_DAEMON_TOKEN_SET`. A bad token is refused (through the write script) before any text is made.
5. **Token path of a machine** (:53): the machine's own `daemon_token_path()` wins over the road's; else the road's;
   `None` where neither names one.
6. **Rotate** (:59): token shape first (nothing runs on a bad token). A machine with no byte road
   (`wsp_engine::land_bytes::has_byte_road` false) runs the rotate script once and answers `exit 0 && stdout contains
   SET`. With a byte road: runs `test -f '<path>' && echo SET || echo NONE` (the path shell-quoted here, unlike 3),
   answers false unless exit 0 and SET, then lands the token's bytes at the path over `land_bytes` and answers true.
   An exec error propagates.
7. **Roots script** (daemon-roots.ts:10): four lines: `next='<path>.next.'$$`, `mkdir -p '<posix dirname of path>'`,
   `printf '%s\n' <each root shell-quoted, space-joined> > "$next"`, `mv -f "$next" '<path>'`. No roots leaves two
   spaces before `>`. The dirname of `roots` is `.`, of `/roots` is `/`. The path defaults to DAEMON_ROOTS_PATH.
8. **Local config dir** (host-id.ts:10): `$XDG_CONFIG_HOME/wsp` when set and not empty, on every platform; else
   `~/Library/Application Support/wsp` on macOS, `~/.config/wsp` elsewhere.
9. **Host identity** (:21): `<hostname>:<id>` where the id is read from `<dir>/host-id` trimmed; an absent or empty
   file mints 4 random bytes as 8 lowercase hex and writes `<id>\n` with `wsp_store::own_file::write_own(dir,
   "host-id", ..)` (dir 0700, file 0600). A file that reads is never written again (same bytes, same mtime). Any
   failure answers the bare hostname and warns once per process: `host id not kept in <dir> (<why>); holds carry the
   hostname alone`; a later failure warns nothing. Nothing is created where the dir cannot be made.
10. **Template host** (:42): the text after the last `:` (all of it where there is none), lowercased (Unicode
    lowercase, as JS `toLowerCase`), then every char outside `a-z0-9` dropped (fixture `templateHost`: `Ünïcode:ÄB9`
    gives `b9`, `host:` gives the empty string, `x:İ` gives `i`).
11. **GitHub cache** (github-cache.ts:18): get answers a copy of the entry set under the key, `None` for one never set
    or deleted; set replaces; delete of an absent key does nothing. Held for the host's life; nothing survives a
    restart.

## Decisions
1. Errors are `HostError` (`HostError::new(TOKEN_NOT_HEX)`), the TS's plain `Error`.
2. HMAC is RFC 2104 over `sha2::Sha256` (64-byte block); the crate needs no new dependency.
3. `hostname()` is the OS's (`libc::gethostname`), what node's `os.hostname()` reads. The warn-once flag is a
   process-wide atomic, as the TS's module `let`.

## After the port
- The rotate script (4) puts the unquoted path in `test -f`, `>` and `mv`; a path with a space breaks it. The byte
  road quotes it (6). Pinned as the TS writes it.

## Tests (`tests/host_files.rs`)
| Rule | Test |
|---|---|
| 1 | `the_scripts_refuse_anything_but_hex_so_the_quoting_is_always_enough`, `the_token_shape_check_is_the_ts_answer_for_every_row` |
| 2 | `a_machines_own_token_is_the_seeds_one_answer_for_that_machine`, `a_machines_own_token_is_hex_and_a_seed_that_is_not_hex_is_refused_where_it_is_given`, `the_token_for_a_seed_and_a_machine_is_the_ts_answer_for_every_row` |
| 3 | `the_write_script_holds_the_token_only_in_its_assignment_owner_readable_and_replaced_whole`, `the_token_write_script_is_the_ts_script_for_every_row` |
| 4 | `rotate_leaves_a_machine_without_a_daemon_alone_and_names_each_outcome_on_stdout`, `the_token_rotate_script_is_the_ts_script_for_every_row` |
| 5 | `takes_the_machines_own_file_where_it_names_one_whatever_road_asked_and_the_roads_otherwise` |
| 6 | `rotate_on_a_machine_with_no_byte_road_runs_the_one_script_and_answers_true_where_the_daemon_was_there`, `rotate_on_a_machine_with_no_byte_road_answers_false_where_there_was_no_daemon_or_the_script_failed`, `rotate_refuses_a_token_that_is_not_hex_before_anything_runs` |
| 7 | `the_roots_script_is_the_ts_script_for_every_row`, `the_daemon_file_constants_are_the_ts_values` |
| 8 | `xdg_config_home_names_the_default_dir_so_no_test_reaches_the_developers_real_config_dir` |
| 9 | `host_identity_is_the_hostname_plus_a_per_install_id_made_once_in_the_given_dir_and_read_back_after`, `a_second_read_writes_nothing_over_the_id_it_finds_the_same_bytes_and_the_same_mtime`, `the_id_and_the_folder_it_sits_in_are_this_users_alone_whatever_the_umask`, `host_identity_falls_back_to_the_hostname_alone_when_the_dir_cannot_be_made_and_says_so_once` |
| 10 | `template_host_is_the_per_install_hex_id_alone_never_the_hostname`, `template_host_reduces_an_identity_that_fell_back_to_the_bare_hostname_to_the_same_character_class`, `template_host_is_the_ts_answer_for_every_pair` |
| 11 | the three `the_memory_github_cache_*` tests |

The byte-road half of 6 is not tested here: it runs `wsp_engine::land_bytes`, whose tests are the engine's.
