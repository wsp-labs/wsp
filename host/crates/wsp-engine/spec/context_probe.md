# context_probe (machine-context.ts:1-302)

The facts kept on a guest between builds, the probe command and its parse. Rust: `src/machine_context/probe.rs`, and
from `src/golden_seam.rs` `df_kb_cmd` and `parse_versions` (golden-tools.ts:49, golden-base.ts:114), which this unit
implements since golden waits on #2441.

## Paths and agents

- `context_agents()` (:28): the catalog agents whose `context` is set, in catalog order: claude, codex, gemini,
  opencode, pi, hermes (oracle `contextAgents`).
- `guest_roots()` `{ etc: /etc, home: /root }` (:34). `context_path` `<etc>/wsp/machine-context.md`, `skill_path`
  `<etc>/wsp/skills/wsp-machine/SKILL.md`, `facts_path` `<etc>/wsp/machine-context.json` (:43-45).

## The facts (survive on the guest's disk, not in the host)

- `BuildFacts { tools, agents, files }` serializes in that key order; items `{ id, label, note }`, files
  `{ path, note }`. The guest keeps it as `JSON.stringify(facts, null, 2) + "\n"` at `facts_path` (render unit writes
  it); a later build reads it back through the probe's `FACTS` line.
- `merge_facts(prior, result)` (:72): copies of prior's three lists (`None` prior is three empty lists); no result
  answers the copy. `fold(list, rows)`: `missing` is every row whose outcome is not `installed`, as
  `{ id, label, note: note ?? outcome }`; each kept item whose id appears in `rows` is replaced, **in place**, by the
  missing rows of that id (zero or more); then the missing rows whose id is not in `list` are appended in row order.
  Tools fold over `[...base, ...tools]` (base first); agents over the agent rows with `label = name`. Then every
  retired id leaves both lists. Then, where `files` is set: each kept file whose path is skipped again leaves, and every
  skipped row is appended in order (so a re-skipped path moves to the end with its new note).
- `parse_facts(b64)` (:93): base64, UTF-8, JSON; each of the three keys is taken where it is an array and `[]`
  otherwise; any failure is `None`. **Open point 1**: the TS keeps array elements unchecked.

## The probe

- `ALIAS_PROBES` (:161) zsh, bash, fish: the TS strings byte for byte; `alias_probe(name)` is `None` for any other.
- `bounded_command(s, cmd)` (:197): the TS's one line, s in seconds; it fails (non-zero) exactly when the bound killed
  the group (exit 137), and kills the whole group and the timer at the end. `ALIAS_BOUND_S` 8.
- `overlay_probe(fs)` (:206), `/proc/filesystems` by default.
- `probe_command(roots, path, shells)` (:212): the TS script byte for byte (oracle `probeCommand`), defaults
  guest roots, golden-import's `TOOLS_PATH`, `Login`. `None` shells prints `ALIASES unread` and opens no shell.
  The secrets file read is `<etc>` + `/profile.d/wsp-secrets.sh` (the catalog's sh secrets file with its `/etc` cut).
- `parse_probe(stdout)` (:247): lines split on `\n` and `trimEnd`ed; `WSP_CTX` and `WSP_CTX_END` must both be there
  with the end not before the start (first index of each), else `None`. Start: overlay false, has empty, versions
  `parse_versions` of the lines between, agents, secrets, aliases empty, shell `bash`, conflicts empty. Per line, the
  key is up to the first space and the rest is `trim`med:
  `KERNEL` sets kernel when non-empty (a later one wins); `DISK size free` sets disk in bytes (`* 1024`) only when both
  are finite numbers and size above 0; `OVERLAY` is `rest == "yes"`; `HAS` adds a non-empty word; `AGENT` adds a known
  context agent once; `SECRET` adds a name matching `^[A-Za-z_][A-Za-z0-9_]*$` once; `SHELL` sets a non-empty shell;
  `ALIAS name word` needs a space after at least one character (`word` trimmed); `ALIASES unread` is shared, `late` is
  late; `CONFLICT` adds a known agent; `FACTS` sets facts where they parse. Unknown keys and lines outside the markers
  are ignored. Aliases are then sorted by name in UTF-16 code unit order (`B` before `a`).
- `parse_versions` (golden-base.ts:114): `^VERSION ([^:]+): (.*)$` over each line `trimEnd`ed; the first
  `\d+\.\d+(?:\.\d+)?` in the value, rows with none dropped. `df_kb_cmd` (golden-tools.ts:49): columns size 2, used 3,
  free 4, in the order asked; the path written as given, `/root` by default.

## Open points

1. `parse_facts` keeps elements as they are in the TS, a `{"id":"a"}` with no label or note among them, and a render
   then prints `undefined`. `BuildFacts` holds whole items only, so the port cannot hold such an element. Ruled (the
   machines slice's ruling 2, kept by the engine's fix round): a partial element is dropped and the hard test skips that
   one case.
2. fish is not installed on aws-mumbai; `the_fish_alias_probe_*` fails on its own assert until it is.

## Rules against tests

| Rule | Tests |
|---|---|
| parse: markers, each key, sort, empties | `context_probe::parses_every_line_kind_between_the_markers_and_ignores_the_rest`, `context_probe::is_nothing_without_both_markers_and_tolerates_empty_reads`, `context_oracle::parse_probe_reads_every_answer_the_way_the_ts_does` |
| versions on the tools PATH, first line with a number | `context_probe::asks_the_base_floors_commands_for_their_versions_on_the_tools_path_keeping_the_first_line_that_carries_one` |
| merge: replace in place, append, base first, retired, files | `context_probe::adds_what_did_not_install_drops_what_did_and_keeps_the_rest`, `context_probe::a_base_floor_row_that_did_not_land_is_a_tool_that_did_not_install_beside_the_persons`, `context_probe::a_retired_row_takes_its_own_row_out_of_the_facts_and_no_result_leaves_the_facts_as_they_were`, `context_oracle::merge_facts_folds_a_run_over_what_the_guest_kept_as_the_ts_does` |
| facts file shape, key order, parse | `context_oracle::the_facts_file_is_two_space_json_with_its_keys_in_order_and_a_newline` |
| scripts byte for byte, paths, agents | `context_oracle::the_probe_command_bounded_command_and_overlay_probe_are_the_ts_scripts`, `context_oracle::every_constant_and_path_is_the_ts_text`, `context_oracle::the_context_agents_are_the_catalogs_six_in_catalog_order` |
| scripts on a real shell: markers, overlay, no shell, aliases | `context_probe::the_probe_prints_its_markers_the_kernel_and_the_disk_and_names_each_hook_the_persons_file_claims`, `context_probe::asks_modprobe_for_an_overlay_the_kernel_does_not_list_yet_and_says_no_where_it_will_not_load`, `context_probe::opens_no_shell_of_the_machines_own_where_it_may_open_none_and_reads_every_other_fact_just_the_same`, `context_probe::the_zsh_alias_probe_*`, `context_probe::the_bash_alias_probe_*`, `context_probe::the_fish_alias_probe_*` |
| the bound: kill the group, fail, go on, say late | `context_probe::a_bounded_command_is_killed_at_its_bound_fails_there_so_the_script_can_say_so_and_the_script_goes_on`, `context_probe::a_login_shell_whose_rc_file_leaves_a_job_holding_the_output_open_*`, `context_probe::a_login_shell_that_has_not_answered_by_its_bound_*` |

## Pins (wave 2)

Values from `oracle/unit_pins.oracle.ts` (`tests/fixtures/scenes/units.json`, section `context_probe`), held by
`tests/context_probe_pins.rs`.

- P-1 `DISK`: the rest split on JS `\s+`, each word read with `Number`; the first two are size and free in kB, more
  words are ignored; both finite and size above 0, free may be negative (`DISK 100 -5` is free -5,120 bytes); `0x10`
  and `1e3` count; `-0` and a non-finite figure are no disk; one word is no disk
  (`disk_words_are_read_with_js_number`).
- P-2 `FACTS`: base64 as Node reads it (no padding needed, white space and other characters skipped, decoding stops at
  the first `=`), then `JSON.parse`: JSON `null` is no facts, while `5`, `[]` and `"abc"` are the empty facts
  `{ tools: [], agents: [], files: [] }`; `!!!` and an empty word are no facts
  (`facts_decode_as_node_and_a_scalar_is_the_empty_facts`).
- P-3 The key is the line up to its first space only: `KERNEL\t6.6` is one unknown key, a leading space makes the key
  empty; `ALIAS ll\tls -l` is name `ll\tls`, word `-l`; the rest is JS-trimmed, so `ALIAS  ll ls` is `ll`
  (`keys_split_at_a_space_only`).
- P-4 Lines are trimmed at the end with JS white space before the markers are looked for: `WSP_CTX` followed by U+FEFF
  or U+00A0 is the marker, followed by U+0085 is not (`the_markers_are_found_after_a_js_trim_end`).
