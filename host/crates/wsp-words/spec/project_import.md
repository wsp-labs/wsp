# project_import
Source: the functions of `packages/protocol/src/project-import.ts` (76 lines) at a3b870a6e; its request shape (`ProjectImportRequest`) is `wsp_wire::types::project_import`, and the plan it reads (`ProjectPlan`, `ProjectSecret`, `ProjectAgent`) is `wsp_wire::views::project_bundle`. Tests: `packages/protocol/test/project-import.test.ts` (2 of 3 cases ported in `tests/project_import.rs`). Skipped: :51 (reads the host's source files to hold `importRequest` to one caller: a law test, not a unit test).

## What it does
What every client works out from a project plan before an import: which secret-shaped files and which agent's sessions start ticked, when a person's answers count as consent, the one `project.import` request the ticks become, and the words a secret-shaped row says for its tick.

## Behaviour
1. `default_consent(secrets)`: the paths of the secrets that carry a `rewrite`, in the plan's order (project-import.ts:10-12). A file that would travel as it is never starts ticked.
2. `consent_request(secrets, ticked)`: walks the plan's secrets in order and keeps the ones whose path is ticked; `carry` is those with no rewrite, `rewrite` those with one (:15-21). A ticked path the plan does not list is dropped. Both lists are present even when empty.
3. `can_travel(agent)`: no `error` and `sessions > 0` (:24). `default_agents(agents)`: the id of each agent that can travel, in the plan's order (:27-29).
4. `agents_request(agents, ticked)`: the plan's agent ids that are ticked, in the plan's order, not the order of the ticks; None where none is ticked (:32-35). A ticked id the plan does not list is dropped.
5. `import_consented(answers)`: true for `yes` exactly true, or a non-empty `keep` or `cut`; `yes` false or absent with both lists empty is the dry run (:45-47).
6. `import_request(plan, source, ticked, agents, replace)`: `source` as given, `dest` is `plan.source`, then `carry` and `rewrite` from rule 2, `agents` only where rule 4 answers a list, and `replace: true` only where `replace` is true; false and None both leave it out (:62-65). Key order: source, dest, carry, rewrite, agents, replace.
7. `secret_offer(secret, ticked)` (:69-76), in order: unticked is "left out" for both forms; no rewrite is "travels as is" for both; else short is "rewritten without keys", and full adds "; the remote reads <urls joined ", ">" where there are urls, then "; <drop joined ", "> left out" where there are dropped keys.

## Edge cases
- `import_request(plan, "/Users/me/code/proj/", {.env, .git/config}, {claude}, Some(true))` keeps the trailing slash on `source` and takes `dest` "/Users/me/code/proj" from the plan (test).
- Nothing ticked and `replace` false: `{ source, dest, carry: [], rewrite: [] }` with no `agents` and no `replace` key (test).
- Agents ticked as {codex, claude} come out ["claude", "codex"], the plan's order (test).
- `import_consented` with `yes` false and empty lists is false (test).

## Behaviour nobody wrote down
- A JS `Set` iterates in insertion order and so does `IndexSet`; the answers here walk the plan's lists, so the order of a ticked set never reaches an answer.

## Tapes
The `project.import` request `wsp init` sends for its first import (packages/host/src/init-first.ts:198); the plan lines and the `--keep`, `--cut` and agent choices of the import verb (packages/host/src/verbs/io.ts:146-170).

## Decisions
1. A `ReadonlySet<string>` is an `IndexSet<String>`, in and out.
2. The TS answers `{ carry, rewrite }` and `{ short, full }` are the structs `ConsentRequest` and `SecretOffer`; `ImportAnswers` is a struct with `yes: Option<bool>`.
3. The optional `replace?: boolean` is `Option<bool>`; only `Some(true)` writes `replace`.

## After the port
- none.

## Rules pinned
| Rule | Test |
|---|---|
| 1 | none: no test calls `default_consent` |
| 2 | `import_request::is_the_one_request_every_client_sends_the_source_as_given_dest_from_the_plan_car` |
| 3 | none: no test calls `can_travel` or `default_agents` |
| 4 | `import_request::is_the_one_request_every_client_sends_the_source_as_given_dest_from_the_plan_car` |
| 5 | `import_consented::is_yes_or_an_answer_to_a_secret_shaped_row_nothing_given_is_the_dry_run` |
| 6 | `import_request::is_the_one_request_every_client_sends_the_source_as_given_dest_from_the_plan_car` |
| 7 | none: no test calls `secret_offer` |
