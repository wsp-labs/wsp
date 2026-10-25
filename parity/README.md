# Parity

The TS host's answers to recorded traffic, frozen, and one command that replays the traffic against a host and
diffs its answers with them.

```sh
cargo xtask parity                  # every tape against the TS host's goldens; a table per op family
cargo xtask parity --host <wsp>     # the same tapes against another wsp command, such as the Rust host
cargo xtask parity --seed 0         # frames sent in seeded windows; a failure prints its seed
cargo xtask parity record           # the TS host's answers written again as the goldens, keyed to the TS tree
cargo xtask parity collect          # the suites that drive the host from outside, run with the tap on, as tapes
cargo xtask parity held             # the tapes held back from implementers
cargo xtask unit <unit>             # one unit held to its packet, then its tests and the tapes it may see
```

- `tapes/`: one JSON line a frame, `{"in": frame}` sent to the host and `{"out": frame}` sent by it, after an
  optional `{"meta": {"door": "ws" | "mcp" | "cli", "source": ...}}`. `recorded/` comes from `collect`, `hand/` is
  written by hand, `shadow/` is a day of a real host's traffic (`parity/record/shadow-day.sh` on the Mac), which git
  ignores since it holds the person's own prompts and paths: it is copied to a reviewer's checkout, never pushed.
  Every fenced `tape` block in `docs/rust-host-spec/*.md` is a tape too, and the answers it writes down are held to
  the golden.
- `goldens/`: the TS host's answers to each tape, normalized; `KEY` is the TS tree's hash they were recorded at and
  `ops.json` the host's ops then.
- `accepted.tsv`: differences the owner said yes to. Anything else fails the run.
- `held-seed`: draws the held-back set, every shadow tape and about one in five of each op family's tapes.
- `units.tsv`: the units `cargo xtask unit` knows.

Each tape replays against a fresh host on a temp home and state folder, inside a sandbox: Linux network, mount and
process namespaces where every folder but /tmp is read-only, /run is empty and only loopback is up. A tape's command
line can be `wsp leave`, which takes what wsp installed off the computer it runs on, so with no namespaces (a Mac) the
run is refused unless `--no-sandbox` says to run the tapes on the computer as it is.
