// SPDX-License-Identifier: AGPL-3.0-only
//! The GitHub starts, the machine verbs, the image and `wsp delete` (`verbs/workspaces.ts:322-711`): start, review,
//! review post, pause, resize, wake, rebuild, image, image build, forget and delete. Spec: `spec/workspaces-life.md`.
//! The verb table is in the parent module.

use super::*;

fn opened_line(thread_id: &str) -> String {
    opened_thread_line(thread_id, None, None)
}

/// A start's answer as the host wrote it (`startedFrom` and `reviewStarted`, workspaces-help.ts:402-413): its
/// `workspace` first, then the thread and session ids, with the typed read the lines take.
async fn started_answer(client: &dyn HostClient, op: &str, asked: Frame) -> Result<(Value, StartResult), HostError> {
    let reply = client.request(op, asked).await?;
    let answer = pick(vec![
        ("workspace", reply.get("workspace").cloned()),
        ("threadId", reply.get("threadId").cloned()),
        ("sessionId", reply.get("sessionId").cloned()),
    ]);
    let typed = serde_json::from_value(Value::Object(answer.clone())).map_err(|_| HostError::new(other_version(op)))?;
    Ok((Value::Object(answer), typed))
}

/// What `startedFrom` asks, the fields given in the order url, project, agent, model, effort, access.
fn started_frame(ask: &StartedFromAsk) -> Result<Frame, HostError> {
    let access = match &ask.access {
        Some(given) => Some(json_of(&access_word_of(given)?)),
        None => None,
    };
    Ok(pick(vec![
        ("url", Some(json!(ask.url))),
        ("project", ask.project.as_ref().map(|v| json!(v))),
        ("agent", ask.agent.as_ref().map(|v| json!(v))),
        ("model", ask.model.as_ref().map(|v| json!(v))),
        ("effort", ask.effort.as_ref().map(|v| json!(v))),
        ("access", access),
    ]))
}

/// What `reviewStarted` asks, the fields given in the order url, workspaceId, agent, model, effort.
fn review_frame(ask: &ReviewStartedAsk) -> Frame {
    pick(vec![
        ("url", ask.url.as_ref().map(|v| json!(v))),
        ("workspaceId", ask.workspace_id.as_ref().map(|v| json!(v))),
        ("agent", ask.agent.as_ref().map(|v| json!(v))),
        ("model", ask.model.as_ref().map(|v| json!(v))),
        ("effort", ask.effort.as_ref().map(|v| json!(v))),
    ])
}

/// `nap` (workspaces-help.ts:202-205) with the record kept as the host wrote it, and its typed read.
async fn nap_answer(client: &dyn HostClient, reference: &str) -> Result<(Value, WorkspaceOut), HostError> {
    let source = workspace_of(client, reference).await?;
    let reply = client.request("workspaces.nap", pick(vec![("workspaceId", Some(json!(source.id)))])).await?;
    let workspace = reply.get("workspace").cloned().unwrap_or(Value::Null);
    let typed = serde_json::from_value(workspace.clone()).map_err(|_| HostError::new(other_version("workspaces.nap")))?;
    Ok((workspace, typed))
}

/// `resize` (workspaces-help.ts:209-212) with the record kept as the host wrote it, and its typed read.
async fn resize_answer(client: &dyn HostClient, reference: &str, size: &WorkspaceSize) -> Result<(Value, WorkspaceOut), HostError> {
    let thread = thread_of(client, reference).await?;
    let reply = client
        .request("workspaces.resize", pick(vec![("workspaceId", Some(json!(thread.workspace_id))), ("size", Some(json_of(size)))]))
        .await?;
    let workspace = reply.get("workspace").cloned().unwrap_or(Value::Null);
    let typed = serde_json::from_value(workspace.clone()).map_err(|_| HostError::new(other_version("workspaces.resize")))?;
    Ok((workspace, typed))
}

/// The `start` tool's call (`verbs/workspaces.ts:345-348`).
pub(super) fn call_start(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let ask = StartedFromAsk {
            url: frame_str(&args, "link").unwrap_or_default(),
            project: frame_str(&args, "project"),
            agent: frame_str(&args, "agent"),
            model: frame_str(&args, "model"),
            effort: frame_str(&args, "effort"),
            access: frame_str(&args, "access"),
        };
        let (answer, started) = started_answer(&*client, "workspaces.start", started_frame(&ask)?).await?;
        let said: &(dyn Fn(&str) -> String + Send + Sync) = &opened_line;
        Ok(as_text(&started_lines(&started, Some(said)), answer))
    })
}

/// `wsp start` (`verbs/workspaces.ts:327-334`).
pub(super) fn run_start(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let url = match ctx.args.as_slice() {
            [url] => url.clone(),
            _ => return Err(usage_refusal("wsp start takes one link.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let ask = StartedFromAsk {
            url,
            project: flag(&ctx.flags, "project").map(str::to_owned),
            agent: flag(&ctx.flags, "agent").map(str::to_owned),
            model: flag(&ctx.flags, "model").map(str::to_owned),
            effort: flag(&ctx.flags, "effort").map(str::to_owned),
            access: flag(&ctx.flags, "access").map(str::to_owned),
        };
        let (answer, started) = started_answer(&*client, "workspaces.start", started_frame(&ask)?).await?;
        ctx.out.emit(&answer, Some(&started_lines(&started, None)));
        Ok(0)
    })
}

/// The `review` tool's call (`verbs/workspaces.ts:376-381`).
pub(super) fn call_review(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let target = frame_str(&args, "target").unwrap_or_default();
        let (url, workspace_id) = if is_link(&target) {
            (Some(target), None)
        } else {
            (None, Some(thread_at(&*client, &target, "wsp review", false).await?.1.id))
        };
        let ask = ReviewStartedAsk {
            url,
            workspace_id,
            agent: frame_str(&args, "agent"),
            model: frame_str(&args, "model"),
            effort: frame_str(&args, "effort"),
        };
        let (answer, started) = started_answer(&*client, "workspaces.review", review_frame(&ask)).await?;
        let said: &(dyn Fn(&str) -> String + Send + Sync) = &opened_line;
        Ok(as_text(&review_lines(&started, Some(said)), answer))
    })
}

/// `wsp review` (`verbs/workspaces.ts:357-364`).
pub(super) fn run_review(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let target = match ctx.args.as_slice() {
            [target] => target.clone(),
            _ => return Err(usage_refusal("wsp review takes one pull request link or thread.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let (url, workspace_id) = if is_link(&target) {
            (Some(target), None)
        } else {
            (None, Some(thread_at(&*client, &target, "wsp review", false).await?.1.id))
        };
        let ask = ReviewStartedAsk {
            url,
            workspace_id,
            agent: flag(&ctx.flags, "agent").map(str::to_owned),
            model: flag(&ctx.flags, "model").map(str::to_owned),
            effort: flag(&ctx.flags, "effort").map(str::to_owned),
        };
        let (answer, started) = started_answer(&*client, "workspaces.review", review_frame(&ask)).await?;
        ctx.out.emit(&answer, Some(&review_lines(&started, None)));
        Ok(0)
    })
}

/// The `review_post` tool's call (`verbs/workspaces.ts:411-416`).
pub(super) fn call_review_post(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let at = thread_at(&*client, &frame_str(&args, "thread").unwrap_or_default(), "wsp review post", false).await?;
        let edits =
            ReviewEdits { verdict: frame_str(&args, "verdict").and_then(|word| verdict_of(&word)), summary: frame_str(&args, "summary") };
        let posted = review_posted(&*client, &at.1.id, &edits, &thread_id_of(&at.0)).await?;
        Ok(as_text(&posted_line(&thread_label(&at.0), &posted), json_of(&posted)))
    })
}

fn posted_line(label: &str, posted: &ReviewPostResult) -> String {
    start_words::posted(label, posted.number.as_f64(), posted.comments.as_f64(), posted.folded.as_f64())
}

/// `wsp review post` (`verbs/workspaces.ts:390-401`).
pub(super) fn run_review_post(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp review post takes one thread.", &usage_is(ctx))),
        };
        let verdict = match flag(&ctx.flags, "verdict") {
            None => None,
            Some(named) => Some(verdict_of(named).ok_or_else(|| {
                usage_refusal(&format!("--verdict takes comment, approve or request-changes, and got {named}."), &usage_is(ctx))
            })?),
        };
        let client = ctx.client(None).await?;
        let at = thread_at(&*client, &reference, "wsp review post", false).await?;
        let edits = ReviewEdits { verdict, summary: flag(&ctx.flags, "summary").map(str::to_owned) };
        let posted = review_posted(&*client, &at.1.id, &edits, &thread_id_of(&at.0)).await?;
        ctx.out.emit(&json_of(&posted), Some(&posted_line(&thread_label(&at.0), &posted)));
        Ok(0)
    })
}

/// The `pause` tool's call (`verbs/workspaces.ts:470-470`).
pub(super) fn call_pause(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let (workspace, _) = nap_answer(&*client, &frame_str(&args, "workspace").unwrap_or_default()).await?;
        Ok(as_json(json!({ "workspace": workspace })))
    })
}

/// `wsp pause` (`verbs/workspaces.ts:458-465`).
pub(super) fn run_pause(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp pause takes one workspace.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let (workspace, typed) = nap_answer(&*client, &reference).await?;
        ctx.out.emit(&json!({ "workspace": workspace }), Some(&state_line(&view_of(&typed)?)));
        Ok(0)
    })
}

/// The `resize` tool's call (`verbs/workspaces.ts:494-498`).
pub(super) fn call_resize(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let word = frame_str(&args, "size").unwrap_or_default();
        let size = size_from_word(&word).ok_or_else(|| usage_refusal(&resize_word_line(&word), RESIZE_WORD_FIX))?;
        let client = deps.client(None).await?;
        let (workspace, _) = resize_answer(&*client, &frame_str(&args, "thread").unwrap_or_default(), &size).await?;
        Ok(as_json(json!({ "workspace": workspace })))
    })
}

/// `wsp resize` (`verbs/workspaces.ts:479-488`).
pub(super) fn run_resize(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (reference, word) = match (ctx.args.as_slice(), flag(&ctx.flags, "size")) {
            ([reference], Some(word)) => (reference.clone(), word.to_owned()),
            _ => return Err(usage_refusal("wsp resize takes one thread and --size.", &usage_is(ctx))),
        };
        let size = size_from_word(&word).ok_or_else(|| usage_refusal(&resize_word_line(&word), RESIZE_WORD_FIX))?;
        let client = ctx.client(None).await?;
        let (workspace, typed) = resize_answer(&*client, &reference, &size).await?;
        ctx.out.emit(&json!({ "workspace": workspace }), Some(&resized_line(&typed.name, &size)));
        Ok(0)
    })
}

/// The `wake` tool's call (`verbs/workspaces.ts:520-523`).
pub(super) fn call_wake(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let found = workspace_of(&*client, &frame_str(&args, "workspace").unwrap_or_default()).await?;
        let woke = awake(&*client, &view_of(&found)?, "wake", &quiet_tell).await?;
        Ok(as_json(json!({ "workspace": json_of(&woke.workspace) })))
    })
}

/// `wsp wake` (`verbs/workspaces.ts:508-515`).
pub(super) fn run_wake(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp wake takes one workspace.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let found = workspace_of(&*client, &reference).await?;
        let say = stderr_of(ctx);
        let woke = awake(&*client, &view_of(&found)?, "wake", &say).await?;
        let line = woke_line(&*client, &woke.workspace).await?;
        ctx.out.emit(&json!({ "workspace": json_of(&woke.workspace) }), Some(&line));
        Ok(0)
    })
}

/// The `rebuild` tool's call (`verbs/workspaces.ts:545-548`).
pub(super) fn call_rebuild(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let workspace = rebuild(&*client, &frame_str(&args, "workspace").unwrap_or_default()).await?;
        Ok(as_text(&rebuilt_line(&view_of(&workspace)?), json!({ "workspace": json_of(&workspace) })))
    })
}

/// `wsp rebuild` (`verbs/workspaces.ts:533-539`).
pub(super) fn run_rebuild(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp rebuild takes one workspace.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let workspace = rebuild(&*client, &reference).await?;
        ctx.out.emit(&json!({ "workspace": json_of(&workspace) }), Some(&rebuilt_line(&view_of(&workspace)?)));
        Ok(0)
    })
}

/// The `image` tool's call (`verbs/workspaces.ts:568-571`).
pub(super) fn call_image(_args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let view = image_view(&*client).await?;
        Ok(as_text(&image_lines(&view).join("\n"), json_of(&view)))
    })
}

/// `wsp image` (`verbs/workspaces.ts:557-562`).
pub(super) fn run_image(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if !ctx.args.is_empty() {
            return Err(usage_refusal("wsp image takes no positional arguments.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let view = image_view(&*client).await?;
        ctx.out.emit(&json_of(&view), Some(&image_lines(&view).join("\n")));
        Ok(0)
    })
}

/// The `image_build` tool's call (`verbs/workspaces.ts:592-595`).
pub(super) fn call_image_build(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let force = args.get("force").and_then(Value::as_bool);
        let built = build_image_at(&*client, &Out::quiet(), &frame_str(&args, "place").unwrap_or_default(), force).await?;
        Ok(as_text(&sealed_built_line(&built.image, &built.built.copy, built.built.built), json_of(&built.built)))
    })
}

/// `wsp image build` (`verbs/workspaces.ts:580-586`).
pub(super) fn run_image_build(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let place = match ctx.args.as_slice() {
            [place] => place.clone(),
            _ => return Err(usage_refusal("wsp image build takes one place.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let built = build_image_at(&*client, &ctx.out, &place, Some(flag_set(&ctx.flags, "force"))).await?;
        ctx.out.emit(&json_of(&built.built), Some(&sealed_built_line(&built.image, &built.built.copy, built.built.built)));
        Ok(0)
    })
}

/// The `forget` tool's call (`verbs/workspaces.ts:620-625`).
pub(super) fn call_forget(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let f = dropping(&*client, &frame_str(&args, "workspace").unwrap_or_default()).await?;
        forget(&*client, &f).await?;
        let going = json!({ "workspaceId": f.workspace.id, "name": f.workspace.name, "threads": f.threads });
        Ok(as_text(&forgot_line(&f), going))
    })
}

/// `wsp forget` (`verbs/workspaces.ts:605-614`).
pub(super) fn run_forget(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp forget takes one workspace.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let f = dropping(&*client, &reference).await?;
        if !confirmed(ctx, &forget_question(&f), &f.workspace.name).await? {
            return Ok(1);
        }
        forget(&*client, &f).await?;
        ctx.out.emit(&json!({ "workspaceId": f.workspace.id, "name": f.workspace.name, "threads": f.threads }), Some(&forgot_line(&f)));
        Ok(0)
    })
}

/// The `delete` tool's call (`verbs/workspaces.ts:675-708`).
pub(super) fn call_delete(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let confirm = frame_true(&args, "confirm");
        let processes = frame_str(&args, "processes");
        if let Some(reference) = frame_str(&args, "thread") {
            let thread = thread_of(&*client, &reference).await.ok();
            if let Some(sub) = thread.as_ref().filter(|t| t.sub.is_some()) {
                let id = thread_id_of(sub);
                if !confirm {
                    let question = sub_deleting(&*client, Some(sub)).await?.and_then(|s| s.question).unwrap_or_default();
                    let text =
                        format!("thread {id} kept. {} Ask the person, then call delete again with confirm true.", second_line(&question));
                    return Ok(kept_result(text, json!({ "threadId": id, "workspaceId": sub.workspace_id, "threads": 1 })));
                }
                let gone = thread_deleted(&*client, &id, &ThreadDeletedAsk::default()).await?;
                return Ok(as_text(&sub_deleted_line(&id, gone.on.as_deref().unwrap_or("")), deleted_value(&id, &gone, false)));
            }
            let Some(here) = thread_here(&*client, thread.as_ref().map(ThreadRef::Row)).await? else {
                if cloud_on() {
                    refuse_machine_thread(&*client, &reference).await?;
                }
                return Err(usage_refusal(&format!("no thread {reference}"), NO_THREAD_FIX));
            };
            let asked = processes_ask(processes.as_deref());
            thread_deleted(&*client, &here.id, &ThreadDeletedAsk { processes: Some(asked), check: Some(true) }).await?;
            if !confirm {
                let question = thread_delete_question(&here.id, here.worktree.as_deref());
                let text = format!(
                    "thread {} kept. {} Ask the person, then call delete again with confirm true.",
                    here.id,
                    second_line(&question)
                );
                return Ok(kept_result(text, json!({ "threadId": here.id, "workspaceId": here.workspace_id, "threads": 1 })));
            }
            let gone = thread_deleted(&*client, &here.id, &ThreadDeletedAsk { processes: Some(asked), check: None }).await?;
            let mut lines = ended_lines_of(&gone);
            lines.push(thread_deleted_line(&here.id, gone.worktree.as_deref(), gone.threads as f64));
            return Ok(as_text(&lines.join("\n"), deleted_value(&here.id, &gone, true)));
        }
        let Some(reference) = frame_str(&args, "workspace") else {
            return Err(usage_refusal(
                &cloud_text("delete takes a thread<!-- cloud -->, or a workspace<!-- /cloud -->.", cloud_on()),
                "Name one.",
            ));
        };
        let d = deleting(&*client, &reference).await?;
        let going = workspace_gone_value(&d);
        // The command line asks a person before this and the app will; over MCP the second call is that step, so a
        // machine is never killed by one tool call the caller made on its own.
        if !confirm {
            let notice = delete_notice(
                d.threads as f64,
                &workspace_kind(d.workspace.kind.as_ref()),
                made_worktree(d.workspace.worktree.as_ref()).as_ref(),
                Some(&d.workspace.machine_id),
                d.on.as_ref(),
            );
            let text = format!("{} kept. {notice} Ask the person, then call delete again with confirm true.", d.workspace.name);
            return Ok(kept_result(text, going));
        }
        delete_workspace(&*client, &d).await?;
        Ok(as_text(&deleted_line(&d), going))
    })
}

/// `wsp delete` (`verbs/workspaces.ts:636-669`).
pub(super) fn run_delete(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => {
                return Err(usage_refusal(
                    &cloud_text("wsp delete takes one thread<!-- cloud -->, or one workspace<!-- /cloud -->.", cloud_on()),
                    &usage_is(ctx),
                ));
            }
        };
        let choice = processes_asked(flag(&ctx.flags, "processes"), None)?;
        let client = ctx.client(None).await?;
        let thread = thread_of(&*client, &reference).await.ok();
        if let Some(subbed) = sub_deleting(&*client, thread.as_ref()).await? {
            if !confirmed(ctx, subbed.question.as_deref().unwrap_or_default(), &subbed.id).await? {
                return Ok(1);
            }
            let gone = thread_deleted(&*client, &subbed.id, &ThreadDeletedAsk::default()).await?;
            ctx.out.emit(&deleted_value(&subbed.id, &gone, false), Some(&sub_deleted_line(&subbed.id, gone.on.as_deref().unwrap_or(""))));
            return Ok(0);
        }
        if let Some(here) = thread_here(&*client, thread.as_ref().map(ThreadRef::Row)).await? {
            let processes = Some(processes_ask(choice.as_deref()));
            thread_deleted(&*client, &here.id, &ThreadDeletedAsk { processes, check: Some(true) }).await?;
            if !confirmed(ctx, &thread_delete_question(&here.id, here.worktree.as_deref()), &here.id).await? {
                return Ok(1);
            }
            let gone = thread_deleted(&*client, &here.id, &ThreadDeletedAsk { processes, check: None }).await?;
            let mut lines = ended_lines_of(&gone);
            lines.push(thread_deleted_line(&here.id, gone.worktree.as_deref(), gone.threads as f64));
            ctx.out.emit(&deleted_value(&here.id, &gone, true), Some(&lines.join("\n")));
            return Ok(0);
        }
        if !cloud_on() {
            return Err(usage_refusal(&format!("no thread {reference}"), NO_THREAD_FIX));
        }
        // A workspace by that name comes first; a ref that names none but is a thread on a box's machine says how that
        // thread goes, rather than that no workspace has its id.
        let d = match deleting(&*client, &reference).await {
            Ok(d) => d,
            Err(e) => {
                refuse_machine_thread(&*client, &reference).await?;
                return Err(e);
            }
        };
        if !confirmed(ctx, &delete_question(&d), &d.workspace.name).await? {
            return Ok(1);
        }
        delete_workspace(&*client, &d).await?;
        ctx.out.emit(&workspace_gone_value(&d), Some(&deleted_line(&d)));
        Ok(0)
    })
}
