# mcp_args
Source: `mcp.ts:1-290` (of 464 lines) at a3b870a6e. Tests: 2 of `test/mcp.test.ts`'s 16 cases call these alone (the
rest reach them through `detect_mcp`). Rust: `tests/detect_mcp_args.rs`. Pure: no host.

## What it does
Reads one MCP server definition: whether it runs on Linux and what it needs there, the mcp-remote it runs with the
store key that bridge keeps its sign-in under, and one reading of its arguments that both the row's detail line and
its count of secrets come from.

## Behaviour
1. Mac-only (mcp.ts:31-34): a path, `~/` expanded to `home`, starting with `<home>/Library/` or any of MAC_ONLY
   (`/Applications/`, `/System/`, `/Library/`, `/Volumes/`, `/private/`, `/opt/homebrew/Caskroom/`).
2. `path_of(s)`: a `^--?[\w-]+=` argument's text after its first `=`, else `s` (mcp.ts:37).
3. `binary_of(command, home)` (mcp.ts:40-45): `~/` expanded; not absolute: as is; under one of MCP_BIN_DIRS or
   `<home>/.local/bin/`: its basename; else `tilde(home, abs)`.
4. `linux_fit` (mcp.ts:49-60): http is `Ok("nothing to install")`. stdio: a mac-only command is
   `No("command <tilde(home, command)> is macOS-only, will not run")`; then each of `args` through `path_of`, `cwd`,
   then `env` values in order, the first mac-only one is `No("path <tilde(home, s)> is macOS-only, will not run")`.
   Else by `binary_of`: `npx` is `Ok("runs via npx")`; `uvx` or `uv` is `Ok("needs uv, installed on the machine when
   missing")`; any other `Ok("needs <bin> on the machine")`.
5. `mcp_remote_of(args)` (mcp.ts:82-109): the first argument matching `^mcp-remote(@[^/]*)?$`; none is `None`. `rest`
   is what follows it; the url is the first of `rest` matching `^https?://`; none is `None`. For every index `i` of
   `rest` with a next argument: `--header` or `-H` with a `:` past index 0 in the next sets
   `headers[js_trim(before)] = js_trim(after)`; `--authorize-param` with an `=` past index 0 sets params the same way
   (the loop does not skip the value). `--resource` and `--client-metadata-url` read the argument after the first
   occurrence, `js_trim`med; a flag with nothing after it reads nothing. The parts are url, resource, params as JSON,
   headers as JSON, metadata url, each only when present (an empty map is absent); a map's JSON is
   `JSON.stringify(o, sortedKeys)`: keys in UTF-16 order, compact, JavaScript's escaping. `hash` is the md5 hex of the
   parts joined by `|`. `client_credentials` is whether `rest` holds `--client-credentials`.
6. `v1` (mcp.ts:70-75): the version after the first `@` of the matched argument; none or `latest` is true; an exact
   `x.y.z` is true when x > 0, or y > 3, or y = 3 and z >= 2; anything else (a range, a tag, `0.3.1`) is false.
7. `secret_named(name)`: `is_secret_name(name)`, or a `_`-split part of the uppercased name equal to `CREDENTIAL`,
   `CREDENTIALS`, `AUTH` or `CERT` (mcp.ts:122-124). `hides_value(flag)`: HIDDEN_FLAGS holds it, or
   `secret_named(flag with every "-" as "_")` (mcp.ts:136).
8. A shown url is `host` plus `pathname` unless that is `/`, as WHATWG `URL` gives them (host lowercased, a default port
   dropped, the path percent-encoded); a url that does not parse is shown as is (mcp.ts:138-145).
9. A plain address parses as a URL with no username, no password, an empty `search` (a bare `?` is empty) and no path
   segment matching KEYLIKE_SEGMENT; one that does not parse is not plain (mcp.ts:173-180). `plain_arg(arg)`: starts
   with `/` or `~/`, or matches `^[A-Za-z][A-Za-z0-9+.-]*://` and is a plain address (mcp.ts:193).
10. An assignment argument `^([A-Za-z_]\w*)=` whose name is `secret_named`, or whose value is an address that is not
    plain, reads `{show: "<name>=…", secret: "arg <name> (<utf8 bytes of value> B)", key: name, kind: Assign}`; any
    other assignment reads `{show: arg}` (mcp.ts:185-190).
11. `read_args(command, args, home)` (mcp.ts:200-250), one pass with `program` = RUNNERS holds `binary_of(command)`
    and `flags` = true. `shown(a)` is `(<shown url>)` for an http(s) url, else `tilde(home, a)`. For each argument:
    - with flags on, `-y` or `--yes` reads `{}`; `--` turns flags off and reads `{show: "--"}`;
    - with flags on, an argument starting with `-`: its flag is `^(--?[A-Za-z][\w-]*)(=|$)` group 1 (maybe none);
      it hides when it has a flag and `hides_value(flag)`. With an `=`: a hiding one reads `{show: "<flag>=…", secret:
      "flag <flag> (<n> B)", key: flag, kind: Flag}`; else the value after the first `=` read as an assignment (rule
      10) shows `<text up to and with the first =><that show>` with its secret, key and kind; else `{show: shown(a)}`.
      Without an `=`: reads `{show: a}`; then when a next argument exists and does not start with `-`, it is consumed:
      a hiding flag reads `{show: "…", secret: "flag <a> (<n> B)", key: a, kind: Flag}`, else the value as an
      assignment, else `{show: shown(value)}`;
    - any other argument: an assignment reads as rule 10; else while `program` is on, or for a `plain_arg`, it reads
      `{show: shown(a)}` and turns `program` off; else `{show: "…", secret: "arg <i+1> (<n> B)", key:
      "argument <i+1>", kind: Positional}`, `i` the argument's 0-based index in `args`.
12. `stdio_line` (mcp.ts:259-263): the shows in order, past the fifth cut to five and `…`, after `tilde(home, command)`,
    joined by a space.
13. `home_paths` (mcp.ts:280-290): http is `[]`; else `args` through `path_of`, `cwd`, `env` values, each `~/`
    expanded, kept when under `<home>/` and not under `<home>/.local/bin/`, as `tilde`, once each in first-seen order.

## Edge cases
- `mcp_remote_of(["-y","@scope/other",url])` is `None`; `["mcp-remote"]` with no url is `None`.
- `--header` as the last argument sets nothing. A header with the colon first (`:x`) sets nothing.
- Byte counts are UTF-8: `é` is `(2 B)`.

## Decisions
1. `LinuxFit` is an enum `Ok { needs } | No { reason }` for TS's `{ok: true, needs} | {ok: false, reason}`.
2. `read_args` takes `command` and `args` for TS's `{command, args}` object.

## After the port
- `keepsV1` reads `0.4.0` as v1 but `1.0.0-beta` and `^0.3.2` as not, which can point at a folder the bridge never
  wrote.

## Wave 2 rules

Cases: `tests/fixtures/oracle/w2/mcp_args.json` (`oracle/w2-mcp_args.oracle.ts`, every value printed by the TS),
replayed by `tests/detect_mcp_args.rs` `w2_oracle::*`.

Corrections to the rules above, found against the TS (each overrides the rule it names):

- C1. Rule 6: the pin is `spec.split("@")[1]`, the text between the first and the second `@`; an empty pin (`mcp-remote@`) is `""`, neither absent nor `latest`, so `v1` is false; `mcp-remote@latest@0.1.0` is v1.
- C2. Rule 5: a `--resource` or `--client-metadata-url` value that is empty or blank after trim reads `""`, which is present, so the hash text ends in `|`.
- C3. Rule 8: only the parenthesised form goes through the url shortening, and only for an argument matching the case-sensitive `^https?://`; a flag's value `HTTPS://X.EXAMPLE/p` is shown raw.
- C4. Rule 5 edge: a header whose trimmed key is empty but whose colon is past index 0 (`" :x"`) sets `headers[""] = "x"`.

- W1. `linux_fit`, `mcp_remote_of`, `mcp_remote_hash`, `secret_named`, `hides_value`, `plain_arg`, `read_args`, `stdio_line`, `home_paths` over every edge the findings name (each key of the fixture).

## Tests
| Rule | Test |
|---|---|
| 1, 3, 4 | `linux_fit_home_paths_and_homebrews_prefix_have_a_linux_equivalent_library_and` |
| 5, 6 | `mcp_remote_hash_matches_the_bridges_store_md5_of_the_url_resource_sorted`, `mcp_remote_of_reads_v1_off_the_pinned_version`, `mcp_remote_of_lets_a_later_header_replace_an_earlier_one` |
| 7 | `hides_value_reads_a_flag_as_a_secret_name_with_dashes_as_underscores` |
| 8, 9 | `plain_arg_takes_paths_and_plain_addresses_and_refuses_a_login_query_or_key_segment` |
| 10, 11 | `read_args_reads_flags_assignments_and_positionals_as_the_detail_line_shows_them` |
| 12 | `stdio_line_cuts_past_five_words` |
| 13 | `home_paths_lists_home_paths_once_in_order_leaving_local_bin_out` |
