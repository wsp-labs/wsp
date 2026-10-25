#!/usr/bin/env python3
"""Writes tests/cli_c/<unit>.rs: one #[test] per recorded case of tests/fixtures/cli_c/<unit>.json, each calling
holds(unit, fn, case, drivers_<unit>::<fn in snake case>). Run after the recorder: python3 gentests.py <unit>..."""
import json, re, sys, pathlib
here = pathlib.Path(__file__).resolve().parent
fix = here.parent
tests = here.parent.parent.parent / "cli_c"
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
    out = [f"// SPDX-License-Identifier: AGPL-3.0-only\n// Written by tests/fixtures/cli_c/record/gentests.py from {unit}.json: one test per recorded case.\nuse super::common::holds;\nuse super::drivers_{unit} as d;\n"]
    for c in data["cases"]:
        name = f"{snake(c['fn'])}__{slug(c['case'])}"
        seen[name] = seen.get(name, 0) + 1
        if seen[name] > 1: name += f"_{seen[name]}"
        out.append(f"#[test]\nfn {name}() {{\n    holds({rs(unit)}, {rs(c['fn'])}, {rs(c['case'])}, d::{snake(c['fn'])});\n}}\n")
    (tests / f"{unit}.rs").write_text("\n".join(out))
    print(unit, len(data["cases"]))
