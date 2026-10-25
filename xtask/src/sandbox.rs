// SPDX-License-Identifier: AGPL-3.0-only
//! The namespaces and mounts every replay, every unit's own tests and every collection run inside, which
//! `cargo xtask parity` and `cargo xtask unit` put each run in.

/// What the copy inside the namespaces does before it runs. A read-only mount does not stop a `connect()` to a unix
/// socket, so every socket a process of the computer listens on by path is covered with /dev/null first, read off the
/// computer's /proc before a /proc of its own replaces it: /root/.wsp/daemon.sock is a door to the live host where the
/// file itself is the gate. Then every mount, the root of the disk and the kernel's own files included, is made
/// read-only on this namespace's side alone (`remount,bind,ro`; a plain `remount,ro` would flip the computer's own disk
/// for every process). A socket or a mount point that goes away meanwhile is out of reach already; anything else that
/// refuses ends the run. A socket bound after that scan is a door all the same, so the folders where the computer's
/// other processes bind them (the home folders and /var/tmp) are covered whole and only what the run needs is put back,
/// read-only: the command itself, the checkout (`WSP_PARITY_ROOT`), the caller's folder (`WSP_PARITY_CWD`), the
/// writable folders, the toolchains' homes (cargo's, rustup's, corepack's), the top folder of each PATH entry and whatever `WSP_PARITY_KEEP` names,
/// split by colons. The live host's folders are covered whole as well, unless the checkout is inside one. On top go a
/// /tmp and a /run of its own, a /dev with no disk in it, a read-only /proc of its own processes, and only the folders
/// `WSP_PARITY_WRITABLE` names, split by colons, writable again: none for a check, so the host under test cannot
/// rewrite the goldens it is judged by. A `node_modules` in a writable folder stays read-only, since pnpm hard-links
/// each of its files to the one store every checkout on the computer shares. Then the caller's folder (else /) entered
/// again, since one taken before the binds still writes through the old mount, and the command run as root with no
/// capability left: root that keeps them can unmount /proc and write the computer's disk through /proc/1/root.
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
mount -t tmpfs -o mode=755 tmpfs /run
mkdir /run/keep
tops=/root:/home:/var/tmp
set -f
IFS=:
kept() {
  for t in $tops; do
    case "$1" in "$t"/*) if [ -e "$1" ]; then printf '%s\n' "$1"; fi ;; esac
  done
}
{
  kept "$0"
  for k in ${WSP_PARITY_ROOT:-} ${WSP_PARITY_CWD:-} ${WSP_PARITY_WRITABLE:-} ${CARGO_HOME:-$HOME/.cargo} ${RUSTUP_HOME:-$HOME/.rustup} ${COREPACK_HOME:-$HOME/.cache/node/corepack} ${WSP_PARITY_KEEP:-}; do
    kept "$k"
  done
  for p in $PATH; do
    for t in $tops; do
      case "$p" in "$t"/*) r=${p#"$t"/}; kept "$t/${r%%/*}" ;; esac
    done
  done
} | LC_ALL=C sort -u > /run/keep/list
n=0
while IFS= read -r k; do
  n=$((n + 1))
  if [ -d "$k" ]; then mkdir "/run/keep/$n"; else touch "/run/keep/$n"; fi
  mount --rbind "$k" "/run/keep/$n"
done < /run/keep/list
for t in $tops; do
  if [ -d "$t" ]; then mount -t tmpfs -o "mode=$(stat -c %a "$t")" tmpfs "$t"; fi
done
n=0
while IFS= read -r k; do
  n=$((n + 1))
  if [ -d "/run/keep/$n" ]; then mkdir -p "$k"; else mkdir -p "$(dirname "$k")"; touch "$k"; fi
  mount --move "/run/keep/$n" "$k"
  mount -o remount,bind,ro "$k"
done < /run/keep/list
for t in $tops; do
  if [ -d "$t" ]; then mount -o remount,ro "$t"; fi
done
unset IFS
set +f
rm -r /run/keep
for d in "$HOME/.wsp" "${WSP_HOME:-}" /root/.wsp; do
  [ -n "$d" ] && [ -d "$d" ] || continue
  case "${WSP_PARITY_ROOT:-/nonexistent}/" in "$d"/*) continue ;; esac
  mount -t tmpfs -o ro,mode=700 tmpfs "$d"
done
mount -t tmpfs -o mode=1777 tmpfs /tmp
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
    find "$w" -maxdepth 3 -name node_modules -type d -prune | while IFS= read -r m; do
      mount --bind "$m" "$m"
      mount -o remount,bind,ro "$m"
    done
  done
  unset IFS
fi
cd "${WSP_PARITY_CWD:-/}"
exec setpriv --bounding-set=-all --inh-caps=-all --no-new-privs -- "$0" "$@""#;

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
