# agents-set: skills, defaults and setup (agents-help.ts:300-573)

Rust: `src/verbs/agents_help/set.rs`. Tests: `tests/cli_c/agents_set.rs` (374 recorded cases, generated), `drivers_agents_set.rs`,
`agents_set_rules.rs` (hand tests). Probe: `tests/fixtures/cli_c/record/agents_set.probe.ts`. Citations are `agents-help.ts:<line>`.

## Scope

The skills.sh search and the skills verbs' calls and lines (300-368), the three `--reset` word lists and their two private helpers
(370-384), `agentDefaultsSet` and its lines (386-446), `envNameOf` and the setup of an agent (448-503), `projectDefaultsSet`
and its sentences (505-549), the words of a resolved default (551-567), and the plain text of the tools (570-573).
Not here: `agentsTarget` and `ProjectAsked` (read.rs), `preferencesOf` and `projectDefaultsOf` (turns picks), `projectOf` (ws rows).

## Behaviour

- `searchSkillsSh` (301): `skills.search {q, limit?}`. `limit` is left out when absent and sent when 0. `q` goes as given, an empty one too.
- `skillHitLines` (308): the empty list is one line; else `table` of SKILL, INSTALLS, ADD WITH, name and id through `cell`.
- `skillShown` (318): an id with exactly three `/` parts (`a//c` too) is on skills.sh: with a thread, a computer or a project asked it is
  refused before any frame (315), else `skills.get {skill}`. Any other name goes through `agentsTarget(.., project.name)` then
  `skills.preview {target, name, project: true?}`. A project asked but unnamed with `on` is sent as `project: true` and a target with no project.
- `shownText` (329): `printable(text)`; over 65536 bytes (not at it) `\n\n(the first 64 KB of N KB)` with N the size over 1024 rounded up (327).
- `skillAdded` (334): `skills.add {target, skill, agents?, project?}`; `agents` only when non-empty. `addedLine` (343): name is `cell` of the last
  `/` part (empty for a trailing `/`); the copies are `<Agent name>'s <path>` joined by `, `, in the order answered.
- `skillChanged` (354): `skills.remove` or `skills.toggle {target, name, project?, on?}`; `on` goes for either op when `turn` is given.
  Remove reads `removed`, toggle reads `paths`, whatever else the reply holds. `removedLine` (351): `<name> is gone from <folders>.` (no folder: `from .`).
- `resetsOf` (376): each word must be in the verb's list (exact case); the first bad one is refused, quoted as JSON, naming the list; the words come back with duplicates.
  `patched` (382): a value given wins, else null when its reset word is named, else the key is left out.
- `agentDefaultsSet` (405), the hard one. Order: reset words; then `models`: `reset models` is null and reads nothing; else if any of hide, show,
  order, addModel, dropModel is non-empty, `preferences.get` is read once and the kept picker of that agent merged: hide = kept hide minus show, then hide,
  de-duplicated (so a model both hidden and shown ends hidden); custom = kept custom minus dropModel, then addModel, de-duplicated; order = asked order
  if non-empty (never de-duplicated) else the kept one; only non-empty lists are kept, in the order hide, order, custom; an emptied picker is sent as `{}`.
  Then model, effort, access (`accessWordOf`, a wsp word, refused before any frame), patch key order model, effort, access, models. An empty patch is refused.
  `preferences.set {patch: {agentDefaults: {agent: patch}}}` answers the agent's entry of the reply (`{}` when none).
  `agentDefaultsLine` (433): model, effort, access as stored, then `hides`, `lists .. first`, `adds` for the non-empty lists, joined by `; `; none is `starts on its own defaults`.
- `defaultAgentSet` (441): `preferences.set {patch: {defaultAgent}}`, the reply not read; answers `{defaultAgent}`.
- `envNameOf` (452): `^[A-Za-z_][A-Za-z0-9_]*$` or a usage refusal opening with the line's own name (`named`) and quoting the name as JSON.
- `agentSetupSet` (471), hard. Order: reset words, then `agentsTarget(client, none, on, usage, "wsp agents setup")` (with `on` this sends `places.list`, so the
  later refusals come after it), then each unset name through `envNameOf(.., "--unset-env")`, then the config folder through `absolutePath` (only its first
  character is read: `/`; nothing is resolved, expanded or read from disk), then the empty-change refusal. Change key order: on, program, configDir, args, env.
  `env` is `values` then each unset name as null (a name in both keeps its first place and is null); `values` names are not checked here. Empty `args` is not given.
  `agents.setup {placeId, agent, ..change}`: the target's place id, else `here`. The reply's `agent` read as an `AgentRow`.
  `agentSetupLines` (493): the row's own name; `program`, `config folder`, `launch words` (empty list prints the label and nothing), `variables` (only non-empty);
  the sign-in line (`configDirSignInLine(row.name)` and a full stop) only when `configMoved` and a config folder is set.
- `projectDefaultsSet` (528), hard. Refusals, each before any frame, in this order: blank after-worktree (trimmed); first tracker not `owner/name`
  (`^[^/\s]+/[^/\s]+$`); reset words; access word; empty patch. Patch key order: agent, model, effort, access, afterWorktree, trackers (reset words `after-worktree` and `trackers`).
  The command goes untrimmed; trackers are de-duplicated first-seen, empty is not given. Frames: `projects.resolve {ref}`, `preferences.set {patch: {projectDefaults: {id: patch}}}`,
  `projects.defaults`. No entry for the project id in the defaults is a plain error (no kind). The answer: the resolved project (as the host sent it), its defaults, and
  `afterWorktree` and `trackers` of the set reply's `projectDefaults[id]` where present (an empty list is kept).
- `threadDefaultsLines` (560): the agent (shown by `agentName`), then model, effort, access when present, `label value: from word`; access shows its value, not its mode.
  `defaultsCell` (566): `-`, or agent, model, access (not effort) joined by a space, ` (project)` after those from the project.
- Sentences and words (305-573): each is one function or const in set.rs; `PICKER_WORDS` is three functions, `SETUP_WORDS` and `DEFAULTS_LABELS` are structs, `FROM_WORDS` is `from_word`.

Declared by the tool module (not written here), with their text: `SkillNameIn` "the skill's name, as skills lists it"; `SkillProjectIn` (boolean or text,
optional) "the project's skill of that name rather than the one that is not a project's: true with a thread, the thread's own project, or the project's name, as projects lists it, with on";
`AgentsThreadIn` (text, optional) "a thread, by its id or a prefix of it, whose computer and project to read; absent reads a computer"; `AgentsOnIn` (text, optional)
"the computer to read, by the name computers lists; absent with no thread is the computer the app runs on". The plain strings `SKILL_CHANGE_WORDS`, `AGENTS_ON_WORDS`,
`AGENTS_READ_WORDS` are in set.rs and pinned by `const_text` cases.

## Limits and caps

`SKILL_PREVIEW_BYTES` 65536: sizes 65535, 65536 (no cut line), 65537, 66559, 66560 (65 KB), 66561 (66 KB), 70000, 10 000 000 (`shownText` cases). Skills.sh id: 1, 2, 3, 4 parts.
Table width of `skillHitLines` counts UTF-16 units (an emoji is two). Trackers: ten words for the shape. Env names: eleven bad and seven good words.

## Null and empty inputs

Empty hits, empty query, empty skill name, empty agents list, empty `args`, empty trackers (alone is "nothing to change"; with `reset trackers` null), empty lists of the picker
(alone is nothing to change, with a model reads no preferences), empty model/program/agent string (sent as given), empty config folder (refused), empty `removed`,
no setup on a row, no preferences entry for the agent, defaults for another project only.

## Order

Frames: `agentDefaultsSet`: preferences.get (only for a list), preferences.set. `agentSetupSet`: places.list (with `on`), agents.setup. `projectDefaultsSet`: projects.resolve,
preferences.set, projects.defaults. `skillShown`/`skillAdded`/`skillChanged`: the target's frames (sessions.list and workspaces.get for a thread, places.list for `on`), then the op.
Refusal order is above. Lines: `agentDefaultsLine` words model, effort, access, hide, order, custom; `agentSetupLines` on, program, config, launch words, variables, sign-in.

## Invariants held

Host invariant 7 (no cloud word with the cloud off): `invariant_7_no_word_or_sentence_of_this_unit_names_a_cloud`, and the cloud-on twin of a case in each group. The guest rule: no
helper here reads a folder; the config folder is checked by its first character and sent as given: `guest_rule_a_relative_config_folder_is_refused_before_any_frame_or_path`,
`guest_rule_an_absolute_config_folder_is_sent_as_given_never_resolved`, `guest_rule_a_skills_sh_id_names_no_computer_and_is_refused_before_any_frame`.

## What the TS leaves unclear

- `addedLine` with a skill ending in `/` or empty prints ` is in <path>.` (an empty name): no check.
- `projectDefaultsSet` answers the `project` of `projects.resolve` unparsed, so a key the host adds rides along; the Rust ProjectView drops unknown keys.
- A model both hidden and shown in one ask ends hidden (hide is appended after the filter); an emptied picker is sent as `models: {}`, not null.
- `agentSetupSet` does not check the names of `values`; the host does. A thread naming nothing reads projects (and with the cloud, workspaces twice) before it fails.
- `--reset` words are exact case; `afterWorktree` is refused, only `after-worktree` is a word.

## Rule -> test

| Rule | Tests |
|---|---|
| frames and value of every helper, null, empty, caps, order | the 374 recorded cases in `agents_set.rs` (by helper: `search_skills_sh__`, `skill_shown__`, `skill_added__`, `skill_changed__`, `agent_defaults_set__`, `agent_setup_set__`, `project_defaults_set__`, and the pure helpers) |
| plain text, word tables and reset lists | `const_text__*`, `sentence__*` |
| cap 65536 under, at, over; KB rounding up | `shown_text__size_*`, `shown_text_cuts_the_line_over_the_cap_only_and_rounds_the_size_up` |
| skills.sh id has three parts | `skill_shown__*`, `skills_sh_id_has_exactly_three_parts` |
| nothing sent before the patch is whole | `agent_defaults_set_sends_nothing_before_the_patch_is_whole`, `project_defaults_set_refuses_in_a_fixed_order_before_any_frame`, `agent_setup_set_refuses_a_name_a_shell_would_not_read_before_any_frame` |
| picker merge: kept lists, de-dupe, order replaced or kept, models null | `agent_defaults_set_merges_the_picker_over_what_stands`, `..with_nothing_standing..`, `agent_defaults_set_reset_models_is_null_and_reads_nothing`, `agent_defaults_set__*` cases |
| each reset word | `agent_defaults_set_each_reset_word..`, `project_defaults_set_each_reset_word..`, `agent_setup_set_each_reset_word..` |
| access words | `agent_defaults_set_access_words_go_to_the_host_as_wsp_words` |
| patch key order | `agent_defaults_set_the_patch_keeps..`, `project_defaults_set_the_patch_keeps..`, `agent_setup_set_sends_every_field_in_the_changes_own_order` |
| project frames order, missing defaults, echoed fields | `project_defaults_set_asks_resolve_then_set_then_defaults`, `..answers_a_plain_error..`, `..stops_at_the_first_frame_the_host_refuses`, `project_defaults_set__*` cases |
| setup: env values then null, computer lookup first, here id | `agent_setup_set_env_is_the_values..`, `agent_setup_set_looks_up_the_computer_before_its_own_refusals`, `agent_setup_set_without_a_computer..` |
| remove reads removed, toggle reads paths | `skill_changed_remove_reads_removed_and_toggle_reads_paths` |
| TS tests ported: skills-sh.test.ts, agents-cloud.test.ts, contract.test.ts | `hostile_text_from_skills_sh..`, `folders_printed_by_added_and_removed..`, `project_defaults_set_trackers_keep_the_whole_list..`, `project_defaults_set_a_blank_command_is_blank..` |
| invariant 7 and the guest rule | the four tests named above |
