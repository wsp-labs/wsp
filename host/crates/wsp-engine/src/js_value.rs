// SPDX-License-Identifier: AGPL-3.0-only
//! What JavaScript writes for a value a provider's JSON carries where the TS never checked its type.
use serde_json::Value;
use wsp_words::js::number_string;

/// `String(value)`: what a template literal writes for a JSON value.
pub fn js_string(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Null => "null".to_string(),
        Value::Number(n) => number_string(n.as_f64().unwrap_or(f64::NAN)),
        Value::Bool(b) => b.to_string(),
        Value::Array(items) => array_string(items),
        Value::Object(_) => "[object Object]".to_string(),
    }
}

/// An array's `String(...)`, its nested arrays written in place, so the depth of a tree is no depth of the stack.
fn array_string(items: &[Value]) -> String {
    let mut out = String::new();
    let mut open = vec![(items.iter(), false)];
    while let Some((rest, started)) = open.last_mut() {
        let Some(item) = rest.next() else {
            open.pop();
            continue;
        };
        if std::mem::replace(started, true) {
            out.push(',');
        }
        match item {
            Value::Null => {}
            Value::Array(inner) => open.push((inner.iter(), false)),
            other => out.push_str(&js_string(other)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::js_string;
    use serde_json::json;

    // node: String([1,[2,null,[3,[]]],null,"a",true,{},[[],[]],[null,[null]],1.5,-0])
    #[test]
    fn nested_arrays_write_as_node_joins_them() {
        let value = json!([1, [2, null, [3, []]], null, "a", true, {}, [[], []], [null, [null]], 1.5, -0.0]);
        assert_eq!(js_string(&value), "1,2,,3,,,a,true,[object Object],,,,,1.5,0");
    }
}
