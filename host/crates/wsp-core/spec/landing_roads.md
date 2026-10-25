# landing_roads
Source at 65f6b89c7: packages/runtime/src/project-landing.ts:354-481 and :506-639 (the roads, about 300 lines) and
project-sources.ts (124). Tests: box-folder.test.ts:578, :590; project-landing.test.ts drives `createRuntime` and
stays with the projects area. Rust: `src/project_landing.rs` (the roads), `src/project_sources.rs`; tests
`tests/landing_roads.rs` (16), `tests/project_sources.rs` (10). Fixtures: `landing_roads.json`, `project_sources.json`.
A road's machine is a test machine that answers by needle and records each command with its timeout and input.

## Project sources (project-sources.ts)
- **S1** (:109-124) Four modules in order folder, git, github, gitlab; another kind is `no project source module for
  <kind>` (no kind stamped). `project_remote` is the module's own remote: the empty string for a folder, the url for
  git, `host.https_url(repo)` for github and gitlab.
- **S2** (:64-79) A folder not seeding reads its remote once and answers it with its branch, `main` (DEFAULT_BRANCH)
  where the branch read empty. Seeding: the plan; no remote refuses `no_remote_line(path)` with kind `invalid`; an
  ssh remote of a known git host becomes that host's https url (an https remote stays); the branch is the plan's
  default branch, else its branch, else `main`; the plan rides the answer.
- **S3** (:83-107) git, github and gitlab ask the computer nothing: their remote, `main`, and `project_name_of`. git and
  folder clone with `git clone [--branch b] -- <remote> <dest>`; github and gitlab with the host's own command on the
  owner/repo the remote reads back to (the remote itself where it reads none), never a branch, and carry `cli` with
  the host's bin, setup line and missing line.

## Roads (project-landing.ts)
- **B1** (:613-624) Three roads in order box, provider, mac; another kind is `no project landing road for <kind>`.
- **B2** path (:513, :574, :597): `project_path_on(source, name, home)` with the computer's home on a box (a box with no
  computer refuses `<name> answered no login to run the add as, so nothing can be cloned there`), `/root` at a
  provider, and none on the Mac, where a repo refuses (`<name> is a repo, and that computer holds no checkout ...`).
- **B3** places: a box keeps the memory under the login's home (`under_login_home(home, CLAUDE_CONFIG_DIR)`); a
  provider under the guest's config folder; the Mac under its state home, with the checkout the folder itself or the
  project's path. lands-at-add: box true, the others false. refusal: none on every road. binds: none on every road.
- **B4** remove (:520, :580, :603): one sentence each (fixture `remove`); the box leaves the folder as it is.
- **B5** branch (:539-543, :588, :610): provider is the base, else the default branch, `folder: false`. Mac reads the
  folder here; a box runs `git -C <path> symbolic-ref --quiet --short HEAD` with INLINE_EXEC_MS and answers its trimmed
  output; a failed, empty or thrown read, or no computer, is `branch: null`; both `folder: true`.
- **B6** box `folder_stands` (:523-527): `test -e '<path>'` with STEP_MS; exit 0 true, 1 false, above 1 an error of
  stderr's last line or `reading <path> on <computer> exited <n>`. `seed_into` (:528-534): lands the archive at
  `<home>/.wsp/put/seed-<id>.tgz`, then runs `seed_into_script`; a failure is stderr's or stdout's last line, else
  `putting the kept files into <path> exited <n>`. `unsaved` (:535-538): none without a checkout, else `read_unsaved`
  of the checkout named `project_folder_named(name, checkout)`. Provider and Mac have none of the three.
- **B7** box `land` (:545-566): a folder source with no seed answers `{git: {top: path}}` and runs nothing. Else
  `free_folder_script(home, name)` (no folder: stderr's last line, else `no folder for <name> in <home>: exit <n>`);
  the memory key is the folder's Claude key and the memory dir under the login's home; then clone, seed and install
  with scratch `<home>/.wsp/put` and the install log `place_install_log(home, id)`, the clone reading the vault's
  clone env on its input (`with_env_from_input`, the env as `env_input`) where it holds any, with no setup-git line.
  The answer adds `path, memoryKey, memoryDir, git: {top}`. Any failure runs `rm -rf -- '<folder>'` with STEP_MS; a
  sweep that fails warns `<folder> on <computer> was not swept after the add of <name> failed: <why>`; the add's own
  error is the answer.
- **B8** clone, seed, install (:356-438): land the seed archive at `<scratch>/seed-<id>.tgz`; report `cloning`
  (`Cloning <remote> into <holds>.`); report `seeding` (`seeding_line`) where seeded; the clone script with CLONE_MS
  (600,000), at the project's base branch; a failed clone is stderr's or stdout's last line, else `the clone exited
  <n>`. The patch step (B9); the seed's rest with STEP_MS and memory where chosen and planned (a failure: last line,
  else `landing the seed exited <n>`); `wsp-memory-kept` reports `seed_memory_kept_line(computer)`. `seeded` records
  the files ticked, `seed_bytes`, memory `none`/`kept`/`landed` with its files, the commits git counted and `at` as
  ISO. Then `ls -A '<checkout>'`, the installs `project_installs` names in order, each reported `installing` (`<command>
  in <checkout>.`) and run with INSTALL_MS; a failed install is `<command> in <checkout>: <last line>; its whole output
  is <log> on <project.computer>`. `installed` joins rows with `, ` and commands with `; `, seconds rounded.
- **B9** patch (:444-481): only where commits were chosen and the plan has unpushed ones. The branch to go back to is
  `symbolic-ref --short HEAD`'s last line, else the base, else the plan's default branch, else its branch. A patch that
  exits 0 with an integer last line reports `seed_commits_landed_line(n)` and counts n. Otherwise the cleanup runs (its
  exit never read) and `seed_commits_lost_line(...)` is reported and becomes the land's `notice`; commits 0.
- **B10** provider and Mac `land` answer `{}`.

## Decisions
1. Optional TS members (`folderStands?`, `seedInto?`, `unsaved?`) answer `None` where the road has none.
2. `path` and `places` answer `Result`, since the TS throws from them on a box with no computer and a repo on the Mac.
3. wsp-machines' create area carries its own `LandingRoad` stand-in trait (`deps: &serde_json::Value`); its join
   switches to `ProjectLanding` and `LandingDeps`.

## After the port
- B8 names `project.computer`, the computer's id, in the install failure's sentence (`... on p`); the module's own rule
  says no sentence a person reads may name the id. `deps.computer_name` would be the person's word.
- B2 joins the home and the name without trimming the home: a home `/home/dev/` gives `/home/dev//lab`.

## Tests
| Rule | Test |
|---|---|
| S1 to S3 | the ten tests of `tests/project_sources.rs`, one per fixture key |
| B1 | `the_roads_are_box_provider_and_mac_and_any_other_kind_is_a_wiring_fault` |
| B2 | `each_road_names_the_folder_a_project_takes_on_its_computer` |
| B3, B4 | `each_road_keeps_the_memory_where_its_computer_reads_it`, `what_each_road_does_at_the_add_at_a_remove_and_for_every_workspace` |
| B5 | `each_road_reads_the_branch_a_new_thread_starts_on` |
| B6 | `the_box_road_reads_whether_the_folder_stands_*`, `the_box_road_puts_a_seeds_files_*`, `the_box_road_reads_what_a_checkout_holds_unpushed_*` |
| B7, B8 | `the_box_road_claims_a_folder_clones_and_installs_*`, `writes_the_installs_log_under_a_path_a_leave_takes_*`, `the_box_road_clones_with_the_vaults_env_on_the_clones_input_*`, `a_clone_or_an_install_that_fails_sweeps_the_folder_*`, `a_sweep_that_fails_is_warned_*` |
| B8, B9 | `the_box_road_lands_a_seed_its_commits_and_keeps_the_memory_already_there`, `a_patch_git_refuses_is_put_back_and_said_once_and_the_add_still_lands` |
| B7, B10 | `a_folder_already_on_the_computer_is_kept_where_it_stands_and_the_other_roads_land_nothing` |
