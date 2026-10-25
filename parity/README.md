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
  the answers it writes down are held to the golden. A tape whose answers read the computer the host runs on (its
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
