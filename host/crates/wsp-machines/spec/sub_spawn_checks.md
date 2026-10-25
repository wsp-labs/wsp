# sub_spawn_checks

Source: `packages/runtime/src/machines/sub-spawn.ts:25-30, 76-281, 343-412` (about 330 of 622 lines) at `65f6b89c7`.
Tests: `packages/runtime/test/sub-spawn.test.ts` (every case but the machine faults and the fork's own lines: 41 with
the 24 rows of `every check refuses before Boat is asked`), `sub-start-events.test.ts:20`. Rust:
`src/machines/sub_spawn/checks.rs` (`SubSpawnArea::spawn_sub` and the checks it reads; the area struct is declared
in `src/machines/sub_spawn.rs`).

## What it does

`spawn_sub` makes a sub-thread's record on a cloud, under the asking thread (its lead). Every check that can refuse
runs first, in a fixed order (S1 to S13), so nothing is asked of the cloud for a spawn that is refused; then the
record is written `running` with no machine and handed to the queue (`enqueue`), which makes the machine once the
thread's first turn arrives. It answers the record's view, a notice for what did not travel, and the preamble the
sub-thread opens with. The ref and picked-file reads it uses (`from_ref`, `free_branch`, `seed_of`) live here too;
`sub_spawn_machine` reuses `seed_of`, `sold_by`, `image_at`, `agent_on`, `starts_left` and `provider_at`.

## Behaviour

1. Constants (:26-30): `SETUP_IMAGE = "default"`; `HOST_WORDS`: github `GitHub`, gitlab `GitLab`; `LOGIN_HELD`:
   `copied`, `signed-in`.
2. `spawn_sub(opts, origin)` takes `began = now` and `name = name_given(opts.name)` first, then, in order, each a
   refusal before any cloud call:
   - S1 (:142-143): no `scope_of(origin)` refuses `sub_no_thread_refusal(ask.on)`.
   - S2 (:146-152): `spawn_guard("fork", origin)` takes the place; an error from it is re-thrown with kind `usage`
     when it carries none. Everything after runs under a guard that frees the place on every exit (TS `finally`).
   - S3 (:156-162): the provider rows of `computer_rows()`; the row `names_place(row, ask.on)`; its backend is
     `provider_at(id)` = `places.backend(id)`, else the place door's `backend_of(id)`. No row or no backend refuses
     `sub_no_cloud_refusal(ask.on, <provider_key_name of every provider row that has a backend, in row order>)`.
     `on = provider_key_name(row.name)`; `place_id` is `landing_place(row.id).place_id`; `place = place_id ?? row.id`.
   - The lead (:163-167): the live entry of the scope's workspace; a kind that does not `runs_in_folder` refuses
     `sub_lead_on_machine_refusal(on, scope.threadId, lead.name)`. `opts.project` not `""`, not the lead project's id
     and not its name refuses `sub_other_project_refusal(ask.on, opts.project, project.name)`.
   - S4 (:169-172): remote `""` refuses `sub_no_remote_refusal(project.name, on)`; `git_host_of(remote)` none refuses
     `sub_other_host_refusal(project.name, remote_host(remote) ?? remote, on)`. `repo = host.https_url(owner_repo)`.
   - S5 (:175-188): `copy_blocked(lead)`; then one `git.status` frame through the lead's daemon with `cwd =
     checkout_of(lead.record)`. A failure becomes a plain error (no kind, class provider) `branch_unread_refusal(
     lead.name, <message>)`. With no `--from`: `pushed = "origin/" + (defaultBranch ?? "main")`; head `""` or
     `DETACHED_HEAD` refuses `sub_detached_refusal(project.name, pushed)`; no upstream refuses
     `sub_no_upstream_refusal(head, pushed)`; `ahead > 0` refuses `sub_ahead_refusal(head, ahead, pushed)`;
     `countsUnknown` refuses `sub_ahead_unknown_refusal(head, pushed)`. `base` is `{branch: head, sha: oid}`, or
     `from_ref(from, lead, defaultBranch)`. `uncommitted` counts status entries whose `xy` is not `!!`, and is 0 with
     `--from`.
   - S6 (:192-193): `brokered = ctx.brokered({kind: "cloud", place?: place_id})`; `image_at(place, on, host, remote,
     brokered)`.
   - S7 (:195-201): `limits` is the backend's `limits()` where offered (its failure propagates), else none.
     `sold = sold_by(at, limits)`; the size word is `ask.size`, else `sub_size_word(<the place's kept "size" setting>
     ?? pricing.default_size())`; the size sold with the same `cpu` and `memMb` as `sub_size_of(word)`, else refuses
     `sub_size_refusal(on, word, sold)`.
   - S8 (:203-205): `ask.branch` that `sub_branch_name_ok` refuses gives `sub_branch_name_refusal`; equal to the
     lead's head gives `sub_lead_branch_refusal`. `result = ask.branch ?? free_branch(sub_branch_of(name), lead)`.
     `branchOfRecord` (:207) only when `brokered_start(ctx, true)`: `{...repo_of_record(ctx, remote, project.name,
     on), branch: result, startSha: base.sha}`.
   - S10 `place_guard(place)`. S11 (:211-212): `harness = ask.harness ?? default_agent_of(preferences_held ??
     preferences.get(), lead)`; `agent_on(image_record, on, harness, brokered)`. S12 `seed_of(project)`. S13
     `starts_left(limits, name)`.
   - `name_refusal(name)` refuses with kind `conflict` (:218-219).
3. The record (:220-249), keys in this order: `id: "ws_" + <4 random bytes hex>` (the `Ids` trait), `name`, `kind:
   "cloud"`, `machineId: ""`, `phase: "running"`, `golden: head.snapshotId`, `createdAt: <now ISO>`, `project`, `spec:
   {}`, `place?`, `size {cpu, memMb}`, `firstLife: true`, the tree of the scope (`tree_of`), `lead {threadId,
   workspaceId}`, `branchOfRecord?`, `sub {on: on lowercased, repo, base, branch: result, folder: sub_folder_of(
   remote), rateUsdPerHour, sizeName, spawn {startedAt: began, branch {branch, sha, ms: now - began}, uncommitted?
   (only > 0), notSent? (seed's why), log: []}}`.
4. Hand-off (:253-259): `forking.add(name)`, `supersede_failed(name)`, then `enqueue(record, queue_wait(place,
   limits))`, with `forking.remove(name)` after it whether it failed or not.
5. The answer (:260-277): notices in this order: `sub_uncommitted_line(n, project.name, base.branch, base.sha)` for
   `n > 0`, `sub_not_sent_line(why, project.name)`, `sub_stayed_line(stayed)` for a non-empty `stayed`, joined with
   `\n` as `notice`, absent when none. `deleteAfterMinutes = js_round((settings_at(place).deleteAfterMs ??
   DELETE_AFTER_MS) / 60_000)`. `out` keys: `on` (lowercased), `machine` (name), `size {cpu, memMb}`,
   `rateUsdPerHour`, `repo`, `base`, `branch`, `sent {paths, bytes, missing}`, `notSent {uncommitted, why?}`,
   `deleteAfterMinutes`. The preamble is `sub_preamble({on, size, lead: scope.threadId, folder, sent: seed.paths,
   setupGit: host.setupGit, repo, base, branch, deleteAfterMinutes, brokered?: true when a branch of record})`. The
   result is the record's view, then `notice?`, then `sub {out, folder, preamble, harness}`.
6. `sold_by(at, limits)` (:80-83): with limits, `BOX_CLASSES` (in table order) whose type the plan sells, as `{name:
   class.name, cpu, memMb, rateUsdPerHour}`; without, the backend's capability sizes as `{name: s.name ??
   sub_size_word(s), cpu, memMb, rateUsdPerHour: s.rate ?? pricing.rate_usd_per_hour(s)}`.
7. `host_signed_in()` (:93-100): `gh auth token --hostname github.com > /dev/null 2>&1` on the local machine with
   `INLINE_EXEC_MS`; true only for exit 0; any error is false.
8. `image_at(place, on, host, remote, brokered)` (:104-119): `golden_head(copy_of(place, "default"))` none refuses
   `sub_no_image_refusal(on)`; `record = record_of("default")`. Brokered: a host other than github refuses
   `sub_host_other_git_refusal(on, label, owner_repo)`; gh here not signed in refuses
   `sub_host_no_sign_in_refusal(on, owner_repo)`; the image's logins are not read. Otherwise the logins are the
   head's, else the image record's, else none; one whose name is `host.cli` or starts with `<label> CLI`, in state
   `copied` or `signed-in`, must exist, else `sub_no_sign_in_refusal(on, {label, cli, login: "<label> CLI login"},
   owner_repo)`.
9. `agent_on(record, on, harness, brokered)` (:123-128): brokered and `unbrokered_refusal(harness)` refuses it; the
   recipe rows of kind `agent` with `on` are the carried ids; a non-empty list without `harness` refuses
   `sub_no_agent_refusal(on, harness, carried)`.
10. `starts_left(limits, name)` (:131-135): the first of `day` then `hour` with `used >= limit` fails with a plain error
    (class provider) `starts_used_line(window, used, limit, boat_plan_words(trial, planUsd), "sub-thread <name>")`.
11. `free_branch(wanted, lead)` (:347-350): `ls-remote --heads origin` in the lead's folder; its output only on exit 0,
    else (and on an exec error) none; then `free_branch_name(ctx, wanted, heads)`.
12. `git_at_lead(lead, args)` (:354-357): on the local machine when the project's computer's kind `copies_folder`,
    else on the lead's machine; the command is `GIT_TERMINAL_PROMPT=0 ` + `shell_line(["git", "-C", checkout_of(
    lead.record), ...args])` with `INLINE_EXEC_MS`.
13. `remote_read(lead, from, args)` (:360-364): `ls-remote <args>`; an exec error reads as exit -1 with its message as
    stderr; non-zero refuses `sub_from_unread_refusal(from, last_line(stderr) ?? "git ls-remote exited <n>")`.
14. `from_ref(from, lead, preferred)` (:369-388): `name` is `from` less a leading `origin/`. `ls-remote origin <name>
    <name>^{}` mapped `ref -> sha` from tab-split lines with both halves (a later ref replaces). The first of `name`,
    `refs/heads/<name>`, `refs/tags/<name>^{}`, `refs/tags/<name>` held gives `{branch: name, sha}`. Else a name not
    matching `^[0-9a-f]{7,40}$` (any case) refuses `sub_from_not_on_remote_refusal(from, name)`. Else `rev-parse
    --verify --quiet <name>^{commit}`; on exit 0, `for-each-ref --contains <sha> --format=%(refname:lstrip=3)
    refs/remotes/origin`, its non-empty lines other than `HEAD`; none refuses `sub_from_not_on_remote_refusal(from,
    None)`. Those still on the remote (`ls-remote --heads origin`, `refs/heads/` stripped) are kept, in the
    for-each-ref order; none refuses the same. The answer is `preferred` when kept, else the first kept.
15. `seed_of(project)` (:392-412): a project whose computer's kind does not `copies_folder` sends nothing, why
    `sub_picked_elsewhere_why(name, place_name(computer))`. No `seed-choices` row under `folder_named(path)`, or no seed
    wiring, sends nothing, why `sub_nothing_picked_why(name)`. Else the plan for the folder with the homes here, marked
    `provider: true`; travelling rows are those `seed_travels`; `paths` are the picks in pick order that travel;
    `missing` the picks the plan does not list; `stayed` the rest of the picks. No paths answers nothing sent with
    `missing` and `stayed` and no why. Else the pack of `{files: paths, memory: false, commits: false}`; a tar over
    `SUB_SEED_MAX_BYTES` refuses `sub_seed_too_big_refusal(name, fmt_bytes(tar), fmt_bytes(max), <the two largest
    picks by plan bytes, each "path (fmt_bytes)">)` (a stable sort, descending). Else `{tar, files: packed.files,
    paths, bytes: packed.bytes, missing, stayed: [...stayed, ...packed.left]}`.

## Edge cases

- A daemon that cannot read the lead's branch is a provider-class error, not a usage refusal (rule 2, S5).
- `--from origin/feat/later` refuses with the name `feat/later` in the push line (sub-spawn.test.ts:310).
- `ls-remote` matches a name's tail: `release/v1` is not the tag `v1` (:287); only the exact refs of rule 14 count.
- A size equal to the lead's default with no plan read uses the backend's capability sizes (rule 6).
- Exactly `SUB_SEED_MAX_BYTES` is allowed (`>`).
- The place is freed on every refusal after S2 (the guard), so a refused spawn holds no slot.

## Behaviour nobody wrote down

- Rule 3.4: no await between `spawn_guard`'s take and the first check (the guard is the take-and-release kind).
- Rule 4.26: the record's and `out`'s key order are the TS literals'; compare as JSON text.
- Rule 8.8: none here; branch numbering is `free_branch_name`'s (branch-of-record, not this slice).
- Invariant 3 (origin off the token): the lead is the caller's scope, never a field of `opts`.

## Decisions

1. The guard of S2 is a drop guard holding the release closure.
2. `ws_` ids and `createdAt` come from the `Ids` trait and the clock.

## After the port

- S5's daemon failure has no kind, so a person reads a provider-class exit for what is a usage problem.
- S7's `limits()` failure propagates as is, with no line naming the spawn.

## Tests (tests/sub_spawn_checks.rs)

| Rule | Test (TS case) |
|---|---|
| 3, 4, 5 | `writes_the_record_before_the_cloud_and_answers_out_notice_and_preamble` (:71) |
| 15 | `leaves_values_and_keys_here_whatever_the_picks_say` (:153) |
| 15 | `says_why_nothing_travels_where_the_only_pick_is_values` (:164) |
| 15 | `sends_nothing_without_picks_and_names_a_pick_no_longer_there` (:172) |
| 2 S7, 11 | `takes_the_size_the_lead_names_and_numbers_the_default_branch` (:187) |
| 11 | `numbers_the_default_branch_past_one_the_remote_holds` (:202) |
| 2 lead | `takes_the_leads_own_project_and_refuses_another` (:209) |
| 2 S7 | `refuses_a_size_the_plan_does_not_sell_before_any_create` (:226) |
| 14 | `from_starts_at_the_remotes_commit_and_says_nothing_of_uncommitted` (:272) |
| 14 | `from_takes_a_tag_by_its_commit_and_a_commit_by_its_live_branch` (:287, real git) |
| 14 | `from_refuses_a_commit_no_remote_branch_holds_and_a_ref_the_remote_lacks` (:310, real git) |
| 2 S5 | `names_from_beside_the_push_line_where_the_branch_is_not_pushed` (:323) |
| 2 | `every_check_refuses_before_the_cloud_is_asked` (:342, the 24 rows in one table, each asserting message, class, no create, no place held) |
| 7, 8 | `s6_asks_gh_here_in_place_of_the_image_while_the_broker_is_ready` (:396) |
| 8 | `s6_asks_the_image_while_the_broker_is_not_ready` (:402) |
| 15 | `s12_names_the_largest_picks_and_the_sizes` (:408) |
| 2 lead | `refuses_a_lead_on_a_forked_machine_before_any_create` (:492) |
| 15, 5 | `starts_from_a_lead_on_an_added_computer_and_says_no_picks_went` (:505) |
| 12, 11 | `numbers_past_the_remotes_branches_read_in_the_leads_folder_on_its_box` (:536) |
| 4 | `the_record_is_queued_under_the_lead_before_any_machine` (start-events :20) |
| 2 S2 | `a_refusal_after_the_guard_frees_the_place` (new) |
| 6 | `sold_by_reads_the_plan_classes_else_the_backend_sizes` (new) |
