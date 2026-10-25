// SPDX-License-Identifier: AGPL-3.0-only
//! The one in-place editor of an agent's JSON or jsonc config: each edit made on the text so every comment and byte it
//! does not touch stays, and refused where the result would read as anything but the edits made to the parsed value.
//! Port of packages/catalog/src/mcp.ts:458-768 at a3b870a6e; the spec is spec/json_edit.md.
use serde_json::{Map, Value};

use crate::jsonc::{is_js_space, js_string, js_stringify, js_stringify_pretty, json_object, read_jsonc};
use crate::jsonc_tree::{comments_of, find_member, find_node_at_location, parse_tree, Comment, JsonPath, Node, NodeType, PathSeg};
use wsp_words::exit::HostError;
use wsp_words::js::js_trim;
use wsp_words::js::utf16;

/// One change: the value at the path set, or taken out where it is `None` (mcp.ts:486). A last step of `Index(-1)`
/// puts the value after an array's last element.
pub type JsonEdit = (JsonPath, Option<Value>);

/// What an edit that would drop a comment outside what it takes out is refused with (mcp.ts:620).
pub const LOST_COMMENT: &str = "the change would lose a comment that is not the server's, so it was not made; change the file by hand";

/// What an edit whose result does not read as the edited value is refused with (mcp.ts:717).
pub const NOT_IN_PLACE: &str = "the file could not be changed in place, which keeps its comments; change it by hand";

/// The text with the edits made in place (mcp.ts:724). Errors with "the file is not a JSON object" where the text is
/// not one, with `LOST_COMMENT`, or with `NOT_IN_PLACE`.
pub fn edit_json(text: &str, edits: &[JsonEdit]) -> Result<String, HostError> {
    let mut root = read_jsonc(text)?;
    if json_object(&root).is_none() {
        return Err(HostError::new("the file is not a JSON object"));
    }
    edit_jsonc(text, &mut root, edits)
}

/// The same over text whose parsed value the caller holds: `root` takes each edit as it is made, so on return it is
/// the parsed value of the text returned (mcp.ts:702). Runs of new keys into one object go in with one parse
/// (`newKeys`, mcp.ts:685).
pub fn edit_jsonc(text: &str, root: &mut Value, edits: &[JsonEdit]) -> Result<String, HostError> {
    match apply_edits(text, root, edits) {
        Ok(out) => Ok(out),
        Err(e) if e.message == LOST_COMMENT => Err(e),
        Err(_) => Err(not_in_place()),
    }
}

/// The edit made to a parsed value, a missing object on the way made as the in-place edit makes one (mcp.ts:490): a
/// number last on an array holder sets, appends (-1) or removes that element; a key is set or removed as an own key.
pub fn set_at(root: &mut Value, edit: &JsonEdit) {
    let (path, value) = edit;
    let Some((last, via)) = path.split_last() else {
        return;
    };
    set_in(root, via, last, value.as_ref());
}

/// A value with every object's keys sorted (UTF-16 order, rule 8.8) at every depth, written compactly as
/// `JSON.stringify` writes (mcp.ts:764): what makes two entries the same entry.
pub fn json_canonical(value: &Value) -> String {
    match value {
        Value::Array(items) => format!("[{}]", items.iter().map(json_canonical).collect::<Vec<_>>().join(",")),
        Value::Object(map) => {
            let mut keys: Vec<&str> = map.keys().map(String::as_str).collect();
            keys.sort_by(|a, b| utf16::cmp(a, b));
            let members: Vec<String> = keys.iter().map(|k| format!("{}:{}", js_string(k), json_canonical(&map[*k]))).collect();
            format!("{{{}}}", members.join(","))
        }
        _ => js_stringify(value),
    }
}

/// One edit made to the text (mcp.ts:625): a member taken out by `member_cuts`, a value put in where it stood, or put
/// into the deepest object or array of the path that is there; then `keeps_comments`.
pub fn edit_once(text: &str, edit: &JsonEdit) -> Result<String, HostError> {
    let (path, value) = edit;
    if path.is_empty() {
        return Err(not_in_place());
    }
    let comments = comments_of(text);
    let doc = parse_tree(text).ok_or_else(|| HostError::new("the file is not JSON"))?;
    let eol = line_end(text);
    let mut depth = path.len();
    while depth > 0 && find_node_at_location(&doc, &path[..depth]).is_none() {
        depth -= 1;
    }
    let node = find_node_at_location(&doc, &path[..depth]).unwrap_or(&doc);
    let lay = Layout { text, comments: &comments };
    let mut own = (0, 0);
    let out = match value {
        None => {
            if depth < path.len() {
                return Ok(text.to_string());
            }
            let (container, index) = find_member(&doc, path).ok_or_else(not_in_place)?;
            let (mut cuts, span) = member_cuts(text, &container.children, index, &comments);
            own = span;
            cuts.sort_by_key(|cut| std::cmp::Reverse(cut.0));
            let mut out = text.to_string();
            for (from, to) in cuts {
                out.replace_range(from..to, "");
            }
            out
        }
        Some(value) => {
            let wrapped = wrap(&path[(depth + 1).min(path.len())..], value);
            match (node.kind, path.get(depth)) {
                (NodeType::Object, Some(PathSeg::Key(key))) => inserted(
                    text,
                    node,
                    &|indent: Option<&str>| format!("{}: {}", js_string(key), rendered(&wrapped, indent, eol)),
                    &comments,
                ),
                (NodeType::Array, Some(PathSeg::Index(-1))) => {
                    inserted(text, node, &|indent: Option<&str>| rendered(&wrapped, indent, eol), &comments)
                }
                _ => {
                    own = (node.offset, node.offset + node.length);
                    let lined = text[node.offset..own.1].contains('\n');
                    let indent = lined.then(|| lay.indent(node.offset));
                    let put = rendered(&wrap(&path[depth..], value), indent.as_deref(), eol);
                    format!("{}{}{}", &text[..node.offset], put, &text[own.1..])
                }
            }
        }
    };
    keeps_comments(text, &comments, &out, own)?;
    Ok(out)
}

/// What taking out child `i` of a container cuts, and the span whose comments go with it (mcp.ts:560).
pub fn member_cuts(text: &str, kids: &[Node], i: usize, comments: &[Comment]) -> (Vec<(usize, usize)>, (usize, usize)) {
    let Some(unit) = kids.get(i) else {
        return (Vec::new(), (0, 0));
    };
    let lay = Layout { text, comments };
    let bytes = text.as_bytes();
    let unit_end = unit.offset + unit.length;
    let after = lay.past(unit_end);
    let comma = (bytes.get(after) == Some(&b',')).then_some(after);
    let end = comma.map_or(unit_end, |c| c + 1);
    let mut cuts = Vec::new();
    if comma.is_none() && i > 0 {
        let previous = &kids[i - 1];
        let before = lay.past(previous.offset + previous.length);
        if bytes.get(before) == Some(&b',') {
            cuts.push((before, before + 1));
        }
    }
    let rest = lay.line_rest(end);
    let line_from = lay.line_start(unit.offset);
    match rest {
        Some(rest) if js_trim(&text[line_from..unit.offset]).is_empty() => {
            let mut from = line_from;
            while from > 0 {
                let above = lay.line_start(from - 1);
                let mut first = above;
                while bytes.get(first).is_some_and(|&b| b == b' ' || b == b'\t') {
                    first += 1;
                }
                if lay.comment_at(first).is_none() || lay.line_rest(above) != Some(from) {
                    break;
                }
                from = above;
            }
            cuts.push((from, rest));
            (cuts, (from, rest))
        }
        _ => {
            let mut to = end;
            if comma.is_some() {
                while bytes.get(to).is_some_and(|&b| b == b' ' || b == b'\t') {
                    to += 1;
                }
            }
            cuts.push((unit.offset, to));
            (cuts, (unit.offset, to))
        }
    }
}

/// The text with an entry put after a container's last member (mcp.ts:597); `entry` is handed the indent to lay a
/// multi-line value under, or nothing where it goes on one line.
pub fn inserted(text: &str, container: &Node, entry: &dyn Fn(Option<&str>) -> String, comments: &[Comment]) -> String {
    let lay = Layout { text, comments };
    let eol = line_end(text);
    let bytes = text.as_bytes();
    let at = |p: usize, put: &str| format!("{}{}{}", &text[..p], put, &text[p..]);
    match container.children.last() {
        None => {
            let open = container.offset + 1;
            match lay.line_rest(open) {
                Some(rest) if bytes[rest - 1] == b'\n' => {
                    let inner = format!("{}  ", lay.indent(container.offset));
                    at(rest, &format!("{inner}{}{eol}", entry(Some(&inner))))
                }
                _ => at(open, &entry(None)),
            }
        }
        Some(last) => {
            let end = last.offset + last.length;
            let after = lay.past(end);
            let trailing = bytes.get(after) == Some(&b',');
            match lay.line_rest(if trailing { after + 1 } else { end }) {
                Some(rest) if bytes[rest - 1] == b'\n' => {
                    let inner = lay.indent(last.offset);
                    let comma = if trailing { "," } else { "" };
                    let line = format!("{inner}{}{comma}{eol}", entry(Some(&inner)));
                    if trailing {
                        at(rest, &line)
                    } else {
                        format!("{},{}{}{}", &text[..end], &text[end..rest], line, &text[rest..])
                    }
                }
                _ if trailing => at(after + 1, &format!(" {},", entry(None))),
                _ => at(end, &format!(", {}", entry(None))),
            }
        }
    }
}

/// Refuses with `LOST_COMMENT` where a comment outside `own` no longer stands, in order, in `out` (mcp.ts:662).
pub fn keeps_comments(text: &str, comments: &[Comment], out: &str, own: (usize, usize)) -> Result<(), HostError> {
    let kept: Vec<&str> = comments.iter().filter(|c| c.at < own.0 || c.end > own.1).map(|c| &text[c.at..c.end]).collect();
    let found = comments_of(out);
    let left: Vec<&str> = found.iter().map(|c| &out[c.at..c.end]).collect();
    if kept == left {
        Ok(())
    } else {
        Err(HostError::new(LOST_COMMENT))
    }
}

fn not_in_place() -> HostError {
    HostError::new(NOT_IN_PLACE)
}

/// The line ending the text lays its lines out with.
fn line_end(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// The positions in the text a line is read by, as jsonc-parser's comments and the editor's layout read them.
struct Layout<'a> {
    text: &'a str,
    comments: &'a [Comment],
}

impl Layout<'_> {
    fn comment_at(&self, p: usize) -> Option<Comment> {
        self.comments.binary_search_by_key(&p, |c| c.at).ok().map(|i| self.comments[i])
    }

    /// The offset just past the last newline before `p`, or 0.
    fn line_start(&self, p: usize) -> usize {
        let bytes = self.text.as_bytes();
        bytes[..p.min(bytes.len())].iter().rposition(|&b| b == b'\n').map_or(0, |i| i + 1)
    }

    /// The first place at or after `p` that is neither white space nor a comment.
    fn past(&self, mut p: usize) -> usize {
        loop {
            while let Some(c) = self.text.get(p..).and_then(|s| s.chars().next()) {
                if !is_js_space(c) {
                    break;
                }
                p += c.len_utf8();
            }
            match self.comment_at(p) {
                None => return p,
                Some(c) => p = c.end,
            }
        }
    }

    /// Past the spaces, tabs and one-line comments that end the line from `p`, or nothing where anything else stands
    /// on it.
    fn line_rest(&self, mut p: usize) -> Option<usize> {
        let bytes = self.text.as_bytes();
        loop {
            while bytes.get(p).is_some_and(|&b| b == b' ' || b == b'\t' || b == b'\r') {
                p += 1;
            }
            match bytes.get(p) {
                None => return Some(p),
                Some(b'\n') => return Some(p + 1),
                Some(_) => {
                    let c = self.comment_at(p)?;
                    if self.text[c.at..c.end].contains('\n') {
                        return None;
                    }
                    p = c.end;
                }
            }
        }
    }

    /// The spaces and tabs that start the line of `p`.
    fn indent(&self, p: usize) -> String {
        let from = self.line_start(p);
        let bytes = self.text.as_bytes();
        let mut to = from;
        while bytes.get(to).is_some_and(|&b| b == b' ' || b == b'\t') {
            to += 1;
        }
        self.text[from..to].to_string()
    }
}

/// The text the edits are made to, one at a time or as a run of new keys, each checked against `root` as it stands.
fn apply_edits(text: &str, root: &mut Value, edits: &[JsonEdit]) -> Result<String, HostError> {
    let mut out = text.to_string();
    let mut i = 0;
    while i < edits.len() {
        let run = new_keys(root, edits, i);
        if run > 1 {
            let parent = &edits[i].0[..edits[i].0.len() - 1];
            let members: Vec<(String, Value)> =
                edits[i..i + run].iter().filter_map(|(path, value)| Some((seg_text(path.last()?), value.clone()?))).collect();
            out = insert_many(&out, parent, &members)?;
            for edit in &edits[i..i + run] {
                set_at(root, edit);
            }
            i += run;
        } else {
            out = edit_once(&out, &edits[i])?;
            set_at(root, &edits[i]);
            i += 1;
        }
    }
    if json_canonical(&read_jsonc(&out)?) != json_canonical(root) {
        return Err(not_in_place());
    }
    Ok(out)
}

/// Several keys an object does not hold put into it with one parse of the text, in the order given; an object the
/// text does not hold yet is made whole as editOnce makes one.
fn insert_many(text: &str, at: &[PathSeg], members: &[(String, Value)]) -> Result<String, HostError> {
    let doc = parse_tree(text).ok_or_else(|| HostError::new("the file is not JSON"))?;
    let Some(node) = find_node_at_location(&doc, at) else {
        let object: Map<String, Value> = members.iter().cloned().collect();
        return edit_once(text, &(at.to_vec(), Some(Value::Object(object))));
    };
    let comments = comments_of(text);
    let eol = line_end(text);
    let entry = |indent: Option<&str>| {
        let separator = match indent {
            None => ", ".to_string(),
            Some(ind) => format!(",{eol}{ind}"),
        };
        members
            .iter()
            .map(|(key, value)| format!("{}: {}", js_string(key), rendered(value, indent, eol)))
            .collect::<Vec<_>>()
            .join(&separator)
    };
    let out = inserted(text, node, &entry, &comments);
    keeps_comments(text, &comments, &out, (0, 0))?;
    Ok(out)
}

/// How many edits from `from` on each put a new key into one same object that holds none of them, so `insert_many`
/// makes them in one parse; 0 or 1 where the first edit is not one of them.
fn new_keys(root: &Value, edits: &[JsonEdit], from: usize) -> usize {
    let Some((first, _)) = edits.get(from) else {
        return 0;
    };
    let Some((_, parent)) = first.split_last() else {
        return 0;
    };
    let holder = seg_path(root, parent);
    let mut keys: Vec<&str> = Vec::new();
    let mut count = 0;
    for (path, value) in &edits[from..] {
        let Some((PathSeg::Key(key), head)) = path.split_last() else {
            break;
        };
        if value.is_none() || head != parent {
            break;
        }
        let held_here = match holder {
            None => false,
            Some(h) => match h.as_object() {
                None => true,
                Some(object) => object.contains_key(key),
            },
        };
        if held_here || keys.contains(&key.as_str()) {
            break;
        }
        keys.push(key);
        count += 1;
    }
    count
}

/// The value at a path of an object, each step an own key of the object before it (mcp.ts:700's `reduce`).
fn seg_path<'a>(root: &'a Value, segs: &[PathSeg]) -> Option<&'a Value> {
    segs.iter().try_fold(root, |v, seg| v.as_object()?.get(&seg_text(seg)))
}

/// A path step as the key an object holds it under.
fn seg_text(seg: &PathSeg) -> String {
    match seg {
        PathSeg::Key(key) => key.clone(),
        PathSeg::Index(i) => i.to_string(),
    }
}

/// The value at `segs`, each step a key of an object or an index into an array, wrapped in the containers the path
/// names: an index as an array of one, a key as an object of one (mcp.ts:669's `wrap`).
fn wrap(segs: &[PathSeg], value: &Value) -> Value {
    segs.iter().rev().fold(value.clone(), |v, seg| match seg {
        PathSeg::Index(_) => Value::Array(vec![v]),
        PathSeg::Key(key) => {
            let mut object = Map::new();
            object.insert(key.clone(), v);
            Value::Object(object)
        }
    })
}

/// A value as it is put into the text: on lines of its own under `indent` in a file laid out on lines, else on one.
fn rendered(value: &Value, indent: Option<&str>, eol: &str) -> String {
    match indent {
        None => js_stringify(value),
        Some(ind) => js_stringify_pretty(value).replace('\n', &format!("{eol}{ind}")),
    }
}

fn set_in(at: &mut Value, via: &[PathSeg], last: &PathSeg, value: Option<&Value>) {
    let Some((seg, rest)) = via.split_first() else {
        put_last(at, last, value);
        return;
    };
    let key = seg_text(seg);
    if let (true, PathSeg::Index(n)) = (rest.is_empty(), last) {
        if let Some(Value::Array(items)) = child_mut(at, &key) {
            splice_or_set(items, *n, value);
            return;
        }
    }
    if !matches!(child_mut(at, &key), Some(Value::Object(_) | Value::Array(_))) {
        put_own(at, &key, Value::Object(Map::new()));
    }
    if let Some(child) = child_mut(at, &key) {
        set_in(child, rest, last, value);
    }
}

fn put_last(at: &mut Value, last: &PathSeg, value: Option<&Value>) {
    match at {
        Value::Array(items) => {
            if let PathSeg::Index(n) = last {
                splice_or_set(items, *n, value);
            }
        }
        Value::Object(map) => {
            let key = seg_text(last);
            match value {
                Some(v) => {
                    map.insert(key, v.clone());
                }
                None => {
                    map.shift_remove(&key);
                }
            }
        }
        _ => {}
    }
}

fn child_mut<'a>(at: &'a mut Value, key: &str) -> Option<&'a mut Value> {
    match at {
        Value::Object(map) => map.get_mut(key),
        Value::Array(items) => items.get_mut(key.parse::<usize>().ok()?),
        _ => None,
    }
}

fn put_own(at: &mut Value, key: &str, value: Value) {
    match at {
        Value::Object(map) => {
            map.insert(key.to_string(), value);
        }
        Value::Array(items) => {
            if let Ok(i) = key.parse::<usize>() {
                if i >= items.len() {
                    items.resize(i, Value::Null);
                    items.push(value);
                } else {
                    items[i] = value;
                }
            }
        }
        _ => {}
    }
}

/// `splice(last, 1)` where the value is taken out, `push` for -1, `held[last] = value` otherwise (mcp.ts:490).
fn splice_or_set(items: &mut Vec<Value>, n: i64, value: Option<&Value>) {
    match value {
        None => {
            let len = items.len() as i64;
            let start = if n < 0 { (len + n).max(0) } else { n };
            if start < len {
                items.remove(start as usize);
            }
        }
        Some(v) if n == -1 => items.push(v.clone()),
        Some(v) if n >= 0 => {
            let i = n as usize;
            if i >= items.len() {
                items.resize(i, Value::Null);
                items.push(v.clone());
            } else {
                items[i] = v.clone();
            }
        }
        Some(_) => {}
    }
}
