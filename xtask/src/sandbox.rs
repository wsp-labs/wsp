// SPDX-License-Identifier: AGPL-3.0-only
//! The namespaces and mounts every replay runs inside, which `cargo xtask parity` puts each run in.

/// What the copy inside the namespaces does before it runs: loopback up; every top-level folder but the kernel's, the
/// devices and /tmp read-only, so nothing a replayed command does reaches what is installed on this computer; an empty
/// /run, so no service manager or bus socket answers it; and the parity folder writable again for the goldens, which a
/// bind of it alone does not do: it keeps the read-only flag of the folder it was taken from. Then / as the working
/// folder, since one taken before the binds still writes through the old mount.
pub const SCRIPT: &str = r#"set -e
PATH="$PATH:/usr/sbin:/sbin"
ip link set lo up
mount --make-rprivate /
for d in /*; do
  case "$d" in /proc|/sys|/dev|/tmp|/run) continue ;; esac
  if [ -d "$d" ] && [ ! -L "$d" ]; then mount --bind -o ro "$d" "$d"; fi
done
mount -t tmpfs -o mode=755 tmpfs /run
mount --bind "$WSP_PARITY_WRITABLE" "$WSP_PARITY_WRITABLE"
mount -o remount,bind,rw "$WSP_PARITY_WRITABLE"
cd /
exec "$0" "$@""#;

/// unshare's flags for the copy inside: its own network, mounts and processes; a user of its own for one not root.
pub fn flags(root_user: bool) -> Vec<&'static str> {
    // --kill-child: unshare ends the copy inside if unshare itself is killed.
    let mut flags = vec!["--net", "--mount", "--pid", "--fork", "--mount-proc", "--kill-child"];
    if !root_user {
        flags.push("--map-root-user");
    }
    flags
}
