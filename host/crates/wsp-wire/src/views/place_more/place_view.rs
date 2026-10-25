// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// TS: packages/protocol/src/views/place.ts:513 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceView {
    pub id: String,
    pub kind: PlaceKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub mac: Option<MacKind>,
    pub default: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub os: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub shape: Option<WorkspaceSize>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub disk_free_bytes: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub engine: Option<PlaceViewEngine>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub copies: Option<CopyWord>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub present: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub takes_runtime: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub joined_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub last_seen_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon_version: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub behind: Option<PlaceViewBehind>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub agents: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub agent_versions: Option<IndexMap<String, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::types::kept::record_opt",
        serialize_with = "crate::types::check::js_record_opt"
    )]
    pub sign_ins: Option<IndexMap<String, AgentSignInState>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub logins: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub rate_usd_per_hour: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sizes: Option<Vec<MachineSizeOffer>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub forks: Option<PlaceViewForks>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub takes_forks: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub road: Option<PlaceRoad>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub home: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub uptime_ms: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub reported_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub dialled: Option<PlaceDialled>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub blocked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub tools_blocked: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub build: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub build_stopped: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub builds_images: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub setup: Option<PlaceSetup>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub applied: Option<PlaceApplied>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub picks: Option<RecipeFile>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub recipe: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub sync: Option<PlaceSync>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cap: Option<PlaceCap>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub cap_default: Option<PlaceCap>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub settings: Option<PlaceSettings>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub nap_ms: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub nap_default: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_ms: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", with = "crate::types::check::double_option")]
    pub turn_limit_default: Option<Option<JsInt>>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spawn_default: Option<WorkspaceAgents>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub running: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub plan: Option<CloudPlan>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub starts: Option<CloudStarts>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size: Option<PlaceViewSize>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size_default: Option<PlaceViewSizeDefault>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub delete_after_ms: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub credit_hours: Option<JsNumber>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub spent_today_usd: Option<JsNumber>,
}
impl Check for PlaceView {
    /// Rules: uptimeMs: >= 0; running: >= 0.
    /// Nested checks: kind, mac, shape, copies, behind, signIns, sizes, forks, road, dialled, setup, applied, picks, sync, cap, capDefault, settings, spawn, spawnDefault, plan, starts, size, sizeDefault.
    fn check(&self) -> Result<(), SchemaError> {
        self.kind.check().map_err(|e| e.under("kind"))?;
        self.mac.check().map_err(|e| e.under("mac"))?;
        self.engine.check().map_err(|e| e.under("engine"))?;
        self.behind.check().map_err(|e| e.under("behind"))?;
        self.sign_ins.check().map_err(|e| e.under("signIns"))?;
        self.sizes.check().map_err(|e| e.under("sizes"))?;
        self.forks.check().map_err(|e| e.under("forks"))?;
        if let Some(v) = self.uptime_ms {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("uptimeMs"))?;
        }
        self.picks.check().map_err(|e| e.under("picks"))?;
        self.cap.check().map_err(|e| e.under("cap"))?;
        self.cap_default.check().map_err(|e| e.under("capDefault"))?;
        self.settings.check().map_err(|e| e.under("settings"))?;
        self.spawn.check().map_err(|e| e.under("spawn"))?;
        self.spawn_default.check().map_err(|e| e.under("spawnDefault"))?;
        if let Some(v) = self.running {
            crate::types::check::num_at_least(v.0 .0, 0.0).map_err(|e| e.under("running"))?;
        }
        self.plan.check().map_err(|e| e.under("plan"))?;
        self.starts.check().map_err(|e| e.under("starts"))?;
        self.size.check().map_err(|e| e.under("size"))?;
        self.size_default.check().map_err(|e| e.under("sizeDefault"))?;
        self.shape.check().map_err(|e| e.under("shape"))?;
        self.copies.check().map_err(|e| e.under("copies"))?;
        self.road.check().map_err(|e| e.under("road"))?;
        self.dialled.check().map_err(|e| e.under("dialled"))?;
        self.setup.check().map_err(|e| e.under("setup"))?;
        self.applied.check().map_err(|e| e.under("applied"))?;
        self.sync.check().map_err(|e| e.under("sync"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<PlaceKind>(obj, "kind", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::opt_key::<String>(obj, "label", &mut out);
        crate::types::keys::opt_key::<MacKind>(obj, "mac", &mut out);
        crate::types::keys::req_key::<bool>(obj, "default", &mut out);
        crate::types::keys::opt_key::<String>(obj, "os", &mut out);
        crate::types::keys::opt_key::<WorkspaceSize>(obj, "shape", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "diskFreeBytes", &mut out);
        crate::types::keys::opt_key::<PlaceViewEngine>(obj, "engine", &mut out);
        crate::types::keys::opt_key::<CopyWord>(obj, "copies", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "present", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "takesRuntime", &mut out);
        crate::types::keys::opt_key::<String>(obj, "joinedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "lastSeenAt", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "daemonVersion", &mut out);
        crate::types::keys::opt_key::<PlaceViewBehind>(obj, "behind", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "agents", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "agentVersions", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, AgentSignInState>>(obj, "signIns", &mut out);
        crate::types::keys::opt_key::<String>(obj, "logins", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "rateUsdPerHour", &mut out);
        crate::types::keys::opt_key::<Vec<MachineSizeOffer>>(obj, "sizes", &mut out);
        crate::types::keys::opt_key::<PlaceViewForks>(obj, "forks", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "takesForks", &mut out);
        crate::types::keys::opt_key::<PlaceRoad>(obj, "road", &mut out);
        crate::types::keys::opt_key::<String>(obj, "home", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "uptimeMs", &mut out, Self::rule_uptime_ms);
        crate::types::keys::opt_key::<String>(obj, "reportedAt", &mut out);
        crate::types::keys::opt_key::<PlaceDialled>(obj, "dialled", &mut out);
        crate::types::keys::opt_key::<String>(obj, "blocked", &mut out);
        crate::types::keys::opt_key::<String>(obj, "toolsBlocked", &mut out);
        crate::types::keys::opt_key::<String>(obj, "build", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "buildStopped", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "buildsImages", &mut out);
        crate::types::keys::opt_key::<PlaceSetup>(obj, "setup", &mut out);
        crate::types::keys::opt_key::<PlaceApplied>(obj, "applied", &mut out);
        crate::types::keys::opt_key::<RecipeFile>(obj, "picks", &mut out);
        crate::types::keys::opt_key::<String>(obj, "recipe", &mut out);
        crate::types::keys::opt_key::<PlaceSync>(obj, "sync", &mut out);
        crate::types::keys::opt_key::<PlaceCap>(obj, "cap", &mut out);
        crate::types::keys::opt_key::<PlaceCap>(obj, "capDefault", &mut out);
        crate::types::keys::opt_key::<PlaceSettings>(obj, "settings", &mut out);
        crate::types::keys::dbl_key::<JsInt>(obj, "napMs", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "napDefault", &mut out);
        crate::types::keys::dbl_key::<JsInt>(obj, "turnLimitMs", &mut out);
        crate::types::keys::dbl_key::<JsInt>(obj, "turnLimitDefault", &mut out);
        crate::types::keys::opt_key::<WorkspaceAgents>(obj, "spawn", &mut out);
        crate::types::keys::opt_key::<WorkspaceAgents>(obj, "spawnDefault", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "running", &mut out, Self::rule_running);
        crate::types::keys::opt_key::<CloudPlan>(obj, "plan", &mut out);
        crate::types::keys::opt_key::<CloudStarts>(obj, "starts", &mut out);
        crate::types::keys::opt_key::<PlaceViewSize>(obj, "size", &mut out);
        crate::types::keys::opt_key::<PlaceViewSizeDefault>(obj, "sizeDefault", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "deleteAfterMs", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "creditHours", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "spentTodayUsd", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "kind" => <PlaceKind as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "label" => <Option<String> as Check>::describe(rest),
                "mac" => <Option<MacKind> as Check>::describe(rest),
                "default" => <bool as Check>::describe(rest),
                "os" => <Option<String> as Check>::describe(rest),
                "shape" => None,
                "diskFreeBytes" => <Option<i64> as Check>::describe(rest),
                "engine" => <Option<PlaceViewEngine> as Check>::describe(rest),
                "copies" => None,
                "present" => <Option<bool> as Check>::describe(rest),
                "takesRuntime" => <Option<bool> as Check>::describe(rest),
                "joinedAt" => <Option<String> as Check>::describe(rest),
                "lastSeenAt" => <Option<String> as Check>::describe(rest),
                "daemonVersion" => <Option<i64> as Check>::describe(rest),
                "behind" => <Option<PlaceViewBehind> as Check>::describe(rest),
                "agents" => <Option<Vec<String>> as Check>::describe(rest),
                "agentVersions" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "signIns" => <Option<IndexMap<String, AgentSignInState>> as Check>::describe(rest),
                "logins" => <Option<String> as Check>::describe(rest),
                "rateUsdPerHour" => <Option<JsNumber> as Check>::describe(rest),
                "sizes" => <Option<Vec<MachineSizeOffer>> as Check>::describe(rest),
                "forks" => <Option<PlaceViewForks> as Check>::describe(rest),
                "takesForks" => <Option<bool> as Check>::describe(rest),
                "road" => None,
                "home" => <Option<String> as Check>::describe(rest),
                "uptimeMs" => <Option<i64> as Check>::describe(rest),
                "reportedAt" => <Option<String> as Check>::describe(rest),
                "dialled" => None,
                "blocked" => <Option<String> as Check>::describe(rest),
                "toolsBlocked" => <Option<String> as Check>::describe(rest),
                "build" => <Option<String> as Check>::describe(rest),
                "buildStopped" => <Option<bool> as Check>::describe(rest),
                "buildsImages" => <Option<bool> as Check>::describe(rest),
                "setup" => None,
                "applied" => None,
                "picks" => <Option<RecipeFile> as Check>::describe(rest),
                "recipe" => <Option<String> as Check>::describe(rest),
                "sync" => None,
                "cap" => <Option<PlaceCap> as Check>::describe(rest),
                "capDefault" => <Option<PlaceCap> as Check>::describe(rest),
                "settings" => <Option<PlaceSettings> as Check>::describe(rest),
                "napMs" => <Option<Option<i64>> as Check>::describe(rest),
                "napDefault" => <Option<i64> as Check>::describe(rest),
                "turnLimitMs" => <Option<Option<i64>> as Check>::describe(rest),
                "turnLimitDefault" => <Option<Option<i64>> as Check>::describe(rest),
                "spawn" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "spawnDefault" => <Option<WorkspaceAgents> as Check>::describe(rest),
                "running" => <Option<i64> as Check>::describe(rest),
                "plan" => <Option<CloudPlan> as Check>::describe(rest),
                "starts" => <Option<CloudStarts> as Check>::describe(rest),
                "size" => <Option<PlaceViewSize> as Check>::describe(rest),
                "sizeDefault" => <Option<PlaceViewSizeDefault> as Check>::describe(rest),
                "deleteAfterMs" => <Option<i64> as Check>::describe(rest),
                "creditHours" => <Option<JsNumber> as Check>::describe(rest),
                "spentTodayUsd" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceView {
    fn rule_uptime_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
    fn rule_running(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0 .0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:662 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceDial {
    pub dialled: PlaceDialled,
    pub line: String,
    pub place: PlaceView,
}
impl Check for PlaceDial {
    /// Nested checks: dialled, place.
    fn check(&self) -> Result<(), SchemaError> {
        self.place.check().map_err(|e| e.under("place"))?;
        self.dialled.check().map_err(|e| e.under("dialled"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceDialled>(obj, "dialled", &mut out);
        crate::types::keys::req_key::<String>(obj, "line", &mut out);
        crate::types::keys::req_key::<PlaceView>(obj, "place", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "dialled" => None,
                "line" => <String as Check>::describe(rest),
                "place" => <PlaceView as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PlaceUpdateReplyDaemonRoad {
    #[serde(rename = "link")]
    Link,
    #[serde(rename = "ssh")]
    Ssh,
}
impl Check for PlaceUpdateReplyDaemonRoad {
    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::leaf(path, crate::types::check::Zod::Options(&["link", "ssh"]))
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceUpdateReplyDaemon {
    pub from: JsInt,
    pub to: JsInt,
    pub road: PlaceUpdateReplyDaemonRoad,
    pub at: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub kept: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
}
impl Check for PlaceUpdateReplyDaemon {
    fn check(&self) -> Result<(), SchemaError> {
        self.road.check().map_err(|e| e.under("road"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "from", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "to", &mut out);
        crate::types::keys::req_key::<PlaceUpdateReplyDaemonRoad>(obj, "road", &mut out);
        crate::types::keys::req_key::<String>(obj, "at", &mut out);
        crate::types::keys::opt_key::<String>(obj, "kept", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "from" => <i64 as Check>::describe(rest),
                "to" => <i64 as Check>::describe(rest),
                "road" => <PlaceUpdateReplyDaemonRoad as Check>::describe(rest),
                "at" => <String as Check>::describe(rest),
                "kept" => <Option<String> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:671 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceUpdateReply {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub daemon: Option<PlaceUpdateReplyDaemon>,
}
impl Check for PlaceUpdateReply {
    /// Rules: name: at least 1 UTF-16 units.
    /// Nested checks: daemon.
    fn check(&self) -> Result<(), SchemaError> {
        crate::types::check::text_at_least(&self.name, 1).map_err(|e| e.under("name"))?;
        self.daemon.check().map_err(|e| e.under("daemon"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<String>(obj, "name", &mut out, Self::rule_name);
        crate::types::keys::opt_key::<PlaceUpdateReplyDaemon>(obj, "daemon", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "daemon" => <Option<PlaceUpdateReplyDaemon> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceUpdateReply {
    fn rule_name(x: &String) -> Result<(), crate::types::check::SchemaError> {
        crate::types::check::text_at_least(x, 1)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHoldsForksItem {
    pub name: String,
    pub threads: JsInt,
}
impl Check for PlaceHoldsForksItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "threads", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "name" => <String as Check>::describe(rest),
                "threads" => <i64 as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHoldsImagesItem {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub size_bytes: Option<JsNumber>,
}
impl Check for PlaceHoldsImagesItem {
    /// Rules: sizeBytes: >= 0.
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.size_bytes {
            crate::types::check::num_at_least(v.0, 0.0).map_err(|e| e.under("sizeBytes"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::opt_key_with::<JsNumber>(obj, "sizeBytes", &mut out, Self::rule_size_bytes);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "sizeBytes" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl PlaceHoldsImagesItem {
    fn rule_size_bytes(x: &Option<JsNumber>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            crate::types::check::num_at_least(v.0, 0.0)?;
        }
        Ok(())
    }
}

/// TS: packages/protocol/src/views/place.ts:691 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceHolds {
    pub forks: Vec<PlaceHoldsForksItem>,
    pub projects: Vec<PlaceHoldsForksItem>,
    pub unsaved: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub away: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub images: Option<Vec<PlaceHoldsImagesItem>>,
}
impl Check for PlaceHolds {
    /// Nested checks: forks, images.
    fn check(&self) -> Result<(), SchemaError> {
        self.forks.check().map_err(|e| e.under("forks"))?;
        self.projects.check().map_err(|e| e.under("projects"))?;
        self.images.check().map_err(|e| e.under("images"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<PlaceHoldsForksItem>>(obj, "forks", &mut out);
        crate::types::keys::req_key::<Vec<PlaceHoldsForksItem>>(obj, "projects", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "unsaved", &mut out);
        crate::types::keys::opt_key::<True>(obj, "away", &mut out);
        crate::types::keys::opt_key::<Vec<PlaceHoldsImagesItem>>(obj, "images", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "forks" => <Vec<PlaceHoldsForksItem> as Check>::describe(rest),
                "projects" => <Vec<PlaceHoldsForksItem> as Check>::describe(rest),
                "unsaved" => <Vec<String> as Check>::describe(rest),
                "away" => None,
                "images" => <Option<Vec<PlaceHoldsImagesItem>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRemovedTook {
    pub forks: Vec<PlaceHoldsForksItem>,
    pub projects: Vec<PlaceHoldsForksItem>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub away: Option<True>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub images: Option<Vec<PlaceHoldsImagesItem>>,
}
impl Check for PlaceRemovedTook {
    fn check(&self) -> Result<(), SchemaError> {
        self.forks.check().map_err(|e| e.under("forks"))?;
        self.projects.check().map_err(|e| e.under("projects"))?;
        self.images.check().map_err(|e| e.under("images"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<PlaceHoldsForksItem>>(obj, "forks", &mut out);
        crate::types::keys::req_key::<Vec<PlaceHoldsForksItem>>(obj, "projects", &mut out);
        crate::types::keys::opt_key::<True>(obj, "away", &mut out);
        crate::types::keys::opt_key::<Vec<PlaceHoldsImagesItem>>(obj, "images", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "forks" => <Vec<PlaceHoldsForksItem> as Check>::describe(rest),
                "projects" => <Vec<PlaceHoldsForksItem> as Check>::describe(rest),
                "away" => None,
                "images" => <Option<Vec<PlaceHoldsImagesItem>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:709 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRemoved {
    pub removed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub took: Option<PlaceRemovedTook>,
    pub swept: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub note: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub deleted: Option<JsInt>,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub snapshots: Option<JsInt>,
}
impl Check for PlaceRemoved {
    /// Nested checks: took.
    fn check(&self) -> Result<(), SchemaError> {
        self.took.check().map_err(|e| e.under("took"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<bool>(obj, "removed", &mut out);
        crate::types::keys::opt_key::<PlaceRemovedTook>(obj, "took", &mut out);
        crate::types::keys::req_key::<Vec<String>>(obj, "swept", &mut out);
        crate::types::keys::opt_key::<String>(obj, "note", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "deleted", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "snapshots", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "removed" => <bool as Check>::describe(rest),
                "took" => <Option<PlaceRemovedTook> as Check>::describe(rest),
                "swept" => <Vec<String> as Check>::describe(rest),
                "note" => <Option<String> as Check>::describe(rest),
                "deleted" => <Option<i64> as Check>::describe(rest),
                "snapshots" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:717 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceSpend {
    pub place: String,
    pub today_usd: JsNumber,
    pub month_usd: JsNumber,
    pub rate_usd_per_hour: JsNumber,
}
impl Check for PlaceSpend {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "place", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "todayUsd", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "monthUsd", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "rateUsdPerHour", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "place" => <String as Check>::describe(rest),
                "todayUsd" => <JsNumber as Check>::describe(rest),
                "monthUsd" => <JsNumber as Check>::describe(rest),
                "rateUsdPerHour" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:722 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceJoinedEvent {
    pub place: PlaceView,
    pub from: String,
}
impl PlaceJoinedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.joined";
}
impl Check for PlaceJoinedEvent {
    /// Nested checks: place.
    fn check(&self) -> Result<(), SchemaError> {
        self.place.check().map_err(|e| e.under("place"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceView>(obj, "place", &mut out);
        crate::types::keys::req_key::<String>(obj, "from", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "place" => <PlaceView as Check>::describe(rest),
                "from" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:723 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacePresentEvent {
    pub place_id: String,
    pub from: String,
}
impl PlacePresentEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.present";
}
impl Check for PlacePresentEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<String>(obj, "from", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "from" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:725 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceAbsentEvent {
    pub place_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub said: Option<String>,
}
impl PlaceAbsentEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.absent";
}
impl Check for PlaceAbsentEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "said", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                "said" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:726 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceRemovedEvent {
    pub place_id: String,
}
impl PlaceRemovedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.removed";
}
impl Check for PlaceRemovedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "placeId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:728 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlaceChangedEvent {
    pub place: PlaceView,
}
impl PlaceChangedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.changed";
}
impl Check for PlaceChangedEvent {
    /// Nested checks: place.
    fn check(&self) -> Result<(), SchemaError> {
        self.place.check().map_err(|e| e.under("place"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceView>(obj, "place", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "place" => <PlaceView as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:731 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlacePendingEvent {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub pending: Option<PendingComputer>,
}
impl PlacePendingEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "place.pending";
}
impl Check for PlacePendingEvent {
    /// Nested checks: pending.
    fn check(&self) -> Result<(), SchemaError> {
        self.pending.check().map_err(|e| e.under("pending"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::opt_key::<PendingComputer>(obj, "pending", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "pending" => None,
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:738 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAddedEvent {
    pub project: ProjectView,
}
impl ProjectAddedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.added";
}
impl Check for ProjectAddedEvent {
    /// Nested checks: project.
    fn check(&self) -> Result<(), SchemaError> {
        self.project.check().map_err(|e| e.under("project"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ProjectView>(obj, "project", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "project" => <ProjectView as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:743 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectAddEvent {
    pub project_id: String,
    pub computer: String,
    pub stage: ProjectAddStage,
    pub message: String,
    pub elapsed_ms: JsNumber,
}
impl ProjectAddEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.add";
}
impl Check for ProjectAddEvent {
    /// Nested checks: stage.
    fn check(&self) -> Result<(), SchemaError> {
        self.stage.check().map_err(|e| e.under("stage"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "projectId", &mut out);
        crate::types::keys::req_key::<String>(obj, "computer", &mut out);
        crate::types::keys::req_key::<ProjectAddStage>(obj, "stage", &mut out);
        crate::types::keys::req_key::<String>(obj, "message", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "elapsedMs", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "projectId" => <String as Check>::describe(rest),
                "computer" => <String as Check>::describe(rest),
                "stage" => <ProjectAddStage as Check>::describe(rest),
                "message" => <String as Check>::describe(rest),
                "elapsedMs" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:754 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HostNoticeEvent {
    pub message: String,
}
impl HostNoticeEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "host.notice";
}
impl Check for HostNoticeEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "message", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "message" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:757 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AsideTextEvent {
    pub workspace_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none", deserialize_with = "crate::types::check::present")]
    pub thread_id: Option<String>,
    pub ask_id: String,
    pub text: String,
}
impl AsideTextEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "aside.text";
}
impl Check for AsideTextEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
        crate::types::keys::req_key::<String>(obj, "askId", &mut out);
        crate::types::keys::req_key::<String>(obj, "text", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "workspaceId" => <String as Check>::describe(rest),
                "threadId" => <Option<String> as Check>::describe(rest),
                "askId" => <String as Check>::describe(rest),
                "text" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// TS: packages/protocol/src/views/place.ts:761 at a3b870a6e.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRemovedEvent {
    pub project_id: String,
}
impl ProjectRemovedEvent {
    /// The `type` key's value: the enum holding this struct writes and reads it as its tag.
    pub const TYPE: &'static str = "project.removed";
}
impl Check for ProjectRemovedEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "projectId", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "projectId" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
