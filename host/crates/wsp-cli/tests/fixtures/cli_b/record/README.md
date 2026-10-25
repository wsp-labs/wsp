# How cli-b's fixtures were made

The expected bytes in `../*.json` come from running the TS at `65f6b89c7` (a clean checkout with node_modules),
never from reading it:

```sh
cd tests/fixtures/cli_b/record   # illustrative
TZ=UTC P=record.probe.ts <checkout>/node_modules/.bin/vitest run --config vitest.config.mjs   # the command lines
P=pure.probe.ts <checkout>/node_modules/.bin/vitest run --config vitest.config.mjs           # pure.json
P=table.probe.ts <checkout>/node_modules/.bin/vitest run --config vitest.config.mjs          # the verb tables
python3 gen.py        # the verb tables of src/verbs/*.rs, bodies todo!()
python3 gentests.py   # tests/cli_b/<unit>.rs, one test per case and page
```

`record.probe.ts` runs each case of `daemon/crates/wsp-mcp/tests/answers/<tool>.json` through the TS `runVerb` with
a host answering every op from the case's replies, and the hand cases it lists, as text and as `--json`. The paths
at the top of each script name the checkout and this worktree.
