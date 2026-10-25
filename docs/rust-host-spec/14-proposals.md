# 14 Proposals awaiting a ruling, and what waits for after the port

Everything in this spec is a description of the TS host or a rule that follows from it, except the proposals below. Each is written where it belongs with its reasons; this chapter lists them in one place for the owner to rule on the map, and lists the wrong-looking behaviours the port keeps on purpose.

## Proposals

| # | Ticket decision | Pick | Chapter |
|---|---|---|---|
| 1 | the crate list | 15 crates in a new workspace `host/` beside `daemon/`, path dependencies on `wsp-frames`, `wsp-seal`, `wsp-mcp`; the runtime cut along its areas through a core trait; `wsp-runtime` renamed `wsp-core` (the daemon has one); five named second cuts when a crate passes 25,000 Rust lines | 01 |
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

The workspace's place, a `host/` workspace beside `daemon/`, is part of Proposal 1.

## The map's open questions this spec does not settle

- Whether the owner's threads are exported. Under Proposal 3 the layout is unchanged, so an export is a copy of rows; leaving them behind costs nothing (rule 2.19).
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

## Differences to settle toward the TS behaviour in the port

Not after-port tickets: the existing Rust tool server differs from the TS command line in four places, and the port settles each toward the TS (rule 10.16): the lock reader's missing program-gone check; an empty `--host` skipped instead of refused; the starter's missing service check; clap's exit 2 for a bad flag where the TS class is 3.
