// SPDX-License-Identifier: AGPL-3.0-only
//! The three reads of the `jsonc-parser` package the in-place editor makes: `parseTree` with trailing commas allowed,
//! `findNodeAtLocation`, and `visit`'s comments. Only text `read_jsonc` accepted reaches it. Offsets are byte offsets
//! into the text, used only to slice that same text. The spec is spec/jsonc.md.

/// One step of a path into a JSON value: a key, or an index where -1 means after the last element.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PathSeg {
    Key(String),
    Index(i64),
}

pub type JsonPath = Vec<PathSeg>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeType {
    Object,
    Array,
    /// A member of an object: `children[0]` is the key's string node, `children[1]` its value.
    Property,
    String,
    Number,
    Boolean,
    Null,
}

/// A node of the tree: where it stands in the text and what it holds. A string node's `text` is its decoded value.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: NodeType,
    pub offset: usize,
    pub length: usize,
    pub children: Vec<Node>,
    pub text: Option<String>,
}

/// One comment, `//` to the end of its line (the newline not included) or `/* */` whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comment {
    pub at: usize,
    pub end: usize,
}

/// The tree of the text, or nothing for text that holds no value.
pub fn parse_tree(text: &str) -> Option<Node> {
    todo!()
}

/// The node at the path: a key finds the first property of that name in an object, an index the element of an array;
/// a key on an array, an index on an object, or a step past a scalar finds nothing. The empty path is the root.
pub fn find_node_at_location<'a>(root: &'a Node, path: &[PathSeg]) -> Option<&'a Node> {
    todo!()
}

/// The member a path names, as its container (an object or an array) and its index among the container's children:
/// for an object member the child is the property node. Nothing where the path names no member.
pub fn find_member<'a>(root: &'a Node, path: &[PathSeg]) -> Option<(&'a Node, usize)> {
    todo!()
}

/// Every comment in the text, in order.
pub fn comments_of(text: &str) -> Vec<Comment> {
    todo!()
}
