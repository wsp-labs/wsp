// SPDX-License-Identifier: AGPL-3.0-only
//! TeX as readable text, for a copy and for a formula this typesetter does not set.

use super::parse::MAX_NESTING;

/// TeX as readable text: what copying a formula gives, and what shows when it cannot be set.
pub(super) fn tex_text(tex: &str) -> String {
    let depth = tex.chars().scan(0i64, |d, c| {
        *d += match c {
            '{' => 1,
            '}' => -1,
            _ => 0,
        };
        Some(*d)
    });
    if depth.max().unwrap_or(0) > MAX_NESTING as i64 {
        return tex.trim().to_owned();
    }
    let mut out = String::new();
    let mut chars = tex.trim().chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                let mut name = String::new();
                while let Some(&n) = chars.peek().filter(|n| n.is_ascii_alphabetic()) {
                    name.push(n);
                    chars.next();
                }
                if name == "begin" || name == "end" {
                    group(&mut chars);
                    continue;
                }
                if name.is_empty() {
                    match chars.next() {
                        Some(',' | ';' | ':' | ' ' | '\\') => out.push(' '),
                        Some('!') => {}
                        Some(other) => out.push(other),
                        None => {}
                    }
                    continue;
                }
                if name == "frac" {
                    let (top, bottom) = (group(&mut chars), group(&mut chars));
                    out.push_str(&format!("{}/{}", tex_text(&top), tex_text(&bottom)));
                } else if name == "sqrt" {
                    out.push('√');
                    out.push_str(&tex_text(&group(&mut chars)));
                } else if let Some(symbol) = text_symbol(&name) {
                    out.push_str(symbol);
                } else {
                    out.push_str(&name);
                }
            }
            '^' | '_' => {
                let script = tex_text(&group(&mut chars));
                let mapped: Option<String> = script.chars().map(|s| if c == '^' { superscript(s) } else { subscript(s) }).collect();
                match mapped {
                    Some(mapped) => out.push_str(&mapped),
                    None => {
                        out.push(c);
                        out.push_str(&script);
                    }
                }
            }
            '{' | '}' => {}
            '\n' | '&' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// A command's argument: a `{...}` group, a command name, or one character.
fn group(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> String {
    while chars.next_if(|c| c.is_whitespace()).is_some() {}
    match chars.next() {
        Some('{') => {
            let mut depth = 1;
            let mut inner = String::new();
            for c in chars.by_ref() {
                depth += match c {
                    '{' => 1,
                    '}' => -1,
                    _ => 0,
                };
                if depth == 0 {
                    break;
                }
                inner.push(c);
            }
            inner
        }
        Some('\\') => {
            let mut name = String::from('\\');
            while let Some(n) = chars.next_if(|n| n.is_ascii_alphabetic()) {
                name.push(n);
            }
            name
        }
        Some(c) => c.to_string(),
        None => String::new(),
    }
}

fn text_symbol(name: &str) -> Option<&'static str> {
    Some(match name {
        "alpha" => "α",
        "beta" => "β",
        "gamma" => "γ",
        "delta" => "δ",
        "epsilon" | "varepsilon" => "ε",
        "theta" => "θ",
        "lambda" => "λ",
        "mu" => "μ",
        "pi" => "π",
        "sigma" => "σ",
        "tau" => "τ",
        "phi" | "varphi" => "φ",
        "omega" => "ω",
        "Delta" => "Δ",
        "Sigma" => "Σ",
        "Omega" => "Ω",
        "int" => "∫",
        "sum" => "∑",
        "prod" => "∏",
        "infty" => "∞",
        "cdot" => "·",
        "times" => "×",
        "pm" => "±",
        "le" | "leq" => "≤",
        "ge" | "geq" => "≥",
        "ne" | "neq" => "≠",
        "approx" => "≈",
        "to" | "rightarrow" => "→",
        "Rightarrow" | "implies" => "⇒",
        "in" => "∈",
        "partial" => "∂",
        "nabla" => "∇",
        "ldots" | "dots" | "cdots" => "…",
        "quad" | "qquad" => " ",
        "left" | "right" | "mathrm" | "mathbf" | "mathit" | "text" | "displaystyle" => "",
        _ => return None,
    })
}

fn superscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '⁰',
        '1' => '¹',
        '2' => '²',
        '3' => '³',
        '4' => '⁴',
        '5' => '⁵',
        '6' => '⁶',
        '7' => '⁷',
        '8' => '⁸',
        '9' => '⁹',
        '+' => '⁺',
        '-' => '⁻',
        '=' => '⁼',
        '(' => '⁽',
        ')' => '⁾',
        'n' => 'ⁿ',
        'i' => 'ⁱ',
        _ => return None,
    })
}

fn subscript(c: char) -> Option<char> {
    Some(match c {
        '0' => '₀',
        '1' => '₁',
        '2' => '₂',
        '3' => '₃',
        '4' => '₄',
        '5' => '₅',
        '6' => '₆',
        '7' => '₇',
        '8' => '₈',
        '9' => '₉',
        '+' => '₊',
        '-' => '₋',
        '=' => '₌',
        '(' => '₍',
        ')' => '₎',
        'i' => 'ᵢ',
        'j' => 'ⱼ',
        'n' => 'ₙ',
        'x' => 'ₓ',
        _ => return None,
    })
}
