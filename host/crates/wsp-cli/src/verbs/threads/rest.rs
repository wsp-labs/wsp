// SPDX-License-Identifier: AGPL-3.0-only
//! The thread verbs from worktree to export (threads.ts:225-711), split from `threads.rs` for its length. Spec:
//! `spec/threads-a.md` and `spec/threads-b.md`.

use super::*;

/// `worktree`'s run (threads.ts:225).
pub(super) fn run_worktree(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (project, branch) = match ctx.args.as_slice() {
            [project, branch] => (project.clone(), branch.clone()),
            _ => return Err(usage_refusal("wsp worktree takes a project and a branch.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let made = worktree_for(&*client, &project, &branch).await?;
        ctx.out.emit(&json_of(&made), Some(&made.path));
        Ok(0)
    })
}

/// `worktree`'s tool call (threads.ts:237).
pub(super) fn call_worktree(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let project = frame_str(&args, "project").unwrap_or_default();
        let branch = frame_str(&args, "branch").unwrap_or_default();
        let made = worktree_for(&*client, &project, &branch).await?;
        Ok(as_text(&made.path, json_of(&made)))
    })
}

/// `worktree remove`'s run (threads.ts:249-259).
pub(super) fn run_worktree_remove(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (project, branch) = match ctx.args.as_slice() {
            [project, branch] => (project.clone(), branch.clone()),
            _ => return Err(usage_refusal("wsp worktree remove takes a project and a branch.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let force = flag_set(&ctx.flags, "force");
        let mut asked = frame(&[("project", json!(project)), ("branch", json!(branch))]);
        if force {
            asked.insert("force".to_owned(), json!(true));
        }
        let mut checked = asked.clone();
        checked.insert("check".to_owned(), json!(true));
        client.request("worktree.remove", checked).await?;
        let question = format!(
            "Remove the worktree for {branch} in {project}?\nThe branch stays{}.",
            if force { ", and the files no commit holds there go" } else { "" }
        );
        if !confirmed(ctx, &question, &branch).await? {
            return Ok(1);
        }
        client.request("worktree.remove", asked).await?;
        ctx.out.emit(&json!({ "project": project, "branch": branch, "removed": true }), Some(&worktree_removed_line(&branch)));
        Ok(0)
    })
}

/// `worktree remove`'s tool call (threads.ts:266-268).
pub(super) fn call_worktree_remove(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let project = frame_str(&args, "project").unwrap_or_default();
        let branch = frame_str(&args, "branch").unwrap_or_default();
        let mut asked = frame(&[("project", json!(project)), ("branch", json!(branch))]);
        if frame_bool(&args, "force") {
            asked.insert("force".to_owned(), json!(true));
        }
        deps.client(None).await?.request("worktree.remove", asked).await?;
        Ok(as_text(&worktree_removed_line(&branch), json!({ "project": project, "branch": branch, "removed": true })))
    })
}

/// `thread read`'s run (threads.ts:279-287).
pub(super) fn run_thread_read(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp thread read takes one thread.", &usage_is(ctx))),
        };
        let last = flag_set(&ctx.flags, "last");
        let client = ctx.client(None).await?;
        let thread = thread_of(&*client, &reference).await?;
        let read = read_thread(&*client, &thread, last).await?;
        ctx.out.emit(&read_json(&read), Some(&read_line(&read, last, &local_offset)));
        Ok(0)
    })
}

/// `thread read`'s tool call (threads.ts:296-300).
pub(super) fn call_thread_read(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let thread = thread_of(&*client, &frame_str(&args, "thread").unwrap_or_default()).await?;
        let last = frame_bool(&args, "last");
        let read = read_thread(&*client, &thread, last).await?;
        Ok(as_text(&read_line(&read, last, &local_offset), read_json(&read)))
    })
}

/// `thread head`'s run (threads.ts:310-316).
pub(super) fn run_thread_head(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp thread head takes one thread.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let thread = thread_of(&*client, &reference).await?;
        let head = thread_head(&*client, &thread).await?;
        ctx.out.emit(&json_of(&head), Some(&head_line(&head, &local_offset)));
        Ok(0)
    })
}

/// `thread head`'s tool call (threads.ts:323-326).
pub(super) fn call_thread_head(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let thread = thread_of(&*client, &frame_str(&args, "thread").unwrap_or_default()).await?;
        let head = thread_head(&*client, &thread).await?;
        Ok(as_text(&head_line(&head, &local_offset), json_of(&head)))
    })
}

/// `thread rename`'s run (threads.ts:336-344).
pub(super) fn run_thread_rename(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (reference, title) = match ctx.args.as_slice() {
            [reference, title] => (reference.clone(), title.clone()),
            _ => return Err(usage_refusal("wsp thread rename takes a thread and one name.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let thread = thread_of(&*client, &reference).await?;
        let workspace = workspace_of(&*client, &thread.workspace_id).await?;
        let say = |line: &str| ctx.io.error(line);
        awake(&*client, &view_of(&workspace)?, "rename", &say).await?;
        let renamed = rename(&*client, &thread, &title).await?;
        ctx.out.emit(&json_of(&renamed), Some(&rename_line(&renamed)));
        Ok(0)
    })
}

/// `thread rename`'s tool call (threads.ts:351-356).
pub(super) fn call_thread_rename(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let thread = thread_of(&*client, &frame_str(&args, "thread").unwrap_or_default()).await?;
        let workspace = workspace_of(&*client, &thread.workspace_id).await?;
        let quiet = |_: &str| {};
        awake(&*client, &view_of(&workspace)?, "rename", &quiet).await?;
        let renamed = rename(&*client, &thread, &frame_str(&args, "title").unwrap_or_default()).await?;
        Ok(as_text(&rename_line(&renamed), json_of(&renamed)))
    })
}

/// `thread forget`'s run (threads.ts:366-375).
pub(super) fn run_thread_forget(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp thread forget takes one thread.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let thread = thread_of(&*client, &reference).await?;
        forget_thread(&*client, &thread, true).await?;
        let question = format!("Forget thread {}?\nIts row leaves this computer and the sidebar.", thread.id);
        if !confirmed(ctx, &question, &thread.id).await? {
            return Ok(1);
        }
        forget_thread(&*client, &thread, false).await?;
        ctx.out.emit(&json!({ "threadId": thread.id, "workspaceId": thread.workspace_id }), Some(&thread_forgot_line(&thread)));
        Ok(0)
    })
}

/// `thread forget`'s tool call (threads.ts:382-386).
pub(super) fn call_thread_forget(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let thread = thread_of(&*client, &frame_str(&args, "thread").unwrap_or_default()).await?;
        forget_thread(&*client, &thread, false).await?;
        Ok(as_text(&thread_forgot_line(&thread), json!({ "threadId": thread.id, "workspaceId": thread.workspace_id })))
    })
}

/// The processes an answer names, as the settle reads them: the words `processes_asked` checked, `ask` where none.
pub(super) fn processes_ask(word: Option<&str>) -> ProcessesAsk {
    match word {
        Some("stop") => ProcessesAsk::Stop,
        Some("keep") => ProcessesAsk::Keep,
        _ => ProcessesAsk::Ask,
    }
}

/// The lines a settle answers with: each process that ended, then what settled (threads.ts:403, 419).
pub(super) fn settle_lines(answer: &SessionSettleResult) -> String {
    let mut lines: Vec<String> = answer.ended.iter().flatten().map(ended_lines).collect();
    lines.push(settled_lines(answer));
    lines.join("\n")
}

/// `thread settle`'s run (threads.ts:396-404).
pub(super) fn run_thread_settle(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            return Err(usage_refusal("wsp thread settle takes one thread or more.", &usage_is(ctx)));
        }
        let choice = processes_asked(flag(&ctx.flags, "processes"), None)?;
        let client = ctx.client(None).await?;
        let threads = threads_of(&*client, &ctx.args).await?;
        let finished = flag_set(&ctx.flags, "finished");
        let answer = settle_threads(&*client, &threads, finished, Some(processes_ask(choice.as_deref()))).await?;
        ctx.out.emit(&json_of(&answer), Some(&settle_lines(&answer)));
        Ok(0)
    })
}

/// `thread settle`'s tool call (threads.ts:415-419).
pub(super) fn call_thread_settle(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let threads = threads_of(&*client, &frame_list(&args, "threads")).await?;
        let processes = frame_str(&args, "processes");
        let answer = settle_threads(&*client, &threads, frame_bool(&args, "finished"), Some(processes_ask(processes.as_deref()))).await?;
        Ok(as_text(&settle_lines(&answer), json_of(&answer)))
    })
}

/// `thread restore`'s run (threads.ts:429-435).
pub(super) fn run_thread_restore(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            return Err(usage_refusal("wsp thread restore takes one thread or more.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let answer = restore_threads(&*client, &threads_of(&*client, &ctx.args).await?).await?;
        ctx.out.emit(&json_of(&answer), Some(&restored_lines(&answer)));
        Ok(0)
    })
}

/// `thread restore`'s tool call (threads.ts:441-444).
pub(super) fn call_thread_restore(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let answer = restore_threads(&*client, &threads_of(&*client, &frame_list(&args, "threads")).await?).await?;
        Ok(as_text(&restored_lines(&answer), json_of(&answer)))
    })
}

/// The road of an answer verb: the one `ANSWER_ROADS` row whose verb is `verb`.
pub(super) fn answer_road(verb: &str) -> &'static AnswerRoad {
    answer_roads().iter().find(|road| road.answer.as_ref().is_some_and(|answer| answer.verb == verb)).expect("a road for each answer verb")
}

/// `thread allow`/`thread deny`'s run (ANSWER_VERBS, threads.ts:62-68).
async fn answer_run(ctx: &VerbContext, verb: &str) -> Result<i32, HostError> {
    let reference = match ctx.args.as_slice() {
        [reference] => reference.clone(),
        _ => return Err(usage_refusal(&format!("wsp thread {verb} takes one thread."), &usage_is(ctx))),
    };
    let client = ctx.client(None).await?;
    let answered = answer_open_ask(&*client, &reference, answer_road(verb), flag(&ctx.flags, "reason")).await?;
    ctx.out
        .emit(&json!({ "threadId": answered.thread_id, "askId": answered.ask_id, "optionId": answered.option_id }), Some(&answered.line));
    Ok(0)
}

/// `thread allow`/`thread deny`'s tool call (threads.ts:76-79).
async fn answer_call(args: &Frame, deps: &VerbDeps, verb: &str) -> Result<CallToolResult, HostError> {
    let client = deps.client(None).await?;
    let reason = frame_str(args, "reason");
    let answered = answer_open_ask(&*client, &frame_str(args, "thread").unwrap_or_default(), answer_road(verb), reason.as_deref()).await?;
    Ok(as_text(&answered.line, json!({ "threadId": answered.thread_id, "askId": answered.ask_id, "optionId": answered.option_id })))
}

pub(super) fn run_thread_allow(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { answer_run(ctx, "allow").await })
}

pub(super) fn call_thread_allow(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move { answer_call(&args, deps, "allow").await })
}

pub(super) fn run_thread_deny(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move { answer_run(ctx, "deny").await })
}

pub(super) fn call_thread_deny(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move { answer_call(&args, deps, "deny").await })
}

/// `send`'s run (threads.ts:456-470).
pub(super) fn run_send(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (reference, message) = match ctx.args.as_slice() {
            [reference, message] => (reference.clone(), message.clone()),
            _ => return Err(usage_refusal("wsp send takes a thread and one message.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let picks = picks_from(ctx);
        let say = |line: &str| ctx.io.error(line);
        let (thread, woken) = before_sending(
            &*client,
            Box::pin(async {
                let thread = thread_of(&*client, &reference).await?;
                checked_start(&*client, &message, Some(&thread.harness), &picks, Some(&thread.workspace_id), None).await?;
                let workspace = workspace_of(&*client, &thread.workspace_id).await?;
                let woken = awake(&*client, &view_of(&workspace)?, "send", &say).await?;
                Ok((thread, woken))
            }),
        )
        .await?;
        let files = flag_list(&ctx.flags, "file");
        let start = message_to(&thread, &message, &picks, &files, ctx.elsewhere)?;
        let said = woken.said.as_deref();
        if flag_set(&ctx.flags, "detach") {
            detach_verb(ctx, client.clone(), &start, &picks, None, None, said).await?;
        } else {
            let spend = spend_of(Some(&woken));
            let turn = follow_verb(ctx, client.clone(), &start, false, &picks, &SaidAbout { opened: None, spend }, None).await?;
            ctx.out.emit(&noticed_value(turn_view(&turn), said), None);
        }
        Ok(0)
    })
}

/// `send`'s tool call (threads.ts:475-485).
pub(super) fn call_send(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let reference = frame_str(&args, "thread").unwrap_or_default();
        let message = frame_str(&args, "message").unwrap_or_default();
        let picks = frame_picks(&args);
        let files = frame_list(&args, "files");
        let quiet = |_: &str| {};
        let (thread, said) = before_sending(
            &*client,
            Box::pin(async {
                let thread = thread_of(&*client, &reference).await?;
                checked_start(&*client, &message, Some(&thread.harness), &picks, Some(&thread.workspace_id), None).await?;
                let workspace = workspace_of(&*client, &thread.workspace_id).await?;
                let woken = awake(&*client, &view_of(&workspace)?, "send", &quiet).await?;
                Ok((thread, woken.said))
            }),
        )
        .await?;
        let redial = redial_of(deps);
        let start = message_to(&thread, &message, &picks, &files, deps.elsewhere)?;
        if frame_bool(&args, "detach") {
            let turn = start_detached(client.clone(), &start, SessionOrigin::Agent, None, Some(&redial)).await?;
            return Ok(detached_out(&turn, None, None, said.as_deref()));
        }
        let turn = follow(client.clone(), &start, SessionOrigin::Agent, &FollowOn::default(), Some(&redial)).await?;
        let out = noticed(turn_out(&turn)?, said.as_deref());
        Ok(as_text(&turn_text(&out), json_of(&out)))
    })
}

/// `restart`'s run (threads.ts:495-500).
pub(super) fn run_restart(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if !ctx.args.is_empty() {
            return Err(usage_refusal("wsp restart takes no arguments.", &usage_is(ctx)));
        }
        let running = restart_host(&ctx.deps).await?;
        ctx.out.emit(&json!({ "running": running }), Some(&host_restarted_line(&running)));
        Ok(0)
    })
}

/// `restart`'s tool call (threads.ts:506-508).
pub(super) fn call_restart(_args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let running = restart_host(deps).await?;
        Ok(as_text(&host_restarted_line(&running), json!({ "running": running })))
    })
}

/// `stop`'s run (threads.ts:518-524).
pub(super) fn run_stop(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let reference = match ctx.args.as_slice() {
            [reference] => reference.clone(),
            _ => return Err(usage_refusal("wsp stop takes one thread.", &usage_is(ctx))),
        };
        let client = ctx.client(None).await?;
        let stopped = stop(&*client, &reference, flag(&ctx.flags, "subagent")).await?;
        ctx.out.emit(&json_of(&stopped), Some(&stop_line(&stopped)));
        Ok(0)
    })
}

/// `stop`'s tool call (threads.ts:529-532).
pub(super) fn call_stop(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let client = deps.client(None).await?;
        let stopped = stop(&*client, &frame_str(&args, "thread").unwrap_or_default(), frame_str(&args, "subagent").as_deref()).await?;
        Ok(as_text(&stop_line(&stopped), json_of(&stopped)))
    })
}

/// `ssh`'s run (threads.ts:544-554): the bytes of this terminal, piped to the workspace's ssh port.
pub(super) fn run_ssh(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (reference, rest) = match ctx.args.split_first() {
            Some((reference, rest)) => (reference.clone(), rest),
            None => return Err(usage_refusal("wsp ssh takes one workspace.", &usage_is(ctx))),
        };
        if !rest.is_empty() {
            return Err(usage_refusal("wsp ssh takes one workspace.", &usage_is(ctx)));
        }
        let bytes = ctx.io.bytes();
        if bytes.is_none() || ctx.elsewhere {
            return Err(usage_refusal(SSH_PIPES_HERE_LINE, "Run it from an ssh client on that computer, as wsp's ssh config does."));
        }
        let client = ctx.client(None).await?;
        let workspace = ssh_workspace_of(&*client, &reference).await?;
        let reply = client.request("ssh.port", frame(&[("workspaceId", json!(workspace.id))])).await?;
        let port = reply
            .get("port")
            .and_then(Value::as_u64)
            .and_then(|port| u16::try_from(port).ok())
            .ok_or_else(|| HostError::new(other_version("ssh.port")))?;
        if let Some(bytes) = bytes {
            pipe_bytes(port, bytes).await?;
        }
        Ok(0)
    })
}

/// The `exec.exit` answer as `exec`'s own line carries it: `exitCode`, and `cwd` where the host named one.
pub(super) fn exec_answer(exit_code: Option<i64>, ran_in: Option<&str>) -> Map<String, Value> {
    let mut answer = Map::new();
    answer.insert("exitCode".to_owned(), json!(exit_code));
    if let Some(cwd) = ran_in {
        answer.insert("cwd".to_owned(), json!(cwd));
    }
    answer
}

/// `exec`'s run (threads.ts:562-577).
pub(super) fn run_exec(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (reference, words) = match ctx.args.split_first() {
            Some((reference, words)) if !words.is_empty() => (reference.clone(), words.to_vec()),
            _ => return Err(usage_refusal("wsp exec takes a thread, then -- and the command.", &usage_is(ctx))),
        };
        let given = absolute_folder(flag(&ctx.flags, "cwd"))?;
        let client = ctx.client(None).await?;
        let (thread, workspace) = thread_at(&*client, &reference, "wsp exec", false).await?;
        let say = |line: &str| ctx.io.error(line);
        let woken = awake(&*client, &view_of(&workspace)?, "exec", &say).await?;
        let folder = given.or_else(|| thread_cwd(thread.cwd.as_deref(), workspace.worktree.as_ref()));
        let out = ctx.out.clone();
        let sink: ExecSink = Arc::new(move |event: &ExecEvent| {
            if let ExecEvent::ExecOutput { text, .. } = event {
                out.emit(&json_of(event), Some(text));
            }
        });
        let run = exec_on(&*client, &woken.workspace.id, &words, folder.as_deref(), sink).await?;
        if let Some(error) = &run.exit.error {
            return Err(HostError::new(error.clone()));
        }
        let answer = exec_answer(run.exit.exit_code, run.ran_in.as_deref());
        ctx.out.emit(&Value::Object(answer), None);
        if run.exit.exit_code.is_some_and(|code| code != 0) {
            ctx.io.error(&exec_folder_line(run.ran_in.as_deref()));
        }
        Ok(run.exit.exit_code.map_or(ExitClass::Provider.code(), |code| code as i32))
    })
}

/// `exec`'s tool call (threads.ts:583-595).
pub(super) fn call_exec(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let given = absolute_folder(frame_str(&args, "cwd").as_deref())?;
        let client = deps.client(None).await?;
        let (thread, workspace) = thread_at(&*client, &frame_str(&args, "thread").unwrap_or_default(), "wsp exec", false).await?;
        let asked = given.or_else(|| thread_cwd(thread.cwd.as_deref(), workspace.worktree.as_ref()));
        let quiet = |_: &str| {};
        let woken = awake(&*client, &view_of(&workspace)?, "exec", &quiet).await?;
        let output: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let collected = output.clone();
        let sink: ExecSink = Arc::new(move |event: &ExecEvent| {
            if let ExecEvent::ExecOutput { text, .. } = event {
                collected.lock().unwrap().push(text.clone());
            }
        });
        let run = exec_on(&*client, &woken.workspace.id, &frame_list(&args, "argv"), asked.as_deref(), sink).await?;
        if let Some(error) = run.exit.error {
            return Err(HostError::new(error));
        }
        let output = output.lock().unwrap().clone();
        let mut structured = exec_answer(run.exit.exit_code, run.ran_in.as_deref());
        structured.insert("output".to_owned(), json!(output));
        let text = output.join("\n");
        Ok(as_text(&text, Value::Object(structured)))
    })
}

/// `folders`' run (threads.ts:604-611).
pub(super) fn run_folders(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.len() > 1 {
            return Err(usage_refusal("wsp folders takes one folder on this computer at most.", &usage_is(ctx)));
        }
        let asked = FoldersAsked {
            folder: ctx.args.first().cloned(),
            hidden: Some(flag_set(&ctx.flags, "hidden")),
            repos: Some(flag_set(&ctx.flags, "repos")),
            on: flag(&ctx.flags, "on").map(str::to_owned),
        };
        let cwd = here_cwd(ctx.cwd.as_deref());
        let client = ctx.client(None).await?;
        let here = |folder: &str| Ok::<String, HostError>(resolve_path(&cwd, folder));
        let listing = host_folders(&*client, &asked, &here).await?;
        ctx.out.emit(&json_of(&listing), Some(&folder_lines(&listing).join("\n")));
        Ok(0)
    })
}

/// `folders`' tool call (threads.ts:622-625).
pub(super) fn call_folders(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let asked = FoldersAsked {
            folder: frame_str(&args, "folder"),
            hidden: frame_bool_opt(&args, "hidden"),
            repos: frame_bool_opt(&args, "repos"),
            on: frame_str(&args, "on"),
        };
        let client = deps.client(None).await?;
        let listing = host_folders(&*client, &asked, &|folder: &str| absolute_path("folder is a path on this computer", folder)).await?;
        Ok(as_text(&folder_lines(&listing).join("\n"), json_of(&listing)))
    })
}

/// `setup`'s run (threads.ts:634-639).
pub(super) fn run_setup(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if !ctx.args.is_empty() {
            return Err(usage_refusal("wsp setup takes no positional arguments.", &usage_is(ctx)));
        }
        let client = ctx.client(None).await?;
        let setup = init_setup(&*client).await?;
        ctx.out.emit(&json!({ "setup": json_of(&setup) }), Some(&init_setup_lines(&setup).join("\n")));
        Ok(0)
    })
}

/// `setup`'s tool call (threads.ts:645).
pub(super) fn call_setup(_args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let setup = init_setup(&*deps.client(None).await?).await?;
        Ok(as_json(json!({ "setup": json_of(&setup) })))
    })
}

/// The Ghostty config of this computer, read as the scheme names (dark where none is named).
async fn read_terminal_config(scheme: Option<TerminalScheme>) -> Result<TerminalConfig, HostError> {
    let scheme = scheme.unwrap_or(TerminalScheme::Dark);
    let config = tokio::task::spawn_blocking(move || -> Result<Map<String, Value>, HostError> {
        let host = node_host()?;
        Ok(futures::executor::block_on(read_ghostty_config(&host, scheme)))
    })
    .await
    .map_err(|e| HostError::new(e.to_string()))??;
    serde_json::from_value(Value::Object(config)).map_err(|e| HostError::new(e.to_string()))
}

/// `terminal config`'s run (threads.ts:656-662).
pub(super) fn run_terminal_config(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if !ctx.args.is_empty() {
            return Err(usage_refusal("wsp terminal config takes no positional arguments.", &usage_is(ctx)));
        }
        let scheme = io::scheme_flag(flag(&ctx.flags, "scheme"))?;
        let config = read_terminal_config(scheme).await?;
        ctx.out.emit(&json_of(&config), Some(&terminal_config_lines(&config).join("\n")));
        Ok(0)
    })
}

/// `terminal config`'s tool call (threads.ts:667-669).
pub(super) fn call_terminal_config(args: Frame, _deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let scheme = io::scheme_flag(frame_str(&args, "scheme").as_deref())?;
        let config = read_terminal_config(scheme).await?;
        Ok(as_text(&terminal_config_lines(&config).join("\n"), json_of(&config)))
    })
}

/// `export`'s run (threads.ts:680-699).
pub(super) fn run_export(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        let (reference, folder) = match ctx.args.as_slice() {
            [reference, folder] => (reference.clone(), folder.clone()),
            _ => return Err(usage_refusal("wsp export takes a workspace and a folder on this computer.", &usage_is(ctx))),
        };
        let dest = resolve_path(&here_cwd(ctx.cwd.as_deref()), &folder);
        let agents = io::agents_flag(flag(&ctx.flags, "agents"))?;
        let client = ctx.client(None).await?;
        let workspace = workspace_of(&*client, &reference).await?;
        let req = ExportRequest {
            source: flag(&ctx.flags, "from").map(str::to_owned).unwrap_or_else(|| dest.clone()),
            dest: dest.clone(),
            replace: flag_set(&ctx.flags, "replace").then_some(true),
            agents,
        };
        let done: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let (landed, out) = (done.clone(), ctx.out.clone());
        let exported = io::export_project(
            &*client,
            &workspace.id,
            &req,
            Arc::new(move |frame: &Value| {
                let stage = frame.get("stage").and_then(Value::as_str).unwrap_or_default();
                let message = frame.get("message").and_then(Value::as_str).unwrap_or_default();
                if stage == "done" {
                    *landed.lock().unwrap() = message.to_owned();
                } else if stage != "failed" {
                    out.stream(&format!("{message}\n"));
                }
            }),
        )
        .await
        .map_err(io::with_replace_hint)?;
        let done = done.lock().unwrap().clone();
        ctx.out.emit(&json_of(&exported), Some(&done));
        Ok(0)
    })
}

/// `export`'s tool call (threads.ts:711-719).
pub(super) fn call_export(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let folder = frame_str(&args, "folder").unwrap_or_default();
        let client = deps.client(None).await?;
        let target = workspace_of(&*client, &frame_str(&args, "workspace").unwrap_or_default()).await?;
        let req = ExportRequest {
            source: frame_str(&args, "from").unwrap_or_else(|| folder.clone()),
            dest: folder.clone(),
            replace: frame_bool_opt(&args, "replace"),
            agents: frame_list_opt(&args, "agents"),
        };
        let done: Arc<Mutex<String>> = Arc::new(Mutex::new(String::new()));
        let landed = done.clone();
        let exported = io::export_project(
            &*client,
            &target.id,
            &req,
            Arc::new(move |frame: &Value| {
                if frame.get("stage").and_then(Value::as_str) == Some("done") {
                    *landed.lock().unwrap() = frame.get("message").and_then(Value::as_str).unwrap_or_default().to_owned();
                }
            }),
        )
        .await?;
        let done = done.lock().unwrap().clone();
        Ok(as_text(&done, json_of(&exported)))
    })
}
