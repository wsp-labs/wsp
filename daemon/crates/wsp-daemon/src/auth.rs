// SPDX-License-Identifier: AGPL-3.0-only
//! The token file is the only source, read at every auth frame so the host can rotate it while the daemon runs;
//! an exported token or a query string never counts.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use subtle::ConstantTimeEq;

use crate::Ctx;

/// How often the watcher reads the token file. A rotation is answered within this, which is the gap between the
/// host writing the file and the sockets the old token opened going.
pub(crate) const WATCH_EVERY: Duration = Duration::from_secs(1);

/// The most of the token file read. The host writes a token of a few dozen bytes, so a file past this is not one.
pub(crate) const TOKEN_READ_MAX_BYTES: u64 = 4096;

/// The token as the file holds it now, trimmed; nothing when the file is missing, unreadable, blank or no plain file
/// under the cap. On a place it sits in a home the place's own threads write, and the watcher reads it on the
/// daemon's one thread, so a fifo there held every socket and a link to /dev/zero read until memory ran out.
pub(crate) fn current_token(path: &Path) -> Option<String> {
    let text = String::from_utf8(crate::beneath::read_capped(path, TOKEN_READ_MAX_BYTES).ok()??).ok()?;
    let token = text.trim();
    (!token.is_empty()).then(|| token.to_owned())
}

/// Whether the token a peer sent is the one the file holds now, compared in constant time.
pub(crate) fn token_matches(given: &str, path: &Path) -> bool {
    match current_token(path) {
        Some(expected) => same(given, &expected),
        None => false,
    }
}

/// Two tokens compared in constant time, the one rule both the door and the watcher read.
pub(crate) fn same(given: &str, expected: &str) -> bool {
    expected.as_bytes().ct_eq(given.as_bytes()).into()
}

/// The token file, read once a second: every socket that came through the door on a token the file no longer
/// holds is cut. Reading the file at the auth frame alone let a rotation revoke nothing, so a box that held the
/// old token kept every socket it had already opened for as long as it cared to. A read that answers nothing cuts
/// nothing: a file being renamed into place is a moment, not a rotation.
pub(crate) async fn watch(ctx: Arc<Ctx>) {
    loop {
        tokio::time::sleep(ctx.options.token_watch_ms.map_or(WATCH_EVERY, Duration::from_millis)).await;
        let Some(held) = current_token(&ctx.options.token_path) else { continue };
        ctx.cut_stale_tokens(&held);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn token_file(text: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(text.as_bytes()).unwrap();
        f
    }

    #[test]
    fn the_file_is_read_trimmed_and_blank_counts_as_none() {
        assert_eq!(current_token(token_file("abc\n").path()).as_deref(), Some("abc"));
        assert_eq!(current_token(token_file("  \n").path()), None);
        assert_eq!(current_token(Path::new("/nonexistent/token")), None);
    }

    #[test]
    fn a_rotated_file_refuses_the_old_token_on_the_next_read() {
        let f = token_file("first\n");
        assert!(token_matches("first", f.path()));
        assert!(!token_matches("firs", f.path()));
        assert!(!token_matches("first2", f.path()));
        std::fs::write(f.path(), "second\n").unwrap();
        assert!(!token_matches("first", f.path()));
        assert!(token_matches("second", f.path()));
    }

    #[test]
    fn a_missing_or_blank_file_matches_nothing() {
        assert!(!token_matches("", token_file("").path()));
        assert!(!token_matches("x", Path::new("/nonexistent/token")));
    }

    /// The read on its own thread, and what it answered inside a second; None where it was still reading then.
    fn read_within_a_second(path: &Path) -> Option<Option<String>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let at = path.to_owned();
        std::thread::spawn(move || drop(tx.send(current_token(&at))));
        rx.recv_timeout(Duration::from_secs(1)).ok()
    }

    #[test]
    fn a_fifo_at_the_token_file_reads_as_none_at_once() {
        let home = tempfile::tempdir().unwrap();
        let fifo = home.path().join("daemon-token");
        nix::unistd::mkfifo(&fifo, nix::sys::stat::Mode::from_bits_truncate(0o600)).unwrap();
        let read = read_within_a_second(&fifo);
        // A read still waiting on the fifo's open is let go by a writer that opens it and closes it at once.
        let _freed = nix::fcntl::open(&fifo, nix::fcntl::OFlag::O_WRONLY | nix::fcntl::OFlag::O_NONBLOCK, nix::sys::stat::Mode::empty());
        assert_eq!(read, Some(None), "the read of a fifo at the token file did not answer none inside 1s");
    }

    #[test]
    fn a_link_to_dev_zero_at_the_token_file_reads_as_none_at_once() {
        let home = tempfile::tempdir().unwrap();
        let link = home.path().join("daemon-token");
        std::os::unix::fs::symlink("/dev/zero", &link).unwrap();
        assert_eq!(
            read_within_a_second(&link),
            Some(None),
            "the read of a link to /dev/zero at the token file did not answer none inside 1s"
        );
    }

    #[test]
    fn a_link_out_at_the_token_file_matches_nothing_even_to_a_whole_token() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = token_file("abc\n");
        let link = home.path().join("daemon-token");
        std::os::unix::fs::symlink(elsewhere.path(), &link).unwrap();
        assert!(token_matches("abc", elsewhere.path()), "the file the link points at holds the token");
        assert!(!token_matches("abc", &link), "a link at the token file was followed");
    }

    #[test]
    fn a_token_file_past_the_cap_matches_nothing_and_one_at_it_matches() {
        let cap = usize::try_from(TOKEN_READ_MAX_BYTES).unwrap();
        let at_cap = format!("abc{}", " ".repeat(cap - 3));
        assert!(token_matches("abc", token_file(&at_cap).path()), "a token file padded to the cap reads");
        assert!(!token_matches("abc", token_file(&format!("{at_cap} ")).path()), "a token file past the cap was read");
    }
}
