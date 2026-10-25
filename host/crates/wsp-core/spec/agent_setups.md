# agent_setups
Source at 65f6b89c7: packages/runtime/src/agent-setup.ts (128), adapters.ts (89), and the protocol helpers they read,
packages/protocol/src/thread-defaults.ts:101-117 and :337-433, which no Rust crate holds yet (stand-ins in
`src/protocol_pending.rs`). Tests: adapters.test.ts (6), thread-defaults.test.ts:333 (1); the rest of
thread-defaults.test.ts drives `serveRuntime` and stays with the agents area. Rust: `src/agent_setup.rs`,
`src/adapters.rs`, `src/protocol_pending.rs`; tests `tests/agent_setups.rs` (10), `tests/adapters.rs` (11). Fixtures:
`agent_setups.json`, `adapters_setups.json`.

## Agent setups (agent-setup.ts)
- **R1** `real_folder_here` (:27-44): start at `posix.normalize(path)` (a trailing slash kept). Loop: `realpath` of it
  answers, joined with the parts peeled so far; else `readlink` of it answers a target, which is resolved against its
  folder (absolute targets as given, normalized) and counted as a hop; else at `/` answer the normalized input; else
  peel the last part and go up. More than 40 hops (`LINK_HOPS`) refuses with a usage error `loop_line(path)`, the path
  as given. Runs on this computer's own file system.
- **R2** `real_folder_script` (:48-56): the fixture's text word for word, one line joined by `; `; the path is
  normalized and shell-quoted, the loop line quoted for stderr, exit 3 on a loop; it prints the resolved folder then
  the home (`cd ~ && pwd -P`).
- **R3** `key_of` (:57) is `<place>:<agent>`; the collection is `agentSetups`.
- **R4** `load` (:82-84) reads every key of the collection into memory, in key order; nothing is read before it.
  `get` and `off` read memory alone; `off` is true only where the setup says `on: false`.
- **R5** `launch_of` (:87-100): `config_dir` where the setup names one; `env` is the setup's variables in their order,
  less every one `agent_env_refusal(name, agent, the agent's dropped test)` refuses now (a variable kept before the
  rule stood is left off), then the agent's state-home variable (claude `CLAUDE_CONFIG_DIR`, codex `CODEX_HOME`; none
  for the others) set to the config folder, last; `launch` with `program` and `args` where either is set.
- **R6** `set` (:101-126), checked in this order, each refusal a usage error and nothing written: a string config
  folder on an agent with no state-home variable (`no_config_dir_line(agent_name)`); a config folder not starting
  with `/` (`<agent_name>'s config folder must be an absolute path, and got <dir>`); `config_dir_refusal` of what
  `folder` resolved (the folder the links led to is what is kept); every variable set to a value that
  `agent_env_refusal(name, agent_name, dropped)` refuses (a variable set to null is never refused). Then
  `setup_patched(kept, change)`: `None` deletes the document from the store and then memory; else the store is written
  and then memory. The answer is the new setup or `None`.
- **R7** Only claude has a dropped test: `wsp_adapters::claude::landmines::is_landmine` (adapter-claude's
  `ENV_STRIP_PATTERNS`).
- What survives a restart: every document of `agentSetups`, read again by `load`; the stored document's keys are in the
  TS's order (`on, program, configDir, args, env`), which wsp-wire's `AgentSetup` writes.

## Protocol stand-ins (thread-defaults.ts)
- **P1** `setup_patched` (:101-117): env is kept's env with each named variable set or, for null, deleted; `on` is
  kept's where the change names none, absent where it says true, false where false; `program`, `configDir`, `args`
  each kept's where the change names none, absent where it names null, else the change's. The answer writes only
  present keys in the order `on, program, configDir, args, env` (env only where not empty) and is `None` where none is
  present. Fixture `patched`, key order included.
- **P2** `agent_env_refusal` (:402-407): the name upper-cased starting `WSP_` (`<name> is wsp's own, ...`); one of the
  process variables or starting `LD_` or `DYLD_` (`<name> decides how <agent> starts or what it loads, ...`); one the
  dropped test matches, tested on the name as given (`<name> never reaches <agent>: ...`); else none. Fixture
  `envRefusal`.
- **P3** `config_dir_refusal` (:421-428): home and folder with trailing slashes dropped; equal is the home line; not
  under the home (`within`: equal or starting `<home>/`) names the home and the path as given; within a kept folder
  (each with trailing slashes dropped) is wsp's own; else none. Fixture `configDirRefusal`.
- **P4** `not_run_refusal` (:337-351): `None` unless the lists are a binary's own (`source: "harness"`, a version), a
  table and a model are given, the lists do not carry the model and the table does. Older than the table's pin (the
  first `x.y.z` in the table's version, compared by `wsp_words::semver::compare_versions`, a prerelease older) with an
  update line: `<label> <version> does not run <model label>; a newer <label> does` with the fix `Update it with
  <update>, or pick a model it lists.`; otherwise `...does not run <model label>; it lists <label (value), ...>` with
  `Pick a model it lists.`. Both usage refusals. Fixture `notRun`.

## Adapters (adapters.ts)
- **D1** `secrets_of` (:33-46): nothing where the catalog has no such agent or a login stands; a token from the
  vault where the agent's sign-in mints one and the vault holds its variable (an empty token counts); else the key
  under the agent's key variable with that variable beside it; else nothing. Fixture `secrets`.
- **D2** `secret_env_of` (:50-55): the token under its variable, else the key under its variable, else empty.
- **D3** `harness_adapters` (:57-79): claude, codex, opencode, cursor, each built with the context's exec stream,
  base env, launch and sign-in refusal; claude's config folder is `home("claude")` and its project key the context's;
  codex's home `home("codex")` and its login `machine_login("codex")` (the row's fallback, else its login). claude
  takes the brokered credential where the context has one, else `secrets_of(vault, "claude", login_stands("claude"))`;
  codex with a broker takes the broker and the vault's secrets as if no login stood, else the same as claude.
- **D4** `BROKERED_AGENTS` is claude and codex; `plan_resets` and `log_limits` hold codex alone.

## Decisions
1. Factories answer `Result` (`AdapterFactory`): the adapters refuse a relative home and a key beside a ChatGPT
   broker, which the TS throws from the factory. `types::harness::HarnessAdapterFactory` cannot fail; the join makes it
   fallible or wraps these.
2. `set` takes the folder resolver and the agent's name as `SetupSetOptions`.

## Tests
| Rule | Test |
|---|---|
| R1 | `the_same_folder_read_here_follows_every_link_and_keeps_the_part_not_made_yet`, `a_path_nothing_of_which_exists_is_answered_as_named` |
| R2 | `reads_a_folder_on_another_computer_the_same_way_*` (real bash), `the_folder_read_on_another_computer_is_the_tss_shell_line_word_for_word` |
| R3, R4, R5 | `a_load_reads_every_kept_setup_and_a_launch_leaves_off_a_variable_kept_before_the_rule` |
| R5, R6, R7 | `a_setup_is_checked_kept_in_the_store_then_in_memory_and_read_by_every_launch` |
| P1 | `a_change_over_a_kept_setup_is_the_tss_patch_key_for_key_*` |
| P2 | `a_variable_is_refused_where_it_is_wsps_own_*` |
| P3 | `a_config_folder_is_refused_as_the_home_outside_it_or_inside_wsps_own_folder` |
| P4 | `a_model_the_binary_does_not_run_is_said_as_its_version_not_running_it` |
| D1, D2 | `every_vault_reads_into_the_secrets_and_the_env_the_ts_made_of_it` (tests/adapters.rs) |
| D3 | the six adapters.test.ts cases in tests/adapters.rs |
| D4 | `the_brokered_agents_are_the_ts_list`, `the_plan_reset_agents_are_the_ts_list`, `the_log_limit_agents_are_the_ts_list`, `the_adapter_registry_lists_the_agents_in_the_ts_order` |
