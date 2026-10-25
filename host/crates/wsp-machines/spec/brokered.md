# brokered
Source: `packages/runtime/src/machines/brokered.ts` (133 lines) at `65f6b89c7`. Tests: no TS unit test; the behaviour
is pinned through `packages/runtime/test/channels.test.ts:293-440` (8 cases that run the line under `sh`) and by the
oracle `oracle/brokered.oracle.ts`, which writes `tests/fixtures/oracle/brokered.json`. Rust: `src/machines/brokered.rs`.

## What it does
What a turn and a terminal carry on a machine whose credentials the host keeps (a brokered machine): the turn token
wherever a credential would sit, gh pointed at the broker's unix socket, and a folder of per-agent programs a terminal's
shell finds first. All pure functions over strings and maps, plus one wrapper of an exec road. Nothing is stored and
nothing survives a restart; nothing reads the clock.

## Behaviour
1. Constants (brokered.ts:150,158,161,166): `BROKERED_GH_DIR` = `GUEST_SOCKET_DIR + "/gh"` = `/run/wsp-daemon/gh`;
   `BROKERED_BIN_DIR` = `/usr/local/libexec/wsp-terminal`; `TERMINAL_ENV` = `WSP_TERMINAL`; `TERMINAL_PROFILE` =
   `/etc/profile.d/wsp-terminal.sh`. `GH_DOOR_LINE` (:153) is private: `mkdir -p <gh dir> && printf '%s\n'
   'http_unix_socket: /run/wsp-daemon/broker.sock' > <gh dir>/config.yml`, the value through `shell_quote`.
2. `terminal_bin_of(own)` (:163) is `BROKERED_BIN_DIR + "/" + own`, with no quoting and no check: `""` gives a trailing
   slash.
3. `name_of(id)` (:168) is the catalog agent's `name` where `catalog_agents()` has that id, else the id itself.
4. `other_agent_program(other, own)` (:172): four lines joined by `\n`: `#!/bin/sh`, `printf '%s\n' <quoted said> >&2`,
   `exit 1`, `""` (so the text ends in one `\n`). `said` names `name_of(other)` twice and `name_of(own)` once.
5. `terminal_line(own, own_program)` (:181): the steps joined by ` && `: `mkdir -p <terminal_bin_of(own)>`; then one
   step per catalog agent in `catalog_agents()` order that is in `BROKERED_AGENTS` (claude, then codex): a `put` of
   `other_agent_program(agent, own)` mode 755 for every agent but `own`; for `own`, a `put` of `own_program` mode 755,
   or `rm -f <file>` when it is `None`; last a `put` of the profile line at `TERMINAL_PROFILE` mode 644. A `put` is
   `printf %s <quoted text> > <path>.$$ && chmod <mode> <path>.$$ && mv -f <path>.$$ <path>`. The file of an agent is
   `<folder>/<agent's bin>`. An `own` outside `BROKERED_AGENTS` gets the refusal program under every brokered agent.
6. The profile line (:190) is exactly `[ -z "${WSP_TERMINAL:-}" ] || [ ! -d /usr/local/libexec/wsp-terminal/"$WSP_TERMINAL" ] || PATH=/usr/local/libexec/wsp-terminal/"$WSP_TERMINAL":$PATH`
   followed by `\n`.
7. `brokered_env(token, repo, direct)` (:198): an ordered map, keys in this order: `WSP_HOST_TOKEN`, `GH_CONFIG_DIR`,
   `GH_TOKEN`, `GH_TELEMETRY` (`"false"`), then `brokered_git_env(token, repo, direct)`'s keys in its order, then
   `CARGO_NET_GIT_FETCH_WITH_CLI` (`"true"`). Later keys win a clash, which none of these have.
8. `placeholders_of(status, token)` (:215): `claude.kind` is `status.claude` when that is `"key"` or `"gateway"`, else
   `"token"` (absent status, absent field and any other word alike). `vault` is `{<codex key env>: "sk-wsp-" + token}`
   only when `status.codex == "key"` and the codex sign-in has a key variable, else empty.
9. `terminal_sign_in(own, read, token)` (:243): `claude` reads `read(Some(token))` and answers `{env:
   brokered_sign_in_env(claude)}` with no program, or `None` where the read or its `claude` is absent. `codex` reads
   `read(None)` first for its `codex` broker (absent: `None`), then `read(Some(token))` for the vault (absent read: an
   empty vault), and answers `{env: brokered_codex_env(codex, secrets_of(vault, "codex").api_key), program:
   codex_for_shell(codex, terminal_bin_of("codex"))}`. Any other agent: `None`, and `read` is never called.
10. `codex_broker_of(status, claims, models)` (:249): `status.codex == "key"` gives `{mode: "apikey"}` whatever the
    claims; else a present `status.codex` and present claims give `{mode: "chatgpt", claims, models?}` with `models`
    only when given; anything else `None`.
11. `unbrokered_refusal(harness)` (:255): `None` for `claude` and `codex`; for any other id, the empty string included,
    `brokered_agent_refusal(name_of(harness), [names of BROKERED_AGENTS in set order])`, whose kind is `usage`.
12. `with_gh_door(exec)` (:261): a road that prefixes `GH_DOOR_LINE + " && "` to the command exactly when
    `options.env["GH_CONFIG_DIR"]` equals `BROKERED_GH_DIR` byte for byte (a trailing slash does not match), and passes
    the options through untouched. The wrapped road keeps `attach` and `sweep` where the inner road has them, bound to
    it, and has none where it has none.

## Edge cases
- `terminal_line("cursor", None)` writes the refusal under both `claude` and `codex` in `.../wsp-terminal/cursor` and
  removes nothing (fixture `terminalLine[4]`).
- A program text holding `'` is quoted as `'\''` inside the `printf %s` word (fixture `terminalLine[2]`).
- `brokered_env` with a public repo also named in `direct` lists it once: the set dedups `owner/name.git` (fixture
  `brokeredEnv[3]`, 15 pairs after the 4 GitHub forms).
- `placeholders_of(Some({claude: "nonsense"}))` is kind `token` (fixture `placeholdersOf[5]`).

## Behaviour nobody wrote down
- Rule 4.26: every map here is an `IndexMap` in the TS insertion order; the fixtures compare as ordered pairs.
- Rule 8.x: no length or slice is taken, so no UTF-16 question arises.

## Tapes
None: no op serves these directly. The channel and launch ops that call them are tested in their own units.

## Decisions
1. `read` is a `&dyn Fn(Option<&str>) -> Option<LaunchRead>`; the TS passes `undefined` for "no token".
2. `with_gh_door` takes and returns `wsp_adapters::port::ExecStreamFactory` as the adapters crate declares it; where that
   type carries `attach` and `sweep` as optional members, the wrapper keeps them as above.

## After the port
- `terminal_bin_of` and `terminal_line` put `own` into a path unquoted (`mkdir -p .../<own>`): an agent id with a space
  or a quote would break the line. Ids come from the catalog today, so nothing reaches it.

## Rules against tests
| Rule | Tests (`tests/brokered.rs`) |
|---|---|
| 1, 2 | `constants_match_the_ts`, `terminal_bin_of_matches_the_ts` |
| 3, 4 | `other_agent_program_matches_the_ts_for_every_pair` |
| 5, 6 | `terminal_line_matches_the_ts_byte_for_byte`, `the_profile_puts_codex_first_only_in_a_shell_marked_codex`, `codex_typed_on_a_claude_thread_says_why_it_cannot_sign_in`, `a_second_line_for_another_agent_keeps_both_folders` |
| 7 | `brokered_env_matches_the_ts_in_key_order` |
| 8 | `placeholders_of_matches_the_ts_for_every_status` |
| 9 | `terminal_sign_in_matches_the_ts_for_every_read`, `terminal_sign_in_reads_codex_without_a_token_first` |
| 10 | `codex_broker_of_matches_the_ts` |
| 11 | `unbrokered_refusal_matches_the_ts_words_and_kind` |
| 12 | `with_gh_door_prefixes_only_the_exact_gh_dir`, `with_gh_door_keeps_attach_and_sweep_only_where_the_road_has_them` |
