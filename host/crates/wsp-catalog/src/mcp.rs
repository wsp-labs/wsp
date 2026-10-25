// SPDX-License-Identifier: AGPL-3.0-only
//! How an agent keeps its MCP servers: the shapes every format reads and writes, and the four formats. Each method
//! dispatches to the format's own module (mcp_json, mcp_json_refer, mcp_toml, mcp_toml_refer, mcp_codex). Port of the
//! types and format tables of packages/catalog/src/mcp.ts at a3b870a6e; the spec is spec/mcp_refs.md.
use indexmap::{IndexMap, IndexSet};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::mcp_codex::{resolve_codex, trust_codex};
use crate::mcp_json;
use crate::mcp_json_refer;
use crate::mcp_toml::{codex_editor, codex_merge, enable_codex, entry_of_codex, only_codex, place_codex, read_codex, remove_codex};
use crate::mcp_toml_refer::refer_codex;
use crate::mcp_travelled::FromTo;
use wsp_words::exit::HostError;

/// How a server starts (mcp.ts:15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum McpTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: IndexMap<String, String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        cwd: Option<String>,
        #[serde(rename = "toolTimeoutSec", skip_serializing_if = "Option::is_none")]
        tool_timeout_sec: Option<u64>,
    },
    Http {
        url: String,
        headers: IndexMap<String, String>,
    },
}

/// `home`: Claude Code's project scope for the home folder itself (mcp.ts:22).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum McpServerScope {
    User,
    Home,
}

/// What Codex hands a server from its own environment (mcp.ts:30).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct McpReads {
    pub env: Vec<String>,
    pub headers: IndexMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bearer: Option<String>,
}

/// One server as every format reads it (mcp.ts:19). `disabled` is `Some(true)` or absent, never `Some(false)`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpServer {
    pub name: String,
    pub scope: McpServerScope,
    pub transport: McpTransport,
    pub env_refs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reads: Option<McpReads>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
}

/// One config on the machine as its editor gets it (mcp.ts:46).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpEditScope {
    pub keep: Vec<String>,
    pub drop: Vec<String>,
    pub project: Option<FromTo>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditOutcome {
    Written,
    Missing,
    Dropped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpEditResult {
    pub name: String,
    pub outcome: EditOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpEdited {
    pub text: String,
    pub results: Vec<McpEditResult>,
}

/// One config as a merge gets it: `replace` names wsp's own entries (mcp.ts:62).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct McpMergeScope {
    pub keep: Vec<String>,
    pub drop: Vec<String>,
    pub project: Option<FromTo>,
    pub replace: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MergeOutcome {
    Added,
    Replaced,
    Same,
    Theirs,
    Missing,
    Dropped,
    Left,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpMergeResult {
    pub name: String,
    pub outcome: MergeOutcome,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpMerged {
    pub text: String,
    pub results: Vec<McpMergeResult>,
}

/// A kept definition's string as the machine reads it; `command` marks the program (mcp.ts:85).
pub type RewriteString<'a> = &'a dyn Fn(&str, bool) -> String;

/// A string that is one reference and nothing else (mcp.ts:161).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WholeRef {
    pub name: String,
    pub scheme: String,
    pub fallback: Option<String>,
}

/// What a resolve came to (mcp.ts:167).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum McpResolved {
    Resolved { transport: McpTransport, values: Vec<String> },
    Missing(String),
}

/// One server a reference writer weighed (mcp.ts:175).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ReferredServer {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    pub values: IndexMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unread: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct McpReferred {
    pub text: String,
    pub servers: Vec<ReferredServer>,
    /// Every server definition the returned text holds, parsed.
    pub entries: Vec<Value>,
}

/// A variable servers.env holds, with its value (mcp.ts:305).
pub type KnownValue = (String, String);

/// The four config formats (mcp.ts:1208, 1212, 1228, 1737).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum McpFormat {
    /// Claude Code's `mcpServers`, `${X}` and `${X:-d}`.
    McpServersJson,
    /// Gemini CLI's settings: also `$X`, `httpUrl`, `mcp.excluded`.
    GeminiSettingsJson,
    /// OpenCode's `mcp`, `{env:X}`, `enabled`.
    OpencodeJson,
    /// Codex's `[mcp_servers.<name>]` tables.
    CodexToml,
}

impl McpFormat {
    /// Every server the text defines; text that is not the format defines none. `home` is the folder whose
    /// project-scoped servers count as the person's own (mcp.ts:103).
    pub fn read(self, text: &str, home: &str) -> Vec<McpServer> {
        if self.is_json() {
            mcp_json::read(self, text, home)
        } else {
            read_codex(text)
        }
    }

    /// The text with the server placed, or replaced where it stands; `None` text is a file not there yet (mcp.ts:106).
    pub fn place(self, text: Option<&str>, name: &str, server: &McpTransport) -> Result<String, HostError> {
        if self.is_json() {
            mcp_json::place(self, text, name, server)
        } else {
            Ok(place_codex(text, name, server))
        }
    }

    /// Whether the format keeps a switch per server (OpenCode, Gemini CLI, Codex).
    pub fn has_enable(self) -> bool {
        self != McpFormat::McpServersJson
    }

    /// The text with the server turned on or off by the agent's own switch (mcp.ts:110); `None` for a format with no
    /// switch.
    pub fn enable(self, text: &str, name: &str, on: bool, project: Option<&str>) -> Option<Result<String, HostError>> {
        if self.is_json() {
            mcp_json::enable(self, text, name, on, project)
        } else {
            Some(enable_codex(text, name, on))
        }
    }

    /// The text with the folder marked trusted (mcp.ts:113); `None` for every format but Codex's.
    pub fn trust(self, text: Option<&str>, folder: &str) -> Option<Result<String, HostError>> {
        (!self.is_json()).then(|| trust_codex(text, folder))
    }

    /// A copy holding the named servers alone (mcp.ts:117).
    pub fn only(self, text: &str, names: &[String], from: Option<&str>, to: Option<&str>) -> Result<String, HostError> {
        if self.is_json() {
            mcp_json::only(self, text, names, from, to)
        } else {
            Ok(only_codex(text, names))
        }
    }

    /// The edit the import runs over the text it read off the machine (mcp.ts:119).
    pub fn edit(self, lib: RewriteString<'_>, scope: &McpEditScope, text: &str) -> Result<McpEdited, HostError> {
        if self.is_json() {
            mcp_json::edit(self, lib, scope, text)
        } else {
            codex_editor(lib, scope, text)
        }
    }

    /// One server's definition in the one shape it keeps when the agent rewrites the file (mcp.ts:125).
    pub fn entry_of(self, text: &str, name: &str, project: Option<&str>) -> Option<String> {
        if self.is_json() {
            mcp_json::entry_of(self, text, name, project)
        } else {
            entry_of_codex(text, name)
        }
    }

    /// The agent's own file with wsp's servers merged in (mcp.ts:130).
    pub fn merge(self, lib: RewriteString<'_>, scope: &McpMergeScope, own: Option<&str>, travelled: &str) -> Result<McpMerged, HostError> {
        if self.is_json() {
            mcp_json::merge(self, lib, scope, own, travelled)
        } else {
            codex_merge(lib, scope, own, travelled)
        }
    }

    /// The text with the named servers taken out, every other byte of theirs where it was (mcp.ts:135).
    pub fn remove(self, text: &str, names: &[String], project: Option<&str>) -> Result<String, HostError> {
        if self.is_json() {
            mcp_json::remove(self, text, names, project)
        } else {
            Ok(remove_codex(text, names))
        }
    }

    /// The text with every header and command variable written by name (mcp.ts:146). The TS returns a Promise only to
    /// load its TOML parser lazily; the port is synchronous.
    pub fn refer(self, text: &str, only: Option<&str>, known: &[KnownValue], held: &IndexSet<String>) -> Result<McpReferred, HostError> {
        if self.is_json() {
            mcp_json_refer::refer(self, text, only, known, held)
        } else {
            refer_codex(text, only, known, held)
        }
    }

    /// The server's transport as its agent starts it (mcp.ts:150).
    pub fn resolve(self, server: &McpServer, value: &dyn Fn(&str) -> Option<String>) -> McpResolved {
        if self.is_json() {
            mcp_json::resolve(self, server, value)
        } else {
            resolve_codex(server, value)
        }
    }

    /// A variable as the agent expands it in arguments and addresses; `None` for Codex (mcp.ts:153).
    pub fn arg_ref(self, variable: &str) -> Option<String> {
        self.is_json().then(|| mcp_json::ref_of(self, variable))
    }

    /// The string as one whole reference in the agent's syntax; `None` for Codex and for a string that is not one
    /// (mcp.ts:156).
    pub fn named(self, value: &str) -> Option<WholeRef> {
        if self.is_json() {
            mcp_json::whole_ref(self, value, false)
        } else {
            None
        }
    }

    fn is_json(self) -> bool {
        self != McpFormat::CodexToml
    }
}

pub const MCP_SERVERS_JSON: McpFormat = McpFormat::McpServersJson;
pub const GEMINI_SETTINGS_JSON: McpFormat = McpFormat::GeminiSettingsJson;
pub const OPENCODE_JSON: McpFormat = McpFormat::OpencodeJson;
pub const CODEX_TOML: McpFormat = McpFormat::CodexToml;
