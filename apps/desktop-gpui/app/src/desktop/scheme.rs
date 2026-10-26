// SPDX-License-Identifier: AGPL-3.0-only
//! Who opens wsp:// links and what the app menu opens. A Mac reads the scheme off the bundle's Info.plist
//! (`CFBundleURLTypes`, scripts/package.sh), and LaunchServices' handler for the scheme is keyed by bundle id, which
//! today's app set to its own, `wsp.desktop`, on every launch (`setAsDefaultProtocolClient`), so a bundle built with
//! that id takes it over with nothing to tell. A Linux AppImage has no install step, so each launch names it the
//! scheme's handler in an entry of its own, never the app menu's `wsp.desktop`, which is the install line's. Where the
//! install line set the menu up, its launcher is brought to the install line's own text, which runs
//! `~/Applications/wsp.AppImage`, and this AppImage is put there where that file is not the GPUI app: an earlier install
//! line's launcher and AppImage open today's app.

use std::io::{self, Write as _};
use std::path::{Path, PathBuf};

use wsp_host_client::Env;

/// The entry that handles wsp:// links, apart from the app menu's.
pub const HANDLER: &str = "wsp-links.desktop";
pub const MIME: &str = "x-scheme-handler/wsp";
const DEFAULTS: &str = "[Default Applications]";
/// The install line, whose launcher this app writes as the install line writes it.
const INSTALL: &str = include_str!("../../../../www/public/install");

/// Where the login keeps its own desktop entries and its default handlers (the XDG base directories), and where the
/// install line keeps the app menu's launcher and the AppImage it runs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Places {
    pub applications: PathBuf,
    pub mimeapps: PathBuf,
    pub launcher: PathBuf,
    pub image: PathBuf,
}

impl Places {
    /// `$XDG_DATA_HOME/applications` and `$XDG_CONFIG_HOME/mimeapps.list`, each under HOME where its variable is unset
    /// or not absolute, as the spec says to read them; `~/.wsp/bin/wsp-desktop` and `~/Applications/wsp.AppImage`.
    pub fn of(env: &Env) -> Option<Places> {
        let home = env.get("HOME").filter(|home| Path::new(home).is_absolute()).map(PathBuf::from)?;
        let base = |name: &str, under: &str| {
            env.get(name).filter(|dir| Path::new(dir).is_absolute()).map(PathBuf::from).unwrap_or_else(|| home.join(under))
        };
        Some(Places {
            applications: base("XDG_DATA_HOME", ".local/share").join("applications"),
            mimeapps: base("XDG_CONFIG_HOME", ".config").join("mimeapps.list"),
            launcher: home.join(".wsp/bin/wsp-desktop"),
            image: home.join("Applications/wsp.AppImage"),
        })
    }
}

/// The handler's entry for the AppImage at `image`, out of the app menu. The path is quoted as the Desktop Entry spec
/// reads an Exec line only where it holds a character the spec reserves, since xdg-utils' own opener takes the line's
/// first word as it stands; one the spec would need escaped twice over is refused rather than guessed at.
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

/// The launcher the install line writes for the app menu (`write_launcher` in apps/www/public/install).
pub fn install_launcher() -> &'static str {
    let from =
        INSTALL.find("  cat <<'LAUNCHER'\n").map(|at| at + "  cat <<'LAUNCHER'\n".len()).expect("the install line writes a launcher");
    let to = from + INSTALL[from..].find("\nLAUNCHER\n").expect("its launcher ends") + 1;
    &INSTALL[from..to]
}

/// Whether the AppImage at `path` is the GPUI app, read off its squashfs.
fn is_this_app(path: &Path) -> bool {
    let exe = Path::new("usr/bin").join(crate::update::stage::EXE);
    crate::update::image::Image::open(path).and_then(|mut image| image.entry(&exe)).is_ok_and(|entry| entry.is_some())
}

/// `mimeapps.list` with the handler the scheme's default, every other line kept as it stood.
pub fn with_default(list: &str) -> String {
    let line = format!("{MIME}={HANDLER}");
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

/// Names the AppImage at `image` the handler of wsp:// links in `places`, and where the install line set the app menu
/// up, makes its launcher run `~/Applications/wsp.AppImage` and puts `image` there unless the GPUI app already stands
/// there; says which files it wrote. A file that already says exactly that is left.
pub fn register(image: &Path, places: &Places) -> io::Result<Vec<PathBuf>> {
    // A list kept as a link into the person's dotfiles is written where it points, so the link stays theirs.
    let mimeapps = std::fs::canonicalize(&places.mimeapps).unwrap_or_else(|_| places.mimeapps.clone());
    let was = std::fs::read_to_string(&mimeapps).unwrap_or_default();
    let mut files =
        vec![(places.applications.join(HANDLER), entry(image)?.into_bytes(), 0o644), (mimeapps, with_default(&was).into_bytes(), 0o644)];
    if places.launcher.is_file() {
        files.push((places.launcher.clone(), install_launcher().as_bytes().to_vec(), 0o755));
        let same = std::fs::canonicalize(&places.image).ok() == std::fs::canonicalize(image).ok();
        if !same && !is_this_app(&places.image) {
            files.push((places.image.clone(), std::fs::read(image)?, 0o755));
        }
    }
    let mut wrote = Vec::new();
    for (file, bytes, mode) in files {
        if std::fs::read(&file).is_ok_and(|was| was == bytes) {
            continue;
        }
        write_over(&file, &bytes, mode)?;
        wrote.push(file);
    }
    Ok(wrote)
}

/// Writes `text` beside `file` and renames it over, so a link standing at the name is replaced, never written through.
fn write_over(file: &Path, bytes: &[u8], mode: u32) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = file.parent().ok_or_else(|| io::Error::other(format!("{} has no folder", file.display())))?;
    std::fs::create_dir_all(dir)?;
    let name = file.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let part = dir.join(format!(".{name}.{}.part", std::process::id()));
    let written = std::fs::File::create(&part)
        .and_then(|mut f| f.write_all(bytes))
        .and_then(|()| std::fs::set_permissions(&part, std::fs::Permissions::from_mode(mode)))
        .and_then(|()| std::fs::rename(&part, file));
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
            super::log::info(&format!("wsp:// links and the app menu open {}: wrote {}", image.file.display(), files.join(", ")));
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

    /// A home whose places sit under `root`.
    fn places(root: &Path) -> Places {
        Places {
            applications: root.join("share/applications"),
            mimeapps: root.join("config/mimeapps.list"),
            launcher: root.join(".wsp/bin/wsp-desktop"),
            image: root.join("Applications/wsp.AppImage"),
        }
    }

    fn write(path: &Path, text: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    #[test]
    fn the_places_follow_the_xdg_variables_and_fall_back_under_home() {
        let plain = Places::of(&env(&[("HOME", "/h")])).unwrap();
        assert_eq!(plain.applications, Path::new("/h/.local/share/applications"));
        assert_eq!(plain.mimeapps, Path::new("/h/.config/mimeapps.list"));
        assert_eq!(plain.launcher, Path::new("/h/.wsp/bin/wsp-desktop"));
        assert_eq!(plain.image, Path::new("/h/Applications/wsp.AppImage"));
        let named = Places::of(&env(&[("HOME", "/h"), ("XDG_DATA_HOME", "/d"), ("XDG_CONFIG_HOME", "rel")])).unwrap();
        assert_eq!(named.applications, Path::new("/d/applications"));
        assert_eq!(named.mimeapps, Path::new("/h/.config/mimeapps.list"), "a relative XDG_CONFIG_HOME is ignored, as the spec says");
        assert_eq!(Places::of(&env(&[])), None);
    }

    #[test]
    fn the_handler_runs_the_appimage_on_the_link_out_of_the_menu_and_refuses_what_it_cannot_quote() {
        let text = entry(Path::new("/home/a/Apps/wsp 0.3.9.AppImage")).unwrap();
        assert!(text.contains("\nExec=\"/home/a/Apps/wsp 0.3.9.AppImage\" %u\n") && text.contains("\nNoDisplay=true\n"), "{text}");
        let bare = entry(Path::new("/home/a/Downloads/wsp-0.3.9.AppImage")).unwrap();
        assert!(
            bare.contains("\nExec=/home/a/Downloads/wsp-0.3.9.AppImage %u\n"),
            "a plain path stands bare, as xdg-open reads it: {bare}"
        );
        for bad in ["/a/$HOME.AppImage", "/a/\"q\".AppImage", "/a/100%.AppImage", "rel/wsp.AppImage"] {
            assert!(entry(Path::new(bad)).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_launcher_is_the_install_lines_own() {
        let launcher = install_launcher();
        assert!(launcher.starts_with("#!/bin/sh\n") && launcher.ends_with("exec \"$image\" \"$@\"\n"), "{launcher}");
        assert!(launcher.contains("image=\"$HOME/Applications/wsp.AppImage\""), "{launcher}");
    }

    #[test]
    fn the_default_is_set_in_its_section_and_every_other_line_stays() {
        assert_eq!(with_default(""), "[Default Applications]\nx-scheme-handler/wsp=wsp-links.desktop\n");
        let theirs = "[Added Associations]\ntext/plain=gedit.desktop;\n\n[Default Applications]\nx-scheme-handler/wsp=wsp.desktop\ntext/html=firefox.desktop\n";
        assert_eq!(
            with_default(theirs),
            "[Added Associations]\ntext/plain=gedit.desktop;\n\n[Default Applications]\nx-scheme-handler/wsp=wsp-links.desktop\ntext/html=firefox.desktop\n"
        );
        let no_wsp = "[Default Applications]\ntext/html=firefox.desktop\n[Removed Associations]\nx-scheme-handler/wsp=old.desktop\n";
        assert_eq!(
            with_default(no_wsp),
            "[Default Applications]\ntext/html=firefox.desktop\nx-scheme-handler/wsp=wsp-links.desktop\n[Removed Associations]\nx-scheme-handler/wsp=old.desktop\n",
            "a removed association is the person's own"
        );
    }

    /// Catches: the app menu, as an earlier install line left it on a computer that ran today's app, opening today's
    /// app after the move (its launcher runs the newest folder under `~/.wsp/app`, its AppImage is today's); or this app
    /// writing over the menu's own `wsp.desktop`.
    #[test]
    fn an_earlier_install_lines_menu_opens_this_app_and_its_entry_is_never_written() {
        let root = tempfile::tempdir().unwrap();
        let places = places(root.path());
        let menu = places.applications.join("wsp.desktop");
        let installed = format!("[Desktop Entry]\nName=wsp\nExec=\"{}\" %U\nIcon=/h/wsp.png\n", places.launcher.display());
        write(&menu, &installed);
        write(&places.launcher, "#!/bin/sh\n# opens wsp from the newest folder under ~/.wsp/app\n");
        write(&places.image, "today's Electron AppImage");
        let image = root.path().join("Downloads/wsp-0.3.9.AppImage");
        write(&image, "this app's AppImage");
        let wrote = register(&image, &places).unwrap();
        let handler = places.applications.join(HANDLER);
        assert_eq!(wrote, [handler.clone(), places.mimeapps.clone(), places.launcher.clone(), places.image.clone()]);
        assert_eq!(std::fs::read_to_string(&menu).unwrap(), installed, "the menu's entry is the install line's");
        assert_eq!(std::fs::read_to_string(&places.launcher).unwrap(), install_launcher());
        assert_eq!(std::fs::read_to_string(&places.image).unwrap(), "this app's AppImage", "the launcher opens this app");
        use std::os::unix::fs::PermissionsExt as _;
        for runs in [&places.launcher, &places.image] {
            assert_eq!(std::fs::metadata(runs).unwrap().permissions().mode() & 0o777, 0o755, "{}", runs.display());
        }
        assert_eq!(register(&image, &places).unwrap(), Vec::<PathBuf>::new(), "a second launch writes nothing");
    }

    /// Catches: this app copying itself over `~/Applications/wsp.AppImage` when it runs from there, or touching a home
    /// whose menu no install line set up.
    #[test]
    fn the_install_lines_own_appimage_and_a_home_without_its_menu_are_left() {
        let root = tempfile::tempdir().unwrap();
        let places = places(root.path());
        write(&places.launcher, install_launcher());
        write(&places.image, "this app's AppImage");
        assert_eq!(register(&places.image, &places).unwrap(), [places.applications.join(HANDLER), places.mimeapps.clone()]);
        let bare = tempfile::tempdir().unwrap();
        let places = super::tests::places(bare.path());
        let image = bare.path().join("Downloads/wsp.AppImage");
        write(&image, "this app's AppImage");
        register(&image, &places).unwrap();
        assert!(!places.image.exists() && !places.launcher.exists(), "no install line's menu, nothing of it written");
    }

    #[test]
    fn a_link_at_a_name_is_replaced_and_a_linked_list_written_through() {
        let dir = tempfile::tempdir().unwrap();
        let places = places(dir.path());
        let image = Path::new("/home/a/wsp.AppImage");
        let elsewhere = dir.path().join("elsewhere");
        std::fs::write(&elsewhere, "theirs").unwrap();
        std::fs::create_dir_all(&places.applications).unwrap();
        std::os::unix::fs::symlink(&elsewhere, places.applications.join(HANDLER)).unwrap();
        let dotfiles = dir.path().join("dotfiles/mimeapps.list");
        write(&dotfiles, "[Default Applications]\ntext/html=firefox.desktop\n");
        std::fs::create_dir_all(places.mimeapps.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&dotfiles, &places.mimeapps).unwrap();
        register(image, &places).unwrap();
        assert_eq!(std::fs::read_to_string(&elsewhere).unwrap(), "theirs", "a link at the handler's name is replaced, not written through");
        assert!(std::fs::symlink_metadata(&places.mimeapps).unwrap().file_type().is_symlink(), "the person's list stays a link");
        assert!(std::fs::read_to_string(&dotfiles).unwrap().contains("x-scheme-handler/wsp=wsp-links.desktop"));
        let names: Vec<_> = std::fs::read_dir(&places.applications).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, [HANDLER], "no part file is left");
    }
}
