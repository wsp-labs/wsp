// SPDX-License-Identifier: AGPL-3.0-only
//! The one in-place editor of an agent's JSON or jsonc config: each edit made on the text so every comment and byte it
//! does not touch stays, and refused where the result would read as anything but the edits made to the parsed value.
//! Port of packages/catalog/src/mcp.ts:458-768 at a3b870a6e; the spec is spec/json_edit.md.
use serde_json::Value;

use crate::jsonc_tree::{Comment, JsonPath, Node};
use wsp_words::exit::HostError;

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
    todo!()
}

/// The same over text whose parsed value the caller holds: `root` takes each edit as it is made, so on return it is
/// the parsed value of the text returned (mcp.ts:702). Runs of new keys into one object go in with one parse
/// (`newKeys`, mcp.ts:685).
pub fn edit_jsonc(text: &str, root: &mut Value, edits: &[JsonEdit]) -> Result<String, HostError> {
    todo!()
}

/// The edit made to a parsed value, a missing object on the way made as the in-place edit makes one (mcp.ts:490): a
/// number last on an array holder sets, appends (-1) or removes that element; a key is set or removed as an own key.
pub fn set_at(root: &mut Value, edit: &JsonEdit) {
    todo!()
}

/// A value with every object's keys sorted (UTF-16 order, rule 8.8) at every depth, written compactly as
/// `JSON.stringify` writes (mcp.ts:764): what makes two entries the same entry.
pub fn json_canonical(value: &Value) -> String {
    todo!()
}

/// One edit made to the text (mcp.ts:625): a member taken out by `member_cuts`, a value put in where it stood, or put
/// into the deepest object or array of the path that is there; then `keeps_comments`.
pub fn edit_once(text: &str, edit: &JsonEdit) -> Result<String, HostError> {
    todo!()
}

/// What taking out child `i` of a container cuts, and the span whose comments go with it (mcp.ts:560).
pub fn member_cuts(text: &str, kids: &[Node], i: usize, comments: &[Comment]) -> (Vec<(usize, usize)>, (usize, usize)) {
    todo!()
}

/// The text with an entry put after a container's last member (mcp.ts:597); `entry` is handed the indent to lay a
/// multi-line value under, or nothing where it goes on one line.
pub fn inserted(text: &str, container: &Node, entry: &dyn Fn(Option<&str>) -> String, comments: &[Comment]) -> String {
    todo!()
}

/// Refuses with `LOST_COMMENT` where a comment outside `own` no longer stands, in order, in `out` (mcp.ts:662).
pub fn keeps_comments(text: &str, comments: &[Comment], out: &str, own: (usize, usize)) -> Result<(), HostError> {
    todo!()
}
