// SPDX-License-Identifier: AGPL-3.0-only
// A test binary of its own: it moves the process's working folder, which every test in a binary shares.
use std::fs;
use std::path::Path;

use wsp_store::own_file::write_own;

#[test]
fn a_relative_root_climbed_out_of_lands_in_the_working_folder_as_node_joins_it() {
    let t = tempfile::tempdir().unwrap();
    std::env::set_current_dir(t.path()).unwrap();
    fs::create_dir("state").unwrap();
    write_own(Path::new("state"), "../y", "1").unwrap();
    assert_eq!(fs::read_to_string(t.path().join("y")).unwrap(), "1");
}
