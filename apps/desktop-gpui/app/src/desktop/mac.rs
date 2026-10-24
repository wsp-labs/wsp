// SPDX-License-Identifier: AGPL-3.0-only
//! The Mac's own pieces GPUI lacks, through `objc2-app-kit`: the Dock's badge and bounce, the alert sound, an
//! app-modal alert with no parent window, and the move to Applications (`desktop:move.ts`). The move's gate and words
//! are plain data, read on every platform.

use std::path::{Path, PathBuf};
use std::process::Command;

use gpui_kit::{App, AppContext as _, AsyncApp};

use crate::screens::dialogs::startup;

/// `MOVE_PROMPT`.
pub const MOVE_MESSAGE: &str = "Move wsp to your Applications folder?";
pub const MOVE_DETAIL: &str = "Opened from anywhere else, macOS runs wsp from a copy whose path changes every launch, and the wsp command it writes for your agents stops working. wsp restarts itself once it has moved.";
pub const MOVE_BUTTONS: [&str; 2] = ["Move to Applications", "Not now"];
pub const APPLICATIONS: &str = "/Applications";

/// Whether a bundle sits in an Applications folder: the computer's, or the person's own under their home.
pub fn in_applications(bundle: &Path, home: Option<&Path>) -> bool {
    bundle.starts_with(APPLICATIONS) || home.is_some_and(|home| bundle.starts_with(home.join("Applications")))
}

/// `offersMove`: only a packaged Mac bundle outside Applications is asked, and never a launch driven by a test.
pub fn offers_move(mac: bool, bundle: Option<&Path>, home: Option<&Path>, driven: bool) -> bool {
    let packaged = bundle.filter(|b| b.extension().is_some_and(|e| e == "app"));
    mac && !driven && packaged.is_some_and(|b| !in_applications(b, home))
}

/// Where the moved bundle lands: Applications, under the bundle's own name.
pub fn moved_to(bundle: &Path) -> PathBuf {
    Path::new(APPLICATIONS).join(bundle.file_name().unwrap_or_default())
}

/// The copy that moves the bundle, as Finder copies one: `ditto` keeps its signature, links and attributes.
pub fn move_command(bundle: &Path, to: &Path) -> Command {
    let mut ditto = Command::new("/usr/bin/ditto");
    ditto.env_clear().arg(bundle).arg(to);
    ditto
}

/// The move's answer acted on (`offerMove`): Not now opens this copy (none); Move copies the bundle into Applications
/// and names the copy to restart as, which writes the wsp command from its settled path; a copy that failed is the
/// line to log before this copy opens. The launch writes nothing before this answers.
pub fn answered(picked: usize, bundle: &Path, copy: impl FnOnce(&Path, &Path) -> Result<(), String>) -> Result<Option<PathBuf>, String> {
    if picked != 0 {
        return Ok(None);
    }
    let to = moved_to(bundle);
    copy(bundle, &to).map_err(|why| format!("wsp could not move itself to Applications: {why}"))?;
    Ok(Some(to))
}

/// What starts the moved copy once this process has gone, as GPUI's own restart does, but whose spawn this app sees:
/// bash waits on this pid, then `open` starts the bundle at `to`.
pub fn relaunch_command(pid: u32, to: &Path) -> Command {
    let mut bash = Command::new("/bin/bash");
    bash.env_clear().arg("-c").arg(RELAUNCH).arg(pid.to_string()).arg(to);
    bash
}

const RELAUNCH: &str = "while kill -0 \"$0\" 2>/dev/null; do /bin/sleep 0.1; done; exec /usr/bin/open \"$1\"";

/// Starts the moved copy after this process, in a process group of its own so this one's exit does not take it.
pub fn relaunch(to: &Path) -> Result<(), String> {
    use std::os::unix::process::CommandExt as _;
    relaunch_command(std::process::id(), to).process_group(0).spawn().map(drop).map_err(|e| e.to_string())
}

/// `offerMove`, with every other open held back while it stands: the Dock's reopen and a second launch see the launch
/// still opening, so neither writes the wsp command or starts the service off this copy while it is asked about or
/// copied. `ask` puts the question, `answer` acts on the button off the main thread, and the moved copy is started by
/// `relaunch` before this one quits. Not now, a copy that failed and a relaunch that never started release the hold
/// and run `then`, this copy's own open; a failure is logged first.
pub fn held_move(
    cx: &mut App,
    ask: impl FnOnce() -> usize + 'static,
    answer: impl FnOnce(usize) -> Result<Option<PathBuf>, String> + Send + 'static,
    relaunch: impl FnOnce(&Path) -> Result<(), String> + 'static,
    then: impl FnOnce(&mut App) + 'static,
) {
    startup::hold_opening(true, cx);
    cx.spawn(async move |cx: &mut AsyncApp| {
        let picked = ask();
        let answer = cx.background_spawn(async move { answer(picked) }).await;
        cx.update(|cx| {
            let failed = match answer {
                Ok(Some(to)) => match relaunch(&to) {
                    Ok(()) => return cx.quit(),
                    Err(why) => Some(format!("wsp moved itself to {} and could not restart as that copy: {why}", to.display())),
                },
                Ok(None) => None,
                Err(why) => Some(why),
            };
            if let Some(why) = failed {
                crate::desktop::log::error(&why);
            }
            startup::hold_opening(false, cx);
            then(cx);
        });
    })
    .detach();
}

#[cfg(target_os = "macos")]
pub use native::*;

#[cfg(target_os = "macos")]
mod native {
    use std::path::Path;

    use gpui_kit::App;
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSAlert, NSAlertFirstButtonReturn, NSAlertStyle, NSApp, NSBeep, NSRequestUserAttentionType};
    use objc2_foundation::{NSBundle, NSFileManager, NSString, NSURL};

    /// How an alert reads: a question, or a failure.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum Style {
        Question,
        Failure,
    }

    /// The Dock's badge: the count, and none at zero (`app.setBadgeCount`).
    pub fn set_badge(count: usize) {
        let Some(mtm) = MainThreadMarker::new() else { return };
        let label = (count > 0).then(|| NSString::from_str(&count.to_string()));
        NSApp(mtm).dockTile().setBadgeLabel(label.as_deref());
    }

    /// Window's Bring All to Front (`role: "front"`): every window of the app in front, in the order they stood.
    pub fn bring_all_to_front() {
        let Some(mtm) = MainThreadMarker::new() else { return };
        NSApp(mtm).arrangeInFront(None);
    }

    /// The system's alert sound (`shell.beep`).
    pub fn beep() {
        NSBeep();
    }

    /// One bounce of the Dock's icon (`app.dock.bounce("informational")`).
    pub fn bounce() {
        let Some(mtm) = MainThreadMarker::new() else { return };
        NSApp(mtm).requestUserAttention(NSRequestUserAttentionType::InformationalRequest);
    }

    /// Whether this process runs from an app bundle, without which macOS shows none of its notifications.
    pub fn bundled() -> bool {
        NSBundle::mainBundle().bundleIdentifier().is_some()
    }

    /// An app-modal alert with no parent window, the app raised first so it does not open behind another app's
    /// window; the first button is the default, and `cancel` takes Escape unless it is the only button, whose Return
    /// Escape would replace. The index of the button pressed. Call it from a spawned task and never inside a GPUI
    /// update: its modal loop runs every event while it stands.
    pub fn alert(style: Style, message: &str, detail: &str, buttons: &[&str], cancel: Option<usize>) -> usize {
        let Some(mtm) = MainThreadMarker::new() else { return cancel.unwrap_or(0) };
        #[allow(deprecated)]
        NSApp(mtm).activateIgnoringOtherApps(true);
        let alert = NSAlert::new(mtm);
        alert.setAlertStyle(if style == Style::Failure { NSAlertStyle::Critical } else { NSAlertStyle::Informational });
        alert.setMessageText(&NSString::from_str(message));
        alert.setInformativeText(&NSString::from_str(detail));
        for (at, label) in buttons.iter().enumerate() {
            let button = alert.addButtonWithTitle(&NSString::from_str(label));
            if Some(at) == cancel && buttons.len() > 1 {
                button.setKeyEquivalent(&NSString::from_str("\u{1b}"));
            }
        }
        let picked = alert.runModal() - NSAlertFirstButtonReturn;
        usize::try_from(picked).ok().filter(|&at| at < buttons.len()).or(cancel).unwrap_or(0)
    }

    /// `offerMove` before `installCommand`: a bundle outside Applications asks once at launch, and `then`, the launch's
    /// open that writes the wsp command, runs only once it is answered (`held_move`). Move puts a bundle already there
    /// in the Trash, copies this one there by `ditto` off the main thread and restarts as the moved one, never opening
    /// this copy; Not now opens it, and a move that failed is logged and opens it.
    pub fn offer_move(cx: &mut App, then: impl FnOnce(&mut App) + 'static) {
        let bundle = cx.app_path().ok();
        let home = std::env::var_os("HOME").map(std::path::PathBuf::from);
        let driven = std::env::var_os("WSP_DESKTOP_SMOKE").is_some_and(|v| v == "1");
        let Some(bundle) = bundle.filter(|bundle| super::offers_move(true, Some(bundle), home.as_deref(), driven)) else {
            return then(cx);
        };
        let ask = || alert(Style::Question, super::MOVE_MESSAGE, super::MOVE_DETAIL, &super::MOVE_BUTTONS, Some(1));
        super::held_move(cx, ask, move |picked| super::answered(picked, &bundle, moved), super::relaunch, then);
    }

    fn moved(bundle: &Path, to: &Path) -> Result<(), String> {
        if to.exists() {
            let url = NSURL::fileURLWithPath(&NSString::from_str(&to.to_string_lossy()));
            NSFileManager::defaultManager()
                .trashItemAtURL_resultingItemURL_error(&url, None)
                .map_err(|e| format!("{} could not be moved to the Trash: {}", to.display(), e.localizedDescription()))?;
        }
        let out = super::move_command(bundle, to).output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_owned())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `move.test.ts`: only a packaged Mac bundle outside Applications is asked, never a driven launch.
    #[test]
    fn the_move_is_offered_only_to_a_packaged_mac_bundle_outside_applications() {
        let downloads = Path::new("/Users/z/Downloads/wsp.app");
        let home = Some(Path::new("/Users/z"));
        assert!(offers_move(true, Some(downloads), home, false));
        assert!(!offers_move(false, Some(downloads), home, false), "Linux has nothing to move");
        assert!(!offers_move(true, Some(downloads), home, true), "a driven launch answers no box");
        assert!(!offers_move(true, None, home, false), "a build run out of a checkout is no bundle");
        assert!(!offers_move(true, Some(Path::new("/Users/z/src/target/debug")), home, false));
        assert!(!offers_move(true, Some(Path::new("/Applications/wsp.app")), home, false));
        assert!(!offers_move(true, Some(Path::new("/Users/z/Applications/wsp.app")), home, false));
    }

    /// Catches: the launch writing the wsp command and starting the service off the Downloads copy while the move's
    /// question stands, so Move's copy (which keeps the original) leaves the service on a copy the person trashes next.
    #[test]
    fn only_the_moves_answer_lets_this_copy_open_and_move_restarts_as_the_copy() {
        let bundle = Path::new("/Users/z/Downloads/wsp.app");
        let copied = std::cell::Cell::new(false);
        assert_eq!(answered(1, bundle, |_, _| panic!("Not now copies nothing")), Ok(None), "Not now opens this copy");
        let restart = answered(0, bundle, |from, to| {
            copied.set(from == bundle && to == Path::new("/Applications/wsp.app"));
            Ok(())
        });
        assert_eq!(restart, Ok(Some(PathBuf::from("/Applications/wsp.app"))), "Move restarts as the copy and opens nothing here");
        assert!(copied.get());
        let failed = answered(0, bundle, |_, _| Err("ditto: Permission denied".to_owned()));
        assert_eq!(failed, Err("wsp could not move itself to Applications: ditto: Permission denied".to_owned()));
    }

    /// Catches: the Dock's reopen or a second launch opening this copy while the move's question or its copy stands,
    /// which writes the wsp command and starts the service off the copy being moved, and two opens racing after Not
    /// now; and a relaunch that never started leaving the app with no window.
    #[gpui_kit::test]
    fn other_opens_wait_on_the_move_and_a_relaunch_that_never_started_opens_this_copy(cx: &mut gpui_kit::TestAppContext) {
        let opened = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let bundle = PathBuf::from("/Users/z/Downloads/wsp.app");
        let cases: [(usize, Result<(), String>); 3] = [(1, Ok(())), (0, Err("bash: No such file".to_owned())), (0, Ok(()))];
        for (at, (picked, relaunched)) in cases.into_iter().enumerate() {
            let (kept, from) = (opened.clone(), bundle.clone());
            cx.update(|cx| {
                held_move(
                    cx,
                    move || picked,
                    move |picked| answered(picked, &from, |_, _| Ok(())),
                    move |to| {
                        assert_eq!(to, Path::new("/Applications/wsp.app"));
                        relaunched
                    },
                    move |cx| kept.borrow_mut().push((at, startup::opening(cx))),
                );
                assert!(startup::opening(cx), "case {at}: the Dock's reopen and a second launch wait while the move is asked");
            });
            cx.run_until_parked();
        }
        assert_eq!(*opened.borrow(), [(0, false), (1, false)], "Not now and a failed relaunch open this copy, the hold released");
        let bash = relaunch_command(42, Path::new("/Applications/wsp.app"));
        assert_eq!(bash.get_program(), "/bin/bash");
        let args: Vec<_> = bash.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect();
        assert_eq!(args, ["-c", RELAUNCH, "42", "/Applications/wsp.app"]);
        assert_eq!(bash.get_envs().count(), 0, "the relaunch carries none of the app's environment");
    }

    #[test]
    fn the_move_copies_the_bundle_into_applications_under_its_own_name() {
        let bundle = Path::new("/private/var/folders/x/AppTranslocation/y/d/wsp.app");
        assert_eq!(moved_to(bundle), Path::new("/Applications/wsp.app"));
        let ditto = move_command(bundle, &moved_to(bundle));
        assert_eq!(ditto.get_program(), "/usr/bin/ditto");
        assert_eq!(ditto.get_args().collect::<Vec<_>>(), [bundle.as_os_str(), Path::new("/Applications/wsp.app").as_os_str()]);
        assert_eq!(ditto.get_envs().count(), 0, "the copy carries none of the app's environment");
    }
}
