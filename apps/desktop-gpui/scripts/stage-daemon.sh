#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# Stages a real wsp-daemon for the terminal's tests and captures as CI's flows job does (.github/workflows/flows.yml),
# at packages/wspx/daemon/<triple>/wsp-daemon, where tests/common's wsp_daemon() and the host's asset table read it,
# and builds the host and the web app a throwaway host serves. Linux only; each step is skipped once it is done.
set -eu

cd "$(dirname "$0")/../../.."
triple="$(uname -m)-unknown-linux-musl"

for tool in musl-gcc gperf; do
  command -v "$tool" >/dev/null || { echo "refused: $tool is missing; apt-get install -y musl-tools gperf" >&2; exit 1; }
done

(cd daemon && ./scripts/libseccomp-archive.sh musl)
(cd daemon && LIBSECCOMP_LIB_PATH=$PWD/target/libseccomp/musl cargo build --locked --release --target "$triple" -p wsp-daemon-bin)
# Cargo reads a relative CARGO_TARGET_DIR from daemon/, where it runs.
built=${CARGO_TARGET_DIR:-target}
case $built in /*) ;; *) built=daemon/$built ;; esac
node packages/wspx/scripts/daemon-binary.mjs --triple "$triple" --from "$built/$triple/release/wsp-daemon"

[ -d node_modules ] || pnpm install --frozen-lockfile
[ -f packages/host/dist/bin.js ] && [ -f apps/web/dist/index.html ] || pnpm --filter @wsp/host... --filter @wsp/web... build
echo "staged: packages/wspx/daemon/$triple/wsp-daemon; a host from tests/common's Throwaway::start_with(_, true) carries it"
