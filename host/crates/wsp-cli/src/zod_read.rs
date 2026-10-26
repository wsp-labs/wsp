// SPDX-License-Identifier: AGPL-3.0-only
//! A reply the TS reads with zod (`Shape.parse(reply)`) and the host's answer fails: the error is the issue list as
//! `ZodError.message` writes it, which `host_schema_refusal` turns into the verb's usage line (exit 3) and a tool
//! prints as it is.
use serde::de::DeserializeOwned;
use serde_json::Value;
use wsp_wire::types::check::issues_of;
use wsp_wire::types::{parse, Check, SchemaError};
use wsp_words::exit::HostError;

/// `Shape.parse(value)`: the value read as `T`, else a plain error holding the issue list zod's `ZodError.message` writes.
pub fn zod_read<T: DeserializeOwned + Check>(value: &Value) -> Result<T, HostError> {
    parse::<T>(value).map_err(|first| {
        let mut issues = issues_of::<T>(value);
        if issues.is_empty() {
            issues.push(first);
        }
        zod_refusal(&issues)
    })
}

/// `Shape.parse(reply.field)`: a field the reply lacks is `undefined` to zod, which says it is required.
pub fn zod_read_field<T: DeserializeOwned + Check>(value: Option<&Value>) -> Result<T, HostError> {
    let Some(value) = value else {
        let mut first = parse::<T>(&Value::Null).err().ok_or_else(|| HostError::new("Required"))?;
        if first.code == "invalid_type" {
            first.detail.insert("received".to_owned(), Value::from("undefined"));
            first.message = "Required".to_owned();
        }
        return Err(zod_refusal(&[first]));
    };
    zod_read(value)
}

/// The error a failed `Shape.parse` throws: `JSON.stringify(issues, null, 2)`.
pub fn zod_refusal(issues: &[SchemaError]) -> HostError {
    let listed: Vec<Value> = issues
        .iter()
        .filter_map(|issue| serde_json::from_str::<Value>(&issue.issue_text()).ok())
        .filter_map(|list| match list {
            Value::Array(mut one) if one.len() == 1 => one.pop(),
            _ => None,
        })
        .collect();
    HostError::new(serde_json::to_string_pretty(&Value::Array(listed)).unwrap_or_default())
}
