# recipe_file
Source: the functions of `packages/protocol/src/recipe-file.ts` (215 lines) at a3b870a6e; its zod shapes (`RecipeSignIn`, `GitHubSignIn`, `ServerSignIn`, `RecipeFile`, `RecipeView`) and flat constants (`WSP_COMMAND`, `RECIPE_KINDS`, `NO_RECIPE`, `COPY_KEYS_WORD`, `RECIPE_NAME_REFUSAL`, `NO_RECIPES`, `RECIPES_TICKET_REFUSAL`) are `wsp_wire::types::recipe_file`. Tests: `packages/protocol/test/gh-status.test.ts` (2 cases on `ghStatusOf`), ported in `tests/recipe_file.rs`, with `pinned` beside them. No other TS test of `packages/protocol` reads these functions.

## What it does
A recipe, a named pick of what goes on a computer: what `gh auth status` says of its account, the file name a recipe's name is kept under, the counts and the one line of what it holds, the canonical text its resolved form hashes to, wsp's own pieces taken out of an old one, and the refusals of the recipe ops.

## Behaviour
1. `gh_status_of(said)` (recipe-file.ts:27-35): the account is the first "Logged in to <non-space> account <login>" or "... as <login>", the login a run of non-space. The scopes are what follows the first "Token scopes:" and any whitespace, to the end of that line, split on ",", each with every "'" taken out and trimmed, empties dropped. Each is kept only where found, and scopes only where at least one is left.
2. `wsp_own_server(name)`: exactly "wsp", `MCP_SERVER_NAME` (:84).
3. `without_wsp_own(file, packages)`: the file less every `mcp` row `wsp_own_server` names and every `clis` row named in `packages`, the rest in their order; a file that loses nothing comes back as it was (:91-95).
4. `recipe_slug(name)`: trimmed, lower-cased, every run of anything but `a-z0-9` one "-", leading and trailing dashes off, then cut to 64 units. A name with no ASCII letter or digit is "" (:105-111).
5. `recipe_counts(file)`: in `RECIPE_KINDS` order (`agents`, `mcp`, `clis`, `skills`, `plugins`, `folders`, `configs`), the rows of each table; configs counts which of git, shell and github are present (:117-127).
6. `recipe_summary(file)`: `plural(n, word)` for every kind above 0, in that order, joined ", ", with the words agent, MCP server, CLI, skill, plugin, folder, config; no row at all is "nothing picked" (:130-134).
7. `recipe_canon(resolved)`: `JSON.stringify` of `{ file, items }` with the keys of every object at every depth sorted, arrays kept in their order, and absent fields left out (:147-158). Its sha256 is the recipe hash a computer that applied the recipe is compared against, so the text must be byte for byte the TS's, across a restart and across the port.
8. `keys_kept_line(keys, recipe)`: "not copied: it needs <a key for none, else list_words(keys)>; to send <it for exactly 1, else them>, " then, with no recipe, "press Copy the keys on this row in the computer's Setup in Settings > Computers", else "pick Copy the keys under MCP servers on the recipe <recipe> in Settings > Recipes" (:194-195).
9. `no_such_recipe_refusal(word, held)`: "no recipe named <word>; none is saved yet" for none held, else "...; you have <held joined ", ">" (:201-202). `recipe_from_here_refusal` (:209), `no_picks_refusal` (:212).

## Edge cases
- `gh_status_of` of gh's two-account answer is account "Zingzy", scopes gist, read:org, repo, workflow: the first account wins; "...Logged in to github.com as dev (oauth_token)" is account "dev"; "You are not logged into any GitHub hosts." is nothing (gh-status.test.ts asserts).
- `gh_status_of("Token scopes: '', ,repo")` is scopes ["repo"]; `("Token scopes: none")` is ["none"].
- `recipe_slug("  My Mac \u{2014} Pro!! ")` is "my-mac-pro"; `("ÀB")` is "b"; `("İstanbul")` is "i-stanbul"; `("---")` is "".
- `recipe_slug` of 63 "a", "-b" is 63 "a" and "-": the cut comes after the dashes are trimmed.
- `recipe_summary` of a file with 1 agent, 2 CLIs and the git and github configs is "1 agent, 2 CLIs, 2 configs"; of a file with no rows, "nothing picked".
- `recipe_canon` writes `{"B":1,"Z":4,"a":2,"é":3}` for those keys: code unit order, upper case before lower.
- `keys_kept_line(&["A", "B", "C"], None)` reads "it needs A, B and C; to send them, press ...".
- `no_such_recipe_refusal("x", &["a", "b"])` is "no recipe named x; you have a, b".

## Behaviour nobody wrote down
- Rule 8.6: `\s`, `\S` and every trim are JavaScript's whitespace.
- Rule 8.7: the two `gh` regexes are ported by hand; `.` stops at "\n", "\r", U+2028 and U+2029; the slug's class works per UTF-16 unit.
- Rule 8.8: `recipe_canon` sorts keys with `utf16::cmp`, as `Array.prototype.sort` does.
- Rule 8.9: the slug lower-cases with full mapping, so "İ" becomes "i" and U+0307, and the mark becomes a dash.
- Rule 8.11: a count in `recipe_summary` is `plural`, so `number_string`.

## Tapes
`wsp recipes`, `wsp recipes show`, `wsp recipes save` and `wsp recipes remove` and their tools (the summary line, the slug, the refusals); the Recipes page in Settings; the GitHub row's account and scopes in a computer's Setup; the "not copied" note on a server row; the recipe hash a computer's applied picks are compared against.

## Decisions
1. `gh_status_of` answers `GhStatus { account, scopes }`, both `Option`, for the TS object.
2. `without_wsp_own` takes `&IndexSet<String>` for the TS `ReadonlySet` and answers an owned `RecipeFile`, a clone where nothing came off.
3. `RecipeKind` is an enum; `recipe_counts` answers `IndexMap<RecipeKind, usize>` in `RECIPE_KINDS` order for the TS record.
4. `ResolvedRecipe.items` is an `IndexMap<String, String>`; `recipe_canon` sorts it, so its order does not matter.
5. `RecipeOptions` and its rows are structs (`AgentOption`, `McpOption`, `CliOption`, `SkillOption`, `PluginOption`, `ConfigOption` with a `ConfigId` enum, `FolderOption`); weights and counts are `f64`, and `PluginOption.on` is `Option<bool>` for the TS `true` or absent.
6. `keys_kept_line` and `no_such_recipe_refusal` take `&[impl AsRef<str>]`.

## After the port
- `keys_kept_line(&[], ..)` reads "it needs a key; to send them": the pronoun counts no keys as many.
- `gh_status_of`'s `\s*` after "Token scopes:" runs over a line end, so an empty scopes line reads the next line as a scope: "Token scopes:\n  - Active account: true" is scopes ["- Active account: true"].
- `recipe_slug` cuts after trimming the dashes, so a long name can end in "-".

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `gh_s_status_as_wsp_reads_it::*` |
| 2, 4, 6 | `pinned::a_recipe_is_named_counted_and_its_own_server_left_out` (the summary of an empty file only) |
| 8, 9 | `pinned::the_lines_a_recipe_refuses_or_keeps_keys_with` |
| 3, 5, 7 | not pinned: a RecipeFile with rows reads through wsp-wire's own unfinished deserializers |
