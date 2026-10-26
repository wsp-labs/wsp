#!/usr/bin/env python3
"""Prints one line per recorded case of a unit: helper, case, then the outcome in short. python3 show.py <unit> [fn]"""
import json,sys
d=json.load(open(sys.argv[1]+'.json'))
for c in d['cases']:
    if len(sys.argv)>2 and c['fn']!=sys.argv[2]: continue
    if c.get('hung'): out='HUNG'
    elif 'error' in c: out='ERR '+json.dumps(c['error'])[:150]
    else: out='= '+json.dumps(c.get('value'))[:150]
    print(f"{c['fn']} / {c['case']}\n   frames={len(c['frames'])} said={c['said'][:6]}\n   {out}")
