// SPDX-License-Identifier: AGPL-3.0-only
//! The one line wsp puts in a person's own login files, taken back out by a leave over the link.

use std::os::unix::ffi::OsStrExt as _;
use std::path::Path;

use wsp_frames::{place_daemon_paths, place_login_files, words};

/// Takes wsp's line back out of each of the person's own login files, every line naming wsp's profile file whatever
/// spelling it was written in, and answers one line per file it changed. Their file, so it is written back in place
/// and never made or removed. Only a plain file with one name is opened, and with no link followed: a workspace on a
/// computer somebody owns writes in this home, and a link or a second name planted there would have this write, as
/// root, into a file of the computer's own. The host's own leave writes a linked `.profile` through its config write.
pub(crate) fn sweep_login_lines(home: &Path) -> Vec<String> {
    use std::io::{Read as _, Seek as _, Write as _};
    use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _};
    let named = place_daemon_paths(home).profile_file;
    let named = named.as_os_str().as_bytes();
    let mut said = Vec::new();
    for file in place_login_files(home) {
        let Ok(mut opened) = std::fs::OpenOptions::new().read(true).write(true).custom_flags(libc::O_NOFOLLOW).open(&file) else {
            continue;
        };
        if !opened.metadata().is_ok_and(|meta| meta.is_file() && meta.nlink() == 1) {
            continue;
        }
        let mut bytes = Vec::new();
        if opened.read_to_end(&mut bytes).is_err() {
            continue;
        }
        let names = |row: &[u8]| row.windows(named.len()).any(|at| at == named);
        let rows: Vec<&[u8]> = bytes.split(|b| *b == b'\n').collect();
        let gone: Vec<String> = rows.iter().filter(|row| names(row)).map(|row| String::from_utf8_lossy(row).trim().to_owned()).collect();
        if gone.is_empty() {
            continue;
        }
        let kept = rows.iter().filter(|row| !names(row)).copied().collect::<Vec<_>>().join(&b'\n');
        if opened.rewind().and_then(|()| opened.set_len(0)).and_then(|()| opened.write_all(&kept)).is_ok() {
            said.push(words::place_line_out(gone.join("; "), file.display()));
        }
    }
    said
}

#[cfg(test)]
mod tests {
    use super::*;

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
        // A .bash_login a workspace could have planted, pointing at a file of the computer's own that names the line.
        let outside = elsewhere.path().join("theirs");
        std::fs::write(&outside, format!("{line}\n")).unwrap();
        std::os::unix::fs::symlink(&outside, &bash_login).unwrap();
        let swept = sweep_login_lines(home.path());
        assert_eq!(
            swept,
            [words::place_line_out(format!(". {named}; {line}"), profile.display()), words::place_line_out(&line, bash_profile.display()),]
        );
        assert_eq!(std::fs::read_to_string(&profile).unwrap(), "# theirs\nexport EDITOR=vi\n");
        assert_eq!(std::fs::read_to_string(&bash_profile).unwrap(), "export X=1");
        assert_eq!(std::fs::read_to_string(&outside).unwrap(), format!("{line}\n"));
        // A file with a second name is the same file somewhere else: it is left whole.
        std::fs::remove_file(&bash_login).unwrap();
        std::fs::write(&bash_profile, format!("{line}\n")).unwrap();
        std::fs::hard_link(&bash_profile, elsewhere.path().join("twin")).unwrap();
        assert!(sweep_login_lines(home.path()).is_empty());
        assert_eq!(std::fs::read_to_string(&bash_profile).unwrap(), format!("{line}\n"));
    }
}
