// SPDX-License-Identifier: AGPL-3.0-only
//! A `git.diff` (or `git.turn`, `git.range`) reply read into the files the code view and the changed-files tree draw,
//! as `apps/web/src/diffs/model.ts` reads it: each file the daemon listed, its patch parsed once, and the cut the
//! daemon made at its 2 MiB budget, past which files come with an empty patch.

use std::hash::{DefaultHasher, Hash as _, Hasher as _};
use std::ops::Deref;
use std::sync::{Arc, OnceLock};

use gpui_kit::component::diff::{DiffFile, DiffFileStatus};
use gpui_kit::SharedString;
use serde::de::IntoDeserializer as _;
use serde::Deserialize;
use serde_json::{Map, Value};

/// Pierre's `tokenizeMaxLineLength`: a line this long or longer is drawn plain, with no syntax colors and no
/// changed-word emphasis. gpui-kit's length knobs take the longest line still colored, one less, and count bytes where
/// pierre counts UTF-16 units; they agree on ASCII.
pub const PLAIN_LINE: usize = 1_000;

/// How a file changed, as the tree and the head glyph tell it apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FileStatus {
    Modified,
    Added,
    Deleted,
    Renamed,
    /// Git reported binary contents: no lines to draw.
    Binary,
}

/// One file of a reply.
#[derive(Clone)]
pub struct FilePatch {
    /// The path now, relative to the repository's top, as the daemon names it.
    pub path: SharedString,
    /// The path before a rename.
    pub old_path: Option<SharedString>,
    pub status: FileStatus,
    /// Lines added and removed, as the daemon counted them; a binary file counts none.
    pub additions: usize,
    pub deletions: usize,
    /// The daemon's patch for the file, whole; `None` for a file past the cut.
    pub patch: Option<PatchText>,
    /// The daemon listed the file with no patch: it changed, past the 2 MiB budget.
    pub cut: bool,
    /// The id git gives the file's worktree contents, which a viewed mark is kept against; absent for a file that is gone.
    pub blob: Option<String>,
    pub(super) parsed: Option<DiffFile>,
    pub(super) hunks: Vec<Hunk>,
}

/// A file's patch text. A parsed file's is written back from its parsed lines when first read, so the reply's text is
/// not kept beside them; the text of a patch that did not parse is the reply's own.
#[derive(Clone)]
pub struct PatchText(Arc<Patch>);

struct Patch {
    parsed: Option<DiffFile>,
    text: OnceLock<String>,
    digest: u64,
}

impl PatchText {
    fn new(text: String, parsed: Option<&DiffFile>) -> Self {
        let mut hasher = DefaultHasher::new();
        text.hash(&mut hasher);
        let digest = hasher.finish();
        let text = match parsed {
            Some(_) => OnceLock::new(),
            None => OnceLock::from(text),
        };
        PatchText(Arc::new(Patch { parsed: parsed.cloned(), text, digest }))
    }

    /// Tells one patch text from another without reading either.
    pub fn digest(&self) -> u64 {
        self.0.digest
    }
}

impl Deref for PatchText {
    type Target = str;

    fn deref(&self) -> &str {
        self.0.text.get_or_init(|| self.0.parsed.as_ref().map(DiffFile::to_patch).unwrap_or_default())
    }
}

/// One `@@ -a,b +c,d @@` of a patch: where it starts and how many lines it spans on each side.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Hunk {
    pub old_start: usize,
    pub old_len: usize,
    pub new_start: usize,
    pub new_len: usize,
}

/// The hunks of one file's patch, in order.
pub(super) fn hunks(patch: &str) -> Vec<Hunk> {
    let range = |part: &str| -> Option<(usize, usize)> {
        let (start, len) = part.split_once(',').unwrap_or((part, "1"));
        Some((start.parse().ok()?, len.parse().ok()?))
    };
    let mut out: Vec<Hunk> = Vec::new();
    for line in patch.lines() {
        let Some(rest) = line.strip_prefix("@@ -") else { continue };
        let mut parts = rest.split(' ');
        let (Some(old), Some(new)) = (parts.next(), parts.next().and_then(|p| p.strip_prefix('+'))) else { continue };
        if let (Some((old_start, old_len)), Some((new_start, new_len))) = (range(old), range(new)) {
            out.push(Hunk { old_start, old_len, new_start, new_len });
        }
    }
    out
}

impl Hunk {
    /// The first new-side line the hunk stands at: git names the line before an empty side (`+9,0` sits after line 9).
    fn begin(&self) -> usize {
        if self.new_len == 0 {
            self.new_start + 1
        } else {
            self.new_start
        }
    }

    /// The unmodified lines between the end of `previous` (or the file's top) and this hunk, which today's view counts
    /// in its "N unmodified lines" bar.
    pub fn hidden_before(&self, previous: Option<&Hunk>) -> usize {
        let end = previous.map_or(1, |p| p.begin() + p.new_len);
        self.begin().saturating_sub(end)
    }
}

impl FilePatch {
    /// Whether the code view has lines for it: a patch that parsed into at least one hunk or a binary notice.
    pub fn drawn(&self) -> bool {
        self.parsed.is_some()
    }

    /// The highlighter language the code view colors the file in, detected from its path as gpui-kit's diff does.
    pub fn language(&self) -> SharedString {
        match &self.parsed {
            Some(parsed) => parsed.language(),
            None => DiffFile::unchanged(self.path.clone(), "").language(),
        }
    }

    /// `editable` in `model.ts`, but for the scope: a whole text patch of a file that is still there.
    pub fn whole_text(&self) -> bool {
        self.patch.is_some() && !matches!(self.status, FileStatus::Deleted | FileStatus::Binary)
    }
}

/// A patch that did not parse, drawn raw with the reason (`getRenderablePatch` in `lib/diffRendering.ts`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RawPatch {
    pub reason: &'static str,
    pub text: SharedString,
    /// `text` split at its line ends, which the raw view draws one row each.
    pub lines: std::sync::Arc<Vec<SharedString>>,
}

/// The words `getRenderablePatch` gives a patch it could not draw as files.
pub const RAW_UNREAD: &str = "This patch could not be read. Showing it raw.";
pub const RAW_FAILED: &str = "Failed to parse patch. Showing raw patch.";

/// A whole reply read.
#[derive(Clone, Default)]
pub struct DiffModel {
    pub files: Vec<FilePatch>,
    /// The daemon cut the reply at its budget (`truncated`).
    pub cut: bool,
    /// The commit the diff is against, for a branch scope.
    pub base: Option<String>,
    /// A turn range's HEAD moves the turn did not write, oldest first.
    pub moved: Vec<String>,
    /// Set when the patch text could not be parsed into files; `files` then still lists every path.
    pub raw: Option<RawPatch>,
}

impl DiffModel {
    pub fn additions(&self) -> usize {
        self.files.iter().map(|f| f.additions).sum()
    }

    pub fn deletions(&self) -> usize {
        self.files.iter().map(|f| f.deletions).sum()
    }

    /// Every file the daemon listed came with no patch: the pane says so instead of drawing an empty view.
    pub fn all_cut(&self) -> bool {
        !self.files.is_empty() && self.files.iter().all(|f| f.cut)
    }
}

#[derive(Deserialize)]
struct ReplySpec {
    #[serde(default)]
    base: Option<String>,
    #[serde(default)]
    files: Vec<FileSpec>,
    #[serde(default)]
    truncated: bool,
    #[serde(default)]
    moved: Vec<String>,
}

#[derive(Deserialize)]
struct FileSpec {
    path: String,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    additions: Option<usize>,
    #[serde(default)]
    deletions: Option<usize>,
    #[serde(default)]
    patch: String,
    #[serde(default)]
    blob: Option<String>,
}

/// Reads a daemon reply map (`GitDiffReply`); `Err` names what was missing. It parses every patch, up to the daemon's
/// 2 MiB, so a caller runs it off the UI thread: `cx.background_spawn(async move { diff::read_reply(&reply) })`.
pub fn read_reply(reply: &Map<String, Value>) -> Result<DiffModel, String> {
    let spec = ReplySpec::deserialize(reply.into_deserializer()).map_err(|e| format!("git.diff reply: {e}"))?;
    Ok(model_of(spec))
}

/// `/^Binary files /m` or a `GIT binary patch` block, as `editable` in `model.ts` tells a binary patch.
pub fn is_binary_patch(patch: &str) -> bool {
    patch.starts_with("Binary files ") || patch.contains("\nBinary files ") || patch.contains("\nGIT binary patch")
}

/// A patch the daemon cut mid-hunk at its budget, with the last hunk's head counting only the lines that came, so the
/// lines before the cut are drawn; `None` when the patch has no hunk to close.
pub(super) fn close_cut_hunk(patch: &str) -> Option<String> {
    let at = patch.rfind("\n@@ -").map(|i| i + 1).or_else(|| patch.starts_with("@@ -").then_some(0))?;
    let (before, hunk) = patch.split_at(at);
    let (head, body) = hunk.split_once('\n').unwrap_or((hunk, ""));
    let rest = head.strip_prefix("@@ -")?;
    let (ranges, tail) = rest.split_once(" @@")?;
    let (old, new) = ranges.split_once(" +")?;
    let start = |r: &str| r.split(',').next().unwrap_or(r).to_owned();
    let (mut olds, mut news) = (0, 0);
    let mut kept = String::new();
    for line in body.split_inclusive('\n') {
        match line.as_bytes().first() {
            Some(b' ') => (olds, news) = (olds + 1, news + 1),
            Some(b'-') => olds += 1,
            Some(b'+') => news += 1,
            Some(b'\\') => {}
            _ => break,
        }
        kept.push_str(line);
    }
    if !kept.is_empty() && !kept.ends_with('\n') {
        kept.push('\n');
    }
    Some(format!("{before}@@ -{},{olds} +{},{news} @@{tail}\n{kept}", start(old), start(new)))
}

fn model_of(spec: ReplySpec) -> DiffModel {
    let truncated = spec.truncated;
    let mut raw = None;
    let files = spec
        .files
        .into_iter()
        .map(|file| {
            let mut file = file;
            let binary = is_binary_patch(&file.patch);
            let parsed = if file.patch.trim().is_empty() {
                None
            } else {
                let mut parsed = DiffFile::parse(&file.patch);
                if parsed.is_err() && truncated {
                    if let Some(whole) = close_cut_hunk(&file.patch) {
                        if let Ok(files) = DiffFile::parse(&whole) {
                            file.patch = whole;
                            parsed = Ok(files);
                        }
                    }
                }
                match parsed {
                    Ok(mut files) if !files.is_empty() => Some(files.swap_remove(0)),
                    Ok(_) => {
                        raw.get_or_insert(RAW_UNREAD);
                        None
                    }
                    // The daemon's cut can leave a file no mending reads; it is listed as past the cut.
                    Err(_) if truncated => {
                        file.patch.clear();
                        None
                    }
                    // gpui-kit refuses text holding no file where pierre reads it as no files, its other words.
                    Err(e) if e.message().as_ref() == "expected a unified or Git diff" => {
                        raw.get_or_insert(RAW_UNREAD);
                        None
                    }
                    Err(_) => {
                        raw.get_or_insert(RAW_FAILED);
                        None
                    }
                }
            };
            let status = if binary {
                FileStatus::Binary
            } else {
                match (file.kind.as_str(), parsed.as_ref().map(DiffFile::status)) {
                    ("added" | "copied", _) | (_, Some(DiffFileStatus::Added | DiffFileStatus::Copied)) => FileStatus::Added,
                    ("deleted", _) | (_, Some(DiffFileStatus::Deleted)) => FileStatus::Deleted,
                    ("renamed", _) | (_, Some(DiffFileStatus::Renamed)) => FileStatus::Renamed,
                    _ => FileStatus::Modified,
                }
            };
            let old_path = parsed
                .as_ref()
                .and_then(|p| p.original_path().cloned())
                .filter(|old| old.as_ref() != file.path.as_str() && status == FileStatus::Renamed);
            let counted = parsed.as_ref().map_or((0, 0), |p| (p.additions(), p.deletions()));
            let cut = file.patch.trim().is_empty();
            let hunks = if parsed.is_some() { hunks(&file.patch) } else { Vec::new() };
            FilePatch {
                old_path,
                status,
                additions: file.additions.unwrap_or(counted.0),
                deletions: file.deletions.unwrap_or(counted.1),
                patch: (!cut).then(|| PatchText::new(file.patch, parsed.as_ref())),
                cut,
                blob: file.blob,
                hunks,
                parsed,
                path: file.path.into(),
            }
        })
        .collect::<Vec<_>>();
    let raw = raw.map(|reason| {
        let text = files.iter().filter_map(|f| f.patch.as_deref()).collect::<Vec<_>>().join("\n");
        let text: SharedString = text.trim().to_owned().into();
        let lines = std::sync::Arc::new(text.lines().map(|l| SharedString::from(l.to_owned())).collect());
        RawPatch { reason, text, lines }
    });
    DiffModel { files, cut: spec.truncated, base: spec.base, moved: spec.moved, raw }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screens::pr_pane::comments::patch_rows;
    use serde_json::json;

    fn reply(value: Value) -> DiffModel {
        read_reply(value.as_object().unwrap()).unwrap()
    }

    const EDIT: &str = "diff --git a/src/a.rs b/src/a.rs\nindex 1111111..2222222 100644\n--- a/src/a.rs\n+++ b/src/a.rs\n@@ -1,3 +1,3 @@\n fn a() {\n-    1\n+    2\n }\n";
    const ADD: &str = "diff --git a/new.ts b/new.ts\nnew file mode 100644\nindex 0000000..3333333\n--- /dev/null\n+++ b/new.ts\n@@ -0,0 +1,2 @@\n+export const x = 1;\n+export const y = 2;\n";
    const GONE: &str = "diff --git a/old.md b/old.md\ndeleted file mode 100644\nindex 4444444..0000000\n--- a/old.md\n+++ /dev/null\n@@ -1 +0,0 @@\n-gone\n";
    const MOVE: &str = "diff --git a/from.txt b/to.txt\nsimilarity index 90%\nrename from from.txt\nrename to to.txt\nindex 5555555..6666666 100644\n--- a/from.txt\n+++ b/to.txt\n@@ -1,2 +1,2 @@\n same\n-old\n+new\n";
    const BIN: &str = "diff --git a/logo.png b/logo.png\nindex 7777777..8888888 100644\nBinary files a/logo.png and b/logo.png differ\n";

    #[test]
    fn each_kind_reads_as_the_tree_names_it() {
        let model = reply(json!({
            "base": null, "truncated": false, "moved": [],
            "files": [
                {"path": "src/a.rs", "kind": "modified", "additions": 1, "deletions": 1, "patch": EDIT, "blob": "2222222"},
                {"path": "new.ts", "kind": "added", "additions": 2, "deletions": 0, "patch": ADD, "blob": "3333333"},
                {"path": "old.md", "kind": "deleted", "additions": 0, "deletions": 1, "patch": GONE},
                {"path": "to.txt", "kind": "renamed", "additions": 1, "deletions": 1, "patch": MOVE},
                {"path": "logo.png", "kind": "modified", "additions": 0, "deletions": 0, "patch": BIN},
            ]
        }));
        let kinds: Vec<_> = model.files.iter().map(|f| (f.path.to_string(), f.status)).collect();
        assert_eq!(
            kinds,
            [
                ("src/a.rs".into(), FileStatus::Modified),
                ("new.ts".into(), FileStatus::Added),
                ("old.md".into(), FileStatus::Deleted),
                ("to.txt".into(), FileStatus::Renamed),
                ("logo.png".into(), FileStatus::Binary),
            ]
        );
        assert_eq!(model.files[3].old_path.as_deref(), Some("from.txt"));
        assert_eq!(model.files[0].old_path, None);
        assert!(model.files.iter().all(|f| f.drawn() && !f.cut));
        assert_eq!(model.files[0].blob.as_deref(), Some("2222222"));
        assert_eq!((model.additions(), model.deletions()), (4, 3));
        assert!(model.raw.is_none() && !model.cut);
        assert!(model.files[0].whole_text() && !model.files[2].whole_text() && !model.files[4].whole_text());
    }

    #[test]
    fn files_past_the_cut_are_listed_with_no_patch() {
        let model = reply(json!({
            "base": "abc", "truncated": true, "moved": [],
            "files": [
                {"path": "src/a.rs", "kind": "modified", "additions": 1, "deletions": 1, "patch": EDIT},
                {"path": "big.json", "kind": "modified", "additions": 90000, "deletions": 3, "patch": ""},
            ]
        }));
        assert!(model.cut);
        assert_eq!(model.base.as_deref(), Some("abc"));
        let big = &model.files[1];
        assert!(big.cut && big.patch.is_none() && !big.drawn());
        assert_eq!((big.additions, big.deletions), (90000, 3));
        assert!(!model.all_cut());
        assert!(model.raw.is_none());
    }

    #[test]
    fn a_patch_cut_mid_hunk_draws_the_lines_that_came() {
        let cut = "diff --git a/big.txt b/big.txt\n--- a/big.txt\n+++ b/big.txt\n@@ -1,4 +1,400000 @@\n one\n-two\n+2\n+3\n+4";
        let model = reply(json!({"truncated": true, "files": [
            {"path": "big.txt", "kind": "modified", "additions": 399999, "deletions": 1, "patch": cut},
            {"path": "small.txt", "kind": "added", "patch": ""},
        ]}));
        assert!(model.raw.is_none(), "a cut reply keeps its files");
        let big = &model.files[0];
        assert!(big.drawn() && !big.cut);
        assert_eq!(big.hunks, [Hunk { old_start: 1, old_len: 2, new_start: 1, new_len: 4 }]);
        assert_eq!((big.additions, big.deletions), (399999, 1), "the counts stay the daemon's");
        assert!(model.files[1].cut);
        let uncut = reply(json!({"truncated": false, "files": [{"path": "big.txt", "patch": cut}]}));
        assert!(uncut.raw.is_some(), "a reply the daemon did not cut is not mended");
    }

    #[test]
    fn a_reply_whose_every_file_was_cut_is_all_cut() {
        let model = reply(json!({"base": null, "truncated": true, "moved": [], "files": [{"path": "a", "kind": "added", "patch": ""}]}));
        assert!(model.all_cut());
        assert_eq!(model.files[0].status, FileStatus::Added);
        assert!(!reply(json!({"files": []})).all_cut());
    }

    #[test]
    fn counts_fall_back_to_the_patch_when_the_daemon_sends_none() {
        let model = reply(json!({"files": [{"path": "src/a.rs", "patch": EDIT}]}));
        assert_eq!((model.files[0].additions, model.files[0].deletions), (1, 1));
        assert_eq!(model.files[0].status, FileStatus::Modified);
    }

    #[test]
    fn a_patch_that_does_not_parse_is_kept_raw_with_the_reason() {
        let broken = "diff --git a/x b/x\n--- a/x\n+++ b/x\n@@ -1,2 +1,2 @@ \n-a\n+b\n@@ nonsense @@\n";
        let model = reply(json!({"files": [{"path": "x", "kind": "modified", "patch": broken}, {"path": "src/a.rs", "patch": EDIT}]}));
        let raw = model.raw.expect("raw");
        assert_eq!(raw.reason, "Failed to parse patch. Showing raw patch.");
        assert!(raw.text.contains("@@ nonsense @@") && raw.text.contains("fn a()"));
        assert_eq!(model.files.len(), 2);
        let unread = reply(json!({"files": [{"path": "x", "kind": "modified", "patch": "not a patch at all\n"}]}));
        assert_eq!(unread.raw.expect("raw").reason, "This patch could not be read. Showing it raw.");
    }

    #[test]
    fn hunks_count_the_unmodified_lines_between_them() {
        let model = reply(json!({"files": [{"path": "to.txt", "patch": MOVE}, {"path": "new.ts", "patch": ADD}]}));
        assert_eq!(model.files[1].hunks, [Hunk { old_start: 0, old_len: 0, new_start: 1, new_len: 2 }]);
        assert_eq!(model.files[1].hunks[0].hidden_before(None), 0);
        let two = "@@ -7,11 +7,12 @@ export interface Row {\n a\n-b\n+c\n@@ -30,7 +31,8 @@\n x\n-y\n";
        let pair = hunks(two);
        assert_eq!(pair.len(), 2);
        assert_eq!(pair[0].hidden_before(None), 6);
        assert_eq!(pair[1].hidden_before(Some(&pair[0])), 12);
        assert_eq!(hunks("@@ -1 +1 @@\n-a\n+b\n x\n")[0], Hunk { old_start: 1, old_len: 1, new_start: 1, new_len: 1 });
    }

    #[test]
    fn with_no_context_git_names_the_line_before_an_empty_side() {
        // git diff -U0: line 1 changed, lines 10 and 11 removed (`+9,0` sits after new line 9), a line added after 20.
        let zero = hunks("@@ -1 +1 @@\n-a\n+b\n@@ -10,2 +9,0 @@\n-c\n-d\n@@ -21,0 +20 @@\n+e\n");
        assert_eq!(zero[1].hidden_before(Some(&zero[0])), 8, "new lines 2 to 9");
        assert_eq!(zero[2].hidden_before(Some(&zero[1])), 10, "new lines 10 to 19");
        assert_eq!(hunks("@@ -1,2 +0,0 @@\n-a\n-b\n")[0].hidden_before(None), 0);
    }

    #[test]
    fn a_parsed_patch_reads_back_as_the_rows_it_came_with() {
        let cut = "diff --git a/big.txt b/big.txt\n--- a/big.txt\n+++ b/big.txt\n@@ -1,4 +1,400000 @@\n one\n-two\n+2\n+3\n+4";
        let model = reply(json!({"truncated": true, "files": [
            {"path": "src/a.rs", "patch": EDIT}, {"path": "new.ts", "patch": ADD}, {"path": "old.md", "patch": GONE},
            {"path": "to.txt", "patch": MOVE}, {"path": "logo.png", "patch": BIN}, {"path": "big.txt", "patch": cut},
        ]}));
        let mended = close_cut_hunk(cut).unwrap();
        for (file, sent) in model.files.iter().zip([EDIT, ADD, GONE, MOVE, BIN, &mended]) {
            let text = file.patch.as_ref().unwrap();
            assert_eq!(patch_rows(text), patch_rows(sent), "{}", file.path);
            assert_eq!(text.lines().count(), sent.lines().count(), "{}", file.path);
        }
        let again = reply(json!({"files": [{"path": "src/a.rs", "patch": EDIT}, {"path": "to.txt", "patch": MOVE}]}));
        let digest = |m: &DiffModel, ix: usize| m.files[ix].patch.as_ref().unwrap().digest();
        assert_eq!(digest(&again, 0), digest(&model, 0));
        assert_ne!(digest(&again, 0), digest(&again, 1));
    }

    #[test]
    fn a_reply_missing_its_files_is_an_error_not_a_panic() {
        assert!(read_reply(json!({"files": "no"}).as_object().unwrap()).is_err());
    }
}
