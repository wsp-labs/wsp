// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/views/place.ts:341 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSetup {
    pub state: PlaceSetupState,
    pub add_id: String,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub finished_at: Option<String>,
    pub steps: Vec<PlaceSetupLine>,
    pub waiting: Vec<PlaceWait>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
}
impl Check for PlaceSetup {
    /// Nested checks: steps, waiting.
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        self.steps.check().map_err(|e| e.under("steps"))?;
        self.waiting.check().map_err(|e| e.under("waiting"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceSetupState>(obj, "state", &mut out);
        crate::types::keys::req_key::<String>(obj, "addId", &mut out);
        crate::types::keys::req_key::<String>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "finishedAt", &mut out);
        crate::types::keys::req_key::<Vec<PlaceSetupLine>>(obj, "steps", &mut out);
        crate::types::keys::req_key::<Vec<PlaceWait>>(obj, "waiting", &mut out);
        crate::types::keys::opt_key::<String>(obj, "said", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <PlaceSetupState as Check>::describe(rest),
                "addId" => <String as Check>::describe(rest),
                "startedAt" => <String as Check>::describe(rest),
                "finishedAt" => <Option<String> as Check>::describe(rest),
                "steps" => <Vec<PlaceSetupLine> as Check>::describe(rest),
                "waiting" => <Vec<PlaceWait> as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:355 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceApplied {
    pub hash: String,
    pub at: String,
    pub rows: Vec<PlaceProvisionRow>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub items: Option<IndexMap<String, String>>,
}
impl Check for PlaceApplied {
    /// Nested checks: rows.
    fn check(&self) -> Result<(), SchemaError> {
        self.rows.check().map_err(|e| e.under("rows"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "hash", &mut out);
        crate::types::keys::req_key::<String>(obj, "at", &mut out);
        crate::types::keys::req_key::<Vec<PlaceProvisionRow>>(obj, "rows", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "items", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "hash" => <String as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "rows" => <Vec<PlaceProvisionRow> as Check>::describe(rest),
                "items" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceSyncState {
    #[serde(rename = "behind")]
    Behind,
    #[serde(rename = "running")]
    Running,
}
impl Check for PlaceSyncState {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["behind", "running"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:368 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSync {
    pub state: PlaceSyncState,
    pub changes: Vec<String>,
    pub since: String,
}
impl Check for PlaceSync {
    fn check(&self) -> Result<(), SchemaError> {
        self.state.check().map_err(|e| e.under("state"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceSyncState>(obj, "state", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "changes", &mut out);
        crate::types::keys::req_key::<String>(obj, "since", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <PlaceSyncState as Check>::describe(rest),
                "changes" => <Vec<String> as Check>::describe(rest),
                "since" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:373 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SetupEnd {
    #[serde(rename = "ready")]
    Ready,
    #[serde(rename = "needs-you")]
    NeedsYou,
    #[serde(rename = "failed")]
    Failed,
}
impl Check for SetupEnd {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["ready", "needs-you", "failed"]))
    }
}

/// TS: packages/protocol/src/views/place.ts:379 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSetupEvent {
    pub add_id: String,
    pub place_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<PlaceSetupLine>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub wait: Option<PlaceWait>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub landed: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub running: Option<Vec<PlaceSetupStep>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub end: Option<SetupEnd>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
}
impl PlaceSetupEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.setup";
}
impl Check for PlaceSetupEvent {
    /// Nested checks: line, wait, running, end.
    fn check(&self) -> Result<(), SchemaError> {
        self.line.check().map_err(|e| e.under("line"))?;
        self.wait.check().map_err(|e| e.under("wait"))?;
        self.running.check().map_err(|e| e.under("running"))?;
        self.end.check().map_err(|e| e.under("end"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "addId", &mut out);
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::opt_key::<PlaceSetupLine>(obj, "line", &mut out);
        crate::types::keys::opt_key::<PlaceWait>(obj, "wait", &mut out);
        crate::types::keys::opt_key::<String>(obj, "landed", &mut out);
        crate::types::keys::opt_key::<Vec<PlaceSetupStep>>(obj, "running", &mut out);
        crate::types::keys::opt_key::<SetupEnd>(obj, "end", &mut out);
        crate::types::keys::opt_key::<String>(obj, "said", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "addId" => <String as Check>::describe(rest),
                "placeId" => <String as Check>::describe(rest),
                "line" => <Option<PlaceSetupLine> as Check>::describe(rest),
                "wait" => <Option<PlaceWait> as Check>::describe(rest),
                "landed" => <Option<String> as Check>::describe(rest),
                "running" => <Option<Vec<PlaceSetupStep>> as Check>::describe(rest),
                "end" => <Option<SetupEnd> as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:397 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSyncEvent {
    pub place_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sync: Option<PlaceSync>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub line: Option<PlaceSetupLine>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub applied: Option<PlaceApplied>,
}
impl PlaceSyncEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.sync";
}
impl Check for PlaceSyncEvent {
    /// Nested checks: sync, line, applied.
    fn check(&self) -> Result<(), SchemaError> {
        self.sync.check().map_err(|e| e.under("sync"))?;
        self.line.check().map_err(|e| e.under("line"))?;
        self.applied.check().map_err(|e| e.under("applied"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::opt_key::<PlaceSync>(obj, "sync", &mut out);
        crate::types::keys::opt_key::<PlaceSetupLine>(obj, "line", &mut out);
        crate::types::keys::opt_key::<PlaceApplied>(obj, "applied", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "sync" => <Option<PlaceSync> as Check>::describe(rest),
                "line" => <Option<PlaceSetupLine> as Check>::describe(rest),
                "applied" => <Option<PlaceApplied> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:402 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipesChangedEvent {
    pub slug: String,
}
impl RecipesChangedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "recipes.changed";
}
impl Check for RecipesChangedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "slug", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "slug" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// A zod union: the first variant that parses wins, in this order.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum AddLineStep {
    PlaceAddStep(PlaceAddStep),
    PlaceSetupStep(PlaceSetupStep),
}

crate::zod_union!(AddLineStep { PlaceAddStep(PlaceAddStep), PlaceSetupStep(PlaceSetupStep) }

);

impl Check for AddLineStep {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            AddLineStep::PlaceAddStep(v) => v.check(),
            AddLineStep::PlaceSetupStep(v) => v.check(),
        }
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<PlaceAddStep>(value),
            crate::types::check::issues_of::<PlaceSetupStep>(value),
        ]))
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Union);
        }
        <PlaceAddStep as Check>::describe(path).or_else(|| <PlaceSetupStep as Check>::describe(path))
    }
}

/// TS: packages/protocol/src/views/place.ts:406 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddLine {
    pub step: AddLineStep,
    pub state: PlaceSetupLineState,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ms: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub started_at: Option<String>,
}
impl Check for AddLine {
    /// Rules: ms: >= 0.
    /// Nested checks: step.
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        self.state.check().map_err(|e| e.under("state"))?;
        if let Some(v) = self.ms {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("ms"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<AddLineStep>(obj, "step", &mut out);
        crate::types::keys::req_key::<PlaceSetupLineState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "ms", &mut out, Self::rule_ms);
        crate::types::keys::opt_key::<String>(obj, "startedAt", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "step" => <AddLineStep as Check>::describe(rest),
                "state" => <PlaceSetupLineState as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "ms" => <Option<i64> as Check>::describe(rest),
                "startedAt" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl AddLine {
    fn rule_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:411 at a3b870a6e.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PendingStep {
    #[serde(rename = "connect")]
    Connect,
    #[serde(rename = "check")]
    Check,
    #[serde(rename = "wsp")]
    Wsp,
    #[serde(rename = "floor")]
    Floor,
    #[serde(rename = "choosing")]
    Choosing,
}
impl Check for PendingStep {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["connect", "check", "wsp", "floor", "choosing"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingComputerFailed {
    pub said: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub fix: Option<String>,
}
impl Check for PendingComputerFailed {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "said", &mut out);
        crate::types::keys::opt_key::<String>(obj, "fix", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "said" => <String as Check>::describe(rest),
                "fix" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:416 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingComputer {
    pub id: String,
    pub address: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ssh_port: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub place_id: Option<String>,
    pub step: PendingStep,
    pub choices: RecipeFile,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe: Option<String>,
    pub started_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub failed: Option<PendingComputerFailed>,
}
impl Check for PendingComputer {
    /// Nested checks: step, choices, failed.
    fn check(&self) -> Result<(), SchemaError> {
        self.step.check().map_err(|e| e.under("step"))?;
        self.choices.check().map_err(|e| e.under("choices"))?;
        self.failed.check().map_err(|e| e.under("failed"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "address", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "sshPort", &mut out);
        crate::types::keys::opt_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<PendingStep>(obj, "step", &mut out);
        crate::types::keys::req_key::<RecipeFile>(obj, "choices", &mut out);
        crate::types::keys::opt_key::<String>(obj, "recipe", &mut out);
        crate::types::keys::req_key::<String>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<PendingComputerFailed>(obj, "failed", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "address" => <String as Check>::describe(rest),
                "sshPort" => <Option<i64> as Check>::describe(rest),
                "name" => <Option<String> as Check>::describe(rest),
                "placeId" => <Option<String> as Check>::describe(rest),
                "step" => <PendingStep as Check>::describe(rest),
                "choices" => <RecipeFile as Check>::describe(rest),
                "recipe" => <Option<String> as Check>::describe(rest),
                "startedAt" => <String as Check>::describe(rest),
                "failed" => <Option<PendingComputerFailed> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// ---- Hand-written below: the functions of the unit.

// views/place.ts:114 at a3b870a6e
// export function withPlaceStage(job: PlaceAddJob, e: Pick<PlaceStageEvent, "step" | "state" | "note" | "placeId" | "ms">): PlaceAddJob {
//   const line = { step: e.step, state: e.state, ...(e.note !== undefined ? { note: e.note } : {}), ...(e.ms !== undefined ? { ms: e.ms } : {}) };
//   const at = job.steps.findIndex(s => s.step === e.step);
//   const steps = at === -1 ? [...job.steps, line] : job.steps.map((s, i) => (i === at ? line : s));
//   if (e.step === "join" && e.state === "done") return { ...job, steps, state: "done", ...(e.placeId !== undefined ? { placeId: e.placeId } : {}) };
//   if (e.state === "failed") return { ...job, steps, state: "failed", ...(job.said === undefined && e.note !== undefined ? { said: e.note } : {}) };
//   return { ...job, steps };
// }
/// The job with one stage line folded in: the step's line replaced where the job has one (in place), else appended;
/// a done `join` ends the job done (taking the event's place id where it has one); a failed line ends it failed, its
/// note becoming the job's `said` only where the job has none yet.
pub fn with_place_stage(job: &PlaceAddJob, e: &PlaceStageEvent) -> PlaceAddJob {
    let state = match e.state {
        PlaceStageEventState::Running => PlaceAddJobStepsItemState::Running,
        PlaceStageEventState::Done => PlaceAddJobStepsItemState::Done,
        PlaceStageEventState::Failed => PlaceAddJobStepsItemState::Failed,
    };
    let line = PlaceAddJobStepsItem { step: e.step, state, note: e.note.clone(), ms: e.ms };
    let mut steps = job.steps.clone();
    match steps.iter().position(|s| s.step == e.step) {
        Some(at) => steps[at] = line,
        None => steps.push(line),
    }
    let mut next = PlaceAddJob { steps, ..job.clone() };
    if e.step == PlaceAddStep::Join && e.state == PlaceStageEventState::Done {
        next.state = PlaceAddJobState::Done;
        if let Some(place_id) = &e.place_id {
            next.place_id = Some(place_id.clone());
        }
    } else if e.state == PlaceStageEventState::Failed {
        next.state = PlaceAddJobState::Failed;
        if let (None, Some(note)) = (&job.said, &e.note) {
            next.said = Some(note.clone());
        }
    }
    next
}

// views/place.ts:254 at a3b870a6e (private)
// const SIGN_IN_ROW = "signins/";
const SIGN_IN_ROW: &str = "signins/";

// views/place.ts:256 at a3b870a6e
// export const signInRowId = (agent: string): string => `${SIGN_IN_ROW}${agent}`;
pub fn sign_in_row_id(agent: &str) -> String {
    format!("{SIGN_IN_ROW}{agent}")
}

// views/place.ts:258 at a3b870a6e
// export const signInOfRow = (id: string): string | undefined => (id.startsWith(SIGN_IN_ROW) ? id.slice(SIGN_IN_ROW.length) : id === GITHUB_ROW ? GITHUB_CLI : undefined);
pub fn sign_in_of_row(id: &str) -> Option<String> {
    match id.strip_prefix(SIGN_IN_ROW) {
        Some(agent) => Some(agent.to_string()),
        None if id == GITHUB_ROW => Some(GITHUB_CLI.to_string()),
        None => None,
    }
}

// views/place.ts:261 at a3b870a6e
// export function provisionCounts(rows: readonly PlaceProvisionRow[]): Partial<Record<PlaceProvisionKind, number>> {
//   const counts: Partial<Record<PlaceProvisionKind, number>> = {};
//   for (const r of rows) {
//     const kind = r.kind ?? "tool";
//     counts[kind] = (counts[kind] ?? 0) + 1;
//   }
//   return counts;
// }
/// How many rows of each kind (an absent kind counts as `tool`), keys in the order each kind is first met.
pub fn provision_counts(rows: &[PlaceProvisionRow]) -> indexmap::IndexMap<PlaceProvisionKind, i64> {
    let mut counts = indexmap::IndexMap::new();
    for row in rows {
        *counts.entry(row.kind.unwrap_or(PlaceProvisionKind::Tool)).or_insert(0) += 1;
    }
    counts
}

// views/place.ts:502 at a3b870a6e
// export function macKindOf(said: string): MacKind | undefined {
//   const word = said.replace(/\s+/g, "").toLowerCase();
//   if (word.startsWith("macbook")) return "macbook";
//   if (word.startsWith("imac")) return "imac";
//   if (word.startsWith("macmini")) return "mac-mini";
//   if (word.startsWith("macstudio")) return "mac-studio";
//   if (word.startsWith("macpro")) return "mac-pro";
//   return undefined;
// }
/// Read with every whitespace character taken out and lower-cased.
pub fn mac_kind_of(said: &str) -> Option<MacKind> {
    let is_js_space = |c: char| (c.is_whitespace() && c != '\u{85}') || c == '\u{feff}';
    let word = said.chars().filter(|c| !is_js_space(*c)).collect::<String>().to_lowercase();
    if word.starts_with("macbook") {
        Some(MacKind::Macbook)
    } else if word.starts_with("imac") {
        Some(MacKind::Imac)
    } else if word.starts_with("macmini") {
        Some(MacKind::MacMini)
    } else if word.starts_with("macstudio") {
        Some(MacKind::MacStudio)
    } else if word.starts_with("macpro") {
        Some(MacKind::MacPro)
    } else {
        None
    }
}

// views/place.ts:462 at a3b870a6e
// export interface PlaceSetAlso {
//   name?: string | undefined;
//   ssh?: string | undefined;
//   recipe?: string | undefined;
// }
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSetAlso {
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub ssh: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe: Option<String>,
}

// views/place.ts:702 at a3b870a6e
// export const placeForgetsOnly = (holds: PlaceHolds): boolean => holds.away === true && (holds.forks.length > 0 || holds.projects.length > 0);
pub fn place_forgets_only(holds: &PlaceHolds) -> bool {
    holds.away.is_some() && (!holds.forks.is_empty() || !holds.projects.is_empty())
}
