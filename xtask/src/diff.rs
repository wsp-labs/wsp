// SPDX-License-Identifier: AGPL-3.0-only
//! A structural diff of two JSON answers, each difference named by its field's path, as `replies[2].places[0].name`.

use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Difference {
    pub field: String,
    pub golden: String,
    pub live: String,
}

pub fn diff(golden: &Value, live: &Value) -> Vec<Difference> {
    let mut out = Vec::new();
    walk("", golden, live, &mut out);
    out
}

fn walk(path: &str, golden: &Value, live: &Value, out: &mut Vec<Difference>) {
    match (golden, live) {
        (Value::Object(g), Value::Object(l)) => {
            // The TS host writes an object's keys in the order it built them, on the wire and on disk, so a host that
            // sorts them, as serde_json does without preserve_order, answers differently.
            let order = |of: &serde_json::Map<String, Value>, other: &serde_json::Map<String, Value>| {
                of.keys().filter(|k| other.contains_key(*k)).cloned().collect::<Vec<_>>()
            };
            let (golden_order, live_order) = (order(g, l), order(l, g));
            if golden_order != live_order {
                out.push(Difference {
                    field: format!("{path} (key order)"),
                    golden: shown(Some(&Value::from(golden_order))),
                    live: shown(Some(&Value::from(live_order))),
                });
            }
            let mut keys: Vec<&String> = g.keys().chain(l.keys()).collect();
            keys.sort();
            keys.dedup();
            for k in keys {
                let field = if path.is_empty() { k.clone() } else { format!("{path}.{k}") };
                match (g.get(k), l.get(k)) {
                    (Some(a), Some(b)) => walk(&field, a, b, out),
                    (a, b) => out.push(Difference { field, golden: shown(a), live: shown(b) }),
                }
            }
        }
        (Value::Array(g), Value::Array(l)) => {
            for i in 0..g.len().max(l.len()) {
                let field = format!("{path}[{i}]");
                match (g.get(i), l.get(i)) {
                    (Some(a), Some(b)) => walk(&field, a, b, out),
                    (a, b) => out.push(Difference { field, golden: shown(a), live: shown(b) }),
                }
            }
        }
        (a, b) if a == b => {}
        (Value::String(a), Value::String(b)) => {
            let (golden, live) = around_first_difference(a, b);
            out.push(Difference { field: path.to_string(), golden, live });
        }
        (a, b) => out.push(Difference { field: path.to_string(), golden: shown(Some(a)), live: shown(Some(b)) }),
    }
}

/// Two long strings shown where they part, a little before and after, so a changed word deep in a page of text is the
/// thing printed rather than the opening both share.
fn around_first_difference(a: &str, b: &str) -> (String, String) {
    let (ac, bc): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if ac.len() <= 100 && bc.len() <= 100 {
        return (shown(Some(&Value::from(a))), shown(Some(&Value::from(b))));
    }
    let at = ac.iter().zip(&bc).position(|(x, y)| x != y).unwrap_or(ac.len().min(bc.len()));
    let from = at.saturating_sub(40);
    let cut = |c: &[char]| {
        let end = (at + 60).min(c.len());
        let piece: String = c[from.min(c.len())..end].iter().collect();
        format!("{}{}{}", if from > 0 { "..." } else { "" }, Value::from(piece), if end < c.len() { "..." } else { "" })
    };
    (cut(&ac), cut(&bc))
}

/// A value as a difference prints it, cut so one line stays one line.
fn shown(v: Option<&Value>) -> String {
    let Some(v) = v else { return "(absent)".into() };
    let text = v.to_string();
    if text.chars().count() <= 120 {
        return text;
    }
    let head: String = text.chars().take(117).collect();
    format!("{head}...")
}
