// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/dotfiles-presets.ts. Spec: spec/small.md.

/// dotfiles-presets.ts:5. Guest exec is bash -c with no HOME; under set -u the first "$HOME" would abort the script.
pub const PRELUDE: &str = "set -euo pipefail\nexport HOME=\"${HOME:-$(getent passwd \"$(id -u)\" | cut -d: -f6)}\"";

/// dotfiles-presets.ts:8. `command -v <cmd> >/dev/null 2>&1 || { apt-get update -qq; apt-get install -y -qq <pkg>; }`;
/// `cmd` is `pkg` when `None`. Neither is quoted.
pub fn apt(pkg: &str, cmd: Option<&str>) -> String {
    let cmd = cmd.unwrap_or(pkg);
    format!("command -v {cmd} >/dev/null 2>&1 || {{ apt-get update -qq; apt-get install -y -qq {pkg}; }}")
}
