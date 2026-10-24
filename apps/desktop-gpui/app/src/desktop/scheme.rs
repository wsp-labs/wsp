// SPDX-License-Identifier: AGPL-3.0-only
//! Who opens wsp:// links. A Mac reads the scheme off the bundle's Info.plist (`CFBundleURLTypes`, scripts/package.sh),
//! and LaunchServices' handler for the scheme is keyed by bundle id, which today's app set to its own, `wsp.desktop`, on
//! every launch (`setAsDefaultProtocolClient`): a bundle built with that id takes the handler over with nothing to
//! tell, and one built with another id is never the handler. A Linux AppImage
//! has no install step, so each launch names it as the handler in the login's own desktop entries: `wsp.desktop`, the
//! entry today's app set the scheme's handler to (`desktopName`), running this AppImage on the link, and the scheme's
//! default in `mimeapps.list`.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use wsp_host_client::Env;

/// The desktop entry's file name, today's app's `desktopName`.
pub const ENTRY: &str = "wsp.desktop";
pub const MIME: &str = "x-scheme-handler/wsp";
const DEFAULTS: &str = "[Default Applications]";

/// Where the login keeps its own desktop entries and its default handlers (the XDG base directories).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Places {
    pub applications: PathBuf,
    pub mimeapps: PathBuf,
}

impl Places {
    /// `$XDG_DATA_HOME/applications` and `$XDG_CONFIG_HOME/mimeapps.list`, each under HOME where its variable is unset
    /// or not absolute, as the spec says to read them.
    pub fn of(env: &Env) -> Option<Places> {
        let home = env.get("HOME").filter(|home| Path::new(home).is_absolute()).map(PathBuf::from)?;
        let base = |name: &str, under: &str| {
            env.get(name).filter(|dir| Path::new(dir).is_absolute()).map(PathBuf::from).unwrap_or_else(|| home.join(under))
        };
        Some(Places {
            applications: base("XDG_DATA_HOME", ".local/share").join("applications"),
            mimeapps: base("XDG_CONFIG_HOME", ".config").join("mimeapps.list"),
        })
    }
}

/// The entry's text for the AppImage at `image`, quoted as the Desktop Entry spec reads an Exec line only where the
/// path holds a character the spec reserves, since xdg-utils' own opener takes the line's first word as it stands; a
/// path with a character the spec would need escaped twice over is refused rather than guessed at.
pub fn entry(image: &Path) -> io::Result<String> {
    let path = image.to_str().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "the AppImage's path is not UTF-8"))?;
    if !image.is_absolute() || path.chars().any(|c| matches!(c, '"' | '`' | '$' | '\\' | '%' | '\n' | '\r')) {
        return Err(io::Error::new(io::ErrorKind::InvalidData, format!("{path} cannot be named on a desktop entry's Exec line")));
    }
    let exec = if path.chars().any(|c| " \t'><~|&;*?#()".contains(c)) { format!("\"{path}\"") } else { path.to_owned() };
    Ok(format!(
        "[Desktop Entry]\nType=Application\nName=wsp\nComment=Opens wsp:// links in wsp\nExec={exec} %u\nTerminal=false\nNoDisplay=true\nMimeType={MIME};\n"
    ))
}

/// `mimeapps.list` with `wsp.desktop` the scheme's default, every other line kept as it stood.
pub fn with_default(list: &str) -> String {
    let line = format!("{MIME}={ENTRY}");
    let mut out: Vec<String> = Vec::new();
    let (mut section, mut placed) = (String::new(), false);
    for raw in list.lines() {
        let trimmed = raw.trim();
        if trimmed.starts_with('[') {
            if section == DEFAULTS && !placed {
                out.push(line.clone());
                placed = true;
            }
            section = trimmed.to_owned();
        } else if section == DEFAULTS && trimmed.split_once('=').is_some_and(|(key, _)| key.trim() == MIME) {
            if !placed {
                out.push(line.clone());
                placed = true;
            }
            continue;
        }
        out.push(raw.to_owned());
    }
    if section == DEFAULTS && !placed {
        out.push(line.clone());
        placed = true;
    }
    if !placed {
        if out.last().is_some_and(|last| !last.trim().is_empty()) {
            out.push(String::new());
        }
        out.push(DEFAULTS.to_owned());
        out.push(line);
    }
    out.join("\n") + "\n"
}

/// Names the AppImage at `image` the handler of wsp:// links in `places`, and says which files it wrote; a file that
/// already says exactly that is left.
pub fn register(image: &Path, places: &Places) -> io::Result<Vec<PathBuf>> {
    let entry_file = places.applications.join(ENTRY);
    // A list kept as a link into the person's dotfiles is written where it points, so the link stays theirs.
    let mimeapps = std::fs::canonicalize(&places.mimeapps).unwrap_or_else(|_| places.mimeapps.clone());
    let was = std::fs::read_to_string(&mimeapps).unwrap_or_default();
    let mut wrote = Vec::new();
    for (file, text) in [(entry_file, entry(image)?), (mimeapps, with_default(&was))] {
        if std::fs::read_to_string(&file).is_ok_and(|was| was == text) {
            continue;
        }
        write_over(&file, &text)?;
        wrote.push(file);
    }
    Ok(wrote)
}

/// Writes `text` beside `file` and renames it over, so a link standing at the name is replaced, never written through.
fn write_over(file: &Path, text: &str) -> io::Result<()> {
    let dir = file.parent().ok_or_else(|| io::Error::other(format!("{} has no folder", file.display())))?;
    std::fs::create_dir_all(dir)?;
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let part = dir.join(format!(".{name}.{}.part", std::process::id()));
    let written = std::fs::File::create(&part).and_then(|mut f| f.write_all(text.as_bytes())).and_then(|()| std::fs::rename(&part, file));
    if written.is_err() {
        _ = std::fs::remove_file(&part);
    }
    written
}

/// This launch's AppImage named the handler, where it runs from one, with what it wrote in the app's log.
pub fn on_launch(env: &Env) {
    let (Some(image), Some(places)) = (crate::service::Image::of(env), Places::of(env)) else { return };
    match register(&image.file, &places) {
        Ok(wrote) if wrote.is_empty() => {}
        Ok(wrote) => {
            let files: Vec<_> = wrote.iter().map(|file| file.display().to_string()).collect();
            super::log::info(&format!("wsp:// links open {}: wrote {}", image.file.display(), files.join(" and ")));
        }
        Err(e) => super::log::error(&format!("wsp:// links were not pointed at {}: {e}", image.file.display())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Env {
        pairs.iter().map(|(k, v)| ((*k).to_owned(), (*v).to_owned())).collect()
    }

    #[test]
    fn the_places_follow_the_xdg_variables_and_fall_back_under_home() {
        let plain = Places::of(&env(&[("HOME", "/h")])).unwrap();
        assert_eq!(plain.applications, Path::new("/h/.local/share/applications"));
        assert_eq!(plain.mimeapps, Path::new("/h/.config/mimeapps.list"));
        let named = Places::of(&env(&[("HOME", "/h"), ("XDG_DATA_HOME", "/d"), ("XDG_CONFIG_HOME", "rel")])).unwrap();
        assert_eq!(named.applications, Path::new("/d/applications"));
        assert_eq!(named.mimeapps, Path::new("/h/.config/mimeapps.list"), "a relative XDG_CONFIG_HOME is ignored, as the spec says");
        assert_eq!(Places::of(&env(&[])), None);
    }

    #[test]
    fn the_entry_runs_the_appimage_on_the_link_and_refuses_what_it_cannot_quote() {
        let text = entry(Path::new("/home/a/Apps/wsp 0.3.9.AppImage")).unwrap();
        assert!(text.contains("\nExec=\"/home/a/Apps/wsp 0.3.9.AppImage\" %u\n"), "{text}");
        let bare = entry(Path::new("/home/a/Downloads/wsp-0.3.9.AppImage")).unwrap();
        assert!(
            bare.contains("\nExec=/home/a/Downloads/wsp-0.3.9.AppImage %u\n"),
            "a plain path stands bare, as xdg-open reads it: {bare}"
        );
        assert!(text.contains("\nMimeType=x-scheme-handler/wsp;\n"), "{text}");
        for bad in ["/a/$HOME.AppImage", "/a/\"q\".AppImage", "/a/100%.AppImage", "rel/wsp.AppImage"] {
            assert!(entry(Path::new(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_default_is_set_in_its_section_and_every_other_line_stays() {
        assert_eq!(with_default(""), "[Default Applications]\nx-scheme-handler/wsp=wsp.desktop\n");
        let theirs = "[Added Associations]\ntext/plain=gedit.desktop;\n\n[Default Applications]\nx-scheme-handler/wsp=appimagekit-wsp.desktop\ntext/html=firefox.desktop\n";
        assert_eq!(
            with_default(theirs),
            "[Added Associations]\ntext/plain=gedit.desktop;\n\n[Default Applications]\nx-scheme-handler/wsp=wsp.desktop\ntext/html=firefox.desktop\n"
        );
        let no_wsp = "[Default Applications]\ntext/html=firefox.desktop\n[Removed Associations]\nx-scheme-handler/wsp=old.desktop\n";
        assert_eq!(
            with_default(no_wsp),
            "[Default Applications]\ntext/html=firefox.desktop\nx-scheme-handler/wsp=wsp.desktop\n[Removed Associations]\nx-scheme-handler/wsp=old.desktop\n",
            "a removed association is the person's own"
        );
        assert_eq!(
            with_default("[Added Associations]\nx=y.desktop"),
            "[Added Associations]\nx=y.desktop\n\n[Default Applications]\nx-scheme-handler/wsp=wsp.desktop\n"
        );
    }

    #[test]
    fn a_launch_writes_the_entry_and_the_default_once_and_replaces_a_link() {
        let dir = tempfile::tempdir().unwrap();
        let places = Places { applications: dir.path().join("share/applications"), mimeapps: dir.path().join("config/mimeapps.list") };
        let image = Path::new("/home/a/wsp.AppImage");
        assert_eq!(register(image, &places).unwrap(), [places.applications.join(ENTRY), places.mimeapps.clone()]);
        assert_eq!(register(image, &places).unwrap(), Vec::<PathBuf>::new(), "a second launch from the same file writes nothing");
        let moved = Path::new("/home/a/Downloads/wsp.AppImage");
        assert_eq!(register(moved, &places).unwrap(), [places.applications.join(ENTRY)], "an AppImage that moved is named again");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::write(&elsewhere, "theirs").unwrap();
        std::fs::remove_file(places.applications.join(ENTRY)).unwrap();
        std::os::unix::fs::symlink(&elsewhere, places.applications.join(ENTRY)).unwrap();
        register(image, &places).unwrap();
        assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "theirs", "a link at the name is replaced, not written through");
        let names: Vec<_> = std::fs::read_dir(&places.applications).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, [ENTRY], "no part file is left");
        let dotfiles = dir.path().join("dotfiles/mimeapps.list");
        std::fs::create_dir_all(dotfiles.parent().unwrap()).unwrap();
        std::fs::write(&dotfiles, "[Default Applications]\ntext/html=firefox.desktop\n").unwrap();
        std::fs::remove_file(&places.mimeapps).unwrap();
        std::os::unix::fs::symlink(&dotfiles, &places.mimeapps).unwrap();
        register(image, &places).unwrap();
        assert!(std::fs::symlink_metadata(&places.mimeapps).unwrap().file_type().is_symlink(), "the person's link stays a link");
        assert!(
            std::fs::read_to_string(&dotfiles).unwrap().contains("x-scheme-handler/wsp=wsp.desktop"),
            "and the list it names holds the default"
        );
    }
}
