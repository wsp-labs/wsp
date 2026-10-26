// SPDX-License-Identifier: AGPL-3.0-only
//! node's `util.parseArgs` as the command line calls it: `strict: true`, `allowPositionals: true`, no defaults, no
//! negatives. Its error messages are part of the command line's words (a parse refusal prints them), so they are
//! node 22's byte for byte. Spec: `spec/verbs-table.md`, "The parser".

use std::collections::VecDeque;

use crate::verbs::client::{FlagKind, FlagTable, FlagValue, Flags};

/// node's error codes for the two failures a strict parse with positionals allowed can raise.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParseArgsCode {
    /// `ERR_PARSE_ARGS_UNKNOWN_OPTION`
    UnknownOption,
    /// `ERR_PARSE_ARGS_INVALID_OPTION_VALUE`
    InvalidOptionValue,
}

/// A refused parse: node's code and its message, word for word.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ParseArgsError {
    pub code: ParseArgsCode,
    pub message: String,
}

impl std::fmt::Display for ParseArgsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for ParseArgsError {}

/// What a parse answers: the values, keyed in the order each flag was first met, and the positionals in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Parsed {
    pub values: Flags,
    pub positionals: Vec<String>,
}

/// One token of node's `argsToTokens`; node's `index` is not kept, since nothing here reads it.
enum Token {
    Option(OptionToken),
    Positional(String),
    Terminator,
}

struct OptionToken {
    name: String,
    raw_name: String,
    value: Option<String>,
    inline: bool,
}

/// `parseArgs({ args, options, allowPositionals: true, strict: true })`.
///
/// - `--` ends the options; every word after it is a positional, `--` itself dropped. `-` alone is a positional.
/// - `--name=value` and `--name value`; a string flag whose next word opens with `-` is refused as ambiguous
///   (`Option '--host' argument is ambiguous.\nDid you forget to specify the option argument for '--host'?\nTo
///   specify an option argument starting with a dash use '--host=-XYZ'.`); with a short letter the last line
///   reads `use '--<name>=-XYZ' or '-<s>-XYZ'.`
/// - A string flag at the end of the line: `Option '--host <value>' argument missing`, or
///   `Option '-m, --n <value>' argument missing` where it has a short letter.
/// - A boolean given `=value`: `Option '--json' does not take an argument`, or `Option '-h, --help' does not take
///   an argument` where it has a short letter.
/// - `-abc` is a group of short flags; a string short flag takes the rest of the word as its value (`-mfoo`), or
///   the next word when it ends the group (`-hm v`).
/// - An unknown flag: `Unknown option '<as typed, without =value>'. To specify a positional argument starting with a
///   '-', place it at the end of the command after '--', as in '-- "<as typed>"` (the closing quote
///   missing, as node writes it). A short letter in a group is reported as `-<letter>`.
/// - A repeated flag: the last value wins; a `multiple` flag collects every value in order.
pub fn parse_args(args: &[String], options: &FlagTable) -> Result<Parsed, ParseArgsError> {
    let tokens = args_to_tokens(args, options);
    let mut values = Flags::new();
    let mut positionals = Vec::new();
    for token in tokens {
        match token {
            Token::Option(token) => {
                check_option_usage(&token, options)?;
                check_option_like_value(&token)?;
                store_option(&token, options, &mut values);
            }
            Token::Positional(value) => positionals.push(value),
            Token::Terminator => {}
        }
    }
    Ok(Parsed { values, positionals })
}

fn units(text: &str) -> Vec<u16> {
    text.encode_utf16().collect()
}

fn text_of(units: &[u16]) -> String {
    String::from_utf16_lossy(units)
}

fn kind_of(options: &FlagTable, name: &str) -> Option<FlagKind> {
    options.get(name).map(|row| row.kind)
}

fn is_string_option(options: &FlagTable, name: &str) -> bool {
    kind_of(options, name) == Some(FlagKind::String)
}

/// `findLongOptionForShort`: the option whose `short` is this letter, else the letter itself.
fn find_long_for_short(short: &str, options: &FlagTable) -> String {
    options
        .0
        .iter()
        .find(|(_, row)| row.short.map(|c| c.to_string()).as_deref() == Some(short))
        .map_or_else(|| short.to_string(), |(name, _)| (*name).to_string())
}

fn is_lone_short_option(arg: &[u16]) -> bool {
    arg.len() == 2 && arg[0] == u16::from(b'-') && arg[1] != u16::from(b'-')
}

fn is_lone_long_option(arg: &[u16]) -> bool {
    arg.len() > 2 && arg.starts_with(&[u16::from(b'-'), u16::from(b'-')]) && !arg[3.min(arg.len())..].contains(&u16::from(b'='))
}

fn is_long_option_and_value(arg: &[u16]) -> bool {
    arg.len() > 2 && arg.starts_with(&[u16::from(b'-'), u16::from(b'-')]) && arg[3.min(arg.len())..].contains(&u16::from(b'='))
}

fn is_short_option_group(arg: &[u16], options: &FlagTable) -> bool {
    if arg.len() <= 2 || arg[0] != u16::from(b'-') || arg[1] == u16::from(b'-') {
        return false;
    }
    let long = find_long_for_short(&text_of(&arg[1..2]), options);
    kind_of(options, &long) != Some(FlagKind::String)
}

fn is_short_option_and_value(arg: &[u16], options: &FlagTable) -> bool {
    if arg.len() <= 2 || arg[0] != u16::from(b'-') || arg[1] == u16::from(b'-') {
        return false;
    }
    let long = find_long_for_short(&text_of(&arg[1..2]), options);
    is_string_option(options, &long)
}

/// `argsToTokens`: each word read into its token, a short group opened into its letters at the front of what is left.
fn args_to_tokens(args: &[String], options: &FlagTable) -> Vec<Token> {
    let mut tokens = Vec::new();
    let mut remaining: VecDeque<String> = args.iter().cloned().collect();
    while let Some(arg) = remaining.pop_front() {
        let next = remaining.front().cloned();
        if arg == "--" {
            tokens.push(Token::Terminator);
            tokens.extend(remaining.drain(..).map(Token::Positional));
            break;
        }
        let unit = units(&arg);
        if is_lone_short_option(&unit) {
            let short = text_of(&unit[1..2]);
            let name = find_long_for_short(&short, options);
            let value = if is_string_option(options, &name) && next.is_some() { remaining.pop_front() } else { None };
            tokens.push(Token::Option(OptionToken { name, raw_name: arg, value, inline: false }));
            continue;
        }
        if is_short_option_group(&unit, options) {
            let mut expanded: Vec<String> = Vec::new();
            for index in 1..unit.len() {
                let short = text_of(&unit[index..index + 1]);
                let name = find_long_for_short(&short, options);
                if !is_string_option(options, &name) || index == unit.len() - 1 {
                    expanded.push(format!("-{short}"));
                } else {
                    expanded.push(format!("-{}", text_of(&unit[index..])));
                    break;
                }
            }
            let mut front: VecDeque<String> = expanded.into();
            front.extend(remaining.drain(..));
            remaining = front;
            continue;
        }
        if is_short_option_and_value(&unit, options) {
            let short = text_of(&unit[1..2]);
            let name = find_long_for_short(&short, options);
            let value = text_of(&unit[2..]);
            tokens.push(Token::Option(OptionToken { name, raw_name: format!("-{short}"), value: Some(value), inline: true }));
            continue;
        }
        if is_lone_long_option(&unit) {
            let name = text_of(&unit[2..]);
            let value = if is_string_option(options, &name) && next.is_some() { remaining.pop_front() } else { None };
            tokens.push(Token::Option(OptionToken { name, raw_name: arg, value, inline: false }));
            continue;
        }
        if is_long_option_and_value(&unit) {
            let equal = unit.iter().position(|u| *u == u16::from(b'=')).unwrap_or(unit.len());
            let name = text_of(&unit[2..equal]);
            let value = text_of(&unit[(equal + 1).min(unit.len())..]);
            tokens.push(Token::Option(OptionToken { raw_name: format!("--{name}"), name, value: Some(value), inline: true }));
            continue;
        }
        tokens.push(Token::Positional(arg));
    }
    tokens
}

/// `checkOptionUsage`: an unknown flag, a string flag with no value, a boolean given one.
fn check_option_usage(token: &OptionToken, options: &FlagTable) -> Result<(), ParseArgsError> {
    let Some(row) = options.get(&token.name) else {
        return Err(unknown_option(&token.raw_name));
    };
    let short = row.short.map(|c| format!("-{c}, ")).unwrap_or_default();
    let short_and_long = format!("{short}--{}", token.name);
    match row.kind {
        FlagKind::String if token.value.is_none() => Err(invalid_value(format!("Option '{short_and_long} <value>' argument missing"))),
        FlagKind::Boolean if token.value.is_some() => Err(invalid_value(format!("Option '{short_and_long}' does not take an argument"))),
        _ => Ok(()),
    }
}

/// `checkOptionLikeValue`: a string flag's value that opens with a dash, typed as its own word.
fn check_option_like_value(token: &OptionToken) -> Result<(), ParseArgsError> {
    let Some(value) = token.value.as_deref() else { return Ok(()) };
    let like = !token.inline && units(value).len() > 1 && units(value)[0] == u16::from(b'-');
    if !like {
        return Ok(());
    }
    let raw = &token.raw_name;
    let example = if raw.starts_with("--") { format!("'{raw}=-XYZ'") } else { format!("'--{}=-XYZ' or '{raw}-XYZ'", token.name) };
    Err(invalid_value(format!(
        "Option '{raw}' argument is ambiguous.\nDid you forget to specify the option argument for '{raw}'?\nTo specify an option argument starting with a dash use {example}."
    )))
}

/// `storeOption`: a `multiple` flag collects its values in order; any other keeps the last one.
fn store_option(token: &OptionToken, options: &FlagTable, values: &mut Flags) {
    if token.name == "__proto__" {
        return;
    }
    let multiple = options.get(&token.name).is_some_and(|row| row.multiple);
    if multiple {
        let value = token.value.clone().unwrap_or_default();
        match values.get_mut(&token.name) {
            Some(FlagValue::List(list)) => list.push(value),
            _ => {
                values.insert(token.name.clone(), FlagValue::List(vec![value]));
            }
        }
        return;
    }
    let value = match &token.value {
        Some(text) => FlagValue::Str(text.clone()),
        None => FlagValue::Bool(true),
    };
    values.insert(token.name.clone(), value);
}

fn unknown_option(raw: &str) -> ParseArgsError {
    let quoted = serde_json::to_string(raw).expect("a string writes as JSON");
    ParseArgsError {
        code: ParseArgsCode::UnknownOption,
        message: format!(
            "Unknown option '{raw}'. To specify a positional argument starting with a '-', place it at the end of the command after '--', as in '-- {quoted}"
        ),
    }
}

fn invalid_value(message: String) -> ParseArgsError {
    ParseArgsError { code: ParseArgsCode::InvalidOptionValue, message }
}
