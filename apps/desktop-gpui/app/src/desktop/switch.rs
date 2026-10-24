// SPDX-License-Identifier: AGPL-3.0-only
//! Today's app's Experimental switch (`desktop:experimental.ts`): a file under `<home>/desktop/` that hands today's app
//! over to this one on every launch, offered only while `<home>/desktop/desktop-gpui.path` names this app. This app is
//! the one wsp ships, and today's app is a separate download, so a launch takes both files away: today's app, opened
//! later, opens as itself and offers no switch, and nothing hands one app to the other.

use std::io;
use std::path::{Path, PathBuf};

/// Present while today's app hands itself over to this one.
pub const SWITCH_FILE: &str = "experimental";
/// This app's executable, which today's app opened on its hand-over.
pub const PATH_FILE: &str = "desktop-gpui.path";

/// The folder today's app keeps its own lock and its switch in.
pub fn desktop_dir(home: &Path) -> PathBuf {
    home.join("desktop")
}

/// Takes the switch and the path today's app opens this one by out of `home`, and says which it took; the rest of
/// today's folder stays.
pub fn retire(home: &Path) -> io::Result<Vec<PathBuf>> {
    let mut taken = Vec::new();
    for name in [SWITCH_FILE, PATH_FILE] {
        let file = desktop_dir(home).join(name);
        match std::fs::remove_file(&file) {
            Ok(()) => taken.push(file),
            Err(e) if e.kind() == io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    Ok(taken)
}
