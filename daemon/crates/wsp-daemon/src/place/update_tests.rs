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
    take_update_part(&part, 0, b"new ", "u1").unwrap();
    part
}

#[test]
fn a_fifo_at_the_seq_refuses_the_part_at_once() {
    let home = tempfile::tempdir().unwrap();
    let part = upload_begun(home.path());
    std::fs::remove_file(update_next(&part)).unwrap();
    fifo_at(&update_next(&part));
    let at = part.clone();
    let taken = within_a_second(move || take_update_part(&at, 1, b"daemon", "u1"));
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
    let at = part.clone();
    let taken = within_a_second(move || take_update_part(&at, 1, b"daemon", "u1"));
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
    let refused = take_update_part(&part, 1, b"daemon", "u1").unwrap_err();
    assert!(refused.contains("a link stands at"), "{refused}");
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
    let refused = take_update_part(&part, 1, b"daemon", "u1").unwrap_err();
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
    let refused = install_daemon(&exe, &part, &sha256_hex(b"new daemon"), "u1").unwrap_err();
    assert!(refused.contains("a link stands at"), "{refused}");
    assert_eq!(std::fs::read(&exe).unwrap(), b"old daemon", "a link at the part was installed");

    std::fs::remove_file(&part).unwrap();
    std::os::unix::fs::symlink("/dev/zero", &part).unwrap();
    let (at, sha) = (exe.clone(), sha256_hex(b"new daemon"));
    let installed = within_a_second(move || install_daemon(&at, &part, &sha, "u1"));
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
        let part = update_part(home.path(), "u1");
        take_update_part(&part, 0, b"new daemon", "u1").unwrap();
        let theirs = elsewhere.path().join("bashrc");
        std::fs::write(&theirs, "theirs\n").unwrap();
        std::os::unix::fs::symlink(&theirs, exe.with_extension(beside)).unwrap();
        let refused = install_daemon(&exe, &part, &sha256_hex(b"new daemon"), "u1").unwrap_err();
        assert!(refused.contains("a link stands at"), "{beside}: {refused}");
        assert_eq!(std::fs::read(&theirs).unwrap(), b"theirs\n", "{beside}: the install wrote through the link");
        assert_eq!(std::fs::read(&exe).unwrap(), b"old daemon", "{beside}");

        std::fs::remove_file(exe.with_extension(beside)).unwrap();
        fifo_at(&exe.with_extension(beside));
        let (at, sha) = (exe.clone(), sha256_hex(b"new daemon"));
        let installed = within_a_second(move || install_daemon(&at, &part, &sha, "u1"));
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
