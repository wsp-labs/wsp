// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/views/workspace.ts:557 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceStatus {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub queue: Option<WorkspaceViewQueue>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub screen: Option<WorkspaceViewScreen>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review: Option<ReviewDraft>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub branch_of_record: Option<BranchOfRecord>,
    pub machine_state: MachineState,
    pub reach: ReachStatus,
    pub size: WorkspaceSize,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub facts: Option<MachineFacts>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_ask: Option<WorkspaceStatusWakeAsk>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub idle_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub checkout: Option<Checkout>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pr: Option<PullRequestSeen>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tree: Option<TreeFact>,
}
impl Check for WorkspaceStatus {
    /// Nested checks: phase, kind, project, theme, glyph, agents, worktree, pauseMode, from, review, branchOfRecord, machineState, reach, size, facts, wakeAsk, checkout, pr, tree.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.queue.check().map_err(|e| e.under("queue"))?;
        self.screen.check().map_err(|e| e.under("screen"))?;
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        self.daemon_refused_at.check().map_err(|e| e.under("daemonRefusedAt"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.worktree.check().map_err(|e| e.under("worktree"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.facts.check().map_err(|e| e.under("facts"))?;
        self.wake_ask.check().map_err(|e| e.under("wakeAsk"))?;
        self.checkout.check().map_err(|e| e.under("checkout"))?;
        self.pr.check().map_err(|e| e.under("pr"))?;
        self.tree.check().map_err(|e| e.under("tree"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        self.branch_of_record.check().map_err(|e| e.under("branchOfRecord"))?;
        self.machine_state.check().map_err(|e| e.under("machineState"))?;
        self.reach.check().map_err(|e| e.under("reach"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
        crate::types::keys::req_key::<WorkspacePhase>(obj, "phase", &mut out);
        crate::types::keys::opt_key::<WorkspaceKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "golden", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<ProjectRef>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "folder", &mut out);
        crate::types::keys::opt_key::<String>(obj, "home", &mut out);
        crate::types::keys::opt_key::<String>(obj, "claudeSessionId", &mut out);
        crate::types::keys::opt_key::<WorkspaceViewQueue>(obj, "queue", &mut out);
        crate::types::keys::opt_key::<WorkspaceViewScreen>(obj, "screen", &mut out);
        crate::types::keys::opt_key::<String>(obj, "gone", &mut out);
        crate::types::keys::opt_key::<WorkspaceTheme>(obj, "theme", &mut out);
        crate::types::keys::opt_key::<WorkspaceGlyph>(obj, "glyph", &mut out);
        crate::types::keys::opt_key::<String>(obj, "daemonNote", &mut out);
        crate::types::keys::opt_key::<String>(obj, "vaultedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "vaultRefused", &mut out);
        crate::types::keys::opt_key::<WorkspaceViewDaemonRefusedAt>(obj, "daemonRefusedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "wakeRefused", &mut out);
        crate::types::keys::opt_key::<String>(obj, "diskGone", &mut out);
        crate::types::keys::opt_key::<WorkspaceAgents>(obj, "agents", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "rootThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentWorkspaceId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "place", &mut out);
        crate::types::keys::opt_key::<WorktreeFolder>(obj, "worktree", &mut out);
        crate::types::keys::opt_key::<String>(obj, "provider", &mut out);
        crate::types::keys::opt_key::<PauseMode>(obj, "pauseMode", &mut out);
        crate::types::keys::opt_key::<WorkspaceFrom>(obj, "from", &mut out);
        crate::types::keys::opt_key::<ReviewDraft>(obj, "review", &mut out);
        crate::types::keys::opt_key::<BranchOfRecord>(obj, "branchOfRecord", &mut out);
        crate::types::keys::req_key::<MachineState>(obj, "machineState", &mut out);
        crate::types::keys::req_key::<ReachStatus>(obj, "reach", &mut out);
        crate::types::keys::req_key::<WorkspaceSize>(obj, "size", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "rateUsdPerHour", &mut out);
        crate::types::keys::opt_key::<MachineFacts>(obj, "facts", &mut out);
        crate::types::keys::opt_key::<String>(obj, "reason", &mut out);
        crate::types::keys::opt_key::<WorkspaceStatusWakeAsk>(obj, "wakeAsk", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "idleAt", &mut out);
        crate::types::keys::opt_key::<Checkout>(obj, "checkout", &mut out);
        crate::types::keys::opt_key::<PullRequestSeen>(obj, "pr", &mut out);
        crate::types::keys::opt_key::<TreeFact>(obj, "tree", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "home" => <Option<String> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "queue" => <Option<WorkspaceViewQueue> as Check>::describe(rest),
                "screen" => <Option<WorkspaceViewScreen> as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                "theme" => <Option<WorkspaceTheme> as Check>::describe(rest),
                "glyph" => <Option<WorkspaceGlyph> as Check>::describe(rest),
                "daemonNote" => <Option<String> as Check>::describe(rest),
                "vaultedAt" => <Option<String> as Check>::describe(rest),
                "vaultRefused" => <Option<String> as Check>::describe(rest),
                "daemonRefusedAt" => <Option<WorkspaceViewDaemonRefusedAt> as Check>::describe(rest),
                "wakeRefused" => <Option<String> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "parentWorkspaceId" => <Option<String> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "worktree" => <Option<WorktreeFolder> as Check>::describe(rest),
                "provider" => <Option<String> as Check>::describe(rest),
                "pauseMode" => None,
                "from" => <Option<WorkspaceFrom> as Check>::describe(rest),
                "review" => <Option<ReviewDraft> as Check>::describe(rest),
                "branchOfRecord" => None,
                "machineState" => None,
                "reach" => None,
                "size" => None,
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                "facts" => <Option<MachineFacts> as Check>::describe(rest),
                "reason" => <Option<String> as Check>::describe(rest),
                "wakeAsk" => <Option<WorkspaceStatusWakeAsk> as Check>::describe(rest),
                "idleAt" => <Option<JsNumber> as Check>::describe(rest),
                "checkout" => <Option<Checkout> as Check>::describe(rest),
                "pr" => <Option<PullRequestSeen> as Check>::describe(rest),
                "tree" => <Option<TreeFact> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:596 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOut {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review: Option<ReviewDraft>,
}
impl Check for WorkspaceOut {
    /// Nested checks: phase, kind, project, theme, glyph, agents, pauseMode, worktree, from, review.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        self.daemon_refused_at.check().map_err(|e| e.under("daemonRefusedAt"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.worktree.check().map_err(|e| e.under("worktree"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
        crate::types::keys::req_key::<WorkspacePhase>(obj, "phase", &mut out);
        crate::types::keys::opt_key::<WorkspaceKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "golden", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<ProjectRef>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "folder", &mut out);
        crate::types::keys::opt_key::<String>(obj, "home", &mut out);
        crate::types::keys::opt_key::<String>(obj, "claudeSessionId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "gone", &mut out);
        crate::types::keys::opt_key::<WorkspaceTheme>(obj, "theme", &mut out);
        crate::types::keys::opt_key::<WorkspaceGlyph>(obj, "glyph", &mut out);
        crate::types::keys::opt_key::<String>(obj, "daemonNote", &mut out);
        crate::types::keys::opt_key::<WorkspaceViewDaemonRefusedAt>(obj, "daemonRefusedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "vaultedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "vaultRefused", &mut out);
        crate::types::keys::opt_key::<String>(obj, "wakeRefused", &mut out);
        crate::types::keys::opt_key::<String>(obj, "diskGone", &mut out);
        crate::types::keys::opt_key::<WorkspaceAgents>(obj, "agents", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "rootThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentWorkspaceId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "place", &mut out);
        crate::types::keys::opt_key::<String>(obj, "provider", &mut out);
        crate::types::keys::opt_key::<PauseMode>(obj, "pauseMode", &mut out);
        crate::types::keys::opt_key::<WorktreeFolder>(obj, "worktree", &mut out);
        crate::types::keys::opt_key::<WorkspaceFrom>(obj, "from", &mut out);
        crate::types::keys::opt_key::<ReviewDraft>(obj, "review", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "home" => <Option<String> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                "theme" => <Option<WorkspaceTheme> as Check>::describe(rest),
                "glyph" => <Option<WorkspaceGlyph> as Check>::describe(rest),
                "daemonNote" => <Option<String> as Check>::describe(rest),
                "daemonRefusedAt" => <Option<WorkspaceViewDaemonRefusedAt> as Check>::describe(rest),
                "vaultedAt" => <Option<String> as Check>::describe(rest),
                "vaultRefused" => <Option<String> as Check>::describe(rest),
                "wakeRefused" => <Option<String> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "parentWorkspaceId" => <Option<String> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "provider" => <Option<String> as Check>::describe(rest),
                "pauseMode" => None,
                "worktree" => <Option<WorktreeFolder> as Check>::describe(rest),
                "from" => <Option<WorkspaceFrom> as Check>::describe(rest),
                "review" => <Option<ReviewDraft> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:602 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTalked {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
}
impl Check for WorkspaceTalked {
    /// Nested checks: phase, kind, project.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
        crate::types::keys::req_key::<WorkspacePhase>(obj, "phase", &mut out);
        crate::types::keys::opt_key::<WorkspaceKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "golden", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<ProjectRef>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "gone", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/workspace.ts:608 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListing {
    pub id: String,
    pub name: String,
    pub machine_id: String,
    pub phase: WorkspacePhase,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kind: Option<WorkspaceKind>,
    pub golden: String,
    pub created_at: String,
    pub project: ProjectRef,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub folder: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub claude_session_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub theme: Option<WorkspaceTheme>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub glyph: Option<WorkspaceGlyph>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_refused_at: Option<WorkspaceViewDaemonRefusedAt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vaulted_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub vault_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wake_refused: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_gone: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub root_thread_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub parent_workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pause_mode: Option<PauseMode>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub worktree: Option<WorktreeFolder>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub from: Option<WorkspaceFrom>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub review: Option<ReviewDraft>,
    pub machine_state: MachineState,
    pub size: WorkspaceSize,
    pub rate_usd_per_hour: JsNumber,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub idle_at: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub facts: Option<MachineFacts>,
    pub reach: ReachView,
}
impl Check for WorkspaceListing {
    /// Nested checks: phase, kind, project, theme, glyph, agents, pauseMode, worktree, from, review, machineState, size, facts, reach.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.project.check().map_err(|e| e.under("project"))?;
        self.theme.check().map_err(|e| e.under("theme"))?;
        self.glyph.check().map_err(|e| e.under("glyph"))?;
        self.daemon_refused_at.check().map_err(|e| e.under("daemonRefusedAt"))?;
        self.agents.check().map_err(|e| e.under("agents"))?;
        self.worktree.check().map_err(|e| e.under("worktree"))?;
        self.from.check().map_err(|e| e.under("from"))?;
        self.review.check().map_err(|e| e.under("review"))?;
        self.facts.check().map_err(|e| e.under("facts"))?;
        self.phase.check().map_err(|e| e.under("phase"))?;
        self.pause_mode.check().map_err(|e| e.under("pauseMode"))?;
        self.machine_state.check().map_err(|e| e.under("machineState"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        self.reach.check().map_err(|e| e.under("reach"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
        crate::types::keys::req_key::<WorkspacePhase>(obj, "phase", &mut out);
        crate::types::keys::opt_key::<WorkspaceKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "golden", &mut out);
        crate::types::keys::req_key::<String>(obj, "createdAt", &mut out);
        crate::types::keys::req_key::<ProjectRef>(obj, "project", &mut out);
        crate::types::keys::opt_key::<String>(obj, "folder", &mut out);
        crate::types::keys::opt_key::<String>(obj, "home", &mut out);
        crate::types::keys::opt_key::<String>(obj, "claudeSessionId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "gone", &mut out);
        crate::types::keys::opt_key::<WorkspaceTheme>(obj, "theme", &mut out);
        crate::types::keys::opt_key::<WorkspaceGlyph>(obj, "glyph", &mut out);
        crate::types::keys::opt_key::<String>(obj, "daemonNote", &mut out);
        crate::types::keys::opt_key::<WorkspaceViewDaemonRefusedAt>(obj, "daemonRefusedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "vaultedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "vaultRefused", &mut out);
        crate::types::keys::opt_key::<String>(obj, "wakeRefused", &mut out);
        crate::types::keys::opt_key::<String>(obj, "diskGone", &mut out);
        crate::types::keys::opt_key::<WorkspaceAgents>(obj, "agents", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "rootThreadId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "parentWorkspaceId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "place", &mut out);
        crate::types::keys::opt_key::<String>(obj, "provider", &mut out);
        crate::types::keys::opt_key::<PauseMode>(obj, "pauseMode", &mut out);
        crate::types::keys::opt_key::<WorktreeFolder>(obj, "worktree", &mut out);
        crate::types::keys::opt_key::<WorkspaceFrom>(obj, "from", &mut out);
        crate::types::keys::opt_key::<ReviewDraft>(obj, "review", &mut out);
        crate::types::keys::req_key::<MachineState>(obj, "machineState", &mut out);
        crate::types::keys::req_key::<WorkspaceSize>(obj, "size", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "rateUsdPerHour", &mut out);
        crate::types::keys::opt_key::<String>(obj, "reason", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "idleAt", &mut out);
        crate::types::keys::opt_key::<MachineFacts>(obj, "facts", &mut out);
        crate::types::keys::req_key::<ReachView>(obj, "reach", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "machineId" => <String as Check>::describe(rest),
                "phase" => None,
                "kind" => <Option<WorkspaceKind> as Check>::describe(rest),
                "golden" => <String as Check>::describe(rest),
                "createdAt" => <String as Check>::describe(rest),
                "project" => <ProjectRef as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "home" => <Option<String> as Check>::describe(rest),
                "claudeSessionId" => <Option<String> as Check>::describe(rest),
                "gone" => <Option<String> as Check>::describe(rest),
                "theme" => <Option<WorkspaceTheme> as Check>::describe(rest),
                "glyph" => <Option<WorkspaceGlyph> as Check>::describe(rest),
                "daemonNote" => <Option<String> as Check>::describe(rest),
                "daemonRefusedAt" => <Option<WorkspaceViewDaemonRefusedAt> as Check>::describe(rest),
                "vaultedAt" => <Option<String> as Check>::describe(rest),
                "vaultRefused" => <Option<String> as Check>::describe(rest),
                "wakeRefused" => <Option<String> as Check>::describe(rest),
                "diskGone" => <Option<String> as Check>::describe(rest),
                "agents" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "parentThreadId" => <Option<String> as Check>::describe(rest),
                "rootThreadId" => <Option<String> as Check>::describe(rest),
                "parentWorkspaceId" => <Option<String> as Check>::describe(rest),
                "place" => <Option<String> as Check>::describe(rest),
                "provider" => <Option<String> as Check>::describe(rest),
                "pauseMode" => None,
                "worktree" => <Option<WorktreeFolder> as Check>::describe(rest),
                "from" => <Option<WorkspaceFrom> as Check>::describe(rest),
                "review" => <Option<ReviewDraft> as Check>::describe(rest),
                "machineState" => None,
                "size" => None,
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                "reason" => <Option<String> as Check>::describe(rest),
                "idleAt" => <Option<JsNumber> as Check>::describe(rest),
                "facts" => <Option<MachineFacts> as Check>::describe(rest),
                "reach" => None,
                _ => None,
            },
        }
    }
}
