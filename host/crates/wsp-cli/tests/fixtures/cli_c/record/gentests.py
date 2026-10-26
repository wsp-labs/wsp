#!/usr/bin/env python3
"""Writes tests/cli_c/<unit>.rs: one #[test] per recorded case of tests/fixtures/cli_c/<unit>.json, each calling
holds(unit, fn, case, drivers_<unit>::<fn in snake case>). A unit of more than PART cases is written in parts,
<unit>.rs, <unit>_2.rs and so on, so no file passes 1,500 lines; each part is a module of tests/cli_c/main.rs.
Run after the recorder: python3 gentests.py <unit>..."""
import json, re, sys, pathlib
here = pathlib.Path(__file__).resolve().parent
fix = here.parent
tests = here.parent.parent.parent / "cli_c"
PART = 250
def snake(n):
    return re.sub(r'(?<!^)(?=[A-Z])', '_', n).lower()
def rs(s):
    """A Rust string literal: backslash and quote escaped, control characters as \\u{..}, the rest as written."""
    out = []
    for ch in s:
        if ch == '\\': out.append('\\\\')
        elif ch == '"': out.append('\\"')
        elif ord(ch) < 0x20 or ord(ch) == 0x7f: out.append('\\u{%x}' % ord(ch))
        else: out.append(ch)
    return '"' + ''.join(out) + '"'
def slug(s):
    s = re.sub(r'[^a-z0-9]+', '_', s.lower()).strip('_')
    return s[:60] or "case"
for unit in sys.argv[1:]:
    data = json.load(open(fix / f"{unit}.json"))
    seen = {}
    head = f"// SPDX-License-Identifier: AGPL-3.0-only\n// Written by tests/fixtures/cli_c/record/gentests.py from {unit}.json: one test per recorded case.\nuse super::common::holds;\nuse super::drivers_{unit} as d;\n"
    parts = [[head]]
    for c in data["cases"]:
        if len(parts[-1]) > PART:
            parts.append([head])
        name = f"{snake(c['fn'])}__{slug(c['case'])}"
        seen[name] = seen.get(name, 0) + 1
        if seen[name] > 1: name += f"_{seen[name]}"
        parts[-1].append(f"#[test]\nfn {name}() {{\n    holds({rs(unit)}, {rs(c['fn'])}, {rs(c['case'])}, d::{snake(c['fn'])});\n}}\n")
    for at, out in enumerate(parts):
        (tests / (f"{unit}.rs" if at == 0 else f"{unit}_{at + 1}.rs")).write_text("\n".join(out))
    print(unit, len(data["cases"]), len(parts))
