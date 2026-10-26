// SPDX-License-Identifier: AGPL-3.0-only
//! The swap (`SWAP_SCRIPT` in `self-update.ts`), as a script the app hands its own pid and paths and then quits under.
//! Every tool is named by its path: a Mac's fixed ones, and on Linux each found once on the PATH it is handed, the
//! swap refused before anything moves where one is missing. One swap runs at a time, holding a lock in the stage. It
//! ignores the TERM, HUP and INT a logout sends, so nothing stops it between the
//! two renames, and hands the new app those signals as the system gives them. It checks the staged copy again right
//! before the rename, so what was checked is what moves, and on Linux that the installed app is still an AppImage file
//! of its own, never a link or a folder it would half replace, then launches the new app itself and waits for it to write
//! the version it runs into the stage, which it does once its host accepted it and its first window drew. On any
//! failed step
//! it stops that app, TERM then KILL, puts the old app back and opens it, leaving a note that says whether the host
//! was moved onto the new files, so the old app moves it back. The old app is deleted only once the new one is up.
//! Today's app's own swap (`SWAP_SCRIPT` in `self-update.ts`), which replaces today's app with this one at the same
//! path, waits on the same `up` in its own stage, and this app answers it the same way.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::stage::{stage_of, Kind, Staged, PREVIOUS};

pub const SCRIPT: &str = r#"
pid=$1 app=$2 root=$3 stage=$4 staged=$5 exe=$6 version=$7 kind=$8 sum=$9 patience=${10} grace=${11} log=${12} requirement=${13}
exec >>"$log" 2>&1
trap '' TERM HUP INT
aside="$stage/old.app"
failed="$stage/failed.app"
lock="$root/swapping"
newpid=
alive() { kill -0 "$1" 2>/dev/null; }
if [ "$kind" = mac ]; then
  mv=/bin/mv rm=/bin/rm sleep=/bin/sleep date=/bin/date sha=
else
  missing=
  for tool in mv rm sleep date sha256sum; do
    at=$(command -v "$tool" 2>/dev/null) && [ -x "$at" ] || missing="$missing $tool"
  done
  if [ -n "$missing" ]; then echo "no$missing on $PATH; nothing was changed"; exit 1; fi
  mv=$(command -v mv) rm=$(command -v rm) sleep=$(command -v sleep) date=$(command -v date) sha=$(command -v sha256sum)
fi
say() { echo "$("$date" -u +%Y-%m-%dT%H:%M:%SZ) $*"; }
take() { ( set -C; echo "$$" > "$lock" ) 2>/dev/null; }
if ! take; then
  other=
  read -r other 2>/dev/null < "$lock"
  if [ -n "$other" ] && alive "$other"; then say "another update (pid $other) is under way; nothing was changed"; exit 1; fi
  "$rm" -f "$lock"
  take || { say "another update took the stage; nothing was changed"; exit 1; }
fi
trap '"$rm" -f "$lock"' EXIT
launch() {
  if [ "$kind" = mac ]; then ( trap - TERM HUP INT; exec "$1/Contents/MacOS/$2" ) </dev/null >/dev/null 2>&1 &
  else ( trap - TERM HUP INT; exec "$1" ) </dev/null >/dev/null 2>&1 &
  fi
}
reopen() {
  if [ "$kind" = mac ]; then /usr/bin/open "$app" || say "the old app did not open"
  else ( trap - TERM HUP INT; exec "$app" ) </dev/null >/dev/null 2>&1 &
  fi
}
stop_new() {
  { [ -n "$newpid" ] && alive "$newpid"; } || return 0
  kill -TERM "$newpid"
  n=0
  while alive "$newpid"; do
    n=$((n + 1))
    if [ "$n" -gt $((grace * 10)) ]; then say "the new app (pid $newpid) did not stop; killing it"; kill -KILL "$newpid"; break; fi
    "$sleep" 0.1
  done
  wait "$newpid" 2>/dev/null
}
back() {
  say "$1; putting the old app back"
  stop_new
  if [ -e "$app" ] && ! "$mv" "$app" "$failed"; then say "the new app could not be moved out of the way; the old app is at $aside"; exit 1; fi
  if ! "$mv" "$aside" "$app"; then say "the old app could not be put back; it is at $aside"; exit 1; fi
  if [ -e "$root/restarted" ]; then echo restart > "$root/back"; else echo keep > "$root/back"; fi
  "$rm" -f "$root/opening" "$root/up" "$root/restarted"
  reopen
  exit 1
}
recheck() {
  for folder in "$root" "$stage"; do
    if [ -L "$folder" ] || [ ! -d "$folder" ] || [ ! -O "$folder" ]; then echo "$folder is not this account's own folder"; return 1; fi
  done
  if [ "$kind" = mac ]; then
    if [ -L "$staged" ] || [ ! -d "$staged" ]; then echo "it is not a folder of its own"; return 1; fi
    if [ -n "$version" ] && [ "$(/usr/bin/plutil -extract CFBundleShortVersionString raw -o - "$staged/Contents/Info.plist" 2>/dev/null)" != "$version" ]; then echo "its version is not $version"; return 1; fi
    if [ "$(/usr/bin/plutil -extract CFBundleExecutable raw -o - "$staged/Contents/Info.plist" 2>/dev/null)" != "$exe" ] || [ -L "$staged/Contents/MacOS/$exe" ] || [ ! -f "$staged/Contents/MacOS/$exe" ]; then echo "its executable is not $exe"; return 1; fi
    if [ -n "$requirement" ]; then
      /usr/bin/codesign --verify --deep --strict "-R=$requirement" "$staged" 2>/dev/null || { echo "its signature is not this app's"; return 1; }
    elif ! /usr/bin/codesign --verify --deep --strict "$staged" 2>/dev/null; then echo "its signature does not verify"; return 1; fi
  else
    if [ -L "$staged" ] || [ ! -f "$staged" ]; then echo "it is not a file of its own"; return 1; fi
    got=$("$sha" "$staged" 2>/dev/null) || { echo "it could not be read"; return 1; }
    if [ "${got%% *}" != "$sum" ]; then echo "its sha256 is not the release's"; return 1; fi
  fi
}
up() {
  got=
  read -r got 2>/dev/null < "$root/up"
  [ -n "$got" ] && { [ -z "$version" ] || [ "$got" = "$version" ]; }
}
n=0
while alive "$pid"; do
  n=$((n + 1))
  if [ "$n" -gt $((patience * 10)) ]; then say "the app (pid $pid) did not quit; nothing was changed"; exit 1; fi
  "$sleep" 0.1
done
if [ "$kind" != mac ] && { [ -L "$app" ] || [ ! -f "$app" ]; }; then
  say "$app is not an AppImage file of its own, so the update cannot tell what it would replace; nothing was changed"
  "$rm" -rf "$stage"
  reopen
  exit 1
fi
if ! why=$(recheck); then
  say "the checked app changed after it was checked ($why); nothing was changed"
  "$rm" -rf "$stage"
  reopen
  exit 1
fi
say "updating $app to ${version:-the app it replaced}"
"$rm" -rf "$aside" "$failed" "$root/up" "$root/opening" "$root/restarted" "$root/back" "$root/left"
say "moving $app aside to $aside"
if ! "$mv" "$app" "$aside"; then say "the app could not be moved aside; nothing was changed"; reopen; exit 1; fi
"$mv" "$staged" "$app" || back "the new app could not be moved into place"
echo $$ > "$root/opening"
launch "$app" "$exe"
newpid=$!
say "opened wsp ${version:-the app it replaced} as pid $newpid"
n=0
until up; do
  alive "$newpid" || back "the new app exited before it was up"
  n=$((n + 1))
  if [ "$n" -gt $((patience * 10)) ]; then back "the new app did not start within $patience s"; fi
  "$sleep" 0.1
done
say "wsp $got is up"
if "$rm" -rf "$root"; then say "deleted the old app"; else say "the old app could not be deleted; the next launch sweeps it"; fi
"#;

/// How long the script waits for the app to quit, and then for the new app to say it is up: the host's restart is
/// inside that wait, and a service host takes seconds to come back.
pub const PATIENCE_S: u32 = 90;
/// How long a new app that failed is given to stop on TERM before it is killed.
pub const GRACE_S: u32 = 10;

pub struct Swap {
    pub pid: u32,
    pub installed: PathBuf,
    pub kind: Kind,
    pub staged: Staged,
    /// The version the new app must say it runs; empty for an app put back, which says whatever it is.
    pub version: String,
    /// The staged AppImage's sha256, which the script checks again before the rename; empty for a Mac's app, whose
    /// signature it checks instead.
    pub sum: String,
    pub log: PathBuf,
    pub patience: u32,
    pub grace: u32,
}

/// The script's arguments, in the order it reads them.
pub fn args(s: &Swap) -> Vec<String> {
    let path = |p: &Path| p.to_string_lossy().into_owned();
    vec![
        s.pid.to_string(),
        path(&s.installed),
        path(&stage_of(&s.installed)),
        path(&s.staged.stage),
        path(&s.staged.app),
        s.staged.exe.clone(),
        s.version.clone(),
        match s.kind {
            Kind::Mac => "mac".into(),
            Kind::AppImage => "appimage".into(),
        },
        s.sum.clone(),
        s.patience.to_string(),
        s.grace.to_string(),
        path(&s.log),
        s.staged.requirement.clone(),
    ]
}

/// The tools the Linux swap finds on its PATH; a Mac's are named by their paths in the script.
pub const LINUX_TOOLS: [&str; 5] = ["mv", "rm", "sleep", "date", "sha256sum"];

/// The swap's PATH: the system's own, and on Linux the folder a NixOS system keeps its tools in.
pub fn path() -> &'static str {
    if cfg!(target_os = "macos") {
        "/usr/bin:/bin:/usr/sbin:/sbin"
    } else {
        "/usr/bin:/bin:/usr/sbin:/sbin:/run/current-system/sw/bin"
    }
}

/// The Linux tools that no folder of `path` holds as a runnable file.
pub fn missing_tools(path: &str) -> Vec<&'static str> {
    use std::os::unix::fs::PermissionsExt as _;
    let runnable = |file: PathBuf| std::fs::metadata(file).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
    LINUX_TOOLS.into_iter().filter(|tool| !path.split(':').any(|dir| !dir.is_empty() && runnable(Path::new(dir).join(tool)))).collect()
}

/// The pid of the swap holding the stage's lock, where it still runs.
pub fn swapping(root: &Path) -> Option<i32> {
    let note = root.join(LOCK);
    std::fs::symlink_metadata(&note).ok().filter(|m| m.is_file())?;
    std::fs::read_to_string(note).ok()?.trim().parse::<i32>().ok().filter(|pid| running(*pid))
}

/// The lock the script holds in the stage while it runs.
const LOCK: &str = "swapping";

/// What the script and the app it launches run with (`swapEnv`): the system's own PATH and, of this launch's
/// environment, only what says whose login and which wsp home it is, the person's switch for update checks, which the
/// host the new app restarts keeps, and on Linux which display to draw on and whether the AppImage unpacks itself for
/// want of FUSE, without which the new app would not start where the old one did.
pub fn env(get: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut kept = vec![("PATH".to_owned(), path().to_owned())];
    let names: &[&str] = if cfg!(target_os = "macos") {
        &["HOME", "USER", "LOGNAME", "TMPDIR", "LANG", "WSP_HOME", "WSP_UPDATE_CHECK"]
    } else {
        &[
            "HOME",
            "USER",
            "LOGNAME",
            "TMPDIR",
            "LANG",
            "WSP_HOME",
            "WSP_UPDATE_CHECK",
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XAUTHORITY",
            "XDG_RUNTIME_DIR",
            "XDG_SESSION_TYPE",
            "XDG_CURRENT_DESKTOP",
            "DBUS_SESSION_BUS_ADDRESS",
            "APPIMAGE_EXTRACT_AND_RUN",
        ]
    };
    kept.extend(names.iter().filter_map(|name| get(name).map(|v| ((*name).to_owned(), v))));
    kept
}

/// Starts the swap in a session of its own, so the app quitting takes nothing of it.
pub fn start(s: &Swap, env: &[(String, String)]) -> std::io::Result<()> {
    let mut cmd = Command::new("/bin/sh");
    cmd.arg("-c").arg(SCRIPT).arg("wsp-update").args(args(s)).env_clear();
    cmd.envs(env.iter().map(|(k, v)| (k, v))).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null());
    // SAFETY: setsid is async-signal-safe and touches nothing of the parent.
    unsafe {
        cmd.pre_exec(|| if libc::setsid() < 0 { Err(std::io::Error::last_os_error()) } else { Ok(()) });
    }
    cmd.spawn().map(drop)
}

pub(crate) fn running(pid: i32) -> bool {
    // SAFETY: signal 0 checks the pid and sends nothing.
    pid > 0 && (unsafe { libc::kill(pid, 0) } == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
}

/// Today's app's stage beside the app it replaces (`STAGE_DIR` in `self-update.ts`), where its swap writes `opening` and
/// waits on `up` holding the version it asked for.
pub const TODAYS_STAGE: &str = ".wsp-update";

/// What a launch found beside its installed app (`settleStage`).
#[derive(Debug, PartialEq)]
pub enum Settled {
    Nothing,
    /// Whatever an earlier update left was deleted.
    Swept,
    /// A swap opened this app and waits on it: the host follows onto these files, then `up` answers the swap.
    Waiting(Waiting),
    /// A swap put this app back: `restart` where the host was moved onto the files that failed, so it follows back
    /// onto these, and what the new app said of why it did not stay.
    Back {
        restart: bool,
        said: Option<String>,
    },
    /// A swap that ended with no `up`: the app it replaced is kept at `previous` and offered back.
    Unfinished {
        previous: PathBuf,
    },
}

#[derive(Debug, PartialEq)]
pub struct Waiting {
    root: PathBuf,
}

impl Waiting {
    /// Said before the host is moved, so a swap that then fails knows to move it back.
    pub fn restarting(&self) -> std::io::Result<()> {
        write_own(&self.root.join("restarted"), "")
    }

    /// Why this app does not stay, for the app the swap puts back to say.
    pub fn left(&self, why: &str) -> std::io::Result<()> {
        write_own(&self.root.join("left"), why)
    }

    /// Said where the host was left as it was after all, so the app put back does not move it.
    pub fn not_restarted(&self) -> std::io::Result<()> {
        match std::fs::remove_file(self.root.join("restarted")) {
            Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    /// The version this app runs, which the swap waits on before it deletes the old app.
    pub fn up(&self, version: &str) -> std::io::Result<()> {
        write_own(&self.root.join("up"), version)
    }
}

/// A note in the stage, never written through a link planted at its name.
fn write_own(path: &Path, text: &str) -> std::io::Result<()> {
    use std::io::Write as _;
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut file =
        std::fs::OpenOptions::new().write(true).create(true).truncate(true).custom_flags(libc::O_NOFOLLOW).mode(0o600).open(path)?;
    file.write_all(text.as_bytes())
}

/// Whether the stage is this account's own folder, as the swap and the download hold it: anything else is left as it
/// stands and nothing in it is read.
fn own(root: &Path) -> bool {
    use std::os::unix::fs::MetadataExt as _;
    // SAFETY: getuid reads this process's user and cannot fail.
    let uid = unsafe { libc::getuid() };
    std::fs::symlink_metadata(root).is_ok_and(|m| m.is_dir() && m.uid() == uid)
}

/// The pid of a swap that opened this app within its patience, the only one still waiting on it; an older note's pid
/// may be anyone's by now.
fn waiting_pid(root: &Path) -> Option<i32> {
    let note = root.join("opening");
    let age = std::fs::symlink_metadata(&note).ok().filter(|m| m.is_file())?.modified().ok()?.elapsed().ok()?;
    if age > std::time::Duration::from_secs(u64::from(PATIENCE_S)) {
        return None;
    }
    std::fs::read_to_string(note).ok()?.trim().parse::<i32>().ok().filter(|pid| running(*pid))
}

/// The app an unfinished swap moved aside, where one is: already kept, or still in its stage.
fn replaced(root: &Path) -> Option<PathBuf> {
    let kept = root.join(PREVIOUS);
    if std::fs::symlink_metadata(&kept).is_ok() {
        return Some(kept);
    }
    let stages = std::fs::read_dir(root).ok()?.filter_map(|e| e.ok());
    stages
        .filter(|e| e.file_name().to_string_lossy().starts_with("stage-"))
        .map(|e| e.path().join("old.app"))
        .find(|old| std::fs::symlink_metadata(old).is_ok())
}

/// Settles the stage beside `installed` at launch: a swap still running is left alone, and answered later where it
/// waits on this app, today's app's swap among them; one that put this app back is followed and swept. The app a swap
/// replaced goes only once `up` was written: with no `up` it is kept and offered back. Anything else an earlier update
/// left is deleted. Today's stage is otherwise today's app's to sweep, since it may stand beside this one.
pub fn settle(installed: &Path) -> Settled {
    let today = installed.parent().unwrap_or(Path::new("/")).join(TODAYS_STAGE);
    if own(&today) && waiting_pid(&today).is_some() {
        return Settled::Waiting(Waiting { root: today });
    }
    let root = stage_of(installed);
    if !own(&root) {
        return Settled::Nothing;
    }
    if waiting_pid(&root).is_some() {
        return Settled::Waiting(Waiting { root });
    }
    if swapping(&root).is_some() {
        return Settled::Nothing;
    }
    let up = std::fs::symlink_metadata(root.join("up")).is_ok();
    if let Some(old) = replaced(&root).filter(|_| !up) {
        let kept = root.join(PREVIOUS);
        if old == kept || std::fs::rename(&old, &kept).is_ok() {
            super::stage::discard(installed);
            return Settled::Unfinished { previous: kept };
        }
        return Settled::Unfinished { previous: old };
    }
    let back = std::fs::read_to_string(root.join("back")).ok();
    let left = std::fs::File::open(root.join("left")).ok().and_then(|f| {
        let mut why = String::new();
        std::io::Read::read_to_string(&mut std::io::Read::take(f, 1024), &mut why).ok().map(|_| why.trim().to_owned())
    });
    _ = std::fs::remove_dir_all(&root);
    match back {
        Some(note) => Settled::Back { restart: note.trim() == "restart", said: left.filter(|why| !why.is_empty()) },
        None => Settled::Swept,
    }
}
