# seed (seed, first-run, git-hosts, thread-agents, ecosystems)

Source: packages/catalog/src/seed.ts (224), first-run.ts (102), git-hosts.ts (65), thread-agents.ts (13),
ecosystems/*.ts (about 80) at a3b870a6e. Tests: seed.test.ts (16), first-run.test.ts (6), git-hosts.test.ts (2),
ecosystems.test.ts (2, one skipped without cargo). Rust: tests/seed.rs, tests/first_run.rs, tests/git_hosts.rs,
tests/ecosystems.rs, tests/oracle_data.rs (rows, judgements, installs, ecosystems, git hosts, thread agents).

## What it does

Which ignored paths of a project travel and how (seed rows), which install a project's root runs, which commands on
PATH are first-run installers, which git hosts clone through a signed-in CLI, which agents run threads, and what a new
worktree carries per ecosystem.

## Behaviour

1. A seed pattern (seed.ts:182): a trailing `/` asks for a directory; no `/` matches any segment; a `/` matches the
   path's tail at a segment boundary; `*` is `[^/]*`; every other regex character is literal.
2. `seed_row_for(path, dir)` (seed.ts:203): `./` and trailing slashes stripped, split on `/`; kinds in the order never,
   junk, secret, config, rebuilt, data; rows in table order within each; a row's `except` patterns skip it.
3. `seed_installs_for(names)`: per row in order, the first of its installs whose lockfile is among the names.
4. First-run installers (first-run.ts): mise reads the first `mise use|install` line's first non-flag word (quotes and
   trailing `;` stripped, stopping at `|| && ; |`), and tests its installs folder named by heck's kebab case of the
   spec less its version; lazy-run needs `# ascii-harness-shim` as line 2 and takes the last `exec /path "$@"` line.
5. `installs_on_first_run(run, bins)`: none for no bins; one script printing each command's first 4096 bytes behind a
   `\x1e`; only where some head is an installer, a second script of one `echo` per bin; true only where that line is
   `1`. A run that fails answers false for every bin.
6. Git hosts: github (gh) and gitlab (glab) rows; `remote_host` strips a `scheme://`, a `user@`, takes the part
   before the first `/` or `:`, lower-cased, nothing where empty; `git_host_of` matches it against a row's ssh hosts.
7. `runs_threads` is claude, codex, opencode or cursor exactly.
8. Ecosystems: npm, pnpm, yarn, bun, composer, uv, poetry, cargo, go, each with its lockfiles, what a worktree
   carries and never carries, where its install leaves a copy of the lockfile, and the rebuild command.

## Edge cases

- `config/credentials/x/y.key` and `myconfig/master.key` are both secret (a `*.key` before any configuration row).
- `.env.example` is no row's (node's `except`).

## Behaviour nobody wrote down

- Rule 4.26: rows and installs keep table order.

## Tapes

project.seed.plan, projects.seed and worktree.make (#2443).

## Decisions

1. `RunScript` is a trait with an async `run` for the TS callback.

## After the port

- None found.

## Rules to tests

| Rule | Tests |
|---|---|
| 1, 2 | seed: the_row_a_path_git_ignores_lands_on::* (9), the_rows_themselves::* (2); oracle_data: the_seed_rows_are_the_ts_rows_and_judge_every_path_as_the_ts_does |
| 3 | seed: the_install_a_projects_own_root_picks::* (4), a_managers_check_on_its_install_row::*; oracle_data: a_root_picks_one_install_per_ecosystem_the_first_listed_lockfile_winning |
| 4, 5 | first_run: a_mise_wrapper::* (2), an_ascii_lazy_run_shim::* (2), installs_on_first_run::* (2) |
| 6 | git_hosts: the_host_a_remote_names::* (2); oracle_data: the_ecosystems_and_the_git_hosts_are_the_ts_tables |
| 7 | oracle_data: only_the_four_thread_agents_run_threads; catalog: catalog::says_which_agents_wsp_opens_threads_on |
| 8 | ecosystems: a_module_s_rebuild_on_a_branch_whose_lockfile_is_stale::* (2, the cargo one ignored where no cargo); oracle_data: the_ecosystems_and_the_git_hosts_are_the_ts_tables |
