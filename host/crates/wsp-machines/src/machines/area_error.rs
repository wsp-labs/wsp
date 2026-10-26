// SPDX-License-Identifier: AGPL-3.0-only
//! A failure as the machines-b areas pass it between them. `wsp-words`' `HostError` has no `status` or `cause` and
//! `wsp-engine` has no `From<EngineError> for HostError` yet, so an engine road's own error is kept whole beside the
//! host error until both land; then this file goes and `AreaResult<T>` becomes `Result<T, HostError>`.
use wsp_engine::errors::{EngineError, ErrorKind};
use wsp_words::exit::{HostError, Kind};

/// An engine road's own error, whole so `fault_of` and `is_missing` can read its variant, or a host error with its
/// kind. The TS throws either through the same `catch`.
#[derive(Debug, Clone)]
pub enum AreaError {
    Engine(EngineError),
    Host(HostError),
}

impl AreaError {
    /// `e instanceof Error ? e.message : String(e)`: the engine error's Display, or the host error's message.
    pub fn message(&self) -> String {
        match self {
            AreaError::Engine(e) => e.to_string(),
            AreaError::Host(e) => e.message.clone(),
        }
    }
    /// The kind stamped on it: the engine error's kind string, or the host error's kind, as the TS string.
    pub fn kind(&self) -> Option<String> {
        match self {
            AreaError::Engine(e) => e.kind().map(engine_kind_word),
            AreaError::Host(e) => e.kind.as_ref().map(|k| k.as_str().to_string()),
        }
    }
    /// The `fix` a refusal carries; none for an engine error.
    pub fn fix(&self) -> Option<String> {
        match self {
            AreaError::Engine(_) => None,
            AreaError::Host(e) => e.fix.clone(),
        }
    }
    /// engine errors.ts `isMissing`: the engine's own reading, or a host error of kind `missing`.
    pub fn is_missing(&self) -> bool {
        match self {
            AreaError::Engine(e) => wsp_engine::errors::is_missing(e),
            AreaError::Host(e) => e.kind.as_ref().is_some_and(|k| k.as_str() == "missing"),
        }
    }
    /// types/internal.ts:879 `isCapRefusal`: kind `concurrency`.
    pub fn is_cap_refusal(&self) -> bool {
        match self {
            AreaError::Engine(e) => wsp_core::types::internal::is_cap_refusal(e),
            AreaError::Host(e) => e.kind.as_ref().is_some_and(|k| k.as_str() == "concurrency"),
        }
    }
    /// engine errors.ts `failureWords` for an engine error; the message for a host error.
    pub fn failure_words(&self) -> String {
        match self {
            AreaError::Engine(e) => wsp_engine::errors::failure_words(e),
            AreaError::Host(e) => e.message.clone(),
        }
    }
    /// At the area's edge: the same words and kind as a `HostError`.
    pub fn into_host(self) -> HostError {
        match self {
            AreaError::Host(e) => e,
            AreaError::Engine(e) => {
                let mut host = HostError::new(e.to_string());
                host.kind = e.kind().map(|k| Kind::parse(&engine_kind_word(k)));
                host.status = e.status().map(f64::from);
                host.code = e.code().map(str::to_string);
                host
            }
        }
    }
}

fn engine_kind_word(kind: ErrorKind) -> String {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

impl From<HostError> for AreaError {
    fn from(e: HostError) -> Self {
        AreaError::Host(e)
    }
}

impl From<EngineError> for AreaError {
    fn from(e: EngineError) -> Self {
        AreaError::Engine(e)
    }
}

pub type AreaResult<T> = Result<T, AreaError>;
