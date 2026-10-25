// SPDX-License-Identifier: AGPL-3.0-only
//! The release road's scripts: a catalog row's recorded asset by arch, or a person's own row read off the release's
//! listing, with the go fall-through. Port of packages/catalog/src/road-modules.ts:436-563 at a3b870a6e; the spec is
//! spec/road_modules.md.
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::golden_image::ToolPin;
use crate::road_modules::go_binary;
use crate::roads::{pin_check_line, ReleaseAssets};

/// road-modules.ts:472.
pub const ASSET_ARCH_X86_64: &str = "amd64|x86_64|x64";
pub const ASSET_ARCH_AARCH64: &str = "arm64|aarch64";
/// road-modules.ts:475.
pub const ASSET_SKIPPED: &str = "\\.(sha256|sha256sum|sha512|sig|asc|txt|md5|pem|deb|rpm|apk|zst|tar\\.zst|json)$";

/// What a downloaded asset becomes on the machine (road-modules.ts:436).
fn unpack_lines(read: &str) -> Vec<String> {
    [
        r#"case "$asset" in"#,
        r#"  *.tar.gz|*.tgz) tar -xzf "$tmp/$asset" -C "$tmp" ;;"#,
        r#"  *.tar.xz) tar -xJf "$tmp/$asset" -C "$tmp" ;;"#,
        r#"  *.zip) if command -v unzip >/dev/null 2>&1; then unzip -qo "$tmp/$asset" -d "$tmp"; else python3 -m zipfile -e "$tmp/$asset" "$tmp"; fi ;;"#,
        r#"  *) mv "$tmp/$asset" "$tmp/$name"; chmod +x "$tmp/$name" ;;"#,
        "esac",
        r#"bin="$(find "$tmp" -type f -name "$name" | head -1)""#,
        r#"[ -n "$bin" ] || bin="$(find "$tmp" -type f -perm -u+x ! -name "$asset" ! -name '*.md' ! -name '*.txt' -printf '%s %p\n' | sort -rn | head -1 | cut -d' ' -f2-)""#,
        r#"[ -n "$bin" ] || { echo "Error: no binary in $asset" >&2; exit 1; }"#,
        r#"install -m 0755 "$bin" "/usr/local/bin/$name""#,
        &*format!(r#"echo "WSP_ROAD release $asset {read}""#),
    ]
    .iter()
    .map(|l| l.to_string())
    .collect()
}

/// The fall-through for an arch the release has no Linux asset for (road-modules.ts:457).
fn go_lines(go_at: &str, name: &str) -> Vec<String> {
    let mut out =
        vec!["elif command -v go >/dev/null 2>&1; then".to_string(), format!("  GOBIN=/usr/local/bin go install {}", shell_quote(go_at))];
    if go_binary(go_at) != name {
        out.push(format!("  mv {} \"/usr/local/bin/$name\"", shell_quote(&format!("/usr/local/bin/{}", go_binary(go_at)))));
    }
    out.push(format!("  echo \"WSP_ROAD go \"{}", shell_quote(go_at)));
    out
}

/// The module at a version, for the fall-through: its own where it names one, else the tag the road installs at.
fn go_at_tag(go: Option<&str>, tag: &str) -> Option<String> {
    match go {
        None => None,
        Some(g) if g.contains('@') => Some(g.to_string()),
        Some(g) => Some(format!("{g}@{tag}")),
    }
}

/// The script for a row the catalog recorded assets for (road-modules.ts:481).
pub fn asset_install(name: &str, repo: &str, tag: &str, assets: &ReleaseAssets, go: Option<&str>) -> String {
    let go_at = go_at_tag(go, tag);
    let pick = |a: &Option<crate::roads::ReleaseAsset>| match a {
        None => "asset= sha=".to_string(),
        Some(a) => format!("asset={} sha={}", shell_quote(&a.name), a.sha256),
    };
    let mut body: Vec<String> = vec![
        format!("url={}\"$asset\"", shell_quote(&format!("https://github.com/{repo}/releases/download/{tag}/"))),
        r#"curl -o "$tmp/$asset" "$url""#.to_string(),
        r#"echo "$sha  $tmp/$asset" | sha256sum -c - >/dev/null"#.to_string(),
    ];
    body.extend(unpack_lines("$sha $tag"));
    let complete = assets.x86_64.is_some() && assets.aarch64.is_some();
    let mut lines: Vec<String> = vec![
        "set -euo pipefail".into(),
        format!("name={}", shell_quote(name)),
        format!("tag={}", shell_quote(tag)),
        r#"arch="$(uname -m)""#.into(),
        format!(
            r#"case "$arch" in x86_64) {} ;; aarch64) {} ;; *) echo "Error: unsupported arch: $arch" >&2; exit 1 ;; esac"#,
            pick(&assets.x86_64),
            pick(&assets.aarch64)
        ),
        r#"tmp="$(mktemp -d /tmp/wsp-road-XXXXXX)""#.into(),
        r#"trap 'rm -rf "$tmp"' EXIT"#.into(),
    ];
    if complete {
        lines.extend(body);
    } else {
        lines.push(r#"if [ -n "$asset" ]; then"#.into());
        lines.extend(body.iter().map(|l| format!("  {l}")));
        if let Some(go_at) = &go_at {
            lines.extend(go_lines(go_at, name));
        }
        lines.push("else".into());
        let no_go = if go_at.is_some() { ", and go is not on the machine" } else { "" };
        lines.push(format!(
            r#"  echo "Error: release "{}" of "{}" has no Linux build for $arch{no_go}" >&2"#,
            shell_quote(tag),
            shell_quote(repo)
        ));
        lines.push("  exit 1".into());
        lines.push("fi".into());
    }
    lines.join("\n")
}

/// The script for a person's own row with no assets (road-modules.ts:518).
pub fn release_install(name: &str, repo: &str, tag: &str, pin: Option<&ToolPin>, go: Option<&str>) -> String {
    let go_at = go_at_tag(go, tag);
    let mut lines: Vec<String> = vec![
        "set -euo pipefail".into(),
        format!("name={}", shell_quote(name)),
        format!("tag={}", shell_quote(tag)),
        r#"arch="$(uname -m)""#.into(),
        format!(
            r#"case "$arch" in x86_64) pat="{ASSET_ARCH_X86_64}" ;; aarch64) pat="{ASSET_ARCH_AARCH64}" ;; *) echo "Error: unsupported arch: $arch" >&2; exit 1 ;; esac"#
        ),
        r#"tmp="$(mktemp -d /tmp/wsp-road-XXXXXX)""#.into(),
        r#"trap 'rm -rf "$tmp"' EXIT"#.into(),
        format!(r#"release="$(curl {} || true)""#, shell_quote(&format!("https://api.github.com/repos/{repo}/releases/tags/{tag}"))),
        r#"urls="$(printf '%s\n' "$release" | grep -o '"browser_download_url": *"[^"]*"' | cut -d'"' -f4 || true)""#.into(),
        format!(r#"url="$(printf '%s\n' "$urls" | grep -i linux | grep -iE "$pat" | grep -viE '{ASSET_SKIPPED}' | head -1 || true)""#),
        r#"if [ -n "$url" ]; then"#.into(),
        r#"  asset="${url##*/}""#.into(),
        r#"  curl -o "$tmp/$asset" "$url""#.into(),
        r#"  sum="$(sha256sum "$tmp/$asset" | cut -d' ' -f1)""#.into(),
    ];
    match pin.and_then(|p| p.sha256.as_ref().map(|sum| (p, sum))) {
        None => {}
        Some((p, sum)) => match &p.arch {
            None => lines.push(format!("  {}", pin_check_line("$asset", "$tag", sum))),
            Some(arch) => lines.push(format!(
                r#"  if [ "$arch" = {} ]; then {}; fi"#,
                shell_quote(arch_word(*arch)),
                pin_check_line("$asset", "$tag", sum)
            )),
        },
    }
    lines.extend(unpack_lines("$sum $tag $arch").iter().map(|l| format!("  {l}")));
    if let Some(go_at) = &go_at {
        lines.extend(go_lines(go_at, name));
    }
    lines.push("else".into());
    let no_go = if go_at.is_some() { ", and go is not on the machine" } else { "" };
    lines.push(format!(r#"  echo "Error: release "{}" of "{}" has no Linux build{no_go}" >&2"#, shell_quote(tag), shell_quote(repo)));
    lines.push("  exit 1".into());
    lines.push("fi".into());
    lines.join("\n")
}

fn arch_word(arch: wsp_wire::views::golden_image::ToolPinArch) -> &'static str {
    match arch {
        wsp_wire::views::golden_image::ToolPinArch::X8664 => "x86_64",
        wsp_wire::views::golden_image::ToolPinArch::Aarch64 => "aarch64",
    }
}
