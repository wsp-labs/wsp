// SPDX-License-Identifier: AGPL-3.0-only
//! Our patches over gpui-pre (scripts/gpui-patches.sh) still apply to fresh copies of the pinned crates, and vendor/,
//! which the build uses, is exactly what they make.

use std::path::Path;
use std::process::Command;

#[test]
fn the_gpui_patches_apply_to_the_pinned_crates_and_match_vendor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut check = Command::new(root.join("scripts/gpui-patches.sh"));
    check.arg("check").env_clear();
    for name in ["PATH", "HOME", "CARGO_HOME", "TMPDIR"] {
        if let Some(value) = std::env::var_os(name) {
            check.env(name, value);
        }
    }
    let output = check.output().expect("scripts/gpui-patches.sh runs");
    let said = format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
    assert!(output.status.success(), "gpui-patches.sh check failed:\n{said}");
    assert!(said.contains("every patch applies, and vendor/ matches"), "{said}");
}
