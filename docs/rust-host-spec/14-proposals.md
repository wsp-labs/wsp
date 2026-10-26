# 14 Proposals awaiting a ruling, and what waits for after the port

Everything in this spec is a description of the TS host or a rule that follows from it, except the proposals below. Each is written where it belongs with its reasons; this chapter lists them in one place for the owner to rule on the map, and lists the wrong-looking behaviours the port keeps on purpose.

## Proposals

| # | Ticket decision | Pick | Chapter |
|---|---|---|---|
| 1 | the crate list | 17 crates and the binary in a new workspace `host/` beside `daemon/`, path dependencies on `wsp-frames`, `wsp-seal`, `wsp-mcp`, measured so that no crate imports its own layer or above: the runtime's areas as a chain of four crates over `wsp-core` (`wsp-runtime` renamed, the daemon has one), the host package in three (the verb table, the setup roads, the server with the command dispatcher), the slate module in its own crate; five second cuts, checked the same way, when a crate passes 25,000 Rust lines, up to 22 crates | 01 |
| 2 | one owner of the state | one thread owns all state (a current-thread runtime with a `LocalSet`), handlers are local tasks with borrows that never cross an await; not one actor with a message per access | 03 |
| 2b | seeded replay | a replay build whose inputs are all seams, a seeded scheduler that interleaves sources, the seed printed on failure, invariants as the oracle for races | 03 |
| 3 | SQLite, fresh | the same JSON-bag tables and pragmas through `rusqlite`, no migrations, no import, a new shape number; typed tables after the port | 02 |
| 4 | one error type | `HostError {message, fix, kind, errors, warnings, left}` with a closed `Kind` enum and the TS class table | 05 |
| 5 | which caps match exactly | all of them: every length, cap, slice and index that leaves its unit counts UTF-16 units through `wsp_words::utf16` | 08 |
| 6 | the interface form | the packet, the gate and the implementer's page, with the research's ladder and held-out tapes and the large-module pilot's Haiku-first and review-clean rules | 13 |
| 7 | finding a host and the version range | discovery unchanged; a protocol number in the auth reply and an optional client range in the auth request, added to the TS host before the goldens freeze | 11 |
| 8 | which TS laws become Rust lints | three piles: wire laws run against the Rust binary, text laws get twins in `cargo xtask laws`, TS-shape laws stay with the TS code; plus the port's own clippy deny list | 12 |
| 9 | (new) the schema refusal's text | zod 3's issue shape with the first issue's code and path exact, the rest accepted as a difference | 05 |
| 10 | (new, for #2443) two tape line kinds | `{"close": {"code", "reason"}}` for a socket the host closed, and `{"in": "<text>"}` (a JSON string) for a raw text frame that is not JSON; `"<host-token>"` stands for the host's token | 00 |
| 11 | (new) a lone surrogate escape in a frame | read as U+FFFD on the way in, where serde_json would refuse the frame and answer `invalid json`; accepted-difference class `lone-surrogate` beside rule 8.5's dropped half | 08 |

The workspace's place, a `host/` workspace beside `daemon/`, is part of Proposal 1.

## The map's open questions this spec does not settle

- Whether the owner's threads are exported: settled by the owner's ruling on Proposal 3 (26 Oct 2026). The Rust host opens today's `state.db` as it is, so nothing is exported or left behind (rule 2.19).
- Windows: no Windows host today, so nothing here is ported for it.
- Whether the web app keeps a TS protocol package or reads types generated from the Rust wire (01). Until it is settled a wire change touches the Rust type and the zod schema in one change.

## Kept on purpose, ticket after the port

The TS host is the oracle, so these are ported as they are and each becomes a ticket once the port lands. A builder who finds another writes it under After the port in the unit's spec (13.9).

| # | Behaviour | Rule |
|---|---|---|
| 1 | A second `events.subscribe` on one socket doubles every later event | 4.22 |
| 2 | A schema failure carries no kind, so it exits 1 as a provider failure | 05 |
| 3 | `ticket` and `conflict` kinds exit 1 | 05 |
| 4 | Node's errno sentences (`ENOENT: ...`) reach clients as `error` | 5.3 |
| 5 | Two opening starts can pick the same thread id; `sessions.access` and `mark` write from a stale copy; `sessions.forget` misses a row started during its awaits; two deletes of a worktree workspace both run | 3.10 |
| 6 | `hold(purpose)` is not FIFO | 3.9 |
| 7 | Host git fetches through `gitHere` run without `GIT_TERMINAL_PROMPT=0` and can wait on a prompt | 7.2 |
| 8 | Local children inherit the host's whole environment, provider keys and `GH_TOKEN` included | 7.23 |
| 9 | Plaintext secrets inside `state.db`: turn tokens, pairing codes, agent setup values, image vault blobs | 02 |
| 10 | The spawn helpers report a timeout as 124, 1, 127 or nothing | 6.23 |
| 11 | A killed host leaves its local daemon running; the next start removes only its token folder | 6.16 |
| 12 | A slate timer with a huge `every` fires every millisecond (node's timer clamp) | 8.18 |
| 13 | A host restart with 4 slate runs running holds itself busy until another run ends; `stopAll` never drains the queue (the large-module pilot) | 13 |
| 14 | `runEndedWords` prints `Exited 0` for a run with neither a code nor a signal | 13 |
| 15 | The lock is written at the umask, not 0600 | 2.2 |
| 16 | Tickets nobody redeems stay in the host's map for the life of the process; entries leave only on redemption | 04, the paragraph before 4.35 |
| 17 | A thrown `null` or `undefined` sends no reply: the request waits forever and the host logs an unhandled rejection. The port cannot reproduce it (a Rust error is a value); every TS site that can throw one is a ticket | 5.15 |
| 18 | On the command line, a request sent after the socket closed is never failed and waits forever unless the caller races `client.closed` | 10.20 |
| 19 | The TS MCP server drops calls in flight when stdin ends | 10.25 |
| 20 | Thread records lose `filesIn` and `clouds` across a restart | 2.36 |
| 21 | An `events.subscribe` cursor sent without its stream is trusted across a restart | 3.28 |
| 22 | A lock with pid 0 or -1 holds forever, and `wsp down` signals the lock's pid without checking it | 6.28 |
| 23 | Attaching a run handle refuses in a loop as unreached forever | 6.20 |
| 24 | A non-numeric `.exit` settles a turn as `NaN`, and a run with no readable `.pid` is never killed | 6.34 |
| 25 | A cloudflared that ignores SIGTERM hangs the host's close | 6.41 |
| 26 | A local daemon that dies is never restarted until someone asks | 6.38 |
| 27 | An exit 1 of the copy verb with no output refuses with the empty sentence | 7.6 |
| 28 | A worktree's `kept` is never cleared when its reason goes away, and `keep()` compares by `JSON.stringify` | 7.13 |
| 29 | Catch-up reads two failed `rev-parse`s as nothing to do | 07, the catch-up row |
| 30 | `merge` lets the daemon's `no-host-cli` sentence through where every post as the person rewords it | 7.15 |
| 31 | A pull request read in flight at a stop re-arms a timer after the close | 7.33 |
| 32 | cloudflared's restart wait never resets after a healthy run | 7.38 |
| 33 | `git.snapshot`'s missing commit reads as the text `undefined` | 7.35 |
| 34 | A cut keeps a lone surrogate half; the port drops it, accepted-difference class `lone-surrogate` | 8.5 |
| 35 | `ANSWER_CAP` reads by bytes and checks the length in units | 8.2 |
| 36 | `DOOR_BODY_CAP` and `HTTP_URL_MAX` are named as bytes and count units | 8.2 |
| 37 | `localeCompare` follows the host's locale, so two hosts list the same names in two orders | 8.8 |
| 38 | The `replaced by a newer link`, `removed from this host` and `this host is stopping` reasons never reach the place | 9.7 |
| 39 | A held reach has no connect or ritual deadline | 9.1 |
| 40 | `place-host-key.json` is remade without a word when missing or malformed, which breaks every joined computer's pin | 9.20 |
| 41 | The relay heartbeat's `version` carries cloudflared's version, not wsp's | 9.13 |
| 42 | `stopRecordedConnector` gives up after 5 s with no `SIGKILL` | 9.21 |

## Differences between the two tool servers, for the owner's ruling

Rule 10.25 found three more differences between the TS MCP server and `wsp-mcp`. They are not settled: `wsp mcp` hands off to the Rust server whenever the binary carries one, so the Rust behaviour is what a person meets today, and in the first two the TS side looks wrong. The owner rules which side the port follows:

1. end of stdin: the TS server drops calls in flight; `wsp-mcp` waits and writes every answer before it exits 0;
2. a line past 10 MiB: the TS server closes; `wsp-mcp` has no cap;
3. `tools/call` with no `name` or bad `params`: the TS server answers `-32603` with the SDK's zod text, `wsp-mcp` `-32602 Invalid params`; and `wsp-mcp` echoes any JSON `id` and ignores `jsonrpc`, where the TS server drops a fraction, a `null` id and a message without `"jsonrpc":"2.0"`.

## Differences to settle toward the TS behaviour in the port

Not after-port tickets: the existing Rust tool server differs from the TS command line in four places, and the port settles each toward the TS (rule 10.16): the lock reader's missing program-gone check; an empty `--host` skipped instead of refused; the starter's missing service check; clap's exit 2 for a bad flag where the TS class is 3.
