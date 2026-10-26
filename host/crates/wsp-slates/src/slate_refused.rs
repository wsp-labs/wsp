// SPDX-License-Identifier: AGPL-3.0-only
//! Unit helpers: slate-refused.ts at 65f6b89c7. Spec: spec/helpers.md, Behaviour 9.
use wsp_words::exit::{HostError, Kind};

use crate::pending::slate::{SlateProblem, SLATE_LIMITS};

const ERRORS_LISTED: usize = SLATE_LIMITS.errors_per_pass;

/// Kind `invalid`, the first 20 errors and every warning as JSON, no fix.
pub fn invalid(errors: &[SlateProblem], warnings: &[SlateProblem]) -> HostError {
    let listed = &errors[..errors.len().min(ERRORS_LISTED)];
    let more = if errors.len() > ERRORS_LISTED { format!(" And {} more.", errors.len() - ERRORS_LISTED) } else { String::new() };
    let plural = if errors.len() == 1 { "" } else { "s" };
    let body = if listed.len() == 1 {
        said(&listed[0])
    } else {
        listed.iter().enumerate().map(|(i, p)| format!("{}) {}", i + 1, said(p))).collect::<Vec<_>>().join(" ")
    };
    let message = format!("slate refused: {} error{plural}: {body}{more}", errors.len());
    HostError {
        kind: Some(Kind::Invalid),
        errors: Some(listed.iter().map(to_json).collect()),
        warnings: Some(warnings.iter().map(to_json).collect()),
        ..HostError::new(message)
    }
}

fn to_json(p: &SlateProblem) -> serde_json::Value {
    serde_json::to_value(p).expect("a problem serializes")
}

fn said(p: &SlateProblem) -> String {
    let mut place = Vec::new();
    if let Some(line) = p.line {
        place.push(format!("line {line}"));
    }
    if let Some(piece) = &p.piece {
        place.push(match &p.prop {
            Some(prop) => format!("{piece}.{prop}"),
            None => piece.clone(),
        });
    }
    let prefix = if place.is_empty() { String::new() } else { format!("{}: ", place.join(", ")) };
    let fix = p.fix.as_ref().map_or(String::new(), |f| format!(". Did you mean {f}?"));
    format!("{prefix}{} {} {}{fix}", p.code, p.name, p.message)
}
