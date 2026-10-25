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
  written by hand, `shadow/` is a real host's traffic (`node parity/record/collect.mjs --from <folder>` after serving
  it with `WSP_TAPE_DIR=<folder>`). Every fenced `tape` block in `docs/rust-host-spec/*.md` is a tape too, and the
  answers it writes down are held to the golden.
- `goldens/`: the TS host's answers to each tape, normalized; `KEY` is the TS tree's hash they were recorded at and
  `ops.json` the host's ops then.
- `accepted.tsv`: differences the owner said yes to. Anything else fails the run.
- `held-seed`: draws the held-back set, every shadow tape and about one in five of each op family's tapes.
- `units.tsv`: the units `cargo xtask unit` knows.

Each tape replays against a fresh host on a temp home and state folder. On Linux the run goes on in a network
namespace whose only interface is loopback, so a frame that would reach a provider, a box or the relay is refused
the same way every time; `--online` turns that off.
