// SPDX-License-Identifier: AGPL-3.0-only
//! The reader: the tokens of the JSX-like form, its elements and attributes.
use super::*;

fn tag_len(s: &str) -> usize {
    let b = s.as_bytes();
    if !b.first().is_some_and(u8::is_ascii_alphabetic) {
        return 0;
    }
    b.iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'-').count()
}

fn attr_len(s: &str) -> usize {
    let b = s.as_bytes();
    if !b.first().is_some_and(u8::is_ascii_alphabetic) {
        return 0;
    }
    b.iter().take_while(|c| c.is_ascii_alphanumeric() || **c == b'_' || **c == b'-').count()
}

fn closing_tag(s: &str) -> Option<(&str, usize)> {
    let body = s.strip_prefix("</")?;
    let n = tag_len(body);
    if n == 0 {
        return None;
    }
    let after = body[n..].trim_start_matches(is_js_whitespace).strip_prefix('>')?;
    Some((&body[..n], s.len() - after.len()))
}

fn byte_at_utf16(src: &str, target: usize) -> usize {
    let mut units = 0;
    for (at, c) in src.char_indices() {
        if units >= target {
            return at;
        }
        units += c.len_utf16();
    }
    src.len()
}

pub(super) struct Reader<'a> {
    src: &'a str,
    i: usize,
    newlines: Vec<usize>,
}

impl<'a> Reader<'a> {
    pub(super) fn new(src: &'a str) -> Self {
        Reader { src, i: 0, newlines: src.match_indices('\n').map(|(at, _)| at).collect() }
    }

    pub(super) fn line(&self, at: usize) -> i64 {
        (self.newlines.partition_point(|&p| p < at) + 1) as i64
    }

    fn here(&self) -> i64 {
        self.line(self.i)
    }

    pub(super) fn rest(&self) -> &'a str {
        &self.src[self.i..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn starts(&self, s: &str) -> bool {
        self.rest().starts_with(s)
    }

    fn fail<T>(&self, message: String, fix: Option<String>) -> Result<T, Fatal> {
        Err(Fatal { code: "P100", message, line: self.here(), fix })
    }

    pub(super) fn ws(&mut self) {
        while let Some(c) = self.peek() {
            if !is_js_whitespace(c) {
                break;
            }
            self.i += c.len_utf8();
        }
    }

    pub(super) fn elements(&mut self) -> Result<Vec<El>, Fatal> {
        let most = SLATE_LIMITS.document_bytes + SLATE_LIMITS.files_bytes;
        if utf16::len(self.src) > most || slate_bytes(self.src) > most {
            return Err(fatal(
                "D208",
                format!("the text is over {} KB, the most a slate and its files hold; write less, or patch it in parts", most / 1024),
                1,
            ));
        }
        let mut out = Vec::new();
        loop {
            self.ws();
            self.comments()?;
            self.ws();
            if self.i >= self.src.len() {
                break;
            }
            if self.peek() != Some('<') && out.is_empty() && matches!(self.peek(), Some('{') | Some('[')) {
                return self.fail(
                    "this is JSON, and a slate is JSX-like text that starts with <slate>; slate_catalog shows it".into(),
                    Some(r#"<slate title="Gold"><number label="Spot" value={$spot.json.usd} unit="USD" /></slate>"#.into()),
                );
            }
            if self.peek() != Some('<') {
                return self.fail(format!("expected an element at line {}, found text", self.here()), None);
            }
            out.push(self.element(1)?);
        }
        Ok(out)
    }

    fn comments(&mut self) -> Result<(), Fatal> {
        loop {
            if self.starts("<!--") {
                let Some(end) = self.rest().find("-->") else {
                    return self.fail("a comment never closes".into(), None);
                };
                self.i += end + 3;
            } else if self.starts("{/*") {
                let Some(end) = self.rest().find("*/}") else {
                    return self.fail("a comment never closes".into(), None);
                };
                self.i += end + 3;
            } else {
                break;
            }
            self.ws();
        }
        Ok(())
    }

    pub(super) fn element(&mut self, depth: usize) -> Result<El, Fatal> {
        let line = self.here();
        if depth > READ_DEPTH {
            return Err(fatal(
                "D206",
                format!("elements nest more than {} deep at line {line}; flatten the layout", SLATE_LIMITS.depth),
                line,
            ));
        }
        self.i += 1;
        let len = tag_len(self.rest());
        if len == 0 {
            return self.fail(format!("cannot read a tag name at line {line}"), None);
        }
        let name = self.rest()[..len].to_string();
        self.i += len;
        let mut attrs: Vec<Attr> = Vec::new();
        loop {
            self.ws();
            if self.starts("/>") {
                self.i += 2;
                return Ok(El { tag: name, attrs, children: Vec::new(), text: None, raw: None, line });
            }
            if self.peek() == Some('>') {
                self.i += 1;
                break;
            }
            if self.i >= self.src.len() {
                return self.fail(format!("<{name}> at line {line} is not closed; write <{name} ... /> or </{name}>"), None);
            }
            let alen = attr_len(self.rest());
            if alen == 0 {
                return self.fail(
                    format!(
                        "cannot read an attribute of <{name}> at line {}; write <{name} prop=\"literal\" prop={{formula}} />",
                        self.here()
                    ),
                    None,
                );
            }
            let aname = self.rest()[..alen].to_string();
            let aline = self.here();
            self.i += alen;
            self.ws();
            if self.peek() != Some('=') {
                attrs.push(Attr { name: aname, kind: Kind::Bare, value: String::new(), line: aline });
                continue;
            }
            self.i += 1;
            self.ws();
            match self.peek() {
                Some(q @ ('"' | '\'')) => {
                    let body = &self.src[self.i + 1..];
                    let Some(end_rel) = body.find(q) else {
                        return self.fail(format!("the string for {aname} at line {aline} never closes"), None);
                    };
                    let raw = &body[..end_rel];
                    if raw.ends_with('\\') {
                        let fix = if q == '"' {
                            format!(
                                r#"{aname}='echo "hi"', single quotes outside the double ones; with both quotes inside, the block form <run name="x">{{`...`}}</run>"#
                            )
                        } else {
                            format!(
                                r#"{aname}="echo 'hi'", double quotes outside the single ones; with both quotes inside, the block form <run name="x">{{`...`}}</run>"#
                            )
                        };
                        return self.fail(format!("attribute strings take no escapes ({aname} at line {aline}): a backslash does not hide a {q} inside {q}...{q}"), Some(fix));
                    }
                    if let Some(entity) = HTML_ENTITY.find(raw) {
                        let quote = ENTITY_QUOTE.is_match(entity.as_str());
                        let fix = if quote {
                            format!("{aname}='python3 \"$SLATE_DIR/x.py\"', single quotes outside the double ones")
                        } else {
                            format!("write the character itself: {aname}=\"... & ...\"")
                        };
                        return self.fail(
                            format!("attribute strings take no HTML entities ({aname} at line {aline}): {} stays as written, nothing decodes it", entity.as_str()),
                            Some(fix),
                        );
                    }
                    let value = raw.to_string();
                    self.i += end_rel + 2;
                    attrs.push(Attr { name: aname, kind: Kind::Str, value, line: aline });
                    continue;
                }
                Some('{') => {
                    let end = self.balanced(self.i)?;
                    let value = self.src[self.i + 1..end].to_string();
                    attrs.push(Attr { name: aname, kind: Kind::Braced, value, line: aline });
                    self.i = end + 1;
                    continue;
                }
                Some('`') => {
                    return self.fail(format!("a template for {aname} goes inside braces: {aname}={{`...`}}"), None);
                }
                _ => {
                    return self.fail(format!("cannot read the value of {aname} at line {aline}"), None);
                }
            }
        }
        let raw_piece = slate_piece_module(&name).is_some_and(|m| m.raw_text);
        if raw_piece || is_file_decl(&name, &attrs) {
            let close = format!("</{name}>");
            let Some(end) = self.rest().find(&close).map(|p| self.i + p) else {
                return Err(fatal("P100", format!("<{name}> at line {line} is not closed; write </{name}>"), line));
            };
            let raw = dedent(&self.src[self.i..end]);
            self.i = end + close.len();
            return Ok(El { tag: name, attrs, children: Vec::new(), text: None, raw: (!raw.is_empty()).then_some(raw), line });
        }
        let mut children = Vec::new();
        let mut text: Vec<Part> = Vec::new();
        loop {
            if self.i >= self.src.len() {
                return self.fail(format!("<{name}> at line {line} is not closed; write <{name} ... /> or </{name}>"), None);
            }
            if self.starts("</") {
                let Some((closing, consumed)) = closing_tag(self.rest()) else {
                    return self.fail(format!("cannot read the closing tag at line {}", self.here()), None);
                };
                if closing != name {
                    return self.fail(
                        format!("</{closing}> at line {} closes <{name}> opened at line {line}; expected </{name}>", self.here()),
                        Some(format!("</{name}>")),
                    );
                }
                self.i += consumed;
                break;
            }
            if self.starts("<!--") || self.starts("{/*") {
                self.comments()?;
                continue;
            }
            if self.peek() == Some('<') {
                children.push(self.element(depth + 1)?);
                continue;
            }
            if self.peek() == Some('{') {
                let end = self.balanced(self.i)?;
                text.push(Part::Hole(self.src[self.i + 1..end].to_string()));
                self.i = end + 1;
                continue;
            }
            let rest = self.rest();
            let j = rest.find(['<', '{']).unwrap_or(rest.len());
            text.push(Part::Text(rest[..j].to_string()));
            self.i += j;
        }
        let meaningful = text.iter().any(|t| match t {
            Part::Hole(_) => true,
            Part::Text(s) => !js_trim(s).is_empty(),
        });
        Ok(El { tag: name, attrs, children, text: meaningful.then_some(text), raw: None, line })
    }

    fn balanced(&self, i: usize) -> Result<usize, Fatal> {
        let start = utf16::len(&self.src[..i + 1]);
        match closing_brace(self.src, start) {
            Some(end) => Ok(byte_at_utf16(self.src, end)),
            None => Err(fatal("P100", format!("a {{ at line {} never closes", self.line(i)), self.line(i))),
        }
    }
}
