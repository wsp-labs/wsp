// SPDX-License-Identifier: AGPL-3.0-only
//! The three reads of the `jsonc-parser` package the in-place editor makes: `parseTree` with trailing commas allowed,
//! `findNodeAtLocation`, and `visit`'s comments. Only text `read_jsonc` accepted reaches it. Offsets are byte offsets
//! into the text, used only to slice that same text. The spec is spec/jsonc.md.
use crate::jsonc::string_end;

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
pub struct Node {
    pub kind: NodeType,
    pub offset: usize,
    pub length: usize,
    pub children: Vec<Node>,
    pub text: Option<String>,
}

impl Drop for Node {
    /// A tree dropped field by field recurses once per level.
    fn drop(&mut self) {
        let mut work = std::mem::take(&mut self.children);
        while let Some(mut node) = work.pop() {
            work.append(&mut node.children);
        }
    }
}

/// One comment, `//` to the end of its line (the newline not included) or `/* */` whole.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Comment {
    pub at: usize,
    pub end: usize,
}

/// The tree of the text, or nothing for text that holds no value.
pub fn parse_tree(text: &str) -> Option<Node> {
    let mut parser = Parser { text, bytes: text.as_bytes(), pos: 0 };
    parser.tree()
}

/// The node at the path: a key finds the first property of that name in an object, an index the element of an array;
/// a key on an array, an index on an object, or a step past a scalar finds nothing. The empty path is the root.
pub fn find_node_at_location<'a>(root: &'a Node, path: &[PathSeg]) -> Option<&'a Node> {
    let mut node = root;
    for seg in path {
        node = match seg {
            PathSeg::Key(key) => {
                if node.kind != NodeType::Object {
                    return None;
                }
                let property = node.children.iter().find(|p| is_member_named(p, key))?;
                &property.children[1]
            }
            PathSeg::Index(i) => {
                if node.kind != NodeType::Array || *i < 0 {
                    return None;
                }
                node.children.get(usize::try_from(*i).ok()?)?
            }
        };
    }
    Some(node)
}

/// The member a path names, as its container (an object or an array) and its index among the container's children:
/// for an object member the child is the property node. Nothing where the path names no member.
pub fn find_member<'a>(root: &'a Node, path: &[PathSeg]) -> Option<(&'a Node, usize)> {
    let (last, rest) = path.split_last()?;
    let container = find_node_at_location(root, rest)?;
    match last {
        PathSeg::Key(key) => {
            if container.kind != NodeType::Object {
                return None;
            }
            let index = container.children.iter().position(|p| is_member_named(p, key))?;
            Some((container, index))
        }
        PathSeg::Index(i) => {
            if container.kind != NodeType::Array || *i < 0 {
                return None;
            }
            let index = usize::try_from(*i).ok()?;
            (index < container.children.len()).then_some((container, index))
        }
    }
}

/// Every comment in the text, in order.
pub fn comments_of(text: &str) -> Vec<Comment> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => i = string_end(bytes, i).unwrap_or(bytes.len()),
            b'/' => match comment_end(bytes, i) {
                Some(end) => {
                    out.push(Comment { at: i, end });
                    i = end;
                }
                None => i += 1,
            },
            _ => i += 1,
        }
    }
    out
}

fn is_member_named(property: &Node, key: &str) -> bool {
    property.kind == NodeType::Property && property.children.len() == 2 && property.children[0].text.as_deref() == Some(key)
}

/// Where the comment starting at `at` ends (jsonc-parser 3.3: a line comment stops before `\r` or `\n`).
fn comment_end(bytes: &[u8], at: usize) -> Option<usize> {
    match (bytes.get(at), bytes.get(at + 1)) {
        (Some(b'/'), Some(b'/')) => Some(bytes[at..].iter().position(|&b| b == b'\n' || b == b'\r').map_or(bytes.len(), |k| at + k)),
        (Some(b'/'), Some(b'*')) => Some(find_bytes(bytes, at + 2, b"*/").map_or(bytes.len(), |k| k + 2)),
        _ => None,
    }
}

fn find_bytes(bytes: &[u8], from: usize, needle: &[u8]) -> Option<usize> {
    bytes.get(from..)?.windows(needle.len()).position(|w| w == needle).map(|k| from + k)
}

/// Containers nested past this are not read: the cap of the JSON.parse port, so a tree kept this deep clones and drops.
const MAX_DEPTH: usize = 500;

enum Frame {
    Object(usize, Vec<Node>),
    Array(usize, Vec<Node>),
    Property(Node),
}

struct Parser<'a> {
    text: &'a str,
    bytes: &'a [u8],
    pos: usize,
}

impl Parser<'_> {
    fn skip_trivia(&mut self) {
        loop {
            while self.pos < self.bytes.len() && matches!(self.bytes[self.pos], b' ' | b'\t' | b'\r' | b'\n') {
                self.pos += 1;
            }
            match comment_end(self.bytes, self.pos) {
                Some(end) if self.bytes[self.pos] == b'/' => self.pos = end,
                _ => return,
            }
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// The value at `pos`, read with a stack of its own: a container is a frame, not a call. Containers nested past
    /// `MAX_DEPTH` are not read.
    fn tree(&mut self) -> Option<Node> {
        let mut stack: Vec<Frame> = Vec::new();
        let mut open = 0;
        loop {
            self.skip_trivia();
            let offset = self.pos;
            let mut done = match self.peek()? {
                open_byte @ (b'{' | b'[') => {
                    if open >= MAX_DEPTH {
                        return None;
                    }
                    open += 1;
                    self.pos += 1;
                    let (close, frame) = match open_byte {
                        b'{' => (b'}', Frame::Object(offset, Vec::new())),
                        _ => (b']', Frame::Array(offset, Vec::new())),
                    };
                    self.lone_comma(close)?;
                    stack.push(frame);
                    match self.head(&mut stack, &mut open)? {
                        Some(node) => node,
                        None => continue,
                    }
                }
                _ => self.leaf_value()?,
            };
            loop {
                match stack.pop() {
                    None => return Some(done),
                    Some(Frame::Property(key)) => {
                        let offset = key.offset;
                        let length = done.offset + done.length - offset;
                        let property = container(NodeType::Property, offset, length, vec![key, done]);
                        let Some(Frame::Object(_, children)) = stack.last_mut() else {
                            return None;
                        };
                        children.push(property);
                        self.skip_trivia();
                        match self.peek()? {
                            b',' => self.pos += 1,
                            b'}' => {
                                self.pos += 1;
                                done = self.close(&mut stack, &mut open);
                                continue;
                            }
                            _ => return None,
                        }
                    }
                    Some(Frame::Array(at, mut children)) => {
                        children.push(done);
                        stack.push(Frame::Array(at, children));
                        self.skip_trivia();
                        match self.peek()? {
                            b',' => self.pos += 1,
                            b']' => {
                                self.pos += 1;
                                done = self.close(&mut stack, &mut open);
                                continue;
                            }
                            _ => return None,
                        }
                    }
                    Some(Frame::Object(..)) => return None,
                }
                match self.head(&mut stack, &mut open)? {
                    Some(node) => done = node,
                    None => break,
                }
            }
        }
    }

    fn leaf_value(&mut self) -> Option<Node> {
        self.skip_trivia();
        let offset = self.pos;
        match self.peek()? {
            b'"' => {
                let end = string_end(self.bytes, offset)?;
                let text = crate::json_parse::parse_json_string(&self.text[offset..end])?;
                self.pos = end;
                Some(leaf(NodeType::String, offset, end - offset, Some(text)))
            }
            b't' | b'f' | b'n' => {
                let (word, kind) = match self.bytes[offset] {
                    b't' => ("true", NodeType::Boolean),
                    b'f' => ("false", NodeType::Boolean),
                    _ => ("null", NodeType::Null),
                };
                if !self.text[offset..].starts_with(word) {
                    return None;
                }
                self.pos = offset + word.len();
                Some(leaf(kind, offset, word.len(), None))
            }
            b'-' | b'0'..=b'9' => {
                let end = offset
                    + self.bytes[offset..].iter().take_while(|b| matches!(b, b'-' | b'+' | b'.' | b'e' | b'E' | b'0'..=b'9')).count();
                self.pos = end;
                Some(leaf(NodeType::Number, offset, end - offset, None))
            }
            _ => None,
        }
    }

    /// What the open container on top of `stack` holds next: its close, as the finished node, or the start of its next
    /// member (an object's property name is read and pushed), as nothing.
    fn head(&mut self, stack: &mut Vec<Frame>, open: &mut usize) -> Option<Option<Node>> {
        self.skip_trivia();
        let (close, object) = match stack.last()? {
            Frame::Object(..) => (b'}', true),
            Frame::Array(..) => (b']', false),
            Frame::Property(_) => return None,
        };
        if self.peek()? == close {
            self.pos += 1;
            return Some(Some(self.close(stack, open)));
        }
        if object {
            self.skip_trivia();
            if self.peek()? != b'"' {
                return None;
            }
            let key = self.leaf_value()?;
            self.skip_trivia();
            if self.peek()? != b':' {
                return None;
            }
            self.pos += 1;
            stack.push(Frame::Property(key));
        }
        Some(None)
    }

    /// The container on top of `stack`, whose close bracket `pos` has just passed.
    fn close(&mut self, stack: &mut Vec<Frame>, open: &mut usize) -> Node {
        *open -= 1;
        match stack.pop() {
            Some(Frame::Object(offset, children)) => container(NodeType::Object, offset, self.pos - offset, children),
            Some(Frame::Array(offset, children)) => container(NodeType::Array, offset, self.pos - offset, children),
            _ => unreachable!("a container is open"),
        }
    }

    /// jsonc-parser recovers a container holding a comma and nothing else (`{,}`, which `read_jsonc` takes as `{}`): the
    /// comma is passed over and the container is empty.
    fn lone_comma(&mut self, close: u8) -> Option<()> {
        self.skip_trivia();
        if self.peek()? == b',' {
            let after = self.pos + 1;
            self.pos = after;
            self.skip_trivia();
            if self.peek()? != close {
                self.pos = after - 1;
            }
        }
        Some(())
    }
}

fn leaf(kind: NodeType, offset: usize, length: usize, text: Option<String>) -> Node {
    Node { kind, offset, length, children: Vec::new(), text }
}

fn container(kind: NodeType, offset: usize, length: usize, children: Vec<Node>) -> Node {
    Node { kind, offset, length, children, text: None }
}
