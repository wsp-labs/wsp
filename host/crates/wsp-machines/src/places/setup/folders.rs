// SPDX-License-Identifier: AGPL-3.0-only
//! `packages/runtime/src/places/setup.ts:633-744, 767-806` at 65f6b89c7: the folders, their servers, and a sync's
//! removals. Spec: `spec/places_setup_folders.md`.

use std::rc::Rc;

use indexmap::{IndexMap, IndexSet};
use tokio::task::spawn_local;
use wsp_core::recipe_sync::{RecipeChange, RecipeChangeHow};
use wsp_engine::machine::ExecOptions;
use wsp_engine::provision::ProvisionOn;
use wsp_engine::provision_files::unland_files;
use wsp_engine::target_line::stores_reached;
use wsp_wire::types::recipe_file::RecipeFileFoldersValue;
use wsp_wire::types::shell_quote::shell_quote;
use wsp_wire::views::place::{
    PlaceProvisionRow, PlaceProvisionRowOutcome, PlaceProvisionRowProject, PlaceSetupStep,
};
use wsp_words::exit::HostError;
use wsp_words::start::github_address;
use wsp_words::views::place_setup_words::{
    edited_there_line, was_there_line, FOLDER_SERVERS_WAIT_LINE, NEEDS_GITHUB_LINE, NO_FOLDER_ROAD,
    UNLAND_FAILED_LINE, WAITS_ON_GITHUB_LINE,
};
use wsp_words::words::base::last_line;

use super::super::helpers::{first_line_of, PROBE_MS, UNDO_MS};
use super::super::types::{FolderMove, HeldRow, PlaceResult, PlaceUndo, RemovedRow};
use super::run::{plain_row, SetupRun};
use super::{held_of, host_of, under_folders_of};

/// The kind a recipe change's key names, as the removal reads it (setup.ts:777).
fn kind_of(change: &RecipeChange) -> String {
    change
        .key
        .strip_suffix(&change.name)
        .and_then(|k| k.strip_suffix('/'))
        .unwrap_or_default()
        .to_string()
}

/// The row a folder's claim keeps when it is pinned to the project wsp made from it (setup.ts:656).
fn claimed(claim: Option<&FolderMove>, r: HeldRow) -> HeldRow {
    match claim {
        Some(claim) if r.row.project.is_none() => HeldRow {
            row: PlaceProvisionRow {
                project: Some(PlaceProvisionRowProject {
                    id: claim.id.clone(),
                }),
                ..r.row
            },
            pick: Some(claim.pick.clone()),
            created_at: claim.created_at.clone(),
        },
        _ => r,
    }
}

impl SetupRun {
    /// Whether a folder's repository clones there only with GitHub signed in: one on GitHub that an anonymous read of
    /// it from that computer is refused (setup.ts:635).
    pub async fn needs_github(self: &Rc<Self>, folder: &RecipeFileFoldersValue) -> bool {
        let remote = match self.area.ctx.recording.folder_remote(folder).await {
            Some(Ok(remote)) => remote,
            _ => None,
        };
        let Some(at) = remote.as_deref().and_then(github_address) else {
            return false;
        };
        let cmd = format!("GIT_TERMINAL_PROMPT=0 git -c credential.helper= ls-remote --quiet {} HEAD >/dev/null 2>&1", shell_quote(&at));
        match self
            .machine
            .exec(
                &cmd,
                ExecOptions {
                    timeout_ms: Some(PROBE_MS),
                    ..Default::default()
                },
            )
            .await
        {
            Ok(read) => read.exit_code.get() != 0,
            Err(_) => true,
        }
    }

    /// One folder made a project there by the add's own road, under the id of the project wsp made from it before
    /// where there is one (setup.ts:645).
    pub async fn add_folder(
        self: &Rc<Self>,
        key: &str,
        folder: &RecipeFileFoldersValue,
        mv: Option<&FolderMove>,
    ) -> HeldRow {
        let label = folder.name.clone().unwrap_or_else(|| key.to_string());
        let id = format!("folders/{key}");
        let stage: Rc<dyn Fn(&str)> = {
            let log = Rc::clone(&self.log);
            let label = label.clone();
            Rc::new(move |line: &str| log.write(&format!("[folders] {label}: {line}")))
        };
        let road =
            self.area
                .ctx
                .recording
                .add_folder(&self.ask.place_id, key, folder, mv, Some(stage));
        match under_folders_of(&self.ask.place_id, road).await {
            None => HeldRow {
                row: PlaceProvisionRow {
                    note: Some(NO_FOLDER_ROAD.to_string()),
                    ..plain_row(&id, &label, PlaceProvisionRowOutcome::Failed)
                },
                pick: None,
                created_at: None,
            },
            Some(Ok(row)) => row,
            Some(Err(e)) => HeldRow {
                row: PlaceProvisionRow {
                    note: Some(first_line_of(&e)),
                    ..plain_row(&id, &label, PlaceProvisionRowOutcome::Failed)
                },
                pick: None,
                created_at: None,
            },
        }
    }

    /// One landed folder's own servers carried into its project there (setup.ts:674).
    pub async fn carry_servers(
        self: &Rc<Self>,
        key: &str,
        row: &HeldRow,
    ) -> Vec<PlaceProvisionRow> {
        self.carried_folders.borrow_mut().insert(key.to_string());
        let Some(project) = row.row.project.as_ref().map(|p| p.id.clone()) else {
            return Vec::new();
        };
        let Some(provisioner) = self.area.ctx.wiring.provision.clone() else {
            return Vec::new();
        };
        if !provisioner.has_project_servers() {
            return Vec::new();
        }
        let servers_id = format!("folders/{key}/servers");
        let servers_label = format!("{} servers", row.row.label);
        if !self.agent_files_there.get() {
            let note = FOLDER_SERVERS_WAIT_LINE.to_string();
            return vec![PlaceProvisionRow {
                note: Some(note),
                ..plain_row(
                    &servers_id,
                    &servers_label,
                    PlaceProvisionRowOutcome::Skipped,
                )
            }];
        }
        let path = match self
            .area
            .ctx
            .recording
            .projects_on(&self.ask.place_id)
            .await
        {
            Ok(projects) => projects
                .into_iter()
                .find(|p| p.id == project)
                .map(|p| p.path),
            Err(_) => None,
        };
        let Some(path) = path else { return Vec::new() };
        let stores = self.stores_here().await;
        let on = ProvisionOn {
            home: self.home.clone(),
            held: Some(self.server_names()),
            stores,
        };
        let recipe = self.follows.then(|| self.ask.picks.name.clone());
        let got = provisioner
            .project_servers(
                Rc::clone(&self.machine),
                &self.ask.picks,
                key,
                &path,
                self.stage_of("folderServers"),
                &on,
                recipe.as_deref(),
            )
            .await;
        let rows = match got {
            Ok(rows) => rows,
            Err(e) => vec![PlaceProvisionRow {
                note: Some(first_line_of(&e)),
                ..plain_row(
                    &servers_id,
                    &servers_label,
                    PlaceProvisionRowOutcome::Failed,
                )
            }],
        };
        self.ours(rows)
    }

    /// The folderServers step: every folder landed so far and not carried yet (setup.ts:689).
    pub async fn folder_servers(self: &Rc<Self>) -> Vec<PlaceProvisionRow> {
        let mut out = Vec::new();
        let mut at = 0;
        loop {
            let next = self
                .landed_folders
                .borrow()
                .get_index(at)
                .map(|(key, row)| (key.clone(), row.clone()));
            let Some((key, row)) = next else { break };
            at += 1;
            let carried = self.carried_folders.borrow().contains(&key);
            if !carried {
                out.extend(self.carry_servers(&key, &row).await);
            }
        }
        out
    }

    /// The folders, each a project on that computer. One whose repository needs GitHub there waits on the GitHub
    /// sign-in while the person has it open, and lands once they are through (setup.ts:698).
    pub async fn folders(self: &Rc<Self>) -> Result<Vec<HeldRow>, HostError> {
        let mut out: Vec<HeldRow> = Vec::new();
        let standing: IndexSet<String> = self
            .area
            .ctx
            .recording
            .projects_on(&self.ask.place_id)
            .await?
            .into_iter()
            .map(|p| p.id)
            .collect();
        for (name, folder) in self.ask.picks.folders.iter() {
            let key = name.0.clone();
            if let Some(sync) = &self.ask.sync {
                if !sync.moved.contains(&format!("folders/{key}")) {
                    continue;
                }
            }
            let label = folder.name.clone().unwrap_or_else(|| key.clone());
            let id = format!("folders/{key}");
            let was = self.made.iter().find(|r| r.row.id == id).cloned();
            let claim: Option<FolderMove> =
                was.as_ref()
                    .and_then(|was| match (&was.row.project, &was.pick) {
                        (Some(project), Some(pick)) => Some(FolderMove {
                            id: project.id.clone(),
                            pick: pick.clone(),
                            created_at: was.created_at.clone(),
                        }),
                        _ => None,
                    });
            let ours = claim.clone().filter(|claim| standing.contains(&claim.id));
            let same_pick = ours.as_ref().filter(|ours| {
                ours.pick.from == folder.from
                    && ours.pick.name == folder.name
                    && ours.pick.keep.join("\n") == folder.keep.join("\n")
            });
            if let Some(ours) = same_pick {
                let was_label = was
                    .as_ref()
                    .map(|w| w.row.label.clone())
                    .unwrap_or_else(|| label.clone());
                let base = HeldRow {
                    row: PlaceProvisionRow {
                        project: Some(PlaceProvisionRowProject {
                            id: ours.id.clone(),
                        }),
                        ..plain_row(&id, &was_label, PlaceProvisionRowOutcome::Installed)
                    },
                    pick: Some(folder.clone()),
                    created_at: None,
                };
                let landed = if ours.pick.icon == folder.icon
                    && ours.pick.hue == folder.hue
                    && ours.pick.image == folder.image
                {
                    HeldRow {
                        row: PlaceProvisionRow {
                            earlier: Some(true),
                            ..base.row
                        },
                        ..base
                    }
                } else {
                    match self
                        .area
                        .ctx
                        .recording
                        .folder_look(&ours.id, &ours.pick, folder)
                        .await
                    {
                        Ok(()) => base,
                        Err(e) => HeldRow {
                            row: PlaceProvisionRow {
                                outcome: PlaceProvisionRowOutcome::Failed,
                                note: Some(first_line_of(&e)),
                                ..base.row
                            },
                            pick: Some(ours.pick.clone()),
                            created_at: None,
                        },
                    }
                };
                out.push(self.landed_folder(&key, landed));
                continue;
            }
            let github_known = self.github_known.get();
            if github_known == Some(true) || !self.needs_github(folder).await {
                let added = self.add_folder(&key, folder, claim.as_ref()).await;
                out.push(self.landed_folder(&key, claimed(claim.as_ref(), added)));
                continue;
            }
            if github_known == Some(false) {
                let failed = HeldRow {
                    row: PlaceProvisionRow {
                        note: Some(NEEDS_GITHUB_LINE.to_string()),
                        ..plain_row(&id, &label, PlaceProvisionRowOutcome::Failed)
                    },
                    pick: None,
                    created_at: None,
                };
                out.push(claimed(claim.as_ref(), failed));
                continue;
            }
            let waiting = HeldRow {
                row: PlaceProvisionRow {
                    note: Some(WAITS_ON_GITHUB_LINE.to_string()),
                    ..plain_row(&id, &label, PlaceProvisionRowOutcome::Skipped)
                },
                pick: None,
                created_at: None,
            };
            out.push(claimed(claim.as_ref(), waiting));
            self.folders_waiting.set(self.folders_waiting.get() + 1);
            let run = Rc::clone(self);
            let github = self.github_ready.clone();
            let folder = folder.clone();
            spawn_local(async move {
                let ok = github.await;
                let got = if ok {
                    run.add_folder(&key, &folder, claim.as_ref()).await
                } else {
                    HeldRow {
                        row: PlaceProvisionRow {
                            note: Some(NEEDS_GITHUB_LINE.to_string()),
                            ..plain_row(&id, &label, PlaceProvisionRowOutcome::Failed)
                        },
                        pick: None,
                        created_at: None,
                    }
                };
                let row = run.landed_folder(&key, claimed(claim.as_ref(), got));
                run.land_row(stepped(&row, PlaceSetupStep::Folders));
                run.steps_done.clone().await;
                let carried = run.carried_folders.borrow().contains(&key);
                if row.row.outcome == PlaceProvisionRowOutcome::Installed && !carried {
                    for servers in run.carry_servers(&key, &row).await {
                        run.land_row(held_of(PlaceProvisionRow {
                            step: Some(PlaceSetupStep::FolderServers),
                            ..servers
                        }));
                    }
                }
                run.folders_waiting.set(run.folders_waiting.get() - 1);
                run.land_row(stepped(&row, PlaceSetupStep::Folders));
            });
        }
        Ok(out)
    }

    /// The folder each agent's threads there are pointed at that the computer's login reaches (setup.ts:769).
    pub async fn stores_here(self: &Rc<Self>) -> Option<IndexMap<String, String>> {
        let stores = self
            .area
            .ctx
            .recording
            .stores_on(&self.ask.place_id, &self.home)?;
        let folder = self.area.ctx.door().folder_computer(&self.ask.place_id)?;
        let login = folder.machine.login_of().await.ok()?;
        Some(stores_reached(&login, &stores))
    }
}

/// A held row with its step set to the given one, as the rows that land after a step read (setup.ts:666).
fn stepped(row: &HeldRow, step: PlaceSetupStep) -> HeldRow {
    HeldRow {
        row: PlaceProvisionRow {
            step: Some(step),
            ..row.row.clone()
        },
        pick: row.pick.clone(),
        created_at: row.created_at.clone(),
    }
}

impl SetupRun {
    /// The rows of the steps a sync's removals take off (setup.ts:777).
    pub async fn undo(self: &Rc<Self>) -> PlaceResult<()> {
        let Some(sync) = self.ask.sync.as_ref() else {
            return Ok(());
        };
        let removed_changes: Vec<&RecipeChange> = sync
            .changes
            .iter()
            .filter(|c| c.how == RecipeChangeHow::Removed)
            .collect();
        let Some(provisioner) = self.area.ctx.wiring.provision.clone() else {
            return Ok(());
        };
        if removed_changes.is_empty() || !provisioner.has_undo() {
            return Ok(());
        }
        let removed: Vec<RemovedRow> = removed_changes
            .iter()
            .map(|c| RemovedRow {
                kind: kind_of(c),
                name: c.name.clone(),
            })
            .collect();
        let stores = self.stores_here().await;
        let planned: Vec<PlaceUndo> = provisioner
            .undo(&sync.before, &removed, &self.home, stores.as_ref())
            .await?;
        let machine = Rc::clone(&self.machine);
        for u in planned {
            let owned = u.owner.as_ref().and_then(|owner| {
                self.rows
                    .borrow()
                    .iter()
                    .find(|r| r.row.id == *owner)
                    .map(|r| r.row.outcome)
            });
            let ours = owned == Some(PlaceProvisionRowOutcome::Installed);
            let mut left: Vec<HeldRow> = Vec::new();
            if owned == Some(PlaceProvisionRowOutcome::Present) {
                let note = was_there_line(&self.record.name);
                left.push(held_of(PlaceProvisionRow {
                    note: Some(note),
                    ..plain_row(&u.key, &u.label, PlaceProvisionRowOutcome::Skipped)
                }));
            }
            if let Some(dests) = &u.dests {
                let stores_now = self.stores_here().await.unwrap_or_default();
                let out = unland_files(&*machine, &self.home, dests, &stores_now).await;
                match &out {
                    None => left.push(held_of(PlaceProvisionRow {
                        note: Some(UNLAND_FAILED_LINE.into()),
                        ..plain_row(&u.key, &u.label, PlaceProvisionRowOutcome::Failed)
                    })),
                    Some(out) if !out.kept.is_empty() => {
                        let note = edited_there_line(&self.record.name, &out.kept);
                        left.push(held_of(PlaceProvisionRow {
                            note: Some(note),
                            ..plain_row(&u.key, &u.label, PlaceProvisionRowOutcome::Skipped)
                        }));
                    }
                    Some(_) => {}
                }
                let said = match &out {
                    None => "its files were not taken off".to_string(),
                    Some(out) => format!(
                        "{} files taken off, {} kept",
                        out.gone.len(),
                        out.kept.len()
                    ),
                };
                self.log.write(&format!("{}: {said}", u.label));
            }
            let project = self
                .rows
                .borrow()
                .iter()
                .find(|r| Some(&r.row.id) == u.owner.as_ref())
                .and_then(|r| r.row.project.clone());
            if let (Some(_), Some(project)) = (u.folder.as_ref(), project.as_ref()) {
                if let Err(e) = self
                    .area
                    .ctx
                    .recording
                    .remove_folder(&self.ask.place_id, &project.id)
                    .await
                {
                    self.log
                        .write(&format!("{}: {}", u.label, first_line_of(&e)));
                }
            }
            if u.cmd.is_none() && ours {
                if let Some(note) = &u.note {
                    left.push(held_of(PlaceProvisionRow {
                        note: Some(note.clone()),
                        ..plain_row(&u.key, &u.label, PlaceProvisionRowOutcome::Skipped)
                    }));
                }
            }
            if let (Some(cmd), true) = (u.cmd.as_ref(), ours) {
                let (exit, stdout, stderr) = match machine
                    .exec(
                        cmd,
                        ExecOptions {
                            timeout_ms: Some(UNDO_MS),
                            ..Default::default()
                        },
                    )
                    .await
                {
                    Ok(res) => (res.exit_code.get(), res.stdout, res.stderr),
                    Err(e) => (-1, String::new(), first_line_of(&host_of(&e))),
                };
                if exit != 0 {
                    let words = if stderr.is_empty() { stdout } else { stderr };
                    let note = format!(
                        "not taken off: {}",
                        last_line(&words).unwrap_or_else(|| format!("exit {exit}"))
                    );
                    left.push(held_of(PlaceProvisionRow {
                        note: Some(note),
                        ..plain_row(&u.key, &u.label, PlaceProvisionRowOutcome::Failed)
                    }));
                }
                self.log.write(&format!(
                    "{}: {}",
                    u.label,
                    if exit == 0 {
                        "taken off"
                    } else {
                        "not taken off"
                    }
                ));
            }
            let (ids, key) = (u.ids.clone(), u.key.clone());
            self.rows
                .borrow_mut()
                .retain(|r| !ids.contains(&r.row.id) && r.row.id != key);
            self.rows.borrow_mut().extend(left);
        }
        Ok(())
    }
}
