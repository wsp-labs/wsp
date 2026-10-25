# The wsp host in Rust: a port specification

| Field | Value |
|---|---|
| Status | First draft. A specification of the Rust host as a port of the TypeScript host: it describes what the TS host does and pins it as rules. Ten proposals await the owner's ruling (14); nothing else here is a design choice. |
| Written | 2026-10-25 |
| Map | wsp-labs/wsp-map#2440 (the wsp host in Rust); this spec answers #2442 |
| Code read against | wsp main at `a3b870a6e`; facts about the GPUI app at `origin/gpui/main` `ca56a2a1f`. Every `file:line` is at one of those two and says which. |
| Inputs | #2440 and its comments (the measured facts, the research on other ports, the harness design, the model plan, the order, warm builds, the estimate, the owner's two directions of 2026-10-25), #2442, the port research (#2440 comment 6396136288), the large-module pilot (#2440 comment 6396255198), and the Slate spec v2 as the model of the form |
| Checker | `node docs/rust-host-spec/checker/check.mjs` replays every `tape` block in these files against a fresh TS host and diffs the answers. The comparison harness (#2443) reads the same blocks. |

## What this spec is, and is not

It is the cross-cutting part of the port: the architecture and crates, the state and its storage, concurrency, the wire and its errors, processes, the git and other commands, text and numbers, the links, the command line and tools, finding a host, the laws, and the porting guide that turns each module into a unit a cheap model can implement. Each module still gets its own one-page spec from its code and its tapes (13).

It is not a redesign. The TS host is the oracle: where its behaviour looks wrong the port keeps it and the spec lists it as an after-port ticket (14). It does not cover the GPUI app's screens, Windows, or migrating old state.

Three rules from the map hold over every chapter:

1. **The TS host is the oracle.** Describe it, don't redesign it.
2. **Tests first, with a spec.** Before any unit's Rust body: its interface, its one-page spec and its failing tests (13).
3. **Nothing lands on main before gpui/main has landed there.**

## Conventions

- A `file:line` is repo-relative at `a3b870a6e`. Chapters shorten `packages/runtime/src` to RT or name the file they mean at the top.
- A **rule** is numbered `N.M`, N the chapter: "rule 4.21". The key words MUST, MUST NOT, SHOULD and MAY are RFC 2119's. A rule with no subject binds the Rust host.
- A **proposal** is a boxed block starting "Proposal, awaiting the owner's ruling", numbered to match #2442's decisions where it answers one.
- **After the port** marks a behaviour kept on purpose and ticketed for later.
- The glossary is the root `AGENTS.md`'s. Words this spec adds: **oracle** (the TS host, whose answers are right by definition); **tape** (recorded frames, below); **unit** and **packet** (13); **the owner thread** (03); **kind** and **class** (05); **the own road** (rule 4.4).

## Tapes

Every example of wire behaviour is a fenced block with the language `tape`. Each line is one JSON object:

- `{"in": <frame>}`: a frame the host receives, exactly as sent. A frame that is a JSON string is sent as raw text, so `{"in": "not json"}` sends the bytes `not json`.
- `{"out": <frame>}`: a frame the host sends, exactly as it carried it.
- `{"close": {"code": <n>, "reason": "<text>"}}`: the host closed the socket.
- `"<host-token>"` stands for the host's token, put in when the frame is sent and taken back out of what comes back.

The `close` line and raw text frames are Proposal 10 for the harness (14). Each tape runs against a fresh host: an empty temp folder, `env -i`, its own `--state`, a free port, no relay, and a one-line stub page (rule 4.7). The checker waits for each `in` frame's reply by its id, then 250 ms for events that trail it. It normalizes only what differs between two runs of one host: the release in `version`, every UUID by first sight (the event stream's id among them), and the temp folder; #2443's harness owns the full normalizing. The checker compares lines as text, which is stricter than the harness's structural diff.

The first tape in the spec, the plainest exchange there is:

```tape
{"in":{"token":"<host-token>","id":1,"op":"auth"}}
{"out":{"id":1,"ok":true,"version":"0.3.8","road":"up"}}
{"in":{"id":2,"op":"projects.list"}}
{"out":{"id":2,"ok":true,"projects":[]}}
{"in":{"id":3,"op":"sessions.list"}}
{"out":{"id":3,"ok":true,"sessions":[]}}
```

## Conformance

A Rust host conforms when it answers every tape in this spec and every golden of the harness with no difference outside the accepted list, keeps every MUST rule, and passes the ported contract, skill and parity suites (10, 12). A unit conforms when its gate is green and its review is clean (13.19). A proposal binds once the owner rules it; until then a builder follows the pick as written and notes it in the unit's Decisions.

## Files

| File | Holds |
|---|---|
| 00-index | This file: status, conventions, tapes, conformance |
| 01-architecture | the host today, how it starts, its seams, the Rust side, Proposal 1 (the crates) |
| 02-state | the state folder, SQLite as used, transcripts, the records, what lives in memory, a start's rebuild, Proposal 3 (the store), secrets at rest |
| 03-concurrency | how the TS host interleaves, Proposal 2 (one thread owns the state), the bus, the serialization helpers, ordering, time and ids, Proposal 2b (seeded replay) |
| 04-wire | transport, the door, envelopes, who may send what, events, how JSON is written, close codes |
| 05-errors | kinds and classes, the failure reply, refusals, the log line, the schema refusal, Proposal 4 (one error type), Proposal 9 (the schema refusal's text) |
| 06-processes | the lock, starting a host, the service, signals, shutdown, turns, the local daemon, spawn helpers, restart |
| 07-git-and-commands | the two roads for git, every host-run git, the worktree lifecycle, merge, checkpoints, pull request caches, gh, other programs, environments, platforms |
| 08-text-numbers-time | UTF-16 and Proposal 5, whitespace, regexes, ordering, numbers, time, timer clamps |
| 09-links | the daemon links, the daemon's version, the place link, the seal, the relay, guests |
| 10-cli-and-mcp | the verb table, a verb's run, the `--json` contract, refusals, help, the MCP server, the record |
| 11-finding-a-host | discovery today, Proposal 7 (a protocol number) |
| 12-laws | the laws today, Proposal 8 (three piles), the lints the port adds |
| 13-porting-guide | Proposal 6: the recipe, the gates, the implementer's page, a worked example |
| 14-proposals | every proposal in one table, open questions, the after-port list |
| checker/check.mjs | replays every tape against a fresh TS host; `--record` makes a tape from `in` lines |

## Change log

- 2026-10-25, first draft: fifteen files written from the TS host at `a3b870a6e`, read by ten readers with every fact at its `file:line`, and the tapes recorded from a throwaway TS host. The porting guide carries the port research's recipe and the large-module pilot's rules (Haiku first, review-clean is done, the spec pins null and empty inputs, caps, kill paths and restart).
