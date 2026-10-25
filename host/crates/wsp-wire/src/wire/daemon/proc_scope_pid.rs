// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `pid`: int min 0
impl Check for ProcScopePid {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.pid.as_f64(), 0.0).map_err(|e| e.under("pid"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "pid", &mut out, Self::rule_pid);
        crate::types::keys::req_key::<ProcKind>(obj, "kind", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "kind" => <ProcKind as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl ProcScopePid {
    fn rule_pid(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcInspectReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.cwd.check().map_err(|e| e.under("cwd"))?;
        self.ports.check().map_err(|e| e.under("ports"))?;
        self.threads.check().map_err(|e| e.under("threads"))?;
        self.children.check().map_err(|e| e.under("children"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "pid", &mut out);
        crate::types::keys::req_key::<Option<String>>(obj, "cwd", &mut out);
        crate::types::keys::req_key::<Vec<JsInt>>(obj, "ports", &mut out);
        crate::types::keys::opt_key::<JsInt>(obj, "threads", &mut out);
        crate::types::keys::req_key::<Vec<JsInt>>(obj, "children", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "pid" => <i64 as Check>::describe(rest),
                "cwd" => <Option<String> as Check>::describe(rest),
                "ports" => <Vec<i64> as Check>::describe(rest),
                "threads" => <Option<i64> as Check>::describe(rest),
                "children" => <Vec<i64> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ProcKillReply {
    fn check(&self) -> Result<(), SchemaError> {
        self.signalled.check().map_err(|e| e.under("signalled"))?;
        self.forced.check().map_err(|e| e.under("forced"))?;
        self.left.check().map_err(|e| e.under("left"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<Vec<JsInt>>(obj, "signalled", &mut out);
        crate::types::keys::opt_key::<Vec<JsInt>>(obj, "forced", &mut out);
        crate::types::keys::opt_key::<Vec<JsInt>>(obj, "left", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "signalled" => <Option<Vec<i64>> as Check>::describe(rest),
                "forced" => <Option<Vec<i64>> as Check>::describe(rest),
                "left" => <Option<Vec<i64>> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `op`: literal "exec"
/// - `cmd`: string max 16384
/// - `timeoutMs`: int min 0 exclusive
/// - `timeoutMs`: int max 600000
impl Check for DaemonExecRequest {
    fn check(&self) -> Result<(), SchemaError> {
        text_at_most(&self.cmd, EXEC_BODY_MAX as usize).map_err(|e| e.under("cmd"))?;
        if let Some(timeout) = self.timeout_ms {
            num_above(timeout.as_f64(), 0.0).map_err(|e| e.under("timeoutMs"))?;
            num_at_most(timeout.as_f64(), EXEC_TIMEOUT_MAX_MS as f64).map_err(|e| e.under("timeoutMs"))?;
        }
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
        crate::types::keys::req_key::<ExecLit>(obj, "op", &mut out);
        crate::types::keys::req_key_with::<String>(obj, "cmd", &mut out, Self::rule_cmd);
        crate::types::keys::opt_key_with::<JsInt>(obj, "timeoutMs", &mut out, Self::rule_timeout_ms);
        crate::types::keys::opt_key::<String>(obj, "stdin", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "id" => crate::types::check::leaf(rest, crate::types::check::Zod::Union),
                "op" => <ExecLit as Check>::describe(rest),
                "cmd" => <String as Check>::describe(rest),
                "timeoutMs" => <Option<JsInt> as Check>::describe(rest),
                "stdin" => <Option<String> as Check>::describe(rest),
                _ => None,
            },
        }
    }
}
impl DaemonExecRequest {
    fn rule_cmd(x: &String) -> Result<(), crate::types::check::SchemaError> {
        text_at_most(x, EXEC_BODY_MAX as usize)?;
        Ok(())
    }
    fn rule_timeout_ms(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(timeout) = *x {
            num_above(timeout.as_f64(), 0.0)?;
            num_at_most(timeout.as_f64(), EXEC_TIMEOUT_MAX_MS as f64)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for DaemonExecReply {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "exitCode", &mut out);
        crate::types::keys::req_key::<String>(obj, "stdout", &mut out);
        crate::types::keys::req_key::<String>(obj, "stderr", &mut out);
        crate::types::keys::req_key::<bool>(obj, "truncated", &mut out);
        Some(out)
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        match path.split_first() {
            None => Some(crate::types::check::Zod::Word("object")),
            Some((key, rest)) => match key.as_str()? {
                "exitCode" => <i64 as Check>::describe(rest),
                "stdout" => <String as Check>::describe(rest),
                "stderr" => <String as Check>::describe(rest),
                "truncated" => <bool as Check>::describe(rest),
                _ => None,
            },
        }
    }
}

impl Check for DaemonRequest {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            DaemonRequest::Exec(exec) => exec.check()?,
            DaemonRequest::TunnelOpen { port, .. } => {
                num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
                num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
            }
            DaemonRequest::StdioOpen { cmd, .. } => {
                text_at_most(cmd, EXEC_BODY_MAX as usize).map_err(|e| e.under("cmd"))?;
                text_at_least(cmd, 1).map_err(|e| e.under("cmd"))?;
            }
            DaemonRequest::HttpPost { url, headers, body, timeout_ms, .. } => {
                refine(is_http_url(url), "http or https URL").map_err(|e| e.under("url"))?;
                for (key, value) in headers {
                    text_at_most(value, 8192).map_err(|e| e.under(key.as_str()).under("headers"))?;
                }
                refine(headers.keys().filter(|k| *k != "__proto__").count() <= 64, "at most 64 headers").map_err(|e| e.under("headers"))?;
                text_at_most(body, EXEC_OUTPUT_MAX as usize).map_err(|e| e.under("body"))?;
                if let Some(v) = timeout_ms {
                    num_above(v.as_f64(), 0.0).map_err(|e| e.under("timeoutMs"))?;
                    num_at_most(v.as_f64(), EXEC_TIMEOUT_MAX_MS as f64).map_err(|e| e.under("timeoutMs"))?;
                }
            }
            DaemonRequest::PortsWatch { roots, cgroups, watch, .. } => {
                for (i, root) in roots.iter().flatten().enumerate() {
                    num_at_least(root.as_f64(), 0.0).map_err(|e| e.under(i).under("roots"))?;
                }
                check_cgroups(cgroups.as_deref())?;
                check_watch(watch.as_deref())?;
            }
            DaemonRequest::ProcWatch { cgroups, .. } => check_cgroups(cgroups.as_deref())?,
            DaemonRequest::ProcScopes { scopes, .. } => {
                items_at_most(scopes.len(), PROC_SCOPES_MAX as usize).map_err(|e| e.under("scopes"))?;
                scopes.check().map_err(|e| e.under("scopes"))?;
            }
            DaemonRequest::ProcKill { pid, identity, protect, apps, agents, agent_bins, .. } => {
                num_above(pid.as_f64(), 0.0).map_err(|e| e.under("pid"))?;
                if let Some(identity) = identity {
                    text_at_most(identity, 32).map_err(|e| e.under("identity"))?;
                }
                for (key, list) in [("protect", protect), ("apps", apps), ("agents", agents)] {
                    if let Some(list) = list {
                        items_at_most(list.len(), KILL_PIDS_MAX as usize).map_err(|e| e.under(key))?;
                        for (i, pid) in list.iter().enumerate() {
                            num_at_least(pid.as_f64(), 0.0).map_err(|e| e.under(i).under(key))?;
                        }
                    }
                }
                if let Some(bins) = agent_bins {
                    items_at_most(bins.len(), 64).map_err(|e| e.under("agentBins"))?;
                    for (i, bin) in bins.iter().enumerate() {
                        text_at_most(bin, 255).map_err(|e| e.under(i).under("agentBins"))?;
                    }
                }
            }
            DaemonRequest::ContainerStop { container, .. } => {
                regex_check(hex_64(container)).map_err(|e| e.under("container"))?;
            }
            DaemonRequest::UsageLogs { stores, .. } => {
                items_at_most(stores.len(), USAGE_STORES_MAX as usize).map_err(|e| e.under("stores"))?;
            }
            DaemonRequest::TranscriptsList { root, dirs, cwds, .. } => {
                check_transcript_root(root)?;
                check_transcript_dirs(dirs)?;
                items_at_most(cwds.len(), TRANSCRIPT_DIRS_MAX as usize).map_err(|e| e.under("cwds"))?;
                for (i, cwd) in cwds.iter().enumerate() {
                    text_at_most(cwd, GUEST_CWD_MAX as usize).map_err(|e| e.under(i).under("cwds"))?;
                }
            }
            DaemonRequest::TranscriptsRead { root, dirs, session, last, tail, .. } => {
                check_transcript_root(root)?;
                check_transcript_dirs(dirs)?;
                text_at_least(session, 1).map_err(|e| e.under("session"))?;
                text_at_most(session, 64).map_err(|e| e.under("session"))?;
                num_above(last.as_f64(), 0.0).map_err(|e| e.under("last"))?;
                if let Some(tail) = tail {
                    num_above(tail.as_f64(), 0.0).map_err(|e| e.under("tail"))?;
                }
            }
            DaemonRequest::GuestOpen { token, turn_token, argv, cwd, .. } => {
                text_at_most(token, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("token"))?;
                if let Some(turn) = turn_token {
                    text_at_most(turn, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("turnToken"))?;
                }
                items_at_most(argv.len(), GUEST_ARGV_MAX as usize).map_err(|e| e.under("argv"))?;
                text_at_most(cwd, GUEST_CWD_MAX as usize).map_err(|e| e.under("cwd"))?;
            }
            DaemonRequest::GitPrReply { number, reply_to, body, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
                if let Some(reply_to) = reply_to {
                    num_at_least(reply_to.as_f64(), 0.0).map_err(|e| e.under("replyTo"))?;
                }
                text_at_most(body, 65536).map_err(|e| e.under("body"))?;
            }
            DaemonRequest::SysHistory { step_ms, .. } => {
                num_at_least(step_ms.as_f64(), 0.0).map_err(|e| e.under("stepMs"))?;
            }
            DaemonRequest::ProcInspect { pid, .. } => {
                num_above(pid.as_f64(), 0.0).map_err(|e| e.under("pid"))?;
            }
            DaemonRequest::GitPrRead { number: Some(number), .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
            }
            DaemonRequest::GitPrView { number, .. }
            | DaemonRequest::GitPrMerge { number, .. }
            | DaemonRequest::GitIssueRead { number, .. }
            | DaemonRequest::GitPrResolve { number, .. }
            | DaemonRequest::GitPrReact { number, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
            }
            DaemonRequest::GitRunLog { run_id, job_id, .. } => {
                num_at_least(run_id.as_f64(), 0.0).map_err(|e| e.under("runId"))?;
                num_at_least(job_id.as_f64(), 0.0).map_err(|e| e.under("jobId"))?;
            }
            DaemonRequest::GitPrDiff { number, max_bytes, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
                if let Some(max_bytes) = max_bytes {
                    num_above(max_bytes.as_f64(), 0.0).map_err(|e| e.under("maxBytes"))?;
                }
            }
            DaemonRequest::GitPrCheckout { number, token, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
                if let Some(token) = token {
                    text_at_most(token, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("token"))?;
                }
            }
            DaemonRequest::SshStart { authorized_key, .. } => {
                text_at_least(authorized_key, 1).map_err(|e| e.under("authorizedKey"))?;
                text_at_most(authorized_key, 1024).map_err(|e| e.under("authorizedKey"))?;
            }
            DaemonRequest::GitPush { token, .. }
            | DaemonRequest::GitPr { token, .. }
            | DaemonRequest::GitUpdate { token, .. }
            | DaemonRequest::GitStartOn { token, .. }
            | DaemonRequest::GitMergeIn { token, .. } => {
                if let Some(token) = token {
                    text_at_most(token, GUEST_TOKEN_MAX as usize).map_err(|e| e.under("token"))?;
                }
            }
            DaemonRequest::GitPrReview { number, comments, .. } => {
                num_at_least(number.as_f64(), 0.0).map_err(|e| e.under("number"))?;
                comments.check().map_err(|e| e.under("comments"))?;
            }
            DaemonRequest::PlaceUpdate { upload_id, seq, sha256, .. } => {
                text_at_least(upload_id, 1).map_err(|e| e.under("uploadId"))?;
                text_at_most(upload_id, 32).map_err(|e| e.under("uploadId"))?;
                regex_check(!upload_id.is_empty() && upload_id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit()))
                    .map_err(|e| e.under("uploadId"))?;
                num_at_least(seq.as_f64(), 0.0).map_err(|e| e.under("seq"))?;
                regex_check(hex_64(sha256)).map_err(|e| e.under("sha256"))?;
            }
            _ => {}
        }
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::tag_describe(path, "op", DAEMON_REQUEST_OPS)
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("op") else {
            return Some(vec![crate::types::check::missing::<Self>("op")]);
        };
        use crate::wire::daemon_checks as dc;
        match tag.as_str() {
            Some("pty.create") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "cols", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "rows", &mut out);
                crate::types::keys::opt_key::<String>(obj, "shell", &mut out);
                crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "env", &mut out);
                crate::types::keys::opt_key::<String>(obj, "run", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.attach") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.detach") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.resize") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::req_key::<JsNumber>(obj, "cols", &mut out);
                crate::types::keys::req_key::<JsNumber>(obj, "rows", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.kill") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("ports.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::opt_list::<JsInt>(obj, "roots", &mut out, None, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "folder", &mut out);
                dc::opt_list::<String>(obj, "cgroups", &mut out, Some(16), dc::cgroup);
                dc::opt_rules::<String>(obj, "watch", &mut out, dc::watch_name);
                Some(out)
            }
            Some("manifest.get") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("manifest.record") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cmd", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<JsNumber>(obj, "port", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("manifest.restartScript") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("inbox.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("inbox.rescan") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.unwatch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("sys.history") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<JsInt>(obj, "from", &mut out);
                crate::types::keys::req_key::<JsInt>(obj, "to", &mut out);
                dc::req_rules::<JsInt>(obj, "stepMs", &mut out, dc::nonneg);
                Some(out)
            }
            Some("usage.logs") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_list::<UsageStore>(obj, "stores", &mut out, Some(USAGE_STORES_MAX as usize), dc::none);
                Some(out)
            }
            Some("transcripts.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "root", &mut out, dc::root_path);
                dc::req_list::<String>(obj, "dirs", &mut out, Some(TRANSCRIPT_DIRS_MAX as usize), dc::name_255);
                dc::req_list::<String>(obj, "cwds", &mut out, Some(TRANSCRIPT_DIRS_MAX as usize), dc::path_4096);
                Some(out)
            }
            Some("transcripts.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "root", &mut out, dc::root_path);
                dc::req_list::<String>(obj, "dirs", &mut out, Some(TRANSCRIPT_DIRS_MAX as usize), dc::name_255);
                dc::req_rules::<String>(obj, "session", &mut out, dc::session);
                dc::req_rules::<JsInt>(obj, "last", &mut out, dc::positive);
                dc::opt_rules::<JsInt>(obj, "tail", &mut out, dc::positive);
                Some(out)
            }
            Some("proc.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::opt_list::<String>(obj, "cgroups", &mut out, Some(16), dc::cgroup);
                Some(out)
            }
            Some("proc.unwatch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("proc.scopes") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_list::<ProcScope>(obj, "scopes", &mut out, Some(PROC_SCOPES_MAX as usize), dc::none);
                Some(out)
            }
            Some("proc.inspect") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<JsInt>(obj, "pid", &mut out, dc::positive);
                Some(out)
            }
            Some("proc.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("proc.kill") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<JsInt>(obj, "pid", &mut out, dc::positive);
                crate::types::keys::req_key::<ProcSignal>(obj, "signal", &mut out);
                crate::types::keys::opt_key::<KillMode>(obj, "mode", &mut out);
                crate::types::keys::opt_key::<JsInt>(obj, "startedAt", &mut out);
                dc::opt_rules::<String>(obj, "identity", &mut out, dc::identity);
                dc::opt_list::<JsInt>(obj, "protect", &mut out, Some(KILL_PIDS_MAX as usize), dc::nonneg);
                dc::opt_list::<JsInt>(obj, "apps", &mut out, Some(KILL_PIDS_MAX as usize), dc::nonneg);
                dc::opt_list::<JsInt>(obj, "agents", &mut out, Some(KILL_PIDS_MAX as usize), dc::nonneg);
                dc::opt_list::<String>(obj, "agentBins", &mut out, Some(64), dc::name_255);
                Some(out)
            }
            Some("container.stop") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "container", &mut out, dc::container);
                Some(out)
            }
            Some("ping") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.list") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "gitignore", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.files") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.read") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<FsReadEncoding>(obj, "encoding", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.image") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.hash") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "root", &mut out);
                crate::types::keys::req_key::<Vec<String>>(obj, "paths", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::req_key::<String>(obj, "contents", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("pty.tab") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "ptyId", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.search") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::req_key::<String>(obj, "query", &mut out);
                crate::types::keys::req_key::<FsSearchMode>(obj, "mode", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.status") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.diff") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<GitDiffScope>(obj, "scope", &mut out);
                crate::types::keys::opt_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "paths", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "whole", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.discard") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.commit") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "message", &mut out);
                crate::types::keys::req_key::<Vec<String>>(obj, "paths", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.snapshot") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.range") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "from", &mut out);
                crate::types::keys::req_key::<String>(obj, "to", &mut out);
                crate::types::keys::opt_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.turn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "from", &mut out);
                crate::types::keys::req_key::<String>(obj, "to", &mut out);
                crate::types::keys::opt_key::<String>(obj, "path", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.push") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "base", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.pr") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "base", &mut out);
                crate::types::keys::opt_key::<String>(obj, "title", &mut out);
                crate::types::keys::opt_key::<String>(obj, "body", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prRead") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
                dc::opt_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "seen", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prView") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.runLog") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "runId", &mut out, dc::nonneg);
                dc::req_rules::<JsInt>(obj, "jobId", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prMerge") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::req_key::<MergeMethod>(obj, "method", &mut out);
                crate::types::keys::req_key::<bool>(obj, "auto", &mut out);
                crate::types::keys::req_key::<String>(obj, "headOid", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.issueRead") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prCheckout") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prDiff") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                dc::opt_rules::<JsInt>(obj, "maxBytes", &mut out, dc::positive);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prReview") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::req_key::<String>(obj, "headOid", &mut out);
                crate::types::keys::req_key::<GitPrReviewEvent>(obj, "event", &mut out);
                crate::types::keys::req_key::<String>(obj, "body", &mut out);
                crate::types::keys::req_key::<Vec<GitPrReviewComment>>(obj, "comments", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prReply") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                dc::opt_rules::<JsInt>(obj, "replyTo", &mut out, dc::nonneg);
                crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
                dc::req_rules::<String>(obj, "body", &mut out, dc::reply_body);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prResolve") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
                crate::types::keys::req_key::<bool>(obj, "resolved", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prReact") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                dc::req_rules::<JsInt>(obj, "number", &mut out, dc::nonneg);
                crate::types::keys::req_key::<String>(obj, "subject", &mut out);
                crate::types::keys::req_key::<ReactionContent>(obj, "content", &mut out);
                crate::types::keys::req_key::<bool>(obj, "on", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.repoRead") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.update") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "base", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.startOn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.branchCompare") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                crate::types::keys::req_key::<String>(obj, "base", &mut out);
                crate::types::keys::req_key::<String>(obj, "head", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.mergeIn") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                crate::types::keys::opt_key::<String>(obj, "from", &mut out);
                dc::opt_rules::<String>(obj, "token", &mut out, dc::token);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("git.prList") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.checkpoint") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "thread", &mut out);
                crate::types::keys::req_key::<String>(obj, "turn", &mut out);
                crate::types::keys::opt_key::<String>(obj, "scope", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.restore") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "checkpoint", &mut out);
                crate::types::keys::opt_key::<String>(obj, "scope", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.checkpointDrop") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "scope", &mut out);
                crate::types::keys::req_key::<String>(obj, "thread", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.worktrees") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.branches") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.switchNew") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("git.fetchBranch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "cwd", &mut out);
                crate::types::keys::req_key::<String>(obj, "remote", &mut out);
                crate::types::keys::req_key::<String>(obj, "branch", &mut out);
                crate::types::keys::opt_key::<String>(obj, "into", &mut out);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("fs.folders") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::opt_key::<String>(obj, "dir", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "hidden", &mut out);
                crate::types::keys::opt_key::<bool>(obj, "repos", &mut out);
                crate::types::keys::opt_key::<Vec<String>>(obj, "projects", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("tunnel.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "tunnelId", &mut out);
                dc::req_rules::<JsInt>(obj, "port", &mut out, dc::port);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("ssh.start") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "authorizedKey", &mut out, dc::authorized_key);
                crate::types::keys::opt_key::<String>(obj, "machineId", &mut out);
                Some(out)
            }
            Some("tunnel.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "tunnelId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("tunnel.close") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "tunnelId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                dc::req_rules::<String>(obj, "cmd", &mut out, dc::stdio_cmd);
                crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "env", &mut out);
                Some(out)
            }
            Some("stdio.write") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("stdio.close") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "stdioId", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("http.post") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "url", &mut out, dc::http_url);
                dc::req_rules::<IndexMap<String, String>>(obj, "headers", &mut out, dc::headers);
                dc::req_rules::<String>(obj, "body", &mut out, dc::http_body);
                crate::types::keys::opt_key::<HttpBearer>(obj, "bearer", &mut out);
                dc::opt_rules::<JsInt>(obj, "timeoutMs", &mut out, dc::timeout);
                Some(out)
            }
            Some("exec") => <DaemonExecRequest as Check>::pass(value),
            Some("place.leave") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("guest.open") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<GuestKind>(obj, "kind", &mut out);
                dc::req_rules::<String>(obj, "token", &mut out, dc::token);
                dc::opt_rules::<String>(obj, "turnToken", &mut out, dc::token);
                dc::req_list::<String>(obj, "argv", &mut out, Some(GUEST_ARGV_MAX as usize), dc::none);
                dc::req_rules::<String>(obj, "cwd", &mut out, dc::guest_cwd);
                Some(out)
            }
            Some("guest.send") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::any_key::<Value>(obj, "message", &mut out);
                Some(out)
            }
            Some("guest.watch") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("guest.reply") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "session", &mut out);
                crate::types::keys::any_key::<Value>(obj, "message", &mut out);
                Some(out)
            }
            Some("guest.close") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                crate::types::keys::req_key::<String>(obj, "session", &mut out);
                crate::types::keys::opt_key::<String>(obj, "error", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("place.update") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
                dc::req_rules::<String>(obj, "uploadId", &mut out, dc::upload_id);
                dc::req_rules::<JsInt>(obj, "seq", &mut out, dc::nonneg);
                crate::types::keys::req_key::<bool>(obj, "last", &mut out);
                crate::types::keys::req_key::<String>(obj, "data", &mut out);
                dc::req_rules::<String>(obj, "sha256", &mut out, dc::sha256);
                Some(out)
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("op", tag)]),
        }
    }
}

fn check_cgroups(cgroups: Option<&[String]>) -> Result<(), SchemaError> {
    let Some(cgroups) = cgroups else { return Ok(()) };
    items_at_most(cgroups.len(), 16).map_err(|e| e.under("cgroups"))?;
    for (i, path) in cgroups.iter().enumerate() {
        text_at_most(path, CGROUP_PATH_MAX).map_err(|e| e.under(i).under("cgroups"))?;
        refine(is_cgroup_path(path), "Invalid input").map_err(|e| e.under(i).under("cgroups"))?;
    }
    Ok(())
}

fn check_watch(watch: Option<&str>) -> Result<(), SchemaError> {
    match watch {
        Some(name) => regex_check(is_watch_name(name)).map_err(|e| e.under("watch")),
        None => Ok(()),
    }
}

fn check_transcript_root(root: &str) -> Result<(), SchemaError> {
    text_at_least(root, 1).map_err(|e| e.under("root"))?;
    text_at_most(root, 4096).map_err(|e| e.under("root"))
}

fn check_transcript_dirs(dirs: &[String]) -> Result<(), SchemaError> {
    items_at_most(dirs.len(), TRANSCRIPT_DIRS_MAX as usize).map_err(|e| e.under("dirs"))?;
    for (i, dir) in dirs.iter().enumerate() {
        text_at_most(dir, 255).map_err(|e| e.under(i).under("dirs"))?;
    }
    Ok(())
}
