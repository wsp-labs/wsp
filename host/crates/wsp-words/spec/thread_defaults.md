# thread_defaults
Source: eight functions of `packages/protocol/src/thread-defaults.ts` at the cut, 383708c78 (the rest of that file, the layers `resolveThreadDefaults` reads and its `startPicks`, has no Rust home on `rust/2448-base`; its shapes and constants are `wsp_wire::types::thread_defaults`). Tests: `tests/thread_defaults.rs` (54 cases, every value printed by node 22 for the TS at the cut).

## What it does
The sentences and rules the command line, the host and every launch read about how an agent is set up on one computer, a model a binary does not run and an access word an agent does not take. They sat in wsp-cli's join as missing (comment 6402805300 on #2449) and in #2448's rulings (the five protocol helpers).

## Behaviour
1. `setup_patched(kept, set)` (thread-defaults.ts:101): the stored setup with a change over it. The env starts from the kept one; a name the change gives `None` is taken away, any other value is set (a changed name keeps its place, a new one goes last). `on` is `false` only for `set.on == Some(false)` or a kept `false` the change leaves out. `program`, `configDir` and `args` take the change where it names the field (null puts it back on the agent's own) and the kept value where it does not. The result lists on, program, configDir, args, env, and is `None` where none is left.
2. `agent_env_refusal(name, agent, dropped)` (:402): upper-cased, a `WSP_` prefix is `wsp_env_line`; a name in the process list (`PATH`, `HOME`, `SHELL`, ..., `GIT_EXEC_PATH`) or starting `LD_` or `DYLD_` is `process_env_line`; a name one of the adapter's `dropped` patterns matches is `dropped_env_line`; else `None`.
3. `config_dir_refusal(agent, given, real)` (:421): trailing slashes taken off the home and the folder; the folder is the home, `config_dir_is_home_line`; not inside the home (a path equal to it, or starting with it and `/`), `<agent>'s config folder has to be under the home folder <home>, and <given> is not`; inside any of `real.kept`, `<given> is inside wsp's own folder, so it is no agent's config folder`; else `None`.
4. `no_config_dir_line(agent)` (:380) and `config_dir_sign_in_line(agent)` (:436) are the two sentences.
5. `pick_refusal(e, catalog)` (:313): the message of the error `startPicks` threw, with `BUILT_IN_TABLE_CLAUSE` where the catalog is the table's and the error offered no options (`offered == 0`), with `BUILT_IN_LIST_CLAUSE` where the catalog is the table's otherwise, with nothing for a binary's own or no catalog; fix `Drop the flag, or give it a value the agent offers.`; the kind the error carried. `PickError` is the TS `e`: its message, its kind word and its `offered` count (a `ListedRefusal` converts).
6. `access_word_refusal(catalog, word)` (:365): `None` where the row maps the word to a mode it lists (`access_mode`); else `usage_refusal` of `<label> takes no <word> access; it takes <the words it does take, joined by ", ">` (`<label> takes no access of wsp's words` where it takes none), fix `Name one it takes, or drop --access.`
7. `not_run_refusal(lists, table, model, update)` (:337): `None` unless `lists` is a binary's own answer with a version, the table is given, a model is named, the lists do not carry it and the table does. Said as the binary's version not running it (`not_run_line`): an older binary than the table's pin (`older_than_table`, the first `d.d.d` of the table's version against the binary's) with an update command, `usage_refusal` of `not_run_line(.., older)` with fix `Update it with <update>, or pick a model it lists.`; else of `<not_run_line>; it lists <label (value), ...>` with fix `Pick a model it lists.`

## Edge cases
- A name the change gives null that the setup never held leaves the setup as it was; the last variable taken away leaves no `env`.
- `agent_env_refusal("wsp_lower", ..)` is wsp's own whatever the case; `HOMEWORK` is no process variable.
- A kept home or folder with several trailing slashes reads as without them; `/home/mexx/c` is not under `/home/me`.
- A table whose version holds no `d.d.d` pin gives `older_than_table` none, so the newer-binary sentence is said.

## Decisions
1. The functions that throw in the TS answer `Option<HostError>` (null is `None`); `usageRefusal` and `refusal` are `exit::usage_refusal` and `exit::refusal`.
2. `AgentSetup.on` is wsp-frames' `False`; a change to false builds it with `Default`.
3. `access_mode` is public (the access words read it); `access_refusal`, `not_run_line` and `older_than_table` are the sentences the two refusals are made of.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `setup_patched_*` (9) |
| 2 | `agent_env_refusal_*` (13) |
| 3, 4 | `config_dir_refusal_*` (8), `two_lines_of_one_agent` |
| 5 | `pick_refusal_*` (6) |
| 6 | `access_word_refusal_*` (6) |
| 7 | `not_run_refusal_*` (11) |
