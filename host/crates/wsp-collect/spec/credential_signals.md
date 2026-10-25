# credential_signals

Source: `packages/collect/src/credential-signals.ts` (172 lines) at `a3b870a6e`. Tests: `credential-signals.test.ts` (6 cases). Rust: `src/credential_signals.rs`; tests `tests/credential_signals.rs` (6), `tests/oracle_cases.rs` `credential_signals::*` (3: 59 names, 66 texts, 19 files against the TS).

## What it does

Says whether a file looks like a credential: its name, its mode, the key names it holds, a PEM header, a password in a URL. A value that names a secret held elsewhere (a template, an environment reference, a placeholder, nothing) raises nothing. The project bundle reads a folder's files against these rules; no value read here is returned except by `bare_urls`, which returns the text with the passwords cut out.

## Behaviour

1. `extension` (:45-48): lower-cased text after the last `.`; empty when there is no dot or the dot is at index 0.
2. Skipped names (:50-52): exactly `.viminfo .CFUserTextEncoding .z known_hosts .envrc`; ending `_history`; ending `.example .sample .template .dist .pub .lock`; an extension in the source and media list (:37). `name_signal` (:54-57) is false for a skipped name, else true when any pattern of :17-33 matches (JS regex semantics; `i` patterns ignore case).
3. `mode_signal` (:59-61): `mode & 0o077 == 0` and `0 < bytes <= MAX_BYTES` (102,400).
4. Key pairs (:93-107): text that parses as JSON (any JSON) is read as JSON: an object's keys to depth `KEY_DEPTH` (3), depth first in key order, each with its value when a string; a non-object (array, number, string, null) gives none. Text that does not parse: each `\n` line matching `/^\s*(?:\/\/[^\s:]+\/:)?"?([A-Za-z_][\w.-]*)"?\s*[:=]([^\n]*)$/` gives (key, `line_value(rest)`), where `line_value` (:86-90) cuts a `\s+#...` trailing comment, JS-trims, drops one trailing comma, trims, unwraps one pair of `"` or `'`, and trims. A CR left at the line's end is whitespace for the trims.
5. `top_level_keys`: the keys of rule 4 in order. `keys_signal` (:117-122): some key whose normalised form (`([a-z0-9])([A-Z])` gets an underscore, `-` becomes `_`, lower-cased) is in `KEY_NAMES` or ends with a `KEY_SUFFIXES` entry, and whose value is not a reference. A reference (:71) is a string matching `^(\$\{\{[^}]*\}\}|\$\{[^}]*\}|\$[A-Za-z_]\w*|<[^<>]*>)?$`: the empty string counts; a non-string value (a structure, a number, `null`) is never a reference, so it raises.
6. `pem_signal` (:124-126): the JS-trim-started text starts with `-----BEGIN`, then `[ A-Z0-9_-]*`, then `PRIVATE KEY`.
7. `bare_urls` (:130-145): every non-overlapping match, left to right, of `\b([a-z][a-z0-9+.-]*:\/\/)[^\s/@:]*:([^\s/@]+)@([^\s"']+)` (case-insensitive) whose password group is not a reference is replaced by scheme + rest; `urls` lists those replacements in order. A reference match is left as written. `url_signal` is `!urls.is_empty()`.
8. `file_signals` (:152-172): `bytes == 0` or a skipped name is `None` before anything else. Signals in this order: `name` (name_signal), `mode` (mode_signal), then, only when `0 < bytes <= MAX_BYTES` and the extension is one of `json yaml yml toml pem key env` or empty, `read()` is awaited once: text that is only whitespace around nothing, `{}` or `[]` ends with `None` (even with a name signal); otherwise `parsed = !top_level_keys(text).is_empty()` and `keys`, `pem`, `url` are added in that order. No signal is `None`. A lone `mode` stands only when `shallow`, the extension is `json yaml yml toml` and the text gave no keys (or was not read); otherwise `None`.

## Edge cases

- `.env` with text `  {} ` is `None` though its name is credential-shaped (oracle file-signals).
- `id_` and `.env.` are names the patterns do not take; `id_RSA` is not (lower-case pattern, no `i`) (oracle names).
- A file of exactly 102,400 bytes is read; 102,401 is not (oracle file-signals).
- `{"password":null}` raises `keys` (a non-string value is not a reference) (oracle texts).

## Behaviour nobody wrote down

- Rule 8.6: every `trim`, `trimStart` and `\s` here is JavaScript's whitespace set (`wsp_words::js_trim`, `is_js_whitespace`).
- JSON parse is JavaScript's `JSON.parse`: serde_json differs only on duplicate keys (JS keeps the last value, in the first key's place); `top_level_keys` lists such a key once.

## Decisions

1. The regexes have no look-around, so `regex` (in the crate) takes them as written, with `(?i)` where the TS uses `/i`; `\w` and `\b` are ASCII in JS, so use `(?-u:\w)` and `(?-u:\b)` or explicit classes.

## Rules pinned

| Rule | Test |
|---|---|
| 1, 2 | `name_patterns_token_and_secret_as_whole_words_*`, `every_name_of_the_oracle_*` |
| 3 | `mode_owner_only_and_under_100_kb_never_empty` |
| 4, 5 | `key_names_at_any_depth_of_json_yaml_toml_*`, `a_value_that_points_at_a_secret_is_not_one_*`, `every_text_of_the_oracle_*` |
| 6 | `pem_header`, oracle texts |
| 7 | `url_credentials_a_password_in_the_userinfo_*`, oracle texts |
| 8 | `file_signals_reads_only_what_it_may_and_answers_what_the_ts_answered` (signals and read counts) |
