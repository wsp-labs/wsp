// SPDX-License-Identifier: AGPL-3.0-only
//! The git verbs' runs and calls (`verbs/workspaces.ts:59-320` and `:416-449`): rename, fork, commit, discard, fix,
//! merge in, merge and update. Spec: `spec/workspaces-git.md`. The verb table is in the parent module.

use super::*;

/// `renameWorkspace` (workspaces-help.ts:319-323) with the record kept as the host wrote it: its keys in the reply's
/// order. Answers the line and the `{was, workspace}` answer.
async fn renamed_answer(client: &dyn HostClient, reference: &str, name: &str) -> Result<(String, Value), HostError> {
    let source = workspace_of(client, reference).await?;
    let reply =
        client.request("workspaces.rename", pick(vec![("workspaceId", Some(json!(source.id))), ("name", Some(json!(name)))])).await?;
    let workspace = reply.get("workspace").cloned().unwrap_or(Value::Null);
    let typed: WorkspaceOut = serde_json::from_value(workspace.clone()).map_err(|_| HostError::new(other_version("workspaces.rename")))?;
    let line = renamed_workspace_line(&RenamedWorkspace { was: source.name.clone(), workspace: typed });
    Ok((line, json!({ "was": source.name, "workspace": workspace })))
}

async fn fork_reads(
    client: &dyn HostClient,
    reference: &str,
    task: Option<&str>,
    harness: Option<&str>,
    picks: &Picks,
    tell: &[String],
) -> Result<(WorkspaceOut, Option<Vec<String>>), HostError> {
    let source = workspace_of(client, reference).await?;
    if workspace_state(&WorkspaceStateInput { phase: source.phase, machine_state: None, reach: None, wake_refused: None })
        == WorkspaceState::Gone
    {
        return Err(HostError::new(gone_refusal(Some(&source.name), "fork", source.gone.as_deref())));
    }
    let notify = notify_of(client, tell).await?;
    if let Some(task) = task {
        checked_start(client, task, harness, picks, Some(&source.id), None).await?;
    }
    Ok((source, notify))
}

async fn fork_source(
    client: &dyn HostClient,
    reference: &str,
    task: Option<&str>,
    harness: Option<&str>,
    picks: &Picks,
) -> Result<WorkspaceOut, HostError> {
    let source = workspace_of(client, reference).await?;
    if let Some(task) = task {
        checked_start(client, task, harness, picks, Some(&source.id), None).await?;
    }
    Ok(source)
}

/// The `rename` tool's call (`verbs/workspaces.ts:71-74`).
pub(super) fn call_rename(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let reference = frame_str(&args, "workspace").unwrap_or_default();
        let name = frame_str(&args, "name").unwrap_or_default();
        let (line, answer) = renamed_answer(&*client, &reference, &name).await?;
        Ok(as_text(&line, answer))
    })
}

/// `wsp rename` (`verbs/workspaces.ts:59-65`).
pub(super) fn run_rename(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (reference, name) = match ctx.args.as_slice() {
            [reference, name] => (reference.clone(), name.clone()),
            _ => return Err(usage_refusal("wsp rename takes a workspace and one name.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let (line, answer) = renamed_answer(&*client, &reference, &name).await?;
        ctx.out.emit(&answer, Some(&line));
        Ok(0)
    })
}

/// The `fork` tool's call (`verbs/workspaces.ts:113-136`).
pub(super) fn call_fork(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let reference = frame_str(&args, "workspace").unwrap_or_default();
        let name = frame_str(&args, "name");
        let word = frame_str(&args, "size");
        let task = frame_str(&args, "task");
        let harness = frame_str(&args, "agent");
        let folder = frame_str(&args, "cwd");
        let tell = frame_list(&args, "notify");
        let picks = frame_picks(&args);
        absolute_folder(folder.as_deref())?;
        let client = deps.client(None).await?;
        let source = match &task {
            None => fork_source(&*client, &reference, None, harness.as_deref(), &picks).await?,
            Some(task) => {
                before_sending(&*client, Box::pin(fork_source(&*client, &reference, Some(task), harness.as_deref(), &picks))).await?
            }
        };
        let asked = agents_asked(args.get("max_machines"), args.get("max_depth"), None)?;
        let project = project_of(&*client, &source.project.id).await?;
        let child_name = name.unwrap_or_else(|| format!("{}-fork", source.name));
        let created = create_for_unheard(
            &*client,
            &project_pick(&project),
            &child_name,
            &CreateAsked { size: word, agents: asked, parent: Some(source.id.clone()) },
        )
        .await?;
        let Some(task) = task else {
            return Ok(as_json(json_of(&created)));
        };
        let created_value = object_of(&created);
        let outcome: Result<Turn, HostError> = async {
            let notify = notify_of(&*client, &tell).await?;
            let opening = opening_of(
                &deps.env,
                &OpeningAt::Workspace { id: created.workspace.id.clone() },
                &task,
                &OpeningOpts {
                    picks: picks.clone(),
                    harness: harness.clone(),
                    cwd: folder.clone(),
                    notify,
                    elsewhere: deps.elsewhere,
                    ..OpeningOpts::default()
                },
            )?;
            let redial = redial_of(deps);
            follow(client.clone(), &opening, SessionOrigin::Agent, &FollowOn::default(), Some(&redial)).await
        }
        .await;
        let failure = match outcome {
            Ok(turn) => match turn_failure(&turn) {
                None => {
                    let mut answer = created_value;
                    answer.insert("turn".to_owned(), json_of(&turn_view(&turn)));
                    return Ok(as_json(Value::Object(answer)));
                }
                Some(ended) => ended,
            },
            Err(e) => e.message,
        };
        let text = format!("created {} {}; first turn failed: {failure}", created.workspace.name, created.workspace.id);
        let mut structured = created_value;
        structured.insert("failure".to_owned(), json!(failure));
        Ok(kept_result(text, Value::Object(structured)))
    })
}

/// `wsp fork` (`verbs/workspaces.ts:84-107`).
pub(super) fn run_fork(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp fork takes one workspace.", &usage_is(ctx))),
        };
        let task = flag(&ctx.flags, "send").map(str::to_owned);
        for dependent in ["agent", "model", "effort", "access", "cwd", "notify"] {
            if task.is_none() && flag(&ctx.flags, dependent).is_some() {
                return Err(usage_refusal(
                    &format!("--{dependent} says how a thread opens, and this line opens none."),
                    &format!("Add --send \"<task>\", or drop --{dependent}."),
                ));
            }
        }
        let client = ctx.client(None).await?;
        let harness = flag(&ctx.flags, "agent").map(str::to_owned);
        let picks = pick_flags(&ctx.flags);
        let tell = flag_list(&ctx.flags, "notify");
        let (source, notify) = match &task {
            None => fork_reads(&*client, &reference, None, harness.as_deref(), &picks, &tell).await?,
            Some(task) => {
                before_sending(&*client, Box::pin(fork_reads(&*client, &reference, Some(task), harness.as_deref(), &picks, &tell))).await?
            }
        };
        let max_machines = flag(&ctx.flags, "max-machines").map(|word| json!(word));
        let max_depth = flag(&ctx.flags, "max-depth").map(|word| json!(word));
        let asked = agents_asked(max_machines.as_ref(), max_depth.as_ref(), None)?;
        let project = project_of(&*client, &source.project.id).await?;
        let child_name = flag(&ctx.flags, "name").map(str::to_owned).unwrap_or_else(|| format!("{}-fork", source.name));
        let size = flag(&ctx.flags, "size").map(str::to_owned);
        let created = create_for(
            &*client,
            &ctx.out,
            &project_pick(&project),
            &child_name,
            &CreateAsked { size, agents: asked, parent: Some(source.id.clone()) },
        )
        .await?;
        let Some(task) = task else { return Ok(0) };
        let opening = opening_of(
            &ctx.env,
            &OpeningAt::Workspace { id: created.workspace.id.clone() },
            &task,
            &OpeningOpts {
                picks: picks.clone(),
                harness,
                cwd: flag(&ctx.flags, "cwd").map(str::to_owned),
                notify,
                elsewhere: ctx.elsewhere,
                ..OpeningOpts::default()
            },
        )?;
        let said = SaidAbout { opened: None, spend: turn_spend_word(Some(created.workspace.kind.as_ref())) };
        let turn = follow_verb(ctx, client.clone(), &opening, true, &Picks::default(), &said, None).await?;
        ctx.out.emit(&json!({ "turn": json_of(&turn_view(&turn)) }), None);
        Ok(0)
    })
}

/// The `commit` tool's call (`verbs/workspaces.ts:165-172`).
pub(super) fn call_commit(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let at = thread_at_in(&*client, &frame_str(&args, "thread").unwrap_or_default(), "wsp commit", false, cloud_on_in(&deps.env)).await?;
        let woke = awake(&*client, &view_of(&at.1)?, "commit", &quiet_tell).await?;
        let message = frame_str(&args, "message");
        let made = committed(
            &*client,
            &woke.workspace.id,
            message.as_deref(),
            &frame_list(&args, "files"),
            &quiet_tell,
            Some(&thread_id_of(&at.0)),
        )
        .await?;
        let shared = sharing_with(&*client, &at).await?;
        let line = with_shared(&committed_line(&thread_label(&at.0), &made_commit(&made)), &shared);
        Ok(as_text(&line, with_shared_field(object_of(&made), &shared)))
    })
}

/// `wsp commit` (`verbs/workspaces.ts:145-155`).
pub(super) fn run_commit(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp commit takes one thread.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let at = thread_at_in(&*client, &reference, "wsp commit", false, cloud_on_in(&ctx.env)).await?;
        let say = stderr_of(ctx);
        let woke = awake(&*client, &view_of(&at.1)?, "commit", &say).await?;
        let message = flag(&ctx.flags, "message");
        let made =
            committed(&*client, &woke.workspace.id, message, &flag_list(&ctx.flags, "file"), &say, Some(&thread_id_of(&at.0))).await?;
        let shared = sharing_with(&*client, &at).await?;
        let line = with_shared(&committed_line(&thread_label(&at.0), &made_commit(&made)), &shared);
        ctx.out.emit(&with_shared_field(object_of(&made), &shared), Some(&line));
        Ok(0)
    })
}

/// The `discard` tool's call (`verbs/workspaces.ts:200-207`).
pub(super) fn call_discard(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let at = thread_at_in(&*client, &frame_str(&args, "thread").unwrap_or_default(), "wsp discard", false, cloud_on_in(&deps.env)).await?;
        let woke = awake(&*client, &view_of(&at.1)?, "discard", &quiet_tell).await?;
        let path = frame_str(&args, "path").unwrap_or_default();
        let put: GitDiscardReply = parsed(
            client
                .request(
                    "workspaces.discard",
                    pick(vec![
                        ("workspaceId", Some(json!(woke.workspace.id))),
                        ("path", Some(json!(path))),
                        ("threadId", Some(json!(thread_id_of(&at.0)))),
                    ]),
                )
                .await?,
            "workspaces.discard",
        )?;
        let shared = sharing_with(&*client, &at).await?;
        let line = with_shared(&discarded_line(&thread_label(&at.0), &put.path), &shared);
        Ok(as_text(&line, with_shared_field(object_of(&put), &shared)))
    })
}

/// `wsp discard` (`verbs/workspaces.ts:181-194`).
pub(super) fn run_discard(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (reference, path) = match ctx.args.as_slice() {
            [reference, path] => (reference.clone(), path.clone()),
            _ => return Err(usage_refusal("wsp discard takes one thread and one file.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let at = thread_at_in(&*client, &reference, "wsp discard", false, cloud_on_in(&ctx.env)).await?;
        let say = stderr_of(ctx);
        let woke = awake(&*client, &view_of(&at.1)?, "discard", &say).await?;
        let thread_id = thread_id_of(&at.0);
        client
            .request(
                "workspaces.discard",
                pick(vec![
                    ("workspaceId", Some(json!(woke.workspace.id))),
                    ("path", Some(json!(path))),
                    ("check", Some(json!(true))),
                    ("threadId", Some(json!(thread_id))),
                ]),
            )
            .await?;
        let folder = folder_of(&at.1, home().as_deref());
        let question = format!("Discard the change to {path} in {folder}?\nThe file goes back to its last commit, which cannot be undone.");
        if !confirmed(ctx, &question, &path).await? {
            return Ok(1);
        }
        let put: GitDiscardReply = parsed(
            client
                .request(
                    "workspaces.discard",
                    pick(vec![
                        ("workspaceId", Some(json!(woke.workspace.id))),
                        ("path", Some(json!(path))),
                        ("threadId", Some(json!(thread_id))),
                    ]),
                )
                .await?,
            "workspaces.discard",
        )?;
        let shared = sharing_with(&*client, &at).await?;
        let line = with_shared(&discarded_line(&thread_label(&at.0), &put.path), &shared);
        ctx.out.emit(&with_shared_field(object_of(&put), &shared), Some(&line));
        Ok(0)
    })
}

/// The `fix` tool's call (`verbs/workspaces.ts:242-250`).
pub(super) fn call_fix(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let check = frame_str(&args, "check");
        let child = frame_str(&args, "child");
        if check.is_some() && child.is_some() {
            return Err(HostError::new(FIX_CHECK_OR_CHILD));
        }
        let client = deps.client(None).await?;
        let at = thread_at_in(&*client, &frame_str(&args, "thread").unwrap_or_default(), "wsp fix", false, cloud_on_in(&deps.env)).await?;
        let kid = match &child {
            Some(child) => Some(child_of(&*client, &at, child, "wsp fix", cloud_on_in(&deps.env)).await?),
            None => None,
        };
        let woke_id = match &check {
            None => awake(&*client, &view_of(&at.1)?, "fix", &quiet_tell).await?.workspace.id.clone(),
            Some(_) => at.1.id.clone(),
        };
        let child_ref = kid.as_ref().map(|k| ChildRef { workspace_id: k.1.id.clone(), thread_id: thread_id_of(&k.0) });
        let asked = asked_to_fix(&*client, &woke_id, check.as_deref(), child_ref.as_ref(), &thread_id_of(&at.0)).await?;
        let child_label = kid.as_ref().map(|k| thread_label(&k.0));
        Ok(as_text(&fix_line(&thread_label(&at.0), &asked, child_label.as_deref()), json_of(&asked)))
    })
}

/// `wsp fix` (`verbs/workspaces.ts:216-229`).
pub(super) fn run_fix(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp fix takes one thread.", &usage_is(ctx))),
        };
        let check = flag(&ctx.flags, "check").map(str::to_owned);
        let child = flag(&ctx.flags, "child").map(str::to_owned);
        if check.is_some() && child.is_some() {
            return Err(usage_refusal(FIX_CHECK_OR_CHILD, &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let at = thread_at_in(&*client, &reference, "wsp fix", false, cloud_on_in(&ctx.env)).await?;
        let kid = match &child {
            Some(child) => Some(child_of(&*client, &at, child, "wsp fix", cloud_on_in(&ctx.env)).await?),
            None => None,
        };
        let say = stderr_of(ctx);
        let woke_id = match &check {
            None => awake(&*client, &view_of(&at.1)?, "fix", &say).await?.workspace.id.clone(),
            Some(_) => at.1.id.clone(),
        };
        let child_ref = kid.as_ref().map(|k| ChildRef { workspace_id: k.1.id.clone(), thread_id: thread_id_of(&k.0) });
        let asked = asked_to_fix(&*client, &woke_id, check.as_deref(), child_ref.as_ref(), &thread_id_of(&at.0)).await?;
        let child_label = kid.as_ref().map(|k| thread_label(&k.0));
        ctx.out.emit(&json_of(&asked), Some(&fix_line(&thread_label(&at.0), &asked, child_label.as_deref())));
        Ok(0)
    })
}

/// The `merge_in` tool's call (`verbs/workspaces.ts:277-283`).
pub(super) fn call_merge_in(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let at = thread_at_in(&*client, &frame_str(&args, "lead").unwrap_or_default(), "wsp merge in", false, cloud_on_in(&deps.env)).await?;
        let kid = child_of(&*client, &at, &frame_str(&args, "child").unwrap_or_default(), "wsp merge in", cloud_on_in(&deps.env)).await?;
        let lead = MergeLead { workspace: at.1.clone(), thread_id: thread_id_of(&at.0) };
        let child = ChildRef { workspace_id: kid.1.id.clone(), thread_id: thread_id_of(&kid.0) };
        let done = merged_in(&*client, &lead, &child, &quiet_tell).await?;
        let said = MergeInResult { lead: thread_label(&at.0), child: thread_label(&kid.0), ..done.clone() };
        Ok(as_text(&merge_in_line(&said, Some(&thread_id_of(&kid.0))), json_of(&done)))
    })
}

/// `wsp merge in` (`verbs/workspaces.ts:259-268`).
pub(super) fn run_merge_in(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (lead, child) = match ctx.args.as_slice() {
            [lead, child] => (lead.clone(), child.clone()),
            _ => return Err(usage_refusal("wsp merge in takes a lead thread and one of its children.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let at = thread_at_in(&*client, &lead, "wsp merge in", false, cloud_on_in(&ctx.env)).await?;
        let kid = child_of(&*client, &at, &child, "wsp merge in", cloud_on_in(&ctx.env)).await?;
        let lead_ask = MergeLead { workspace: at.1.clone(), thread_id: thread_id_of(&at.0) };
        let child_ask = ChildRef { workspace_id: kid.1.id.clone(), thread_id: thread_id_of(&kid.0) };
        let say = stderr_of(ctx);
        let done = merged_in(&*client, &lead_ask, &child_ask, &say).await?;
        let said = MergeInResult { lead: thread_label(&at.0), child: thread_label(&kid.0), ..done.clone() };
        ctx.out.emit(&json_of(&done), Some(&merge_in_line(&said, Some(&thread_id_of(&kid.0)))));
        Ok(0)
    })
}

/// The `merge` tool's call (`verbs/workspaces.ts:313-318`).
pub(super) fn call_merge(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let asked = frame_str(&args, "method");
        let method = asked.as_deref().and_then(merge_method_of);
        if let (Some(word), None) = (&asked, &method) {
            return Ok(off_the_list("merge", "method", &["merge", "squash", "rebase"], word));
        }
        let client = deps.client(None).await?;
        let at = thread_at_in(&*client, &frame_str(&args, "thread").unwrap_or_default(), "wsp merge", false, cloud_on_in(&deps.env)).await?;
        let merged = merged_pr(&*client, &at.1.id, method, frame_true(&args, "when_checks_pass"), &thread_id_of(&at.0)).await?;
        Ok(as_text(&merged_line(&thread_label(&at.0), &merged), json_of(&merged)))
    })
}

/// `wsp merge` (`verbs/workspaces.ts:292-303`).
pub(super) fn run_merge(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp merge takes one thread.", &usage_is(ctx))),
        };
        let method = match flag(&ctx.flags, "method") {
            None => None,
            Some(named) => Some(
                merge_method_of(named)
                    .ok_or_else(|| usage_refusal(&format!("--method takes merge, squash or rebase, and got {named}."), &usage_is(ctx)))?,
            ),
        };
        let client = ctx.client(None).await?;
        let at = thread_at_in(&*client, &reference, "wsp merge", false, cloud_on_in(&ctx.env)).await?;
        let merged = merged_pr(&*client, &at.1.id, method, flag_set(&ctx.flags, "when-checks-pass"), &thread_id_of(&at.0)).await?;
        ctx.out.emit(&json_of(&merged), Some(&merged_line(&thread_label(&at.0), &merged)));
        Ok(0)
    })
}

/// The `update` tool's call (`verbs/workspaces.ts:441-448`).
pub(super) fn call_update(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let at = thread_at_in(&*client, &frame_str(&args, "thread").unwrap_or_default(), "wsp update", false, cloud_on_in(&deps.env)).await?;
        let woke = awake(&*client, &view_of(&at.1)?, "update", &quiet_tell).await?;
        let done: GitUpdateReply = parsed(
            client
                .request(
                    "workspaces.update",
                    pick(vec![("workspaceId", Some(json!(woke.workspace.id))), ("threadId", Some(json!(thread_id_of(&at.0))))]),
                )
                .await?,
            "workspaces.update",
        )?;
        let shared = sharing_with(&*client, &at).await?;
        let line = with_shared(&update_line(&thread_label(&at.0), &done), &shared);
        Ok(as_text(&line, with_shared_field(object_of(&done), &shared)))
    })
}

/// `wsp update` (`verbs/workspaces.ts:425-435`).
pub(super) fn run_update(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp update takes one thread.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let at = thread_at_in(&*client, &reference, "wsp update", false, cloud_on_in(&ctx.env)).await?;
        let say = stderr_of(ctx);
        let woke = awake(&*client, &view_of(&at.1)?, "update", &say).await?;
        let done: GitUpdateReply = parsed(
            client
                .request(
                    "workspaces.update",
                    pick(vec![("workspaceId", Some(json!(woke.workspace.id))), ("threadId", Some(json!(thread_id_of(&at.0))))]),
                )
                .await?,
            "workspaces.update",
        )?;
        let shared = sharing_with(&*client, &at).await?;
        let line = with_shared(&update_line(&thread_label(&at.0), &done), &shared);
        ctx.out.emit(&with_shared_field(object_of(&done), &shared), Some(&line));
        Ok(0)
    })
}
