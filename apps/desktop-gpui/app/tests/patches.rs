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

/// The gpui-kit commit the script patches is the one the app builds against, so a bump in one place cannot leave the
/// other building an unpatched copy.
#[test]
fn the_patched_gpui_kit_is_the_pinned_one() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let script = std::fs::read_to_string(root.join("scripts/gpui-patches.sh")).unwrap();
    let pinned = script.lines().find_map(|l| l.strip_prefix("kit_rev=")).expect("kit_rev in gpui-patches.sh");
    let manifest = std::fs::read_to_string(root.join("app/Cargo.toml")).unwrap();
    let revs: Vec<&str> = manifest.split("rev = \"").skip(1).filter_map(|r| r.split('"').next()).collect();
    assert!(!revs.is_empty() && revs.iter().all(|r| *r == pinned), "app/Cargo.toml pins {revs:?}, the script patches {pinned}");
    let workspace = std::fs::read_to_string(root.join("Cargo.toml")).unwrap();
    assert!(workspace.contains(&format!("vendor/gpui-kit-{}", &pinned[..7])), "Cargo.toml points at another vendored gpui-kit");
}
