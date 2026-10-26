// SPDX-License-Identifier: AGPL-3.0-only
// What the owner's state folder stands at under a wide umask (store.test.ts:577-594; rules 2.1, 2.3). Its own test
// binary, with one test, since the umask is the process's.
mod common;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use common::*;
use serde_json::json;
use wsp_store::sqlite_store::sqlite_store;
use wsp_store::store::Store;

fn mode(p: &Path) -> u32 {
    fs::metadata(p).unwrap().permissions().mode() & 0o777
}

#[test]
fn the_database_its_wal_and_the_folder_over_it_are_the_owners_alone_whatever_the_umask() {
    let over = Home::new();
    fs::set_permissions(over.dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    let home = over.dir.path().join("wsp");
    fs::create_dir(&home).unwrap();
    fs::set_permissions(&home, fs::Permissions::from_mode(0o755)).unwrap();
    let was = unsafe { libc::umask(0o022) };
    let store = sqlite_store(&home.join("state.json"), writer());
    let wrote = run(store.put("workspaces", "a", json!({"id": "a"})))
        .and_then(|_| run(store.put_blob("image-vaults", "default@v1", b"sign-ins".to_vec())));
    unsafe { libc::umask(was) };
    wrote.unwrap();
    assert_eq!(mode(&home), 0o700);
    assert_eq!(
        ["state.db", "state.db-wal", "state.db-shm"].map(|f| (f, mode(&home.join(f)))),
        [("state.db", 0o600), ("state.db-wal", 0o600), ("state.db-shm", 0o600)]
    );
    assert_eq!(mode(over.dir.path()), 0o755, "nothing above the state folder is touched");
}
