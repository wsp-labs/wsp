// SPDX-License-Identifier: AGPL-3.0-only
//! The swap (`SWAP_SCRIPT` in `self-update.ts`), as a script the app hands its own pid and paths and then quits under.
//! Every tool is named by its path. It ignores the TERM, HUP and INT a logout sends, so nothing stops it between the
//! two renames, and hands the new app those signals as the system gives them. It checks the staged copy again right
//! before the rename, so what was checked is what moves, then launches the new app itself and waits for it to write
//! the version it runs into the stage, which it does once the service host runs on the new files. On any failed step
//! it stops that app, TERM then KILL, puts the old app back and opens it, leaving a note that says whether the host
//! was moved onto the new files, so the old app moves it back. The old app is deleted only once the new one is up.

use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use super::stage::{stage_of, Kind, Staged};

pub const SCRIPT: &str = r#"
pid=$1 app=$2 root=$3 stage=$4 staged=$5 exe=$6 version=$7 kind=$8 sum=$9 patience=${10} grace=${11} log=${12}
exec >>"$log" 2>&1
trap '' TERM HUP INT
aside="$stage/old.app"
failed="$stage/failed.app"
newpid=
say() { echo "$(/bin/date -u +%Y-%m-%dT%H:%M:%SZ) $*"; }
alive() { /bin/kill -0 "$1" 2>/dev/null; }
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
  /bin/kill -TERM "$newpid"
  n=0
  while alive "$newpid"; do
    n=$((n + 1))
    if [ "$n" -gt $((grace * 10)) ]; then say "the new app (pid $newpid) did not stop; killing it"; /bin/kill -KILL "$newpid"; break; fi
    /bin/sleep 0.1
  done
  wait "$newpid" 2>/dev/null
}
back() {
  say "$1; putting the old app back"
  stop_new
  if [ -e "$app" ] && ! /bin/mv "$app" "$failed"; then say "the new app could not be moved out of the way; the old app is at $aside"; exit 1; fi
  if ! /bin/mv "$aside" "$app"; then say "the old app could not be put back; it is at $aside"; exit 1; fi
  if [ -e "$root/restarted" ]; then echo restart > "$root/back"; else echo keep > "$root/back"; fi
  /bin/rm -f "$root/opening" "$root/up" "$root/restarted"
  reopen
  exit 1
}
recheck() {
  for folder in "$root" "$stage"; do
    if [ -L "$folder" ] || [ ! -d "$folder" ] || [ ! -O "$folder" ]; then echo "$folder is not this account's own folder"; return 1; fi
  done
  if [ "$kind" = mac ]; then
    if [ -L "$staged" ] || [ ! -d "$staged" ]; then echo "it is not a folder of its own"; return 1; fi
    if [ "$(/usr/bin/plutil -extract CFBundleShortVersionString raw -o - "$staged/Contents/Info.plist" 2>/dev/null)" != "$version" ]; then echo "its version is not $version"; return 1; fi
    if [ "$(/usr/bin/plutil -extract CFBundleExecutable raw -o - "$staged/Contents/Info.plist" 2>/dev/null)" != "$exe" ] || [ -L "$staged/Contents/MacOS/$exe" ] || [ ! -f "$staged/Contents/MacOS/$exe" ]; then echo "its executable is not $exe"; return 1; fi
    if ! /usr/bin/codesign --verify --deep --strict "$staged" 2>/dev/null; then echo "its signature does not verify"; return 1; fi
  else
    if [ -L "$staged" ] || [ ! -f "$staged" ]; then echo "it is not a file of its own"; return 1; fi
    got=$(/usr/bin/sha256sum "$staged" 2>/dev/null) || { echo "it could not be read"; return 1; }
    if [ "${got%% *}" != "$sum" ]; then echo "its sha256 is not the release's"; return 1; fi
  fi
}
n=0
while alive "$pid"; do
  n=$((n + 1))
  if [ "$n" -gt $((patience * 10)) ]; then say "the app (pid $pid) did not quit; nothing was changed"; exit 1; fi
  /bin/sleep 0.1
done
if ! why=$(recheck); then
  say "the checked app changed after it was checked ($why); nothing was changed"
  /bin/rm -rf "$root"
  reopen
  exit 1
fi
say "updating $app to $version"
/bin/rm -rf "$aside" "$failed" "$root/up" "$root/opening" "$root/restarted" "$root/back"
say "moving $app aside to $aside"
if ! /bin/mv "$app" "$aside"; then say "the app could not be moved aside; nothing was changed"; reopen; exit 1; fi
/bin/mv "$staged" "$app" || back "the new app could not be moved into place"
echo $$ > "$root/opening"
launch "$app" "$exe"
newpid=$!
say "opened wsp $version as pid $newpid"
n=0
until [ "$(/bin/cat "$root/up" 2>/dev/null)" = "$version" ]; do
  alive "$newpid" || back "the new app exited before it was up"
  n=$((n + 1))
  if [ "$n" -gt $((patience * 10)) ]; then back "the new app did not start within $patience s"; fi
  /bin/sleep 0.1
done
say "wsp $version is up"
if /bin/rm -rf "$root"; then say "deleted the old app"; else say "the old app could not be deleted; the next launch sweeps it"; fi
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
    ]
}

/// What the script and the app it launches run with (`swapEnv`): the system's own PATH and, of this launch's
/// environment, only what says whose login and which wsp home it is, the person's switch for update checks, which the
/// host the new app restarts keeps, and on Linux which display to draw on.
pub fn env(get: impl Fn(&str) -> Option<String>) -> Vec<(String, String)> {
    let mut kept = vec![("PATH".to_owned(), "/usr/bin:/bin:/usr/sbin:/sbin".to_owned())];
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

fn running(pid: i32) -> bool {
    // SAFETY: signal 0 checks the pid and sends nothing.
    pid > 0 && (unsafe { libc::kill(pid, 0) } == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM))
}

/// What a launch found beside its installed app (`settleStage`).
#[derive(Debug, PartialEq)]
pub enum Settled {
    Nothing,
    /// Whatever an earlier update left was deleted.
    Swept,
    /// A swap opened this app and waits on it: the host follows onto these files, then `up` answers the swap.
    Waiting(Waiting),
    /// A swap put this app back, and the host was moved onto the files that failed, so it follows back onto these.
    Back {
        restart: bool,
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

/// Settles the stage beside `installed` at launch: a swap still running that waits on this app is answered later,
/// one that put this app back is followed and swept, and anything else an earlier update left is deleted.
pub fn settle(installed: &Path) -> Settled {
    let root = stage_of(installed);
    if !own(&root) {
        return Settled::Nothing;
    }
    if waiting_pid(&root).is_some() {
        return Settled::Waiting(Waiting { root });
    }
    let back = std::fs::read_to_string(root.join("back")).ok();
    _ = std::fs::remove_dir_all(&root);
    match back {
        Some(said) => Settled::Back { restart: said.trim() == "restart" },
        None => Settled::Swept,
    }
}
