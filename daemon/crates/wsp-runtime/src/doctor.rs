// SPDX-License-Identifier: AGPL-3.0-only
//! What this computer can and cannot do as a place, read off the kernel without mounting anything, so it can run
//! at every dial and land in the report the host reads. Three things a person is told: whether workspaces run
//! here, and if not the one kernel reason why; whether the box has an engine for a project's own containers,
//! which is not this crate's to build; and whether it has KVM, for the microVM road. The authoritative gate
//! when a fork is actually asked for is still the self check, which mounts an overlay and makes a cgroup; this is
//! the read-only twin of it, so what the doctor says and what a create does cannot part ways on the same box.

use std::path::Path;

/// The container engine a project's own `docker compose` would run on, when the box has one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    None,
    Docker,
    Podman,
}

impl Engine {
    /// The word the report carries and the host reads.
    pub fn word(self) -> &'static str {
        match self {
            Engine::None => "none",
            Engine::Docker => "docker",
            Engine::Podman => "podman",
        }
    }
}

/// The kernel facts the doctor reads, split out so a test can hand it a box it is not running on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Facts {
    /// This computer runs Linux, which the kernel work a workspace is built from needs; the rest of the facts are
    /// read only where it does.
    pub linux: bool,
    /// cgroup v2 unified is mounted at the cgroup root.
    pub cgroup2: bool,
    /// The controllers the cgroup root delegates, when it is v2.
    pub controllers: Vec<String>,
    /// overlay is a filesystem this kernel knows, or why it is not.
    pub overlay: Overlay,
    /// This process is root, which is the mode the daemon runs workspaces in on a box.
    pub root: bool,
    /// /dev/kvm is here, so the microVM road is open on this box.
    pub kvm: bool,
    /// The engine a project's own containers would run on.
    pub engine: Engine,
}

/// Whether this kernel can mount an overlay, and when it cannot, which of the two ways it is missing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Overlay {
    Known,
    /// The kernel has no overlay built in and no module of it to load.
    Absent,
    /// The module is there and would not load, in modprobe's own last line.
    WontLoad(String),
}

/// What the doctor tells the host about this box: enough for the three sentences, no English of its own, since the
/// words a person reads live in one place above the engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doctor {
    /// The daemon runs workspaces here.
    pub runs_workspaces: bool,
    /// When it does not, the one kernel reason, in the same words the self check refuses with.
    pub blocked: Option<String>,
    pub kvm: bool,
    pub engine: Engine,
}

/// The controllers a memory cap and a cpu quota need.
const WANTED_CONTROLLERS: [&str; 2] = ["memory", "cpu"];

/// The doctor's reading of a box, from its facts alone: workspaces run here when the kernel offers cgroup v2 with
/// the controllers a cap needs and an overlay to read its own directories through, and the daemon is root; else
/// the first thing
/// missing is the reason, in the self check's own words so the two never disagree.
pub fn assess(facts: &Facts) -> Doctor {
    let blocked = blocking_reason(facts);
    Doctor { runs_workspaces: blocked.is_none(), blocked, kvm: facts.kvm, engine: facts.engine }
}

fn blocking_reason(facts: &Facts) -> Option<String> {
    if !facts.linux {
        return Some("wsp runs workspaces on a Linux computer, and this computer is not one".to_owned());
    }
    if !facts.cgroup2 {
        return Some(
            "this computer mounts cgroup v1 at /sys/fs/cgroup, and wsp runs workspaces on cgroup v2 alone: boot it with systemd.unified_cgroup_hierarchy=1".to_owned(),
        );
    }
    for wanted in WANTED_CONTROLLERS {
        if !facts.controllers.iter().any(|c| c == wanted) {
            return Some(format!("this computer's cgroup root offers no {wanted} controller, which wsp needs to run workspaces here"));
        }
    }
    match &facts.overlay {
        Overlay::Known => {}
        Overlay::Absent => {
            return Some(
                "this computer's kernel has no overlay filesystem and no overlay module to load, which a workspace here reads this computer's own directories through"
                    .to_owned(),
            );
        }
        Overlay::WontLoad(said) => {
            return Some(format!(
                "this computer's kernel has an overlay module that would not load ({said}), and a workspace here reads this computer's own directories through an overlay"
            ));
        }
    }
    if !facts.root {
        return Some("wsp runs workspaces on this computer as root, and this daemon is not root".to_owned());
    }
    None
}

/// The facts read off this box: nothing is mounted, created or written, so it costs a few file reads and can
/// run at every dial and on every engine socket lookup; the one act is loading the overlay module where the
/// kernel does not list it yet. Off Linux only the engine is read; the rest stand false,
/// and the doctor names Linux as the reason a workspace does not run here, since youki's kernel work has no
/// answer on another kernel. How a copy is made here is not a kernel fact but a probe on the disk, so it is read
/// where it is reported and where a copy is about to be made, never here.
pub fn read_facts() -> Facts {
    let engine = engine_on_path(&std::env::var("PATH").unwrap_or_default());
    #[cfg(target_os = "linux")]
    {
        let controllers = std::fs::read_to_string(Path::new(crate::cgroup::CGROUP_ROOT).join("cgroup.controllers"));
        let cgroup2 = controllers.is_ok();
        Facts {
            linux: true,
            cgroup2,
            controllers: controllers.map(|text| text.split_whitespace().map(str::to_owned).collect()).unwrap_or_default(),
            overlay: overlay_on(Path::new("/proc/filesystems"), Path::new(MODPROBE)),
            root: euid_is_root(),
            kvm: Path::new("/dev/kvm").exists(),
            engine,
        }
    }
    #[cfg(not(target_os = "linux"))]
    {
        Facts { linux: false, cgroup2: false, controllers: Vec::new(), overlay: Overlay::Absent, root: false, kvm: false, engine }
    }
}

/// The kernel's own module loader on a merged-usr computer, which is the only kind a workspace runs on.
#[cfg(target_os = "linux")]
const MODPROBE: &str = "/sbin/modprobe";

/// Whether the kernel lists overlay in /proc/filesystems, loading the module first where it does not yet: a box that
/// booted a new kernel has the module on disk and lists nothing until something mounts one. A load that fails is
/// told apart by modprobe's dry run, which fails only where there is no module to load.
#[cfg(any(target_os = "linux", test))]
fn overlay_on(filesystems: &Path, modprobe: &Path) -> Overlay {
    use std::process::{Command, Stdio};
    let listed = || std::fs::read_to_string(filesystems).is_ok_and(|text| text.split_whitespace().any(|word| word == "overlay"));
    if listed() {
        return Overlay::Known;
    }
    let said = match Command::new(modprobe).arg("overlay").stdin(Stdio::null()).output() {
        Ok(out) if out.status.success() && listed() => return Overlay::Known,
        Ok(out) => String::from_utf8_lossy(&out.stderr).lines().last().unwrap_or_default().trim().to_owned(),
        Err(e) => return Overlay::WontLoad(format!("{} could not run: {e}", modprobe.display())),
    };
    let dry = Command::new(modprobe).args(["-n", "overlay"]).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).status();
    if !dry.is_ok_and(|status| status.success()) {
        return Overlay::Absent;
    }
    Overlay::WontLoad(if said.is_empty() { "modprobe loaded it and the kernel still lists no overlay".to_owned() } else { said })
}

/// The effective uid is root, the mode the daemon runs workspaces in on a box. nix is here on Linux, which is the
/// only target this reads on.
#[cfg(target_os = "linux")]
fn euid_is_root() -> bool {
    nix::unistd::geteuid().is_root()
}

/// The computer's own system directories, one overlay each: what a workspace here is made of. Each is the box's
/// directory as it is now, read through an upper of the workspace's own, so a workspace installs a package and
/// the box does not have it, and the box upgrades its tools and a workspace that boots next reads them. Named in
/// the frames crate beside the shared tool roots outside them, since the host's catalogue is held to the same
/// list: a road that installs outside these trees and outside /root is in no workspace unless a root brings it in.
pub use wsp_frames::numbers::OVERLAID;

/// The four names a computer with merged usr keeps as symlinks into /usr. A workspace's rootfs recreates them as
/// the links they are and reads everything under them through the overlay over /usr; a computer that keeps them
/// as directories of their own would hand a workspace no shell and no library at all, since nothing here overlays
/// them.
pub const MERGED_INTO_USR: [&str; 4] = ["bin", "sbin", "lib", "lib64"];

/// Why a workspace cannot be made of this computer's directories: it keeps one of the four names above as a
/// directory of its own rather than as a link into /usr. One sentence naming them, since the person can neither
/// merge their root nor guess why a workspace came up with no shell. Nothing where every one of them is a link or
/// absent, which every merged-usr distribution is.
pub fn root_not_merged(directories: &[&str]) -> Option<String> {
    let its_own: Vec<&str> = MERGED_INTO_USR.iter().copied().filter(|name| directories.contains(name)).collect();
    if its_own.is_empty() {
        return None;
    }
    Some(format!(
        "this computer keeps /{} of its own rather than as a link into /usr, and a workspace here is made of /usr alone: wsp runs workspaces on a computer with a merged /usr",
        its_own.join(", /")
    ))
}

/// Why this computer runs no workspace under the root it was given: the root sits inside one of the directories
/// every workspace overlays, so a workspace's upper directory would sit inside the tree that workspace reads
/// through the overlay. The kernel takes that mount (measured on 6.8.0-139) and what it gives is a workspace
/// reading its own upper, and every other workspace's, inside the directory it overlays. One sentence naming
/// both paths, since the fix is to move the root and the person has to know what to move it out of. Nothing
/// where the root is clear of all five, which `/wsp` is.
pub fn root_under_a_lower(root: &Path) -> Option<String> {
    let lower = OVERLAID.iter().find(|lower| root.starts_with(lower))?;
    Some(format!(
        "{} sits under {lower}, which every workspace here reads through an overlay; a workspace's upper would then sit inside the tree it overlays, so the root belongs somewhere else, {} by default",
        root.display(),
        crate::DEFAULT_ROOT
    ))
}

/// The engine a project's containers would run on: Docker where its cli is on the PATH, else podman, else none.
pub fn engine_on_path(path: &str) -> Engine {
    if on_path("docker", path) {
        Engine::Docker
    } else if on_path("podman", path) {
        Engine::Podman
    } else {
        Engine::None
    }
}

pub fn on_path(name: &str, path: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;
    path.split(':').filter(|dir| !dir.is_empty()).any(|dir| {
        std::fs::metadata(Path::new(dir).join(name)).map(|m| m.is_file() && m.permissions().mode() & 0o111 != 0).unwrap_or(false)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn box_that_runs() -> Facts {
        Facts {
            linux: true,
            cgroup2: true,
            controllers: ["cpuset", "cpu", "io", "memory", "pids"].iter().map(|s| (*s).to_owned()).collect(),
            overlay: Overlay::Known,
            root: true,
            kvm: true,
            engine: Engine::None,
        }
    }

    #[test]
    fn a_box_with_cgroup_v2_the_controllers_an_overlay_and_root_runs_workspaces() {
        let d = assess(&box_that_runs());
        assert!(d.runs_workspaces);
        assert_eq!(d.blocked, None);
        assert!(d.kvm);
        assert_eq!(d.engine, Engine::None);
    }

    #[test]
    fn reading_the_box_writes_nothing_on_it_and_the_copy_word_is_asked_for_on_its_own() {
        // The engine socket lookup reads these facts on every create, every wake and every remove, so the read
        // may not touch a disk: it takes no root and can write under none. How a copy is made here is a probe
        // rather than a kernel fact, and it lives in the copy module, which the report and the create call.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("root");
        for _ in 0..3 {
            let _ = read_facts();
        }
        assert!(!root.exists(), "reading the box made something under a root");
        let word = crate::copy::copies_word(&root);
        assert!(matches!(word, wsp_frames::CopyWord::Plain | wsp_frames::CopyWord::Reflink), "{word:?}");
        // And the probe that answered it left the root as it found it.
        assert_eq!(std::fs::read_dir(&root).map(Iterator::count).unwrap_or(0), 0);
    }

    #[test]
    fn a_box_on_cgroup_v1_cannot_and_the_reason_names_the_boot_flag() {
        let d = assess(&Facts { cgroup2: false, controllers: vec![], ..box_that_runs() });
        assert!(!d.runs_workspaces);
        assert!(d.blocked.as_deref().unwrap().contains("systemd.unified_cgroup_hierarchy=1"), "{:?}", d.blocked);
    }

    #[test]
    fn a_v2_box_missing_the_memory_controller_cannot_and_the_reason_names_it() {
        let d = assess(&Facts { controllers: ["cpu", "pids"].iter().map(|s| (*s).to_owned()).collect(), ..box_that_runs() });
        assert!(!d.runs_workspaces);
        assert!(d.blocked.as_deref().unwrap().contains("no memory controller"), "{:?}", d.blocked);
    }

    #[test]
    fn a_box_without_an_overlay_or_without_root_cannot_and_each_reason_is_its_own() {
        assert!(assess(&Facts { overlay: Overlay::Absent, ..box_that_runs() }).blocked.as_deref().unwrap().contains("overlay filesystem"));
        assert!(assess(&Facts { root: false, ..box_that_runs() }).blocked.as_deref().unwrap().contains("as root"));
    }

    /// A kernel that lists no overlay, beside a modprobe that answers as kmod 31 does (measured on 6.8.0-117):
    /// a module it loads lands in /proc/filesystems, one with no file fails both the load and the dry run with
    /// "not found", and one that is there but refused fails the load alone.
    fn kernel_and_modprobe(modprobe: &str) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let filesystems = dir.path().join("filesystems");
        std::fs::write(&filesystems, "nodev\tsysfs\n\text4\n").unwrap();
        let script = dir.path().join("modprobe");
        let body = modprobe.replace("FILESYSTEMS", &filesystems.to_string_lossy());
        let text = format!("#!/bin/sh\necho \"$*\" >> {}/asked\n{body}\n", dir.path().display());
        // A child writes it: a fork in this binary while it was open here for writing would make its exec read ETXTBSY.
        let wrote = std::process::Command::new("/bin/sh")
            .args(["-c", "printf %s \"$1\" > \"$2\" && chmod 755 \"$2\"", "sh", text.as_str()])
            .arg(&script)
            .status()
            .unwrap();
        assert!(wrote.success());
        (dir, filesystems, script)
    }

    fn asked(dir: &tempfile::TempDir) -> String {
        std::fs::read_to_string(dir.path().join("asked")).unwrap_or_default()
    }

    #[test]
    fn an_overlay_the_kernel_lists_is_known_without_asking_modprobe() {
        let (dir, filesystems, modprobe) = kernel_and_modprobe("exit 1");
        std::fs::write(&filesystems, "nodev\tsysfs\nnodev\toverlay\n").unwrap();
        assert_eq!(overlay_on(&filesystems, &modprobe), Overlay::Known);
        assert_eq!(asked(&dir), "");
    }

    #[test]
    fn an_overlay_module_not_loaded_yet_is_loaded_and_read_again() {
        let (dir, filesystems, modprobe) = kernel_and_modprobe("[ \"$1\" = -n ] && exit 0\nprintf 'nodev\\toverlay\\n' >> FILESYSTEMS");
        assert_eq!(overlay_on(&filesystems, &modprobe), Overlay::Known);
        assert_eq!(asked(&dir), "overlay\n");
        assert!(assess(&Facts { overlay: overlay_on(&filesystems, &modprobe), ..box_that_runs() }).runs_workspaces);
    }

    #[test]
    fn a_kernel_with_no_overlay_module_is_refused_as_absent() {
        let (dir, filesystems, modprobe) =
            kernel_and_modprobe("echo 'modprobe: FATAL: Module overlay not found in directory /lib/modules/6.8.0-146-generic' >&2\nexit 1");
        assert_eq!(overlay_on(&filesystems, &modprobe), Overlay::Absent);
        assert_eq!(asked(&dir), "overlay\n-n overlay\n");
        let said = assess(&Facts { overlay: Overlay::Absent, ..box_that_runs() }).blocked.unwrap();
        assert_eq!(
            said,
            "this computer's kernel has no overlay filesystem and no overlay module to load, which a workspace here reads this computer's own directories through"
        );
    }

    #[test]
    fn an_overlay_module_that_will_not_load_is_refused_in_modprobes_own_words() {
        let (_dir, filesystems, modprobe) = kernel_and_modprobe(
            "[ \"$1\" = -n ] && exit 0\necho 'modprobe: ERROR: could not insert '\"'\"'overlay'\"'\"': Operation not permitted' >&2\nexit 1",
        );
        let overlay = overlay_on(&filesystems, &modprobe);
        assert_eq!(overlay, Overlay::WontLoad("modprobe: ERROR: could not insert 'overlay': Operation not permitted".to_owned()));
        assert_eq!(
            assess(&Facts { overlay, ..box_that_runs() }).blocked.unwrap(),
            "this computer's kernel has an overlay module that would not load (modprobe: ERROR: could not insert 'overlay': Operation not permitted), and a workspace here reads this computer's own directories through an overlay"
        );
    }

    #[test]
    fn the_kvm_and_the_engine_travel_whatever_the_kernel_says_about_workspaces() {
        let d = assess(&Facts { kvm: false, engine: Engine::Docker, ..box_that_runs() });
        assert!(d.runs_workspaces && !d.kvm && d.engine == Engine::Docker);
        assert_eq!(Engine::Podman.word(), "podman");
    }

    /// The one reading that decides whether a workspace can be built under a root at all, in the words the open
    /// and the self check both refuse with: a root under any of the five directories a workspace overlays, named
    /// with the lower it sits under, and nothing for the default root. The sentence says what is wrong with the
    /// root and claims nothing about the kernel, which takes the mount and gives a workspace its own upper to
    /// read inside the directory it overlays.
    #[test]
    fn a_root_under_a_directory_every_workspace_overlays_is_named_with_that_directory() {
        for (root, lower) in
            [("/var/wsp", "/var"), ("/usr/local/wsp", "/usr"), ("/etc/wsp", "/etc"), ("/opt/wsp", "/opt"), ("/srv/wsp", "/srv")]
        {
            let said = root_under_a_lower(Path::new(root)).unwrap_or_else(|| panic!("{root} read as clear of {lower}"));
            assert!(said.contains(root) && said.contains(lower), "{said}");
            assert!(said.contains(crate::DEFAULT_ROOT), "{said}");
            // What is wrong is where the upper would sit, which is this daemon's own doing; the kernel takes
            // that mount, so the sentence may not blame it.
            assert!(said.contains("upper would then sit inside the tree it overlays"), "{said}");
            assert!(!said.contains("kernel"), "{said}");
        }
        for clear in [crate::DEFAULT_ROOT, "/wsp/one", "/root/wsp-roots/plain-root", "/tmp/x", "/home/z/wsp"] {
            assert_eq!(root_under_a_lower(Path::new(clear)), None, "{clear}");
        }
        // A name that only starts with a lower's letters is not under it.
        assert_eq!(root_under_a_lower(Path::new("/vary/wsp")), None);
    }

    /// A computer that keeps /bin or /lib of its own: every tool a workspace runs would be missing from it, so
    /// the reading names what it found and the create says so rather than booting a rootfs with no shell.
    #[test]
    fn a_root_that_is_not_merged_into_usr_is_named_with_the_directories_it_keeps() {
        assert_eq!(root_not_merged(&[]), None);
        assert_eq!(root_not_merged(&["etc", "var", "opt", "home"]), None);
        let said = root_not_merged(&["bin", "etc", "lib"]).expect("a root with its own /bin read as merged");
        assert!(said.contains("/bin, /lib") && said.contains("merged /usr"), "{said}");
        assert!(root_not_merged(&["lib64"]).unwrap().contains("/lib64"));
        // The order is the one written down, not the one the directory handed back.
        assert!(root_not_merged(&["lib", "bin"]).unwrap().contains("/bin, /lib"));
    }

    #[test]
    fn the_engine_is_read_off_the_path_docker_first_then_podman() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        let path = bin.to_string_lossy().into_owned();
        let put = |name: &str| {
            let p = bin.join(name);
            std::fs::write(&p, "#!/bin/sh\n").unwrap();
            std::fs::set_permissions(&p, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
        };
        assert_eq!(engine_on_path(&path), Engine::None);
        assert_eq!(engine_on_path(""), Engine::None);
        put("podman");
        assert_eq!(engine_on_path(&path), Engine::Podman);
        put("docker");
        assert_eq!(engine_on_path(&path), Engine::Docker);
    }

    #[test]
    fn a_computer_that_is_not_linux_cannot_run_workspaces_and_the_reason_says_so() {
        // The doctor is built for every target the daemon builds for, so it answers a Mac honestly rather than
        // failing to compile: no cgroup and no overlay to read, and Linux named as the reason a workspace does not
        // run there. The engine still travels, since a project's own containers are the box's own affair.
        let d = assess(&Facts {
            linux: false,
            cgroup2: false,
            controllers: vec![],
            overlay: Overlay::Absent,
            root: false,
            kvm: false,
            engine: Engine::Docker,
        });
        assert!(!d.runs_workspaces);
        assert!(d.blocked.as_deref().unwrap().contains("Linux computer"), "{:?}", d.blocked);
        assert_eq!(d.engine, Engine::Docker);
    }
}
