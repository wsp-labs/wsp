// SPDX-License-Identifier: AGPL-3.0-only
//! `run` and its tool call (threads.ts:112-215), split from `threads.rs` for its length. Spec: `spec/threads-a.md`.

use super::*;

/// `run`'s run (threads.ts:112).
pub(super) fn run_run(ctx: &VerbContext) -> BoxFuture<'_, Result<i32, HostError>> {
    Box::pin(async move {
        if ctx.args.is_empty() {
            return Err(usage_refusal(EMPTY_MESSAGE_LINE, "Put the message in quotes: wsp run <project> \"say hi\"."));
        }
        if ctx.args.len() > 2 {
            return Err(usage_refusal(
                &format!("wsp run takes a project and a message; {} reads as a third word.", ctx.args[2]),
                &usage_is(ctx),
            ));
        }
        let client = ctx.client(None).await?;
        let (project, message) =
            if ctx.args.len() == 2 { (Some(ctx.args[0].clone()), ctx.args[1].clone()) } else { (None, ctx.args[0].clone()) };
        let say: Arc<dyn Fn(&str) + Send + Sync> = {
            let io = ctx.io.clone();
            Arc::new(move |line: &str| io.error(line))
        };
        let flags = &ctx.flags;
        let on = flag(flags, "on").map(str::to_owned);
        if flag(flags, "size").is_some() && on.is_none() {
            return Err(sub_size_no_cloud_refusal());
        }
        if flag(flags, "from").is_some() && on.is_none() {
            return Err(sub_from_no_cloud_refusal());
        }
        let branch = flag(flags, "branch").map(str::to_owned);
        let cwd = flag(flags, "cwd").map(str::to_owned);
        let harness = flag(flags, "agent").map(str::to_owned);
        let notify_refs = flag_list(flags, "notify");
        let files = flag_list(flags, "file");
        let title = flag(flags, "title").map(str::to_owned);
        let detach = flag_set(flags, "detach");
        if let Some(on) = on {
            let given: Vec<(String, Option<Value>)> = ["beside", "fork", "at", "cwd", "resume", "copy", "replaces"]
                .into_iter()
                .map(|name| (format!("--{name}"), flag_value(ctx, name)))
                .collect();
            let pairs: Vec<(&str, Option<Value>)> = given.iter().map(|(name, value)| (name.as_str(), value.clone())).collect();
            if let Some(word) = beside_on(&pairs) {
                return Err(sub_beside_refusal(&word));
            }
            let picks = picks_from(ctx);
            let sub = before_sending(
                &*client,
                Box::pin(async {
                    let notify = notify_of(&*client, &notify_refs).await?;
                    let req = SubReq {
                        message: message.clone(),
                        on: on.clone(),
                        size: flag(flags, "size").map(str::to_owned),
                        from: flag(flags, "from").map(str::to_owned),
                        branch: branch.clone(),
                        project: project.clone(),
                        harness: harness.clone(),
                        picks: picks.clone(),
                        notify,
                        title: title.clone(),
                        files: files.clone(),
                        elsewhere: ctx.elsewhere,
                    };
                    sub_start(&*client, &ctx.env, &req, say.clone()).await
                }),
            )
            .await?;
            let result = if detach {
                detach_verb(ctx, client.clone(), &sub.opening, &Picks::default(), Some(&*sub.opened), Some(&sub.sub), None).await
            } else {
                let said = SaidAbout { opened: Some(sub.opened.clone()), spend: None };
                match follow_verb(ctx, client.clone(), &sub.opening, true, &Picks::default(), &said, None).await {
                    Ok(turn) => {
                        ctx.out.emit(&with_sub(turn_view(&turn), &sub.sub), None);
                        Ok(())
                    }
                    Err(e) => Err(e),
                }
            };
            (sub.stop)();
            result?;
            return Ok(0);
        }
        let beside = flag(flags, "beside").map(str::to_owned);
        let fork = flag(flags, "fork").map(str::to_owned);
        let at = flag(flags, "at").map(str::to_owned);
        let replaces = flag(flags, "replaces").map(str::to_owned);
        let resume_word = flag(flags, "resume").map(str::to_owned);
        let copy = flag_set(flags, "copy");
        let ask = RunAsk {
            message: message.clone(),
            project: project.clone(),
            beside: beside.clone(),
            fork,
            at: at.map(AtWord::Word),
            branch: branch.clone(),
            cwd: cwd.clone(),
            replaces: replaces.clone(),
            harness,
            picks: picks_from(ctx),
            notify: notify_refs,
            title,
            files,
        };
        run_refusals(&ask, copy, resume_word.as_deref(), replaces.is_some(), &usage_is(ctx))?;
        let resume = resume_asked(resume_word.as_deref(), copy, &ResumeWhere { beside, branch, cwd })?;
        let prepared =
            before_sending(&*client, Box::pin(prepare_run(&*client, &ctx.env, ctx.cwd.as_deref(), ctx.elsewhere, &ask, resume, &*say)))
                .await?;
        let started: Mutex<Option<SharedTurn>> = Mutex::new(None);
        let hook = |turn: &SharedTurn| *started.lock().unwrap() = Some(turn.clone());
        let said_line = prepared.woken.as_ref().and_then(|woke| woke.said.as_deref());
        let outcome: Result<(), HostError> = if detach {
            detach_verb(ctx, client.clone(), &prepared.opening, &Picks::default(), prepared.opened.as_deref(), None, said_line).await
        } else {
            let said = SaidAbout { opened: prepared.opened.clone(), spend: spend_of(prepared.woken.as_ref()) };
            match follow_verb(ctx, client.clone(), &prepared.opening, true, &Picks::default(), &said, Some(&hook)).await {
                Ok(turn) => {
                    ctx.out.emit(&noticed_value(turn_view(&turn), said_line), None);
                    Ok(())
                }
                Err(e) => Err(e),
            }
        };
        match (outcome, &prepared.woken) {
            (Ok(()), _) => Ok(0),
            (Err(e), None) => Err(e),
            (Err(e), Some(woke)) => {
                let line = nap_line(&*client, woke, &started).await;
                Err(with_line(e, line.as_deref()))
            }
        }
    })
}

/// `run`'s tool call (threads.ts:171-215).
pub(super) fn call_run(args: Frame, deps: &VerbDeps) -> BoxFuture<'_, Result<CallToolResult, HostError>> {
    Box::pin(async move {
        let on = frame_str(&args, "on");
        if frame_value(&args, "size").is_some() && on.is_none() {
            return Err(sub_size_no_cloud_refusal());
        }
        if frame_value(&args, "from").is_some() && on.is_none() {
            return Err(sub_from_no_cloud_refusal());
        }
        let quiet: Arc<dyn Fn(&str) + Send + Sync> = Arc::new(|_: &str| {});
        let client = deps.client(None).await?;
        let message = frame_str(&args, "message").unwrap_or_default();
        let project = frame_str(&args, "project");
        let branch = frame_str(&args, "branch");
        let cwd = frame_str(&args, "cwd");
        let harness = frame_str(&args, "agent");
        let notify_refs = frame_list(&args, "notify");
        let files = frame_list(&args, "files");
        let title = frame_str(&args, "title");
        let detach = frame_bool(&args, "detach");
        let picks = frame_picks(&args);
        if let Some(on) = on {
            let given: Vec<(&str, Option<Value>)> = vec![
                ("beside", frame_value(&args, "beside")),
                ("fork", frame_value(&args, "fork")),
                ("at", frame_value(&args, "at")),
                ("cwd", frame_value(&args, "cwd")),
                ("resume", frame_value(&args, "resume")),
                ("copy", frame_value(&args, "copy")),
                ("replaces", frame_value(&args, "replaces")),
            ];
            if let Some(word) = beside_on(&given) {
                return Err(sub_beside_refusal(&word));
            }
            let sub = before_sending(
                &*client,
                Box::pin(async {
                    let notify = notify_of(&*client, &notify_refs).await?;
                    let req = SubReq {
                        message: message.clone(),
                        on: on.clone(),
                        size: frame_str(&args, "size"),
                        from: frame_str(&args, "from"),
                        branch: branch.clone(),
                        project: project.clone(),
                        harness: harness.clone(),
                        picks: picks.clone(),
                        notify,
                        title: title.clone(),
                        files: files.clone(),
                        elsewhere: deps.elsewhere,
                    };
                    sub_start(&*client, &deps.env, &req, quiet.clone()).await
                }),
            )
            .await?;
            let redial = redial_of(deps);
            let result = async {
                if detach {
                    let turn = start_detached(client.clone(), &sub.opening, SessionOrigin::Agent, None, Some(&redial)).await?;
                    Ok(detached_out(&turn, Some(&*sub.opened), Some(&sub.sub), None))
                } else {
                    let turn = follow(client.clone(), &sub.opening, SessionOrigin::Agent, &FollowOn::default(), Some(&redial)).await?;
                    let out = turn_out(&turn)?;
                    let text = format!("{}\n{}", (sub.opened)(&out.thread_id, None), turn_text(&out));
                    Ok(as_text(&text, with_sub(out, &sub.sub)))
                }
            }
            .await;
            (sub.stop)();
            return result;
        }
        let beside = frame_str(&args, "beside");
        let fork = frame_str(&args, "fork");
        let replaces = frame_str(&args, "replaces");
        let conversation = frame_str(&args, "resume");
        let copy = frame_bool(&args, "copy");
        let ask = RunAsk {
            message: message.clone(),
            project: project.clone(),
            beside: beside.clone(),
            fork: fork.clone(),
            at: frame_at(&args),
            branch: branch.clone(),
            cwd: cwd.clone(),
            replaces: replaces.clone(),
            harness,
            picks,
            notify: notify_refs,
            title,
            files,
        };
        run_refusals(&ask, copy, conversation.as_deref(), replaces.is_some(), BESIDE_ALONE_FIX)?;
        let resume = resume_asked(conversation.as_deref(), copy, &ResumeWhere { beside, branch, cwd })?;
        let prepared = before_sending(
            &*client,
            Box::pin(prepare_run(&*client, &deps.env, deps.cwd.as_deref(), deps.elsewhere, &ask, resume, &*quiet)),
        )
        .await?;
        let redial = redial_of(deps);
        let started: Arc<Mutex<Option<SharedTurn>>> = Arc::new(Mutex::new(None));
        let hook_into = started.clone();
        let hook: Arc<dyn Fn(&SharedTurn) + Send + Sync> =
            Arc::new(move |turn: &SharedTurn| *hook_into.lock().unwrap() = Some(turn.clone()));
        let said_line = prepared.woken.as_ref().and_then(|woke| woke.said.clone());
        let result: Result<CallToolResult, HostError> = async {
            if detach {
                let turn = start_detached(client.clone(), &prepared.opening, SessionOrigin::Agent, None, Some(&redial)).await?;
                return Ok(detached_out(&turn, prepared.opened.as_deref(), None, said_line.as_deref()));
            }
            let on = FollowOn { started: Some(hook), ..FollowOn::default() };
            let turn = follow(client.clone(), &prepared.opening, SessionOrigin::Agent, &on, Some(&redial)).await?;
            let out = noticed(turn_out(&turn)?, said_line.as_deref());
            let text = match &prepared.opened {
                None => turn_text(&out),
                Some(opened) => format!("{}\n{}", opened(&out.thread_id, turn.session.cwd.as_deref()), turn_text(&out)),
            };
            Ok(as_text(&text, json_of(&out)))
        }
        .await;
        match (result, &prepared.woken) {
            (Ok(answer), _) => Ok(answer),
            (Err(e), None) => Err(e),
            (Err(e), Some(woke)) => {
                let line = nap_line(&*client, woke, &started).await;
                Err(with_line(e, line.as_deref()))
            }
        }
    })
}
