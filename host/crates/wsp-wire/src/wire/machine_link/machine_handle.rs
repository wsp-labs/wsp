// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandle {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<MachineKind>(obj, "kind", &mut out);
        crate::types::keys::opt_key::<String>(obj, "streamUrl", &mut out);
        crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "labels", &mut out);
        crate::types::keys::opt_key::<MachineHandleSeen>(obj, "seen", &mut out);
        crate::types::keys::opt_key::<bool>(obj, "replayed", &mut out);
        crate::types::keys::opt_key::<DaemonSupervisor>(obj, "daemonSupervisor", &mut out);
        crate::types::keys::opt_key::<String>(obj, "notice", &mut out);
        crate::types::keys::req_key::<MachineHandleRoads>(obj, "roads", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "kind" => <MachineKind as Check>::describe(rest),
                "streamUrl" => <Option<String> as Check>::describe(rest),
                "labels" => <Option<IndexMap<String, String>> as Check>::describe(rest),
                "seen" => <Option<MachineHandleSeen> as Check>::describe(rest),
                "replayed" => <Option<bool> as Check>::describe(rest),
                "daemonSupervisor" => <Option<DaemonSupervisor> as Check>::describe(rest),
                "notice" => <Option<String> as Check>::describe(rest),
                "roads" => <MachineHandleRoads as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleSeen {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<MachineState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<String>(obj, "createdAt", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <MachineState as Check>::describe(rest),
                "createdAt" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleRoads {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<bool>(obj, "previewUrl", &mut out);
        crate::types::keys::req_key::<bool>(obj, "daemonAnswers", &mut out);
        crate::types::keys::req_key::<bool>(obj, "putBytes", &mut out);
        crate::types::keys::req_key::<bool>(obj, "describe", &mut out);
        crate::types::keys::req_key::<bool>(obj, "facts", &mut out);
        crate::types::keys::req_key::<bool>(obj, "metrics", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "previewUrl" => <bool as Check>::describe(rest),
                "daemonAnswers" => <bool as Check>::describe(rest),
                "putBytes" => <bool as Check>::describe(rest),
                "describe" => <bool as Check>::describe(rest),
                "facts" => <bool as Check>::describe(rest),
                "metrics" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

impl MachineLinkRequest {
    fn rule_cmd(x: &String) -> Result<(), SchemaError> {
        text_at_most(x, crate::wire::limits::EXEC_BODY_MAX as usize)
    }

    fn rule_timeout(x: &Option<JsInt>) -> Result<(), SchemaError> {
        x.as_ref().map_or(Ok(()), |v| num_above(v.as_f64(), 0.0))
    }

    fn rule_port(x: &JsInt) -> Result<(), SchemaError> {
        num_at_least(x.as_f64(), 1.0)?;
        num_at_most(x.as_f64(), 65535.0)
    }

    fn rule_seq(x: &JsInt) -> Result<(), SchemaError> {
        num_at_least(x.as_f64(), 0.0)
    }

    fn upload_id_issues(x: &str) -> Vec<SchemaError> {
        [
            text_at_least(x, 1),
            text_at_most(x, 32),
            regex_check(!x.is_empty() && x.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())),
        ]
        .into_iter()
        .filter_map(Result::err)
        .collect()
    }

    fn upload_id_key(obj: &crate::types::keys::Obj, out: &mut Vec<SchemaError>) {
        match obj.get("uploadId") {
            None => out.push(crate::types::check::required::<String>("uploadId")),
            Some(v) => match crate::types::check::read::<String>(v) {
                Ok(id) => out.extend(Self::upload_id_issues(&id).into_iter().map(|e| e.under("uploadId"))),
                Err(issues) => out.extend(issues.into_iter().map(|e| e.under("uploadId"))),
            },
        }
    }
}
/// No refinement of its own; checks every nested field that has one.
impl Check for MachineLinkRequest {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            MachineLinkRequest::MachineCreate { spec, .. } => spec.check().map_err(|e| e.under("spec"))?,
            MachineLinkRequest::MachineExec { cmd, timeout_ms, .. } => {
                Self::rule_cmd(cmd).map_err(|e| e.under("cmd"))?;
                Self::rule_timeout(timeout_ms).map_err(|e| e.under("timeoutMs"))?;
            }
            MachineLinkRequest::MachineDaemonAnswers { timeout_ms, .. } => {
                Self::rule_timeout(timeout_ms).map_err(|e| e.under("timeoutMs"))?;
            }
            MachineLinkRequest::MachinePreviewUrl { port, .. } => Self::rule_port(port).map_err(|e| e.under("port"))?,
            MachineLinkRequest::MachinePutBytes { upload_id, seq, timeout_ms, .. } => {
                if let Some(e) = Self::upload_id_issues(upload_id).into_iter().next() {
                    return Err(e.under("uploadId"));
                }
                Self::rule_seq(seq).map_err(|e| e.under("seq"))?;
                Self::rule_timeout(timeout_ms).map_err(|e| e.under("timeoutMs"))?;
            }
            _ => {}
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::tag_describe(path, "op", MACHINE_LINK_OPS)
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("op") else {
            return Some(vec![crate::types::check::missing::<Self>("op")]);
        };
        match tag.as_str() {
            Some("machine.backend") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.capacity") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.checkKey") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.create") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<MachineSpec>(obj, "spec", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "labels", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.exec") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                crate::types::keys::req_key_with::<String>(obj, "cmd", &mut out, Self::rule_cmd);
                crate::types::keys::opt_key_with::<JsInt>(obj, "timeoutMs", &mut out, Self::rule_timeout);
                crate::types::keys::opt_key::<String>(obj, "stdin", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.pause") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.resume") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.kill") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.state") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.describe") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.facts") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.metrics") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.daemonAnswers") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "timeoutMs", &mut out, Self::rule_timeout);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.previewUrl") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                crate::types::keys::req_key_with::<JsInt>(obj, "port", &mut out, Self::rule_port);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.downloadUrl") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.uploadUrl") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("machine.putBytes") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "machineId", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                Self::upload_id_key(obj, &mut out);
                crate::types::keys::req_key_with::<JsInt>(obj, "seq", &mut out, Self::rule_seq);
                crate::types::keys::req_key::<bool>(obj, "last", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                crate::types::keys::opt_key_with::<JsInt>(obj, "timeoutMs", &mut out, Self::rule_timeout);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("op", tag)]),
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineHandleReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.machine.check().map_err(|e| e.under("machine"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<MachineHandle>(obj, "machine", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "machine" => <MachineHandle as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineListReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.machines.check().map_err(|e| e.under("machines"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<MachineListRow>>(obj, "machines", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "machines" => <Vec<MachineListRow> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineExecReply {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ExecResult>(obj, "result", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "result" => <ExecResult as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineStateReply {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<MachineState>(obj, "state", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <MachineState as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineShapeReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.shape.check().map_err(|e| e.under("shape"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<MachineShape>(obj, "shape", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "shape" => <MachineShape as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `memBytes`: int min 0
/// - `cpuUsageUsec`: int min 0
/// - `uptimeMs`: int min 0
/// - `procs`: int min 0
/// - `quietForMs`: int min 0
impl Check for MachineReading {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.mem_bytes {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("memBytes"))?;
        }
        if let Some(v) = self.cpu_usage_usec {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("cpuUsageUsec"))?;
        }
        if let Some(v) = self.uptime_ms {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("uptimeMs"))?;
        }
        if let Some(v) = self.procs {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("procs"))?;
        }
        if let Some(v) = self.quiet_for_ms {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("quietForMs"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<MachineState>(obj, "state", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "cpu", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "memMb", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "memBytes", &mut out, Self::rule_mem_bytes);
        crate::types::keys::opt_key_with::<JsInt>(obj, "cpuUsageUsec", &mut out, Self::rule_cpu_usage_usec);
        crate::types::keys::opt_key_with::<JsInt>(obj, "uptimeMs", &mut out, Self::rule_uptime_ms);
        crate::types::keys::opt_key_with::<JsInt>(obj, "procs", &mut out, Self::rule_procs);
        crate::types::keys::opt_key_with::<JsInt>(obj, "quietForMs", &mut out, Self::rule_quiet_for_ms);
        crate::types::keys::opt_key::<String>(obj, "address", &mut out);
        crate::types::keys::req_key::<String>(obj, "cgroup", &mut out);
        crate::types::keys::req_key::<String>(obj, "upper", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "state" => <MachineState as Check>::describe(rest),
                "cpu" => <Option<JsNumber> as Check>::describe(rest),
                "memMb" => <Option<JsNumber> as Check>::describe(rest),
                "memBytes" => <Option<i64> as Check>::describe(rest),
                "cpuUsageUsec" => <Option<i64> as Check>::describe(rest),
                "uptimeMs" => <Option<i64> as Check>::describe(rest),
                "procs" => <Option<i64> as Check>::describe(rest),
                "quietForMs" => <Option<i64> as Check>::describe(rest),
                "address" => <Option<String> as Check>::describe(rest),
                "cgroup" => <String as Check>::describe(rest),
                "upper" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl MachineReading {
    fn rule_mem_bytes(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_cpu_usage_usec(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_uptime_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_procs(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_quiet_for_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineReadingReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.reading.check().map_err(|e| e.under("reading"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<MachineReading>(obj, "reading", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "reading" => <MachineReading as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineFactsReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.facts.check().map_err(|e| e.under("facts"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<MachineFacts>(obj, "facts", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "facts" => <MachineFacts as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineAnswersReply {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<bool>(obj, "answers", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "answers" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineReachReply {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PreviewReach>(obj, "reach", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "reach" => <PreviewReach as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for MachineUrlReply {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "url", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "url" => <String as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonOkResponse {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Option<ReqId>>(obj, "id", &mut out);
        crate::types::keys::req_key::<True>(obj, "ok", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "ok" => crate::types::check::leaf(rest, crate::types::check::Zod::Literal(|| Value::from(true))),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonErrorResponse {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Option<ReqId>>(obj, "id", &mut out);
        crate::types::keys::req_key::<False>(obj, "ok", &mut out);
        crate::types::keys::req_key::<String>(obj, "error", &mut out);
        crate::types::keys::opt_key::<DaemonErrorCode>(obj, "code", &mut out);
        crate::types::keys::opt_key::<MachineErrorKind>(obj, "kind", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "status", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "ok" => crate::types::check::leaf(rest, crate::types::check::Zod::Literal(|| Value::from(false))),
                "error" => <String as Check>::describe(rest),
                "code" => <Option<DaemonErrorCode> as Check>::describe(rest),
                "kind" => <Option<MachineErrorKind> as Check>::describe(rest),
                "status" => <Option<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonResponse {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            DaemonResponse::V0(v) => v.check(),
            DaemonResponse::V1(v) => v.check(),
        }
    }

    fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
        if path.is_empty() {
            return Some(crate::types::check::Zod::Union);
        }
        <DaemonOkResponse as Check>::describe(path).or_else(|| <DaemonErrorResponse as Check>::describe(path))
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<DaemonOkResponse>(value),
            crate::types::check::issues_of::<DaemonErrorResponse>(value),
        ]))
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "sys.sample"
impl Check for SysSample {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "cpu", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "load1", &mut out);
        crate::types::keys::req_key::<SysSampleMem>(obj, "mem", &mut out);
        crate::types::keys::req_key::<SysSampleDisk>(obj, "disk", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "at", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "cpu" => <JsNumber as Check>::describe(rest),
                "load1" => <JsNumber as Check>::describe(rest),
                "mem" => <SysSampleMem as Check>::describe(rest),
                "disk" => <SysSampleDisk as Check>::describe(rest),
                "at" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SysSampleMem {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "used", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "total", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "used" => <JsNumber as Check>::describe(rest),
                "total" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SysSampleDisk {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "used", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "total", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "used" => <JsNumber as Check>::describe(rest),
                "total" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ReadingsAnswer {
    fn check(&self) -> Result<(), SchemaError> {
        self.points.check().map_err(|e| e.under("points"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Vec<SysPoint>>(obj, "points", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "stepMs", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "from", &mut out);
        crate::types::keys::req_key::<JsNumber>(obj, "to", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "points" => <Vec<SysPoint> as Check>::describe(rest),
                "stepMs" => <JsNumber as Check>::describe(rest),
                "from" => <JsNumber as Check>::describe(rest),
                "to" => <JsNumber as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "workspace.sys"
impl Check for WorkspaceSysEvent {
    fn check(&self) -> Result<(), SchemaError> {
        self.sample.check().map_err(|e| e.under("sample"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<WorkspaceSysType>(obj, "type", &mut out);
        crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
        crate::types::keys::req_key::<SysSampleFrame>(obj, "sample", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "type" => <WorkspaceSysType as Check>::describe(rest),
                "workspaceId" => <String as Check>::describe(rest),
                "sample" => <SysSampleFrame as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "place.sys"
impl Check for PlaceSysEvent {
    fn check(&self) -> Result<(), SchemaError> {
        self.sample.check().map_err(|e| e.under("sample"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<PlaceSysType>(obj, "type", &mut out);
        crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
        crate::types::keys::req_key::<SysSampleFrame>(obj, "sample", &mut out);
        Some(out)
    }

    fn describe(path: &[Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "type" => <PlaceSysType as Check>::describe(rest),
                "placeId" => <String as Check>::describe(rest),
                "sample" => <SysSampleFrame as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `placedBy`: catch null: a value that does not parse reads as absent
impl Check for ProcContainer {
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<String>(obj, "name", &mut out);
        crate::types::keys::req_key::<String>(obj, "image", &mut out);
        crate::types::keys::req_key::<String>(obj, "state", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "startedAt", &mut out);
        crate::types::keys::opt_key::<String>(obj, "thread", &mut out);
        crate::types::keys::opt_key::<String>(obj, "folder", &mut out);
        crate::types::keys::opt_key::<String>(obj, "project", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "memory", &mut out);
        crate::types::keys::opt_key::<JsNumber>(obj, "cpu", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => <String as Check>::describe(rest),
                "name" => <String as Check>::describe(rest),
                "image" => <String as Check>::describe(rest),
                "state" => <String as Check>::describe(rest),
                "startedAt" => <Option<JsNumber> as Check>::describe(rest),
                "thread" => <Option<String> as Check>::describe(rest),
                "folder" => <Option<String> as Check>::describe(rest),
                "project" => <Option<String> as Check>::describe(rest),
                "memory" => <Option<JsNumber> as Check>::describe(rest),
                "cpu" => <Option<JsNumber> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "proc.snapshot"
impl Check for ProcSnapshot {
    fn check(&self) -> Result<(), SchemaError> {
        self.procs.check().map_err(|e| e.under("procs"))?;
        self.containers.check().map_err(|e| e.under("containers"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "at", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "daemon", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "total", &mut out);
        crate::types::keys::req_key::<Vec<ProcEntry>>(obj, "procs", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "seq", &mut out);
        crate::types::keys::opt_key::<ProcCost>(obj, "cost", &mut out);
        crate::types::keys::opt_key::<Vec<ProcContainer>>(obj, "containers", &mut out);
        crate::types::keys::opt_key::<Vec<CgroupMemory>>(obj, "cgroups", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <JsNumber as Check>::describe(rest),
                "daemon" => <i64 as Check>::describe(rest),
                "total" => <i64 as Check>::describe(rest),
                "procs" => <Vec<ProcEntry> as Check>::describe(rest),
                "seq" => <Option<i64> as Check>::describe(rest),
                "cost" => <Option<ProcCost> as Check>::describe(rest),
                "containers" => <Option<Vec<ProcContainer>> as Check>::describe(rest),
                "cgroups" => <Option<Vec<CgroupMemory>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `type`: literal "proc.changes"
impl Check for ProcChanges {
    fn check(&self) -> Result<(), SchemaError> {
        self.procs.check().map_err(|e| e.under("procs"))?;
        self.containers.check().map_err(|e| e.under("containers"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsNumber>(obj, "at", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "daemon", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "total", &mut out);
        crate::types::keys::req_key::<Vec<ProcEntry>>(obj, "procs", &mut out);
        crate::types::keys::req_key::<Vec<JsInt>>(obj, "gone", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "seq", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "base", &mut out);
        crate::types::keys::opt_key::<ProcCost>(obj, "cost", &mut out);
        crate::types::keys::opt_key::<Vec<ProcContainer>>(obj, "containers", &mut out);
        crate::types::keys::opt_key::<Vec<CgroupMemory>>(obj, "cgroups", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "at" => <JsNumber as Check>::describe(rest),
                "daemon" => <i64 as Check>::describe(rest),
                "total" => <i64 as Check>::describe(rest),
                "procs" => <Vec<ProcEntry> as Check>::describe(rest),
                "gone" => <Vec<i64> as Check>::describe(rest),
                "seq" => <i64 as Check>::describe(rest),
                "base" => <i64 as Check>::describe(rest),
                "cost" => <Option<ProcCost> as Check>::describe(rest),
                "containers" => <Option<Vec<ProcContainer>> as Check>::describe(rest),
                "cgroups" => <Option<Vec<CgroupMemory>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

// Refinements on the variants of the unions above, each an issue when it fails:
// - MachineLinkRequest::MachineExec.cmd: string max 16384
// - MachineLinkRequest::MachineExec.timeoutMs: int min 0 exclusive
// - MachineLinkRequest::MachineDaemonAnswers.timeoutMs: int min 0 exclusive
// - MachineLinkRequest::MachinePreviewUrl.port: int min 1
// - MachineLinkRequest::MachinePreviewUrl.port: int max 65535
// - MachineLinkRequest::MachinePutBytes.uploadId: string min 1
// - MachineLinkRequest::MachinePutBytes.uploadId: string max 32
// - MachineLinkRequest::MachinePutBytes.uploadId: string regex /^[a-z0-9]+$/
// - MachineLinkRequest::MachinePutBytes.seq: int min 0
// - MachineLinkRequest::MachinePutBytes.timeoutMs: int min 0 exclusive

// machine-link.ts:292
// /** What a fork is refused with when the place it lands on answers that it holds no copy of the snapshot named. A
//  * fork builds the copy it needs only of the image's current version, so this is an older version or a project's
//  * image named at a place it was never built at. */
// export const placeHoldsNoImageLine = (place: string, image: string): string =>
//   `${place} holds no copy of ${image}; a fork there builds a copy first only of your image's current version`;
pub fn place_holds_no_image_line(place: &str, image: &str) -> String {
    format!("{place} holds no copy of {image}; a fork there builds a copy first only of your image's current version")
}

// machine-link.ts:298
// /** What a remove of a computer is refused with while a fork or a project there holds work no remote has, each named
//  * with what it holds, a project folder by its checkout's path there: the roads that keep it, and the flag that takes
//  * it anyway. */
// export const placeUnsavedRefusal = (place: string, unsaved: readonly string[]): { said: string; fix: string } => ({
//   said: `${place} holds work no remote has, which a remove would lose: ${unsaved.join("; ")}`,
//   fix: `Keep a fork's work with wsp export or by pushing its branch, and copy a project folder's off ${place} from the path named; then remove ${place} again, or wsp remove ${place} --force removes it anyway.`,
// });
pub fn place_unsaved_refusal(place: &str, unsaved: &[String]) -> SaidFix {
    SaidFix {
        said: format!("{place} holds work no remote has, which a remove would lose: {}", unsaved.join("; ")),
        fix: format!("Keep a fork's work with wsp export or by pushing its branch, and copy a project folder's off {place} from the path named; then remove {place} again, or wsp remove {place} --force removes it anyway."),
    }
}

// machine-link.ts:306
// /** What a remove of a computer is refused with while a fork or a project stands on it and its link is down: the forks
//  * are deleted and the project folders read over that link, so nothing about them can be done or known until it is
//  * back. A computer that will never answer again is forgotten instead. */
// export const placeAwayRefusal = (place: string, sentence: string): { said: string; fix: string } => ({
//   said: `${sentence}, and its forks and projects go over its link`,
//   fix: `Turn ${place} on and remove it again once it answers; if it never will, wsp remove ${place} --forget takes it, its forks, projects and threads out of this wsp, with nothing done on it beyond a try at wsp's own leave over ssh.`,
// });
pub fn place_away_refusal(place: &str, sentence: &str) -> SaidFix {
    SaidFix {
        said: format!("{sentence}, and its forks and projects go over its link"),
        fix: format!("Turn {place} on and remove it again once it answers; if it never will, wsp remove {place} --forget takes it, its forks, projects and threads out of this wsp, with nothing done on it beyond a try at wsp's own leave over ssh."),
    }
}

// machine-link.ts:314
// /** What a forget of a computer is refused with while its link answers: a remove takes wsp off it. */
// export const placeForgetAnswersRefusal = (place: string): { said: string; fix: string } => ({
//   said: `${place} is answering, so there is nothing to forget`,
//   fix: `wsp remove ${place} takes it out and takes wsp off it.`,
// });
pub fn place_forget_answers_refusal(place: &str) -> SaidFix {
    SaidFix {
        said: format!("{place} is answering, so there is nothing to forget"),
        fix: format!("wsp remove {place} takes it out and takes wsp off it."),
    }
}

// machine-link.ts:320
// /** What a remove that stopped part way through the forks and projects on a computer says: why, and which of them had
//  * already gone, since those are not coming back. */
// export const placeDropStoppedLine = (why: string, went: readonly string[]): string => (went.length === 0 ? why : `${why}; ${went.join(", ")} had already gone with the remove`);
pub fn place_drop_stopped_line(why: &str, went: &[String]) -> String {
    if went.is_empty() {
        return why.to_string();
    }
    format!("{why}; {} had already gone with the remove", went.join(", "))
}

// machine-link.ts:324
// /** What a remove of a project is refused with while its checkout on a computer of the person's holds work no remote
//  * has, named by its path there. */
// export const projectUnsavedRefusal = (project: string, unsaved: string): { said: string; fix: string } => ({
//   said: `${unsaved}, which removing ${project} would lose`,
//   fix: `Copy that work off its computer from the path named or push its branches, then remove ${project} again; wsp projects remove ${project} --force removes it anyway.`,
// });
pub fn project_unsaved_refusal(project: &str, unsaved: &str) -> SaidFix {
    SaidFix {
        said: format!("{unsaved}, which removing {project} would lose"),
        fix: format!("Copy that work off its computer from the path named or push its branches, then remove {project} again; wsp projects remove {project} --force removes it anyway."),
    }
}

// machine-link.ts:331
// /** How an unsaved read names a project folder: by its project and the checkout's path on the computer holding it,
//  * which is where a person goes to keep what is there. */
// export const projectFolderNamed = (project: string, checkout: string): string => `${project} at ${checkout}`;
pub fn project_folder_named(project: &str, checkout: &str) -> String {
    format!("{project} at {checkout}")
}

// machine-link.ts:363
// /** What a fork aimed at a place this host no longer holds a record for is refused with. Every computer on the
//  * list forks, so the only way to reach this is a record that went between the word being read and the fork being
//  * asked for: a remove, or a store another process wrote. */
// export const placeForksNowhereLine = (place: string): string => `${place} is no longer a place in this wsp, so nothing forks there`;
pub fn place_forks_nowhere_line(place: &str) -> String {
    format!("{place} is no longer a place in this wsp, so nothing forks there")
}

// machine-link.ts:368
// /** What a verb aimed at the bare computer a place is, rather than at a workspace forked on it, is refused with. A
//  * place holds its facts and its forks; the forks are the workspaces, so the refusal names the computer and the one
//  * road to a workspace there. */
// export const placeNotAWorkspaceLine = (place: string): string => `${place} is a computer you joined, not a workspace; its forks are the workspaces`;
pub fn place_not_a_workspace_line(place: &str) -> String {
    format!("{place} is a computer you joined, not a workspace; its forks are the workspaces")
}

// machine-link.ts:372
// export const placeNotAWorkspaceFix = (place: string): string => `Name a machine on it: wsp threads lists what runs on ${place}.`;
pub fn place_not_a_workspace_fix(place: &str) -> String {
    format!("Name a machine on it: wsp threads lists what runs on {place}.")
}

// machine-link.ts:374
// /** What a caller asking for the road to a workspace's own daemon is told, where that workspace runs none: a
//  * workspace on a computer somebody owns is that computer's directories under the computer's own daemon, and that
//  * daemon answers its files and its git through the host. Said rather than a route minted to a port nothing listens
//  * on, which is what the panes and the relay read before. */
// export const placeServesDaemonLine = (workspace: string, computer: string): string =>
//   `${workspace} has no daemon of its own: ${computer} answers its files and git through this host`;
pub fn place_serves_daemon_line(workspace: &str, computer: &str) -> String {
    format!("{workspace} has no daemon of its own: {computer} answers its files and git through this host")
}

// machine-link.ts:381
// /** What a watch of the ports or of the load is refused with on a workspace whose computer answers its daemon
//  * frames: both readings are that whole computer's, and one workspace reading them would read another workspace's
//  * listeners and load as its own. A pane asks for both on every link it opens and takes a refusal of either. */
// export const placeWatchesItselfLine = (computer: string): string =>
//   `${computer} watches its own ports and load, which are that computer's rather than one workspace's`;
pub fn place_watches_itself_line(computer: &str) -> String {
    format!("{computer} watches its own ports and load, which are that computer's rather than one workspace's")
}

// machine-link.ts:387
// /** What a watch, read or signal of processes is refused with on a workspace whose computer answers its daemon
//  * frames: that daemon acts on any pid on the computer, the computer's own and every other workspace's, so none of
//  * them goes up its link from one workspace's channel. The Processes pane shows it in place of the table. */
// export const forkProcsUnreadLine = (workspace: string, computer: string): string =>
//   `${workspace}'s processes on ${computer} are not readable from here yet`;
pub fn fork_procs_unread_line(workspace: &str, computer: &str) -> String {
    format!("{workspace}'s processes on {computer} are not readable from here yet")
}

// machine-link.ts:393
// /** What every other op is refused with on that workspace's channel: the frames that computer's daemon answers
//  * inside the workspace they name are its shells, its files and its git, and every other op there runs on the
//  * computer itself. */
// export const forkOpRefusedLine = (op: string, workspace: string, computer: string): string =>
//   `${computer} answers ${workspace}'s shells, files and git from here, not ${op}`;
pub fn fork_op_refused_line(op: &str, workspace: &str, computer: &str) -> String {
    format!("{computer} answers {workspace}'s shells, files and git from here, not {op}")
}
