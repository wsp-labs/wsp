// SPDX-License-Identifier: AGPL-3.0-only
// The one writer under the owner's state folder, against what packages/own-file does on disk (spec/own_file.md).
use std::fs;
use std::os::unix::fs::{symlink, PermissionsExt};
use std::path::Path;

use wsp_store::own_file::{own_folder, write_own};

fn mode(p: &Path) -> u32 {
    fs::symlink_metadata(p).unwrap().permissions().mode() & 0o777
}

fn chmod(p: &Path, m: u32) {
    fs::set_permissions(p, fs::Permissions::from_mode(m)).unwrap();
}

fn names(dir: &Path) -> Vec<String> {
    let mut out: Vec<String> = fs::read_dir(dir).unwrap().map(|e| e.unwrap().file_name().to_string_lossy().into_owned()).collect();
    out.sort();
    out
}

#[test]
fn own_folder_makes_a_missing_folder_and_its_parents() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("a/b/c");
    own_folder(&dir).unwrap();
    assert!(dir.is_dir());
    assert_eq!(mode(&dir), 0o700);
}

#[test]
fn own_folder_repairs_a_folder_an_older_build_left_wider() {
    let t = tempfile::tempdir().unwrap();
    let dir = t.path().join("state");
    fs::create_dir(&dir).unwrap();
    chmod(&dir, 0o755);
    fs::write(dir.join("kept"), "x").unwrap();
    own_folder(&dir).unwrap();
    assert_eq!(mode(&dir), 0o700);
    assert_eq!(fs::read_to_string(dir.join("kept")).unwrap(), "x");
}

#[test]
fn own_folder_repairs_only_the_folder_named_not_a_parent_that_stood() {
    let t = tempfile::tempdir().unwrap();
    let parent = t.path().join("home");
    fs::create_dir(&parent).unwrap();
    chmod(&parent, 0o755);
    own_folder(&parent.join("state")).unwrap();
    assert_eq!(mode(&parent), 0o755);
    assert_eq!(mode(&parent.join("state")), 0o700);
}

#[test]
fn write_own_writes_the_bytes_at_0600_under_folders_at_0700() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("state");
    write_own(&root, "places/p1/link.json", "{\"a\":1}").unwrap();
    let file = root.join("places/p1/link.json");
    assert_eq!(fs::read_to_string(&file).unwrap(), "{\"a\":1}");
    assert_eq!(mode(&file), 0o600);
    assert_eq!(mode(&root), 0o700);
    assert_eq!(mode(&root.join("places")), 0o700);
    assert_eq!(mode(&root.join("places/p1")), 0o700);
}

#[test]
fn write_own_takes_bytes_as_well_as_text() {
    let t = tempfile::tempdir().unwrap();
    write_own(t.path(), "blob", [0u8, 255, 10]).unwrap();
    assert_eq!(fs::read(t.path().join("blob")).unwrap(), vec![0u8, 255, 10]);
}

#[test]
fn a_file_that_stood_at_0644_is_replaced_and_its_old_mode_goes_with_it() {
    let t = tempfile::tempdir().unwrap();
    let file = t.path().join("host-token");
    fs::write(&file, "old").unwrap();
    chmod(&file, 0o644);
    write_own(t.path(), "host-token", "new").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), "new");
    assert_eq!(mode(&file), 0o600);
}

#[test]
fn folders_between_root_and_the_file_that_stood_wider_are_repaired_and_nothing_above_root_is() {
    let t = tempfile::tempdir().unwrap();
    chmod(t.path(), 0o755);
    let root = t.path().join("state");
    fs::create_dir_all(root.join("logins")).unwrap();
    chmod(&root, 0o755);
    chmod(&root.join("logins"), 0o750);
    write_own(&root, "logins/x", "1").unwrap();
    assert_eq!(mode(&root), 0o700);
    assert_eq!(mode(&root.join("logins")), 0o700);
    assert_eq!(mode(t.path()), 0o755);
}

#[test]
fn the_write_leaves_no_temporary_file_beside_the_target() {
    let t = tempfile::tempdir().unwrap();
    write_own(t.path(), "a.json", "1").unwrap();
    write_own(t.path(), "a.json", "2").unwrap();
    assert_eq!(names(t.path()), vec!["a.json"]);
    assert_eq!(fs::read_to_string(t.path().join("a.json")).unwrap(), "2");
}

#[test]
fn a_link_at_the_target_is_replaced_by_a_file_and_what_it_pointed_at_stands() {
    let t = tempfile::tempdir().unwrap();
    let elsewhere = t.path().join("elsewhere");
    fs::write(&elsewhere, "theirs").unwrap();
    let root = t.path().join("state");
    fs::create_dir(&root).unwrap();
    symlink(&elsewhere, root.join("token")).unwrap();
    write_own(&root, "token", "mine").unwrap();
    let meta = fs::symlink_metadata(root.join("token")).unwrap();
    assert!(meta.file_type().is_file());
    assert_eq!(fs::read_to_string(root.join("token")).unwrap(), "mine");
    assert_eq!(fs::read_to_string(&elsewhere).unwrap(), "theirs");
}

#[test]
fn rel_is_joined_as_node_joins_a_leading_slash_staying_under_root_and_dot_dot_resolved() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("state");
    write_own(&root, "/abs/x.json", "1").unwrap();
    assert_eq!(fs::read_to_string(root.join("abs/x.json")).unwrap(), "1");
    assert_eq!(mode(&root.join("abs")), 0o700);
    write_own(&root, "a/../y.json", "2").unwrap();
    assert_eq!(fs::read_to_string(root.join("y.json")).unwrap(), "2");
    assert!(!root.join("a").exists());
}

#[test]
fn a_rel_that_climbs_out_of_root_writes_there_and_repairs_the_folders_it_passes() {
    let t = tempfile::tempdir().unwrap();
    chmod(t.path(), 0o755);
    let root = t.path().join("state");
    fs::create_dir(&root).unwrap();
    let side = t.path().join("side");
    fs::create_dir(&side).unwrap();
    chmod(&side, 0o755);
    write_own(&root, "../side/z", "4").unwrap();
    assert_eq!(fs::read_to_string(side.join("z")).unwrap(), "4");
    assert_eq!(mode(&side), 0o700);
    assert_eq!(mode(t.path()), 0o700);
}

#[test]
fn an_empty_rel_names_root_itself_and_fails() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("state");
    fs::create_dir(&root).unwrap();
    assert!(write_own(&root, "", "3").is_err());
    assert!(root.is_dir());
}

#[test]
fn a_rel_with_a_trailing_slash_names_a_folder_and_writes_no_file() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("state");
    assert!(write_own(&root, "x/", "1").is_err());
    assert!(!root.join("x").exists());
}

#[test]
fn a_dangling_link_on_the_way_fails_as_not_found_and_writes_nothing() {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("state");
    fs::create_dir(&root).unwrap();
    symlink(t.path().join("nowhere"), root.join("sub")).unwrap();
    assert_eq!(write_own(&root, "sub/f", "1").unwrap_err().kind(), std::io::ErrorKind::NotFound);
    assert!(!t.path().join("nowhere").exists());
}
