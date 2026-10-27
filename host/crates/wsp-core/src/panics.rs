// SPDX-License-Identifier: AGPL-3.0-only
//! What a panic carried, for the places where a task that dies must answer as the TS's rejected promise does.
use std::any::Any;

/// The message of a panic payload: the text of `panic!("..")`, or a fixed line where it carried none.
pub fn words(payload: Box<dyn Any + Send>) -> String {
    match payload.downcast::<String>() {
        Ok(words) => *words,
        Err(payload) => payload.downcast_ref::<&str>().map_or_else(|| "the task panicked".to_string(), |words| (*words).to_string()),
    }
}
