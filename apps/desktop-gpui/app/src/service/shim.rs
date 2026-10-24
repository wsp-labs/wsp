// SPDX-License-Identifier: AGPL-3.0-only
//! The wsp command this app installs (`desktop:shim.ts`): a shell script that runs the bundled command on the bundled
//! Node, with the daemon's forwarder in front, so every agent's `wsp mcp` rides the host already running.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use super::Bundled;
use crate::desktop::switch;

/// Where the wsp command sits under a wsp home (`shimPath`), the path every agent's config and the unit run.
pub fn path(home: &Path) -> PathBuf {
    home.join("bin").join("wsp")
}

/// One word for sh (`shellQuote`): bare where nothing in it is special, else single quoted.
fn quote(word: &str) -> String {
    if !word.is_empty() && word.chars().all(|c| c.is_ascii_alphanumeric() || "_./:=@,+-".contains(c)) {
        return word.to_owned();
    }
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// The script's whole text (`shimText`, less Electron's node switch and its V8 flag, which Node refuses).
pub fn text(bundle: &Bundled) -> String {
    let argv = [&bundle.node, &bundle.command].map(|path| format!("--wsp-argv {}", quote(&path.to_string_lossy())));
    format!(
        "#!/bin/sh\n# The wsp app writes this on each launch; it runs the wsp the app bundles.\nexec {} forward {} -- \"$@\"\n",
        quote(&bundle.daemon.to_string_lossy()),
        argv.join(" ")
    )
}

/// The bundled command a wsp command's text runs, today's app's or this app's (`shimText`): the last `--wsp-argv` of
/// the forwarder's line, else the last word before `"$@"`; none where it is not an absolute path.
pub fn command_of(text: &str) -> Option<PathBuf> {
    let line =
        text.lines().map(str::trim).find_map(|line| line.strip_prefix("ELECTRON_RUN_AS_NODE=1 ").unwrap_or(line).strip_prefix("exec "))?;
    let words = switch::shell_words(line)?;
    let command = match words.iter().rposition(|word| word == "--wsp-argv") {
        Some(at) => words.get(at + 1)?,
        None => words.iter().rev().find(|word| *word != "\"$@\"")?,
    };
    Path::new(command).is_absolute().then(|| PathBuf::from(command))
}

/// The release a bundled command's files are, off the `package.json` two folders up from `app/main/cli.mjs`, which the
/// host reads its own release from (`host:version.ts`); none where the files are not there.
pub fn release_of(command: &Path) -> Option<String> {
    let text = std::fs::read_to_string(command.parent()?.parent()?.join("package.json")).ok()?;
    serde_json::from_str::<serde_json::Value>(&text).ok()?.get("version")?.as_str().map(str::to_owned)
}

/// What an install did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Done {
    Written,
    Kept,
}

impl Done {
    pub fn word(self) -> &'static str {
        match self {
            Done::Written => "written",
            Done::Kept => "kept",
        }
    }
}

/// Writes the script at `path`, executable, unless a file there already says exactly this (`installShim`). A fresh
/// file is renamed over whatever stands there, so a link left at the path is replaced and never written through, and
/// only while the path still holds `was`, what the launch read before it copied files: today's app writing its own
/// in that window keeps it.
pub fn install(path: &Path, text: &str, was: Option<&str>) -> io::Result<Done> {
    let link = std::fs::symlink_metadata(path).is_ok_and(|meta| meta.file_type().is_symlink());
    if !link && std::fs::read_to_string(path).is_ok_and(|was| was == text) {
        return Ok(Done::Kept);
    }
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = path.with_file_name(format!("wsp.{}.part", std::process::id()));
    let written = write_executable(&part, text).and_then(|()| {
        if std::fs::read_to_string(path).ok().as_deref() != was {
            return Err(io::Error::other("another app wrote it while this one made its own, so it stays as that app wrote it"));
        }
        std::fs::rename(&part, path)
    });
    if written.is_err() {
        let _ = std::fs::remove_file(&part);
    }
    written.map(|()| Done::Written)
}

#[cfg(unix)]
fn write_executable(part: &Path, text: &str) -> io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file = std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o755).open(part)?;
    file.write_all(text.as_bytes())?;
    use std::os::unix::fs::PermissionsExt as _;
    file.set_permissions(std::fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn write_executable(part: &Path, text: &str) -> io::Result<()> {
    std::fs::File::create(part)?.write_all(text.as_bytes())
}
