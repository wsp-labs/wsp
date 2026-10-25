# app_ports
Source: the functions of `packages/protocol/src/app-ports.ts` (200 lines) at a3b870a6e; its flat constants (`DEFAULT_PORT`, `PLACE_PORT_OFFSET`, `DEFAULT_PLACE_PORT`, `LOOPBACK`, `WS_PATH`, `WILDCARD`, `PORT_TAKEN_REFUSAL`) are `wsp_wire::types::app_ports`. Tests: `packages/protocol/test/app-ports.test.ts` (18 of 20 cases ported in `tests/app_ports.rs`; skipped :138-151, the two source-tree laws that no other file spells 4400 or the loopback address). No `pinned` module.

## What it does
The port and address the host serves the app and its WebSocket on, read off `--port` and `--listen`; the one reading of loopback, the wildcard and a URL; the address a person types to join; and the lines a port somebody else holds says.

## Behaviour
1. `ws_url_of(url)`: the URL parsed; "wss:" for an https or wss scheme, else "ws:"; then "//", the parsed host (with its port where not the default), the path less every trailing "/", and "/ws". Query and fragment are dropped (app-ports.ts:29-34).
2. `join_address_word(url)`: the parsed URL's host (hostname and any non-default port); a text that does not parse comes back as given (:39-45).
3. `peer_address(a)`: one leading "::ffff:" off; None is "" (:50-52).
4. `is_loopback(a)`: exactly "localhost", "::1", "[::1]", or four dot-separated runs of 1 to 3 ASCII digits whose first is exactly "127"; case matters, and no octet is range-checked (:59-61).
5. `is_wildcard(a)`: exactly "0.0.0.0" or "::" (:69-71).
6. `is_url(w)`: starts with http, https, ws or wss and "://", any case (:75-77).
7. `served_hostname(w)`, in order: not `is_url` is None; a text the URL parser refuses is None; a scheme other than http or https is None; an empty hostname is None; else the parsed hostname, lower-cased and with brackets kept for IPv6 (:83-93).
8. `join_address_of(typed)`: trimmed. One starting http:// or https:// (any case) is the trimmed text as typed where `served_hostname` reads it, else None. Else a bare address, a bracketed IPv6 of hex digits and colons or a run of `a-zA-Z0-9.-`, then ":" and 1 to 5 digits, is "http://" and the trimmed text; anything else is None (:98-108).
9. `authority(a, port)`: an address holding ":" and not starting "[" is bracketed; then ":<port>" (:112-115). `relay_url_of(h)` is "https://<h>" (:120-122).
10. `ports_asked(port, listen)`: the port is `Number(port)` where given, else 4400, and `named` says it was given; the address is `listen` where given and not "", else "127.0.0.1" (:156-162).
11. `port_holder_words(h)`: None is "another process"; a host is "the host serving <statePath>"; a process is "<command> (pid <pid>)" (:170-173).
12. The lines: `port_taken_line` "Port <port> is in use on this computer by <holder>." (:176-178); `ports_picked_line` "Serving on <port>; <taken> is held by <holder>." (:182-184); `port_instead_line` (:191-193); `state_file_line` "Setting up <path>; --state <path> starts a fresh setup instead." (:198-200); `listen_beyond_loopback_line` (:126-128); `loopback_threads_line` (:132-134).

## Edge cases
- `served_hostname("HTTP://Box.Example")` is "box.example"; `("http://[2001:db8::5]:4400")` is "[2001:db8::5]"; "ws://box:4410", "http://", "https://", "box", "ftp://box" and "" are None (test asserts).
- `served_hostname("http://user:pw@box")` is "box"; `("http://box:99999")` is None (no such port); `("http://1.2.3")` is "1.2.0.3" (the URL parser's IPv4 reading).
- `join_address_of`: " 192.168.1.20:7788 " is "http://192.168.1.20:7788", "[::1]:4400" is "http://[::1]:4400" (test asserts); "box:99999" is "http://box:99999"; "HTTP://Box" stays "HTTP://Box"; "192.168.1.20:" and "ws://192.168.1.20:7788" are None (test asserts).
- `is_loopback`: "1270.0.0.1", "127.evil.example" and "127.0.0.1.evil.example" are false (test asserts); "127.999.0.1" is true; "LOCALHOST" is false.
- `ws_url_of("https://H:443/p//?q=1#f")` is "wss://h/p/ws"; `("http://h:80")` is "ws://h/ws"; `("ftp://x")` is "ws://x/ws".
- `join_address_word("HTTP://Bücher.example:4400")` is "xn--bcher-kva.example:4400"; `("nope")` is "nope".
- `ports_asked(Some("0"), None)` is port 0, named (test asserts). `Some("")` is port 0, named; `Some(" 12 ")` 12; `Some("0x10")` 16; `Some("1e3")` 1000; `Some("abc")` NaN.
- `peer_address(Some("::ffff:::ffff:1"))` is "::ffff:1": one prefix comes off.
- `authority("2001:db8::5", 4410)` is "[2001:db8::5]:4410", `("[2001:db8::5]", 4410)` unchanged (test asserts); `authority("x", NaN)` is "x:NaN".

## Behaviour nobody wrote down
- `new URL` is the WHATWG URL parser: hostnames lower-cased, internationalised names to punycode, default ports dropped, IPv4 shorthand expanded, ports above 65535 refused.
- Rule 8.6: `join_address_of`'s trim is JavaScript's whitespace.
- Rule 8.7: `\d` in `is_loopback` and the bare address is ASCII digits; every class here is ASCII only.
- Rule 8.11: a port in a sentence is `String(n)`, so `number_string`, NaN included.
- `Number(text)` is JavaScript's string-to-number: whitespace trimmed, "" is 0, hex, octal and binary prefixes and exponents read, anything else NaN.

## Tapes
`wsp up` and `wsp init` (the port and address they serve, the taken, picked and instead lines, the state file line, the beyond-loopback lines); the join screen and `wsp join`; the address a computer you own dials; `wsp host pair`.

## Decisions
1. `ports_asked` takes the two flags as `Option<&str>` arguments for the TS object; `ListenAsked.port` is `f64` so NaN survives, where `AppPorts.port` and `PortsAsked.port` are `u16`.
2. `PortHolder` is an enum (`Host { state_path }`, `Process(PortProcess)`), and the TS `undefined` holder is `Option<&PortHolder>` None; `PortProcess.pid` is `u32`.
3. Ports in sentences and `authority` take `f64`.
4. `ws_url_of` answers a `String`, where the TS throws on a text the URL parser refuses.

## After the port
- `ws_url_of("nope")` throws `Invalid URL` in the TS.
- `ports_asked(Some(""), ..)` reads port 0, which asks the system for any free port, and calls it named.

## Rules pinned
| Rule | Test |
|---|---|
| 1, 2, 3 | not pinned: no TS test reads `wsUrlOf`, `joinAddressWord` or `peerAddress` |
| 4 | `the_address_the_port_is_bound_on::reads_127_x_1_and_localhost_as_this_computer_and_the_wildcard_a_private_address` |
| 5 | `the_address_the_port_is_bound_on::names_the_wildcard_which_covers_loopback_apart_from_an_address_that_answers_only` |
| 6, 7 | `an_address_where_an_alias_could_go::*` |
| 8 | `the_address_a_person_types_to_join_a_wsp::*` |
| 9 | `the_address_the_port_is_bound_on::brackets_an_ipv6_literal_in_a_url_authority_and_leaves_everything_else_alone`; `relay_url_of` not pinned |
| 10 | `the_port_the_app_is_served_on::*`, `the_address_the_port_is_bound_on::binds_this_computer_alone_unless_listen_names_another_and_an_empty_word_is_no_wo` |
| 11 | `who_holds_a_port_and_the_lines_about_it::names_a_wsp_host_by_the_state_file_it_serves_a_process_by_its_command_and_pid_an` |
| 12 | `who_holds_a_port_and_the_lines_about_it::*`, `the_address_the_port_is_bound_on::says_once_in_one_line_with_no_em_dash_that_the_page_is_reachable_and_pairing_is`; `port_instead_line` and `loopback_threads_line` not pinned |
