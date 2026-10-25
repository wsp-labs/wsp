# place_word
Source: the functions and word table of `packages/protocol/src/place-word.ts` (114 lines) and `packages/protocol/src/place-name.ts` (67 lines) at a3b870a6e; their flat constants (`HERE_PLACE_ID`, `SOLARI_CONSOLE`) are `wsp_wire::types::place_word`. Tests: no TS test file; `tests/place_word.rs` (6 cases, 92 rows, every value printed by the TS under node).

## What it does
What a person calls a provider and its key; how a typed word names a place; the two-halves refusals of a computer's rename and ssh login; and the name a computer is read by, with which computer a workspace runs on.

## Behaviour
1. `provider_key_words(word)`: box is `{Boat, Boat API key, boat.dev/dashboard, subThreads}`, solari is `{Solari, Solari API key, console.getsolari.com}`; any other word, case included, None (place-word.ts:25-35). `provider_key_name`: the name, else the word as given (:38). `sub_threads_only`: only box (:41).
2. `place_said(place)`: a provider by its provider name, any other place by its name (:44). `cloud_named(ids, word)`: the first id that `names_place` as a provider (:47).
3. `names_place(place, word)`: the word equals id or name exactly; or, only for kind provider, id, name or provider name equal the word after `to_lowercase` on both (JS `toLowerCase`, rule 8.9) (:53-54).
4. `place_rename_refusal`: here or a provider keep their name; an empty name is refused; a name another place (other id) answers to by `names_place` is refused; else None (:62-67).
5. `place_ssh_refusal`: here or a provider take no login; "" is refused; a login must match `^[\w.-]+@[\w.:\[\]-]+$` whole, `\w` ASCII (:69-74). The other refusals and lines are fixed sentences of their arguments (:76-110).
6. `place_name(place)`: the label when set (even ""), else a provider's provider name, else the name (place-name.ts:44-48). `here_name`: the place with id `here`, else "" (:51-54). `is_brokered(record, place)`: a cloud workspace (by `workspace_kind`) with no place, or whose place is a provider (:23-25).
7. `place_of(places, workspace)` reads `workspace_place_id` (place_state unit); `workspace_computer_name`: that place's name, else here's for a local workspace, else `where_word` with the provider's name (place-name.ts:56-66).

## Edge cases
- `names_place` of a computer is case-sensitive: "SPOO" does not name "spoo"; a provider "box" is named by "boat" and "BOX". "İ" lowers to "i̇" in both languages.
- Logins: `root@[::1]` and `u.s-e_r@h-o.st:22` pass; `ré@h`, `a@b@c`, `root@h h`, `root@`, `@h` are refused.
- A workspace with no kind reads as cloud (`workspace_kind` of a record from before local workspaces), so it is brokered.
- An unknown provider with no place names the provider word itself ("nope").

## Behaviour nobody wrote down
- Rule 8.9: case folding is full-mapping `to_lowercase`, the same as JS.
- Rule 8.7: the login regex is ASCII-only `\w` in both.

## Tapes
`places.set` and `places.list` rename and login refusals; every sentence naming a provider.

## Decisions
1. The TS drops solari from the table in a public build (`__WSP_CLOUD__` false); the Rust host is the cloud build and keeps it.
2. `PlaceRef { id, name, kind }` stands for the TS structural `{ id; name; kind? }` arguments.

## After the port
- `place_name` of a place labelled "" reads "", an empty row name.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2 | `a_provider_is_named_by_its_key_words` |
| 3 | `a_word_names_a_place_by_id_or_name_and_a_provider_in_any_case` |
| 4, 5 | `renames_and_logins_are_refused_in_two_halves` |
| 6 | `place_names::a_place_is_named_by_its_label_else_its_provider_words`, `place_names::a_workspace_without_a_place_is_brokered_on_a_cloud` |
| 7 | `place_names::a_workspace_finds_its_place_by_id_then_kind_then_provider` |
