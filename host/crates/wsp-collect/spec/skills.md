# skills
Source: `skills.ts` (286 lines) at a3b870a6e. Tests: `test/skills.test.ts` (14 cases), `test/skills-script.test.ts`
(3 cases, each run under `sh` and `/bin/dash` where present). Rust: `tests/detect_skills.rs`. Most cases run on the
real filesystem through `live_host::node_host()` and the catalog's agents: red until both exist.

## What it does
Lists the folders every catalog agent loads skills from (user folders, each project's folders, each plugin's skill
folders), then reads every skill under them with one `sh` run of a find-and-awk script, and turns the records into one
`SkillRow` per folder name, scope and project or plugin, with every folder the skill lives in.

## Behaviour
1. The shell texts (FRONTMATTERS, SCRIPT, SKILL_LINK_ABOVE, LINKED_SCRIPT) are constants generated from the TS by
   evaluating its own definitions (skills.ts:41-101, 159-184); the port runs them as they are.
2. `skill_roots` (skills.ts:211-233) builds an ordered map by dir. Adding a dir not yet there inserts `{dir, scope,
   agent (only when own), project (when given)}`; adding one already there with `own` sets its agent only when it has
   none, keeping its place. First, for each agent in order, its `skill_roots.user` dirs through
   `in_store(state_home, dir, home, stores[agent])`, own being index 0, scope user.
3. Then each project: dirs are `posix_join(project.path, root.dir)` over every agent's `skill_roots.project`,
   repeats dropped. `posix.join` normalizes `.`, `..` and repeated slashes but keeps a trailing slash, so
   `./.agents/skills/` and `.agents/skills` are two roots. One link check runs `sh ["-c", LINKED_SCRIPT, "sh", project.path,
   ...dirs]` with `timeout_ms: 20000`. Then every agent's project dirs are added (own is index 0, scope project, with
   the project). Then each dir that the check names, or every dir when the check gave nothing, gets `skipped` set to
   the line in rule 5, keeping its place. Projects run at once; their dirs go in in project order.
4. The link check gives nothing when the run is `None` or its `js_trim_end` does not end with `\u{1e}END`; otherwise
   each `\n` line split on `\t` into dir, link, to (missing parts `""`), kept when dir is one of the dirs asked.
5. Skipped lines: no check: `skills: <~dir> was not read, since whether it links out of the repo could not be told`;
   named: `skills: <~dir> links out of the repo<" through <~link>" unless link is the dir itself>, to <to>, so its
   skills are not read` (skills.ts:202-206), `~dir` being `tilde(home, dir)`.
6. Last, each plugin read's skill dirs: for each agent's read in order, each found plugin with `skills` and without
   `missing`, each of its dirs as `{dir, scope: plugin, agent, plugin: {id, prefix, on}}`, added only when the dir is
   not in the map yet (skills.ts:231,236-240).
7. `detect_skills(host, all)` (skills.ts:245-286): `outside` is every root's `skipped` line in order. No unskipped
   root: `{skills: [], refused: outside}` and nothing runs. Else run `sh ["-c", SCRIPT, "sh", ...dirs]` with
   `timeout_ms: 20000`. `None`: refused is `skills: the folders could not be read` then `outside`, no skills. An
   answer whose `js_trim_end` does not end with `\u{1e}END` adds `skills: the answer was cut short, so the list is not
   whole` before `outside`, and its records are still read.
8. Records: split on `\u{1e}`, the first piece dropped; each split on `\u{1f}` into root dir, dir, link, off, head; a
   record with an unknown root (the last root with that dir wins), no dir or no head is skipped; so is a dir not under
   `<root>/` or with a segment below the root starting with `.`.
9. A record whose dir sits under another found dir of the same root (any proper prefix of its path below the root) is
   that skill's own file and is skipped (skills.ts:266-267).
10. A skill's name is its folder's basename, or `<plugin prefix>:<basename>` under a plugin root. Rows are keyed by
    scope, then the project's id or the plugin's id or `""`, then name. Its path is `{path: tilde(dir), agent: the
    root's agent, link_to: tilde(posix_resolve(dirname(dir), link)) when link is not "", off: true when the record says
    off or the root's plugin is off}`. A new key makes a row `{name, description: frontmatter's description when set,
    paths: [path], scope, project: the root's project with its path tilde'd, plugin: the plugin's id}`; a known key
    adds the path unless a path with the same `path` is there.
11. Rows sort by name, then scope, then project name, each with `collate` (localeCompare), stably.
12. `skill_frontmatter(text)` (skills.ts:125-143): lines split on `\n`; a line matching KEY names `name` or
    `description`; every `name` line counts. Its value is group 4 `js_trim`med or `""`; following lines matching
    `^\s+\S` are taken as continuation, each `js_trim`med. A value matching `^[>|][-+]?$` is a block: the continuation
    lines joined by a space. Else `scalar(value)` then the continuation lines, empties dropped, joined by a space. The
    first non-empty value of each key is kept. `names` is set only when more than one name line was seen.
13. `scalar(v)` (skills.ts:104-111): `js_trim`; `^'((?:[^']|'')*)'` gives its group with `''` as `'`;
    `^"((?:[^"\\]|\\.)*)"` gives its group with `\"` and `\\` unescaped (no other escape); else the text less a
    `(^|\s+)#.*$` comment.
14. `skill_md_frontmatter(text)` (skills.ts:147-152): the first SKILL_HEAD_BYTES UTF-16 units, split on `\r?\n`; the
    first line must be exactly `---`, else empty; the lines up to the next `---` line (or the end) go to
    `skill_frontmatter`.

## Edge cases
- `skill_frontmatter("")` is all `None`. A `description: >` with no continuation is empty, so not set.
- A record line for a root dir given twice is read once per record, and its row's paths do not repeat.
- An `all` holding only skipped roots runs nothing.

## Decisions
1. TS `skillRoots(host, {agents, projects, plugins})` takes an options object; Rust takes the three as arguments.
2. `posix.join` and `posix.resolve` are written out in Rust as node defines them: join, then normalize `.`, `..` and
   repeated slashes, keeping a trailing slash (join) or dropping it (resolve).

## After the port
- Real TS link checks finish in any order, so the order of a project's dirs among several projects is the order the
  checks returned; the Rust port fixes it to project order.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/skills.json` (`oracle/w2-skills.oracle.ts`, every value printed by the TS),
replayed by `tests/detect_skills.rs` `w2_oracle::*` (cases with a `tree` on a real folder).

Corrections to the rules above, found against the TS (each overrides the rule it names):

- C1. Rule 8: `head` is the fifth piece only; pieces after it are dropped, so a front matter line holding U+001F is cut there; a record of exactly four pieces is skipped.
- C2. Rule 12: the KEY regex needs whitespace or the line end after the colon, and its `.` stops at `\r`: `name:pdf` and any `\r`-ended line are not keys.
- C3. Rule 13: the quoted forms are prefix matches: `name: "a" b` is `a`, `name: 'a'b'` is `a`.
- C4. Rule 14: SKILL_HEAD_BYTES counts UTF-16 units only in `skill_md_frontmatter`; the scan's awk cap counts bytes (the `---` line and each newline included), and its stdout is decoded with replacement, so 3000 `é` read through `detect_skills` keeps 2039 `é` and a U+FFFD.
- C5. A root with a trailing slash reads nothing on a real filesystem; a project at the home folder can mark user roots skipped; the link check runs even with no dirs.

- W1. Front matter over 51 texts and the SKILL.md head over 14 (`skill_frontmatter`, `skill_md_frontmatter`; the lone-surrogate case is not compared).
- W2. Roots for agents and projects, the link-check line and its answers (`skill_roots`, plugin roots not recorded).
- W3. Skills read from canned answers and from real trees: links, categories, odd front matter, `.off`, the byte cap (`detect_skills`).

## Tests
| Rule | Test |
|---|---|
| 1 | the three `skills-script` cases (real shells) |
| 2, 6 | `lists_every_skill_once_by_name_with_every_folder_it_lives_in`, `reads_the_agents_own_skill_folders_and_claude_codes_plugin_index_under_the_folder_each_store_names`, `skill_roots_gives_a_shared_folder_the_agent_whose_own_it_is` |
| 3, 4, 5 | `a_workspace_adds_its_projects_own_folders_as_project_skills`, `a_computers_projects_each_add_their_folders`, `a_project_whose_links_could_not_be_told_is_not_read`, `a_project_whose_folder_is_not_on_the_computer_reads_no_skills_there`, `skill_roots_marks_a_project_folder_that_links_out_of_the_repo` |
| 7 | `an_answer_cut_short_is_a_refusal_and_a_read_that_failed_is_one`, `detect_skills_with_only_skipped_roots_runs_nothing`, `an_answer_with_no_end_mark_is_cut_short_and_its_stray_piece_is_no_record` |
| 8, 9 | `leaves_out_dot_folders_folders_with_no_skill_md_a_skills_own_inner_examples`, `detect_skills_reads_records_off_a_canned_answer` |
| 10, 11 | `reads_a_plugins_skills_off_while_settings_switch_its_plugin_off`, `reads_a_folder_whose_skill_md_was_renamed_skill_md_off`, `keys_a_skill_by_its_folders_name`, `detect_skills_sorts_rows_by_name_scope_then_project_with_collate` |
| 12, 13 | `reads_a_frontmatters_plain_quoted_folded_and_continued_values`, `reads_the_name_key_however_yaml_spells_it_and_nothing_past_the_closing` |
| 14 | `reads_a_skill_md_saved_with_crlf_line_ends_as_the_node_reader_reads_it`, `skill_md_frontmatter_reads_only_the_first_4096_units` |
