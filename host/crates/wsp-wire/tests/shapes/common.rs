// SPDX-License-Identifier: AGPL-3.0-only
//! Runs one shape's fixture cases: each sample zod parsed must parse here and write back the same JSON text, keys in
//! the same order, absent and null where zod left them; each sample zod refused must be refused with zod's code and
//! zod's path, whether serde's reading of the shape or a check's rule refuses it.

use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;
use wsp_wire::types::{parse, Check};

fn of_type<'a>(all: &'a [Value], ty: &str) -> Vec<&'a Value> {
    let mine: Vec<&Value> = all.iter().filter(|c| c["type"] == ty).collect();
    assert!(!mine.is_empty(), "no fixture cases for {ty}");
    mine
}

fn run<T: DeserializeOwned + Serialize + Check>(c: &Value, ty: &str, strip: Option<&str>) {
    let case = c["case"].as_str().unwrap_or("?");
    let input = &c["input"];
    if let Some(out) = c.get("output") {
        let read: T = parse(input).unwrap_or_else(|e| panic!("{ty}, {case}: zod parsed it, refused here: {e:?}"));
        let text = serde_json::to_string(&read).expect("serializes");
        if c.get("anyOrder") == Some(&Value::Bool(true)) {
            let want: Value = serde_json::from_str(out.as_str().expect("output is text")).expect("output is JSON");
            let got: Value = serde_json::from_str(&text).expect("written JSON");
            assert_eq!(got, want, "{ty}, {case}: written unlike zod's output (key order aside: the TS code writes its own)");
        } else {
            assert_eq!(text, out.as_str().expect("output is text"), "{ty}, {case}: written unlike zod's output");
        }
        if let Some(key) = strip {
            assert!(
                !text.contains(&format!("\"{key}\":")) || out.as_str().unwrap().contains(&format!("\"{key}\":")),
                "{ty}, {case}: the tag key is the enum's to write"
            );
        }
        return;
    }
    let issue = &c["issue"];
    let err = match parse::<T>(input) {
        Ok(read) => panic!("{ty}, {case}: zod refused it with {issue}, read here as {}", serde_json::to_string(&read).unwrap()),
        Err(e) => e,
    };
    assert_eq!(err.code, issue["code"].as_str().unwrap(), "{ty}, {case}: wrong issue code ({err:?})");
    assert_eq!(Value::Array(err.path.clone()), issue["path"], "{ty}, {case}: wrong issue path ({err:?})");
}

/// Every case of one type.
pub fn cases<T: DeserializeOwned + Serialize + Check>(fixtures: &str, ty: &str) {
    let all: Vec<Value> = serde_json::from_str(fixtures).expect("fixture file is JSON");
    for c in of_type(&all, ty) {
        run::<T>(c, ty, None);
    }
}

/// Every case of a type whose tag key an enum writes: zod's output less that key, and the type's tag constant is the
/// value zod wrote for it.
pub fn cases_tagged<T: DeserializeOwned + Serialize + Check>(fixtures: &str, ty: &str, key: &str, tag: &str) {
    let all: Vec<Value> = serde_json::from_str(fixtures).expect("fixture file is JSON");
    for c in of_type(&all, ty) {
        if let Some(t) = c.get("tag") {
            assert_eq!(t["value"].as_str(), Some(tag), "{ty}: its {key} constant is not the value zod wrote");
        }
        run::<T>(c, ty, Some(key));
    }
}
