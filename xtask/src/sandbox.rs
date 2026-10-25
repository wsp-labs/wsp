// SPDX-License-Identifier: AGPL-3.0-only
//! The namespaces and mounts every replay, every unit's own tests and every collection run inside, which
//! `cargo xtask parity` and `cargo xtask unit` put each run in.

/// What the copy inside the namespaces does before it runs. A read-only mount does not stop a `connect()` to a unix
/// socket, so every socket a process of the computer listens on by path is covered with /dev/null first, read off the
/// computer's /proc before a /proc of its own replaces it: /root/.wsp/daemon.sock is a door to the live host where the
/// file itself is the gate. The live host's folders are covered whole as well, unless the checkout is inside one. Then
/// every mount, the root of the disk and the kernel's own files included, is made read-only on this namespace's side
/// alone (`remount,bind,ro`; a plain `remount,ro` would flip the computer's own disk for every process). A socket or a
/// mount point that goes away meanwhile is out of reach already; anything else that refuses ends the run. On top go a
/// /tmp and a /run of its own, a /dev with no disk in it, a read-only /proc of its own processes, and only the
/// folders `WSP_PARITY_WRITABLE` names, split by colons, writable again: none for a check, so the host under test
/// cannot rewrite the goldens it is judged by. Then the caller's folder (`WSP_PARITY_CWD`, else /) entered again, since
/// one taken before the binds still writes through the old mount.
pub const SCRIPT: &str = r#"set -e
PATH="$PATH:/usr/sbin:/sbin"
ip link set lo up
mount --make-rprivate /
cat /proc/[0-9]*/net/unix 2>/dev/null | awk '$8 ~ /^\// { p = $8; for (i = 9; i <= NF; i++) p = p " " $i; print p }' | sort -u |
while IFS= read -r s; do
  if [ -S "$s" ] && ! mount --bind /dev/null "$s" 2>/dev/null && [ -S "$s" ]; then
    echo "cannot cover the socket $s" >&2
    exit 1
  fi
done
awk '$6 !~ /^ro(,|$)/ { print $5 }' /proc/self/mountinfo | sed 's/\\040/ /g' | while IFS= read -r m; do
  if ! mount -o remount,bind,ro "$m" 2>/dev/null && [ -e "$m" ]; then
    echo "cannot make $m read-only" >&2
    exit 1
  fi
done
for d in "$HOME/.wsp" "${WSP_HOME:-}" /root/.wsp; do
  [ -n "$d" ] && [ -d "$d" ] || continue
  case "${WSP_PARITY_ROOT:-/nonexistent}/" in "$d"/*) continue ;; esac
  mount -t tmpfs -o ro,mode=700 tmpfs "$d"
done
mount -t tmpfs -o mode=1777 tmpfs /tmp
mount -t tmpfs -o mode=755 tmpfs /run
mkdir /run/dev
mount -t tmpfs -o mode=755,nosuid tmpfs /run/dev
for n in null zero full random urandom tty; do
  touch "/run/dev/$n"
  mount --bind "/dev/$n" "/run/dev/$n"
done
mkdir /run/dev/pts /run/dev/shm
mount -t devpts -o newinstance,ptmxmode=0666,mode=620 devpts /run/dev/pts
ln -s pts/ptmx /run/dev/ptmx
mount -t tmpfs -o mode=1777,nosuid,nodev tmpfs /run/dev/shm
ln -s /proc/self/fd /run/dev/fd
ln -s /proc/self/fd/0 /run/dev/stdin
ln -s /proc/self/fd/1 /run/dev/stdout
ln -s /proc/self/fd/2 /run/dev/stderr
mount --move /run/dev /dev
rmdir /run/dev
mount -t proc -o ro,nosuid,nodev,noexec proc /proc
if [ -n "${WSP_PARITY_WRITABLE:-}" ]; then
  IFS=:
  for w in $WSP_PARITY_WRITABLE; do
    mount --bind "$w" "$w"
    mount -o remount,bind,rw "$w"
  done
  unset IFS
fi
cd "${WSP_PARITY_CWD:-/}"
exec "$0" "$@""#;

/// unshare's flags for the copy inside: its own network, mounts, processes, host name and IPC; a user of its own for
/// one not root.
pub fn flags(root_user: bool) -> Vec<&'static str> {
    // --kill-child: unshare ends the copy inside if unshare itself is killed.
    let mut flags = vec!["--net", "--mount", "--pid", "--fork", "--uts", "--ipc", "--kill-child"];
    if !root_user {
        flags.push("--map-root-user");
    }
    flags
}
