# Parity

The TS host's answers to recorded traffic, frozen, and one command that replays the traffic against a host and
diffs its answers with them.

```sh
cargo xtask parity                  # every tape against the TS host's goldens; a table per op family
cargo xtask parity --host <wsp>     # the same tapes against another wsp command, such as the Rust host
cargo xtask parity --seed 0         # frames sent in seeded windows; a failure prints its seed
cargo xtask parity --held <dir>     # the held-back tapes as well, which only the reviewer has
cargo xtask parity record           # the TS host's answers written again as the goldens, keyed to the TS tree
cargo xtask parity collect          # the suites that drive the host from outside, run with the tap on, as tapes
cargo xtask parity held --held <dir> --draw   # move the tapes the folder's seed draws out of the repository
cargo xtask unit <unit>             # one unit held to its packet, then its tests and the tapes it may see
```

- `tapes/`: one JSON line a frame, `{"in": frame}` sent to the host and `{"out": frame}` sent by it, after an
  optional `{"meta": {"door": "ws" | "mcp" | "cli", "source": ..., "computer": ...}}`. `recorded/` comes from
  `collect`, `hand/` is written by hand. Every fenced `tape` block in `docs/rust-host-spec/*.md` is a tape too, and
  the answers it writes down are held to the golden, read as a replay reads its host's: its `auth` answer's version as `<version>`,
  the `/tmp/wsp-tape-...` folder it was recorded in as `<tmp>`, and a time as `<time+5m in ms>` written out. A tape whose answers read the computer the host runs on (its
  tools, cores, user, node) names that computer in `computer`: a check elsewhere skips it and says so, and a record
  elsewhere leaves its golden alone unless `--move-here`.
- `goldens/`: the TS host's answers to each tape, normalized; `KEY` is the TS tree's hash they were recorded at,
  `ops.json` the host's ops and `tools.json` the tool server's tools then. Every op family, both doors and every tool
  on each door need a tape, or the run fails; the table counts each op's success answers apart from its refusals.
- `accepted.tsv`: differences the owner said yes to. Anything else fails the run.
- `units.tsv`: the units `cargo xtask unit` knows.

The held-back tapes are not in this repository, nor in its history: about one in five of each op family's tapes and
every shadow tape (a day of a real host's traffic, `parity/record/shadow-day.sh` on the Mac) live in a folder only
the reviewer has (`tapes/`, `goldens/`, `seed`), given as `--held <dir>`. Without it nothing is held back.

A tape names a joining computer's values by mark (`<joiner-key>`, `<joiner-nonce>`, `<joiner-ephemeral>`,
`<joiner-signature>`, `<join-code>`) and the replay plays that computer: it mints a code on a socket of its own,
signs, seals every frame after the join and opens the host's. Every place.join reply's signature is checked against
the key it names.

Each tape replays against a fresh host on a temp home and state folder, inside a sandbox: Linux network, mount,
process, host name and IPC namespaces where every mount, the root of the disk included, is read-only, every socket
of the computer is covered, /tmp, /run, /dev and /proc are the run's own and only loopback is up. Only `record`
writes, and only the goldens. A tape's command line can be `wsp leave`, which takes what wsp installed off the
computer it runs on, so with no namespaces (a Mac) the run is refused unless `--no-sandbox` says to run the tapes on
the computer as it is, which it says again on every run.

A lone surrogate escape in a tape line, a golden or a host's answer is read as `JSON.parse` reads it and written back as the
same escape (`xtask/src/json.rs`); a text frame or a command-line word carrying one is sent with U+FFFD, as a real client
sends it. The harness keeps U+10F800 to U+10FFFF (Supplementary Private Use Area-B) to stand for a lone half, so a real
character there is refused: a tape line holding one stops the run naming its file and line, and a host's answer holding
one is read as text, not as a frame.

## How answers are compared

Replies are compared in the order their requests were sent. Events are compared in the order they arrived on each
stream the TS host keeps ordered, and never sorted by their text:

- a thread's events (`threadId`): its turns' transcript events, `thread.head`, `session.held` and `session.starting`,
  in emit order (spec 03, rules 3.6 and 3.12);
- a thread's `session.row` pushes, which go out in order on one chain;
- a turn's events that name no thread;
- a workspace's cost ticks (`workspace.cost`);
- each other family's events about the one thing they name (`<family>Id`, `id` or the workspace), or about the host;
- the frames the socket itself sends: a close, a refusal of a frame that is no request, a binary frame.

The windows where the TS host cannot order its events, inside which they are compared as a set:

1. Between two streams, over the whole replay. Two threads' turns are separate processes, a cost tick comes off the
   status poll's timer, the forwards, init, doctor and release doors send beside the bus, and one request's events
   race another's.
2. A thread's rows among its other events. A `session.row` push waits on a microtask and on `placedRows`
   (`packages/runtime/src/machines/boot.ts`, spec 03, rule 3.12), so it lands anywhere after the move it reports.
   The rows keep their order among themselves.

An id keeps its form when its value goes: a uuid in capitals reads `<uuid-1, in capitals>`, a prefixed id of other
than eight hex digits `<ws-1, 16 digits>`, a pairing code with its dash `<code-1, with a dash>`, and any other id says
its length (`<id-1, 32 characters>`). A time reads as its distance from the replay's start in whole minutes
(`<time+10m>`), so a TTL off by under a minute reads alike; the host's TTLs are whole minutes (a ticket 5, a pairing
code 10), so a missing or wrong one still fails. A `code` of eight capitals and digits reads as a pairing code, so a
refusal code such as `"CONFLICT"` there would too; the TS host sends none (its refusal codes are four characters,
zod's are lower case).
