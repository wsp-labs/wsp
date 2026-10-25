#!/usr/bin/env python3
# SPDX-License-Identifier: AGPL-3.0-only
# Writes data/linux-bottles.json from packages/collect/src/data/linux-bottles.ts, the TS host's generated table, so
# the crate embeds the same names without a hand translation. The output is serde_json's pretty form of
# BottleSnapshot plus a newline, which is what brew_bottles::render_snapshot writes on a refresh.
#   python3 host/crates/wsp-collect/data/linux_bottles.py   (from the repository root)
import json, pathlib, re, sys

root = pathlib.Path(__file__).resolve().parents[4]
src = (root / "packages/collect/src/data/linux-bottles.ts").read_text()
fetched = re.search(r'^  fetched: (".*"),$', src, re.M).group(1)
source = re.search(r'^  source: (".*"),$', src, re.M).group(1)

def names(key):
    block = re.search(r"^  " + key + r": \[\n(.*?)^  \],$", src, re.M | re.S).group(1)
    out = []
    for line in block.split("\n"):
        if line == "":
            continue
        m = re.fullmatch(r'    (".*"),', line)
        if m is None:
            sys.exit(f"unexpected line in {key}: {line!r}")
        out.append(json.loads(m.group(1)))
    return out

snapshot = {"fetched": json.loads(fetched), "source": json.loads(source), "linux": names("linux"), "noLinux": names("noLinux")}
text = json.dumps(snapshot, indent=2, ensure_ascii=False) + "\n"
(pathlib.Path(__file__).parent / "linux-bottles.json").write_text(text)
print(f"{len(snapshot['linux'])} with a Linux bottle, {len(snapshot['noLinux'])} without, fetched {snapshot['fetched']}", file=sys.stderr)
