#!/bin/bash
set -euo pipefail
NAME="wsp"
for f in *.txt; do
  echo "$f: ${NAME}" | grep -q x && exit 1
done
if [ -n "$1" ]; then cd "$1"; fi
