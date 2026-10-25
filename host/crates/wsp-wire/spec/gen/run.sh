#!/bin/sh
# SPDX-License-Identifier: AGPL-3.0-only
# run.sh: regenerate wsp-wire's views/ and types/ shapes from the protocol's zod schemas, the shape fixtures zod
# answers and their tests, and the rule oracles the hand-written tests read, from the TS checked out here. Keeps the
# hand-written part of each file (after its "Hand-written below" line). Needs node and `pnpm install` at the root.
set -e
export WSP_REPO=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
export WSP_WORK=$(mktemp -d)
HERE=$(cd "$(dirname "$0")" && pwd)
CRATE=$WSP_REPO/host/crates/wsp-wire
P=$WSP_REPO/packages/protocol/src
cp "$HERE"/*.mjs "$WSP_WORK"/

# One ESM bundle of every protocol module a shape may come from, keyed by its path under src/.
{
  for f in "$P"/views/*.ts "$P"/wire/*.ts "$P"/*.ts "$P"/slate/wire.ts "$P"/slate/types.ts; do
    b=${f#"$P"/}; b=${b%.ts}
    case $b in index|format|exit) continue;; esac
    id=$(echo "$b" | tr '/-' '__')
    echo "import * as m_$id from \"$f\";"
  done
  echo "const M = {};"
  for f in "$P"/views/*.ts "$P"/wire/*.ts "$P"/*.ts "$P"/slate/wire.ts "$P"/slate/types.ts; do
    b=${f#"$P"/}; b=${b%.ts}
    case $b in index|format|exit) continue;; esac
    echo "M[\"$b\"] = m_$(echo "$b" | tr '/-' '__');"
  done
  echo "export default M;"
} > "$WSP_WORK/entry.js"
"$WSP_REPO"/node_modules/.pnpm/esbuild@0.27.7/node_modules/esbuild/bin/esbuild "$WSP_WORK/entry.js" --bundle --format=esm \
  --platform=node --outfile="$WSP_WORK/bundle.mjs" --log-level=warning --resolve-extensions=.ts,.js,.mjs

(cd "$P" && node "$WSP_WORK/calls.mjs" views/*.ts > "$WSP_WORK/calls-views.json" && node "$WSP_WORK/calls.mjs" ./*.ts > "$WSP_WORK/calls-top.json")

node "$WSP_WORK/gen.mjs" "$WSP_WORK/out" > /dev/null
for dir in views types; do
  for f in "$WSP_WORK"/out/$dir/*.rs; do
    dest=$CRATE/src/$dir/$(basename "$f")
    tail=""
    if [ -f "$dest" ] && grep -q '^// ---- Hand-written below' "$dest"; then
      tail=$(sed -n '/^\/\/ ---- Hand-written below/,$p' "$dest")
    fi
    cp "$f" "$dest"
    if [ -n "$tail" ]; then printf '\n%s\n' "$tail" >> "$dest"; fi
  done
  {
    echo "// SPDX-License-Identifier: AGPL-3.0-only"
    for f in "$CRATE"/src/$dir/*.rs; do
      m=$(basename "$f" .rs)
      [ "$m" = mod ] || echo "pub mod $m;"
    done
    if [ $dir = types ]; then echo; echo "pub use check::{parse, Check, SchemaError};"; echo "pub use js_number::JsNumber;"; fi
  } > "$CRATE/src/$dir/mod.rs"
done
node "$WSP_WORK/tsexpand.mjs" $(grep -l '^\s*// @ts ' "$CRATE"/src/views/*.rs "$CRATE"/src/types/*.rs 2>/dev/null) 2>/dev/null || true

node "$WSP_WORK/fixtures.mjs" "$WSP_WORK/fx" > /dev/null
node "$WSP_WORK/gentests.mjs" "$WSP_WORK/fx" "$CRATE"
node "$WSP_WORK/oracle.mjs" "$CRATE/tests/fixtures/rules/oracle.json" > /dev/null
node "$WSP_WORK/issues.mjs" "$CRATE/tests/fixtures/rules/issues.json" > /dev/null
rm -rf "$WSP_WORK"
