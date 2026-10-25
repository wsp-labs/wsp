// SPDX-License-Identifier: AGPL-3.0-only
//! compareVersions has no TS test of its own; every case here is what packages/protocol/src/semver.mjs returned under
//! node at a3b870a6e, as `Math.sign(...)`.

use std::cmp::Ordering;
use wsp_words::semver::compare_versions;

fn sign(o: Ordering) -> f64 {
    match o {
        Ordering::Less => -1.0,
        Ordering::Equal => 0.0,
        Ordering::Greater => 1.0,
    }
}

#[test]
fn orders_cores_numerically_and_a_release_above_its_prereleases() {
    assert_eq!(sign(compare_versions("0.1.10", "0.1.9")), 1.0); // Math.sign(compareVersions("0.1.10", "0.1.9"))
    assert_eq!(sign(compare_versions("0.2", "0.2.0")), 0.0); // Math.sign(compareVersions("0.2", "0.2.0"))
    assert_eq!(sign(compare_versions("1.0.0", "1.0.0-rc.1")), 1.0); // Math.sign(compareVersions("1.0.0", "1.0.0-rc.1"))
    assert_eq!(sign(compare_versions("1.0.0-rc.10", "1.0.0-rc.2")), 1.0); // Math.sign(compareVersions("1.0.0-rc.10", "1.0.0-rc.2"))
    assert_eq!(sign(compare_versions("1.0.0-rc1", "1.0.0-rc2")), -1.0); // Math.sign(compareVersions("1.0.0-rc1", "1.0.0-rc2"))
    assert_eq!(sign(compare_versions("1.0.0-alpha", "1.0.0-beta")), -1.0); // Math.sign(compareVersions("1.0.0-alpha", "1.0.0-beta"))
    assert_eq!(sign(compare_versions("1.0.0-1", "1.0.0-alpha")), -1.0); // Math.sign(compareVersions("1.0.0-1", "1.0.0-alpha"))
    assert_eq!(sign(compare_versions("1.0.0-alpha", "1.0.0-alpha.1")), -1.0); // Math.sign(compareVersions("1.0.0-alpha", "1.0.0-alpha.1"))
    assert_eq!(sign(compare_versions("1.0.0+build.5", "1.0.0")), 0.0); // Math.sign(compareVersions("1.0.0+build.5", "1.0.0"))
    assert_eq!(sign(compare_versions("1.0.0-rc.1+b", "1.0.0-rc.1")), 0.0); // Math.sign(compareVersions("1.0.0-rc.1+b", "1.0.0-rc.1"))
}

#[test]
fn reads_malformed_versions_as_the_ts_does() {
    assert_eq!(sign(compare_versions("", "0")), 1.0); // Math.sign(compareVersions("", "0"))
    assert_eq!(sign(compare_versions("", "")), 0.0); // Math.sign(compareVersions("", ""))
    assert_eq!(sign(compare_versions("1..2", "1.0.2")), 1.0); // Math.sign(compareVersions("1..2", "1.0.2"))
    assert_eq!(sign(compare_versions("v1.0.0", "1.0.0")), 1.0); // Math.sign(compareVersions("v1.0.0", "1.0.0"))
    assert_eq!(sign(compare_versions("1.0.0-", "1.0.0")), -1.0); // Math.sign(compareVersions("1.0.0-", "1.0.0"))
    assert_eq!(sign(compare_versions("1.0.0-B", "1.0.0-a")), -1.0); // Math.sign(compareVersions("1.0.0-B", "1.0.0-a"))
    assert_eq!(sign(compare_versions("10.0.0", "9.99.99")), 1.0); // Math.sign(compareVersions("10.0.0", "9.99.99"))
    assert_eq!(sign(compare_versions("1.0.0-01", "1.0.0-1")), 0.0); // Math.sign(compareVersions("1.0.0-01", "1.0.0-1"))
    assert_eq!(sign(compare_versions("0.3.8", "0.3.8")), 0.0); // Math.sign(compareVersions("0.3.8", "0.3.8"))
    assert_eq!(sign(compare_versions("1.0.0-x.\u{1f600}", "1.0.0-x.\u{ffff}")), -1.0); // Math.sign(compareVersions("1.0.0-x.😀", "1.0.0-x.￿"))
    assert_eq!(sign(compare_versions("99999999999999999999.0.0", "99999999999999999998.0.0")), 0.0);
    // Math.sign(compareVersions("99999999999999999999.0.0", "99999999999999999998.0.0"))
}
