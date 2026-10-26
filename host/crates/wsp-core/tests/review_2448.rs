// SPDX-License-Identifier: AGPL-3.0-only
//! Review of the deep-input round (wsp-labs/wsp-map#2448).
use futures::executor::block_on;

const NAME: &str = "a_tree_300_deep_with_files_beside_each_folder_is_removed_under_200_descriptors";

// reap.rs:641 remove_tree keeps each level's read_dir open while it walks the level below, so a tree 300 deep with files
// beside each folder needs 300 descriptors (the round's comment says none stays open per level). rmSync holds none.
#[test]
fn a_tree_300_deep_with_files_beside_each_folder_is_removed_under_200_descriptors() {
    if std::env::var("REVIEW_2448_CHILD").is_err() {
        let out = std::process::Command::new("sh")
            .args(["-c", "ulimit -n 200; exec \"$0\" \"$@\""])
            .arg(std::env::current_exe().unwrap())
            .args(["--exact", NAME, "--nocapture", "--test-threads=1"])
            .env("REVIEW_2448_CHILD", "1")
            .output()
            .unwrap();
        assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr).lines().find(|l| l.contains("left:")).unwrap_or(""));
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let mut at = root.path().join("r.d");
    for _ in 0..300 {
        std::fs::create_dir_all(&at).unwrap();
        for f in 0..3 {
            std::fs::write(at.join(format!("f{f}")), "x").unwrap();
        }
        at = at.join("d");
    }
    let base = root.path().join("r");
    assert_eq!(block_on(wsp_core::local_exec::reap::reap_run(base.to_str().unwrap(), None, 0)), Ok(()));
}
