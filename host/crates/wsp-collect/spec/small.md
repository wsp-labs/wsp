# small

Source: `packages/collect/src/host.ts` (75), `ssh-config.ts` (26), `cache-names.ts` (8) at `a3b870a6e`. Tests: `ssh-config.test.ts` (5 cases); host.ts has none of its own. Rust: `src/host.rs`, `src/ssh_config.rs`, `src/cache_names.rs`; tests `tests/host.rs` (2), `tests/ssh_config.rs` (5); `tests/common/fake_host.rs` ports `test/fake-host.ts`.

## What it does

The Host seam every detector reads the computer through, its two path helpers, the ssh config prelude, and the cache folder names.

## Behaviour

1. `Host`, `HostFs`, `HostExec` (host.ts:4-63): the TS interface as dyn-safe traits returning one-thread boxed futures (chapter 03 keeps host state on one thread, nothing `Send`). `lines` yields `io::Result<String>`; an `Err` is the TS iterator throwing. `RunOptions.env` is laid over the collector's environment in its own order.
2. `expand(home, path)` (:66-69): `~` is home; `~/x` is `home + "/x"`; anything else unchanged (`~other/x` included).
3. `tilde(home, path)` (:72-75): home is `~`; a path starting `home + "/"` becomes `~` plus the rest; a sibling sharing the prefix (`/Users/devops`) and any other path are unchanged.
4. `with_ignore_unknown` (ssh-config.ts:24-26): text that already starts with `PRELUDE` is returned as is; otherwise `PRELUDE + text`, the empty text included. `PRELUDE` and `SSH_IGNORE_UNKNOWN_DETAIL` are constants written out in full.
5. `CACHE_DIRS` is wsp-wire's; `FINDER_METADATA` is `.DS_Store`.

## The fake host

`fake_host` keeps the TS fake's rules: a key ending `/` is a dir with that size; a dir with no own key is the sum of the files under it; mtimes default to 1000; `read_text` logs `read <path>` and answers only text values; `lines` logs `lines <path>`, splits on LF (a trailing LF gives a last empty line) and ignores `holding`; `run` logs `run <NAME=v ...> <cmd> <args>` plus ` (<ms> ms)` when a timeout is set, and answers the canned stdout for that key.

## Rules pinned

| Rule | Test |
|---|---|
| 2 | `expand_turns_a_tilde_path_into_one_under_home_and_leaves_any_other_alone` |
| 3 | `tilde_turns_a_path_under_home_into_its_tilde_form_and_leaves_any_other_alone` |
| 4 | the five `tests/ssh_config.rs` cases |
