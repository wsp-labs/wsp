# projects_move

Source: `packages/runtime/src/projects/projects.ts:23-106, 482-574` at 65f6b89c7. Tests: the import and export cases of
`packages/runtime/test/` (named in the table). Every report line is the TS's own text, built in projects.ts with
`plural` and `fmtBytes` (protocol/src/format.ts); the tests write each line out whole.

## What it does

Moving a folder of this computer's onto a workspace's machine (`import`, through the kind's import road, of which
`copyImport` is the fork's) and a folder of a machine back here (`export`), each saying its stages on the stream every
client watches.

## Behaviour: `copy_import(entry, o, report)` (:25-106)

1. `plan` = `o.bundler.plan()`; report `planned` `<files>, <bytes>[ and the repository]; <n secret-shaped files>; <n
   caches> left behind.` (`plural`, `fmtBytes`).
2. `carry`, `rewrite` = the asked paths; `rewriting` = each plan secret with a `rewrite` whose path is asked, as
   `{path, ...rewrite}`; `provider` = the computer row of the record's place (or its provider) is a provider place.
3. `held` = `[]` where a secret travels to that computer (`seedTravels({provider}, {kind: "secret"})`), else
   `plan.values ?? []`; `carried` = secrets carried, not rewritten, not held; `cut` = sorted unique (secrets neither
   carried nor rewritten, then held).
4. Clauses: `carrying <a, b>`; `rewriting <path>[ to <urls>][ without <drop>]` joined by `, `; `no secret-shaped file
   travels` when neither; `nothing cut` or `cut <a, b>`; joined `; `.
5. Agents: `travelling` = readable plan agents named in `o.agents`; notes: none when the plan has no agents; else `No
   agent sessions travel` or `Sessions travel for <Name> (<n sessions>), ...`; then `<names> stays|stay` where some
   travel and some stay; then `<Name> could not be read (<error>)` per unreadable one.
6. Report `consented`: `No secret-shaped files.` when the plan has no secrets and nothing is cut, else the clauses with
   the first letter upper-cased and a full stop; then ` <notes joined "; ">.` where any.
7. Report `packing` `Packing <files - cut.length files>.`; `packed` = `bundler.pack(carry, rewrite, provider)`.
8. With travelling agents: `present` = `agentsOnMachine(machine, ids)`; each needs a catalog home (else no kind `<id>
   is not an agent the catalog knows`); `state` = `bundler.packState({dest, agents: [{agent, home, present}]})`.
9. Report `uploading` `Uploading <bytes>.` with `{bytes: 0, total}`; `landBundle(machine, tar, dest, {replace?, tmpDir:
   scratch(entry), timeoutMs: 600000, onPart, onLanding})`: each part reports `uploading` `Part <p> of <n>, <bytes> of
   <total>.` with progress; landing reports `landing` `Landing at <dest>.`.
10. With a state holding files or merges: report `uploading` `Uploading <n session files>[ and ]<the rows to merge>,
    <bytes>.`; `importInto(machine, state tar, "/", {overlay: true, ...})` with the same part reports; merges report
    `landing` `Merging rows into <names>.` and each runs `mergeOnMachine` over that agent's result; report `landing`
    `Landing sessions: <Name outcome, ...>.`.
11. Answers `{result: {dest, files, bytes, parts, cut, rewritten, agents, project: projectOf(dest, bytes)}, done:
    "<files>, <bytes>, landed at <dest>[ in <parts> parts][; sessions: <outcomes>]."}`.

## Behaviour: `import(o, origin)` (:482-517)

`spawnGuard("import")`; `entryOf(workspaceId, origin)`; `actionRefusal(state, "import", gone, name)` thrown as no kind;
`copyBlocked(entry)`; reports emit `{type: "project.import", workspaceId, source, dest, stage, message, elapsedMs,
...progress}`. The kind's import; `dest` added to the record's `landed` (unique, order kept) before
`writeDaemonRoots(entry, true)`; a refused roots write takes this `dest` back alone (unless it was there before), and
`landed` empty is removed, then the refusal propagates; `persist`; report `done` with `done`; answer `result`. Any
failure after the copy-block check reports `failed` with its message.

## Behaviour: `export(o, origin)` (:518-573)

`spawnGuard("export")`; entry; action refusal; `copyBlocked`; reports emit `project.export` likewise. A scratch folder
`<tmp>/wsp-exported-*`; `onMachine` = `guestTmpPath("wsp-state")`; `listing` = `stateListing(homes, source, onMachine,
agents)`; `lander.probe(dest)` standing and not `replace`: `destExists(dest, files)`; report `packing` `Packing <source>
on the machine.`; `exportFolder` with `downloading` reports `The folder: <bytes> of <total>.`; a non-empty listing runs
on the machine with `LISTING_DEADLINE_MS` (a non-zero exit: no kind `could not look for agent state on the machine:
<last 200 of stderr>`); found paths: report `packing` `Packing the agents' state for it on the machine.` and
`exportPathsInto` two groups (`/` for paths outside `onMachine`, `onMachine` for those under it, empty groups dropped)
with `Agent state: ...` reports; report `landing` `Landing at <dest>.`; `lander.land({source, dest, replace, archive,
state?, unread?})`; report `done` `<files>, <bytes>, landed at <dest>[; <n caches> left behind]; <no agent sessions
for it on the machine | sessions: <outcomes>>.`; answer `{dest, files, bytes, excluded, agents (less name)}`. A failure
reports `failed`. Always: where a listing ran, `rm -rf <onMachine quoted>` on the machine (failure ignored), and the
scratch folder removed.

## Behaviour nobody wrote down

- Rule 8.3: `stderr.slice(-200)` counts UTF-16 units.
- Rule 3.15: `elapsedMs` off the clock.

## After the port

- `import` marks `landed` before the roots write, so a concurrent write lists a folder that may be taken back (:500). Kept.

## Rules against tests

| Rule | Tests |
|---|---|
| copy 1-7 | `copy_import_reports_each_stage_as_the_ts_words_it` (every oracle scene's reports whole), `copy_import_holds_values_back_from_a_provider` |
| copy 8-11 | `copy_import_lands_sessions_and_merges_rows`, `copy_import_refuses_an_agent_the_catalog_lacks` |
| import | `import_adds_the_dest_to_landed_before_the_roots_write`, `a_refused_roots_write_takes_back_this_imports_dest_alone`, `import_reports_failed_on_any_failure`, `import_refuses_a_gone_workspace` |
| export | `export_refuses_a_standing_dest_unless_replace`, `export_lands_folder_and_state_and_says_done`, `export_cleans_the_machine_and_the_scratch_on_failure` |
