// SPDX-License-Identifier: AGPL-3.0-only
//! How each agent loads the machine context on the guest. Port of packages/catalog/src/context.ts at a3b870a6e; the
//! spec is spec/skills_context.md.
use serde::Serialize;
use serde_json::{json, Value};

use wsp_wire::types::shell_quote::shell_quote;

pub const CONTEXT_MARKER: &str = "wsp writes this file when a workspace is forked; edits are overwritten.";
pub const SKILL_NAME: &str = "wsp-machine";
/// The mode of every file the context lands (context.ts:293).
pub const MODE: u32 = 0o644;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestRoots {
    pub etc: String,
    pub home: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GuestFile {
    pub path: String,
    pub mode: u32,
    pub content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ContextOutcomeKind {
    Written,
    Fallback,
    NotLoaded,
}

/// context.ts:261.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextHookInput {
    pub short: String,
    pub skill: String,
    pub source: String,
    pub roots: GuestRoots,
    pub conflict: bool,
    pub fish: bool,
}

/// context.ts:274.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ContextHooks {
    pub outcome: ContextOutcomeKind,
    pub files: Vec<GuestFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    pub skill: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CdFacts {
    pub fact: String,
    pub howto: String,
}

/// One module per agent (context.ts:326-400).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentContext {
    Claude,
    Codex,
    Gemini,
    Opencode,
    Pi,
    Hermes,
}

/// context.ts:299: the Gemini CLI check, which reads settings.json with its comments taken out.
const GEMINI_FILENAME_CHECK: &str = concat!(
    r#"const t=require("fs").readFileSync(process.argv[1],"utf8");"#,
    r#"let o="",i=0,s=false;"#,
    r#"while(i<t.length){const c=t[i];"#,
    r#"if(s){o+=c;if(c==="\\"&&i+1<t.length)o+=t[++i];else if(c==='"')s=false;i++}"#,
    r#"else if(c==='"'){s=true;o+=c;i++}"#,
    r#"else if(c==="/"&&t[i+1]==="/"){while(i<t.length&&t[i]!=="\n")i++}"#,
    r#"else if(c==="/"&&t[i+1]==="*"){const e=t.indexOf("*/",i+2);i=e<0?t.length:e+2}"#,
    r#"else{o+=c;i++}}"#,
    r#"let hit;try{const j=JSON.parse(o);hit=!!(j&&j.context&&j.context.fileName!==undefined)}catch{hit=/"fileName"\s*:/.test(o)}"#,
    r#"process.exit(hit?0:1)"#,
);

/// context.ts:305: the Hermes check that agent.environment_hint is set, with awk.
const HERMES_HINT_CHECK: &str = concat!(
    r"/^[^[:space:]#]/{blk=$1}",
    " ",
    r#"blk=="agent:" && /^[[:space:]]+environment_hint[[:space:]]*:/{v=$0; sub(/^[^:]*:[[:space:]]*/,"",v); sub(/[[:space:]]+#.*$/,"",v); if (v!="" && v!="\"\"" && v!="''") f=1}"#,
    " ",
    "END{exit !f}",
);

const CLAUDE_CD_FACT: &str = "- Every agent session starts in the thread's folder; terminal panes open in the home folder. A cd moves your own shell, which persists across your tool calls, not the thread's folder, and the files pane follows that shell's folder unless you pinned the panes.";
const CLAUDE_CD_HOWTO: &str = "- Work in a folder: cd <dir> in your shell and it stays there across your tool calls; the thread's folder does not move. Absolute paths work from anywhere.";

fn file(path: &str, content: &str) -> GuestFile {
    GuestFile { path: path.to_string(), mode: MODE, content: content.to_string() }
}

fn skill_at(dir: &str, skill: &str) -> GuestFile {
    file(&format!("{dir}/{SKILL_NAME}/SKILL.md"), skill)
}

/// JSON.stringify(value, null, 2) with the trailing newline the TS files end with.
fn pretty_json(value: &Value) -> String {
    format!("{}\n", serde_json::to_string_pretty(value).expect("a JSON value serializes"))
}

impl AgentContext {
    /// Claude Code's alone.
    pub fn cd(self) -> Option<CdFacts> {
        match self {
            AgentContext::Claude => Some(CdFacts { fact: CLAUDE_CD_FACT.to_string(), howto: CLAUDE_CD_HOWTO.to_string() }),
            _ => None,
        }
    }

    /// The probe condition: Gemini CLI, Pi and Hermes.
    pub fn conflict(self, roots: &GuestRoots) -> Option<String> {
        let home = &roots.home;
        match self {
            AgentContext::Gemini => Some(format!(
                "[ -f {home}/.gemini/settings.json ] && node -e {} {home}/.gemini/settings.json 2>/dev/null",
                shell_quote(GEMINI_FILENAME_CHECK)
            )),
            AgentContext::Pi => Some(format!(
                "[ -f {home}/.pi/agent/APPEND_SYSTEM.md ] && ! head -n 1 {home}/.pi/agent/APPEND_SYSTEM.md | grep -qF {}",
                shell_quote(CONTEXT_MARKER)
            )),
            AgentContext::Hermes => {
                Some(format!("[ -f {home}/.hermes/config.yaml ] && awk {} {home}/.hermes/config.yaml", shell_quote(HERMES_HINT_CHECK)))
            }
            AgentContext::Claude | AgentContext::Codex | AgentContext::Opencode => None,
        }
    }

    pub fn hooks(self, input: &ContextHookInput) -> ContextHooks {
        let ContextHookInput { short, skill, source, roots, conflict, fish } = input;
        let (etc, home) = (roots.etc.as_str(), roots.home.as_str());
        match self {
            AgentContext::Claude => {
                let s = skill_at(&format!("{etc}/claude-code/.claude/skills"), skill);
                let path = format!("{etc}/claude-code/CLAUDE.md");
                ContextHooks {
                    outcome: ContextOutcomeKind::Written,
                    files: vec![file(&path, short), s.clone()],
                    path: Some(path),
                    note: None,
                    skill: s.path,
                }
            }
            AgentContext::Codex => {
                let s = skill_at(&format!("{etc}/codex/skills"), skill);
                let path = format!("{etc}/codex/requirements.toml");
                let content = format!("# {CONTEXT_MARKER}\nadditional_developer_instructions = {}\n", toml_doc_string(short));
                ContextHooks {
                    outcome: ContextOutcomeKind::Written,
                    files: vec![file(&path, &content), s.clone()],
                    path: Some(path),
                    note: None,
                    skill: s.path,
                }
            }
            AgentContext::Gemini => {
                let s = skill_at(&format!("{home}/.gemini/skills"), skill);
                if *conflict {
                    let dir = format!("{home}/.gemini/extensions/{SKILL_NAME}");
                    let manifest = pretty_json(&json!({ "name": SKILL_NAME, "version": "1.0.0", "contextFileName": "WSP-MACHINE.md" }));
                    return ContextHooks {
                        outcome: ContextOutcomeKind::Fallback,
                        files: vec![
                            file(&format!("{dir}/gemini-extension.json"), &manifest),
                            file(&format!("{dir}/WSP-MACHINE.md"), short),
                            s.clone(),
                        ],
                        path: Some(dir),
                        note: Some("~/.gemini/settings.json sets context.fileName".to_string()),
                        skill: s.path,
                    };
                }
                let defaults = pretty_json(&json!({ "context": { "fileName": ["GEMINI.md", "WSP-MACHINE.md"] } }));
                let path = format!("{home}/.gemini/WSP-MACHINE.md");
                ContextHooks {
                    outcome: ContextOutcomeKind::Written,
                    files: vec![file(&format!("{etc}/gemini-cli/system-defaults.json"), &defaults), file(&path, short), s.clone()],
                    path: Some(path),
                    note: None,
                    skill: s.path,
                }
            }
            AgentContext::Opencode => {
                let s = skill_at(&format!("{home}/.config/opencode/skills"), skill);
                let path = format!("{etc}/opencode/opencode.json");
                let content = pretty_json(&json!({ "instructions": [source] }));
                ContextHooks {
                    outcome: ContextOutcomeKind::Written,
                    files: vec![file(&path, &content), s.clone()],
                    path: Some(path),
                    note: None,
                    skill: s.path,
                }
            }
            AgentContext::Pi => {
                let s = skill_at(&format!("{home}/.pi/agent/skills"), skill);
                if *conflict {
                    let ext = format!("{home}/.pi/agent/extensions/{SKILL_NAME}.ts");
                    let code = [
                        format!("// {CONTEXT_MARKER}"),
                        r#"import { readFileSync } from "node:fs";"#.to_string(),
                        "export default function (pi) {".to_string(),
                        format!(r#"  pi.on("before_agent_start", async event => ({{ systemPrompt: `${{event.systemPrompt}}\n\n${{readFileSync({}, "utf8")}}` }}));"#, serde_json::to_string(source).expect("a string serializes")),
                        "}".to_string(),
                        String::new(),
                    ]
                    .join("\n");
                    return ContextHooks {
                        outcome: ContextOutcomeKind::Fallback,
                        files: vec![file(&ext, &code), s.clone()],
                        path: Some(ext),
                        note: Some("~/.pi/agent/APPEND_SYSTEM.md already exists".to_string()),
                        skill: s.path,
                    };
                }
                let path = format!("{home}/.pi/agent/APPEND_SYSTEM.md");
                ContextHooks {
                    outcome: ContextOutcomeKind::Written,
                    files: vec![file(&path, short), s.clone()],
                    path: Some(path),
                    note: None,
                    skill: s.path,
                }
            }
            AgentContext::Hermes => {
                let s = skill_at(&format!("{home}/.hermes/skills"), skill);
                if *conflict {
                    return ContextHooks {
                        outcome: ContextOutcomeKind::NotLoaded,
                        files: vec![s.clone()],
                        path: None,
                        note: Some("~/.hermes/config.yaml sets agent.environment_hint".to_string()),
                        skill: s.path,
                    };
                }
                let profile = format!("{etc}/profile.d/wsp-machine.sh");
                let mut files = vec![file(&profile, &format!("# {CONTEXT_MARKER}\nexport HERMES_ENVIRONMENT_HINT=\"$(cat {source})\"\n"))];
                if *fish {
                    files.push(file(
                        &format!("{etc}/fish/conf.d/wsp-machine.fish"),
                        &format!("# {CONTEXT_MARKER}\nset -gx HERMES_ENVIRONMENT_HINT (cat {source} | string collect)\n"),
                    ));
                }
                files.push(s.clone());
                ContextHooks { outcome: ContextOutcomeKind::Written, files, path: Some(profile), note: None, skill: s.path }
            }
        }
    }
}

/// A literal multi-line TOML string unless the text holds `'''` (context.ts:299).
pub fn toml_doc_string(doc: &str) -> String {
    if !doc.contains("'''") {
        return format!("'''\n{doc}'''");
    }
    serde_json::to_string(doc).expect("a string serializes")
}
