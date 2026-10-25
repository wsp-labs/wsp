// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

pub(super) fn runtime_op_pass_a(
    tag: Option<&str>,
    obj: &serde_json::Map<String, serde_json::Value>,
    value: &serde_json::Value,
) -> Option<Vec<crate::types::check::SchemaError>> {
    let _ = (obj, value);
    match tag {
        Some("auth") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "token", &mut out);
            Some(out)
        }
        Some("ticket.issue") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<TicketPurpose>(obj, "purpose", &mut out);
            Some(out)
        }
        Some("pair.issue") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<True>(obj, "here", &mut out);
            Some(out)
        }
        Some("pair.redeem") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "code", &mut out, |x| text_at_most(x, 64));
            crate::types::keys::req_key_with::<String>(obj, "name", &mut out, |x| text_at_most(x, 200));
            Some(out)
        }
        Some("seal.open") => {
            let mut body = obj.clone();
            body.shift_remove("op");
            Some(crate::types::check::issues_of::<SealOpenRequest>(&serde_json::Value::Object(body)))
        }
        Some("device.auth") => {
            let mut body = obj.clone();
            body.shift_remove("op");
            Some(crate::types::check::issues_of::<DeviceAuthRequest>(&serde_json::Value::Object(body)))
        }
        Some("account.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("devices.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("devices.revoke") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "deviceId", &mut out);
            Some(out)
        }
        Some("place.join") => {
            let mut body = obj.clone();
            body.shift_remove("op");
            Some(crate::types::check::issues_of::<PlaceJoinRequest>(&serde_json::Value::Object(body)))
        }
        Some("place.auth") => {
            let mut body = obj.clone();
            body.shift_remove("op");
            Some(crate::types::check::issues_of::<PlaceAuthRequest>(&serde_json::Value::Object(body)))
        }
        Some("place.prove") => {
            let mut body = obj.clone();
            body.shift_remove("op");
            Some(crate::types::check::issues_of::<PlaceProveRequest>(&serde_json::Value::Object(body)))
        }
        Some("places.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("places.update") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::opt_key::<SudoPassword>(obj, "sudoPassword", &mut out);
            Some(out)
        }
        Some("places.holds") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            Some(out)
        }
        Some("places.remove") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::opt_key::<SudoPassword>(obj, "sudoPassword", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "forget", &mut out);
            Some(out)
        }
        Some("places.doctor") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "doctorId", &mut out, |x| text_at_most(x, 64));
            crate::types::keys::opt_key_with::<String>(obj, "project", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            Some(out)
        }
        Some("places.dial") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            Some(out)
        }
        Some("places.set") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::opt_key::<Vec<PlaceSettingWord>>(obj, "reset", &mut out);
            crate::types::keys::opt_key_with::<String>(obj, "name", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            crate::types::keys::opt_key_with::<String>(obj, "ssh", &mut out, |x| opt_rule(x, |v| text_at_most(v, 300)));
            crate::types::keys::opt_key_with::<String>(obj, "recipe", &mut out, |x| {
                opt_rule(x, |v| text_at_least(v, 1).and_then(|()| text_at_most(v, 200)))
            });
            crate::types::keys::opt_key_with::<JsInt>(obj, "threads", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 1.0)));
            crate::types::keys::opt_key_with::<JsInt>(obj, "machines", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 1.0)));
            crate::types::keys::opt_key_with::<JsNumber>(obj, "spendPerDayUsd", &mut out, |x| opt_rule(x, |v| num_at_least(v.0, 0.0)));
            crate::types::keys::dbl_key_with::<JsInt>(obj, "napMs", &mut out, |x| {
                dbl_rule(x, |v| num_at_least(v.as_f64(), 60000.0).and_then(|()| num_at_most(v.as_f64(), 10800000.0)))
            });
            crate::types::keys::dbl_key_with::<JsInt>(obj, "turnLimitMs", &mut out, |x| {
                dbl_rule(x, |v| num_at_least(v.as_f64(), 3600000.0).and_then(|()| num_at_most(v.as_f64(), 86400000.0)))
            });
            crate::types::keys::opt_key::<PlacesSetSpawn>(obj, "spawn", &mut out);
            crate::types::keys::opt_key::<PlacesSetSize>(obj, "size", &mut out);
            crate::types::keys::opt_key_with::<JsInt>(obj, "deleteAfterMs", &mut out, |x| {
                opt_rule(x, |v| num_at_least(v.as_f64(), 0.0).and_then(|()| num_at_most(v.as_f64(), 86400000.0)))
            });
            Some(out)
        }
        Some("places.follow") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "recipe", &mut out, |x| {
                text_at_least(x, 1).and_then(|()| text_at_most(x, 200))
            });
            Some(out)
        }
        Some("places.skip") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "row", &mut out, |x| text_at_least(x, 1).and_then(|()| text_at_most(x, 300)));
            Some(out)
        }
        Some("places.setupLog") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::opt_key::<PlaceSetupStep>(obj, "step", &mut out);
            Some(out)
        }
        Some("places.estimate") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "ref", &mut out, |x| text_at_most(x, 200));
            crate::types::keys::req_key::<RecipeFile>(obj, "choices", &mut out);
            Some(out)
        }
        Some("places.loginLanded") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "placeId", &mut out);
            crate::types::keys::req_key::<String>(obj, "agent", &mut out);
            Some(out)
        }
        Some("places.door") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("places.mint") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("places.sshHosts") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("places.add") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key_with::<String>(obj, "addId", &mut out, |x| opt_rule(x, |v| text_at_most(v, 64)));
            crate::types::keys::req_key_with::<String>(obj, "address", &mut out, |x| text_at_most(x, 200));
            crate::types::keys::opt_key_with::<String>(obj, "name", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            crate::types::keys::opt_key_with::<JsInt>(obj, "sshPort", &mut out, |x| {
                opt_rule(x, |v| num_at_least(v.as_f64(), 1.0).and_then(|()| num_at_most(v.as_f64(), 65535.0)))
            });
            crate::types::keys::opt_key_with::<String>(obj, "keyPath", &mut out, |x| opt_rule(x, |v| text_at_most(v, 1024)));
            crate::types::keys::opt_key_with::<String>(obj, "hostKey", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            crate::types::keys::opt_key::<SudoPassword>(obj, "sudoPassword", &mut out);
            crate::types::keys::opt_key_with::<String>(obj, "recipe", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            Some(out)
        }
        Some("places.setup") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "ref", &mut out, |x| text_at_most(x, 200));
            crate::types::keys::opt_key_with::<String>(obj, "recipe", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            crate::types::keys::opt_key::<RecipeFile>(obj, "choices", &mut out);
            crate::types::keys::opt_key_with::<String>(obj, "addId", &mut out, |x| opt_rule(x, |v| text_at_most(v, 64)));
            crate::types::keys::opt_key_with::<String>(obj, "row", &mut out, |x| opt_rule(x, |v| text_at_most(v, 400)));
            Some(out)
        }
        Some("places.choose") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "ref", &mut out, |x| text_at_most(x, 200));
            crate::types::keys::req_key::<RecipeFile>(obj, "choices", &mut out);
            crate::types::keys::opt_key_with::<String>(obj, "recipe", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            Some(out)
        }
        Some("recipes.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("recipes.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "name", &mut out, |x| text_at_most(x, 200));
            Some(out)
        }
        Some("recipes.save") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "name", &mut out, |x| text_at_most(x, 200));
            crate::types::keys::any_key::<Value>(obj, "file", &mut out);
            crate::types::keys::opt_key_with::<String>(obj, "from", &mut out, |x| opt_rule(x, |v| text_at_most(v, 200)));
            Some(out)
        }
        Some("recipes.remove") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "name", &mut out, |x| text_at_most(x, 200));
            crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
            Some(out)
        }
        Some("recipes.options") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("events.subscribe") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key_with::<JsInt>(obj, "after", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 0.0)));
            crate::types::keys::opt_key::<String>(obj, "stream", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "creates", &mut out);
            Some(out)
        }
        Some("status.subscribe") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("status.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("workspaces.create") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "golden", &mut out);
            crate::types::keys::req_key::<String>(obj, "project", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            crate::types::keys::opt_key::<JsNumber>(obj, "cpu", &mut out);
            crate::types::keys::opt_key::<JsNumber>(obj, "memMb", &mut out);
            crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "envs", &mut out);
            crate::types::keys::opt_key::<IndexMap<String, String>>(obj, "labels", &mut out);
            crate::types::keys::opt_key::<WorkspacesCreateAgents>(obj, "agents", &mut out);
            crate::types::keys::dbl_key::<JsNumber>(obj, "idleWindowMs", &mut out);
            crate::types::keys::opt_key::<String>(obj, "parent", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "engine", &mut out);
            crate::types::keys::opt_key::<WorkspacesCreateSub>(obj, "sub", &mut out);
            Some(out)
        }
        Some("workspaces.landing") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "project", &mut out);
            crate::types::keys::opt_key::<True>(obj, "sized", &mut out);
            Some(out)
        }
        Some("workspaces.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("workspaces.resolve") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "ref", &mut out);
            Some(out)
        }
        Some("workspaces.get") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("workspaces.nap") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("workspaces.resize") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<WorkspacesResizeSize>(obj, "size", &mut out);
            Some(out)
        }
        Some("workspaces.wake") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("workspaces.restartDaemon") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("workspaces.stopWake") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("workspaces.upgrade") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("workspaces.rename") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "name", &mut out);
            Some(out)
        }
        Some("workspaces.look") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::dbl_key::<WorkspaceTheme>(obj, "theme", &mut out);
            crate::types::keys::dbl_key::<WorkspaceGlyph>(obj, "glyph", &mut out);
            Some(out)
        }
        Some("workspaces.bringBack") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "title", &mut out);
            crate::types::keys::opt_key::<String>(obj, "body", &mut out);
            Some(out)
        }
        Some("worktree.make") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "project", &mut out);
            crate::types::keys::req_key::<String>(obj, "branch", &mut out);
            Some(out)
        }
        Some("folder.make") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "project", &mut out);
            Some(out)
        }
        Some("worktree.remove") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "project", &mut out);
            crate::types::keys::req_key::<String>(obj, "branch", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "force", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
            Some(out)
        }
        Some("workspaces.checkout") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "fresh", &mut out);
            Some(out)
        }
        Some("workspaces.discard") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "path", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("workspaces.commit") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "message", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "paths", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("workspaces.commitDraft") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<Vec<String>>(obj, "paths", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("workspaces.viewed") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "path", &mut out);
            crate::types::keys::dbl_key::<String>(obj, "blob", &mut out);
            Some(out)
        }
        Some("workspaces.pullRequestView") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "fresh", &mut out);
            Some(out)
        }
        Some("workspaces.pullRequestDiff") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("workspaces.pullRequestSend") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key_with::<Vec<PullRequestItem>>(obj, "items", &mut out, |x| items_at_least(x.len(), 1));
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("workspaces.pullRequestReply") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key_with::<JsInt>(obj, "replyTo", &mut out, |x| opt_rule(x, |v| num_at_least(v.as_f64(), 0.0)));
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key_with::<String>(obj, "body", &mut out, |x| text_at_most(x, 65536));
            Some(out)
        }
        Some("workspaces.pullRequestResolve") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<bool>(obj, "resolved", &mut out);
            Some(out)
        }
        Some("workspaces.pullRequestReact") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "subject", &mut out);
            crate::types::keys::req_key::<ReactionContent>(obj, "content", &mut out);
            crate::types::keys::req_key::<bool>(obj, "on", &mut out);
            Some(out)
        }
        Some("workspaces.fix") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "check", &mut out);
            crate::types::keys::opt_key::<String>(obj, "child", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "childThreadId", &mut out);
            Some(out)
        }
        Some("workspaces.merge") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<MergeMethod>(obj, "method", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "whenChecksPass", &mut out);
            crate::types::keys::opt_key_with::<String>(obj, "head", &mut out, |x| opt_rule(x, |v| text_at_least(v, 1)));
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("workspaces.start") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "url", &mut out);
            crate::types::keys::opt_key::<String>(obj, "project", &mut out);
            crate::types::keys::opt_key::<String>(obj, "agent", &mut out);
            crate::types::keys::opt_key::<String>(obj, "model", &mut out);
            crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
            crate::types::keys::opt_key::<AccessChoice>(obj, "access", &mut out);
            Some(out)
        }
        Some("workspaces.review") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "url", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "agent", &mut out);
            crate::types::keys::opt_key::<String>(obj, "model", &mut out);
            crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
            Some(out)
        }
        Some("workspaces.reviewDraft") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "summary", &mut out);
            crate::types::keys::opt_key::<WorkspacesReviewDraftVerdict>(obj, "verdict", &mut out);
            crate::types::keys::opt_key::<Vec<WorkspacesReviewDraftOnItem>>(obj, "on", &mut out);
            Some(out)
        }
        Some("workspaces.reviewPost") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("workspaces.update") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        Some("workspaces.mergeIn") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "child", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "childThreadId", &mut out);
            Some(out)
        }
        Some("workspaces.delete") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("workspaces.forget") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("workspaces.snapshot") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("projectGoldens.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("projectGoldens.remove") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "snapshotId", &mut out);
            Some(out)
        }
        Some("workspaces.touch") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("daemon.open") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
            Some(out)
        }
        Some("sys.subscribe") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
            Some(out)
        }
        Some("sys.unsubscribe") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "placeId", &mut out);
            Some(out)
        }
        Some("appProcs.watch") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("appProcs.unwatch") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("appProcs.offer") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            Some(out)
        }
        Some("appProcs.set") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<AppProcList>(obj, "procs", &mut out);
            Some(out)
        }
        Some("activity.read") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "computer", &mut out);
            crate::types::keys::opt_key::<String>(obj, "thread", &mut out);
            Some(out)
        }
        Some("activity.end") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "computer", &mut out);
            crate::types::keys::req_key_with::<JsInt>(obj, "pid", &mut out, |x| num_above(x.as_f64(), 0.0));
            crate::types::keys::req_key::<JsNumber>(obj, "startedAt", &mut out);
            Some(out)
        }
        Some("daemon.send") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "channel", &mut out);
            crate::types::keys::req_key::<DaemonFrame>(obj, "frame", &mut out);
            Some(out)
        }
        Some("daemon.close") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "channel", &mut out);
            Some(out)
        }
        Some("sessions.start") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "project", &mut out);
            crate::types::keys::opt_key::<String>(obj, "branch", &mut out);
            crate::types::keys::req_key::<String>(obj, "prompt", &mut out);
            crate::types::keys::opt_key::<String>(obj, "harness", &mut out);
            crate::types::keys::opt_key::<String>(obj, "thread", &mut out);
            crate::types::keys::opt_key::<String>(obj, "cwd", &mut out);
            crate::types::keys::opt_key::<String>(obj, "model", &mut out);
            crate::types::keys::opt_key::<String>(obj, "effort", &mut out);
            crate::types::keys::opt_key::<String>(obj, "permissionMode", &mut out);
            crate::types::keys::opt_key::<AccessChoice>(obj, "access", &mut out);
            crate::types::keys::opt_key::<String>(obj, "contextWindow", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "fast", &mut out);
            crate::types::keys::opt_key::<SessionOrigin>(obj, "startedBy", &mut out);
            crate::types::keys::opt_key::<String>(obj, "requestId", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "answerHeld", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "followed", &mut out);
            crate::types::keys::opt_key::<String>(obj, "attempt", &mut out);
            crate::types::keys::opt_key_with::<Vec<String>>(obj, "notify", &mut out, |x| opt_rule(x, |v| items_at_least(v.len(), 1)));
            crate::types::keys::opt_key::<String>(obj, "turnToken", &mut out);
            crate::types::keys::opt_key::<String>(obj, "title", &mut out);
            crate::types::keys::opt_key::<ResumeAsk>(obj, "resume", &mut out);
            crate::types::keys::opt_key::<String>(obj, "replaces", &mut out);
            crate::types::keys::opt_key::<ForkSource>(obj, "fork", &mut out);
            crate::types::keys::opt_key::<Vec<Attachment>>(obj, "attachments", &mut out);
            Some(out)
        }
        Some("harnesses.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "project", &mut out);
            crate::types::keys::opt_key::<String>(obj, "computer", &mut out);
            Some(out)
        }
        Some("sessions.list") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::opt_key::<String>(obj, "workspaceId", &mut out);
            Some(out)
        }
        Some("sessions.history") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key_with::<JsInt>(obj, "before", &mut out, |x| opt_rule(x, |v| num_above(v.as_f64(), 0.0)));
            crate::types::keys::opt_key_with::<JsInt>(obj, "limit", &mut out, |x| {
                opt_rule(x, |v| num_above(v.as_f64(), 0.0).and_then(|()| num_at_most(v.as_f64(), 1000.0)))
            });
            crate::types::keys::opt_key::<True>(obj, "whole", &mut out);
            Some(out)
        }
        Some("sessions.head") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<True>(obj, "afterRead", &mut out);
            Some(out)
        }
        Some("sessions.attachment") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "workspaceId", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::req_key::<String>(obj, "requestId", &mut out);
            crate::types::keys::req_key_with::<JsInt>(obj, "index", &mut out, |x| num_at_least(x.as_f64(), 0.0));
            Some(out)
        }
        Some("sessions.interrupt") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "task", &mut out);
            Some(out)
        }
        Some("sessions.steer") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
            crate::types::keys::req_key::<String>(obj, "prompt", &mut out);
            crate::types::keys::opt_key::<String>(obj, "requestId", &mut out);
            crate::types::keys::opt_key::<Vec<Attachment>>(obj, "attachments", &mut out);
            Some(out)
        }
        Some("sessions.answer") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
            crate::types::keys::req_key::<String>(obj, "askId", &mut out);
            crate::types::keys::req_key::<String>(obj, "optionId", &mut out);
            crate::types::keys::opt_key::<String>(obj, "reason", &mut out);
            Some(out)
        }
        Some("sessions.access") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
            crate::types::keys::req_key::<String>(obj, "permissionMode", &mut out);
            Some(out)
        }
        Some("sessions.rename") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "sessionId", &mut out);
            crate::types::keys::req_key::<String>(obj, "title", &mut out);
            Some(out)
        }
        Some("sessions.forget") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
            Some(out)
        }
        Some("sessions.delete") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            crate::types::keys::opt_key::<ProcessesAsk>(obj, "processes", &mut out);
            crate::types::keys::opt_key::<bool>(obj, "check", &mut out);
            Some(out)
        }
        Some("sessions.read") => {
            let mut out = Vec::new();
            crate::types::keys::req_key::<ReqId>(obj, "id", &mut out);
            crate::types::keys::req_key::<String>(obj, "threadId", &mut out);
            Some(out)
        }
        _ => None,
    }
}
