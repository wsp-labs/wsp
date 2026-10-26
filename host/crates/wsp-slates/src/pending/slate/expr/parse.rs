// SPDX-License-Identifier: AGPL-3.0-only
//! The parser and the format parser of expr.ts.
use super::*;

// ---- the parser ----

struct Parser {
    toks: Vec<Token>,
    i: usize,
    depth: usize,
    base: usize,
}

/// Each `column N` in a message moved by base, as the TS's replace does on every inner message.
fn shift_columns(message: &str, base: usize) -> String {
    let mut out = String::new();
    let mut rest = message;
    while let Some(pos) = rest.find("column ") {
        let after = &rest[pos + 7..];
        let digits = after.bytes().take_while(u8::is_ascii_digit).count();
        out.push_str(&rest[..pos + 7]);
        if digits == 0 {
            rest = after;
            continue;
        }
        let n: u64 = after[..digits].parse().unwrap_or(u64::MAX);
        out.push_str(&n.saturating_add(base as u64).to_string());
        rest = &after[digits..];
    }
    out.push_str(rest);
    out
}

fn seg_index(x: f64) -> i64 {
    x as i64
}

impl Parser {
    fn peek_n(&self, n: usize) -> &Token {
        &self.toks[(self.i + n).min(self.toks.len() - 1)]
    }

    fn peek(&self) -> &Token {
        self.peek_n(0)
    }

    fn next(&mut self) -> Token {
        let t = self.toks[self.i.min(self.toks.len() - 1)].clone();
        self.i += 1;
        t
    }

    fn is_op(&self, v: &str) -> bool {
        let t = self.peek();
        t.kind == Kind::Op && t.v == v
    }

    fn is_word(&self, v: &str) -> bool {
        let t = self.peek();
        t.kind == Kind::Name && t.v == v
    }

    fn expect(&mut self, v: &str) -> PResult<()> {
        let t = self.peek().clone();
        if t.kind != Kind::Op || t.v != v {
            let ended = if t.kind == Kind::End { ", the expression ended" } else { "" };
            return Err(fault("X400", format!("expected {v} at column {}{ended}", t.at + 1), t.at, None));
        }
        self.i += 1;
        Ok(())
    }

    fn enter(&mut self, at: usize) -> PResult<()> {
        self.depth += 1;
        if self.depth > SLATE_LIMITS.expr_depth {
            return Err(fault("X407", format!("nesting is deeper than {}", SLATE_LIMITS.expr_depth), at, None));
        }
        Ok(())
    }

    fn parse_all(&mut self) -> PResult<SlateExpr> {
        let e = self.pipeline()?;
        let t = self.peek().clone();
        if t.kind != Kind::End {
            if t.kind == Kind::Op && t.v == "=" {
                return Err(fault("X420", "compare with ==".into(), t.at, Some("==".into())));
            }
            let shown = if t.v.is_empty() { "text" } else { t.v.as_str() };
            return Err(fault("X400", format!("unexpected {shown} at column {}", t.at + 1), t.at, None));
        }
        Ok(e)
    }

    fn pipeline(&mut self) -> PResult<SlateExpr> {
        let at = self.peek().at;
        let head = self.expression()?;
        let mut steps: Vec<SlatePipeStepNode> = Vec::new();
        while self.is_op("|") {
            self.next();
            let name = self.next();
            if name.kind != Kind::Name {
                return Err(fault("Q420", format!("expected a step after | at column {}", name.at + 1), name.at, None));
            }
            let Some(spec) = pipe_step(&name.v) else {
                let names: Vec<String> = SLATE_PIPE_STEPS.iter().map(|(n, _)| n.to_string()).collect();
                let fix = match name.v.as_str() {
                    "filter" => Some("where".to_string()),
                    "sort" | "orderBy" => Some("sortBy".to_string()),
                    "limit" => Some("take".to_string()),
                    other => slate_nearest(other, &names),
                };
                let fn_note = if function_spec(&name.v).is_some() {
                    let how =
                        if name.v == "pluck" { "pick(name) or pluck(list, 'name')".to_string() } else { format!("{}(list, ...)", name.v) };
                    format!("; {} is a function, not a step: {how}", name.v)
                } else {
                    String::new()
                };
                let tail = match &fix {
                    Some(f) => format!("; did you mean {f}?"),
                    None => fn_note,
                };
                let fix_out = fix.or_else(|| (name.v == "pluck").then(|| "pick(name) or pluck(list, 'name')".to_string()));
                return Err(fault("Q422", format!("{} is not a step{tail}", name.v), name.at, fix_out));
            };
            let mut args: Vec<SlatePipeArg> = Vec::new();
            if self.is_op("(") {
                self.next();
                if !self.is_op(")") {
                    loop {
                        let mut arg_name: Option<String> = None;
                        if self.peek().kind == Kind::Name && self.peek_n(1).kind == Kind::Op && self.peek_n(1).v == ":" {
                            arg_name = Some(self.next().v);
                            self.next();
                        }
                        let expr = self.pipeline()?;
                        args.push(SlatePipeArg { name: arg_name, expr });
                        if self.is_op(",") {
                            self.next();
                            continue;
                        }
                        break;
                    }
                }
                self.expect(")")?;
            }
            if args.len() < spec.min || args.len() > spec.max {
                let takes = if spec.min == spec.max {
                    spec.min.to_string()
                } else if spec.max >= 99 {
                    format!("at least {}", spec.min)
                } else {
                    format!("{} to {}", spec.min, spec.max)
                };
                let plural = if spec.max == 1 { "" } else { "s" };
                return Err(fault(
                    "Q423",
                    format!("{} takes {takes} argument{plural}, got {}: {}", name.v, args.len(), spec.sig),
                    name.at,
                    Some(spec.sig.to_string()),
                ));
            }
            if (name.v == "take" || name.v == "skip")
                && !matches!(&args[0].expr, SlateExpr::Lit { v, .. } if v.as_f64().is_some_and(|x| x.is_finite() && x.fract() == 0.0))
            {
                return Err(fault("Q425", format!("{}'s count is a literal integer", name.v), name.at, Some(format!("{}(5)", name.v))));
            }
            if name.v == "pick" {
                for a in &args {
                    if !matches!(&a.expr, SlateExpr::Path { own: false, segs, .. } if segs.is_empty()) {
                        return Err(fault("Q423", "pick takes bare field names: pick(name, state)".into(), name.at, None));
                    }
                }
            }
            if name.v == "map" && args.iter().any(|a| a.name.is_none()) {
                return Err(fault("Q423", "map takes named arguments: map(name: item.title)".into(), name.at, None));
            }
            if name.v != "map" && args.iter().any(|a| a.name.is_some()) {
                return Err(fault("Q423", format!("{} takes no named arguments; only map does", name.v), name.at, None));
            }
            steps.push(SlatePipeStepNode { name: name.v, args, at: name.at + self.base });
        }
        if steps.len() > SLATE_LIMITS.pipeline_steps {
            return Err(fault("Q426", format!("{} steps; the most is {}", steps.len(), SLATE_LIMITS.pipeline_steps), at, None));
        }
        if steps.is_empty() {
            return Ok(head);
        }
        Ok(SlateExpr::Pipe { head: Box::new(head), steps, at: at + self.base })
    }

    fn expression(&mut self) -> PResult<SlateExpr> {
        let at = self.peek().at;
        self.enter(at)?;
        let test = self.or()?;
        let out = if self.is_op("?") {
            self.next();
            let then = self.expression()?;
            self.expect(":")?;
            let other = self.expression()?;
            SlateExpr::Cond { test: Box::new(test), then: Box::new(then), other: Box::new(other), at: at + self.base }
        } else {
            test
        };
        self.depth -= 1;
        Ok(out)
    }

    fn or(&mut self) -> PResult<SlateExpr> {
        let mut left = self.and()?;
        while self.is_word("or") || self.is_op("or") {
            let at = self.next().at;
            let right = self.and()?;
            left = SlateExpr::Bin { op: SlateBinOp::Or, left: Box::new(left), right: Box::new(right), at: at + self.base };
        }
        Ok(left)
    }

    fn and(&mut self) -> PResult<SlateExpr> {
        let mut left = self.not()?;
        while self.is_word("and") || self.is_op("and") {
            let at = self.next().at;
            let right = self.not()?;
            left = SlateExpr::Bin { op: SlateBinOp::And, left: Box::new(left), right: Box::new(right), at: at + self.base };
        }
        Ok(left)
    }

    fn not(&mut self) -> PResult<SlateExpr> {
        if self.is_word("not") || self.is_op("not") {
            let at = self.next().at;
            self.enter(at)?;
            let arg = self.not()?;
            self.depth -= 1;
            return Ok(SlateExpr::Not { arg: Box::new(arg), at: at + self.base });
        }
        self.compare()
    }

    fn compare(&mut self) -> PResult<SlateExpr> {
        let left = self.sum()?;
        let t = self.peek().clone();
        if let Some(op) = (t.kind == Kind::Op).then(|| bin_op(&t.v)).flatten().filter(|op| op.compare()) {
            self.next();
            let right = self.sum()?;
            let after = self.peek().clone();
            if after.kind == Kind::Op && bin_op(&after.v).is_some_and(SlateBinOp::compare) {
                return Err(fault(
                    "X400",
                    "comparisons do not chain; write a < b and b < c".into(),
                    after.at,
                    Some("a < b and b < c".into()),
                ));
            }
            return Ok(SlateExpr::Bin { op, left: Box::new(left), right: Box::new(right), at: t.at + self.base });
        }
        Ok(left)
    }

    fn sum(&mut self) -> PResult<SlateExpr> {
        let mut left = self.product()?;
        while self.is_op("+") || self.is_op("-") {
            let t = self.next();
            let right = self.product()?;
            left = SlateExpr::Bin {
                op: bin_op(&t.v).unwrap_or(SlateBinOp::Add),
                left: Box::new(left),
                right: Box::new(right),
                at: t.at + self.base,
            };
        }
        Ok(left)
    }

    fn product(&mut self) -> PResult<SlateExpr> {
        let mut left = self.unary()?;
        while self.is_op("*") || self.is_op("/") || self.is_op("%") {
            let t = self.next();
            let right = self.unary()?;
            left = SlateExpr::Bin {
                op: bin_op(&t.v).unwrap_or(SlateBinOp::Mul),
                left: Box::new(left),
                right: Box::new(right),
                at: t.at + self.base,
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> PResult<SlateExpr> {
        if self.is_op("-") {
            let at = self.next().at;
            self.enter(at)?;
            let arg = self.unary()?;
            self.depth -= 1;
            return Ok(SlateExpr::Neg { arg: Box::new(arg), at: at + self.base });
        }
        self.postfix()
    }

    fn postfix(&mut self) -> PResult<SlateExpr> {
        let mut node = self.primary()?;
        loop {
            if self.is_op(".") {
                self.next();
                let n = self.next();
                if n.kind != Kind::Name {
                    return Err(fault("X400", format!("expected a name after . at column {}", n.at + 1), n.at, None));
                }
                if self.is_op("(") {
                    let fix = method_fix(&n.v).map(str::to_string);
                    let message = if n.v == "test" {
                        "no regular expressions; use contains, startsWith or endsWith".to_string()
                    } else {
                        let how = fix.as_ref().map_or("use a function or a step".to_string(), |f| format!("use {f}"));
                        format!("no method calls; {how}")
                    };
                    return Err(fault("X420", message, n.at, fix));
                }
                if n.v == "length" {
                    return Err(fault("X420", "length is not a field; use len()".into(), n.at, Some("len(x)".into())));
                }
                node = match node {
                    SlateExpr::Path { head, own, mut segs, at } => {
                        segs.push(PathSeg::Key(n.v));
                        SlateExpr::Path { head, own, segs, at }
                    }
                    other => SlateExpr::Field { of: Box::new(other), name: PathSeg::Key(n.v), at: n.at + self.base },
                };
                continue;
            }
            if self.is_op("[") {
                let open = self.next();
                let neg = usize::from(self.is_op("-"));
                let n = self.peek_n(neg).clone();
                let closes = {
                    let t = self.peek_n(neg + 1);
                    t.kind == Kind::Op && t.v == "]"
                };
                if n.kind == Kind::Num && !n.v.is_empty() && n.v.bytes().all(|b| b.is_ascii_digit()) && closes {
                    self.i += neg + 2;
                    let magnitude: f64 = n.v.parse().unwrap_or(f64::INFINITY);
                    let seg = PathSeg::Index(seg_index(if neg == 1 { -magnitude } else { magnitude }));
                    node = match node {
                        SlateExpr::Path { head, own, mut segs, at } => {
                            segs.push(seg);
                            SlateExpr::Path { head, own, segs, at }
                        }
                        other => SlateExpr::Field { of: Box::new(other), name: seg, at: n.at + self.base },
                    };
                    continue;
                }
                let index = self.pipeline()?;
                self.expect("]")?;
                node = SlateExpr::Index { of: Box::new(node), index: Box::new(index), at: open.at + self.base };
                continue;
            }
            break;
        }
        Ok(node)
    }

    fn primary(&mut self) -> PResult<SlateExpr> {
        let t = self.next();
        let at = t.at + self.base;
        match t.kind {
            Kind::Num => {
                let v: f64 = t.v.parse().unwrap_or(f64::NAN);
                if !v.is_finite() {
                    return Err(fault("X400", format!("{} is past the largest number, at column {}", t.v, t.at + 1), t.at, None));
                }
                Ok(SlateExpr::Lit { v: jnum(v), at })
            }
            Kind::Str => Ok(SlateExpr::Lit { v: Value::String(t.v), at }),
            Kind::Tpl => {
                let mut parts: Vec<SlateTemplatePart> = Vec::new();
                for p in &t.parts {
                    match p {
                        RawPart::Text(s) => parts.push(SlateTemplatePart::Text(s.clone())),
                        RawPart::Hole { src, at: pos } => {
                            let inner = parse_at(&units(src), self.base + pos)?;
                            parts.push(SlateTemplatePart::Hole { expr: inner, src: js_trim(src).to_string(), at: self.base + pos });
                        }
                    }
                }
                let holes = parts.iter().filter(|p| matches!(p, SlateTemplatePart::Hole { .. })).count();
                if holes > SLATE_LIMITS.holes {
                    return Err(fault("X407", format!("{holes} holes in one template; the most is {}", SLATE_LIMITS.holes), t.at, None));
                }
                Ok(SlateExpr::Tpl { parts, at })
            }
            Kind::Own => Ok(SlateExpr::Path { head: t.v, own: true, segs: vec![], at }),
            Kind::Op if t.v == "(" => {
                let e = self.pipeline()?;
                self.expect(")")?;
                Ok(e)
            }
            Kind::Op if t.v == "[" => {
                let mut items: Vec<SlateExpr> = Vec::new();
                while !self.is_op("]") {
                    items.push(self.pipeline()?);
                    if !self.is_op(",") {
                        break;
                    }
                    self.next();
                }
                self.expect("]")?;
                Ok(SlateExpr::List { items, at })
            }
            Kind::Op if t.v == "{" => {
                let mut fields: Vec<(String, SlateExpr)> = Vec::new();
                while !self.is_op("}") {
                    let key = self.next();
                    if key.kind != Kind::Name && key.kind != Kind::Str {
                        return Err(fault(
                            "X400",
                            format!("a record's field is a name, at column {}: {{ at: time.now, v: 1 }}", key.at + 1),
                            key.at,
                            None,
                        ));
                    }
                    self.expect(":")?;
                    fields.push((key.v, self.pipeline()?));
                    if !self.is_op(",") {
                        break;
                    }
                    self.next();
                }
                self.expect("}")?;
                Ok(SlateExpr::Rec { fields, at })
            }
            Kind::Op if t.v == "/" => Err(fault("X420", "no regular expressions; use contains, startsWith or endsWith".into(), t.at, None)),
            Kind::Name => {
                match t.v.as_str() {
                    "true" => return Ok(SlateExpr::Lit { v: Value::Bool(true), at }),
                    "false" => return Ok(SlateExpr::Lit { v: Value::Bool(false), at }),
                    "null" | "undefined" => return Ok(SlateExpr::Lit { v: Value::Null, at }),
                    _ => {}
                }
                if is_reserved(&t.v) {
                    return Err(fault("X400", format!("{} cannot start a value (column {})", t.v, t.at + 1), t.at, None));
                }
                if self.is_op("(") {
                    self.next();
                    let mut args: Vec<SlateExpr> = Vec::new();
                    if !self.is_op(")") {
                        loop {
                            args.push(self.pipeline()?);
                            if self.is_op(",") {
                                self.next();
                                continue;
                            }
                            break;
                        }
                    }
                    self.expect(")")?;
                    return Ok(SlateExpr::Call { name: t.v, args, at });
                }
                Ok(SlateExpr::Path { head: t.v, own: false, segs: vec![], at })
            }
            _ => {
                let message = if t.kind == Kind::End {
                    "the expression ended early".to_string()
                } else {
                    format!("unexpected {} at column {}", t.v, t.at + 1)
                };
                Err(fault("X400", message, t.at, None))
            }
        }
    }
}

fn is_reserved(word: &str) -> bool {
    matches!(word, "and" | "or" | "not" | "true" | "false" | "null")
}

fn bin_op(v: &str) -> Option<SlateBinOp> {
    Some(match v {
        "+" => SlateBinOp::Add,
        "-" => SlateBinOp::Sub,
        "*" => SlateBinOp::Mul,
        "/" => SlateBinOp::Div,
        "%" => SlateBinOp::Rem,
        "==" => SlateBinOp::Eq,
        "!=" => SlateBinOp::Ne,
        "<" => SlateBinOp::Lt,
        "<=" => SlateBinOp::Le,
        ">" => SlateBinOp::Gt,
        ">=" => SlateBinOp::Ge,
        "and" => SlateBinOp::And,
        "or" => SlateBinOp::Or,
        _ => return None,
    })
}

fn parse_at(src: &[u16], base: usize) -> PResult<SlateExpr> {
    let result = if js_trim(&text(src)).is_empty() {
        Err(fault("X400", "a hole is empty".into(), 0, None))
    } else {
        tokenize(src).and_then(|toks| Parser { toks, i: 0, depth: 0, base }.parse_all())
    };
    result.map_err(|e| {
        if e.absolute {
            e
        } else {
            Fault { code: e.code, message: shift_columns(&e.message, base), at: e.at + base, fix: e.fix, absolute: true }
        }
    })
}

pub(super) fn problem(code: &str, message: &str, at: Option<usize>, fix: Option<&str>) -> SlateProblem {
    let mut out = slate_problem(code, message, fix);
    out.at = at.map(|a| a as i64);
    out
}

/// The syntax tree of an expression, or the first error it stops at. Positions are UTF-16 offsets into src.
pub fn parse_slate_expression(src: &str) -> SlateExprParse {
    let len = utf16::len(src);
    if len > SLATE_LIMITS.expr_chars {
        let message = format!("the expression is {len} characters; the most is {}", SLATE_LIMITS.expr_chars);
        return SlateExprParse { ast: None, errors: vec![problem("X407", &message, None, None)] };
    }
    if js_trim(src).is_empty() {
        return SlateExprParse { ast: None, errors: vec![problem("X400", "the expression is empty", Some(0), None)] };
    }
    match parse_at(&units(src), 0) {
        Ok(ast) => SlateExprParse { ast: Some(ast), errors: vec![] },
        Err(e) => SlateExprParse { ast: None, errors: vec![problem(e.code, &e.message, Some(e.at), e.fix.as_deref())] },
    }
}

/// The text of a path as dependency sets and resolvers name it: "pr.checks[0].name", "$check.json".
pub fn slate_path_text(head: &str, own: bool, segs: &[PathSeg]) -> String {
    let mut out = if own { format!("${head}") } else { head.to_string() };
    for seg in segs {
        match seg {
            PathSeg::Key(k) => {
                out.push('.');
                out.push_str(k);
            }
            PathSeg::Index(i) => out.push_str(&format!("[{i}]")),
        }
    }
    out
}

// ---- format strings ----

/// A format string's literal runs and its ${...} holes; "$${" is a literal "${".
pub fn parse_slate_format(src: &str) -> SlateFormatParse {
    let u = units(src);
    let mut parts: Vec<SlateFormatPart> = Vec::new();
    let mut lit: Vec<u16> = Vec::new();
    let mut i = 0;
    while i < u.len() {
        if starts(&u, i, "$${") {
            lit.extend("${".encode_utf16());
            i += 3;
            continue;
        }
        if starts(&u, i, "${") {
            let start = i + 2;
            let Some(end) = closing_brace_units(&u, start) else {
                let message = format!("a ${{ at column {} never closes", i + 1);
                return SlateFormatParse { parts, errors: vec![problem("X400", &message, Some(i), None)] };
            };
            if !lit.is_empty() {
                parts.push(SlateFormatPart::Text(text(&lit)));
                lit.clear();
            }
            parts.push(SlateFormatPart::Hole { expr: text(&u[start..end]), at: start });
            i = end + 1;
            continue;
        }
        lit.push(u[i]);
        i += 1;
    }
    if !lit.is_empty() {
        parts.push(SlateFormatPart::Text(text(&lit)));
    }
    let holes = parts.iter().filter(|p| matches!(p, SlateFormatPart::Hole { .. })).count();
    let errors = if holes > SLATE_LIMITS.holes {
        vec![problem("X407", &format!("{holes} holes in one text; the most is {}", SLATE_LIMITS.holes), None, None)]
    } else {
        vec![]
    };
    SlateFormatParse { parts, errors }
}
