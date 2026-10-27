// SPDX-License-Identifier: AGPL-3.0-only
//! `computers`: every computer this host holds, every add not yet set up, and what each cloud spent today, two reads
//! answered as one value;
//! `computers_set`: what the person sets on one of them, answered as its row.

use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;
use serde_json::{Map, Number, Value};

use super::create::{size_from_word, size_refused};
use super::target::place_named;
use super::workspace::agents_asked;
use super::{entry_in, held_field, input, input_refusal, refused_field, Answer, Refused, Tool};
use crate::host::Host;
use crate::record;

const NAME: &str = "computers";

pub const TOOL: Tool = Tool {
    name: NAME,
    listed: include_str!(concat!(env!("OUT_DIR"), "/record/tools/computers.json")),
    call: |host, args| Box::pin(call(host, args)),
};

#[derive(Debug, Default, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct In {}

/// The rows pass through in the bytes the host wrote them, which is what keeps the text byte for byte.
#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct Out {
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    pub computers: Vec<Box<RawValue>>,
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    pub spend: Vec<Box<RawValue>>,
    #[cfg_attr(test, schemars(with = "Vec<serde_json::Value>"))]
    pub pending: Vec<Box<RawValue>>,
}

#[derive(Deserialize)]
struct Places {
    places: Vec<Box<RawValue>>,
    #[serde(default)]
    pending: Vec<Box<RawValue>>,
}

async fn call(host: std::sync::Arc<Host>, arguments: serde_json::Value) -> Result<Answer, Refused> {
    let In {} = input(NAME, arguments)?;
    let client = host.client().await?;
    let (listed, spent) =
        tokio::try_join!(client.request::<Places>("places.list", Map::new()), client.request::<Places>("cost.spend", Map::new()))?;
    Ok(Answer::json(&Out { computers: listed.places, spend: spent.places, pending: listed.pending }))
}

const SET_NAME: &str = "computers_set";

const SET_LISTED: &str = include_str!(concat!(env!("OUT_DIR"), "/record/tools/computers_set.json"));

pub const SET: Tool = Tool { name: SET_NAME, listed: SET_LISTED, call: |host, args| Box::pin(set(host, args)) };

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct SetIn {
    pub computer: String,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub ssh: Option<String>,
    #[serde(default)]
    #[cfg_attr(test, schemars(with = "Option<u64>"))]
    pub threads: Option<Number>,
    #[serde(default)]
    #[cfg_attr(test, schemars(with = "Option<u64>"))]
    pub machines: Option<Number>,
    #[serde(default)]
    #[cfg_attr(test, schemars(with = "Option<f64>"))]
    pub spend: Option<Number>,
    #[serde(default)]
    pub size: Option<String>,
    #[serde(default)]
    #[cfg_attr(test, schemars(with = "Option<u64>"))]
    pub delete_after: Option<Number>,
    #[serde(default)]
    #[cfg_attr(test, schemars(with = "Option<u64>"))]
    pub nap: Option<Number>,
    #[serde(default)]
    #[cfg_attr(test, schemars(with = "Option<u64>"))]
    pub turn_limit: Option<Number>,
    #[serde(default)]
    #[cfg_attr(test, schemars(with = "Option<u64>"))]
    pub max_machines: Option<Number>,
    #[serde(default)]
    #[cfg_attr(test, schemars(with = "Option<u64>"))]
    pub max_depth: Option<Number>,
    #[serde(default)]
    pub recipe: Option<String>,
    #[serde(default)]
    pub reset: Option<Vec<String>>,
}

#[derive(Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(schemars::JsonSchema))]
pub struct SetOut {
    #[cfg_attr(test, schemars(with = "serde_json::Value"))]
    pub computer: Box<RawValue>,
}

#[derive(Deserialize)]
struct Placed {
    place: Box<RawValue>,
}

async fn set(host: std::sync::Arc<Host>, arguments: Value) -> Result<Answer, Refused> {
    let SetIn {
        computer,
        name,
        ssh,
        threads,
        machines,
        spend,
        size,
        delete_after,
        nap,
        turn_limit,
        max_machines,
        max_depth,
        recipe,
        reset,
    } = input(SET_NAME, arguments)?;
    let client = host.client().await?;
    let words = record::words();
    let mut frame = Map::new();
    let (place, offered) = place_named(&client, words, &computer).await?;
    frame.insert("placeId".to_owned(), Value::from(place));
    for (key, value) in [("threads", threads), ("machines", machines), ("spendPerDayUsd", spend)] {
        if let Some(n) = value {
            frame.insert(key.to_owned(), Value::Number(n));
        }
    }
    // sizeAsked's rule: a word that names no size is refused here over the row's sizes where the row offers any (only a
    // cloud's does, which is placeTakes for a size); anywhere else it goes as a reset of the size, for the host's
    // refusal of the kind.
    let mut reset = reset.unwrap_or_default();
    if let Some(word) = size {
        match size_from_word(&word) {
            Some((cpu, mem_mb)) => {
                let mut figures = Map::new();
                figures.insert("cpu".to_owned(), Value::from(cpu as u64));
                figures.insert("memMb".to_owned(), Value::from(mem_mb as u64));
                frame.insert("size".to_owned(), Value::Object(figures));
            }
            None if !offered.is_empty() => return Err(size_refused(&word, &offered).into()),
            None if !reset.iter().any(|w| w == "size") => reset.push("size".to_owned()),
            None => {}
        }
    }
    if let Some(minutes) = &delete_after {
        let ms = minutes
            .as_u64()
            .ok_or_else(|| refused_field(SET_NAME, SET_LISTED, host.cloud(), "delete_after", Value::Number(minutes.clone())))?;
        frame.insert("deleteAfterMs".to_owned(), Value::from(ms * 60_000));
    }
    if let Some(minutes) = &nap {
        let window = nap_asked(entry_in(SET_LISTED, host.cloud()).unwrap_or_default(), minutes).map_err(Refused::Input)?;
        frame.insert("napMs".to_owned(), window);
    }
    if let Some(hours) = &turn_limit {
        let limit = turn_limit_asked(entry_in(SET_LISTED, host.cloud()).unwrap_or_default(), hours).map_err(Refused::Input)?;
        frame.insert("turnLimitMs".to_owned(), limit);
    }
    if let Some(asked) = agents_asked(max_machines.as_ref(), max_depth.as_ref()) {
        frame.insert("spawn".to_owned(), Value::Object(asked));
    }
    if !reset.is_empty() {
        frame.insert("reset".to_owned(), Value::from(reset));
    }
    // One frame carries every argument, as the command line sends them, so the host checks them all before it writes.
    for (key, value) in [("name", name), ("ssh", ssh), ("recipe", recipe)] {
        if let Some(word) = value {
            frame.insert(key.to_owned(), Value::from(word));
        }
    }
    let Placed { place } = client.request("places.set", frame).await?;
    Ok(Answer::json(&SetOut { computer: place }))
}

/// The nap window a call names in minutes, held to the entry's range in its own words: none of them never naps, the
/// rule napMsOf keeps on the host's side.
fn nap_asked(entry: &str, minutes: &Number) -> Result<Value, String> {
    held_field(SET_NAME, entry, "nap", &Value::Number(minutes.clone()))?;
    let Some(m) = minutes.as_u64() else { return Err(input_refusal(SET_NAME, &format!("nap cannot be read as {minutes}"))) };
    Ok(if m == 0 { Value::Null } else { Value::from(m * 60_000) })
}

/// The turn limit a call names in hours, held to the entry's range in its own words: none of them is no limit, the
/// rule turnLimitMsOf keeps on the host's side.
fn turn_limit_asked(entry: &str, hours: &Number) -> Result<Value, String> {
    held_field(SET_NAME, entry, "turn_limit", &Value::Number(hours.clone()))?;
    let Some(h) = hours.as_u64() else { return Err(input_refusal(SET_NAME, &format!("turn_limit cannot be read as {hours}"))) };
    Ok(if h == 0 { Value::Null } else { Value::from(h * 3_600_000) })
}

#[cfg(test)]
mod tests {
    use serde_json::{json, Number, Value};

    use super::super::entry_in;
    use super::{nap_asked, turn_limit_asked, SET};

    fn minutes(v: Value) -> Number {
        v.as_number().unwrap().clone()
    }

    /// What the TypeScript server refused this call with, off the record.
    fn recorded(arguments: Value) -> String {
        let all: Vec<Value> = serde_json::from_str(include_str!("../../tests/refusals.json")).unwrap();
        let found = all.iter().find(|r| r["tool"] == "computers_set" && r["arguments"] == arguments).expect("a recorded refusal");
        found["text"].as_str().unwrap().to_owned()
    }

    #[test]
    fn a_nap_is_whole_minutes_in_range_or_none_and_anything_else_is_refused_in_the_inputs_words() {
        let entry = entry_in(SET.listed, false).unwrap();
        assert_eq!(nap_asked(entry, &minutes(json!(0))).unwrap(), Value::Null);
        assert_eq!(nap_asked(entry, &minutes(json!(45))).unwrap(), json!(2_700_000));
        for given in [json!(2.5), json!(-1), json!(181)] {
            let refused = nap_asked(entry, &minutes(given.clone())).expect_err(&format!("{given} was taken"));
            assert_eq!(refused, recorded(json!({ "computer": "attic", "nap": given })));
        }
    }

    #[test]
    fn a_turn_limit_is_whole_hours_in_range_or_none_and_anything_else_is_refused_in_the_inputs_words() {
        let entry = entry_in(SET.listed, false).unwrap();
        assert_eq!(turn_limit_asked(entry, &minutes(json!(0))).unwrap(), Value::Null);
        assert_eq!(turn_limit_asked(entry, &minutes(json!(12))).unwrap(), json!(43_200_000));
        for given in [json!(1.5), json!(-1), json!(25)] {
            let refused = turn_limit_asked(entry, &minutes(given.clone())).expect_err(&format!("{given} was taken"));
            assert_eq!(refused, recorded(json!({ "computer": "attic", "turn_limit": given })));
        }
    }

    #[test]
    fn its_structs_are_the_recorded_schemas() {
        super::super::held::to_the_record::<super::In, super::Out>(super::TOOL.listed);
        super::super::held::to_the_record::<super::SetIn, super::SetOut>(super::SET.listed);
    }
}
