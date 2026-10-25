// SPDX-License-Identifier: AGPL-3.0-only
//! The host errors a test builds, and the TS words of a kind, written from the fields so no wsp-words body is needed.
//! Both test halves (`common` and `support/base.rs`) include this one file.
#![allow(dead_code)]

use wsp_words::exit::{HostError, Kind};

/// `new Error(message)`, built from fields so no wsp-words body is needed.
pub fn plain(message: &str) -> HostError {
    HostError { message: message.to_owned(), fix: None, kind: None, errors: None, warnings: None, left: None, host_key: None, code: None, status: None, marks: vec![] }
}

/// `Object.assign(new Error(message), { kind })`.
pub fn kinded(message: &str, kind: Kind) -> HostError {
    HostError { kind: Some(kind), ..plain(message) }
}

/// A refusal: `refusalLine(said, fix)` as the message, the fix kept apart.
pub fn refused(message: &str, fix: &str, kind: Kind) -> HostError {
    HostError { fix: Some(fix.to_owned()), kind: Some(kind), ..plain(message) }
}

/// The `Kind` a TS kind string names, read without wsp-words' `Kind::parse` body.
pub fn kind_named(word: &str) -> Kind {
    match word {
        "usage" => Kind::Usage,
        "invalid" => Kind::Invalid,
        "not-found" => Kind::NotFound,
        "conflict" => Kind::Conflict,
        "auth" => Kind::Auth,
        "missing" => Kind::Missing,
        "machineAlive" => Kind::MachineAlive,
        other => Kind::Other(other.to_owned()),
    }
}

/// The TS word of a kind, written here so a test reads it without wsp-words' own table.
pub fn kind_word(kind: &Kind) -> String {
    match kind {
        Kind::Usage => "usage",
        Kind::Invalid => "invalid",
        Kind::NotFound => "not-found",
        Kind::ProcessesLeft => "processes-left",
        Kind::Auth => "auth",
        Kind::Ticket => "ticket",
        Kind::Conflict => "conflict",
        Kind::Concurrency => "concurrency",
        Kind::StartLimit => "startLimit",
        Kind::Plan => "plan",
        Kind::Missing => "missing",
        Kind::SnapshotUnavailable => "snapshotUnavailable",
        Kind::Transient => "transient",
        Kind::Unknown => "unknown",
        Kind::NotFirstLife => "notFirstLife",
        Kind::MachineAlive => "machineAlive",
        Kind::SnapshotFailed => "snapshotFailed",
        Kind::CredentialOnBuilder => "credentialOnBuilder",
        Kind::NoProvider => "noProvider",
        Kind::VaultTooLarge => "vaultTooLarge",
        Kind::Exists => "exists",
        Kind::SshInclude => "sshInclude",
        Kind::HostKey => "host-key",
        Kind::SshBehind => "sshBehind",
        Kind::Unreachable => "unreachable",
        Kind::Refused => "refused",
        Kind::Reauth => "reauth",
        Kind::Other(s) => s,
    }
    .to_string()
}
