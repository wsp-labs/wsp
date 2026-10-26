// SPDX-License-Identifier: AGPL-3.0-only
//! Ported from packages/runtime/src/serve-projects.ts at 65f6b89c7: the project ops a socket asks, answered off the
//! runtime's projects, and the computers a thread's own token is answered. Spec: spec/serve_helpers.md, H20-H21.
use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;
use wsp_core::types::api::{ProjectAddAsk, ProjectRemoved, ProjectsApi, SeedAsk};
use wsp_wire::views::place_kind::PlaceKind;
use wsp_wire::views::place_more::PlaceView;
use wsp_wire::views::workspace::{scope_of, Caller};
use wsp_wire::wire::runtime_ops::{RuntimeOp, RuntimeRequest};
use wsp_words::exit::HostError;
use wsp_words::place_word::{place_said, PlaceRef};
use wsp_words::projects::runs_in_folder;

use crate::serve::served::{DynRuntime, Payload};

/// `msg.op.startsWith("projects.")`.
pub fn is_project_request(msg: &RuntimeRequest) -> bool {
    msg.op.name().starts_with("projects.")
}

/// `answerProject(projects, msg, origin)`. serve-projects.ts:10-33.
pub async fn answer_project(projects: &dyn ProjectsApi, msg: &RuntimeRequest, origin: Option<Caller>) -> Result<Payload, HostError> {
    match &msg.op {
        RuntimeOp::ProjectsAdd { source, on, name, base, into, seed, .. } => {
            let ask = ProjectAddAsk {
                source: source.clone(),
                on: on.clone(),
                name: name.clone(),
                base: base.clone(),
                into: into.clone(),
                seed: seed.clone(),
            };
            let added = projects.add(ask, origin).await?;
            let mut out = Payload::new();
            out.insert("project".into(), to_value(&added.project)?);
            if let Some(notice) = added.notice {
                out.insert("notice".into(), Value::from(notice));
            }
            Ok(out)
        }
        RuntimeOp::ProjectsSeed { project, keep, cut, none, save, .. } => {
            let ask = SeedAsk { keep: keep.clone(), cut: cut.clone(), none: none.filter(|none| *none), save: save.filter(|save| *save) };
            spread(&projects.seed(project, ask, origin).await?)
        }
        RuntimeOp::ProjectsList { .. } => one("projects", to_value(&projects.list(origin).await?)?),
        RuntimeOp::ProjectsDefaults { .. } => one("defaults", to_value(&projects.defaults(origin).await?)?),
        RuntimeOp::ProjectsResolve { r#ref, .. } => one("project", to_value(&projects.resolve(r#ref, origin).await?)?),
        RuntimeOp::ProjectsBranch { project_id, .. } => spread(&projects.branch(project_id, origin).await?),
        RuntimeOp::ProjectsRemove { project_id, force, check, .. } => {
            let removed = projects.remove(project_id, origin, force.filter(|force| *force), check.filter(|check| *check)).await?;
            removed_payload(&removed)
        }
        RuntimeOp::ProjectsIcon { project_id, png, .. } => one("image", Value::from(projects.icon(project_id, png.clone()).await?)),
        RuntimeOp::ProjectsIcons { hashes, .. } => {
            let hashes = hashes.iter().map(|hash| hash.0.clone()).collect();
            one("icons", to_value(&projects.icons(hashes).await?)?)
        }
        _ => Err(HostError::new(format!("{} is not answered here", msg.op.name()))),
    }
}

/// `threadPlaces(rt, origin, now)`: every cloud, and for a thread in a folder each computer holding a project it may
/// start work on; each row less its road, a provider's labelled. serve-projects.ts:35-44, spec H20.
pub async fn thread_places(rt: &DynRuntime, origin: Option<Caller>, now: i64) -> Result<Vec<PlaceView>, HostError> {
    let asking = match scope_of(origin.as_ref()) {
        Some(scope) => rt.workspaces().get(&scope.workspace_id, None, None).await.ok(),
        None => None,
    };
    let runs_in_a_folder = asking.and_then(|workspace| workspace.kind).is_some_and(|kind| runs_in_folder(&kind));
    let reached: HashSet<String> = if runs_in_a_folder {
        rt.projects().list(origin).await?.into_iter().map(|project| project.computer).collect()
    } else {
        HashSet::new()
    };
    let rows = match rt.places() {
        Some(places) => places.list(u64::try_from(now).unwrap_or(0)).await?,
        None => Vec::new(),
    };
    Ok(rows
        .into_iter()
        .filter(|row| row.kind == PlaceKind::Provider || reached.contains(&row.id))
        .map(|mut row| {
            row.road = None;
            if row.kind == PlaceKind::Provider {
                row.label = Some(place_said(&PlaceRef { id: row.id.clone(), name: row.name.clone(), kind: Some("provider".into()) }));
            }
            row
        })
        .collect())
}

/// `thread_places` rows for a reply: a provider's `label` goes last, as serve-projects.ts:43 `{ ...p, label }` puts it.
pub fn thread_place_rows(rows: &[PlaceView]) -> Result<Value, HostError> {
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut value = to_value(row)?;
        if let (PlaceKind::Provider, Value::Object(fields)) = (&row.kind, &mut value) {
            if let Some(label) = fields.remove("label") {
                fields.insert("label".into(), label);
            }
        }
        out.push(value);
    }
    Ok(Value::Array(out))
}

/// `projects.remove`'s answer: `unsaved` only where something was left unsaved.
fn removed_payload(removed: &ProjectRemoved) -> Result<Payload, HostError> {
    let mut out = Payload::new();
    out.insert("said".into(), Value::from(removed.said.clone()));
    if let Some(unsaved) = &removed.unsaved {
        out.insert("unsaved".into(), Value::from(unsaved.clone()));
    }
    Ok(out)
}

fn one(key: &str, value: Value) -> Result<Payload, HostError> {
    let mut out = Payload::new();
    out.insert(key.into(), value);
    Ok(out)
}

fn to_value<T: Serialize>(answer: &T) -> Result<Value, HostError> {
    serde_json::to_value(answer).map_err(|e| HostError::new(e.to_string()))
}

/// The answer's fields beside id and ok, as the TS spreads them.
fn spread<T: Serialize>(answer: &T) -> Result<Payload, HostError> {
    match to_value(answer)? {
        Value::Object(fields) => Ok(fields),
        _ => Err(HostError::new("the answer is not an object")),
    }
}
