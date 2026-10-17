// SPDX-License-Identifier: AGPL-3.0-only
//! The one line wsp puts in a person's own login files, taken back out by a leave over the link.

use std::os::fd::OwnedFd;
use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;

use nix::fcntl::OFlag;
use nix::sys::stat::Mode;
use wsp_frames::{numbers, place_daemon_paths, place_login_files, words};

use crate::beneath;

/// Takes wsp's line back out of each of the person's own login files, every line naming wsp's profile file whatever
/// spelling it was written in, and answers one line per file it changed, left with wsp's line in it, or did not read.
/// Their file, so it is written back in place and never made or removed. A workspace on a computer somebody owns
/// writes in this home: each name is opened through the home's descriptor with no link followed and without waiting,
/// so a fifo there cannot hold the leave and a link never has the leave open, as root, a fifo's other end or a device;
/// read only where it is a plain file, and no further than `LOGIN_READ_MAX_BYTES`; and written only where it is still
/// that file with one name, through the same descriptor, so nothing a workspace planted there has this write land in a
/// file of the computer's own.
pub(crate) fn sweep_login_lines(home: &Path) -> Vec<String> {
    use std::io::Read as _;
    use std::os::unix::fs::MetadataExt as _;
    let named = place_daemon_paths(home).profile_file;
    let named = named.as_os_str().as_bytes();
    let Ok(held) = nix::fcntl::open(home, beneath::dir_flags(), Mode::empty()) else { return Vec::new() };
    let mut said = Vec::new();
    for file in place_login_files(home) {
        let Some(name) = file.file_name().and_then(|name| name.to_str()) else { continue };
        let opened = match beneath::file(&held, name) {
            Ok(Some(opened)) => opened,
            Ok(None) => continue,
            Err(_) => {
                said.push(words::place_login_unread(file.display()));
                continue;
            }
        };
        let Ok(read) = opened.metadata() else { continue };
        let mut bytes = Vec::new();
        if read.len() > numbers::LOGIN_READ_MAX_BYTES
            || opened.take(numbers::LOGIN_READ_MAX_BYTES + 1).read_to_end(&mut bytes).is_err()
            || bytes.len() as u64 > numbers::LOGIN_READ_MAX_BYTES
        {
            said.push(words::place_line_stays("wsp's line", file.display(), words::PLACE_LOGIN_PAST_CAP));
            continue;
        }
        let names = |row: &[u8]| row.windows(named.len()).any(|at| at == named);
        let rows: Vec<&[u8]> = bytes.split(|b| *b == b'\n').collect();
        let gone: Vec<String> = rows.iter().filter(|row| names(row)).map(|row| String::from_utf8_lossy(row).trim().to_owned()).collect();
        if gone.is_empty() {
            continue;
        }
        let lines = gone.join("; ");
        if read.nlink() != 1 {
            said.push(words::place_line_stays(lines, file.display(), words::PLACE_LOGIN_LINKED));
            continue;
        }
        let kept = rows.iter().filter(|row| !names(row)).copied().collect::<Vec<_>>().join(&b'\n');
        said.push(match write_through(&held, name, &read, &kept) {
            Ok(()) => words::place_line_out(lines, file.display()),
            Err(why) => words::place_line_stays(lines, file.display(), why),
        });
    }
    said
}

/// The file written back with what it read less wsp's lines: opened again through the home's descriptor with no link
/// followed and without waiting, and written only where it is still the plain file that was read, with one name.
fn write_through(held: &OwnedFd, name: &str, read: &std::fs::Metadata, kept: &[u8]) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::MetadataExt as _;
    let flags = OFlag::O_WRONLY | OFlag::O_NOFOLLOW | OFlag::O_NONBLOCK | OFlag::O_CLOEXEC;
    let mut file = std::fs::File::from(nix::fcntl::openat(held, name, flags, Mode::empty()).map_err(std::io::Error::from)?);
    let now = file.metadata()?;
    if !now.is_file() || now.dev() != read.dev() || now.ino() != read.ino() || now.nlink() != 1 {
        return Err(std::io::Error::other("it changed while the leave read it"));
    }
    file.set_len(0)?;
    file.write_all(kept)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as _;

    #[test]
    fn the_leave_takes_its_line_out_of_each_login_file_and_writes_through_no_link_or_second_name() {
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let named = place_daemon_paths(home.path()).profile_file.display().to_string();
        let line = format!("[ -f {named} ] && . {named}");
        let [profile, bash_profile, bash_login] = [".profile", ".bash_profile", ".bash_login"].map(|name| home.path().join(name));
        // Both spellings in .profile, the one a computer joined before the line was guarded carries and today's.
        std::fs::write(&profile, format!("# theirs\n. {named}\nexport EDITOR=vi\n{line}\n")).unwrap();
        std::fs::write(&bash_profile, format!("export X=1\n{line}")).unwrap();
        // A .bash_login a workspace could have planted, pointing at a file of the computer's own that names the line: never
        // opened, so never read or written.
        let outside = elsewhere.path().join("theirs");
        std::fs::write(&outside, format!("{line}\n")).unwrap();
        std::os::unix::fs::symlink(&outside, &bash_login).unwrap();
        let swept = sweep_login_lines(home.path());
        assert_eq!(
            swept,
            [
                words::place_line_out(format!(". {named}; {line}"), profile.display()),
                words::place_line_out(&line, bash_profile.display()),
                words::place_login_unread(bash_login.display()),
            ]
        );
        assert_eq!(std::fs::read_to_string(&profile).unwrap(), "# theirs\nexport EDITOR=vi\n");
        assert_eq!(std::fs::read_to_string(&bash_profile).unwrap(), "export X=1");
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), format!("{line}\n"));
        // A file with a second name is the same file somewhere else: it is left whole.
        std::fs::remove_file(&bash_login).unwrap();
        std::fs::write(&bash_profile, format!("{line}\n")).unwrap();
        std::fs::hard_link(&bash_profile, elsewhere.path().join("twin")).unwrap();
        assert_eq!(sweep_login_lines(home.path()), [words::place_line_stays(&line, bash_profile.display(), words::PLACE_LOGIN_LINKED)]);
        assert_eq!(std::fs::read_to_string(&bash_profile).unwrap(), format!("{line}\n"));
    }

    #[test]
    fn a_login_file_past_the_cap_is_left_unread_and_said() {
        let home = tempfile::tempdir().unwrap();
        let named = place_daemon_paths(home.path()).profile_file.display().to_string();
        let file = home.path().join(".bash_login");
        let big = std::fs::File::create(&file).unwrap();
        big.set_len(numbers::LOGIN_READ_MAX_BYTES + 1).unwrap();
        drop(big);
        std::fs::OpenOptions::new().append(true).open(&file).unwrap().write_all(format!("\n. {named}\n").as_bytes()).unwrap();
        let before = std::fs::metadata(&file).unwrap().len();
        assert_eq!(sweep_login_lines(home.path()), [words::place_line_stays("wsp's line", file.display(), words::PLACE_LOGIN_PAST_CAP)]);
        assert_eq!(std::fs::metadata(&file).unwrap().len(), before);
    }

    #[test]
    fn a_login_file_it_cannot_write_back_is_said_with_why() {
        use std::os::unix::fs::PermissionsExt as _;
        if nix::unistd::geteuid().is_root() {
            return;
        }
        let home = tempfile::tempdir().unwrap();
        let named = place_daemon_paths(home.path()).profile_file.display().to_string();
        let line = format!(". {named}");
        let file = home.path().join(".profile");
        std::fs::write(&file, format!("{line}\n")).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o444)).unwrap();
        let swept = sweep_login_lines(home.path());
        assert_eq!(swept.len(), 1, "{swept:?}");
        assert!(swept[0].starts_with(&format!("{line} stays in {}: ", file.display())), "{swept:?}");
        assert_eq!(std::fs::read_to_string(&file).unwrap(), format!("{line}\n"));
    }

    #[test]
    fn a_fifo_at_a_login_file_is_passed_over_and_never_holds_the_leave() {
        let home = tempfile::tempdir().unwrap();
        let fifo = home.path().join(".bash_login");
        assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
        let (tx, rx) = std::sync::mpsc::channel();
        let at = home.path().to_owned();
        std::thread::spawn(move || tx.send(sweep_login_lines(&at)).unwrap());
        let said = rx.recv_timeout(std::time::Duration::from_secs(5));
        assert!(said.is_ok(), "the sweep is still waiting on the fifo at .bash_login after 5s");
        assert_eq!(said.unwrap(), [words::place_login_unread(fifo.display())]);
    }

    #[test]
    fn a_link_at_a_login_file_is_never_opened_whatever_it_points_at() {
        use std::os::unix::fs::OpenOptionsExt as _;
        let home = tempfile::tempdir().unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let fifo = elsewhere.path().join("fifo");
        assert!(std::process::Command::new("mkfifo").arg(&fifo).status().unwrap().success());
        std::os::unix::fs::symlink(&fifo, home.path().join(".bash_login")).unwrap();
        // A writer's open of a fifo waits until something opens its read end, so its return says the sweep opened it.
        let (tx, rx) = std::sync::mpsc::channel();
        let at = fifo.clone();
        std::thread::spawn(move || tx.send(std::fs::OpenOptions::new().write(true).open(&at).is_ok()).unwrap());
        std::thread::sleep(std::time::Duration::from_millis(100));
        assert_eq!(sweep_login_lines(home.path()), [words::place_login_unread(home.path().join(".bash_login").display())]);
        let opened = rx.recv_timeout(std::time::Duration::from_millis(500)).is_ok();
        let _freed = std::fs::OpenOptions::new().read(true).custom_flags(libc::O_NONBLOCK).open(&fifo);
        assert!(!opened, "the sweep opened the fifo a link at .bash_login points at");
    }
}
