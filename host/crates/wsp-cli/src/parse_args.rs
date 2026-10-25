// SPDX-License-Identifier: AGPL-3.0-only
//! node's `util.parseArgs` as the command line calls it: `strict: true`, `allowPositionals: true`, no defaults, no
//! negatives. Its error messages are part of the command line's words (a parse refusal prints them), so they are
//! node 22's byte for byte. Spec: `spec/verbs-table.md`, "The parser".

use crate::verbs::client::{FlagTable, Flags};

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
/// - An unknown flag: `Unknown option '<as typed, without =value>'. To specify a positional argument starting
///   with a '-', place it at the end of the command after '--', as in '-- "<as typed>"` (the closing quote
///   missing, as node writes it). A short letter in a group is reported as `-<letter>`.
/// - A repeated flag: the last value wins; a `multiple` flag collects every value in order.
pub fn parse_args(args: &[String], options: &FlagTable) -> Result<Parsed, ParseArgsError> {
    let _ = (args, options);
    todo!("node:util parseArgs, strict, positionals allowed")
}
