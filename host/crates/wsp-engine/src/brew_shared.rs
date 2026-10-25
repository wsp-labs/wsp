// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/engine/src/brew-shared.ts. Spec: spec/tool_sizes.md.
//!
//! The shared Homebrew step: the dependencies two or more of a recipe's formulae share, installed in one brew process
//! before any of them, and its check.
use wsp_catalog::road_modules::{as_linuxbrew_script, BREW};
use wsp_wire::types::shell_quote::shell_quote;

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
    let brew = BREW.as_str();
    let mut lines = vec!["set -uo pipefail".to_string()];
    lines.extend(shared_deps_lines(formulae));
    lines.extend(
        [r#"if [ -z "$shared" ]; then echo "no shared dependencies"; exit 0; fi"#, r#"echo "shared: $(echo $shared)""#, r#"err=$(mktemp)"#]
            .map(str::to_string),
    );
    lines.push(format!(r#"{brew} install $shared 2>"$err"; rc=$?"#));
    lines.push(r#"cat "$err" >&2"#.to_string());
    lines.push(format!("{brew} tab --no-installed-on-request $shared || true"));
    lines.push("if [ $rc -ne 0 ]; then".to_string());
    lines.push(format!("  have=$({brew} list --versions $shared 2>/dev/null)"));
    lines.push(format!("  gone=$({})", unlisted(true)));
    lines.push(r#"  last=$(grep -v '^[[:space:]]*$' "$err" | tail -1 | sed 's/^[[:space:]]*//')"#.to_string());
    lines.push(r#"  if [ -n "$gone" ]; then echo "Error: $gone did not install${last:+: ${last#Error: }}" >&2; fi"#.to_string());
    lines.push("fi".to_string());
    lines.push(r#"rm -f "$err""#.to_string());
    lines.push("exit $rc".to_string());
    as_linuxbrew_script(&lines.join("\n"))
}

/// brew-shared.ts:74. The check, wrapped the same way; pinned byte for byte by the oracle.
pub fn brew_shared_check(formulae: &[String]) -> String {
    let brew = BREW.as_str();
    let mut lines = vec!["set -uo pipefail".to_string()];
    lines.extend(shared_deps_lines(formulae));
    lines.push(r#"if [ -z "$shared" ]; then exit 0; fi"#.to_string());
    lines.push(format!(r#"have=$({brew} list --versions $shared 2>&1) && exit 0"#));
    lines.push(r#"case "$have" in *Error:*) printf '%s\n' "$have" | grep Error: | tail -1; exit 1 ;; esac"#.to_string());
    lines.push(unlisted(false));
    lines.push("exit 1".to_string());
    as_linuxbrew_script(&lines.join("\n"))
}

/// brew-shared.ts:20. The lines that put each formula's dependencies in `deps` and those two or more share in `shared`,
/// Homebrew's own toolchain left out.
fn shared_deps_lines(formulae: &[String]) -> Vec<String> {
    let brew = BREW.as_str();
    let quoted: Vec<String> = formulae.iter().map(|f| shell_quote(f)).collect();
    let toolchain: Vec<String> = BREW_TOOLCHAIN.iter().map(|f| format!("-e {f}")).collect();
    vec![
        format!("deps=$({brew} deps --for-each {})", quoted.join(" ")),
        format!(
            r#"shared=$(printf '%s\n' "$deps" | sed 's/^[^:]*: *//' | tr ' ' '\n' | grep -vx -e '' {} | sort | uniq -d || true)"#,
            toolchain.join(" ")
        ),
    ]
}

/// The awk program run over `deps`, with `names=1` where the caller wants the names alone.
fn unlisted(names: bool) -> String {
    let names = if names { "names=1 " } else { "" };
    format!(r#"printf '%s\n' "$deps" | {names}shared="$shared" have="$have" awk '{UNLISTED_AWK}'"#)
}
