// SPDX-License-Identifier: AGPL-3.0-only
//! The setup row that lets bwrap make its namespaces for users other than root where Ubuntu's AppArmor stops them.
//! Port of packages/catalog/src/bwrap-apparmor.ts at a3b870a6e; the spec is spec/catalog.md.
use std::sync::LazyLock;

use crate::catalog::SetupNeed;

const PROFILE: &str = "/etc/apparmor.d/bwrap-userns-restrict";

/// bwrap as nobody makes the network namespace Codex's sandbox asks for, which is what a turn as any user but root
/// needs; on failure it prints bwrap's own line, which the setup row reads as its note.
const NOBODY_BWRAP: &str = r#"out="$(setpriv --reuid=65534 --regid=65534 --clear-groups bwrap --ro-bind / / --unshare-net true 2>&1)" || { echo "bwrap fails for users other than root: $out"; exit 1; }"#;

fn load() -> String {
    [
        format!("f={PROFILE}"),
        r#"if [ "$(cat /proc/sys/kernel/apparmor_restrict_unprivileged_userns 2>/dev/null)" != 1 ]; then echo "users other than root cannot run bwrap here, and AppArmor's userns restriction is not what stops them" >&2; exit 1; fi"#.to_string(),
        r#"command -v apparmor_parser >/dev/null 2>&1 || { echo "apparmor_parser is not on this computer to load a profile for bwrap" >&2; exit 1; }"#.to_string(),
        r#"if [ -e "$f" ]; then echo "$f is there but not loaded; apparmor_parser -r $f loads it" >&2; exit 1; fi"#.to_string(),
        r#"d="$(mktemp -d)""#.to_string(),
        // apt's _apt user cannot write into a folder mktemp made for root.
        r#"if ! (cd "$d" && apt-get download -qq -o APT::Sandbox::User=root apparmor-profiles && dpkg-deb --fsys-tarfile apparmor-profiles_*.deb | tar -xO ./usr/share/apparmor/extra-profiles/bwrap-userns-restrict > profile && [ -s profile ]); then rm -rf "$d"; echo "Ubuntu's bwrap-userns-restrict profile could not be read out of apparmor-profiles" >&2; exit 1; fi"#.to_string(),
        r#"install -m 0644 "$d/profile" "$f"; rm -rf "$d""#.to_string(),
        r#"apparmor_parser -r -W "$f" || { rm -f "$f"; echo "AppArmor here refused the bwrap-userns-restrict profile" >&2; exit 1; }"#.to_string(),
        // A failed row is never undone, so a profile that leaves bwrap failing comes off here.
        format!(r#"({NOBODY_BWRAP}) >&2 || {{ apparmor_parser -R "$f" 2>/dev/null; rm -f "$f"; exit 1; }}"#),
    ]
    .join("\n")
}

fn unload() -> String {
    format!(
        r#"f={PROFILE}
if [ -f "$f" ]; then apparmor_parser -R "$f" 2>/dev/null; rm -f "$f" /var/cache/apparmor/*/bwrap-userns-restrict; fi"#
    )
}

/// bwrap-apparmor.ts:33: id `bwrap-apparmor`, after `bubblewrap`, its three scripts built as the TS joins them.
pub static BWRAP_APPARMOR: LazyLock<SetupNeed> = LazyLock::new(|| SetupNeed {
    id: "bwrap-apparmor".to_string(),
    label: "bwrap AppArmor profile".to_string(),
    after: "bubblewrap".to_string(),
    cmd: load(),
    check: NOBODY_BWRAP.to_string(),
    off: unload(),
});
