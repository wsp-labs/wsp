// SPDX-License-Identifier: AGPL-3.0-only
//! TeX parsed into atoms with KaTeX's classes and faces, or the first thing this typesetter does not set.

use super::tables::SYMBOLS;
use super::{Class, Face, Style, DISPLAY, TEXT};

/// A parsed piece of TeX.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Atom {
    /// A glyph; `text` glyphs (`\text`) take no italic correction.
    Sym {
        ch: char,
        class: Class,
        face: Face,
        text: bool,
    },
    /// `{...}`, and `\mathrel{...}` and its kin, which give the group their class.
    Group(Class, Vec<Atom>),
    Scripts {
        base: Box<Atom>,
        sup: Option<Vec<Atom>>,
        sub: Option<Vec<Atom>>,
    },
    Frac {
        num: Vec<Atom>,
        den: Vec<Atom>,
        bar: bool,
        parens: bool,
        style: Option<Style>,
    },
    Sqrt(Vec<Atom>),
    /// A big operator's glyph (`\sum`), or a named one's letters (`\lim`).
    Op {
        glyph: Option<char>,
        name: Vec<Atom>,
        limits: bool,
    },
    LeftRight {
        left: Option<char>,
        right: Option<char>,
        body: Vec<Atom>,
    },
    /// `\big(` and its kin: a delimiter of size 1 to 4.
    Big {
        delim: Option<char>,
        size: usize,
        class: Class,
    },
    Kern(Len),
    Style(Style),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Len {
    Mu(f32),
    /// An em of the current style.
    Em(f32),
    /// An em of the formula, as `\quad` is in every style.
    Quad(f32),
}

/// What `\mathrm`, `\mathbf` and plain math do to a letter or a digit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Variant {
    Normal,
    Rm,
    Bf,
}

#[derive(Clone, Debug, PartialEq)]
enum Token {
    Char(char),
    Command(String),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum End {
    Eof,
    Brace,
    Right,
}

/// What stopped the parse: the command or character this typesetter does not set.
type Parsed<T> = Result<T, String>;

const BIG_OPS: [(&str, char, bool); 11] = [
    ("sum", '\u{2211}', true),
    ("prod", '\u{220f}', true),
    ("coprod", '\u{2210}', true),
    ("bigcup", '\u{22c3}', true),
    ("bigcap", '\u{22c2}', true),
    ("bigoplus", '\u{2a01}', true),
    ("bigotimes", '\u{2a02}', true),
    ("bigodot", '\u{2a00}', true),
    ("int", '\u{222b}', false),
    ("iint", '\u{222c}', false),
    ("oint", '\u{222e}', false),
];

/// KaTeX's named operators (`op.ts`): their letters, a space for a thin space, and whether display sets limits.
const NAMED_OPS: [(&str, &str, bool); 32] = [
    ("arccos", "arccos", false),
    ("arcsin", "arcsin", false),
    ("arctan", "arctan", false),
    ("arg", "arg", false),
    ("cos", "cos", false),
    ("cosh", "cosh", false),
    ("cot", "cot", false),
    ("coth", "coth", false),
    ("csc", "csc", false),
    ("deg", "deg", false),
    ("dim", "dim", false),
    ("exp", "exp", false),
    ("hom", "hom", false),
    ("ker", "ker", false),
    ("lg", "lg", false),
    ("ln", "ln", false),
    ("log", "log", false),
    ("sec", "sec", false),
    ("sin", "sin", false),
    ("sinh", "sinh", false),
    ("tan", "tan", false),
    ("tanh", "tanh", false),
    ("det", "det", true),
    ("gcd", "gcd", true),
    ("inf", "inf", true),
    ("lim", "lim", true),
    ("liminf", "lim inf", true),
    ("limsup", "lim sup", true),
    ("max", "max", true),
    ("min", "min", true),
    ("Pr", "Pr", true),
    ("sup", "sup", true),
];

/// How deep an agent's groups may nest before the formula shows as its TeX: deeper would overflow a parse thread's stack.
pub(super) const MAX_NESTING: usize = 64;

struct Parser {
    chars: Vec<char>,
    at: usize,
    depth: usize,
}

/// TeX to atoms, or the first thing this typesetter does not set.
pub(super) fn parse(tex: &str) -> Parsed<Vec<Atom>> {
    let mut parser = Parser { chars: tex.chars().collect(), at: 0, depth: 0 };
    parser.list(End::Eof, Variant::Normal)
}

impl Parser {
    fn skip_space(&mut self) {
        while let Some(&c) = self.chars.get(self.at) {
            if c == '%' {
                while self.chars.get(self.at).is_some_and(|&c| c != '\n') {
                    self.at += 1;
                }
            } else if c.is_whitespace() {
                self.at += 1;
            } else {
                break;
            }
        }
    }

    fn next(&mut self) -> Parsed<Option<Token>> {
        self.skip_space();
        let Some(&c) = self.chars.get(self.at) else { return Ok(None) };
        self.at += 1;
        if c != '\\' {
            return Ok(Some(Token::Char(c)));
        }
        let start = self.at;
        while self.chars.get(self.at).is_some_and(|c| c.is_ascii_alphabetic()) {
            self.at += 1;
        }
        if self.at == start {
            let &c = self.chars.get(self.at).ok_or("\\")?;
            self.at += 1;
            return Ok(Some(Token::Command(c.to_string())));
        }
        Ok(Some(Token::Command(self.chars[start..self.at].iter().collect())))
    }

    fn peek(&mut self) -> Parsed<Option<Token>> {
        let at = self.at;
        let token = self.next();
        self.at = at;
        token
    }

    fn list(&mut self, end: End, variant: Variant) -> Parsed<Vec<Atom>> {
        self.deeper(|parser| parser.items(end, variant))
    }

    /// Parses `inner` one level deeper: a group, or an argument that is itself a command (`\sqrt\sqrt x`).
    fn deeper<T>(&mut self, inner: impl FnOnce(&mut Self) -> Parsed<T>) -> Parsed<T> {
        if self.depth == MAX_NESTING {
            return Err("groups nested too deep".into());
        }
        self.depth += 1;
        let parsed = inner(self);
        self.depth -= 1;
        parsed
    }

    fn items(&mut self, end: End, variant: Variant) -> Parsed<Vec<Atom>> {
        let mut atoms = Vec::new();
        loop {
            let Some(token) = self.peek()? else {
                return if end == End::Eof { Ok(atoms) } else { Err("an unclosed group".into()) };
            };
            match &token {
                Token::Char('}') => {
                    self.next()?;
                    return if end == End::Brace { Ok(atoms) } else { Err("}".into()) };
                }
                Token::Command(name) if name == "right" => {
                    return if end == End::Right { Ok(atoms) } else { Err("\\right".into()) };
                }
                Token::Char(c @ ('^' | '_')) => {
                    let sup = *c == '^';
                    self.next()?;
                    let script = self.argument(variant)?;
                    attach(&mut atoms, sup, script)?;
                }
                Token::Char('\'') => {
                    let mut primes = Vec::new();
                    while self.peek()? == Some(Token::Char('\'')) {
                        self.next()?;
                        primes.push(symbol("\\prime").ok_or("\\prime")?);
                    }
                    if self.peek()? == Some(Token::Char('^')) {
                        self.next()?;
                        primes.extend(self.argument(variant)?);
                    }
                    attach(&mut atoms, true, primes)?;
                }
                Token::Command(name) if name == "limits" || name == "nolimits" => {
                    self.next()?;
                    let Some(Atom::Op { limits, .. }) = atoms.last_mut() else { return Err(format!("\\{name}")) };
                    *limits = name == "limits";
                }
                _ => {
                    self.next()?;
                    atoms.push(self.atom(token, variant)?);
                }
            }
        }
    }

    /// A command's argument or a script: a `{...}` group, or one atom.
    fn argument(&mut self, variant: Variant) -> Parsed<Vec<Atom>> {
        match self.next()? {
            Some(Token::Char('{')) => self.list(End::Brace, variant),
            Some(Token::Char(c @ ('}' | '^' | '_' | '&'))) => Err(c.to_string()),
            Some(token) => self.deeper(|parser| Ok(vec![parser.atom(token, variant)?])),
            None => Err("a missing argument".into()),
        }
    }

    fn atom(&mut self, token: Token, variant: Variant) -> Parsed<Atom> {
        match token {
            Token::Char('{') => Ok(Atom::Group(Class::Ord, self.list(End::Brace, variant)?)),
            Token::Char(c) if c.is_ascii_alphanumeric() => {
                let face = match variant {
                    Variant::Bf => Face::MainBold,
                    Variant::Rm => Face::Main,
                    Variant::Normal if c.is_ascii_digit() => Face::Main,
                    Variant::Normal => Face::MathItalic,
                };
                Ok(Atom::Sym { ch: c, class: Class::Ord, face, text: false })
            }
            Token::Char('~') => Ok(Atom::Kern(Len::Em(0.25))),
            Token::Char(c) => {
                let named = symbol(c.encode_utf8(&mut [0; 4]));
                named.or_else(|| SYMBOLS.iter().find(|row| row.0.starts_with('\\') && row.1 == c).map(sym)).ok_or_else(|| c.to_string())
            }
            Token::Command(name) => self.command(&name, variant),
        }
    }

    fn command(&mut self, name: &str, variant: Variant) -> Parsed<Atom> {
        let class = |class: Class, list: Vec<Atom>| Atom::Group(class, list);
        Ok(match name {
            "frac" | "dfrac" | "tfrac" | "binom" | "dbinom" | "tbinom" => {
                let (num, den) = (self.argument(variant)?, self.argument(variant)?);
                let style = match name.as_bytes()[0] {
                    b'd' => Some(DISPLAY),
                    b't' => Some(TEXT),
                    _ => None,
                };
                let bar = name.ends_with("frac");
                Atom::Frac { num, den, bar, parens: !bar, style }
            }
            "sqrt" => {
                if self.peek()? == Some(Token::Char('[')) {
                    return Err("\\sqrt[".into());
                }
                Atom::Sqrt(self.argument(variant)?)
            }
            "left" => {
                let left = self.delimiter()?;
                let body = self.list(End::Right, variant)?;
                self.next()?;
                let right = self.delimiter()?;
                Atom::LeftRight { left, right, body }
            }
            "text" | "textrm" | "textnormal" | "textup" | "mbox" => class(Class::Ord, self.text(Face::Main)?),
            "textbf" => class(Class::Ord, self.text(Face::MainBold)?),
            "mathrm" => class(Class::Ord, self.argument(Variant::Rm)?),
            "mathbf" => class(Class::Ord, self.argument(Variant::Bf)?),
            "mathit" | "mathnormal" => class(Class::Ord, self.argument(Variant::Normal)?),
            "operatorname" => {
                let limits = self.peek()? == Some(Token::Char('*'));
                if limits {
                    self.next()?;
                }
                Atom::Op { glyph: None, name: self.argument(Variant::Rm)?, limits }
            }
            "mathord" => class(Class::Ord, self.argument(variant)?),
            "mathop" => class(Class::Op, self.argument(variant)?),
            "mathbin" => class(Class::Bin, self.argument(variant)?),
            "mathrel" => class(Class::Rel, self.argument(variant)?),
            "mathopen" => class(Class::Open, self.argument(variant)?),
            "mathclose" => class(Class::Close, self.argument(variant)?),
            "mathpunct" => class(Class::Punct, self.argument(variant)?),
            "mathinner" => class(Class::Inner, self.argument(variant)?),
            "displaystyle" => Atom::Style(DISPLAY),
            "textstyle" => Atom::Style(TEXT),
            "scriptstyle" => Atom::Style(Style { size: 2, cramped: false }),
            "scriptscriptstyle" => Atom::Style(Style { size: 3, cramped: false }),
            "," | "thinspace" => Atom::Kern(Len::Mu(3.)),
            ":" | ">" | "medspace" => Atom::Kern(Len::Mu(4.)),
            ";" | "thickspace" => Atom::Kern(Len::Mu(5.)),
            "!" | "negthinspace" => Atom::Kern(Len::Mu(-3.)),
            " " | "space" | "nobreakspace" => Atom::Kern(Len::Em(0.25)),
            "quad" => Atom::Kern(Len::Quad(1.)),
            "qquad" => Atom::Kern(Len::Quad(2.)),
            "neq" | "ne" => class(Class::Rel, vec![symbol("\\@not").ok_or("\\@not")?, symbol("=").ok_or("=")?]),
            "colon" => class(Class::Punct, vec![symbol(":").ok_or(":")?]),
            "cdots" | "dotsb" | "dotsm" | "dotsi" => symbol("\\@cdots").ok_or("\\@cdots")?,
            "dots" | "dotsc" | "dotso" => symbol("\\ldots").ok_or("\\ldots")?,
            "vert" => symbol("|").ok_or("|")?,
            "Vert" => symbol("\\|").ok_or("\\|")?,
            _ => {
                if let Some(&(_, glyph, limits)) = BIG_OPS.iter().find(|op| op.0 == name) {
                    return Ok(Atom::Op { glyph: Some(glyph), name: Vec::new(), limits });
                }
                if let Some(&(_, letters, limits)) = NAMED_OPS.iter().find(|op| op.0 == name) {
                    let name = letters
                        .chars()
                        .map(|ch| {
                            if ch == ' ' {
                                Atom::Kern(Len::Mu(3.))
                            } else {
                                Atom::Sym { ch, class: Class::Ord, face: Face::Main, text: false }
                            }
                        })
                        .collect();
                    return Ok(Atom::Op { glyph: None, name, limits });
                }
                if let Some(size) = big(name) {
                    let class = match name.as_bytes()[name.len() - 1] {
                        b'l' => Class::Open,
                        b'r' => Class::Close,
                        b'm' => Class::Rel,
                        _ => Class::Ord,
                    };
                    return Ok(Atom::Big { delim: self.delimiter()?, size, class });
                }
                symbol(&format!("\\{name}")).ok_or_else(|| format!("\\{name}"))?
            }
        })
    }

    /// A delimiter after `\left`, `\right` or `\big`: its glyph, or none for `.`.
    fn delimiter(&mut self) -> Parsed<Option<char>> {
        let token = self.next()?.ok_or("a missing delimiter")?;
        let ch = match &token {
            Token::Char('.') => return Ok(None),
            Token::Char(c @ ('(' | ')' | '[' | ']' | '/')) => *c,
            Token::Char('|') => '\u{2223}',
            Token::Char('<') => '\u{27e8}',
            Token::Char('>') => '\u{27e9}',
            Token::Command(name) => match name.as_str() {
                "{" | "lbrace" => '{',
                "}" | "rbrace" => '}',
                "lbrack" => '[',
                "rbrack" => ']',
                "lparen" => '(',
                "rparen" => ')',
                "langle" => '\u{27e8}',
                "rangle" => '\u{27e9}',
                "lfloor" => '\u{230a}',
                "rfloor" => '\u{230b}',
                "lceil" => '\u{2308}',
                "rceil" => '\u{2309}',
                "vert" | "lvert" | "rvert" => '\u{2223}',
                "|" | "Vert" | "lVert" | "rVert" => '\u{2225}',
                "backslash" => '\\',
                other => return Err(format!("\\{other} as a delimiter")),
            },
            Token::Char(other) => return Err(format!("{other} as a delimiter")),
        };
        Ok(Some(ch))
    }

    /// `\text{...}`'s characters, upright in `face`, its spaces a quarter em each.
    fn text(&mut self, face: Face) -> Parsed<Vec<Atom>> {
        self.skip_space();
        if self.chars.get(self.at) != Some(&'{') {
            return Err("\\text without braces".into());
        }
        self.at += 1;
        let mut atoms = Vec::new();
        let mut space = false;
        loop {
            let &c = self.chars.get(self.at).ok_or("an unclosed \\text")?;
            self.at += 1;
            let ch = match c {
                '}' => return Ok(atoms),
                '{' | '$' | '^' | '_' => return Err(format!("{c} in \\text")),
                '\\' => {
                    let &escaped = self.chars.get(self.at).ok_or("\\")?;
                    self.at += 1;
                    if !"$%&_#{} ".contains(escaped) {
                        return Err(format!("\\{escaped} in \\text"));
                    }
                    escaped
                }
                c => c,
            };
            if ch.is_whitespace() {
                if !space {
                    atoms.push(Atom::Sym { ch: '\u{a0}', class: Class::Ord, face: Face::Main, text: true });
                }
                space = true;
                continue;
            }
            space = false;
            face.metrics(ch).ok_or_else(|| format!("{ch} in \\text"))?;
            atoms.push(Atom::Sym { ch, class: Class::Ord, face, text: true });
        }
    }
}

/// `\big`, `\Big`, `\bigg` and `\Bigg`, with or without `l`, `r` or `m`: the delimiter size each asks for.
fn big(name: &str) -> Option<usize> {
    let stem = name.strip_suffix(['l', 'r', 'm']).unwrap_or(name);
    ["big", "Big", "bigg", "Bigg"].iter().position(|&s| s == stem).map(|ix| ix + 1)
}

/// A script on the atom before it: `{}` stands in for a script with nothing before it.
fn attach(atoms: &mut Vec<Atom>, sup: bool, script: Vec<Atom>) -> Parsed<()> {
    let (base, mut sups, mut subs) = match atoms.pop() {
        Some(Atom::Scripts { base, sup, sub }) => (base, sup, sub),
        Some(atom) => (Box::new(atom), None, None),
        None => (Box::new(Atom::Group(Class::Ord, Vec::new())), None, None),
    };
    let slot = if sup { &mut sups } else { &mut subs };
    if slot.is_some() {
        return Err(if sup { "a double superscript" } else { "a double subscript" }.into());
    }
    *slot = Some(script);
    atoms.push(Atom::Scripts { base, sup: sups, sub: subs });
    Ok(())
}

fn sym(row: &(&str, char, Class, Face)) -> Atom {
    Atom::Sym { ch: row.1, class: row.2, face: row.3, text: false }
}

fn symbol(name: &str) -> Option<Atom> {
    SYMBOLS.binary_search_by_key(&name, |row| row.0).ok().map(|ix| sym(&SYMBOLS[ix]))
}
