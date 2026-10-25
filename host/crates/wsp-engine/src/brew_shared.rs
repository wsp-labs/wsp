// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/brew-shared.ts. Spec: spec/tool_sizes.md.
//!
//! The shared Homebrew step: the dependencies two or more of a recipe's formulae share, installed in one brew process
//! before any of them, and its check.

/// brew-shared.ts:12. Homebrew's own toolchain, each its own single brew process, in this order, before any formula.
pub const BREW_TOOLCHAIN: [&str; 2] = ["glibc", "gcc"];

/// brew-shared.ts:27. The mawk program that names what brew did not list; byte for byte the TS `String.raw`.
pub const UNLISTED_AWK: &str = r#"{ f = $1; sub(/:$/, "", f); for (i = 2; i <= NF; i++) if ($i in users) users[$i] = users[$i] SUBSEP f; else users[$i] = f }
END {
  m = split(ENVIRON["have"], h, "\n"); for (i = 1; i <= m; i++) { split(h[i], w, " "); got[w[1]] = 1 }
  n = split(ENVIRON["shared"], s, /[ \n]+/); k = 0
  for (i = 1; i <= n; i++) { b = s[i]; sub(/.*\//, "", b); if (s[i] != "" && !(b in got)) gone[++k] = s[i] }
  out = ""
  for (i = 1; i <= k && i <= 3; i++) {
    if (ENVIRON["names"] != "") { out = out (i == 1 ? "" : (i == k ? " and " : ", ")) gone[i]; continue }
    u = split(users[gone[i]], who, SUBSEP); list = ""
    for (j = 1; j <= u; j++) list = list (j == 1 ? "" : (j == u ? " and " : ", ")) who[j]
    out = out (i == 1 ? "" : " ") gone[i] " is not installed; " list (u == 1 ? " needs" : " need") " it."
  }
  if (k > 3) out = out (ENVIRON["names"] != "" ? " and " (k - 3) " more" : " " (k - 3) (k == 4 ? " more shared dependency is" : " more shared dependencies are") " not installed.")
  if (k > 0) print out
}"#;

/// brew-shared.ts:47. The install step, wrapped by `as_linuxbrew_script`; pinned byte for byte by the oracle.
pub fn brew_shared_deps(formulae: &[String]) -> String {
    todo!()
}

/// brew-shared.ts:74. The check, wrapped the same way; pinned byte for byte by the oracle.
pub fn brew_shared_check(formulae: &[String]) -> String {
    todo!()
}
