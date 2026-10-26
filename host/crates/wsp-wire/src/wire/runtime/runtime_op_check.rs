// SPDX-License-Identifier: AGPL-3.0-only
use super::*;

pub(super) fn runtime_op_check(op: &RuntimeOp) -> Result<(), SchemaError> {
    match op {
        RuntimeOp::Auth { .. } => Ok(()),
        RuntimeOp::TicketIssue { .. } => Ok(()),
        RuntimeOp::PairIssue { .. } => Ok(()),
        RuntimeOp::PairRedeem { code, name, .. } => {
            text_at_most(code, 64).map_err(|e| e.under("code"))?;
            text_at_most(name, 200).map_err(|e| e.under("name"))?;
            Ok(())
        }
        RuntimeOp::SealOpen(r) => r.check(),
        RuntimeOp::DeviceAuth(r) => r.check(),
        RuntimeOp::AccountGet { .. } => Ok(()),
        RuntimeOp::DevicesList { .. } => Ok(()),
        RuntimeOp::DevicesRevoke { .. } => Ok(()),
        RuntimeOp::PlaceJoin(r) => r.check(),
        RuntimeOp::PlaceAuth(r) => r.check(),
        RuntimeOp::PlaceProve(r) => r.check(),
        RuntimeOp::PlacesList { .. } => Ok(()),
        RuntimeOp::PlacesUpdate { .. } => Ok(()),
        RuntimeOp::PlacesHolds { .. } => Ok(()),
        RuntimeOp::PlacesRemove { .. } => Ok(()),
        RuntimeOp::PlacesDoctor { doctor_id, project, .. } => {
            text_at_most(doctor_id, 64).map_err(|e| e.under("doctorId"))?;
            if let Some(v) = project {
                text_at_most(v, 200).map_err(|e| e.under("project"))?;
            }
            Ok(())
        }
        RuntimeOp::PlacesDial { .. } => Ok(()),
        RuntimeOp::PlacesSet {
            name,
            ssh,
            recipe,
            threads,
            machines,
            spend_per_day_usd,
            nap_ms,
            turn_limit_ms,
            spawn,
            size,
            delete_after_ms,
            ..
        } => {
            if let Some(v) = name {
                text_at_most(v, 200).map_err(|e| e.under("name"))?;
            }
            if let Some(v) = ssh {
                text_at_most(v, 300).map_err(|e| e.under("ssh"))?;
            }
            if let Some(v) = recipe {
                text_at_least(v, 1).map_err(|e| e.under("recipe"))?;
            }
            if let Some(v) = recipe {
                text_at_most(v, 200).map_err(|e| e.under("recipe"))?;
            }
            if let Some(v) = threads {
                num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("threads"))?;
            }
            if let Some(v) = machines {
                num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("machines"))?;
            }
            if let Some(v) = spend_per_day_usd {
                num_at_least(v.0, 0.0).map_err(|e| e.under("spendPerDayUsd"))?;
            }
            spend_per_day_usd.check().map_err(|e| e.under("spendPerDayUsd"))?;
            if let Some(Some(v)) = nap_ms {
                num_at_least(v.as_f64(), 60000.0).map_err(|e| e.under("napMs"))?;
            }
            if let Some(Some(v)) = nap_ms {
                num_at_most(v.as_f64(), 10800000.0).map_err(|e| e.under("napMs"))?;
            }
            if let Some(Some(v)) = turn_limit_ms {
                num_at_least(v.as_f64(), 3600000.0).map_err(|e| e.under("turnLimitMs"))?;
            }
            if let Some(Some(v)) = turn_limit_ms {
                num_at_most(v.as_f64(), 86400000.0).map_err(|e| e.under("turnLimitMs"))?;
            }
            spawn.check().map_err(|e| e.under("spawn"))?;
            size.check().map_err(|e| e.under("size"))?;
            if let Some(v) = delete_after_ms {
                num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("deleteAfterMs"))?;
            }
            if let Some(v) = delete_after_ms {
                num_at_most(v.as_f64(), 86400000.0).map_err(|e| e.under("deleteAfterMs"))?;
            }
            Ok(())
        }
        RuntimeOp::PlacesFollow { recipe, .. } => {
            text_at_least(recipe, 1).map_err(|e| e.under("recipe"))?;
            text_at_most(recipe, 200).map_err(|e| e.under("recipe"))?;
            Ok(())
        }
        RuntimeOp::PlacesSkip { row, .. } => {
            text_at_least(row, 1).map_err(|e| e.under("row"))?;
            text_at_most(row, 300).map_err(|e| e.under("row"))?;
            Ok(())
        }
        RuntimeOp::PlacesSetupLog { .. } => Ok(()),
        RuntimeOp::PlacesEstimate { r#ref, choices, .. } => {
            text_at_most(r#ref, 200).map_err(|e| e.under("ref"))?;
            choices.check().map_err(|e| e.under("choices"))?;
            Ok(())
        }
        RuntimeOp::PlacesLoginLanded { .. } => Ok(()),
        RuntimeOp::PlacesDoor { .. } => Ok(()),
        RuntimeOp::PlacesMint { .. } => Ok(()),
        RuntimeOp::PlacesSshHosts { .. } => Ok(()),
        RuntimeOp::PlacesAdd { add_id, address, name, ssh_port, key_path, host_key, recipe, .. } => {
            if let Some(v) = add_id {
                text_at_most(v, 64).map_err(|e| e.under("addId"))?;
            }
            text_at_most(address, 200).map_err(|e| e.under("address"))?;
            if let Some(v) = name {
                text_at_most(v, 200).map_err(|e| e.under("name"))?;
            }
            if let Some(v) = ssh_port {
                num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("sshPort"))?;
            }
            if let Some(v) = ssh_port {
                num_at_most(v.as_f64(), 65535.0).map_err(|e| e.under("sshPort"))?;
            }
            if let Some(v) = key_path {
                text_at_most(v, 1024).map_err(|e| e.under("keyPath"))?;
            }
            if let Some(v) = host_key {
                text_at_most(v, 200).map_err(|e| e.under("hostKey"))?;
            }
            if let Some(v) = recipe {
                text_at_most(v, 200).map_err(|e| e.under("recipe"))?;
            }
            Ok(())
        }
        RuntimeOp::PlacesSetup { r#ref, recipe, choices, add_id, row, .. } => {
            text_at_most(r#ref, 200).map_err(|e| e.under("ref"))?;
            if let Some(v) = recipe {
                text_at_most(v, 200).map_err(|e| e.under("recipe"))?;
            }
            choices.check().map_err(|e| e.under("choices"))?;
            if let Some(v) = add_id {
                text_at_most(v, 64).map_err(|e| e.under("addId"))?;
            }
            if let Some(v) = row {
                text_at_most(v, 400).map_err(|e| e.under("row"))?;
            }
            Ok(())
        }
        RuntimeOp::PlacesChoose { r#ref, choices, recipe, .. } => {
            text_at_most(r#ref, 200).map_err(|e| e.under("ref"))?;
            choices.check().map_err(|e| e.under("choices"))?;
            if let Some(v) = recipe {
                text_at_most(v, 200).map_err(|e| e.under("recipe"))?;
            }
            Ok(())
        }
        RuntimeOp::RecipesList { .. } => Ok(()),
        RuntimeOp::RecipesGet { name, .. } => {
            text_at_most(name, 200).map_err(|e| e.under("name"))?;
            Ok(())
        }
        RuntimeOp::RecipesSave { name, from, .. } => {
            text_at_most(name, 200).map_err(|e| e.under("name"))?;
            if let Some(v) = from {
                text_at_most(v, 200).map_err(|e| e.under("from"))?;
            }
            Ok(())
        }
        RuntimeOp::RecipesRemove { name, .. } => {
            text_at_most(name, 200).map_err(|e| e.under("name"))?;
            Ok(())
        }
        RuntimeOp::RecipesOptions { .. } => Ok(()),
        RuntimeOp::EventsSubscribe { after, .. } => {
            if let Some(v) = after {
                num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("after"))?;
            }
            Ok(())
        }
        RuntimeOp::StatusSubscribe { .. } => Ok(()),
        RuntimeOp::StatusList { .. } => Ok(()),
        RuntimeOp::WorkspacesCreate { cpu, mem_mb, agents, idle_window_ms, sub, .. } => {
            cpu.check().map_err(|e| e.under("cpu"))?;
            mem_mb.check().map_err(|e| e.under("memMb"))?;
            agents.check().map_err(|e| e.under("agents"))?;
            idle_window_ms.check().map_err(|e| e.under("idleWindowMs"))?;
            sub.check().map_err(|e| e.under("sub"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesLanding { .. } => Ok(()),
        RuntimeOp::WorkspacesList { .. } => Ok(()),
        RuntimeOp::WorkspacesResolve { .. } => Ok(()),
        RuntimeOp::WorkspacesGet { .. } => Ok(()),
        RuntimeOp::WorkspacesNap { .. } => Ok(()),
        RuntimeOp::WorkspacesResize { size, .. } => {
            size.check().map_err(|e| e.under("size"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesWake { .. } => Ok(()),
        RuntimeOp::WorkspacesRestartDaemon { .. } => Ok(()),
        RuntimeOp::WorkspacesStopWake { .. } => Ok(()),
        RuntimeOp::WorkspacesUpgrade { .. } => Ok(()),
        RuntimeOp::WorkspacesRename { .. } => Ok(()),
        RuntimeOp::WorkspacesLook { theme, .. } => {
            theme.check().map_err(|e| e.under("theme"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesBringBack { .. } => Ok(()),
        RuntimeOp::WorktreeMake { .. } => Ok(()),
        RuntimeOp::FolderMake { .. } => Ok(()),
        RuntimeOp::WorktreeRemove { .. } => Ok(()),
        RuntimeOp::WorkspacesCheckout { .. } => Ok(()),
        RuntimeOp::WorkspacesDiscard { .. } => Ok(()),
        RuntimeOp::WorkspacesCommit { .. } => Ok(()),
        RuntimeOp::WorkspacesCommitDraft { .. } => Ok(()),
        RuntimeOp::WorkspacesViewed { .. } => Ok(()),
        RuntimeOp::WorkspacesPullRequestView { .. } => Ok(()),
        RuntimeOp::WorkspacesPullRequestDiff { .. } => Ok(()),
        RuntimeOp::WorkspacesPullRequestSend { items, .. } => {
            items_at_least(items.len(), 1).map_err(|e| e.under("items"))?;
            items.check().map_err(|e| e.under("items"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesPullRequestReply { reply_to, body, .. } => {
            if let Some(v) = reply_to {
                num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("replyTo"))?;
            }
            text_at_most(body, 65536).map_err(|e| e.under("body"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesPullRequestResolve { .. } => Ok(()),
        RuntimeOp::WorkspacesPullRequestReact { content, .. } => {
            content.check().map_err(|e| e.under("content"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesFix { .. } => Ok(()),
        RuntimeOp::WorkspacesMerge { method, head, .. } => {
            method.check().map_err(|e| e.under("method"))?;
            if let Some(v) = head {
                text_at_least(v, 1).map_err(|e| e.under("head"))?;
            }
            Ok(())
        }
        RuntimeOp::WorkspacesStart { .. } => Ok(()),
        RuntimeOp::WorkspacesReview { .. } => Ok(()),
        RuntimeOp::WorkspacesReviewDraft { on, .. } => {
            on.check().map_err(|e| e.under("on"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesReviewPost { .. } => Ok(()),
        RuntimeOp::WorkspacesUpdate { .. } => Ok(()),
        RuntimeOp::WorkspacesMergeIn { .. } => Ok(()),
        RuntimeOp::WorkspacesDelete { .. } => Ok(()),
        RuntimeOp::WorkspacesForget { .. } => Ok(()),
        RuntimeOp::WorkspacesTouch { .. } => Ok(()),
        RuntimeOp::DaemonOpen { .. } => Ok(()),
        RuntimeOp::SysSubscribe { .. } => Ok(()),
        RuntimeOp::SysUnsubscribe { .. } => Ok(()),
        RuntimeOp::AppProcsWatch { .. } => Ok(()),
        RuntimeOp::AppProcsUnwatch { .. } => Ok(()),
        RuntimeOp::AppProcsOffer { .. } => Ok(()),
        RuntimeOp::AppProcsSet { procs, .. } => {
            procs.check().map_err(|e| e.under("procs"))?;
            Ok(())
        }
        RuntimeOp::ActivityRead { .. } => Ok(()),
        RuntimeOp::ActivityEnd { pid, started_at, .. } => {
            num_above(pid.as_f64(), 0.0).map_err(|e| e.under("pid"))?;
            started_at.check().map_err(|e| e.under("startedAt"))?;
            Ok(())
        }
        RuntimeOp::DaemonSend { frame, .. } => {
            frame.check().map_err(|e| e.under("frame"))?;
            Ok(())
        }
        RuntimeOp::DaemonClose { .. } => Ok(()),
        RuntimeOp::SessionsStart { notify, resume, fork, attachments, .. } => {
            if let Some(v) = notify {
                items_at_least(v.len(), 1).map_err(|e| e.under("notify"))?;
            }
            resume.check().map_err(|e| e.under("resume"))?;
            fork.check().map_err(|e| e.under("fork"))?;
            attachments.check().map_err(|e| e.under("attachments"))?;
            Ok(())
        }
        RuntimeOp::HarnessesList { .. } => Ok(()),
        RuntimeOp::SessionsList { .. } => Ok(()),
        RuntimeOp::SessionsHistory { before, limit, .. } => {
            if let Some(v) = before {
                num_above(v.as_f64(), 0.0).map_err(|e| e.under("before"))?;
            }
            if let Some(v) = limit {
                num_above(v.as_f64(), 0.0).map_err(|e| e.under("limit"))?;
            }
            if let Some(v) = limit {
                num_at_most(v.as_f64(), 1000.0).map_err(|e| e.under("limit"))?;
            }
            Ok(())
        }
        RuntimeOp::SessionsHead { .. } => Ok(()),
        RuntimeOp::SessionsAttachment { index, .. } => {
            num_at_least(index.as_f64(), 0.0).map_err(|e| e.under("index"))?;
            Ok(())
        }
        RuntimeOp::SessionsInterrupt { .. } => Ok(()),
        RuntimeOp::SessionsSteer { attachments, .. } => {
            attachments.check().map_err(|e| e.under("attachments"))?;
            Ok(())
        }
        RuntimeOp::SessionsAnswer { .. } => Ok(()),
        RuntimeOp::SessionsAccess { .. } => Ok(()),
        RuntimeOp::SessionsRename { .. } => Ok(()),
        RuntimeOp::SessionsForget { .. } => Ok(()),
        RuntimeOp::SessionsDelete { .. } => Ok(()),
        RuntimeOp::SessionsRead { .. } => Ok(()),
        RuntimeOp::SessionsReplaceable { .. } => Ok(()),
        RuntimeOp::SessionsWarm { .. } => Ok(()),
        RuntimeOp::SessionsSettle { thread_ids, .. } => {
            items_at_least(thread_ids.len(), 1).map_err(|e| e.under("threadIds"))?;
            Ok(())
        }
        RuntimeOp::SessionsMark { thread_ids, marks, .. } => {
            items_at_least(thread_ids.len(), 1).map_err(|e| e.under("threadIds"))?;
            marks.check().map_err(|e| e.under("marks"))?;
            Ok(())
        }
        RuntimeOp::SessionsRestore { thread_ids, .. } => {
            items_at_least(thread_ids.len(), 1).map_err(|e| e.under("threadIds"))?;
            Ok(())
        }
        RuntimeOp::SessionsSearch { .. } => Ok(()),
        RuntimeOp::SessionsAside { .. } => Ok(()),
        RuntimeOp::SessionsRun { .. } => Ok(()),
        RuntimeOp::SessionsRewind { .. } => Ok(()),
        RuntimeOp::GoldenGet { .. } => Ok(()),
        RuntimeOp::CapabilitiesGet { .. } => Ok(()),
        RuntimeOp::GoldenPrepare { kind, .. } => {
            kind.check().map_err(|e| e.under("kind"))?;
            Ok(())
        }
        RuntimeOp::GoldenSeal { .. } => Ok(()),
        RuntimeOp::SnapshotsList { .. } => Ok(()),
        RuntimeOp::SnapshotsStorage { .. } => Ok(()),
        RuntimeOp::CostHistory { .. } => Ok(()),
        RuntimeOp::CostSpend { .. } => Ok(()),
        RuntimeOp::UsageUsed { .. } => Ok(()),
        RuntimeOp::UsageAccounts { .. } => Ok(()),
        RuntimeOp::UsageReset { .. } => Ok(()),
        RuntimeOp::PlacesReadings { .. } => Ok(()),
        RuntimeOp::SnapshotsRollback { version, .. } => {
            version.check().map_err(|e| e.under("version"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesPortReach { port, .. } => {
            num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
            num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesPortProbe { port, .. } => {
            num_at_least(port.as_f64(), 1.0).map_err(|e| e.under("port"))?;
            num_at_most(port.as_f64(), 65535.0).map_err(|e| e.under("port"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesRebuild { .. } => Ok(()),
        RuntimeOp::ForwardsList { .. } => Ok(()),
        RuntimeOp::ForwardsStop { port, .. } => {
            port.check().map_err(|e| e.under("port"))?;
            Ok(())
        }
        RuntimeOp::WorkspacesExec { argv, .. } => {
            items_at_least(argv.len(), 1).map_err(|e| e.under("argv"))?;
            Ok(())
        }
        RuntimeOp::HostFolders { .. } => Ok(()),
        RuntimeOp::HostTerminalConfig { .. } => Ok(()),
        RuntimeOp::EditorList { .. } => Ok(()),
        RuntimeOp::EditorOpen { line, .. } => {
            if let Some(v) = line {
                num_above(v.as_f64(), 0.0).map_err(|e| e.under("line"))?;
            }
            Ok(())
        }
        RuntimeOp::SshPort { .. } => Ok(()),
        RuntimeOp::SshInclude { .. } => Ok(()),
        RuntimeOp::AgentsRead { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::ServersTools { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::ServersIcon { host, .. } => {
            text_at_least(host, 1).map_err(|e| e.under("host"))?;
            text_at_most(host, 260).map_err(|e| e.under("host"))?;
            Ok(())
        }
        RuntimeOp::AgentsSignIn { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::ServersSignIn { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::AgentsSignInCode { code, .. } => {
            text_at_least(code, 1).map_err(|e| e.under("code"))?;
            Ok(())
        }
        RuntimeOp::AgentsSignIns { .. } => Ok(()),
        RuntimeOp::AgentsSignInStop { .. } => Ok(()),
        RuntimeOp::AgentsSignInLine { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::AgentsKey { key, .. } => {
            text_at_least(key, 1).map_err(|e| e.under("key"))?;
            Ok(())
        }
        RuntimeOp::AgentsCloudSignIn { .. } => Ok(()),
        RuntimeOp::ProvidersCheckKey { provider, key, .. } => {
            text_at_most(provider, 64).map_err(|e| e.under("provider"))?;
            text_at_least(key, 1).map_err(|e| e.under("key"))?;
            Ok(())
        }
        RuntimeOp::BrokerStatus { .. } => Ok(()),
        RuntimeOp::AgentsAddTools { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::AgentsSetup { program, config_dir, env, .. } => {
            if let Some(Some(v)) = program {
                text_at_least(v, 1).map_err(|e| e.under("program"))?;
            }
            if let Some(Some(v)) = config_dir {
                text_at_least(v, 1).map_err(|e| e.under("configDir"))?;
            }
            if let Some(v) = env {
                env_names(v).map_err(|e| e.under("env"))?;
            }
            Ok(())
        }
        RuntimeOp::SkillsSearch { limit, .. } => {
            if let Some(v) = limit {
                num_at_least(v.as_f64(), 1.0).map_err(|e| e.under("limit"))?;
            }
            if let Some(v) = limit {
                num_at_most(v.as_f64(), 50.0).map_err(|e| e.under("limit"))?;
            }
            Ok(())
        }
        RuntimeOp::SkillsGet { .. } => Ok(()),
        RuntimeOp::SkillsPreview { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::SkillsAdd { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::SkillsRemove { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::SkillsToggle { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::ServersAdd { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::ServersRemove { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::ServersToggle { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::PluginsToggle { target, .. } => {
            target.check().map_err(|e| e.under("target"))?;
            Ok(())
        }
        RuntimeOp::InitGet { .. } => Ok(()),
        RuntimeOp::InitKeys { provider, .. } => {
            if let Some(v) = provider {
                text_at_most(v, 64).map_err(|e| e.under("provider"))?;
            }
            Ok(())
        }
        RuntimeOp::PreferencesGet { .. } => Ok(()),
        RuntimeOp::PreferencesSet { patch, .. } => {
            patch.check().map_err(|e| e.under("patch"))?;
            Ok(())
        }
        RuntimeOp::ReleaseGet { .. } => Ok(()),
        RuntimeOp::ReleaseCheck { .. } => Ok(()),
        RuntimeOp::HostRestart { .. } => Ok(()),
        RuntimeOp::ProjectsAdd { seed, .. } => {
            seed.check().map_err(|e| e.under("seed"))?;
            Ok(())
        }
        RuntimeOp::ProjectSeedPlan { .. } => Ok(()),
        RuntimeOp::ProjectsSeed { .. } => Ok(()),
        RuntimeOp::ProjectsList { .. } => Ok(()),
        RuntimeOp::ConversationsList { .. } => Ok(()),
        RuntimeOp::ConversationsRead { agent, conversation, .. } => {
            text_at_least(agent, 1).map_err(|e| e.under("agent"))?;
            text_at_least(conversation, 1).map_err(|e| e.under("conversation"))?;
            text_at_most(conversation, 200).map_err(|e| e.under("conversation"))?;
            Ok(())
        }
        RuntimeOp::ProjectsDefaults { .. } => Ok(()),
        RuntimeOp::ProjectsResolve { .. } => Ok(()),
        RuntimeOp::ProjectsBranch { .. } => Ok(()),
        RuntimeOp::ProjectsRemove { .. } => Ok(()),
        RuntimeOp::ProjectsIcon { png, .. } => {
            if let Some(v) = png {
                text_at_most(v, 327680).map_err(|e| e.under("png"))?;
            }
            Ok(())
        }
        RuntimeOp::ProjectsIcons { hashes, .. } => {
            items_at_most(hashes.len(), 1000).map_err(|e| e.under("hashes"))?;
            Ok(())
        }
        RuntimeOp::ProjectPlan { .. } => Ok(()),
        RuntimeOp::ProjectImport { .. } => Ok(()),
        RuntimeOp::ProjectExport { .. } => Ok(()),
        RuntimeOp::ImageGet { .. } => Ok(()),
        RuntimeOp::ImageBuild { place, .. } => {
            text_at_least(place, 1).map_err(|e| e.under("place"))?;
            Ok(())
        }
        RuntimeOp::SlatesGet { .. } => Ok(()),
        RuntimeOp::SlatesWrite { press, .. } => {
            press.check().map_err(|e| e.under("press"))?;
            Ok(())
        }
        RuntimeOp::SlatesState { .. } => Ok(()),
        RuntimeOp::SlatesRead { .. } => Ok(()),
        RuntimeOp::SlatesCatalog { .. } => Ok(()),
        RuntimeOp::SlatesEvent { request_id, scope, row_action, .. } => {
            text_at_least(request_id, 1).map_err(|e| e.under("requestId"))?;
            text_at_most(request_id, 200).map_err(|e| e.under("requestId"))?;
            scope.check().map_err(|e| e.under("scope"))?;
            if let Some(v) = row_action {
                num_at_least(v.as_f64(), 0.0).map_err(|e| e.under("rowAction"))?;
            }
            Ok(())
        }
        RuntimeOp::SlatesApprove { .. } => Ok(()),
        RuntimeOp::SlatesCancel { .. } => Ok(()),
        RuntimeOp::SlatesRevoke { .. } => Ok(()),
        RuntimeOp::SlatesShown { .. } => Ok(()),
        RuntimeOp::SlatesSubscribe { .. } => Ok(()),
        RuntimeOp::SlatesUnsubscribe { .. } => Ok(()),
        RuntimeOp::SlatesResolve { paths, .. } => {
            items_at_most(paths.len(), 200).map_err(|e| e.under("paths"))?;
            Ok(())
        }
        RuntimeOp::SlatesImage { src, have, .. } => {
            text_at_most(src, 4096).map_err(|e| e.under("src"))?;
            if let Some(v) = have {
                text_at_most(v, 200).map_err(|e| e.under("have"))?;
            }
            Ok(())
        }
    }
}
