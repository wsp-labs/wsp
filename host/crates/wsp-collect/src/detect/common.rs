// SPDX-License-Identifier: AGPL-3.0-only
//! The pieces every detector builds rows with. Port of packages/collect/src/detect/common.ts at a3b870a6e; the spec
//! is spec/common.md.
use futures::future::LocalBoxFuture;
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;

use crate::host::{expand, Host};
use crate::manifest::{Linux, ManifestEntry, RowDefault, Rung};

/// `prior` is every row the rungs before this one produced.
/// A detector that rejects in the TS (a hooks file `hook_files` refuses, an apt list read failing part way) is an
/// `Err` here, and `collect` fails with it.
pub type Detector = for<'a> fn(&'a Host, &'a [ManifestEntry]) -> LocalBoxFuture<'a, Result<Vec<ManifestEntry>, HostError>>;

/// The optional half of a row spec: each `None` is a key the row leaves off (common.ts:8-27).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RowOptions {
    pub default: Option<RowDefault>,
    pub reason: Option<String>,
    pub group: Option<String>,
    pub required: Option<bool>,
    pub linux: Option<Linux>,
    pub version: Option<String>,
    pub arch: Option<String>,
    /// Candidates the tool rewrites while it runs; those found land on the row as volatile.
    pub volatile: Option<Vec<String>>,
    /// One line for the detail pane.
    pub detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowSpec {
    pub rung: Rung,
    pub id: String,
    pub label: String,
    /// `~/`-relative candidates; only the ones that exist end up on the row.
    pub paths: Vec<String>,
    /// Emit the row even when none of the candidate paths exist (TS `always?: boolean`, absent is false).
    pub always: bool,
    pub opt: RowOptions,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EntrySpec {
    pub rung: Rung,
    pub id: String,
    pub label: String,
    pub paths: Vec<String>,
    pub bytes: u64,
    pub opt: RowOptions,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    pub paths: Vec<String>,
    pub bytes: u64,
}

/// The subset of candidates that exist, in the order given, with their total size.
pub async fn found(host: &Host, paths: &[String]) -> Found {
    let mut out = Found::default();
    for p in paths {
        let Some(s) = host.fs.stat(&expand(&host.home, p)).await else {
            continue;
        };
        out.paths.push(p.clone());
        out.bytes += s.bytes;
    }
    out
}

pub async fn exists(host: &Host, path: &str) -> bool {
    host.fs.stat(&expand(&host.home, path)).await.is_some()
}

/// One manifest row for whichever of the spec's paths exist; `None` when none do and `always` is false.
pub async fn row(host: &Host, spec: RowSpec) -> Option<ManifestEntry> {
    let f = found(host, &spec.paths).await;
    if f.paths.is_empty() && !spec.always {
        return None;
    }
    Some(entry(EntrySpec { rung: spec.rung, id: spec.id, label: spec.label, paths: f.paths, bytes: f.bytes, opt: spec.opt }))
}

/// Optional fields are left off rather than set, so equality and snapshots stay exact.
pub fn entry(spec: EntrySpec) -> ManifestEntry {
    let RowOptions { default, reason, group, required, linux, version, arch, volatile, detail } = spec.opt;
    let volatile: Vec<String> = volatile.unwrap_or_default().into_iter().filter(|p| spec.paths.contains(p)).collect();
    ManifestEntry {
        rung: spec.rung,
        id: spec.id,
        label: spec.label,
        paths: spec.paths,
        excludes: None,
        volatile: (!volatile.is_empty()).then_some(volatile),
        bytes: spec.bytes,
        default: default.unwrap_or(RowDefault::Bring),
        reason,
        group,
        required,
        bring: None,
        choice: None,
        linux,
        version,
        arch,
        pin: None,
        consent: None,
        detail,
        secrets: None,
        login: None,
        font: None,
        sources: None,
        sign_in: None,
        keys: None,
    }
}

/// A list item (a formula, an extension, a global package): no source path, nothing to upload.
pub fn item(rung: Rung, id: &str, label: &str, opt: RowOptions) -> ManifestEntry {
    entry(EntrySpec { rung, id: id.to_string(), label: label.to_string(), paths: Vec::new(), bytes: 0, opt })
}

pub fn present(rows: Vec<Option<ManifestEntry>>) -> Vec<ManifestEntry> {
    rows.into_iter().flatten().collect()
}

/// The first line of a command's output, trimmed as JavaScript trims; `None` for no output or an empty first line.
pub fn first_line(out: Option<&str>) -> Option<String> {
    let line = js_trim(out?.split('\n').next()?);
    (!line.is_empty()).then(|| line.to_string())
}
