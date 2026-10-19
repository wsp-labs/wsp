#!/usr/bin/env bash
# SPDX-License-Identifier: AGPL-3.0-only
# road1.sh: builds the spike's gpui-kit road (feature kit) inside a pristine copy of gpui-kit's own workspace at our
# pin, with gpui-kit's own Cargo.lock. Our patched gpui-kit in vendor/ does not compile on GPUI Fast, which the webview
# forces on, so this road can only be judged without our patches. Prints the binary's path last.
set -eu

here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
rev=14f59330a4aa0cb5becf140e067d64a616328612
ws=$target/road1-kit

unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE
if [ "$(git -C "$ws" rev-parse HEAD 2>/dev/null)" != "$rev" ]; then
  rm -rf "$ws"
  git init -q "$ws"
  checkout=
  for cached in "${CARGO_HOME:-$HOME/.cargo}"/git/checkouts/gpui-kit-*/"${rev:0:7}"; do
    [ "$(git -C "$cached" rev-parse HEAD 2>/dev/null)" = "$rev" ] && checkout=$cached
  done
  git -C "$ws" fetch -q --depth 1 "${checkout:-https://github.com/longbridge/gpui-kit}" "$rev"
  git -C "$ws" checkout -q FETCH_HEAD
fi

member=$ws/examples/browser-spike
mkdir -p "$member"
ln -sfn "$here/src" "$member/src"
cat > "$member/Cargo.toml" <<'EOF'
[package]
name = "browser-spike-kit"
version = "0.1.0"
publish = false
edition.workspace = true

[features]
default = ["kit"]
kit = []
wry = []

[dependencies]
gpui-kit.workspace = true
gpui-webview = { path = "../../crates/webview" }
lb-wry = "0.53.3"
EOF
if ! grep -q '"examples/browser-spike"' "$ws/Cargo.toml"; then
  awk '{ print } /^members = \[/ { print "    \"examples/browser-spike\"," }' "$ws/Cargo.toml" > "$ws/Cargo.toml.new"
  mv "$ws/Cargo.toml.new" "$ws/Cargo.toml"
fi

(cd "$ws" && CARGO_TARGET_DIR=$target cargo build --release -p browser-spike-kit "$@")
echo "$target/release/browser-spike-kit"
