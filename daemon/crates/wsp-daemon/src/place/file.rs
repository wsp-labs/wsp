// SPDX-License-Identifier: AGPL-3.0-only
//! The place file and its key, read at every dial out of a home a workspace on a computer somebody owns writes. The
//! daemon runs one thread, so a read that waits holds every task it serves.

use std::path::Path;

use wsp_frames::{numbers, PlaceFile};

use crate::beneath;

/// The place file as it stands, or nothing when this computer is no place: a file that is there and is not one
/// reads the same as none, since the one road that writes it is wsp join.
pub(crate) fn read_place_file(path: &Path) -> Option<PlaceFile> {
    PlaceFile::parse(&read_plain(path)?)
}

/// The key the place file names, or nothing where no plain file under the cap stands there.
pub(crate) fn read_place_key(path: &Path) -> Option<String> {
    read_plain(path)
}

/// A plain file's text, no further than the cap, with no link followed at its name and no wait on a fifo, as the
/// host's leave reads the same file.
fn read_plain(path: &Path) -> Option<String> {
    String::from_utf8(beneath::read_capped(path, numbers::PLACE_READ_MAX_BYTES).ok()??).ok()
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    const PLACE_TEXT: &str = r#"{"placeId":"p_ab12cd34","name":"old-macbook","hostName":"lab-mbp","hostUrls":["http://192.168.1.20:4400"],"hostPublicKey":"MCow","keyPath":"/h/.wsp/place-key.pem","joinedAt":"1970-01-01T00:00:00.000Z"}"#;

    /// The read on its own thread, and what it answered inside a second; None where it was still reading then.
    fn read_within_a_second(path: &Path) -> Option<Option<PlaceFile>> {
        let (tx, rx) = mpsc::channel();
        let at = path.to_owned();
        std::thread::spawn(move || drop(tx.send(read_place_file(&at))));
        rx.recv_timeout(Duration::from_secs(1)).ok()
    }

    fn fifo_at(path: &PathBuf) {
        nix::unistd::mkfifo(path, nix::sys::stat::Mode::from_bits_truncate(0o644)).unwrap();
    }

    #[test]
    fn a_fifo_at_the_place_file_reads_as_none_at_once() {
        let home = tempfile::tempdir().unwrap();
        let fifo = home.path().join("place.json");
        fifo_at(&fifo);
        let read = read_within_a_second(&fifo);
        // A read still waiting on the fifo's open is let go by a writer that opens it and closes it at once.
        let _freed = nix::fcntl::open(&fifo, nix::fcntl::OFlag::O_WRONLY | nix::fcntl::OFlag::O_NONBLOCK, nix::sys::stat::Mode::empty());
        assert!(read.is_some(), "the read of a fifo at the place file is still waiting after 1s");
        assert!(read.unwrap().is_none());
    }

    #[test]
    fn a_fifo_or_a_link_at_the_key_reads_as_none_at_once_and_a_key_reads() {
        let home = tempfile::tempdir().unwrap();
        let key = home.path().join("place-key.pem");
        std::fs::write(&key, "-----BEGIN PRIVATE KEY-----\n").unwrap();
        assert_eq!(read_place_key(&key).as_deref(), Some("-----BEGIN PRIVATE KEY-----\n"));
        std::os::unix::fs::symlink(&key, home.path().join("linked.pem")).unwrap();
        assert!(read_place_key(&home.path().join("linked.pem")).is_none(), "a link at the key was followed");
        let fifo = home.path().join("fifo.pem");
        fifo_at(&fifo);
        let (tx, rx) = mpsc::channel();
        let at = fifo.clone();
        std::thread::spawn(move || drop(tx.send(read_place_key(&at))));
        let read = rx.recv_timeout(Duration::from_secs(1)).ok();
        let _freed = nix::fcntl::open(&fifo, nix::fcntl::OFlag::O_WRONLY | nix::fcntl::OFlag::O_NONBLOCK, nix::sys::stat::Mode::empty());
        assert_eq!(read, Some(None), "the read of a fifo at the key did not answer none inside 1s");
    }

    #[test]
    fn a_link_at_the_place_file_reads_as_none_even_to_a_whole_place_file() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let whole = elsewhere.path().join("place.json");
        std::fs::write(&whole, PLACE_TEXT).unwrap();
        assert!(read_place_file(&whole).is_some(), "the file the link points at is a whole place file");
        std::os::unix::fs::symlink(&whole, home.path().join("place.json")).unwrap();
        assert!(read_place_file(&home.path().join("place.json")).is_none(), "a link at the place file was followed");
    }

    #[test]
    fn a_link_to_dev_zero_at_the_place_file_reads_as_none_at_once() {
        let home = tempfile::tempdir().unwrap();
        let link = home.path().join("place.json");
        std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
        let read = read_within_a_second(&link);
        assert!(read.is_some(), "the read of a link to /dev/zero at the place file is still reading after 1s");
        assert!(read.unwrap().is_none());
    }

    #[test]
    fn a_place_file_past_the_cap_reads_as_none_and_one_at_it_reads() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("place.json");
        let cap = usize::try_from(numbers::PLACE_READ_MAX_BYTES).unwrap();
        let padded = |len: usize| format!("{PLACE_TEXT}{}", " ".repeat(len - PLACE_TEXT.len()));
        std::fs::write(&path, padded(cap)).unwrap();
        assert!(read_place_file(&path).is_some(), "a place file padded to the cap reads");
        std::fs::write(&path, padded(cap + 1)).unwrap();
        assert!(read_place_file(&path).is_none(), "a place file past the cap was read");
    }
}
