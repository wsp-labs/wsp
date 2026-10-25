// SPDX-License-Identifier: AGPL-3.0-only
//! The one version order: which half of a launch is behind, and the tag below a tag. Port of
//! packages/protocol/src/semver.mjs at the cut (383708c78). See spec/semver.md.

use std::cmp::Ordering;

use crate::js::utf16;

// A version split the way precedence reads it: the dotted core, then the prerelease identifiers, or none. Build
// metadata after a + carries no precedence at all, so it is dropped here.
// semver.mjs:11
// private in the TS: port it as a private helper
// function parts(version) {
//   const [rest] = version.split("+");
//   const cut = rest.indexOf("-");
//   return {
//     core: (cut === -1 ? rest : rest.slice(0, cut)).split("."),
//     pre: cut === -1 ? undefined : rest.slice(cut + 1).split("."),
//   };
// }
struct Parts<'a> {
    core: Vec<&'a str>,
    pre: Option<Vec<&'a str>>,
}

fn parts(version: &str) -> Parts<'_> {
    let rest = version.split('+').next().unwrap_or_default();
    match rest.split_once('-') {
        Some((core, pre)) => Parts { core: core.split('.').collect(), pre: Some(pre.split('.').collect()) },
        None => Parts { core: rest.split('.').collect(), pre: None },
    }
}

// One identifier against another: two numbers as numbers, so rc.10 is above rc.2; anything else as text, so rc1
// is below rc2 and alpha below beta; and a number below a word.
// semver.mjs:22
// private in the TS: port it as a private helper; NUMERIC is /^\d+$/, Number(a) an f64, `a < b` UTF-16 order
// function compareIdentifiers(a, b) {
//   if (NUMERIC.test(a) && NUMERIC.test(b)) return Number(a) - Number(b);
//   if (NUMERIC.test(a)) return -1;
//   if (NUMERIC.test(b)) return 1;
//   return a < b ? -1 : a > b ? 1 : 0;
// }
fn compare_identifiers(a: &str, b: &str) -> Ordering {
    let numeric = |s: &str| !s.is_empty() && s.bytes().all(|c| c.is_ascii_digit());
    match (numeric(a), numeric(b)) {
        (true, true) => {
            let (x, y): (f64, f64) = (a.parse().unwrap_or(f64::NAN), b.parse().unwrap_or(f64::NAN));
            x.partial_cmp(&y).unwrap_or(Ordering::Equal)
        }
        (true, false) => Ordering::Less,
        (false, true) => Ordering::Greater,
        (false, false) => utf16::cmp(a, b),
    }
}

/// Semver order: 0.1.10 is above 0.1.9, and a release is above its own prereleases. Less when the first is the
/// older, Greater when the second is, Equal when the two name one release. A core part the shorter of two does not
/// carry reads as 0, so 0.2 and 0.2.0 are one release.
// semver.mjs:32
// export function compareVersions(a, b) {
//   const [left, right] = [parts(a), parts(b)];
//   for (let i = 0; i < Math.max(left.core.length, right.core.length); i++) {
//     const order = compareIdentifiers(left.core[i] ?? "0", right.core[i] ?? "0");
//     if (order !== 0) return order;
//   }
//   if (left.pre === undefined || right.pre === undefined) return left.pre === right.pre ? 0 : left.pre === undefined ? 1 : -1;
//   for (let i = 0; i < Math.max(left.pre.length, right.pre.length); i++) {
//     // A set of identifiers that runs out first is below one that carries more, every field before it being equal.
//     if (left.pre[i] === undefined) return -1;
//     if (right.pre[i] === undefined) return 1;
//     const order = compareIdentifiers(left.pre[i], right.pre[i]);
//     if (order !== 0) return order;
//   }
//   return 0;
// }
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let (left, right) = (parts(a), parts(b));
    for i in 0..left.core.len().max(right.core.len()) {
        let order = compare_identifiers(left.core.get(i).copied().unwrap_or("0"), right.core.get(i).copied().unwrap_or("0"));
        if order != Ordering::Equal {
            return order;
        }
    }
    let (left_pre, right_pre) = match (left.pre, right.pre) {
        (Some(l), Some(r)) => (l, r),
        (None, None) => return Ordering::Equal,
        (None, Some(_)) => return Ordering::Greater,
        (Some(_), None) => return Ordering::Less,
    };
    for i in 0..left_pre.len().max(right_pre.len()) {
        match (left_pre.get(i), right_pre.get(i)) {
            (None, _) => return Ordering::Less,
            (_, None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                let order = compare_identifiers(x, y);
                if order != Ordering::Equal {
                    return order;
                }
            }
        }
    }
    Ordering::Equal
}
