// SPDX-License-Identifier: AGPL-3.0-only
//! The replies and words of wire/daemon.ts that wsp-frames also holds, read as zod reads them: zod's numbers, `null`
//! refused on an optional key, zod's issues in zod's order. See spec/daemon.md rule 9.

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::daemon::ProcEntry;
use crate::types::check::{
    issues_of, items_at_most, leaf, not_object, num_above, num_at_least, num_at_most, regex_check, required, text_at_least, text_at_most,
    Check, SchemaError, Zod,
};
use crate::types::daemon_contract::SSH_KEY_MAX;
use crate::types::js_number::{JsInt, JsNumber};
use crate::types::keys::{self, Obj};

crate::literal!(PipeOpenOp, "pipe.open");

impl PipeOpen {
    pub fn new(door: PipeDoor, held_ms: u64) -> Self {
        PipeOpen { op: PipeOpenOp, door, held_ms: JsInt::from(held_ms as i64) }
    }
}

/// `/^ssh-ed25519 [A-Za-z0-9+\/]+={0,2}( [!-~]+)?$/`: one key line and nothing after it, since known_hosts trusts a
/// second line or a marker for every wsp- alias.
pub fn is_ssh_host_key_line(s: &str) -> bool {
    let Some(rest) = s.strip_prefix("ssh-ed25519 ") else { return false };
    let body = rest.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '+' || c == '/');
    if body.len() == rest.len() {
        return false;
    }
    let tail = body.strip_prefix("==").or_else(|| body.strip_prefix('=')).unwrap_or(body);
    match tail.strip_prefix(' ') {
        None => tail.is_empty(),
        Some(comment) => !comment.is_empty() && comment.bytes().all(|b| (b'!'..=b'~').contains(&b)),
    }
}

/// A required array of strings zod reads: its length rule first, then each item's type and rule, in zod's order.
fn text_list_key(obj: &Obj, key: &str, out: &mut Vec<SchemaError>, max: usize, each: fn(&str) -> Result<(), SchemaError>) {
    let Some(value) = obj.get(key) else {
        out.push(required::<Vec<String>>(key));
        return;
    };
    let Value::Array(items) = value else {
        out.extend(issues_of::<Vec<String>>(value).into_iter().map(|e| e.under(key)));
        return;
    };
    if let Err(e) = items_at_most(items.len(), max) {
        out.push(e.under(key));
    }
    for (i, item) in items.iter().enumerate() {
        let issues = match item {
            Value::String(text) => each(text).err().into_iter().collect(),
            other => issues_of::<String>(other),
        };
        out.extend(issues.into_iter().map(|e| e.under(i).under(key)));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FsEntryType {
    #[serde(rename = "file")]
    File,
    #[serde(rename = "dir")]
    Dir,
    #[serde(rename = "symlink")]
    Symlink,
}
impl Check for FsEntryType {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["file", "dir", "symlink"]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostItemKind {
    #[serde(rename = "pull-request")]
    PullRequest,
    #[serde(rename = "issue")]
    Issue,
}
impl Check for HostItemKind {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["pull-request", "issue"]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KillMode {
    #[serde(rename = "pid")]
    Pid,
    #[serde(rename = "tree")]
    Tree,
}
impl Check for KillMode {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["pid", "tree"]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PipeDoor {
    #[serde(rename = "http")]
    Http,
    #[serde(rename = "gh")]
    Gh,
}
impl Check for PipeDoor {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["http", "gh"]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcKind {
    #[serde(rename = "agent")]
    Agent,
    #[serde(rename = "tool")]
    Tool,
    #[serde(rename = "dev")]
    Dev,
    #[serde(rename = "test")]
    Test,
    #[serde(rename = "build")]
    Build,
    #[serde(rename = "slate")]
    Slate,
    #[serde(rename = "terminal")]
    Terminal,
    #[serde(rename = "tab")]
    Tab,
    #[serde(rename = "app")]
    App,
    #[serde(rename = "wsp")]
    Wsp,
    #[serde(rename = "other")]
    Other,
}
impl Check for ProcKind {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["agent", "tool", "dev", "test", "build", "slate", "terminal", "tab", "app", "wsp", "other"]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcOwnedBy {
    #[serde(rename = "root")]
    Root,
    #[serde(rename = "fork")]
    Fork,
    #[serde(rename = "group")]
    Group,
    #[serde(rename = "env")]
    Env,
    #[serde(rename = "cgroup")]
    Cgroup,
    #[serde(rename = "folder")]
    Folder,
    #[serde(rename = "coalition")]
    Coalition,
}
impl Check for ProcOwnedBy {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["root", "fork", "group", "env", "cgroup", "folder", "coalition"]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ProcSignal {
    #[serde(rename = "TERM")]
    Term,
    #[serde(rename = "KILL")]
    Kill,
}
impl Check for ProcSignal {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["TERM", "KILL"]))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UsageLogFormat {
    #[serde(rename = "claude-jsonl")]
    ClaudeJsonl,
    #[serde(rename = "codex-rollout")]
    CodexRollout,
    #[serde(rename = "opencode-sqlite")]
    OpencodeSqlite,
}
impl Check for UsageLogFormat {
    fn describe(path: &[Value]) -> Option<Zod> {
        leaf(path, Zod::Options(&["claude-jsonl", "codex-rollout", "opencode-sqlite"]))
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CgroupMemory {
    pub cgroup: String,
    pub memory: JsInt,
}
impl Check for CgroupMemory {
    fn check(&self) -> Result<(), SchemaError> {
        Self::rule_memory(&self.memory).map_err(|e| e.under("memory"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "cgroup", &mut out);
        keys::req_key_with::<JsInt>(obj, "memory", &mut out, Self::rule_memory);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cgroup" => <String as Check>::describe(rest),
                "memory" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl CgroupMemory {
    fn rule_memory(x: &JsInt) -> Result<(), SchemaError> {
        num_at_least(x.as_f64(), 0.0)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsEntry {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: FsEntryType,
    pub size: JsNumber,
    pub mtime: JsNumber,
}
impl Check for FsEntry {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "name", &mut out);
        keys::req_key::<FsEntryType>(obj, "type", &mut out);
        keys::req_key::<JsNumber>(obj, "size", &mut out);
        keys::req_key::<JsNumber>(obj, "mtime", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "type" => <FsEntryType as Check>::describe(rest),
                "size" => <JsNumber as Check>::describe(rest),
                "mtime" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsListReply {
    pub entries: Vec<FsEntry>,
    pub truncated: bool,
    pub total: JsNumber,
}
impl Check for FsListReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.entries.check().map_err(|e| e.under("entries"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<Vec<FsEntry>>(obj, "entries", &mut out);
        keys::req_key::<bool>(obj, "truncated", &mut out);
        keys::req_key::<JsNumber>(obj, "total", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "entries" => <Vec<FsEntry> as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                "total" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsFilesReply {
    pub files: Vec<String>,
    pub truncated: bool,
}
impl Check for FsFilesReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<Vec<String>>(obj, "files", &mut out);
        keys::req_key::<bool>(obj, "truncated", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "files" => <Vec<String> as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsHashReply {
    #[serde(serialize_with = "crate::types::check::js_record", deserialize_with = "crate::types::kept::record")]
    pub files: IndexMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub outside: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub past: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub special: Option<Vec<String>>,
}
impl Check for FsHashReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<IndexMap<String, String>>(obj, "files", &mut out);
        keys::opt_key::<Vec<String>>(obj, "outside", &mut out);
        keys::opt_key::<Vec<String>>(obj, "past", &mut out);
        keys::opt_key::<Vec<String>>(obj, "special", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "files" => <IndexMap<String, String> as Check>::describe(rest),
                "outside" => <Option<Vec<String>> as Check>::describe(rest),
                "past" => <Option<Vec<String>> as Check>::describe(rest),
                "special" => <Option<Vec<String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsImageReply {
    pub size: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub modified: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub inode: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub changed: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub media_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub svg: Option<bool>,
}
impl Check for FsImageReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<JsNumber>(obj, "size", &mut out);
        keys::opt_key::<JsNumber>(obj, "modified", &mut out);
        keys::opt_key::<JsNumber>(obj, "inode", &mut out);
        keys::opt_key::<JsNumber>(obj, "changed", &mut out);
        keys::opt_key::<String>(obj, "mediaType", &mut out);
        keys::opt_key::<String>(obj, "content", &mut out);
        keys::opt_key::<bool>(obj, "svg", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "size" => <JsNumber as Check>::describe(rest),
                "modified" => <Option<JsNumber> as Check>::describe(rest),
                "inode" => <Option<JsNumber> as Check>::describe(rest),
                "changed" => <Option<JsNumber> as Check>::describe(rest),
                "mediaType" => <Option<String> as Check>::describe(rest),
                "content" => <Option<String> as Check>::describe(rest),
                "svg" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsSearchHit {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub text: Option<String>,
}
impl Check for FsSearchHit {
    fn check(&self) -> Result<(), SchemaError> {
        Self::rule_line(&self.line).map_err(|e| e.under("line"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "path", &mut out);
        keys::opt_key_with::<JsInt>(obj, "line", &mut out, Self::rule_line);
        keys::opt_key::<String>(obj, "text", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "line" => <Option<JsInt> as Check>::describe(rest),
                "text" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl FsSearchHit {
    fn rule_line(x: &Option<JsInt>) -> Result<(), SchemaError> {
        if let Some(x) = x {
            num_above(x.as_f64(), 0.0)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsSearchReply {
    pub hits: Vec<FsSearchHit>,
    pub truncated: bool,
}
impl Check for FsSearchReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.hits.check().map_err(|e| e.under("hits"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<Vec<FsSearchHit>>(obj, "hits", &mut out);
        keys::req_key::<bool>(obj, "truncated", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "hits" => <Vec<FsSearchHit> as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsWriteReply {
    pub bytes: JsInt,
}
impl Check for FsWriteReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<JsInt>(obj, "bytes", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "bytes" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitLocalBranch {
    pub name: String,
    pub oid: String,
    pub committed: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub upstream: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<String>,
}
impl Check for GitLocalBranch {
    fn check(&self) -> Result<(), SchemaError> {
        Self::rule_committed(&self.committed).map_err(|e| e.under("committed"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "name", &mut out);
        keys::req_key::<String>(obj, "oid", &mut out);
        keys::req_key_with::<JsInt>(obj, "committed", &mut out, Self::rule_committed);
        keys::opt_key::<String>(obj, "upstream", &mut out);
        keys::opt_key::<String>(obj, "worktree", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "oid" => <String as Check>::describe(rest),
                "committed" => <JsInt as Check>::describe(rest),
                "upstream" => <Option<String> as Check>::describe(rest),
                "worktree" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl GitLocalBranch {
    fn rule_committed(x: &JsInt) -> Result<(), SchemaError> {
        num_at_least(x.as_f64(), 0.0)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBranchesReply {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub current: Option<String>,
    pub branches: Vec<GitLocalBranch>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub truncated: Option<bool>,
}
impl Check for GitBranchesReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.branches.check().map_err(|e| e.under("branches"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::opt_key::<String>(obj, "current", &mut out);
        keys::req_key::<Vec<GitLocalBranch>>(obj, "branches", &mut out);
        keys::opt_key::<bool>(obj, "truncated", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "current" => <Option<String> as Check>::describe(rest),
                "branches" => <Vec<GitLocalBranch> as Check>::describe(rest),
                "truncated" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCheckpointDropReply {
    pub dropped: JsInt,
}
impl Check for GitCheckpointDropReply {
    fn check(&self) -> Result<(), SchemaError> {
        Self::rule_dropped(&self.dropped).map_err(|e| e.under("dropped"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key_with::<JsInt>(obj, "dropped", &mut out, Self::rule_dropped);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "dropped" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl GitCheckpointDropReply {
    fn rule_dropped(x: &JsInt) -> Result<(), SchemaError> {
        num_at_least(x.as_f64(), 0.0)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCheckpointReply {
    #[serde(rename = "ref")]
    pub checkpoint_ref: String,
    pub commit: String,
    pub changed: bool,
    pub based: bool,
}
impl Check for GitCheckpointReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "ref", &mut out);
        keys::req_key::<String>(obj, "commit", &mut out);
        keys::req_key::<bool>(obj, "changed", &mut out);
        keys::req_key::<bool>(obj, "based", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "ref" => <String as Check>::describe(rest),
                "commit" => <String as Check>::describe(rest),
                "changed" => <bool as Check>::describe(rest),
                "based" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitCommitReply {
    pub oid: String,
    pub subject: String,
    pub files_changed: JsInt,
    pub insertions: JsInt,
    pub deletions: JsInt,
}
impl Check for GitCommitReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "oid", &mut out);
        keys::req_key::<String>(obj, "subject", &mut out);
        keys::req_key::<JsInt>(obj, "filesChanged", &mut out);
        keys::req_key::<JsInt>(obj, "insertions", &mut out);
        keys::req_key::<JsInt>(obj, "deletions", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "oid" => <String as Check>::describe(rest),
                "subject" => <String as Check>::describe(rest),
                "filesChanged" => <JsInt as Check>::describe(rest),
                "insertions" => <JsInt as Check>::describe(rest),
                "deletions" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiffFile {
    pub path: String,
    pub kind: String,
    pub additions: JsNumber,
    pub deletions: JsNumber,
    pub patch: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub blob: Option<String>,
}
impl Check for GitDiffFile {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "path", &mut out);
        keys::req_key::<String>(obj, "kind", &mut out);
        keys::req_key::<JsNumber>(obj, "additions", &mut out);
        keys::req_key::<JsNumber>(obj, "deletions", &mut out);
        keys::req_key::<String>(obj, "patch", &mut out);
        keys::opt_key::<String>(obj, "blob", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "kind" => <String as Check>::describe(rest),
                "additions" => <JsNumber as Check>::describe(rest),
                "deletions" => <JsNumber as Check>::describe(rest),
                "patch" => <String as Check>::describe(rest),
                "blob" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitDiscardReply {
    pub path: String,
}
impl Check for GitDiscardReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "path", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostItem {
    pub kind: HostItemKind,
    pub number: JsInt,
    pub title: String,
    pub body: String,
    pub url: String,
}
impl Check for HostItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<HostItemKind>(obj, "kind", &mut out);
        keys::req_key::<JsInt>(obj, "number", &mut out);
        keys::req_key::<String>(obj, "title", &mut out);
        keys::req_key::<String>(obj, "body", &mut out);
        keys::req_key::<String>(obj, "url", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "kind" => <HostItemKind as Check>::describe(rest),
                "number" => <JsInt as Check>::describe(rest),
                "title" => <String as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                "url" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitPrListReply {
    pub items: Vec<HostItem>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub no_cli_for: Option<String>,
}
impl Check for GitPrListReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.items.check().map_err(|e| e.under("items"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<Vec<HostItem>>(obj, "items", &mut out);
        keys::opt_key::<String>(obj, "note", &mut out);
        keys::opt_key::<String>(obj, "noCliFor", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "items" => <Vec<HostItem> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "noCliFor" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitRestoreReply {
    pub before: String,
    pub files: JsInt,
}
impl Check for GitRestoreReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "before", &mut out);
        keys::req_key::<JsInt>(obj, "files", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "before" => <String as Check>::describe(rest),
                "files" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktree {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub head: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub prunable: Option<bool>,
}
impl Check for GitWorktree {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<String>(obj, "path", &mut out);
        keys::opt_key::<String>(obj, "branch", &mut out);
        keys::opt_key::<String>(obj, "head", &mut out);
        keys::opt_key::<bool>(obj, "prunable", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "path" => <String as Check>::describe(rest),
                "branch" => <Option<String> as Check>::describe(rest),
                "head" => <Option<String> as Check>::describe(rest),
                "prunable" => <Option<bool> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitWorktreesReply {
    pub worktrees: Vec<GitWorktree>,
}
impl Check for GitWorktreesReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.worktrees.check().map_err(|e| e.under("worktrees"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<Vec<GitWorktree>>(obj, "worktrees", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "worktrees" => <Vec<GitWorktree> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpPostReply {
    pub status: JsInt,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub session_id: Option<String>,
    pub body: String,
}
impl Check for HttpPostReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<JsInt>(obj, "status", &mut out);
        keys::opt_key::<String>(obj, "contentType", &mut out);
        keys::opt_key::<String>(obj, "sessionId", &mut out);
        keys::req_key::<String>(obj, "body", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "status" => <JsInt as Check>::describe(rest),
                "contentType" => <Option<String> as Check>::describe(rest),
                "sessionId" => <Option<String> as Check>::describe(rest),
                "body" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcListReply {
    pub at: JsNumber,
    pub daemon: JsInt,
    pub total: JsInt,
    pub procs: Vec<ProcEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cgroups: Option<Vec<CgroupMemory>>,
}
impl Check for ProcListReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.procs.check().map_err(|e| e.under("procs"))?;
        self.cgroups.check().map_err(|e| e.under("cgroups"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<JsNumber>(obj, "at", &mut out);
        keys::req_key::<JsInt>(obj, "daemon", &mut out);
        keys::req_key::<JsInt>(obj, "total", &mut out);
        keys::req_key::<Vec<ProcEntry>>(obj, "procs", &mut out);
        keys::opt_key::<Vec<CgroupMemory>>(obj, "cgroups", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <JsNumber as Check>::describe(rest),
                "daemon" => <JsInt as Check>::describe(rest),
                "total" => <JsInt as Check>::describe(rest),
                "procs" => <Vec<ProcEntry> as Check>::describe(rest),
                "cgroups" => <Option<Vec<CgroupMemory>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SshStartReply {
    pub port: JsInt,
    pub host_key: String,
}
impl Check for SshStartReply {
    fn check(&self) -> Result<(), SchemaError> {
        Self::rule_port(&self.port).map_err(|e| e.under("port"))?;
        Self::rule_host_key(&self.host_key).map_err(|e| e.under("hostKey"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key_with::<JsInt>(obj, "port", &mut out, Self::rule_port);
        keys::req_key_with::<String>(obj, "hostKey", &mut out, Self::rule_host_key);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "port" => <JsInt as Check>::describe(rest),
                "hostKey" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl SshStartReply {
    fn rule_port(x: &JsInt) -> Result<(), SchemaError> {
        num_at_least(x.as_f64(), 1.0)?;
        num_at_most(x.as_f64(), 65535.0)?;
        Ok(())
    }
    fn rule_host_key(x: &String) -> Result<(), SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, SSH_KEY_MAX as usize)?;
        regex_check(is_ssh_host_key_line(x))?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStore {
    pub agent: String,
    pub format: UsageLogFormat,
    pub root: String,
}
impl Check for UsageStore {
    fn check(&self) -> Result<(), SchemaError> {
        Self::rule_agent(&self.agent).map_err(|e| e.under("agent"))?;
        Self::rule_root(&self.root).map_err(|e| e.under("root"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key_with::<String>(obj, "agent", &mut out, Self::rule_agent);
        keys::req_key::<UsageLogFormat>(obj, "format", &mut out);
        keys::req_key_with::<String>(obj, "root", &mut out, Self::rule_root);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "agent" => <String as Check>::describe(rest),
                "format" => <UsageLogFormat as Check>::describe(rest),
                "root" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl UsageStore {
    fn rule_agent(x: &String) -> Result<(), SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 64)?;
        Ok(())
    }
    fn rule_root(x: &String) -> Result<(), SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 4096)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpBearer {
    pub file: String,
    pub at: Vec<String>,
    pub field: String,
}
impl Check for HttpBearer {
    fn check(&self) -> Result<(), SchemaError> {
        Self::rule_file(&self.file).map_err(|e| e.under("file"))?;
        Self::rule_at(&self.at).map_err(|e| e.under("at"))?;
        Self::rule_field(&self.field).map_err(|e| e.under("field"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key_with::<String>(obj, "file", &mut out, Self::rule_file);
        text_list_key(obj, "at", &mut out, 8, Self::rule_at_item);
        keys::req_key_with::<String>(obj, "field", &mut out, Self::rule_field);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "file" => <String as Check>::describe(rest),
                "at" => <Vec<String> as Check>::describe(rest),
                "field" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl HttpBearer {
    fn rule_file(x: &String) -> Result<(), SchemaError> {
        text_at_least(x, 3)?;
        text_at_most(x, 4096)?;
        Ok(())
    }
    fn rule_at(x: &Vec<String>) -> Result<(), SchemaError> {
        items_at_most(x.len(), 8)?;
        for (i, item) in x.iter().enumerate() {
            Self::rule_at_item(item).map_err(|e| e.under(i))?;
        }
        Ok(())
    }
    fn rule_field(x: &String) -> Result<(), SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 255)?;
        Ok(())
    }
}

impl HttpBearer {
    fn rule_at_item(x: &str) -> Result<(), SchemaError> {
        text_at_least(x, 1)?;
        text_at_most(x, 1024)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PipeOpen {
    op: PipeOpenOp,
    pub door: PipeDoor,
    pub held_ms: JsInt,
}
impl Check for PipeOpen {
    fn check(&self) -> Result<(), SchemaError> {
        Self::rule_held_ms(&self.held_ms).map_err(|e| e.under("heldMs"))?;
        Ok(())
    }

    fn pass(value: &Value) -> Option<Vec<SchemaError>> {
        let Value::Object(obj) = value else {
            return Some(vec![not_object(value)]);
        };
        let mut out = Vec::new();
        keys::req_key::<PipeOpenOp>(obj, "op", &mut out);
        keys::req_key::<PipeDoor>(obj, "door", &mut out);
        keys::req_key_with::<JsInt>(obj, "heldMs", &mut out, Self::rule_held_ms);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<Zod> {
        match path.split_first() {
            None => Some(Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "op" => <PipeOpenOp as Check>::describe(rest),
                "door" => <PipeDoor as Check>::describe(rest),
                "heldMs" => <JsInt as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PipeOpen {
    fn rule_held_ms(x: &JsInt) -> Result<(), SchemaError> {
        num_at_least(x.as_f64(), 0.0)?;
        Ok(())
    }
}
