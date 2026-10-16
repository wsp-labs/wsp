// SPDX-License-Identifier: AGPL-3.0-only
//! The update road under a put folder a home's local threads write: a link, a fifo or a file past the cap there is
//! refused with a line, never followed, waited on or read whole.

use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use super::*;

/// The call on its own thread, and what it answered inside a second; None where it was still running then.
fn within_a_second<T: Send + 'static>(call: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || drop(tx.send(call())));
    rx.recv_timeout(Duration::from_secs(1)).ok()
}

fn fifo_at(path: &Path) {
    nix::unistd::mkfifo(path, nix::sys::stat::Mode::from_bits_truncate(0o644)).unwrap();
}

/// A waiting open of the fifo let go by a writer that opens it and closes it at once.
fn free(fifo: &Path) {
    let _freed = nix::fcntl::open(fifo, nix::fcntl::OFlag::O_WRONLY | nix::fcntl::OFlag::O_NONBLOCK, nix::sys::stat::Mode::empty());
}

/// An upload with part 0 landed, so the next part reads the seq beside it.
fn upload_begun(home: &Path) -> PathBuf {
    let part = update_part(home, "u1");
    take_update_part(home, "u1", 0, b"new ").unwrap();
    part
}

#[test]
fn a_fifo_at_the_seq_refuses_the_part_at_once() {
    let home = tempfile::tempdir().unwrap();
    let part = upload_begun(home.path());
    std::fs::remove_file(update_next(&part)).unwrap();
    fifo_at(&update_next(&part));
    let at = home.path().to_path_buf();
    let taken = within_a_second(move || take_update_part(&at, "u1", 1, b"daemon"));
    free(&update_next(&part));
    let refused = taken.expect("the part is still waiting on a fifo at the seq after 1s").unwrap_err();
    assert!(refused.contains("is not a file"), "{refused}");
    assert!(!part.exists(), "a refused part left the upload on disk");
}

#[test]
fn a_link_to_dev_zero_at_the_seq_refuses_the_part_at_once() {
    let home = tempfile::tempdir().unwrap();
    let part = upload_begun(home.path());
    std::fs::remove_file(update_next(&part)).unwrap();
    std::os::unix::fs::symlink("/dev/zero", update_next(&part)).unwrap();
    let at = home.path().to_path_buf();
    let taken = within_a_second(move || take_update_part(&at, "u1", 1, b"daemon"));
    let refused = taken.expect("the part is still reading a link to /dev/zero at the seq after 1s").unwrap_err();
    assert!(refused.contains("a link stands at"), "{refused}");
}

#[test]
fn a_link_out_at_the_seq_refuses_the_part_even_to_the_seq_it_waits_for() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let part = upload_begun(home.path());
    let seq = elsewhere.path().join("seq");
    std::fs::write(&seq, "1").unwrap();
    std::fs::remove_file(update_next(&part)).unwrap();
    std::os::unix::fs::symlink(&seq, update_next(&part)).unwrap();
    let refused = take_update_part(home.path(), "u1", 1, b"daemon").unwrap_err();
    let put = place_daemon_paths(home.path()).put_dir;
    assert!(refused.contains(&format!("{}: a link stands at u1.next", put.display())), "{refused}");
    assert_eq!(std::fs::read(&seq).unwrap(), b"1", "the seq was written through the link");
}

#[test]
fn a_link_out_at_the_part_is_never_appended_to() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let part = upload_begun(home.path());
    let theirs = elsewhere.path().join("bashrc");
    std::fs::write(&theirs, "theirs\n").unwrap();
    std::fs::remove_file(&part).unwrap();
    std::os::unix::fs::symlink(&theirs, &part).unwrap();
    let refused = take_update_part(home.path(), "u1", 1, b"daemon").unwrap_err();
    assert!(refused.contains("a link stands at"), "{refused}");
    assert_eq!(std::fs::read(&theirs).unwrap(), b"theirs\n", "the part's bytes were appended through the link");
}

#[test]
fn an_install_refuses_a_link_at_the_part_at_once_even_to_the_bytes_it_names() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let exe = home.path().join("wsp-daemon");
    std::fs::write(&exe, b"old daemon").unwrap();
    let part = update_part(home.path(), "u1");
    std::fs::create_dir_all(part.parent().unwrap()).unwrap();

    let whole = elsewhere.path().join("daemon");
    std::fs::write(&whole, b"new daemon").unwrap();
    std::os::unix::fs::symlink(&whole, &part).unwrap();
    let refused = install_daemon(&exe, home.path(), "u1", &sha256_hex(b"new daemon")).unwrap_err();
    assert!(refused.contains("a link stands at"), "{refused}");
    assert_eq!(std::fs::read(&exe).unwrap(), b"old daemon", "a link at the part was installed");

    std::fs::remove_file(&part).unwrap();
    std::os::unix::fs::symlink("/dev/zero", &part).unwrap();
    let (at, sha) = (exe.clone(), sha256_hex(b"new daemon"));
    let installed = within_a_second({
        let root = home.path().to_path_buf();
        move || install_daemon(&at, &root, "u1", &sha)
    });
    assert!(installed.expect("the install is still reading a link to /dev/zero after 1s").is_err());
    assert_eq!(std::fs::read(&exe).unwrap(), b"old daemon");
}

#[test]
fn an_install_refuses_a_link_or_a_fifo_beside_the_binary_at_once_and_the_binary_stays() {
    for beside in ["new", "old"] {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let exe = home.path().join("wsp-daemon");
        std::fs::write(&exe, b"old daemon").unwrap();
        take_update_part(home.path(), "u1", 0, b"new daemon").unwrap();
        let theirs = elsewhere.path().join("bashrc");
        std::fs::write(&theirs, "theirs\n").unwrap();
        std::os::unix::fs::symlink(&theirs, exe.with_extension(beside)).unwrap();
        let refused = install_daemon(&exe, home.path(), "u1", &sha256_hex(b"new daemon")).unwrap_err();
        assert!(refused.contains("a link stands at"), "{beside}: {refused}");
        assert_eq!(std::fs::read(&theirs).unwrap(), b"theirs\n", "{beside}: the install wrote through the link");
        assert_eq!(std::fs::read(&exe).unwrap(), b"old daemon", "{beside}");

        std::fs::remove_file(exe.with_extension(beside)).unwrap();
        fifo_at(&exe.with_extension(beside));
        let (at, sha) = (exe.clone(), sha256_hex(b"new daemon"));
        let installed = within_a_second({
            let root = home.path().to_path_buf();
            move || install_daemon(&at, &root, "u1", &sha)
        });
        // A write still waiting on the fifo's open is let go by a reader that opens it and closes it at once.
        let _freed = nix::fcntl::open(
            &exe.with_extension(beside),
            nix::fcntl::OFlag::O_RDONLY | nix::fcntl::OFlag::O_NONBLOCK,
            nix::sys::stat::Mode::empty(),
        );
        assert!(installed.expect("the install is still waiting on a fifo beside the binary after 1s").is_err(), "{beside}");
        assert_eq!(std::fs::read(&exe).unwrap(), b"old daemon", "{beside}");
    }
}

#[test]
fn a_sweep_follows_no_link_at_the_put_folder_or_in_it_and_takes_only_plain_files() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let theirs = elsewhere.path().join("notes.txt");
    std::fs::write(&theirs, "theirs\n").unwrap();
    let put = place_daemon_paths(home.path()).put_dir;
    std::fs::create_dir_all(put.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), &put).unwrap();
    assert_eq!(sweep_updates(home.path(), None), 0);
    assert_eq!(std::fs::read(&theirs).unwrap(), b"theirs\n", "the sweep emptied the folder a link at put names");

    // A link at ~/.wsp names a folder whose own put folder would be emptied.
    let wsp = put.parent().unwrap();
    std::fs::remove_file(&put).unwrap();
    std::fs::remove_dir(wsp).unwrap();
    let theirs_put = elsewhere.path().join("put");
    std::fs::create_dir(&theirs_put).unwrap();
    std::fs::write(theirs_put.join("notes.txt"), "theirs\n").unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), wsp).unwrap();
    assert_eq!(sweep_updates(home.path(), None), 0);
    assert!(theirs_put.join("notes.txt").exists(), "the sweep emptied a put folder under the folder a link at ~/.wsp names");

    std::fs::remove_file(wsp).unwrap();
    let part = upload_begun(home.path());
    std::os::unix::fs::symlink(&theirs, put.join("u2")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), put.join("u3")).unwrap();
    fifo_at(&put.join("u4"));
    assert_eq!(sweep_updates(home.path(), None), 2, "the part and the seq beside it");
    assert!(!part.exists() && !update_next(&part).exists());
    for name in ["u2", "u3", "u4"] {
        assert!(std::fs::symlink_metadata(put.join(name)).is_ok(), "{name} is not a plain file the sweep made");
    }
    assert_eq!(std::fs::read(&theirs).unwrap(), b"theirs\n");
}

#[test]
fn an_update_takes_only_the_part_it_waits_for_and_moves_the_whole_over_the_running_binary() {
    let home = tempfile::tempdir().unwrap();
    let part = update_part(home.path(), "u1");
    assert_eq!(part, place_daemon_paths(home.path()).put_dir.join("u1"));

    // An upload waits for part 0 first, so a skip to any other part is refused and lands nothing.
    assert_eq!(take_update_part(home.path(), "u1", 1, b"x").unwrap_err(), words::update_out_of_order(1, 0, "u1"));
    assert!(!part.exists());
    take_update_part(home.path(), "u1", 0, b"new ").unwrap();

    // From then on it waits for the one after the last it took: a repeat of the part already landed is refused
    // rather than appended twice, and a skip over the one it wants is refused rather than leaving a hole.
    assert_eq!(take_update_part(home.path(), "u1", 0, b"x").unwrap_err(), words::update_out_of_order(0, 1, "u1"));
    assert!(!part.exists(), "a refused part left the upload on disk");
    take_update_part(home.path(), "u1", 0, b"new ").unwrap();
    assert_eq!(take_update_part(home.path(), "u1", 2, b"x").unwrap_err(), words::update_out_of_order(2, 1, "u1"));
    take_update_part(home.path(), "u1", 0, b"new ").unwrap();
    take_update_part(home.path(), "u1", 1, b"daemon").unwrap();
    assert_eq!(std::fs::read(&part).unwrap(), b"new daemon");

    let exe = home.path().join("wsp-daemon");
    std::fs::write(&exe, b"old daemon").unwrap();
    let wrong = "0".repeat(64);
    let refused = install_daemon(&exe, home.path(), "u1", &wrong).unwrap_err();
    assert_eq!(refused, words::update_bytes_differ("u1", &wrong, &sha256_hex(b"new daemon")));
    assert_eq!(std::fs::read(&exe).unwrap(), b"old daemon", "a binary whose bytes differ was moved anyway");
    assert!(!part.exists(), "the dropped upload stays on disk");

    take_update_part(home.path(), "u1", 0, b"new daemon").unwrap();
    let (at, kept) = install_daemon(&exe, home.path(), "u1", &sha256_hex(b"new daemon")).unwrap();
    assert_eq!(at, exe.to_string_lossy());
    assert_eq!(kept, exe.with_extension("old").to_string_lossy());
    assert_eq!(std::fs::read(&exe).unwrap(), b"new daemon");
    assert_eq!(std::fs::read(exe.with_extension("old")).unwrap(), b"old daemon");
    assert!(!exe.with_extension("new").exists(), "the staged copy stays beside the binary");
    // Both the bytes and the seq beside them go with the landing, so the next upload starts from nothing.
    assert!(!part.exists() && !update_next(&part).exists());
    let mode: u32 = std::os::unix::fs::PermissionsExt::mode(&std::fs::metadata(&exe).unwrap().permissions());
    assert_eq!(mode & 0o777, 0o755);
}

#[test]
fn the_parts_a_dropped_link_left_go_when_the_agent_starts_and_when_the_next_upload_begins() {
    let home = tempfile::tempdir().unwrap();
    let abandoned = update_part(home.path(), "aa11");
    take_update_part(home.path(), "aa11", 0, b"half a daemon").unwrap();
    assert!(abandoned.exists() && update_next(&abandoned).exists());

    // The host picks a fresh id for its next try, so nothing will ever name that one again.
    let fresh = update_part(home.path(), "bb22");
    take_update_part(home.path(), "bb22", 0, b"a whole one").unwrap();
    assert_eq!(sweep_updates(home.path(), Some("bb22")), 2, "the abandoned upload and the seq beside it");
    assert!(!abandoned.exists() && !update_next(&abandoned).exists());
    assert!(fresh.exists(), "the upload arriving now went with them");

    // At the agent's start nothing is arriving, so the lot goes.
    assert_eq!(sweep_updates(home.path(), None), 2);
    assert!(!fresh.exists());
    // A place whose folder is not there at all is not an error.
    assert_eq!(sweep_updates(&home.path().join("nowhere"), None), 0);
}

#[test]
fn the_binary_this_daemon_runs_is_its_own_path_with_the_kernels_deleted_mark_taken_off() {
    assert_eq!(running_daemon(Ok(PathBuf::from("/h/.wsp/daemon/wsp-daemon"))).unwrap(), PathBuf::from("/h/.wsp/daemon/wsp-daemon"));
    // What /proc/self/exe reads once an update has renamed a new binary over the running one's path.
    assert_eq!(
        running_daemon(Ok(PathBuf::from("/h/.wsp/daemon/wsp-daemon (deleted)"))).unwrap(),
        PathBuf::from("/h/.wsp/daemon/wsp-daemon")
    );
    assert!(running_daemon(Err(std::io::Error::other("no /proc"))).unwrap_err().contains("which file it runs from"));
}

#[test]
fn a_dropped_upload_takes_nothing_a_link_at_the_put_folder_names() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let put = place_daemon_paths(home.path()).put_dir;
    std::fs::create_dir_all(put.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), &put).unwrap();
    let theirs = elsewhere.path().join("u1");
    std::fs::write(&theirs, "theirs\n").unwrap();
    // Bytes with no seq beside them drop the upload, and the drop takes both names off.
    assert!(take_update_part(home.path(), "u1", 1, b"daemon").is_err());
    assert!(theirs.exists(), "a dropped upload removed a file through a link at put");
}

#[test]
fn a_dropped_upload_takes_nothing_a_link_at_the_wsp_folder_names() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(elsewhere.path().join("put")).unwrap();
    let theirs = elsewhere.path().join("put/u1");
    std::fs::write(&theirs, "theirs\n").unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), home.path().join(".wsp")).unwrap();
    assert!(take_update_part(home.path(), "u1", 1, b"daemon").is_err());
    assert!(theirs.exists(), "a dropped upload removed a file through a link at ~/.wsp");
}

#[test]
fn a_landing_refuses_a_link_swapped_in_at_the_wsp_folder_and_takes_nothing_it_names() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let exe = home.path().join("wsp-daemon");
    std::fs::write(&exe, b"old daemon").unwrap();
    take_update_part(home.path(), "u1", 0, b"new daemon").unwrap();
    // The thread moves the real ~/.wsp aside and leaves a link to a folder of the person's in its place.
    std::fs::create_dir_all(elsewhere.path().join("put")).unwrap();
    std::fs::write(elsewhere.path().join("put/u1"), b"new daemon").unwrap();
    std::fs::write(elsewhere.path().join("put/u1.next"), b"1").unwrap();
    std::fs::rename(home.path().join(".wsp"), home.path().join(".wsp-aside")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), home.path().join(".wsp")).unwrap();
    assert!(install_daemon(&exe, home.path(), "u1", &sha256_hex(b"new daemon")).is_err());
    assert_eq!(std::fs::read(&exe).unwrap(), b"old daemon");
    assert!(elsewhere.path().join("put/u1").exists(), "a landed upload removed a file through a link at ~/.wsp");
}

#[test]
fn an_upload_writes_nothing_into_the_folder_a_link_at_put_names() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let put = place_daemon_paths(home.path()).put_dir;
    std::fs::create_dir_all(put.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), &put).unwrap();
    let taken = take_update_part(home.path(), "u1", 0, b"daemon");
    let names: Vec<_> = std::fs::read_dir(elsewhere.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert!(names.is_empty(), "the upload wrote {names:?} into the folder a link at put names ({taken:?})");
}

#[test]
fn an_upload_writes_nothing_into_the_folder_a_link_at_the_wsp_folder_names() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), home.path().join(".wsp")).unwrap();
    let taken = take_update_part(home.path(), "u1", 0, b"daemon");
    let names: Vec<_> = std::fs::read_dir(elsewhere.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert!(names.is_empty(), "the upload wrote {names:?} into the folder a link at ~/.wsp names ({taken:?})");
}

#[test]
fn a_landing_writes_nothing_into_the_folder_a_link_at_the_binarys_folder_names() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let exe = place_daemon_paths(home.path()).dir.join("wsp-daemon");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    std::fs::write(&exe, b"old daemon").unwrap();
    take_update_part(home.path(), "u1", 0, b"new daemon").unwrap();
    // The binary's folder moved aside after the start and a link to a folder of the person's left in its place.
    std::fs::write(elsewhere.path().join("wsp-daemon"), "theirs\n").unwrap();
    std::fs::rename(exe.parent().unwrap(), home.path().join(".wsp/daemon-aside")).unwrap();
    std::os::unix::fs::symlink(elsewhere.path(), exe.parent().unwrap()).unwrap();
    let refused = install_daemon(&exe, home.path(), "u1", &sha256_hex(b"new daemon")).unwrap_err();
    assert!(refused.contains(&format!("{} is not there as a folder: a link", exe.parent().unwrap().display())), "{refused}");
    let names: Vec<_> = std::fs::read_dir(elsewhere.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["wsp-daemon"], "the landing wrote beside a binary a link names");
    assert_eq!(std::fs::read(elsewhere.path().join("wsp-daemon")).unwrap(), b"theirs\n");
}

#[test]
fn a_landing_copies_nothing_a_link_at_the_binarys_name_points_at() {
    let home = tempfile::tempdir().unwrap();
    let elsewhere = tempfile::tempdir().unwrap();
    let exe = place_daemon_paths(home.path()).dir.join("wsp-daemon");
    std::fs::create_dir_all(exe.parent().unwrap()).unwrap();
    take_update_part(home.path(), "u1", 0, b"new daemon").unwrap();
    let secret = elsewhere.path().join("id_ed25519");
    std::fs::write(&secret, "theirs\n").unwrap();
    std::os::unix::fs::symlink(&secret, &exe).unwrap();
    let refused = install_daemon(&exe, home.path(), "u1", &sha256_hex(b"new daemon")).unwrap_err();
    assert!(refused.contains(&format!("{}: a link stands at wsp-daemon", exe.parent().unwrap().display())), "{refused}");
    assert!(!exe.with_extension("old").exists() && !exe.with_extension("new").exists(), "the refused landing wrote beside the binary");
}

#[test]
fn an_upload_refused_by_a_plain_file_at_the_wsp_folder_says_so_and_blames_no_link() {
    let home = tempfile::tempdir().unwrap();
    let put = place_daemon_paths(home.path()).put_dir;
    std::fs::write(put.parent().unwrap(), "a file of the person's\n").unwrap();
    let refused = take_update_part(home.path(), "u1", 0, b"daemon").unwrap_err();
    assert!(refused.starts_with(&format!("{} is not there as a folder: ", put.display())), "{refused}");
    assert!(refused.contains("a file stands on the way to it") && !refused.contains("a link stands"), "{refused}");
}
