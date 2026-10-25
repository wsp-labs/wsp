// SPDX-License-Identifier: AGPL-3.0-only
//! `vi.spyOn(console, "warn")`: every line the backend warned, in order.
use std::sync::{Arc, Mutex};

use wsp_engine::box_backend::Warn;

#[derive(Clone, Default)]
pub struct Warned(pub Arc<Mutex<Vec<String>>>);

impl Warned {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn sink(&self) -> Warn {
        let lines = self.0.clone();
        Arc::new(move |line: &str| lines.lock().unwrap().push(line.to_string()))
    }
    pub fn lines(&self) -> Vec<String> {
        self.0.lock().unwrap().clone()
    }
}
