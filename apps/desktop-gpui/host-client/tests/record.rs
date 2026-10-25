// SPDX-License-Identifier: AGPL-3.0-only
//! The parity with what the host and the web app say now: record/protocol.json is what record.mjs writes off the zod
//! schemas in packages/protocol and the ops apps/web sends, byte for byte, so a field the protocol moves or an op the
//! web app starts sending fails here until the record is written again, and then the lib's
//! every_op_the_web_app_sends_has_a_call fails until the op has its call. Needs node and the packages built (pnpm build).

use std::path::Path;
use std::process::Command;

#[test]
fn the_record_is_what_the_protocol_and_the_web_app_say_now() {
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let protocol = here.join("../../..").canonicalize().unwrap().join("packages/protocol/dist/index.js");
    assert!(protocol.exists(), "{} is not built: run pnpm install && pnpm build", protocol.display());
    let out = Command::new("node").arg(here.join("record.mjs")).arg("--check").output().expect("node on PATH");
    assert!(out.status.success(), "{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr));
}
