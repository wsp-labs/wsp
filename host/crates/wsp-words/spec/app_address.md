# app_address
Source: `packages/protocol/src/app-address.ts` (110 lines) at the cut, 383708c78 (115 lines at a3b870a6e); its flat constant `LINK_KINDS` is `wsp_wire::types::app_address`. Tests: `packages/protocol/test/app-address.test.ts` (15 cases, ported whole in `tests/app_address.rs`; none skipped). No `pinned` module.

## What it does
The one writer and reader of the app's address: the hash that opens a page on a workspace, one of its threads, a subagent of that thread or the screen its next thread is written on; the pairing code a hash can carry on its end; and the `wsp://<kind>/<id>` link the desktop takes from the system, with the hash that carries it to the page.

## Behaviour
1. `workspace_hash(id)` is "#w/" and the id through `encodeURIComponent` (app-address.ts:35).
2. `app_hash(a)`: where `thread_id` is present (even ""), the workspace hash, "/t/" and the encoded thread, then "/a/" and the encoded subagent where one is present. With no thread, "/new" follows where `fresh` is exactly true, else the workspace hash alone; a subagent with no thread is dropped, and `fresh` beside a thread is dropped (:38-45).
3. `pairing_code_of(hash)`, in order: a hash starting "#c/" cuts at 0; one starting "#w/" cuts at the last "/c/" in it; anything else, or no "/c/", is None. The code is the decoded text after the marker; "" is None; else the code and `rest`, the hash before the cut (:48-54).
4. `address_from_hash(hash)` (:58-71): the hash less any pairing code (rule 3); not starting "#w/" is None. In what follows "#w/", the first "/t/" splits the workspace from the thread. With no "/t/" and a trailing "/new", the address is fresh and "/new" comes off. The workspace is decoded; "" is None. After "/t/", the first "/a/" splits the thread from the subagent. The thread is decoded; the subagent is decoded only where there is a "/a/" and the thread is not "". An empty thread or subagent is left out; `fresh` is present only as true.
5. `address_from_link(url)`: the first 6 UTF-16 units lower-cased must be "wsp://"; one trailing "/" comes off the rest; the rest split on "/" must be exactly two parts, a kind in `LINK_KINDS` (thread, workspace, project, settings, exact case) and an id matching `^[A-Za-z0-9][A-Za-z0-9_-]{0,127}$`; else None (:95-100, :88-91).
6. `link_hash(t)` is "#open/<kind>/<id>" (:103). `link_from_hash(hash)`: must start "#open/"; the rest split on "/" must be exactly two parts read as in rule 5, with no trailing "/" taken off (:106-110).

## Edge cases
- `address_from_hash("#w/")`, `("#gallery")` and `("")` are None; `"#w/ws_a1b2/t/"` is the workspace alone (test asserts).
- `address_from_hash("#w/new")` is workspace "new"; `"#w/ws_a1b2/t/thr_9/new"` is thread "thr_9/new", never fresh (test asserts).
- `address_from_hash("#w/ws_a1b2/new/c/7K3MQP2X")` is fresh "ws_a1b2"; `"#c/7K3MQP2X"` is None (test asserts).
- `pairing_code_of("#w/ws_a1b2/c/")` and `("#c/")` are None (test asserts); `pairing_code_of("#c/x/c/y")` is code "x/c/y", rest "".
- `address_from_hash("#w/a/c/b/c/code")` is workspace "a/c/b": the last "/c/" is the code's.
- `address_from_hash("#w/ws/t//a/x")` is the workspace alone; `"#w/ws/t/x/a/y/a/z"` is subagent "y/a/z"; `"#w/ws/new/t/x"` is workspace "ws/new", thread "x".
- `address_from_link`: "wsp://settings/keybindings/" reads, "WSP://thread/..." reads; "wsp://Thread/x", "wsp://thread/_x", "wsp://thread//", "wsp://thread/th%201", 129 units of id, and every shape in the test's list are None (test asserts and node).
- `link_from_hash("#open/thread/x/")` is None.
- `workspace_hash("a!*()~'b")` keeps every one of those marks unescaped.

## Behaviour nobody wrote down
- `encodeURIComponent` leaves `A-Za-z0-9-_.!~*'()` as they are and escapes every other byte of the UTF-8 as "%XX", upper-case hex; `decodeURIComponent` reads any "%XX" back, "+" included as itself.
- Rule 8.9: the scheme check lower-cases with full mapping; only ASCII can match "wsp://", so the result is the same.
- Rule 8.7: `LINK_ID` is ASCII only, so it reads per char.
- The address is the app's record of what a person reads and is read on every refresh, so its form must read the same after a restart and from a link written before the port.

## Tapes
The app's location hash on every pick; a thread row's copy-link action; the desktop's `wsp://` deep link and the "#open/" hash it hands the page.

## Decisions
1. `AppAddress` is a struct with `Option` fields for the TS optional ones; `fresh` stays `Option<bool>` so `Some(false)` writes like None.
2. `pairing_code_of` answers `PairingCode { code, rest }` for the TS inline object.
3. `LinkKind` is an enum (lowercase on the wire) for the TS string union.
4. `decodeURIComponent` throws `URIError` on a malformed escape; the Rust signatures carry no error, so `pairing_code_of` and `address_from_hash` have only None to answer with.

## After the port
- A malformed escape throws out of `pairingCodeOf` and `addressFromHash` (`"#w/%E0%A4%A"`, `"#c/%zz"`) instead of naming nothing.
- An id that is exactly "c" with a segment after it reads as a pairing code: `app_hash` of workspace "c" and thread "x" reads back as None, and of thread "c" with subagent "s" reads back as workspace "ws/t". Ids wsp mints never are "c".

## Rules pinned
| Rule | Test |
|---|---|
| 1 | `the_workspace_a_page_opens_on::*` |
| 2 | `the_thread_a_page_opens_on::*`, `the_subagent_a_page_opens_on::*`, `the_screen_a_workspace_s_next_thread_is_written_on::*` |
| 3 | `the_code_wsp_init_puts_in_the_address_of_the_page_it_opens::*` |
| 4 | `the_workspace_a_page_opens_on::*`, `the_thread_a_page_opens_on::*`, `the_subagent_a_page_opens_on::*`, `the_screen_a_workspace_s_next_thread_is_written_on::*`, `the_code_wsp_init_puts_in_the_address_of_the_page_it_opens::*` |
| 5, 6 | `a_wsp_link::*` |
