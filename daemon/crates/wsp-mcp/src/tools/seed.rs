// SPDX-License-Identifier: AGPL-3.0-only
//! `projects_seed`: what git ignores in a project's folder on this computer and which of it goes to a sub-thread's
//! machine, as the host read it, and the picks saved where the call confirms them.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Value};

use super::{entry_in, input, Answer, Refused, Tool};
use crate::host::Host;
use crate::zod::{self, Schema};

const NAME: &str = "projects_seed";

const LISTED: &str = include_str!(concat!(env!("OUT_DIR"), "/record/tools/projects_seed.json"));

pub const TOOL: Tool = Tool { name: NAME, listed: LISTED, call: |host, args| Box::pin(call(host, args)) };

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct In {
    pub project: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub keep: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cut: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub none: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Out {
    pub project: String,
    pub folder: String,
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    pub rows: Box<RawValue>,
    #[cfg_attr(test, schemars(with = "serde_json::Value"))]
    pub picked: Box<RawValue>,
    pub saved: bool,
}

async fn call(host: std::sync::Arc<Host>, arguments: Value) -> Result<Answer, Refused> {
    let In { project, keep, cut, none, confirm } = input(NAME, arguments)?;
    let client = host.client().await?;
    let mut asked = Map::new();
    asked.insert("project".to_owned(), Value::from(project));
    for (field, paths) in [("keep", keep), ("cut", cut)] {
        if let Some(paths) = paths.filter(|p| !p.is_empty()) {
            asked.insert(field.to_owned(), Value::from(paths));
        }
    }
    if none == Some(true) {
        asked.insert("none".to_owned(), Value::Bool(true));
    }
    if confirm == Some(true) {
        asked.insert("save".to_owned(), Value::Bool(true));
    }
    let seeded = client.request::<Box<RawValue>>("projects.seed", asked).await?;
    // The answer as the TypeScript tool's zod parse leaves it: the schema's keys in its order, the frame's id gone.
    let root = Schema::output_of(entry_in(LISTED, host.cloud()).unwrap_or_default());
    let out = zod::parsed(&root, &root, &seeded)
        .and_then(|parsed| serde_json::from_str::<Out>(parsed.get()).ok())
        .ok_or_else(|| super::agents::unread(NAME))?;
    Ok(Answer::json(&out))
}

#[cfg(test)]
mod tests {
    #[test]
    fn its_structs_are_the_recorded_schemas() {
        super::super::held::to_the_record::<super::In, super::Out>(super::TOOL.listed);
    }
}
