// SPDX-License-Identifier: AGPL-3.0-only
//! Port of packages/runtime/src/recipe-sync.ts at 65f6b89c7: what moved between the recipe a computer last applied and
//! the recipe it follows now, row by row, and which steps of the setup carry each move. Spec: spec/setup_steps.md.
use indexmap::{IndexMap, IndexSet};
use serde::Serialize;
use wsp_wire::types::recipe_file::{RecipeFile, RecipeName};
use wsp_wire::views::place::PlaceSetupStep;
use wsp_words::recipe_file::RecipeKind;

const KINDS: [RecipeKind; 7] = [
    RecipeKind::Agents,
    RecipeKind::Mcp,
    RecipeKind::Clis,
    RecipeKind::Skills,
    RecipeKind::Plugins,
    RecipeKind::Folders,
    RecipeKind::Configs,
];

fn kind_key(kind: RecipeKind) -> &'static str {
    match kind {
        RecipeKind::Agents => "agents",
        RecipeKind::Mcp => "mcp",
        RecipeKind::Clis => "clis",
        RecipeKind::Skills => "skills",
        RecipeKind::Plugins => "plugins",
        RecipeKind::Folders => "folders",
        RecipeKind::Configs => "configs",
    }
}

/// Each row of one kind by name, as its JSON text: two rows are equal where their texts are.
fn rows_of(file: &RecipeFile, kind: RecipeKind) -> IndexMap<String, String> {
    fn text<T: Serialize>(table: &IndexMap<RecipeName, T>) -> IndexMap<String, String> {
        table.iter().map(|(name, row)| (name.0.clone(), serde_json::to_string(row).unwrap_or_default())).collect()
    }
    match kind {
        RecipeKind::Agents => text(&file.agents),
        RecipeKind::Mcp => text(&file.mcp),
        RecipeKind::Clis => text(&file.clis),
        RecipeKind::Skills => text(&file.skills),
        RecipeKind::Plugins => text(&file.plugins),
        RecipeKind::Folders => text(&file.folders),
        RecipeKind::Configs => {
            let configs = &file.configs;
            let mut out = IndexMap::new();
            if let Some(git) = &configs.git {
                out.insert("git".to_string(), serde_json::to_string(git).unwrap_or_default());
            }
            if let Some(shell) = &configs.shell {
                out.insert("shell".to_string(), serde_json::to_string(shell).unwrap_or_default());
            }
            if let Some(github) = &configs.github {
                out.insert("github".to_string(), serde_json::to_string(github).unwrap_or_default());
            }
            out
        }
    }
}

/// The sorted (key, value) pairs of the items a row reads: its own key and the keys under `<key>/`.
fn items_of<'a>(items: &'a IndexMap<String, String>, key: &str) -> Vec<(&'a String, &'a String)> {
    let under = format!("{key}/");
    let mut out: Vec<(&String, &String)> = items.iter().filter(|(k, _)| k.as_str() == key || k.starts_with(&under)).collect();
    out.sort_by(|a, b| a.0.cmp(b.0));
    out
}

/// How one row moved (recipe-sync.ts:13).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecipeChangeHow {
    Added,
    Changed,
    Edited,
    Removed,
}

/// One row that moved, keyed `<kind>/<row>` as a resolved recipe keys it (recipe-sync.ts:10).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecipeChange {
    pub key: String,
    pub kind: RecipeKind,
    pub name: String,
    pub how: RecipeChangeHow,
}

/// Every row added, changed, edited or taken out, kind by kind in RECIPE_KINDS order: within a kind the rows of
/// `after` in its order, then the rows only `before` has. recipe-sync.ts:36.
pub fn recipe_changes(
    before: Option<&RecipeFile>,
    before_items: Option<&IndexMap<String, String>>,
    after: &RecipeFile,
    after_items: &IndexMap<String, String>,
) -> Vec<RecipeChange> {
    let mut out = Vec::new();
    let keys_moved = before.is_some_and(|b| b.copy_keys.is_some() != after.copy_keys.is_some());
    for kind in KINDS {
        let was = before.map_or_else(IndexMap::new, |b| rows_of(b, kind));
        let now = rows_of(after, kind);
        for (name, row) in &now {
            let key = format!("{}/{name}", kind_key(kind));
            let how = match was.get(name) {
                None => RecipeChangeHow::Added,
                Some(old) if old != row || (kind == RecipeKind::Mcp && keys_moved) => RecipeChangeHow::Changed,
                Some(_) => {
                    let moved = match before_items {
                        None => !items_of(after_items, &key).is_empty(),
                        Some(before_items) => items_of(before_items, &key) != items_of(after_items, &key),
                    };
                    if !moved {
                        continue;
                    }
                    RecipeChangeHow::Edited
                }
            };
            out.push(RecipeChange { key, kind, name: name.clone(), how });
        }
        for name in was.keys().filter(|name| !now.contains_key(*name)) {
            out.push(RecipeChange { key: format!("{}/{name}", kind_key(kind)), kind, name: name.clone(), how: RecipeChangeHow::Removed });
        }
    }
    out
}

/// The steps that carry a set of changes, in the order first added. recipe-sync.ts:56.
pub fn steps_for(changes: &[RecipeChange], after: &RecipeFile) -> IndexSet<PlaceSetupStep> {
    let mut steps = IndexSet::new();
    for c in changes {
        if c.how == RecipeChangeHow::Removed {
            if c.kind == RecipeKind::Mcp {
                steps.insert(PlaceSetupStep::Mcp);
            }
            if matches!(c.kind, RecipeKind::Agents | RecipeKind::Clis) {
                steps.insert(PlaceSetupStep::Context);
            }
            continue;
        }
        match c.kind {
            RecipeKind::Agents => match c.how {
                RecipeChangeHow::Edited => {
                    steps.insert(PlaceSetupStep::Mcp);
                }
                RecipeChangeHow::Changed => {
                    steps.insert(PlaceSetupStep::Signins);
                }
                _ => {
                    for s in [
                        PlaceSetupStep::Agents,
                        PlaceSetupStep::Signins,
                        PlaceSetupStep::Mcp,
                        PlaceSetupStep::Skills,
                        PlaceSetupStep::Context,
                    ] {
                        steps.insert(s);
                    }
                }
            },
            RecipeKind::Clis => {
                steps.insert(PlaceSetupStep::Clis);
                steps.insert(PlaceSetupStep::Context);
                let needs = after.clis.iter().find(|(n, _)| n.0 == c.name).is_some_and(|(_, row)| row.needs.is_some());
                if c.how == RecipeChangeHow::Added && needs {
                    steps.insert(PlaceSetupStep::Floor);
                }
            }
            RecipeKind::Configs => {
                steps.insert(if c.name == "github" { PlaceSetupStep::Github } else { PlaceSetupStep::Configs });
            }
            RecipeKind::Mcp => {
                steps.insert(PlaceSetupStep::Mcp);
            }
            RecipeKind::Skills => {
                steps.insert(PlaceSetupStep::Skills);
            }
            RecipeKind::Plugins => {
                steps.insert(PlaceSetupStep::Plugins);
            }
            RecipeKind::Folders => {
                steps.insert(PlaceSetupStep::Folders);
            }
        }
    }
    if steps.contains(&PlaceSetupStep::Folders) {
        steps.insert(PlaceSetupStep::FolderServers);
    }
    steps
}
