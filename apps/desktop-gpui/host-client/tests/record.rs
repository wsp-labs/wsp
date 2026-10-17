// SPDX-License-Identifier: AGPL-3.0-only
//! The types this crate generates are the protocol's: record/protocol.json is what record.mjs writes off the zod
//! schemas in packages/protocol now, byte for byte. Needs node and the protocol package built (pnpm build).

use std::path::Path;
use std::process::Command;

#[test]
fn the_record_is_what_the_protocol_says_now() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let protocol = here.join("../../..").canonicalize().unwrap().join("packages/protocol/dist/index.js");
    assert!(protocol.exists(), "{} is not built: run pnpm install && pnpm build", protocol.display());
    let out = Command::new("node").arg(here.join("record.mjs")).arg("--check").output().expect("node on PATH");
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
