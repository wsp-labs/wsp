// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

pub(super) fn runtime_op_pass_b(
    tag: Option<&str>,
    obj: &serde_json::Map<String, serde_json::Value>,
    value: &serde_json::Value,
) -> Option<Vec<crate::types::check::SchemaError>> {
    let _ = (obj, value);
    match tag {
        Some("sessions.replaceable") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("sessions.warm") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "project", &mut out);
            crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
            crate::types::keys::opt_key::<String>(obj, "harness", &mut out);
            crate::types::keys::opt_key::<String>(obj, "model", &mut out);
            crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
            crate::types::keys::opt_key::<String>(obj, "permissionMode", &mut out);
            crate::types::keys::opt_key::<AccessChoice>(obj, "access", &mut out);
            crate::types::keys::opt_key::<String>(obj, "contextWindow", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "fast", &mut out);
            Some(out)
        }
        Some("sessions.settle") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<Vec<String>>(obj, "threadIds", &mut out, |x| items_at_least(x.len(), 1));
            crate::types::keys::opt_key::<bool>(obj, "finished", &mut out);
            crate::types::keys::opt_key::<ProcessesAsk>(obj, "processes", &mut out);
            Some(out)
        }
        Some("sessions.mark") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<Vec<String>>(obj, "threadIds", &mut out, |x| items_at_least(x.len(), 1));
            crate::types::keys::req_key::<ThreadMarks>(obj, "marks", &mut out);
            Some(out)
        }
        Some("sessions.restore") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<Vec<String>>(obj, "threadIds", &mut out, |x| items_at_least(x.len(), 1));
            Some(out)
        }
        Some("sessions.search") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "query", &mut out);
            Some(out)
        }
        Some("sessions.aside") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
            crate::types::keys::req_key::<String>(obj, "question", &mut out);
            crate::types::keys::opt_key::<String>(obj, "askId", &mut out);
            Some(out)
        }
        Some("sessions.run") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "runId", &mut out);
            crate::types::keys::req_key::<String>(obj, "block", &mut out);
            crate::types::keys::req_key::<String>(obj, "command", &mut out);
            crate::types::keys::req_key::<RunState>(obj, "state", &mut out);
            crate::types::keys::opt_key::<String>(obj, "ptyId", &mut out);
            crate::types::keys::opt_key::<JsInt>(obj, "exitCode", &mut out);
            crate::types::keys::opt_key::<JsInt>(obj, "signal", &mut out);
            crate::types::keys::opt_key::<String>(obj, "output", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<String>(obj, "turnId", &mut out);
            Some(out)
        }
        Some("sessions.rewind") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "turnId", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "files", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "undo", &mut out);
            Some(out)
        }
        Some("golden.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            Some(out)
        }
        Some("capabilities.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("golden.prepare") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<MachineKind>(obj, "kind", &mut out);
            Some(out)
        }
        Some("golden.seal") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "builderId", &mut out);
            Some(out)
        }
        Some("snapshots.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "name", &mut out);
            Some(out)
        }
        Some("snapshots.storage") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("cost.history") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("cost.spend") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("usage.used") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<UsageRange>(obj, "range", &mut out);
            crate::types::keys::req_key::<UsageSplit>(obj, "split", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "outside", &mut out);
            Some(out)
        }
        Some("usage.accounts") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "fresh", &mut out);
            Some(out)
        }
        Some("usage.reset") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "account", &mut out);
            crate::types::keys::opt_key::<String>(obj, "creditId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "on", &mut out);
            Some(out)
        }
        Some("places.readings") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<ReadingsRange>(obj, "range", &mut out);
            Some(out)
        }
        Some("snapshots.rollback") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<JsNumber>(obj, "version", &mut out);
            crate::types::keys::opt_key::<String>(obj, "name", &mut out);
            Some(out)
        }
        Some("workspaces.portReach") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key_with::<JsInt>(obj, "port", &mut out, |x| {
                num_at_least(x.as_f64(), 1.0).and_then(|()| num_at_most(x.as_f64(), 65535.0))
            });
            Some(out)
        }
        Some("workspaces.portProbe") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key_with::<JsInt>(obj, "port", &mut out, |x| {
                num_at_least(x.as_f64(), 1.0).and_then(|()| num_at_most(x.as_f64(), 65535.0))
            });
            Some(out)
        }
        Some("workspaces.rebuild") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("forwards.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("forwards.stop") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<RelayPort>(obj, "port", &mut out);
            Some(out)
        }
        Some("workspaces.exec") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key_with::<Vec<String>>(obj, "argv", &mut out, |x| items_at_least(x.len(), 1));
            crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
            Some(out)
        }
        Some("host.folders") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "dir", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "hidden", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "repos", &mut out);
            crate::types::keys::opt_key::<String>(obj, "on", &mut out);
            Some(out)
        }
        Some("host.terminalConfig") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<TerminalScheme>(obj, "scheme", &mut out);
            Some(out)
        }
        Some("editor.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("editor.open") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "path", &mut out);
            crate::types::keys::opt_key_with::<JsInt>(obj, "line", &mut out, |x| opt_rule(x, |v| num_above(v.as_f64(), 0.0)));
            crate::types::keys::opt_key::<EditorId>(obj, "editor", &mut out);
            Some(out)
        }
        Some("ssh.port") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("ssh.include") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "on", &mut out);
            Some(out)
        }
        Some("agents.read") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            Some(out)
        }
        Some("servers.tools") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "refresh", &mut out);
            Some(out)
        }
        Some("servers.icon") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "host", &mut out, |x| text_at_least(x, 1).and_then(|()| text_at_most(x, 260)));
            crate::types::keys::opt_key::<bool>(obj, "refresh", &mut out);
            Some(out)
        }
        Some("agents.signIn") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "terminal", &mut out);
            Some(out)
        }
        Some("servers.signIn") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<McpScope>(obj, "scope", &mut out);
            crate::types::keys::opt_key::<String>(obj, "project", &mut out);
            Some(out)
        }
        Some("agents.signInCode") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "signInId", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "code", &mut out, |x| text_at_least(x, 1));
            Some(out)
        }
        Some("agents.signIns") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("agents.signInStop") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "signInId", &mut out);
            Some(out)
        }
        Some("agents.signInLine") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::opt_key::<String>(obj, "name", &mut out);
            Some(out)
        }
        Some("agents.key") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "key", &mut out, |x| text_at_least(x, 1));
            crate::types::keys::opt_key::<AgentsKeyWay>(obj, "way", &mut out);
            Some(out)
        }
        Some("agents.cloudSignIn") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsCloudSignInAgent>(obj, "agent", &mut out);
            Some(out)
        }
        Some("providers.checkKey") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "provider", &mut out, |x| text_at_most(x, 64));
            crate::types::keys::req_key_with::<String>(obj, "key", &mut out, |x| text_at_least(x, 1));
            Some(out)
        }
        Some("broker.status") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("agents.addTools") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            Some(out)
        }
        Some("agents.setup") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "on", &mut out);
            crate::types::keys::dbl_key_with::<String>(obj, "program", &mut out, |x| dbl_rule(x, |v| text_at_least(v, 1)));
            crate::types::keys::dbl_key_with::<String>(obj, "configDir", &mut out, |x| dbl_rule(x, |v| text_at_least(v, 1)));
            crate::types::keys::dbl_key::<Vec<String>>(obj, "args", &mut out);
            env_key(obj, &mut out);
            Some(out)
        }
        Some("skills.search") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "q", &mut out);
            crate::types::keys::opt_key_with::<JsInt>(obj, "limit", &mut out, |x| {
                opt_rule(x, |v| num_at_least(v.as_f64(), 1.0).and_then(|()| num_at_most(v.as_f64(), 50.0)))
            });
            Some(out)
        }
        Some("skills.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "skill", &mut out);
            Some(out)
        }
        Some("skills.preview") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
            Some(out)
        }
        Some("skills.add") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "skill", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "agents", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
            Some(out)
        }
        Some("skills.remove") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
            Some(out)
        }
        Some("skills.toggle") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
            crate::types::keys::req_key::<bool>(obj, "on", &mut out);
            Some(out)
        }
        Some("servers.add") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "project", &mut out);
            crate::types::keys::opt_key::<String>(obj, "command", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "args", &mut out);
            crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "env", &mut out);
            crate::types::keys::opt_key::<String>(obj, "url", &mut out);
            crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "headers", &mut out);
            Some(out)
        }
        Some("servers.remove") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<McpScope>(obj, "scope", &mut out);
            Some(out)
        }
        Some("servers.toggle") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<McpScope>(obj, "scope", &mut out);
            crate::types::keys::req_key::<bool>(obj, "on", &mut out);
            Some(out)
        }
        Some("plugins.toggle") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AgentsTarget>(obj, "target", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            crate::types::keys::req_key::<String>(obj, "plugin", &mut out);
            crate::types::keys::req_key::<bool>(obj, "on", &mut out);
            Some(out)
        }
        Some("init.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "on", &mut out);
            Some(out)
        }
        Some("init.keys") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key_with::<String>(obj, "provider", &mut out, |x| opt_rule(x, |v| text_at_most(v, 64)));
            crate::types::keys::opt_key::<String>(obj, "key", &mut out);
            crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "rows", &mut out);
            Some(out)
        }
        Some("init.start") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<InitRoad>(obj, "road", &mut out);
            crate::types::keys::opt_key::<String>(obj, "harness", &mut out);
            crate::types::keys::opt_key::<String>(obj, "on", &mut out);
            Some(out)
        }
        Some("init.answer") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<InitScreenId>(obj, "screen", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "ticks", &mut out);
            crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "answers", &mut out);
            Some(out)
        }
        Some("init.step") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<JsInt>(obj, "at", &mut out, |x| num_at_least(x.as_f64(), 0.0));
            Some(out)
        }
        Some("init.draft") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "at", &mut out, |x| text_at_least(x, 1).and_then(|()| text_at_most(x, 64)));
            crate::types::keys::opt_key::<Vec<String>>(obj, "ticks", &mut out);
            crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "answers", &mut out);
            Some(out)
        }
        Some("init.retry") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "tool", &mut out);
            Some(out)
        }
        Some("init.build") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "firstWorkspace", &mut out);
            crate::types::keys::opt_key::<String>(obj, "importFolder", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "yes", &mut out);
            crate::types::keys::opt_key::<String>(obj, "on", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "rebuild", &mut out);
            Some(out)
        }
        Some("init.signInCode") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "tool", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "code", &mut out, |x| text_at_least(x, 1).and_then(|()| text_at_most(x, 256)));
            Some(out)
        }
        Some("init.cancel") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("preferences.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("preferences.set") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<PreferencesPatch>(obj, "patch", &mut out);
            Some(out)
        }
        Some("release.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("release.check") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
            Some(out)
        }
        Some("host.restart") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("projects.add") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "source", &mut out);
            crate::types::keys::opt_key::<String>(obj, "on", &mut out);
            crate::types::keys::opt_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<String>(obj, "base", &mut out);
            crate::types::keys::opt_key::<String>(obj, "into", &mut out);
            crate::types::keys::opt_key::<SeedChoice>(obj, "seed", &mut out);
            Some(out)
        }
        Some("project.seed.plan") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "source", &mut out);
            crate::types::keys::opt_key::<String>(obj, "on", &mut out);
            Some(out)
        }
        Some("projects.seed") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "project", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "keep", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "cut", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "none", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "save", &mut out);
            Some(out)
        }
        Some("projects.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("conversations.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "project", &mut out);
            crate::types::keys::opt_key::<String>(obj, "agent", &mut out);
            Some(out)
        }
        Some("conversations.read") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "project", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "agent", &mut out, |x| text_at_least(x, 1));
            crate::types::keys::req_key_with::<String>(obj, "conversation", &mut out, |x| {
                text_at_least(x, 1).and_then(|()| text_at_most(x, 200))
            });
            crate::types::keys::opt_key::<bool>(obj, "preview", &mut out);
            Some(out)
        }
        Some("projects.defaults") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("projects.resolve") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "ref", &mut out);
            Some(out)
        }
        Some("projects.branch") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "projectId", &mut out);
            Some(out)
        }
        Some("projects.remove") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "projectId", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
            Some(out)
        }
        Some("projects.icon") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "projectId", &mut out);
            crate::types::keys::req_key_with::<Option<String>>(obj, "png", &mut out, |x| opt_rule(x, |v| text_at_most(v, 327680)));
            Some(out)
        }
        Some("projects.icons") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_arr_with::<ProjectIconHash>(obj, "hashes", &mut out, |n| items_at_most(n, 1000), |_| Ok(()));
            Some(out)
        }
        Some("project.plan") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "source", &mut out);
            Some(out)
        }
        Some("project.import") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "source", &mut out);
            crate::types::keys::req_key::<String>(obj, "dest", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "replace", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "carry", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "rewrite", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "agents", &mut out);
            Some(out)
        }
        Some("project.export") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "source", &mut out);
            crate::types::keys::req_key::<String>(obj, "dest", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "replace", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "agents", &mut out);
            Some(out)
        }
        Some("image.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "name", &mut out);
            Some(out)
        }
        Some("image.build") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "place", &mut out, |x| text_at_least(x, 1));
            crate::types::keys::opt_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
            Some(out)
        }
        Some("image.export") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "dest", &mut out, |x| text_at_least(x, 1));
            crate::types::keys::req_key_with::<String>(obj, "passphrase", &mut out, |x| {
                text_at_least(x, 12).and_then(|()| text_at_most(x, 256))
            });
            crate::types::keys::opt_key::<String>(obj, "name", &mut out);
            Some(out)
        }
        Some("slates.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("slates.write") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
            crate::types::keys::opt_key::<String>(obj, "text", &mut out);
            crate::types::keys::opt_key::<IndexMap<String, Value>>(obj, "document", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
            crate::types::keys::opt_key::<JsInt>(obj, "ifVersion", &mut out);
            crate::types::keys::opt_key::<SlateWireValues>(obj, "values", &mut out);
            crate::types::keys::opt_key::<SlatesWritePress>(obj, "press", &mut out);
            Some(out)
        }
        Some("slates.state") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
            crate::types::keys::opt_key::<SlateWireValues>(obj, "values", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "start", &mut out);
            crate::types::keys::opt_key::<JsInt>(obj, "ifVersion", &mut out);
            Some(out)
        }
        Some("slates.read") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "values", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "text", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "sketch", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "document", &mut out);
            Some(out)
        }
        Some("slates.catalog") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
            Some(out)
        }
        Some("slates.event") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<JsInt>(obj, "version", &mut out);
            crate::types::keys::req_key::<String>(obj, "piece", &mut out);
            crate::types::keys::req_key::<SlatesEventEvent>(obj, "event", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "requestId", &mut out, |x| {
                text_at_least(x, 1).and_then(|()| text_at_most(x, 200))
            });
            crate::types::keys::opt_key::<SlatesEventScope>(obj, "scope", &mut out);
            crate::types::keys::opt_key_with::<JsInt>(obj, "rowAction", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 0.0)));
            Some(out)
        }
        Some("slates.approve") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<String>(obj, "key", &mut out);
            crate::types::keys::req_key::<SlatesApproveScope>(obj, "scope", &mut out);
            Some(out)
        }
        Some("slates.cancel") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<String>(obj, "run", &mut out);
            Some(out)
        }
        Some("slates.revoke") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<String>(obj, "key", &mut out);
            Some(out)
        }
        Some("slates.shown") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("slates.subscribe") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<Vec<String>>(obj, "sources", &mut out);
            Some(out)
        }
        Some("slates.unsubscribe") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<Vec<String>>(obj, "sources", &mut out);
            Some(out)
        }
        Some("slates.resolve") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_arr_with::<String>(obj, "paths", &mut out, |n| items_at_most(n, 200), |_| Ok(()));
            Some(out)
        }
        Some("slates.image") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "src", &mut out, |x| text_at_most(x, 4096));
            crate::types::keys::opt_key_with::<String>(obj, "have", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            Some(out)
        }
        _ => None,
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `maxMachines`: int min 0
/// - `maxDepth`: int min 1
impl Check for PlacesSetSpawn {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.max_machines {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<bool>(obj, "spawn", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxMachines", &mut out, Self::rule_max_machines);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxDepth", &mut out, Self::rule_max_depth);
        Some(out)
    }
}
impl PlacesSetSpawn {
    fn rule_max_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_max_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 1.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for PlacesSetSize {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<JsInt>(obj, "cpu", &mut out);
        crate::types::keys::req_key::<JsInt>(obj, "memMb", &mut out);
        Some(out)
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `maxMachines`: int min 0
/// - `maxDepth`: int min 1
impl Check for WorkspacesCreateAgents {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.max_machines {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("maxMachines"))?;
        }
        if let Some(v) = self.max_depth {
            num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("maxDepth"))?;
        }
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::opt_key::<bool>(obj, "spawn", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxMachines", &mut out, Self::rule_max_machines);
        crate::types::keys::opt_key_with::<JsInt>(obj, "maxDepth", &mut out, Self::rule_max_depth);
        Some(out)
    }
}
impl WorkspacesCreateAgents {
    fn rule_max_machines(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_max_depth(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 1.0)?;
        }
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspacesCreateSub {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "on", &mut out);
        crate::types::keys::opt_key::<String>(obj, "size", &mut out);
        crate::types::keys::opt_key::<String>(obj, "from", &mut out);
        crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
        crate::types::keys::opt_key::<String>(obj, "harness", &mut out);
        Some(out)
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `cpu`: int min 0 exclusive
/// - `memMb`: int min 0 exclusive
impl Check for WorkspacesResizeSize {
    fn check(&self) -> Result<(), SchemaError> {
        num_above(self.cpu.as_f64(), 0.0).map_err(|e| e.under("cpu"))?;
        num_above(self.mem_mb.as_f64(), 0.0).map_err(|e| e.under("memMb"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key_with::<JsInt>(obj, "cpu", &mut out, Self::rule_cpu);
        crate::types::keys::req_key_with::<JsInt>(obj, "memMb", &mut out, Self::rule_mem_mb);
        Some(out)
    }
}
impl WorkspacesResizeSize {
    fn rule_cpu(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_above((*x).as_f64(), 0.0)?;
        Ok(())
    }
    fn rule_mem_mb(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_above((*x).as_f64(), 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for WorkspacesReviewDraftOnItem {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "id", &mut out);
        crate::types::keys::req_key::<bool>(obj, "on", &mut out);
        Some(out)
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `index`: int min 0
/// - `action`: int min 0
impl Check for SlatesWritePress {
    fn check(&self) -> Result<(), SchemaError> {
        if let Some(v) = self.index {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("index"))?;
        }
        if let Some(v) = self.action {
            num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("action"))?;
        }
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<String>(obj, "piece", &mut out);
        crate::types::keys::opt_key_with::<JsInt>(obj, "index", &mut out, Self::rule_index);
        crate::types::keys::opt_key_with::<JsInt>(obj, "action", &mut out, Self::rule_action);
        Some(out)
    }
}
impl SlatesWritePress {
    fn rule_index(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
    fn rule_action(x: &Option<JsInt>) -> Result<(), crate::types::check::SchemaError> {
        if let Some(v) = *x {
            num_at_least(v.as_f64(), 0.0)?;
        }
        Ok(())
    }
}

/// Refinements zod puts on this shape, each an issue when it fails:
/// - `index`: int min 0
impl Check for SlatesEventScope {
    fn check(&self) -> Result<(), SchemaError> {
        num_at_least(self.index.as_f64(), 0.0).map_err(|e| e.under("index"))?;
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::any_key::<Value>(obj, "item", &mut out);
        crate::types::keys::req_key_with::<JsInt>(obj, "index", &mut out, Self::rule_index);
        Some(out)
    }
}
impl SlatesEventScope {
    fn rule_index(x: &JsInt) -> Result<(), crate::types::check::SchemaError> {
        num_at_least((*x).as_f64(), 0.0)?;
        Ok(())
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeRequest {
    fn check(&self) -> Result<(), SchemaError> {
        self.op.check()?;
        self.origin.check().map_err(|e| e.under("origin"))?;
        Ok(())
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let mut out = crate::types::check::issues_of::<RuntimeOp>(value);
        let serde_json::Value::Object(obj) = value else {
            out.push(crate::types::check::not_object(value));
            return Some(out);
        };
        if let Some(named) = obj.get("op").and_then(serde_json::Value::as_str).and_then(crate::wire::runtime_ops::strict_keys) {
            let unknown: Vec<String> = obj.keys().filter(|k| !named.contains(&k.as_str())).cloned().collect();
            if !unknown.is_empty() {
                out.push(crate::types::check::unrecognized(Vec::new(), unknown));
            }
        }
        if let Some(origin) = obj.get("origin") {
            if let Err(es) = crate::types::check::read_present::<crate::views::workspace::WorkspaceOrigin>(origin) {
                out.extend(es.into_iter().map(|e| e.under("origin")));
            }
        }
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for ExecEvent {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }

    fn describe(path: &[serde_json::Value]) -> Option<crate::types::check::Zod> {
        crate::types::check::tag_describe(path, "type", &["exec.output", "exec.exit"])
    }

    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let Some(tag) = obj.get("type") else {
            return Some(vec![crate::types::check::missing::<Self>("type")]);
        };
        match tag.as_str() {
            Some("exec.output") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "execId", &mut out);
                crate::types::keys::req_key::<String>(obj, "text", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            Some("exec.exit") => {
                let mut out = Vec::new();
                crate::types::keys::req_key::<String>(obj, "execId", &mut out);
                crate::types::keys::req_key::<Option<JsInt>>(obj, "exitCode", &mut out);
                crate::types::keys::opt_key::<String>(obj, "error", &mut out);
                if out.is_empty() {
                    out = crate::types::check::rules_of::<Self>(value);
                }
                Some(out)
            }
            _ => Some(vec![crate::types::check::unknown_tag::<Self>("type", tag)]),
        }
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeOkResponse {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Option<ReqId>>(obj, "id", &mut out);
        crate::types::keys::req_key::<True>(obj, "ok", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeErrorResponse {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<Option<ReqId>>(obj, "id", &mut out);
        crate::types::keys::req_key::<False>(obj, "ok", &mut out);
        crate::types::keys::req_key::<String>(obj, "error", &mut out);
        crate::types::keys::opt_key::<String>(obj, "kind", &mut out);
        crate::types::keys::opt_key::<String>(obj, "fix", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for RuntimeResponse {
    fn check(&self) -> Result<(), SchemaError> {
        match self {
            Self::V0(r) => r.check(),
            Self::V1(r) => r.check(),
        }
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        Some(crate::types::check::union_answer(vec![
            crate::types::check::issues_of::<RuntimeOkResponse>(value),
            crate::types::check::issues_of::<RuntimeErrorResponse>(value),
        ]))
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionInterruptResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<SessionInterruptOutcome>(obj, "outcome", &mut out);
        crate::types::keys::opt_key::<Vec<String>>(obj, "under", &mut out);
        crate::types::keys::opt_key::<String>(obj, "error", &mut out);
        crate::types::keys::opt_key::<String>(obj, "left", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionSteerResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<SessionSteerOutcome>(obj, "outcome", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAccessResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<SessionAccessOutcome>(obj, "outcome", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionAnswerResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<SessionAnswerOutcome>(obj, "outcome", &mut out);
        Some(out)
    }
}

/// No refinement of its own; checks every nested field that has one.
impl Check for SessionRenameResult {
    fn check(&self) -> Result<(), SchemaError> {
        Ok(())
    }
    fn pass(value: &serde_json::Value) -> Option<Vec<crate::types::check::SchemaError>> {
        let serde_json::Value::Object(obj) = value else {
            return Some(vec![crate::types::check::not_object(value)]);
        };
        let mut out = Vec::new();
        crate::types::keys::req_key::<SessionRenameOutcome>(obj, "outcome", &mut out);
        crate::types::keys::opt_key::<String>(obj, "error", &mut out);
        Some(out)
    }
}
