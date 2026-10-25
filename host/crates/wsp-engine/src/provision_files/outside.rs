// SPDX-License-Identifier: AGPL-3.0-only
//! provision-files.ts:649-796 at a4033dc34: what a setup writes outside the home, recorded around each install step
//! and taken off by a leave. Spec: spec/files_outside.md.
use wsp_wire::types::daemon_contract::TOOL_PREFIX;
use wsp_wire::types::shell_quote::shell_quote;

/// provision-files.ts:650.
pub const OUTSIDE_MARK: &str = "wsp-outside";
/// provision-files.ts:654. The last line of a listing that ran to its end.
pub const LISTED: &str = "wsp-listed";

/// The TS's `TAB` and `NL`: two-character escapes written into the scripts, which the shell's printf and awk read.
const TAB: &str = r"\t";
const NL: &str = r"\n";

fn lines(text: &[&str]) -> Vec<String> {
    text.iter().map(|s| s.to_string()).collect()
}

/// provision-files.ts:660 `outsideHead(root)`.
pub fn outside_head(root: &str) -> Vec<String> {
    vec![
        "set -u".to_string(),
        format!(r#"root={}; prefix="$root{TOOL_PREFIX}"; ledger="$prefix/landed""#, shell_quote(root)),
        r#"nl=$(printf '\nx'); nl=${nl%x}"#.to_string(),
        format!(r#"tab=$(printf "{TAB}")"#),
    ]
}

/// provision-files.ts:667 `OUTSIDE_STATES`, the `states` shell function's lines.
pub fn outside_states() -> Vec<String> {
    let mut out = lines(&[
        "states() {",
        r#"  : > "$1.files""#,
        "  while IFS= read -r p; do",
    ]);
    out.push(format!(r#"    if [ -L "$p" ]; then printf 'link:%s{TAB}%s{NL}' "$(readlink "$p" | sha256sum | cut -c1-64)" "$p""#));
    out.push(format!(r#"    elif [ -d "$p" ]; then printf 'dir{TAB}%s{NL}' "$p""#));
    out.push(r#"    elif [ -f "$p" ]; then printf '%s\000' "$p" >> "$1.files""#.to_string());
    out.extend(lines(&["    fi", "  done"]));
    out.push(format!(
        r#"  [ ! -s "$1.files" ] || xargs -0 sha256sum -z -- < "$1.files" | tr '\000' '\n' | awk '{{ print substr($0, 1, 64) "{TAB}" substr($0, 67) }}'"#
    ));
    out.extend(lines(&[r#"  rm -f "$1.files""#, "}"]));
    out
}

/// provision-files.ts:684 `OUTSIDE_STILL`, the `still` shell function's lines.
pub fn outside_still() -> Vec<String> {
    let mut out = lines(&[
        "still() {",
        r#"  find "$root/usr/local" "$root/opt" \( -path "$prefix" -o -path "*$nl*" \) -prune -o -type d -print > "$1.dirs" 2>/dev/null"#,
    ]);
    out.push(format!(
        r#"  wsp_root="$root" wsp_prefix="$prefix" awk -F"{TAB}" 'NR == FNR {{ d[$0] = 1; next }} {{ s[substr($0, length($1) + 2)] = $1 }} END {{"#
    ));
    out.extend(lines(&[
        r#"    r = ENVIRON["wsp_root"]; px = ENVIRON["wsp_prefix"]"#,
        "    for (p in s) {",
        r#"      q = p; sub(/\/[^\/]*$/, "", q)"#,
        r#"      if (index(p, r "/usr/local/") != 1 && index(p, r "/opt/") != 1) continue"#,
        r#"      if (p == px || index(p, px "/") == 1 || index(p "/", "/./") || index(p "/", "/../") || index(p, "//") || !(q in d)) continue"#,
    ]));
    out.push(format!(r#"      print s[p] "{TAB}" p"#));
    out.extend(lines(&["    }", r#"  }' "$1.dirs" "$ledger" > "$1.latest""#]));
    out.extend(lines(&[r#"  cut -f2- "$1.latest" | states "$1" > "$1.now""#]));
    out.push(format!(
        r#"  awk -F"{TAB}" 'NR == FNR {{ s[substr($0, length($1) + 2)] = $1; next }} {{ p = substr($0, length($1) + 2); if ((p in s) && s[p] == $1) print }}' "$1.latest" "$1.now""#
    ));
    out.extend(lines(&[r#"  rm -f "$1.dirs" "$1.latest" "$1.now""#, "}"]));
    out
}

/// provision-files.ts:705 `outsideListing(walked, out, tests = "")`.
pub fn outside_listing(walked: &[String], out: &str, tests: &str) -> Vec<String> {
    let from: Vec<String> = walked.iter().map(|dir| format!(r#""$root"{}"#, shell_quote(dir))).collect();
    vec![
        format!(
            r#"find {} \( -path "$prefix" -o -path "*$nl*" \) -prune -o {tests}-print > "{out}.found" 2>/dev/null"#,
            from.join(" ")
        ),
        format!(
            r#"[ $? -lt 128 ] && LC_ALL=C sort "{out}.found" > "{out}.part" && echo {LISTED} >> "{out}.part" && mv "{out}.part" "{out}" || {{ rm -f "$at".*; exit 0; }}"#
        ),
    ]
}

/// provision-files.ts:718 `outsideBeforeScript(step, walked, root = "")`: `exit 0` when nothing is walked. Byte for
/// byte the TS's text.
pub fn outside_before_script(step: &str, walked: &[String], root: &str) -> String {
    if walked.is_empty() {
        return "exit 0".to_string();
    }
    let mut out = outside_head(root);
    out.push(format!(r#"at="$prefix/.landing-{step}""#));
    out.extend(lines(&[r#"[ ! -L "$prefix" ] && mkdir -p "$prefix" 2>/dev/null || exit 0"#, r#"rm -f "$at".*"#]));
    out.extend(outside_states());
    out.extend(outside_still());
    out.extend(lines(&[r#": > "$at.mark" || exit 0"#]));
    out.extend(outside_listing(walked, "$at.before", ""));
    out.extend(lines(&[
        r#"if [ -f "$ledger" ] && [ ! -L "$ledger" ]; then"#,
        r#"  still "$at.s" | cut -f2- | LC_ALL=C sort -u > "$at.ours.part" && mv "$at.ours.part" "$at.ours""#,
        "fi",
        "exit 0",
    ]));
    out.join("\n")
}

/// provision-files.ts:742 `outsideAfterScript(step, walked, root = "")`: `exit 0` when nothing is walked. Byte for
/// byte the TS's text.
pub fn outside_after_script(step: &str, walked: &[String], root: &str) -> String {
    if walked.is_empty() {
        return "exit 0".to_string();
    }
    let mut out = outside_head(root);
    out.push(format!(r#"at="$prefix/.landing-{step}""#));
    out.push(format!(
        r#"[ -f "$at.mark" ] && [ ! -L "$ledger" ] && [ "$(tail -n 1 "$at.before" 2>/dev/null)" = {LISTED} ] || {{ rm -f "$at".*; exit 0; }}"#
    ));
    out.extend(outside_states());
    out.extend(outside_listing(walked, "$at.after", ""));
    out.extend(outside_listing(walked, "$at.changed", r#"-cnewer "$at.mark" "#));
    out.extend(lines(&[r#"[ -f "$at.ours" ] || : > "$at.ours""#]));
    out.push(format!(
        r#"cat "$root"/var/lib/dpkg/info/*.list 2>/dev/null | wsp_root="$root" awk '{{ print ENVIRON["wsp_root"] $0 }}' | LC_ALL=C sort -u > "$at.dpkg""#
    ));
    out.push(
        r#"{ LC_ALL=C comm -13 "$at.before" "$at.after"; LC_ALL=C comm -12 "$at.changed" "$at.ours"; } | LC_ALL=C sort -u | LC_ALL=C comm -23 - "$at.dpkg" | states "$at" | while IFS= read -r line; do"#
            .to_string(),
    );
    out.push(format!(r#"  printf '%s{NL}' "$line" >> "$ledger""#));
    out.extend(lines(&["done", r#"rm -f "$at".*"#, "exit 0"]));
    out.join("\n")
}

/// provision-files.ts:767 `outsideSweepScript(root = "")`. Byte for byte the TS's text; `outside_sweep_script("{root}")`
/// plus a newline is `daemon/fixtures/contract/landed-outside.sh`.
pub fn outside_sweep_script(root: &str) -> String {
    let mut out = outside_head(root);
    out.extend(lines(&[
        r#"[ -d "$prefix" ] && [ ! -L "$prefix" ] && [ -f "$ledger" ] && [ ! -L "$ledger" ] || exit 0"#,
        r#"at="$prefix/.leaving""#,
    ]));
    out.extend(outside_states());
    out.extend(outside_still());
    out.extend(lines(&[
        r#"rm -f "$at".*"#,
        r#"still "$at.s" | LC_ALL=C sort -t"$tab" -k2 -r > "$at.take""#,
        r#": > "$at.files"; : > "$at.dirs""#,
        r#"while IFS="$tab" read -r was p; do"#,
    ]));
    out.push(r#"  if [ "$was" = dir ]; then printf '%s\000' "$p" >> "$at.dirs"; else printf '%s\000' "$p" >> "$at.files"; fi"#.to_string());
    out.extend(lines(&[
        r#"done < "$at.take""#,
        r#"xargs -0 rm -f -- < "$at.files""#,
        r#"xargs -0 rmdir -- < "$at.dirs" 2>/dev/null"#,
        r#"while IFS="$tab" read -r was p; do"#,
    ]));
    out.push(format!(r#"  [ -e "$p" ] || [ -L "$p" ] || printf '{OUTSIDE_MARK}{TAB}%s{NL}' "$p""#));
    out.extend(lines(&[r#"done < "$at.take""#, r#"rm -f "$ledger" "$at".*"#, "exit 0"]));
    out.join("\n")
}

/// provision-files.ts:790 `outsideMarks`: in the order printed; a line counts with the mark first and a path (every
/// field from the second on, joined by tabs) that starts with `/`.
pub fn outside_marks(stdout: &str) -> Vec<String> {
    stdout
        .split('\n')
        .filter_map(|line| {
            let mut words = line.split('\t');
            let first = words.next()?;
            let path = words.collect::<Vec<&str>>().join("\t");
            (first == OUTSIDE_MARK && path.starts_with('/')).then_some(path)
        })
        .collect()
}
